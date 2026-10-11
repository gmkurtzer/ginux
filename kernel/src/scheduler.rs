use core::mem::MaybeUninit;
use core::sync::atomic::{AtomicBool, Ordering};

use crate::domain;

const MAX_THREADS: usize = 8;
pub(crate) const IPC_PEER_CLOSED: u64 = u64::MAX - 4;

static RUNTIME_INITIALIZED: AtomicBool = AtomicBool::new(false);
static mut RUNTIME: MaybeUninit<Runtime> = MaybeUninit::uninit();

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ThreadId {
    slot: u16,
    generation: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum State {
    Ready,
    Running,
    Blocked,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WaitReason {
    IpcReceive {
        object: u64,
        service_side: bool,
        peer_domain: u64,
    },
}

#[derive(Clone, Copy)]
struct Thread {
    id: ThreadId,
    domain: u64,
    state: State,
    wait: Option<WaitReason>,
    context: UserContext,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
struct UserContext {
    rax: u64,
    rbx: u64,
    rcx: u64,
    rdx: u64,
    rsi: u64,
    rdi: u64,
    rbp: u64,
    r8: u64,
    r9: u64,
    r10: u64,
    r11: u64,
    r12: u64,
    r13: u64,
    r14: u64,
    r15: u64,
    rip: u64,
    rsp: u64,
    rflags: u64,
    address_space_root: u64,
}

#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct UserTrapFrame {
    r15: u64,
    r14: u64,
    r13: u64,
    r12: u64,
    r11: u64,
    r10: u64,
    r9: u64,
    r8: u64,
    rbp: u64,
    rdi: u64,
    rsi: u64,
    rdx: u64,
    rcx: u64,
    rbx: u64,
    rip: u64,
    cs: u64,
    rflags: u64,
    rsp: u64,
    ss: u64,
}

#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct UserFaultFrame {
    r15: u64,
    r14: u64,
    r13: u64,
    r12: u64,
    r11: u64,
    r10: u64,
    r9: u64,
    r8: u64,
    rbp: u64,
    rdi: u64,
    rsi: u64,
    rdx: u64,
    rcx: u64,
    rbx: u64,
    rax: u64,
    vector: u64,
    error: u64,
    rip: u64,
    cs: u64,
    rflags: u64,
    rsp: u64,
    ss: u64,
}

impl UserFaultFrame {
    pub(crate) fn metadata(&self, address: u64) -> crate::domain::FaultMetadata {
        crate::domain::FaultMetadata {
            vector: self.vector,
            error: self.error,
            address,
            instruction: self.rip,
        }
    }
}

#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct UserInterruptFrame {
    r15: u64,
    r14: u64,
    r13: u64,
    r12: u64,
    r11: u64,
    r10: u64,
    r9: u64,
    r8: u64,
    rbp: u64,
    rdi: u64,
    rsi: u64,
    rdx: u64,
    rcx: u64,
    rbx: u64,
    rax: u64,
    rip: u64,
    cs: u64,
    rflags: u64,
    rsp: u64,
    ss: u64,
}

impl UserInterruptFrame {
    fn selectors_match(&self, code_selector: u16, data_selector: u16) -> bool {
        self.cs == u64::from(code_selector) && self.ss == u64::from(data_selector)
    }

    fn saved_context(&self, address_space_root: u64) -> UserContext {
        UserContext {
            rax: self.rax,
            rbx: self.rbx,
            rcx: self.rcx,
            rdx: self.rdx,
            rsi: self.rsi,
            rdi: self.rdi,
            rbp: self.rbp,
            r8: self.r8,
            r9: self.r9,
            r10: self.r10,
            r11: self.r11,
            r12: self.r12,
            r13: self.r13,
            r14: self.r14,
            r15: self.r15,
            rip: self.rip,
            rsp: self.rsp,
            rflags: self.rflags,
            address_space_root,
        }
    }

    fn restore(&mut self, context: UserContext, code_selector: u16, data_selector: u16) {
        self.r15 = context.r15;
        self.r14 = context.r14;
        self.r13 = context.r13;
        self.r12 = context.r12;
        self.r11 = context.r11;
        self.r10 = context.r10;
        self.r9 = context.r9;
        self.r8 = context.r8;
        self.rbp = context.rbp;
        self.rdi = context.rdi;
        self.rsi = context.rsi;
        self.rdx = context.rdx;
        self.rcx = context.rcx;
        self.rbx = context.rbx;
        self.rax = context.rax;
        self.rip = context.rip;
        self.cs = u64::from(code_selector);
        self.rflags = context.rflags;
        self.rsp = context.rsp;
        self.ss = u64::from(data_selector);
    }
}

impl UserFaultFrame {
    fn selectors_match(&self, code_selector: u16, data_selector: u16) -> bool {
        self.cs == u64::from(code_selector) && self.ss == u64::from(data_selector)
    }

    fn restore(&mut self, context: UserContext, code_selector: u16, data_selector: u16) {
        self.r15 = context.r15;
        self.r14 = context.r14;
        self.r13 = context.r13;
        self.r12 = context.r12;
        self.r11 = context.r11;
        self.r10 = context.r10;
        self.r9 = context.r9;
        self.r8 = context.r8;
        self.rbp = context.rbp;
        self.rdi = context.rdi;
        self.rsi = context.rsi;
        self.rdx = context.rdx;
        self.rcx = context.rcx;
        self.rbx = context.rbx;
        self.rax = context.rax;
        self.rip = context.rip;
        self.cs = u64::from(code_selector);
        self.rflags = context.rflags;
        self.rsp = context.rsp;
        self.ss = u64::from(data_selector);
    }
}

impl UserTrapFrame {
    pub(crate) fn first_argument(&self) -> u64 {
        self.rsi
    }

    pub(crate) fn second_argument(&self) -> u64 {
        self.rdx
    }

    fn selectors_match(&self, code_selector: u16, data_selector: u16) -> bool {
        self.cs == u64::from(code_selector) && self.ss == u64::from(data_selector)
    }

    fn saved_context(&self, result: u64, address_space_root: u64) -> UserContext {
        UserContext {
            rax: result,
            rbx: self.rbx,
            rcx: self.rcx,
            rdx: self.rdx,
            rsi: self.rsi,
            rdi: self.rdi,
            rbp: self.rbp,
            r8: self.r8,
            r9: self.r9,
            r10: self.r10,
            r11: self.r11,
            r12: self.r12,
            r13: self.r13,
            r14: self.r14,
            r15: self.r15,
            rip: self.rip,
            rsp: self.rsp,
            rflags: self.rflags,
            address_space_root,
        }
    }

    fn restore(&mut self, context: UserContext, code_selector: u16, data_selector: u16) {
        self.r15 = context.r15;
        self.r14 = context.r14;
        self.r13 = context.r13;
        self.r12 = context.r12;
        self.r11 = context.r11;
        self.r10 = context.r10;
        self.r9 = context.r9;
        self.r8 = context.r8;
        self.rbp = context.rbp;
        self.rdi = context.rdi;
        self.rsi = context.rsi;
        self.rdx = context.rdx;
        self.rcx = context.rcx;
        self.rbx = context.rbx;
        self.rip = context.rip;
        self.cs = u64::from(code_selector);
        self.rflags = context.rflags;
        self.rsp = context.rsp;
        self.ss = u64::from(data_selector);
    }
}

impl UserContext {
    fn is_valid(&self) -> bool {
        const LOWER_CANONICAL_END: u64 = (1_u64 << 47) - 1;
        const INTERRUPT_ENABLE: u64 = 1 << 9;
        const IOPL: u64 = 3 << 12;
        const NESTED_TASK: u64 = 1 << 14;
        const VIRTUAL_8086: u64 = 1 << 17;
        self.rip <= LOWER_CANONICAL_END
            && self.rsp <= LOWER_CANONICAL_END
            && self.rip != 0
            && self.rsp != 0
            && self.rflags & 2 != 0
            && self.rflags & INTERRUPT_ENABLE != 0
            && self.rflags & (IOPL | NESTED_TASK | VIRTUAL_8086) == 0
            && self.address_space_root != 0
            && self.address_space_root.is_multiple_of(4096)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Scheduled {
    id: ThreadId,
    context: UserContext,
}

struct Scheduler {
    threads: [Option<Thread>; MAX_THREADS],
    generations: [u32; MAX_THREADS],
    current: Option<ThreadId>,
    next_slot: usize,
}

struct Runtime {
    scheduler: Scheduler,
    control_domain: u64,
    yields: u8,
    preemptions: u8,
    exits: u8,
    address_space_switches: u8,
    faults: u8,
    expected_fault_instruction: u64,
    expected_fault_observed: bool,
    service_restart_context: UserContext,
    service_address_space_owner: u64,
    restarts: u8,
    blocks: u8,
    wakes: u8,
    last_wake_source: Option<u64>,
    last_wake_sequence: Option<u64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RuntimeAction {
    Resume {
        result: u64,
        address_space_root: u64,
    },
    Complete,
    Invalid,
}

impl Scheduler {
    const fn new() -> Self {
        Self {
            threads: [None; MAX_THREADS],
            generations: [0; MAX_THREADS],
            current: None,
            next_slot: 0,
        }
    }

    fn create(&mut self, domain: u64, context: UserContext) -> Option<ThreadId> {
        if domain == 0 || !context.is_valid() {
            return None;
        }
        let slot = self.threads.iter().position(Option::is_none)?;
        let generation = self.generations[slot].checked_add(1)?;
        let id = ThreadId {
            slot: slot as u16,
            generation,
        };
        self.generations[slot] = generation;
        self.threads[slot] = Some(Thread {
            id,
            domain,
            state: State::Ready,
            wait: None,
            context,
        });
        Some(id)
    }

    fn schedule_next(&mut self) -> Option<Scheduled> {
        if self.current.is_some() {
            let id = self.current?;
            return Some(Scheduled {
                id,
                context: self.thread(id)?.context,
            });
        }
        for offset in 0..MAX_THREADS {
            let slot = (self.next_slot + offset) % MAX_THREADS;
            let Some(mut thread) = self.threads[slot] else {
                continue;
            };
            if thread.state == State::Ready {
                thread.state = State::Running;
                self.threads[slot] = Some(thread);
                self.current = Some(thread.id);
                self.next_slot = (slot + 1) % MAX_THREADS;
                return Some(Scheduled {
                    id: thread.id,
                    context: thread.context,
                });
            }
        }
        None
    }

    fn yield_current(&mut self, id: ThreadId, saved: UserContext) -> Option<Scheduled> {
        if self.current != Some(id) {
            return None;
        }
        if !saved.is_valid() {
            return None;
        }
        let thread = self.thread_mut(id)?;
        if thread.state != State::Running {
            return None;
        }
        thread.context = saved;
        thread.state = State::Ready;
        self.current = None;
        self.schedule_next()
    }

    fn block_current(
        &mut self,
        id: ThreadId,
        saved: UserContext,
        reason: WaitReason,
    ) -> Option<Scheduled> {
        if self.current != Some(id) || !saved.is_valid() {
            return None;
        }
        let thread = self.thread_mut(id)?;
        if thread.state != State::Running {
            return None;
        }
        let previous = thread.context;
        thread.context = saved;
        thread.state = State::Blocked;
        thread.wait = Some(reason);
        self.current = None;
        if let Some(next) = self.schedule_next() {
            return Some(next);
        }
        let thread = self.thread_mut(id)?;
        thread.context = previous;
        thread.state = State::Running;
        thread.wait = None;
        self.current = Some(id);
        None
    }

    fn wake_blocked_peer(&mut self, sender_domain: u64, reason: WaitReason, result: u64) -> bool {
        let mut blocked_slot = None;
        for (slot, thread) in self.threads.iter().enumerate() {
            let Some(thread) = thread else {
                continue;
            };
            if thread.state == State::Blocked
                && thread.domain != sender_domain
                && thread.wait == Some(reason)
            {
                if blocked_slot.is_some() {
                    return false;
                }
                blocked_slot = Some(slot);
            }
        }
        let Some(slot) = blocked_slot else {
            return false;
        };
        let Some(thread) = self.threads[slot].as_mut() else {
            return false;
        };
        thread.context.rax = result;
        thread.state = State::Ready;
        thread.wait = None;
        true
    }

    fn cancel_blocked_peers(&mut self, dead_domain: u64, reason: WaitReason, result: u64) -> usize {
        let mut cancelled = 0;
        for thread in self.threads.iter_mut().flatten() {
            if thread.state == State::Blocked
                && thread.domain != dead_domain
                && thread.wait == Some(reason)
            {
                thread.context.rax = result;
                thread.state = State::Ready;
                thread.wait = None;
                cancelled += 1;
            }
        }
        cancelled
    }

    fn exit_current(&mut self, id: ThreadId) -> Option<Scheduled> {
        if self.current != Some(id) || self.thread(id)?.state != State::Running {
            return None;
        }
        self.threads[usize::from(id.slot)] = None;
        self.current = None;
        self.schedule_next()
    }

    fn remove_ready_domain(&mut self, domain: u64) -> bool {
        let mut slot = None;
        for (index, thread) in self.threads.iter().enumerate() {
            if thread.is_some_and(|thread| thread.domain == domain) {
                if slot.is_some() || thread.is_some_and(|thread| thread.state != State::Ready) {
                    return false;
                }
                slot = Some(index);
            }
        }
        let Some(slot) = slot else {
            return false;
        };
        self.threads[slot] = None;
        true
    }

    fn thread(&self, id: ThreadId) -> Option<&Thread> {
        self.threads
            .get(usize::from(id.slot))?
            .as_ref()
            .filter(|thread| thread.id == id)
    }

    fn thread_mut(&mut self, id: ThreadId) -> Option<&mut Thread> {
        self.threads
            .get_mut(usize::from(id.slot))?
            .as_mut()
            .filter(|thread| thread.id == id)
    }
}

pub(crate) fn initialize_runtime(
    domains: [u64; 2],
    address_space_roots: [u64; 2],
    entries: [usize; 2],
    stacks: [usize; 2],
    handles: [[u64; 3]; 2],
    expected_fault_instruction: usize,
    service_restart_entry: usize,
) -> bool {
    let [first_address_space_root, second_address_space_root] = address_space_roots;
    let [first_domain, second_domain] = domains;
    let [first_entry, second_entry] = entries;
    let [first_stack, second_stack] = stacks;
    let [first_handles, second_handles] = handles;
    if RUNTIME_INITIALIZED.load(Ordering::Acquire)
        || first_address_space_root == second_address_space_root
    {
        return false;
    }
    let Some(service_identity) = domain::domain_service_identity(second_domain) else {
        return false;
    };
    let mut scheduler = Scheduler::new();
    let mut first = initial_context(
        first_address_space_root,
        first_entry,
        first_stack,
        first_handles[0],
        first_handles[1],
        first_handles[2],
        1,
    );
    first.r10 = second_domain;
    first.r11 = service_identity;
    let second = initial_context(
        second_address_space_root,
        second_entry,
        second_stack,
        second_handles[0],
        second_handles[1],
        second_handles[2],
        2,
    );
    let service_restart_context = UserContext {
        rip: service_restart_entry as u64,
        r9: 3,
        ..second
    };
    if !domain::attach_thread(first_domain) {
        return false;
    }
    if scheduler.create(first_domain, first).is_none() {
        let _ = domain::detach_thread(first_domain);
        return false;
    }
    if !domain::attach_thread(second_domain) {
        let _ = domain::detach_thread(first_domain);
        return false;
    }
    if scheduler.create(second_domain, second).is_none()
        || domain::attach_thread(first_domain)
        || scheduler.schedule_next().map(|next| next.context) != Some(first)
    {
        let _ = domain::detach_thread(second_domain);
        let _ = domain::detach_thread(first_domain);
        return false;
    }

    // SAFETY: Bootstrap is single-core with interrupts disabled. Publication
    // happens only after the complete runtime value has been written.
    unsafe {
        core::ptr::addr_of_mut!(RUNTIME).write(MaybeUninit::new(Runtime {
            scheduler,
            control_domain: first_domain,
            yields: 0,
            preemptions: 0,
            exits: 0,
            address_space_switches: 0,
            faults: 0,
            expected_fault_instruction: expected_fault_instruction as u64,
            expected_fault_observed: false,
            service_restart_context,
            service_address_space_owner: second_domain,
            restarts: 0,
            blocks: 0,
            wakes: 0,
            last_wake_source: None,
            last_wake_sequence: None,
        }));
    }
    RUNTIME_INITIALIZED.store(true, Ordering::Release);
    true
}

pub(crate) fn queue_service_restart(domain: u64, ipc_handle: u64) -> bool {
    let Some(runtime) = runtime_mut() else {
        return false;
    };
    if runtime.restarts != 0 {
        return false;
    }
    let context = UserContext {
        r8: ipc_handle,
        ..runtime.service_restart_context
    };
    if !domain::attach_thread(domain) {
        return false;
    }
    if runtime.scheduler.create(domain, context).is_none() {
        let _ = domain::detach_thread(domain);
        return false;
    }
    runtime.restarts = 1;
    runtime.service_address_space_owner = domain;
    true
}

pub(crate) fn rollback_service_restart(domain: u64, prior_owner: u64) -> bool {
    let Some(runtime) = runtime_mut() else {
        return false;
    };
    if runtime.restarts != 1
        || runtime.service_address_space_owner != domain
        || !runtime.scheduler.remove_ready_domain(domain)
        || !domain::detach_thread(domain)
    {
        return false;
    }
    runtime.restarts = 0;
    runtime.service_address_space_owner = prior_owner;
    true
}

pub(crate) fn service_restart_stack_mapping() -> Option<(u64, u64)> {
    let context = runtime_mut()?.service_restart_context;
    Some((context.address_space_root, context.rsp))
}

pub(crate) fn service_address_space_owner() -> Option<u64> {
    Some(runtime_mut()?.service_address_space_owner)
}

pub(crate) fn current_domain() -> Option<u64> {
    let runtime = runtime_mut()?;
    let current = runtime.scheduler.current?;
    Some(runtime.scheduler.thread(current)?.domain)
}

pub(crate) fn current_address_space_root() -> Option<u64> {
    let runtime = runtime_mut()?;
    let current = runtime.scheduler.current?;
    runtime
        .scheduler
        .thread(current)
        .map(|thread| thread.context.address_space_root)
}

pub(crate) fn current_instruction_and_stack() -> Option<(u64, u64)> {
    let runtime = runtime_mut()?;
    let current = runtime.scheduler.current?;
    let context = runtime.scheduler.thread(current)?.context;
    Some((context.rip, context.rsp))
}

pub(crate) fn fault_runtime(
    frame: &mut UserFaultFrame,
    fault_address: u64,
    code_selector: u16,
    data_selector: u16,
    peer_close: Option<WaitReason>,
) -> RuntimeAction {
    if !frame.selectors_match(code_selector, data_selector) {
        return RuntimeAction::Invalid;
    }
    let Some(runtime) = runtime_mut() else {
        return RuntimeAction::Invalid;
    };
    let Some(current) = runtime.scheduler.current else {
        return RuntimeAction::Invalid;
    };
    let Some(current_root) = runtime
        .scheduler
        .thread(current)
        .map(|thread| thread.context.address_space_root)
    else {
        return RuntimeAction::Invalid;
    };
    if let Some(reason) = peer_close {
        let Some(dead_domain) = runtime
            .scheduler
            .thread(current)
            .map(|thread| thread.domain)
        else {
            return RuntimeAction::Invalid;
        };
        runtime
            .scheduler
            .cancel_blocked_peers(dead_domain, reason, IPC_PEER_CLOSED);
    }
    let expected = frame.vector == 14
        && frame.error == 6
        && fault_address == 0
        && frame.rip == runtime.expected_fault_instruction
        && frame.r9 == 2;
    let next = runtime.scheduler.exit_current(current);
    let Some(faults) = runtime.faults.checked_add(1) else {
        return RuntimeAction::Invalid;
    };
    runtime.faults = faults;
    runtime.expected_fault_observed |= expected;
    if let Some(next) = next {
        if next.context.address_space_root != current_root {
            let Some(switches) = runtime.address_space_switches.checked_add(1) else {
                return RuntimeAction::Invalid;
            };
            runtime.address_space_switches = switches;
        }
        frame.restore(next.context, code_selector, data_selector);
        RuntimeAction::Resume {
            result: next.context.rax,
            address_space_root: next.context.address_space_root,
        }
    } else {
        RuntimeAction::Complete
    }
}

pub(crate) fn yield_runtime(
    frame: &mut UserTrapFrame,
    code_selector: u16,
    data_selector: u16,
) -> RuntimeAction {
    if !frame.selectors_match(code_selector, data_selector) {
        return RuntimeAction::Invalid;
    }
    let Some(runtime) = runtime_mut() else {
        return RuntimeAction::Invalid;
    };
    let Some(current) = runtime.scheduler.current else {
        return RuntimeAction::Invalid;
    };
    let Some(root) = runtime
        .scheduler
        .thread(current)
        .map(|thread| thread.context.address_space_root)
    else {
        return RuntimeAction::Invalid;
    };
    let saved = frame.saved_context(0, root);
    let Some(next) = runtime.scheduler.yield_current(current, saved) else {
        return RuntimeAction::Invalid;
    };
    let Some(yields) = runtime.yields.checked_add(1) else {
        return RuntimeAction::Invalid;
    };
    runtime.yields = yields;
    if next.context.address_space_root != root {
        let Some(switches) = runtime.address_space_switches.checked_add(1) else {
            return RuntimeAction::Invalid;
        };
        runtime.address_space_switches = switches;
    }
    frame.restore(next.context, code_selector, data_selector);
    RuntimeAction::Resume {
        result: next.context.rax,
        address_space_root: next.context.address_space_root,
    }
}

pub(crate) fn preempt_runtime(
    frame: &mut UserInterruptFrame,
    code_selector: u16,
    data_selector: u16,
) -> RuntimeAction {
    if !frame.selectors_match(code_selector, data_selector) {
        return RuntimeAction::Invalid;
    }
    let Some(runtime) = runtime_mut() else {
        return RuntimeAction::Invalid;
    };
    let Some(current) = runtime.scheduler.current else {
        return RuntimeAction::Invalid;
    };
    let Some(root) = runtime
        .scheduler
        .thread(current)
        .map(|thread| thread.context.address_space_root)
    else {
        return RuntimeAction::Invalid;
    };
    let saved = frame.saved_context(root);
    let Some(next) = runtime.scheduler.yield_current(current, saved) else {
        return RuntimeAction::Invalid;
    };
    let Some(preemptions) = runtime.preemptions.checked_add(1) else {
        return RuntimeAction::Invalid;
    };
    runtime.preemptions = preemptions;
    if next.context.address_space_root != root {
        let Some(switches) = runtime.address_space_switches.checked_add(1) else {
            return RuntimeAction::Invalid;
        };
        runtime.address_space_switches = switches;
    }
    frame.restore(next.context, code_selector, data_selector);
    RuntimeAction::Resume {
        result: next.context.rax,
        address_space_root: next.context.address_space_root,
    }
}

pub(crate) fn block_runtime(
    frame: &mut UserTrapFrame,
    code_selector: u16,
    data_selector: u16,
    reason: WaitReason,
) -> RuntimeAction {
    if !frame.selectors_match(code_selector, data_selector) {
        return RuntimeAction::Invalid;
    }
    let Some(runtime) = runtime_mut() else {
        return RuntimeAction::Invalid;
    };
    let Some(current) = runtime.scheduler.current else {
        return RuntimeAction::Invalid;
    };
    let Some(root) = runtime
        .scheduler
        .thread(current)
        .map(|thread| thread.context.address_space_root)
    else {
        return RuntimeAction::Invalid;
    };
    let saved = frame.saved_context(0, root);
    let Some(next) = runtime.scheduler.block_current(current, saved, reason) else {
        return RuntimeAction::Invalid;
    };
    let Some(blocks) = runtime.blocks.checked_add(1) else {
        return RuntimeAction::Invalid;
    };
    runtime.blocks = blocks;
    if next.context.address_space_root != root {
        let Some(switches) = runtime.address_space_switches.checked_add(1) else {
            return RuntimeAction::Invalid;
        };
        runtime.address_space_switches = switches;
    }
    frame.restore(next.context, code_selector, data_selector);
    RuntimeAction::Resume {
        result: next.context.rax,
        address_space_root: next.context.address_space_root,
    }
}

pub(crate) fn wake_ipc_peer(
    sender_domain: u64,
    object: u64,
    service_side: bool,
    result: u64,
    sequence: u64,
) -> bool {
    if sequence == 0 {
        return false;
    }
    let Some(runtime) = runtime_mut() else {
        return false;
    };
    let Some(wakes) = runtime.wakes.checked_add(1) else {
        return false;
    };
    if !runtime.scheduler.wake_blocked_peer(
        sender_domain,
        WaitReason::IpcReceive {
            object,
            service_side,
            peer_domain: sender_domain,
        },
        result,
    ) {
        return false;
    }
    runtime.wakes = wakes;
    runtime.last_wake_source = Some(sender_domain);
    runtime.last_wake_sequence = Some(sequence);
    true
}

pub(crate) fn exit_runtime(
    frame: &mut UserTrapFrame,
    code_selector: u16,
    data_selector: u16,
    peer_close: Option<WaitReason>,
) -> RuntimeAction {
    if !frame.selectors_match(code_selector, data_selector) {
        return RuntimeAction::Invalid;
    }
    let Some(runtime) = runtime_mut() else {
        return RuntimeAction::Invalid;
    };
    let Some(current) = runtime.scheduler.current else {
        return RuntimeAction::Invalid;
    };
    let Some(current_root) = runtime
        .scheduler
        .thread(current)
        .map(|thread| thread.context.address_space_root)
    else {
        return RuntimeAction::Invalid;
    };
    if let Some(reason) = peer_close {
        let Some(dead_domain) = runtime
            .scheduler
            .thread(current)
            .map(|thread| thread.domain)
        else {
            return RuntimeAction::Invalid;
        };
        runtime
            .scheduler
            .cancel_blocked_peers(dead_domain, reason, IPC_PEER_CLOSED);
    }
    let next = runtime.scheduler.exit_current(current);
    let Some(exits) = runtime.exits.checked_add(1) else {
        return RuntimeAction::Invalid;
    };
    runtime.exits = exits;
    if let Some(next) = next {
        if next.context.address_space_root != current_root {
            let Some(switches) = runtime.address_space_switches.checked_add(1) else {
                return RuntimeAction::Invalid;
            };
            runtime.address_space_switches = switches;
        }
        frame.restore(next.context, code_selector, data_selector);
        RuntimeAction::Resume {
            result: next.context.rax,
            address_space_root: next.context.address_space_root,
        }
    } else if runtime.scheduler.threads.iter().all(Option::is_none) {
        RuntimeAction::Complete
    } else {
        RuntimeAction::Invalid
    }
}

pub(crate) fn verify_runtime_complete() -> bool {
    let Some(runtime) = runtime_mut() else {
        return false;
    };
    runtime.yields == 0
        && runtime.preemptions == 2
        && runtime.exits == 2
        && runtime.faults == 1
        && runtime.restarts == 1
        && runtime.blocks == 1
        && runtime.wakes == 1
        && runtime.last_wake_source == Some(runtime.control_domain)
        && runtime.last_wake_sequence == Some(2)
        && runtime.expected_fault_observed
        && runtime.address_space_switches == 5
        && runtime.scheduler.current.is_none()
        && runtime.scheduler.threads.iter().all(Option::is_none)
}

fn runtime_mut() -> Option<&'static mut Runtime> {
    if !RUNTIME_INITIALIZED.load(Ordering::Acquire) {
        return None;
    }
    // SAFETY: Initialization publishes exactly once. Bootstrap remains
    // single-core with interrupts disabled while the runtime is accessed.
    Some(unsafe { &mut *core::ptr::addr_of_mut!(RUNTIME).cast::<Runtime>() })
}

fn initial_context(
    address_space_root: u64,
    entry: usize,
    stack: usize,
    host_handle: u64,
    console_handle: u64,
    ipc_handle: u64,
    identity: u64,
) -> UserContext {
    UserContext {
        rax: 0,
        rbx: host_handle,
        rcx: console_handle,
        rdx: 0,
        rsi: 0,
        rdi: 0,
        rbp: 0,
        r8: ipc_handle,
        r9: identity,
        r10: 0,
        r11: 0,
        r12: 0,
        r13: 0,
        r14: 0,
        r15: 0,
        rip: entry as u64,
        rsp: stack as u64,
        rflags: 2 | (1 << 9),
        address_space_root,
    }
}

pub(crate) fn verify_scheduler_invariants() -> bool {
    verify_scheduler_invariants_inner().unwrap_or(false)
}

fn verify_scheduler_invariants_inner() -> Option<bool> {
    let first_context = test_context(10, 0x1000, 0x3000, 0x100000);
    let second_context = test_context(20, 0x5000, 0x7000, 0x200000);
    let mut scheduler = Scheduler::new();
    if scheduler.create(0, first_context).is_some()
        || scheduler
            .create(
                1,
                UserContext {
                    rip: 0,
                    ..first_context
                },
            )
            .is_some()
        || scheduler
            .create(
                1,
                UserContext {
                    rflags: first_context.rflags & !(1 << 9),
                    ..first_context
                },
            )
            .is_some()
        || scheduler
            .create(
                1,
                UserContext {
                    rflags: first_context.rflags | (3 << 12),
                    ..first_context
                },
            )
            .is_some()
    {
        return Some(false);
    }
    let first = scheduler.create(1, first_context)?;
    let second = scheduler.create(1, second_context)?;
    let first_run = scheduler.schedule_next()?;
    let mut first_saved = first_run.context;
    first_saved.rax = 11;
    if first_run.id != first
        || first_run.context != first_context
        || scheduler.thread(first)?.domain != 1
        || scheduler.yield_current(second, second_context).is_some()
        || scheduler.yield_current(first, first_saved)?.id != second
        || scheduler.thread(first)?.state != State::Ready
        || scheduler.thread(second)?.state != State::Running
        || scheduler.exit_current(second)?.id != first
        || scheduler.schedule_next()?.context != first_saved
        || scheduler.thread(second).is_some()
        || scheduler.exit_current(first).is_some()
        || scheduler.schedule_next().is_some()
    {
        return Some(false);
    }

    let replacement = scheduler.create(2, second_context)?;
    let generation_valid = replacement != first
        && replacement.slot == first.slot
        && scheduler.thread(first).is_none()
        && scheduler.thread(replacement)?.domain == 2;

    let mut blocking = Scheduler::new();
    let blocked = blocking.create(1, first_context)?;
    let peer = blocking.create(2, second_context)?;
    let wait = WaitReason::IpcReceive {
        object: 3,
        service_side: false,
        peer_domain: 2,
    };
    if blocking.schedule_next()?.id != blocked
        || blocking.block_current(blocked, first_context, wait)?.id != peer
        || blocking.thread(blocked)?.state != State::Blocked
        || blocking.wake_blocked_peer(
            2,
            WaitReason::IpcReceive {
                object: 4,
                service_side: false,
                peer_domain: 2,
            },
            66,
        )
        || blocking.wake_blocked_peer(
            2,
            WaitReason::IpcReceive {
                object: 3,
                service_side: true,
                peer_domain: 2,
            },
            66,
        )
        || blocking.wake_blocked_peer(
            3,
            WaitReason::IpcReceive {
                object: 3,
                service_side: false,
                peer_domain: 3,
            },
            66,
        )
        || !blocking.wake_blocked_peer(2, wait, 77)
        || blocking.thread(blocked)?.state != State::Ready
        || blocking.thread(blocked)?.wait.is_some()
        || blocking.exit_current(peer)?.context.rax != 77
    {
        return Some(false);
    }

    let mut closing = Scheduler::new();
    let waiting_client = closing.create(1, first_context)?;
    let unrelated = closing.create(3, first_context)?;
    let dying_service = closing.create(2, second_context)?;
    if closing.schedule_next()?.id != waiting_client
        || closing
            .block_current(waiting_client, first_context, wait)?
            .id
            != unrelated
        || closing.yield_current(unrelated, first_context)?.id != dying_service
        || closing.cancel_blocked_peers(
            2,
            WaitReason::IpcReceive {
                object: 4,
                service_side: false,
                peer_domain: 2,
            },
            IPC_PEER_CLOSED,
        ) != 0
        || closing.thread(waiting_client)?.state != State::Blocked
        || closing.cancel_blocked_peers(2, wait, IPC_PEER_CLOSED) != 1
        || closing.thread(waiting_client)?.state != State::Ready
        || closing.thread(waiting_client)?.wait.is_some()
        || closing.thread(waiting_client)?.context.rax != IPC_PEER_CLOSED
        || closing.thread(unrelated)?.state != State::Ready
        || closing.exit_current(dying_service)?.id != waiting_client
    {
        return Some(false);
    }

    let mut lone = Scheduler::new();
    let lone_id = lone.create(1, first_context)?;
    let _ = lone.schedule_next()?;
    let rollback = lone.block_current(lone_id, first_context, wait).is_none()
        && lone.current == Some(lone_id)
        && lone.thread(lone_id)?.state == State::Running
        && lone.thread(lone_id)?.wait.is_none();
    let mut restart_rollback = Scheduler::new();
    let survivor = restart_rollback.create(1, first_context)?;
    let replacement = restart_rollback.create(2, second_context)?;
    let replacement_removed = restart_rollback.schedule_next()?.id == survivor
        && restart_rollback.remove_ready_domain(2)
        && restart_rollback.thread(replacement).is_none()
        && restart_rollback.current == Some(survivor)
        && !restart_rollback.remove_ready_domain(2);
    Some(generation_valid && rollback && replacement_removed)
}

const fn test_context(rax: u64, rip: u64, rsp: u64, root: u64) -> UserContext {
    UserContext {
        rax,
        rbx: 0,
        rcx: 0,
        rdx: 0,
        rsi: 0,
        rdi: 0,
        rbp: 0,
        r8: 0,
        r9: 0,
        r10: 0,
        r11: 0,
        r12: 0,
        r13: 0,
        r14: 0,
        r15: 0,
        rip,
        rsp,
        rflags: 2 | (1 << 9),
        address_space_root: root,
    }
}
