use core::mem::MaybeUninit;
use core::sync::atomic::{AtomicBool, Ordering};

use crate::domain;

const MAX_THREADS: usize = 8;
pub(crate) const IPC_PEER_CLOSED: u64 = u64::MAX - 4;

static RUNTIME_INITIALIZED: AtomicBool = AtomicBool::new(false);
static mut RUNTIME: MaybeUninit<Runtime> = MaybeUninit::uninit();
static PROCESS_WORKSPACE_ACTIVE: AtomicBool = AtomicBool::new(false);
static mut PROCESS_WORKSPACE: MaybeUninit<ProcessScheduler> = MaybeUninit::uninit();

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ThreadId {
    slot: u16,
    generation: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum State {
    Staged,
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
    DeviceCompletion {
        object: u64,
        domain: u64,
        request: u64,
    },
    PacketAvailable {
        object: u64,
        sequence: u64,
    },
    PacketSpaceAvailable {
        object: u64,
    },
}

#[derive(Clone, Copy)]
struct Thread {
    id: ThreadId,
    domain: u64,
    state: State,
    initialized: bool,
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
    fs_base: u64,
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

#[derive(Clone, Copy)]
pub(crate) struct UserFaultContext {
    pub(crate) r15: u64,
    pub(crate) r14: u64,
    pub(crate) r13: u64,
    pub(crate) r12: u64,
    pub(crate) r11: u64,
    pub(crate) r10: u64,
    pub(crate) r9: u64,
    pub(crate) r8: u64,
    pub(crate) rbp: u64,
    pub(crate) rdi: u64,
    pub(crate) rsi: u64,
    pub(crate) rdx: u64,
    pub(crate) rcx: u64,
    pub(crate) rbx: u64,
    pub(crate) rax: u64,
    pub(crate) rip: u64,
    pub(crate) rflags: u64,
    pub(crate) rsp: u64,
}

impl UserFaultFrame {
    pub(crate) const fn is_page_fault(&self) -> bool {
        self.vector == 14
    }

    pub(crate) fn metadata(&self, address: u64) -> crate::domain::FaultMetadata {
        crate::domain::FaultMetadata {
            vector: self.vector,
            error: self.error,
            address,
            instruction: self.rip,
        }
    }

    pub(crate) const fn context(&self) -> UserFaultContext {
        UserFaultContext {
            r15: self.r15,
            r14: self.r14,
            r13: self.r13,
            r12: self.r12,
            r11: self.r11,
            r10: self.r10,
            r9: self.r9,
            r8: self.r8,
            rbp: self.rbp,
            rdi: self.rdi,
            rsi: self.rsi,
            rdx: self.rdx,
            rcx: self.rcx,
            rbx: self.rbx,
            rax: self.rax,
            rip: self.rip,
            rflags: self.rflags,
            rsp: self.rsp,
        }
    }

    pub(crate) fn enter_signal_handler(
        &mut self,
        handler: u64,
        stack: u64,
        signal: u64,
        arguments: (u64, u64),
        code_selector: u16,
        data_selector: u16,
    ) -> bool {
        let (information, context) = arguments;
        if !self.selectors_match(code_selector, data_selector)
            || handler == 0
            || handler >= 1 << 47
            || stack == 0
            || stack >= 1 << 47
            || information == 0
            || information >= 1 << 47
            || context == 0
            || context >= 1 << 47
        {
            return false;
        }
        self.rip = handler;
        self.rsp = stack;
        self.rdi = signal;
        self.rsi = information;
        self.rdx = context;
        self.rflags &= !((1 << 8) | (1 << 10) | (1 << 16));
        true
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
    pub(crate) fn user_context(
        &self,
        code_selector: u16,
        data_selector: u16,
    ) -> Option<UserFaultContext> {
        self.selectors_match(code_selector, data_selector)
            .then_some(UserFaultContext {
                r15: self.r15,
                r14: self.r14,
                r13: self.r13,
                r12: self.r12,
                r11: self.r11,
                r10: self.r10,
                r9: self.r9,
                r8: self.r8,
                rbp: self.rbp,
                rdi: self.rdi,
                rsi: self.rsi,
                rdx: self.rdx,
                rcx: self.rcx,
                rbx: self.rbx,
                rax: self.rax,
                rip: self.rip,
                rflags: self.rflags,
                rsp: self.rsp,
            })
    }

    pub(crate) fn restore_user_context(
        &mut self,
        context: UserFaultContext,
        code_selector: u16,
        data_selector: u16,
    ) -> bool {
        if !self.selectors_match(code_selector, data_selector) {
            return false;
        }
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
        self.rflags = context.rflags;
        self.rsp = context.rsp;
        true
    }

    fn selectors_match(&self, code_selector: u16, data_selector: u16) -> bool {
        self.cs == u64::from(code_selector) && self.ss == u64::from(data_selector)
    }

    fn saved_context(&self, address_space_root: u64, fs_base: u64) -> UserContext {
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
            fs_base,
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
    pub(crate) fn process_general_registers(&self, rax: u64) -> [u64; 15] {
        [
            rax, self.rbx, self.rcx, self.rdx, self.rsi, self.rdi, self.rbp, self.r8, self.r9,
            self.r10, self.r11, self.r12, self.r13, self.r14, self.r15,
        ]
    }

    pub(crate) fn first_argument(&self) -> u64 {
        self.rsi
    }

    pub(crate) fn second_argument(&self) -> u64 {
        self.rdx
    }

    fn selectors_match(&self, code_selector: u16, data_selector: u16) -> bool {
        self.cs == u64::from(code_selector) && self.ss == u64::from(data_selector)
    }

    fn saved_context(&self, result: u64, address_space_root: u64, fs_base: u64) -> UserContext {
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
            fs_base,
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
            && self.fs_base <= LOWER_CANONICAL_END
    }
}

pub(crate) fn process_user_context(
    general: [u64; 15],
    rip: u64,
    rsp: u64,
    rflags: u64,
    address_space_root: u64,
    fs_base: u64,
) -> Option<ProcessSchedulerContext> {
    let [rax, rbx, rcx, rdx, rsi, rdi, rbp, r8, r9, r10, r11, r12, r13, r14, r15] = general;
    let context = UserContext {
        rax,
        rbx,
        rcx,
        rdx,
        rsi,
        rdi,
        rbp,
        r8,
        r9,
        r10,
        r11,
        r12,
        r13,
        r14,
        r15,
        rip,
        rsp,
        rflags,
        address_space_root,
        fs_base,
    };
    context
        .is_valid()
        .then_some(ProcessSchedulerContext(context))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ProcessSchedulerContext(UserContext);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ProcessThreadHandle(ThreadId);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ScheduledProcess {
    handle: ProcessThreadHandle,
    context: ProcessSchedulerContext,
}

impl ScheduledProcess {
    pub(crate) const fn handle(self) -> ProcessThreadHandle {
        self.handle
    }

    pub(crate) const fn context(self) -> ProcessSchedulerContext {
        self.context
    }
}

pub(crate) struct ProcessScheduler {
    scheduler: Scheduler,
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
        fs_base: u64,
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
            initialized: true,
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

    fn block_current_idle(&mut self, id: ThreadId, saved: UserContext, reason: WaitReason) -> bool {
        if self.current != Some(id)
            || !saved.is_valid()
            || self.threads.iter().flatten().any(|thread| {
                thread.id != id && matches!(thread.state, State::Ready | State::Running)
            })
        {
            return false;
        }
        let Some(thread) = self.thread_mut(id) else {
            return false;
        };
        if thread.state != State::Running {
            return false;
        }
        thread.context = saved;
        thread.state = State::Blocked;
        thread.wait = Some(reason);
        self.current = None;
        true
    }

    fn wake_idle_domain(
        &mut self,
        domain: u64,
        reason: WaitReason,
        result: u64,
    ) -> Option<Scheduled> {
        if self.current.is_some() {
            return None;
        }
        let mut blocked_slot = None;
        for (slot, thread) in self.threads.iter().enumerate() {
            let Some(thread) = thread else {
                continue;
            };
            if thread.state == State::Blocked
                && thread.domain == domain
                && thread.wait == Some(reason)
            {
                if blocked_slot.is_some() {
                    return None;
                }
                blocked_slot = Some(slot);
            } else if thread.state == State::Ready {
                return None;
            }
        }
        let slot = blocked_slot?;
        let id = self.threads[slot]?.id;
        let thread = self.threads[slot].as_mut()?;
        thread.context.rax = result;
        thread.state = State::Ready;
        thread.wait = None;
        let scheduled = self.schedule_next()?;
        (scheduled.id == id).then_some(scheduled)
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

    fn wake_blocked_domain(&mut self, domain: u64, reason: WaitReason, result: u64) -> bool {
        let mut blocked_slot = None;
        for (slot, thread) in self.threads.iter().enumerate() {
            let Some(thread) = thread else {
                continue;
            };
            if thread.state == State::Blocked
                && thread.domain == domain
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

    fn has_blocked_wait(&self, reason: WaitReason) -> bool {
        self.threads
            .iter()
            .flatten()
            .any(|thread| thread.state == State::Blocked && thread.wait == Some(reason))
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

    fn retire_process(&mut self, id: ThreadId) -> bool {
        let Some(thread) = self.thread(id).copied() else {
            return false;
        };
        if !matches!(thread.state, State::Ready | State::Running)
            || thread.state == State::Running && self.current != Some(id)
        {
            return false;
        }
        self.threads[usize::from(id.slot)] = None;
        if self.current == Some(id) {
            self.current = None;
        }
        true
    }

    fn remove_cancellable_domain(&mut self, domain: u64) -> bool {
        let mut slot = None;
        for (index, thread) in self.threads.iter().enumerate() {
            if thread.is_some_and(|thread| thread.domain == domain) {
                if slot.is_some()
                    || thread.is_some_and(|thread| {
                        !matches!(thread.state, State::Staged | State::Ready | State::Blocked)
                    })
                {
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

    fn start_staged_domain(&mut self, domain: u64) -> bool {
        let mut slot = None;
        for (index, thread) in self.threads.iter().enumerate() {
            if thread.is_some_and(|thread| thread.domain == domain) {
                if slot.is_some()
                    || thread
                        .is_some_and(|thread| thread.state != State::Staged || !thread.initialized)
                {
                    return false;
                }
                slot = Some(index);
            }
        }
        let Some(thread) = slot.and_then(|slot| self.threads[slot].as_mut()) else {
            return false;
        };
        thread.state = State::Ready;
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

impl ProcessScheduler {
    pub(crate) const fn new() -> Self {
        Self {
            scheduler: Scheduler::new(),
        }
    }

    pub(crate) fn admit(
        &mut self,
        domain: u64,
        context: ProcessSchedulerContext,
    ) -> Option<ProcessThreadHandle> {
        self.scheduler
            .create(domain, context.0)
            .map(ProcessThreadHandle)
    }

    pub(crate) fn schedule_next(&mut self) -> Option<ScheduledProcess> {
        self.scheduler
            .schedule_next()
            .map(|scheduled| ScheduledProcess {
                handle: ProcessThreadHandle(scheduled.id),
                context: ProcessSchedulerContext(scheduled.context),
            })
    }

    pub(crate) fn yield_current(
        &mut self,
        handle: ProcessThreadHandle,
        context: ProcessSchedulerContext,
    ) -> Option<ScheduledProcess> {
        self.scheduler
            .yield_current(handle.0, context.0)
            .map(|scheduled| ScheduledProcess {
                handle: ProcessThreadHandle(scheduled.id),
                context: ProcessSchedulerContext(scheduled.context),
            })
    }

    pub(crate) fn retire(&mut self, handle: ProcessThreadHandle) -> bool {
        self.scheduler.retire_process(handle.0)
    }
}

pub(crate) fn acquire_process_scheduler_workspace() -> Option<&'static mut ProcessScheduler> {
    if PROCESS_WORKSPACE_ACTIVE
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return None;
    }
    // SAFETY: The active flag serializes the single bootstrap processor. Each
    // field is initialized directly so the bounded kernel stack never holds a
    // complete scheduler temporary.
    unsafe {
        let workspace = core::ptr::addr_of_mut!(PROCESS_WORKSPACE).cast::<ProcessScheduler>();
        for index in 0..MAX_THREADS {
            core::ptr::addr_of_mut!((*workspace).scheduler.threads[index]).write(None);
            core::ptr::addr_of_mut!((*workspace).scheduler.generations[index]).write(0);
        }
        core::ptr::addr_of_mut!((*workspace).scheduler.current).write(None);
        core::ptr::addr_of_mut!((*workspace).scheduler.next_slot).write(0);
        Some(&mut *workspace)
    }
}

pub(crate) fn active_process_scheduler_workspace() -> Option<&'static mut ProcessScheduler> {
    if !PROCESS_WORKSPACE_ACTIVE.load(Ordering::Acquire) {
        return None;
    }
    // SAFETY: Process execution is serialized on the bootstrap processor. The
    // caller that acquired the workspace has returned its mutable reference
    // before a later lifecycle phase resumes it through this accessor.
    unsafe { Some(&mut *core::ptr::addr_of_mut!(PROCESS_WORKSPACE).cast::<ProcessScheduler>()) }
}

pub(crate) fn release_process_scheduler_workspace() -> bool {
    if !PROCESS_WORKSPACE_ACTIVE.load(Ordering::Acquire) {
        return false;
    }
    // SAFETY: The active workspace remains exclusively borrowed by the
    // serialized process proof until every binding is retired.
    let clean = unsafe {
        let workspace = core::ptr::addr_of!(PROCESS_WORKSPACE).cast::<ProcessScheduler>();
        (*workspace).scheduler.threads.iter().all(Option::is_none)
            && (*workspace).scheduler.current.is_none()
    };
    if clean {
        PROCESS_WORKSPACE_ACTIVE.store(false, Ordering::Release);
    }
    clean
}

pub(crate) fn initialize_runtime(
    domains: [u64; 2],
    address_space_roots: [u64; 2],
    entries: [usize; 2],
    stacks: [usize; 2],
    handles: [[u64; 4]; 2],
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
    first.r14 = first_handles[3];
    let second = initial_context(
        second_address_space_root,
        second_entry,
        second_stack,
        second_handles[0],
        second_handles[1],
        second_handles[2],
        2,
    );
    let second = UserContext {
        r14: second_handles[3],
        ..second
    };
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

pub(crate) fn queue_auxiliary(
    domain_id: u64,
    address_space_root: u64,
    entry: usize,
    stack: usize,
    console_handle: u64,
) -> bool {
    let Some(runtime) = runtime_mut() else {
        return false;
    };
    if domain::service_failure_policy(domain_id) != Some(domain::ServiceFailurePolicy::StopOnly) {
        return false;
    }
    let Some(service_identity) = domain::domain_service_identity(domain_id) else {
        return false;
    };
    let mut context = initial_context(address_space_root, entry, stack, 0, console_handle, 0, 4);
    context.r10 = domain_id;
    context.r11 = service_identity;
    if !domain::attach_thread(domain_id) {
        return false;
    }
    let Some(thread) = runtime.scheduler.create(domain_id, context) else {
        let _ = domain::detach_thread(domain_id);
        return false;
    };
    if runtime.scheduler.current.is_some() {
        let Some(queued) = runtime.scheduler.thread_mut(thread) else {
            return false;
        };
        queued.state = State::Staged;
        queued.initialized = false;
        true
    } else {
        let Some(next) = runtime.scheduler.schedule_next() else {
            let _ = runtime.scheduler.remove_cancellable_domain(domain_id);
            let _ = domain::detach_thread(domain_id);
            return false;
        };
        next.context == context
    }
}

pub(crate) fn cancel_auxiliary(domain_id: u64) -> bool {
    let Some(runtime) = runtime_mut() else {
        return false;
    };
    if domain::service_failure_policy(domain_id) != Some(domain::ServiceFailurePolicy::StopOnly)
        || !runtime.scheduler.remove_cancellable_domain(domain_id)
    {
        return false;
    }
    domain::detach_thread(domain_id)
}

pub(crate) fn initialize_staged_auxiliary(
    domain_id: u64,
    payload_address: u64,
    payload_length: u64,
) -> bool {
    let Some(runtime) = runtime_mut() else {
        return false;
    };
    let mut matched = None;
    for thread in runtime.scheduler.threads.iter_mut().flatten() {
        if thread.domain == domain_id {
            if matched.is_some() || thread.state != State::Staged || thread.initialized {
                return false;
            }
            matched = Some(thread);
        }
    }
    let Some(thread) = matched else {
        return false;
    };
    thread.context.rsi = payload_address;
    thread.context.rdx = payload_length;
    thread.initialized = true;
    true
}

pub(crate) fn initialize_staged_packet_auxiliary(domain_id: u64, sequence: u64) -> bool {
    if sequence == 0 {
        return false;
    }
    let Some(runtime) = runtime_mut() else {
        return false;
    };
    let mut matched = None;
    for thread in runtime.scheduler.threads.iter_mut().flatten() {
        if thread.domain == domain_id {
            if matched.is_some() || thread.state != State::Staged || thread.initialized {
                return false;
            }
            matched = Some(thread);
        }
    }
    let Some(thread) = matched else {
        return false;
    };
    thread.context.rbx = sequence;
    thread.initialized = true;
    true
}

pub(crate) fn initialize_staged_native_auxiliary(domain_id: u64, boot_info: u64) -> bool {
    if boot_info == 0 {
        return false;
    }
    let Some(runtime) = runtime_mut() else {
        return false;
    };
    let mut matched = None;
    for thread in runtime.scheduler.threads.iter_mut().flatten() {
        if thread.domain == domain_id {
            if matched.is_some() || thread.state != State::Staged || thread.initialized {
                return false;
            }
            matched = Some(thread);
        }
    }
    let Some(thread) = matched else {
        return false;
    };
    thread.context.rbx = boot_info;
    thread.initialized = true;
    true
}

pub(crate) fn auxiliary_is_staged(domain_id: u64) -> bool {
    let Some(runtime) = runtime_mut() else {
        return false;
    };
    let mut matches = runtime
        .scheduler
        .threads
        .iter()
        .flatten()
        .filter(|thread| thread.domain == domain_id);
    matches
        .next()
        .is_some_and(|thread| thread.state == State::Staged)
        && matches.next().is_none()
}

pub(crate) fn auxiliary_runtime_state(domain_id: u64) -> Option<u64> {
    let runtime = runtime_mut()?;
    let mut matches = runtime
        .scheduler
        .threads
        .iter()
        .flatten()
        .filter(|thread| thread.domain == domain_id);
    let state = matches.next().map(|thread| thread.state);
    if matches.next().is_some() {
        return None;
    }
    Some(match state {
        None => crate::control_abi::AUXILIARY_STATE_CREATED,
        Some(State::Staged) => crate::control_abi::AUXILIARY_STATE_STAGED,
        Some(State::Ready) => crate::control_abi::AUXILIARY_STATE_READY,
        Some(State::Running) => crate::control_abi::AUXILIARY_STATE_RUNNING,
        Some(State::Blocked) => crate::control_abi::AUXILIARY_STATE_BLOCKED,
    })
}

pub(crate) fn auxiliary_runtime_flags(domain_id: u64) -> Option<u64> {
    let runtime = runtime_mut()?;
    let mut matches = runtime
        .scheduler
        .threads
        .iter()
        .flatten()
        .filter(|thread| thread.domain == domain_id);
    let initialized = matches.next().map(|thread| thread.initialized);
    if matches.next().is_some() {
        return None;
    }
    Some(if initialized.unwrap_or(false) {
        crate::control_abi::AUXILIARY_FLAG_INITIALIZED
    } else {
        0
    })
}

pub(crate) fn start_staged_auxiliary(domain_id: u64) -> bool {
    let Some(runtime) = runtime_mut() else {
        return false;
    };
    runtime.scheduler.start_staged_domain(domain_id)
}

pub(crate) fn rollback_service_restart(domain: u64, prior_owner: u64) -> bool {
    let Some(runtime) = runtime_mut() else {
        return false;
    };
    if runtime.restarts != 1
        || runtime.service_address_space_owner != domain
        || !runtime.scheduler.remove_cancellable_domain(domain)
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

pub(crate) fn current_fs_base() -> Option<u64> {
    let runtime = runtime_mut()?;
    let current = runtime.scheduler.current?;
    runtime
        .scheduler
        .thread(current)
        .map(|thread| thread.context.fs_base)
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
            fs_base: next.context.fs_base,
        }
    } else {
        RuntimeAction::Complete
    }
}

pub(crate) fn yield_runtime(
    frame: &mut UserTrapFrame,
    code_selector: u16,
    data_selector: u16,
    fs_base: u64,
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
    let saved = frame.saved_context(0, root, fs_base);
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
        fs_base: next.context.fs_base,
    }
}

pub(crate) fn preempt_runtime(
    frame: &mut UserInterruptFrame,
    code_selector: u16,
    data_selector: u16,
    fs_base: u64,
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
    let saved = frame.saved_context(root, fs_base);
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
        fs_base: next.context.fs_base,
    }
}

pub(crate) fn block_runtime(
    frame: &mut UserTrapFrame,
    code_selector: u16,
    data_selector: u16,
    reason: WaitReason,
    fs_base: u64,
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
    let saved = frame.saved_context(0, root, fs_base);
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
        fs_base: next.context.fs_base,
    }
}

pub(crate) fn block_runtime_idle(
    frame: &mut UserTrapFrame,
    code_selector: u16,
    data_selector: u16,
    reason: WaitReason,
    fs_base: u64,
) -> bool {
    if !frame.selectors_match(code_selector, data_selector) {
        return false;
    }
    let Some(runtime) = runtime_mut() else {
        return false;
    };
    let Some(current) = runtime.scheduler.current else {
        return false;
    };
    let Some(root) = runtime
        .scheduler
        .thread(current)
        .map(|thread| thread.context.address_space_root)
    else {
        return false;
    };
    let Some(blocks) = runtime.blocks.checked_add(1) else {
        return false;
    };
    let saved = frame.saved_context(0, root, fs_base);
    if !runtime.scheduler.block_current_idle(current, saved, reason) {
        return false;
    }
    runtime.blocks = blocks;
    true
}

pub(crate) fn wake_device_completion(domain: u64, object: u64, request: u64, result: u64) -> bool {
    let Some(runtime) = runtime_mut() else {
        return false;
    };
    let Some(wakes) = runtime.wakes.checked_add(1) else {
        return false;
    };
    let reason = WaitReason::DeviceCompletion {
        object,
        domain,
        request,
    };
    let Some(scheduled) = runtime.scheduler.wake_idle_domain(domain, reason, result) else {
        return false;
    };
    if runtime
        .scheduler
        .thread(scheduled.id)
        .map(|thread| thread.domain)
        != Some(domain)
    {
        return false;
    }
    runtime.wakes = wakes;
    true
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

pub(crate) fn packet_wait_is_available(sequence: u64) -> bool {
    if sequence == 0 {
        return false;
    }
    let Some(runtime) = runtime_mut() else {
        return false;
    };
    !runtime
        .scheduler
        .has_blocked_wait(WaitReason::PacketAvailable {
            object: crate::control_abi::PACKET_OBJECT,
            sequence,
        })
}

pub(crate) fn wake_packet_waiter(sender_domain: u64, sequence: u64) -> bool {
    if sender_domain == 0 || sequence == 0 {
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
        WaitReason::PacketAvailable {
            object: crate::control_abi::PACKET_OBJECT,
            sequence,
        },
        sequence,
    ) {
        return false;
    }
    runtime.wakes = wakes;
    runtime.last_wake_source = Some(sender_domain);
    runtime.last_wake_sequence = Some(sequence);
    true
}

pub(crate) fn packet_space_wait_is_available() -> bool {
    let Some(runtime) = runtime_mut() else {
        return false;
    };
    !runtime
        .scheduler
        .has_blocked_wait(WaitReason::PacketSpaceAvailable {
            object: crate::control_abi::PACKET_OBJECT,
        })
}

pub(crate) fn wake_packet_space_waiter(sender_domain: u64) -> bool {
    if sender_domain == 0 {
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
        WaitReason::PacketSpaceAvailable {
            object: crate::control_abi::PACKET_OBJECT,
        },
        0,
    ) {
        return false;
    }
    runtime.wakes = wakes;
    true
}

pub(crate) fn timeout_packet_waiter(domain: u64, sequence: u64) -> bool {
    if domain == 0 || sequence == 0 {
        return false;
    }
    let Some(runtime) = runtime_mut() else {
        return false;
    };
    let Some(wakes) = runtime.wakes.checked_add(1) else {
        return false;
    };
    let was_idle = runtime.scheduler.current.is_none();
    if !runtime.scheduler.wake_blocked_domain(
        domain,
        WaitReason::PacketAvailable {
            object: crate::control_abi::PACKET_OBJECT,
            sequence,
        },
        crate::control_abi::PACKET_RESULT_TIMED_OUT,
    ) {
        return false;
    }
    if was_idle {
        let Some(scheduled) = runtime.scheduler.schedule_next() else {
            return false;
        };
        if runtime
            .scheduler
            .thread(scheduled.id)
            .map(|thread| thread.domain)
            != Some(domain)
        {
            return false;
        }
    }
    runtime.wakes = wakes;
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
            fs_base: next.context.fs_base,
        }
    } else {
        RuntimeAction::Complete
    }
}

pub(crate) fn verify_runtime_complete() -> bool {
    let Some(runtime) = runtime_mut() else {
        return false;
    };
    runtime.yields == 6
        && runtime.preemptions == 2
        && runtime.exits == 3
        && runtime.faults == 1
        && runtime.restarts == 1
        && runtime.blocks == 3
        && runtime.wakes == 3
        && runtime.last_wake_source == Some(runtime.control_domain)
        && runtime.last_wake_sequence == Some(4)
        && runtime.expected_fault_observed
        && (13..=14).contains(&runtime.address_space_switches)
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
        fs_base: 0,
    }
}

pub(crate) fn verify_scheduler_invariants() -> bool {
    verify_scheduler_invariants_inner().unwrap_or(false)
}

fn verify_scheduler_invariants_inner() -> Option<bool> {
    let first_context = test_context(10, 0x1000, 0x3000, 0x100000, 0x1110);
    let second_context = test_context(20, 0x5000, 0x7000, 0x200000, 0x2220);
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
                    fs_base: 1 << 47,
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
    let mut interrupt_idle = Scheduler::new();
    let interrupt_domain = 0x0000_0002_0000_0001;
    let interrupt_id = interrupt_idle.create(interrupt_domain, first_context)?;
    let interrupt_wait = WaitReason::DeviceCompletion {
        object: 7,
        domain: interrupt_domain,
        request: 1,
    };
    let interrupt_idle_valid = interrupt_idle.schedule_next()?.id == interrupt_id
        && interrupt_idle.block_current_idle(interrupt_id, first_context, interrupt_wait)
        && interrupt_idle.current.is_none()
        && interrupt_idle.thread(interrupt_id)?.state == State::Blocked
        && interrupt_idle
            .wake_idle_domain(interrupt_domain.wrapping_add(1), interrupt_wait, 0)
            .is_none()
        && interrupt_idle
            .wake_idle_domain(
                interrupt_domain,
                WaitReason::DeviceCompletion {
                    object: 7,
                    domain: interrupt_domain,
                    request: 2,
                },
                0,
            )
            .is_none()
        && interrupt_idle.current.is_none()
        && interrupt_idle.thread(interrupt_id)?.state == State::Blocked
        && interrupt_idle
            .wake_idle_domain(interrupt_domain, interrupt_wait, 73)
            .is_some_and(|scheduled| scheduled.id == interrupt_id && scheduled.context.rax == 73)
        && interrupt_idle.current == Some(interrupt_id)
        && interrupt_idle.thread(interrupt_id)?.state == State::Running
        && interrupt_idle.thread(interrupt_id)?.wait.is_none()
        && interrupt_idle
            .wake_idle_domain(interrupt_domain, interrupt_wait, 74)
            .is_none();
    let mut restart_rollback = Scheduler::new();
    let survivor = restart_rollback.create(1, first_context)?;
    let replacement = restart_rollback.create(2, second_context)?;
    let replacement_removed = restart_rollback.schedule_next()?.id == survivor
        && restart_rollback.remove_cancellable_domain(2)
        && restart_rollback.thread(replacement).is_none()
        && restart_rollback.current == Some(survivor)
        && !restart_rollback.remove_cancellable_domain(2);
    let mut staged = Scheduler::new();
    let staging_control = staged.create(1, first_context)?;
    let staged_workload = staged.create(2, second_context)?;
    staged.thread_mut(staged_workload)?.state = State::Staged;
    staged.thread_mut(staged_workload)?.initialized = false;
    let staging_run = staged.schedule_next()?;
    let staged_not_implicitly_admitted = staging_run.id == staging_control
        && staged.yield_current(staging_control, first_context)?.id == staging_control
        && staged.thread(staged_workload)?.state == State::Staged;
    let staged_explicitly_started = !staged.start_staged_domain(2)
        && {
            staged.thread_mut(staged_workload)?.initialized = true;
            true
        }
        && staged.start_staged_domain(2)
        && !staged.start_staged_domain(2)
        && staged.yield_current(staging_control, first_context)?.id == staged_workload;
    let mut cancellable = Scheduler::new();
    let packet_waiter = cancellable.create(1, first_context)?;
    let exiting_peer = cancellable.create(2, second_context)?;
    let packet_wait = WaitReason::PacketAvailable {
        object: crate::control_abi::PACKET_OBJECT,
        sequence: 9,
    };
    let blocked_auxiliary_cancelled = cancellable.schedule_next()?.id == packet_waiter
        && cancellable
            .block_current(packet_waiter, first_context, packet_wait)?
            .id
            == exiting_peer
        && cancellable.exit_current(exiting_peer).is_none()
        && cancellable.current.is_none()
        && cancellable.has_blocked_wait(packet_wait)
        && cancellable.remove_cancellable_domain(1)
        && !cancellable.has_blocked_wait(packet_wait)
        && cancellable.threads.iter().all(Option::is_none);
    let mut timed = Scheduler::new();
    let timed_waiter = timed.create(1, first_context)?;
    let timed_peer = timed.create(2, second_context)?;
    let timed_wait_released = timed.schedule_next()?.id == timed_waiter
        && timed
            .block_current(timed_waiter, first_context, packet_wait)?
            .id
            == timed_peer
        && timed.has_blocked_wait(packet_wait)
        && timed.wake_blocked_domain(1, packet_wait, crate::control_abi::PACKET_RESULT_TIMED_OUT)
        && !timed.has_blocked_wait(packet_wait)
        && timed.yield_current(timed_peer, second_context)?.id == timed_waiter
        && timed.thread(timed_waiter)?.context.rax == crate::control_abi::PACKET_RESULT_TIMED_OUT;
    let mut backpressure = Scheduler::new();
    let blocked_producer = backpressure.create(1, first_context)?;
    let draining_consumer = backpressure.create(2, second_context)?;
    let packet_space = WaitReason::PacketSpaceAvailable {
        object: crate::control_abi::PACKET_OBJECT,
    };
    let packet_space_wake_exact = backpressure.schedule_next()?.id == blocked_producer
        && backpressure
            .block_current(blocked_producer, first_context, packet_space)?
            .id
            == draining_consumer
        && backpressure.has_blocked_wait(packet_space)
        && backpressure.wake_blocked_peer(2, packet_space, 0)
        && !backpressure.has_blocked_wait(packet_space)
        && backpressure
            .yield_current(draining_consumer, second_context)?
            .id
            == blocked_producer
        && backpressure.thread(blocked_producer)?.context.rax == 0;
    Some(
        generation_valid
            && rollback
            && interrupt_idle_valid
            && replacement_removed
            && staged_not_implicitly_admitted
            && staged_explicitly_started
            && blocked_auxiliary_cancelled
            && timed_wait_released
            && packet_space_wake_exact,
    )
}

const fn test_context(rax: u64, rip: u64, rsp: u64, root: u64, fs_base: u64) -> UserContext {
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
        fs_base,
    }
}
