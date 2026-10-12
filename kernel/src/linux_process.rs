use core::{
    mem::{size_of, MaybeUninit},
    sync::atomic::{AtomicBool, AtomicU32, Ordering},
};

use crate::{address_space, arch::x86_64::cpu, memory, scheduler};

const PROCESS_LIMIT: usize = 4;
const PROCESS_HIERARCHY_LIMIT: usize = 6;
const PROCESS_PRIVATE_PAGE_LIMIT: usize = cpu::LINUX_PROCESS_MAPPING_LIMIT;
const PROCESS_LEASE_LIMIT: usize = PROCESS_HIERARCHY_LIMIT + PROCESS_PRIVATE_PAGE_LIMIT;
const PAGE_SIZE: usize = 4096;
const FORK_PRIVATE_PAGE_LIMIT: usize = 2;
const PRESENT: u64 = 1 << 0;
const WRITABLE: u64 = 1 << 1;
const USER: u64 = 1 << 2;
const NO_EXECUTE: u64 = 1 << 63;
const TEST_USER_ADDRESS: usize = 1usize << 39;
const PROCESS_RAX_INDEX: usize = 0;
static LIVE_PROCESS_TABLE_INITIALIZED: AtomicBool = AtomicBool::new(false);
static LIVE_PROCESS_REFRESH_COUNT: AtomicU32 = AtomicU32::new(0);
static mut LIVE_PROCESS_TABLE: MaybeUninit<ProcessTable> = MaybeUninit::uninit();
const PROCESS_CHILD_PROGRAM: [u8; 42] = [
    0x48, 0x85, 0xc0, // test rax, rax
    0x75, 0x23, // jne failure
    0x48, 0x83, 0xfb, 0x55, // cmp rbx, 0x55
    0x75, 0x1d, // jne failure
    0x49, 0x83, 0xff, 0x55, // cmp r15, 0x55
    0x75, 0x17, // jne failure
    0x64, 0xc6, 0x04, 0x25, 0, 0, 0, 0, 0x5a, // mov byte ptr fs:[0], 0x5a
    0xcd, 0x80, // context-check trap
    0xb8, 0x3c, 0, 0, 0, // mov eax, 60 (__NR_exit)
    0xbf, 0x2a, 0, 0, 0, // mov edi, 42
    0xcd, 0x80, // exit(42)
    0x0f, 0x0b, // failure: ud2
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ProcessHandle {
    slot: u8,
    generation: u32,
    pid: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ProcessState {
    Preparing,
    Running,
    Exiting,
    Zombie { status: u8 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ProcessExecutionContext {
    general: [u64; 15],
    rip: u64,
    rsp: u64,
    rflags: u64,
    fs_base: u64,
}

impl ProcessExecutionContext {
    fn is_valid(self, address_space: ProcessAddressSpace) -> bool {
        self.validation_error(address_space).is_none()
    }

    fn validation_error(self, address_space: ProcessAddressSpace) -> Option<u8> {
        const REQUIRED_RFLAGS: u64 = (1 << 1) | (1 << 9);
        const FORBIDDEN_RFLAGS: u64 = (3 << 12) | (1 << 14) | (1 << 17);
        if self.rip == 0
            || self.rip >= 1 << 47
            || self.rsp == 0
            || self.rsp >= 1 << 47
            || self.fs_base >= 1 << 47
            || self.rflags & REQUIRED_RFLAGS != REQUIRED_RFLAGS
            || self.rflags & FORBIDDEN_RFLAGS != 0
        {
            return Some(1);
        }
        let contains = |mapping: &cpu::LinuxActiveImageMapping, address: u64| {
            u64::try_from(mapping.address).ok().is_some_and(|start| {
                start
                    .checked_add(PAGE_SIZE as u64)
                    .is_some_and(|end| address >= start && address < end)
            })
        };
        let executable = address_space
            .mappings()
            .iter()
            .any(|mapping| mapping.executable && contains(mapping, self.rip));
        let writable_stack = address_space.mappings().iter().any(|mapping| {
            u64::try_from(mapping.address).ok().is_some_and(|start| {
                mapping.writable
                    && start
                        .checked_add(PAGE_SIZE as u64)
                        .is_some_and(|end| self.rsp > start && self.rsp <= end)
            })
        });
        if !executable {
            Some(2)
        } else if !writable_stack {
            Some(3)
        } else {
            None
        }
    }

    fn fork_child(self) -> Self {
        let mut child = self;
        child.general[PROCESS_RAX_INDEX] = 0;
        child
    }

    fn scheduler_context(
        self,
        address_space: ProcessAddressSpace,
    ) -> Option<scheduler::ProcessSchedulerContext> {
        self.is_valid(address_space).then_some(())?;
        scheduler::process_user_context(
            self.general,
            self.rip,
            self.rsp,
            self.rflags,
            address_space.root().address(),
            self.fs_base,
        )
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
struct ProcessAddressSpace {
    hierarchy: [memory::FrameLease; PROCESS_HIERARCHY_LIMIT],
    hierarchy_count: u8,
    mappings: [cpu::LinuxActiveImageMapping; PROCESS_PRIVATE_PAGE_LIMIT],
    mapping_count: u8,
}

impl ProcessAddressSpace {
    fn new(
        hierarchy: &[memory::FrameLease],
        mappings: &[cpu::LinuxActiveImageMapping],
    ) -> Option<Self> {
        if !(4..=PROCESS_HIERARCHY_LIMIT).contains(&hierarchy.len())
            || mappings.is_empty()
            || mappings.len() > PROCESS_PRIVATE_PAGE_LIMIT
            || !memory::frames_owned_by(memory::FrameOwner::KERNEL, hierarchy)
            || hierarchy
                .iter()
                .enumerate()
                .any(|(index, frame)| hierarchy[..index].contains(frame))
            || mappings.iter().enumerate().any(|(index, mapping)| {
                mapping.address == 0
                    || !mapping.address.is_multiple_of(PAGE_SIZE)
                    || mapping.writable && mapping.executable
                    || !memory::frames_owned_by(memory::FrameOwner::KERNEL, &[mapping.frame])
                    || hierarchy.contains(&mapping.frame)
                    || mappings[..index].iter().any(|prior| {
                        prior.address == mapping.address || prior.frame == mapping.frame
                    })
            })
            || !address_space::validate_linux_process_address_space(hierarchy, mappings)
        {
            return None;
        }
        let mut space = Self {
            hierarchy: [memory::FrameLease::EMPTY; PROCESS_HIERARCHY_LIMIT],
            hierarchy_count: u8::try_from(hierarchy.len()).ok()?,
            mappings: [cpu::LinuxActiveImageMapping::EMPTY; PROCESS_PRIVATE_PAGE_LIMIT],
            mapping_count: u8::try_from(mappings.len()).ok()?,
        };
        space.hierarchy[..hierarchy.len()].copy_from_slice(hierarchy);
        space.mappings[..mappings.len()].copy_from_slice(mappings);
        Some(space)
    }

    fn hierarchy(&self) -> &[memory::FrameLease] {
        &self.hierarchy[..usize::from(self.hierarchy_count)]
    }

    fn root(self) -> memory::FrameLease {
        self.hierarchy[0]
    }

    fn mappings(&self) -> &[cpu::LinuxActiveImageMapping] {
        &self.mappings[..usize::from(self.mapping_count)]
    }

    fn is_valid(self) -> bool {
        self.is_owned()
            && address_space::validate_linux_process_address_space(
                self.hierarchy(),
                self.mappings(),
            )
    }

    fn leases(self) -> ([memory::FrameLease; PROCESS_LEASE_LIMIT], usize) {
        let hierarchy_count = usize::from(self.hierarchy_count);
        let mapping_count = usize::from(self.mapping_count);
        let mut leases = [memory::FrameLease::EMPTY; PROCESS_LEASE_LIMIT];
        leases[..hierarchy_count].copy_from_slice(&self.hierarchy[..hierarchy_count]);
        for (lease, mapping) in leases[hierarchy_count..hierarchy_count + mapping_count]
            .iter_mut()
            .zip(&self.mappings[..mapping_count])
        {
            *lease = mapping.frame;
        }
        (leases, hierarchy_count + mapping_count)
    }

    fn is_owned(self) -> bool {
        let (leases, count) = self.leases();
        memory::frames_owned_by(memory::FrameOwner::KERNEL, &leases[..count])
    }

    fn is_live(self) -> bool {
        let (leases, count) = self.leases();
        memory::any_frame_lease_live(&leases[..count])
    }

    fn overlaps(self, other: Self) -> bool {
        let (left, left_count) = self.leases();
        let (right, right_count) = other.leases();
        left[..left_count].iter().any(|left| {
            right[..right_count]
                .iter()
                .any(|right| left.address() == right.address())
        })
    }

    fn scrub_and_release(self) -> bool {
        let (leases, count) = self.leases();
        memory::scrub_and_release_frames(memory::FrameOwner::KERNEL, &leases[..count])
    }
}

#[derive(Clone, Copy)]
struct Process {
    handle: ProcessHandle,
    parent: Option<ProcessHandle>,
    address_space: Option<ProcessAddressSpace>,
    retiring_address_space: Option<ProcessAddressSpace>,
    context: Option<ProcessExecutionContext>,
    scheduler_thread: Option<scheduler::ProcessThreadHandle>,
    state: ProcessState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct WaitResult {
    pid: u32,
    status: u8,
}

struct ProcessTable {
    processes: [Option<Process>; PROCESS_LIMIT],
    generations: [u32; PROCESS_LIMIT],
    next_pid: u32,
    init: Option<ProcessHandle>,
}

impl ProcessTable {
    const fn new() -> Self {
        Self {
            processes: [None; PROCESS_LIMIT],
            generations: [0; PROCESS_LIMIT],
            next_pid: 1,
            init: None,
        }
    }

    fn create_init(
        &mut self,
        address_space: ProcessAddressSpace,
        context: ProcessExecutionContext,
    ) -> Option<ProcessHandle> {
        if self.init.is_some()
            || self.next_pid != 1
            || !address_space.is_valid()
            || !context.is_valid(address_space)
        {
            return None;
        }
        let handle = self.allocate(
            None,
            Some(address_space),
            Some(context),
            ProcessState::Running,
        )?;
        if handle.pid != 1 {
            return None;
        }
        self.init = Some(handle);
        Some(handle)
    }

    fn prepare_fork(&mut self, parent: ProcessHandle) -> Option<ProcessHandle> {
        let parent_process = self.process(parent)?;
        if parent_process.state != ProcessState::Running
            || parent_process.address_space.is_none()
            || parent_process.context.is_none()
        {
            return None;
        }
        self.allocate(Some(parent), None, None, ProcessState::Preparing)
    }

    fn refresh_init(
        &mut self,
        address_space: ProcessAddressSpace,
        context: ProcessExecutionContext,
    ) -> bool {
        if !address_space.is_valid()
            || !context.is_valid(address_space)
            || self.processes.iter().flatten().count() != 1
        {
            return false;
        }
        let Some(init) = self.init else {
            return false;
        };
        let Some(process) = self.process_mut(init) else {
            return false;
        };
        if process.parent.is_some()
            || process.retiring_address_space.is_some()
            || process.scheduler_thread.is_some()
            || process.state != ProcessState::Running
        {
            return false;
        }
        process.address_space = Some(address_space);
        process.context = Some(context);
        true
    }

    fn commit_fork(
        &mut self,
        handle: ProcessHandle,
        address_space: ProcessAddressSpace,
        context: ProcessExecutionContext,
    ) -> bool {
        if !address_space.is_valid()
            || !context.is_valid(address_space)
            || self.processes.iter().flatten().any(|process| {
                process
                    .address_space
                    .is_some_and(|existing| existing.overlaps(address_space))
                    || process
                        .retiring_address_space
                        .is_some_and(|existing| existing.overlaps(address_space))
            })
        {
            return false;
        }
        let Some(process) = self.process_mut(handle) else {
            return false;
        };
        if process.state != ProcessState::Preparing
            || process.address_space.is_some()
            || process.retiring_address_space.is_some()
            || process.context.is_some()
            || process.scheduler_thread.is_some()
        {
            return false;
        }
        process.address_space = Some(address_space);
        process.context = Some(context);
        process.state = ProcessState::Running;
        true
    }

    fn admit_to_scheduler(
        &mut self,
        handle: ProcessHandle,
        run_queue: &mut scheduler::ProcessScheduler,
        domain: u64,
    ) -> bool {
        let Some(process) = self.process(handle).copied() else {
            return false;
        };
        let (Some(address_space), Some(context)) = (process.address_space, process.context) else {
            return false;
        };
        if process.state != ProcessState::Running || process.scheduler_thread.is_some() {
            return false;
        }
        let Some(scheduler_context) = context.scheduler_context(address_space) else {
            return false;
        };
        let Some(thread) = run_queue.admit(domain, scheduler_context) else {
            return false;
        };
        let Some(process) = self.process_mut(handle) else {
            let _ = run_queue.retire(thread);
            return false;
        };
        if process.state != ProcessState::Running || process.scheduler_thread.is_some() {
            let _ = run_queue.retire(thread);
            return false;
        }
        process.scheduler_thread = Some(thread);
        true
    }

    fn withdraw_from_scheduler(
        &mut self,
        handle: ProcessHandle,
        run_queue: &mut scheduler::ProcessScheduler,
    ) -> bool {
        let Some(thread) = self
            .process(handle)
            .and_then(|process| process.scheduler_thread)
        else {
            return false;
        };
        if !run_queue.retire(thread) {
            return false;
        }
        let Some(process) = self.process_mut(handle) else {
            return false;
        };
        if process.scheduler_thread != Some(thread) {
            return false;
        }
        process.scheduler_thread = None;
        true
    }

    fn commit_cloned_fork(
        &mut self,
        handle: ProcessHandle,
        child_hierarchy: &[memory::FrameLease],
        child_pages: &[memory::FrameLease],
    ) -> bool {
        let Some(preparing) = self.process(handle) else {
            return false;
        };
        if preparing.state != ProcessState::Preparing
            || preparing.address_space.is_some()
            || preparing.retiring_address_space.is_some()
        {
            return false;
        }
        let Some(parent_handle) = preparing.parent else {
            let _ = self.abort_fork(handle);
            return false;
        };
        let Some(parent) = self.process(parent_handle) else {
            let _ = self.abort_fork(handle);
            return false;
        };
        let Some(parent_space) = parent.address_space else {
            let _ = self.abort_fork(handle);
            return false;
        };
        let Some(parent_context) = parent.context else {
            let _ = self.abort_fork(handle);
            return false;
        };
        if parent.state != ProcessState::Running
            || !parent_space.is_valid()
            || child_hierarchy.len() != parent_space.hierarchy().len()
            || child_pages.len() != parent_space.mappings().len()
            || !self.resources_available(child_hierarchy, child_pages)
        {
            let _ = self.abort_fork(handle);
            return false;
        }

        let mut child_mappings = [cpu::LinuxActiveImageMapping::EMPTY; PROCESS_PRIVATE_PAGE_LIMIT];
        let mut parent_pages = [memory::FrameLease::EMPTY; PROCESS_PRIVATE_PAGE_LIMIT];
        for ((child, parent), frame) in child_mappings
            .iter_mut()
            .zip(parent_space.mappings())
            .zip(child_pages)
        {
            *child = cpu::LinuxActiveImageMapping {
                address: parent.address,
                frame: *frame,
                writable: parent.writable,
                executable: parent.executable,
            };
        }
        for (frame, mapping) in parent_pages.iter_mut().zip(parent_space.mappings()) {
            *frame = mapping.frame;
        }
        let mapping_count = parent_space.mappings().len();
        let published = copy_private_pages(&parent_pages[..mapping_count], child_pages);
        let published = published
            && address_space::clone_linux_process_hierarchy(
                parent_space.mappings(),
                parent_space.hierarchy(),
                child_hierarchy,
                child_pages,
            );
        let child_space = if published {
            ProcessAddressSpace::new(child_hierarchy, &child_mappings[..mapping_count])
        } else {
            None
        };
        if let Some(child_space) = child_space {
            if self.commit_fork(handle, child_space, parent_context.fork_child()) {
                return true;
            }
        }
        let _ = scrub_candidate_resources(child_hierarchy, child_pages);
        let _ = self.abort_fork(handle);
        false
    }

    fn resources_available(
        &self,
        hierarchy: &[memory::FrameLease],
        pages: &[memory::FrameLease],
    ) -> bool {
        if !(4..=PROCESS_HIERARCHY_LIMIT).contains(&hierarchy.len())
            || pages.is_empty()
            || pages.len() > PROCESS_PRIVATE_PAGE_LIMIT
            || !memory::frames_owned_by(memory::FrameOwner::KERNEL, hierarchy)
            || !memory::frames_owned_by(memory::FrameOwner::KERNEL, pages)
            || hierarchy
                .iter()
                .enumerate()
                .any(|(index, frame)| hierarchy[..index].contains(frame) || pages.contains(frame))
            || pages
                .iter()
                .enumerate()
                .any(|(index, frame)| pages[..index].contains(frame))
        {
            return false;
        }
        let overlaps = |space: ProcessAddressSpace| {
            let (leases, count) = space.leases();
            leases[..count].iter().any(|lease| {
                hierarchy
                    .iter()
                    .chain(pages)
                    .any(|candidate| candidate.address() == lease.address())
            })
        };
        !self.processes.iter().flatten().any(|process| {
            process.address_space.is_some_and(overlaps)
                || process.retiring_address_space.is_some_and(overlaps)
        })
    }

    fn abort_fork(&mut self, handle: ProcessHandle) -> bool {
        let Some(process) = self.process(handle) else {
            return false;
        };
        if process.state != ProcessState::Preparing
            || process.address_space.is_some()
            || process.retiring_address_space.is_some()
            || process.context.is_some()
            || process.scheduler_thread.is_some()
        {
            return false;
        }
        self.processes[usize::from(handle.slot)] = None;
        true
    }

    fn begin_exit(
        &mut self,
        handle: ProcessHandle,
        expected_space: ProcessAddressSpace,
    ) -> Option<ProcessAddressSpace> {
        let process = self.process_mut(handle)?;
        if process.state != ProcessState::Running
            || process.address_space != Some(expected_space)
            || process.retiring_address_space.is_some()
            || process.context.is_none()
            || process.scheduler_thread.is_some()
            || !expected_space.is_owned()
        {
            return None;
        }
        process.address_space = None;
        process.retiring_address_space = Some(expected_space);
        process.context = None;
        process.state = ProcessState::Exiting;
        Some(expected_space)
    }

    fn exit(&mut self, handle: ProcessHandle, status: u8) -> bool {
        let Some(process) = self.process_mut(handle) else {
            return false;
        };
        if process.state != ProcessState::Exiting
            || process.address_space.is_some()
            || process.context.is_some()
            || process.scheduler_thread.is_some()
        {
            return false;
        }
        let Some(retiring) = process.retiring_address_space else {
            return false;
        };
        if retiring.is_live() {
            return false;
        }
        process.retiring_address_space = None;
        process.state = ProcessState::Zombie { status };
        true
    }

    fn wait(&mut self, parent: ProcessHandle, pid: u32) -> Option<WaitResult> {
        if self.process(parent)?.state != ProcessState::Running {
            return None;
        }
        let slot = self.processes.iter().position(|candidate| {
            candidate.is_some_and(|process| {
                process.handle.pid == pid
                    && process.parent == Some(parent)
                    && matches!(process.state, ProcessState::Zombie { .. })
            })
        })?;
        let process = self.processes[slot].take()?;
        let ProcessState::Zombie { status } = process.state else {
            return None;
        };
        Some(WaitResult { pid, status })
    }

    fn allocate(
        &mut self,
        parent: Option<ProcessHandle>,
        address_space: Option<ProcessAddressSpace>,
        context: Option<ProcessExecutionContext>,
        state: ProcessState,
    ) -> Option<ProcessHandle> {
        let slot = self.processes.iter().position(Option::is_none)?;
        let generation = self.generations[slot].checked_add(1)?;
        let pid = self.next_pid;
        self.next_pid = self.next_pid.checked_add(1)?;
        let handle = ProcessHandle {
            slot: u8::try_from(slot).ok()?,
            generation,
            pid,
        };
        self.generations[slot] = generation;
        self.processes[slot] = Some(Process {
            handle,
            parent,
            address_space,
            retiring_address_space: None,
            context,
            scheduler_thread: None,
            state,
        });
        Some(handle)
    }

    fn process(&self, handle: ProcessHandle) -> Option<&Process> {
        self.processes
            .get(usize::from(handle.slot))?
            .as_ref()
            .filter(|process| process.handle == handle)
    }

    fn process_mut(&mut self, handle: ProcessHandle) -> Option<&mut Process> {
        self.processes
            .get_mut(usize::from(handle.slot))?
            .as_mut()
            .filter(|process| process.handle == handle)
    }
}

fn test_mapping(
    address: usize,
    frame: memory::FrameLease,
    writable: bool,
    executable: bool,
) -> cpu::LinuxActiveImageMapping {
    cpu::LinuxActiveImageMapping {
        address,
        frame,
        writable,
        executable,
    }
}

fn write_test_hierarchy(
    hierarchy: &[memory::FrameLease],
    mappings: &[cpu::LinuxActiveImageMapping],
) -> bool {
    if hierarchy.len() != 4 || mappings.is_empty() {
        return false;
    }
    let mut tables = [core::ptr::null_mut::<u64>(); 4];
    for (table, frame) in tables.iter_mut().zip(hierarchy) {
        let Some(address) = frame.direct_map_address() else {
            return false;
        };
        *table = address as *mut u64;
        unsafe { core::ptr::write_bytes(*table, 0, PAGE_SIZE / size_of::<u64>()) };
    }
    let branch_flags = PRESENT | WRITABLE | USER;
    for mapping in mappings {
        let indexes = [
            (mapping.address >> 39) & 0x1ff,
            (mapping.address >> 30) & 0x1ff,
            (mapping.address >> 21) & 0x1ff,
            (mapping.address >> 12) & 0x1ff,
        ];
        unsafe {
            tables[0]
                .add(indexes[0])
                .write(hierarchy[1].address() | branch_flags);
            tables[1]
                .add(indexes[1])
                .write(hierarchy[2].address() | branch_flags);
            tables[2]
                .add(indexes[2])
                .write(hierarchy[3].address() | branch_flags);
            tables[3].add(indexes[3]).write(
                mapping.frame.address()
                    | PRESENT
                    | USER
                    | if mapping.writable { WRITABLE } else { 0 }
                    | if mapping.executable { 0 } else { NO_EXECUTE },
            );
        }
    }
    true
}

fn test_context() -> ProcessExecutionContext {
    ProcessExecutionContext {
        general: [0x55; 15],
        rip: TEST_USER_ADDRESS as u64,
        rsp: (TEST_USER_ADDRESS + 2 * PAGE_SIZE) as u64,
        rflags: (1 << 1) | (1 << 9),
        fs_base: (TEST_USER_ADDRESS + PAGE_SIZE) as u64,
    }
}

fn verify_user_kernel_mapping_rejected(root: memory::FrameLease) -> bool {
    let Some(address) = root.direct_map_address() else {
        return false;
    };
    let entry = unsafe { (address as *mut u64).add(256) };
    let original = unsafe { entry.read() };
    unsafe { entry.write(original | USER) };
    let rejected = !address_space::verify_linux_process_root_activation(root);
    unsafe { entry.write(original) };
    rejected && unsafe { entry.read() } == original
}

pub(crate) fn verify_lifecycle_contract() -> bool {
    let Some(before) = memory::owner_live_frame_count(memory::FrameOwner::KERNEL) else {
        return false;
    };
    let Some(frames) = memory::allocate_frames::<18>(memory::FrameOwner::KERNEL) else {
        return false;
    };
    let init_mappings = [
        test_mapping(TEST_USER_ADDRESS, frames[4], false, true),
        test_mapping(TEST_USER_ADDRESS + PAGE_SIZE, frames[5], true, false),
    ];
    if !write_test_hierarchy(&frames[0..4], &init_mappings)
        || address_space::verify_linux_process_root_activation(frames[0])
        || !address_space::inherit_linux_process_kernel_mappings(frames[0])
    {
        return false;
    }
    let Some(init_space) = ProcessAddressSpace::new(&frames[0..4], &init_mappings) else {
        return false;
    };
    let init_context = test_context();
    let mut table = ProcessTable::new();
    let mut invalid_context = init_context;
    invalid_context.rip = (TEST_USER_ADDRESS + PAGE_SIZE) as u64;
    if table.create_init(init_space, invalid_context).is_some() {
        return false;
    }
    invalid_context = init_context;
    invalid_context.rsp = (TEST_USER_ADDRESS + PAGE_SIZE) as u64;
    if table.create_init(init_space, invalid_context).is_some() {
        return false;
    }
    invalid_context = init_context;
    invalid_context.rflags |= 3 << 12;
    if table.create_init(init_space, invalid_context).is_some() {
        return false;
    }
    invalid_context = init_context;
    invalid_context.fs_base = 1 << 47;
    if table.create_init(init_space, invalid_context).is_some() {
        return false;
    }
    let Some(init) = table.create_init(init_space, init_context) else {
        return false;
    };
    let Some(init_scheduler_context) = init_context.scheduler_context(init_space) else {
        return false;
    };
    let mut run_queue = scheduler::ProcessScheduler::new();
    if table.admit_to_scheduler(init, &mut run_queue, 0)
        || !table.admit_to_scheduler(init, &mut run_queue, 1)
        || table.admit_to_scheduler(init, &mut run_queue, 1)
    {
        return false;
    }
    let Some(rejected_child) = table.prepare_fork(init) else {
        return false;
    };
    let shared_page_mappings = [
        test_mapping(TEST_USER_ADDRESS, frames[4], false, true),
        test_mapping(TEST_USER_ADDRESS + PAGE_SIZE, frames[11], true, false),
    ];
    if !write_test_hierarchy(&frames[6..10], &shared_page_mappings)
        || !address_space::inherit_linux_process_kernel_mappings(frames[6])
    {
        return false;
    }
    let Some(shared_page_space) = ProcessAddressSpace::new(&frames[6..10], &shared_page_mappings)
    else {
        return false;
    };
    if table.commit_fork(rejected_child, shared_page_space, init_context.fork_child()) {
        return false;
    }
    let first_mappings = [
        test_mapping(TEST_USER_ADDRESS, frames[10], false, true),
        test_mapping(TEST_USER_ADDRESS + PAGE_SIZE, frames[11], true, false),
    ];
    let second_mappings = [
        test_mapping(TEST_USER_ADDRESS, frames[16], false, true),
        test_mapping(TEST_USER_ADDRESS + PAGE_SIZE, frames[17], true, false),
    ];
    if !write_test_hierarchy(&frames[6..10], &first_mappings)
        || !address_space::inherit_linux_process_kernel_mappings(frames[6])
        || !verify_user_kernel_mapping_rejected(frames[6])
        || !write_test_hierarchy(&frames[12..16], &second_mappings)
        || !address_space::inherit_linux_process_kernel_mappings(frames[12])
    {
        return false;
    }
    let mut wrong_permission = first_mappings;
    wrong_permission[0].writable = true;
    wrong_permission[0].executable = false;
    let mut wrong_address = first_mappings;
    wrong_address[0].address += 2 * PAGE_SIZE;
    if ProcessAddressSpace::new(&frames[6..10], &wrong_permission).is_some()
        || ProcessAddressSpace::new(&frames[6..10], &wrong_address).is_some()
    {
        return false;
    }
    let (Some(first_space), Some(second_space)) = (
        ProcessAddressSpace::new(&frames[6..10], &first_mappings),
        ProcessAddressSpace::new(&frames[12..16], &second_mappings),
    ) else {
        return false;
    };
    let Some(stale_generation) = frames[6].generation().checked_add(1) else {
        return false;
    };
    let Some(stale_root) = memory::FrameLease::from_parts(frames[6].address(), stale_generation)
    else {
        return false;
    };
    let mut stale_hierarchy: [memory::FrameLease; 4] =
        core::array::from_fn(|index| frames[6 + index]);
    stale_hierarchy[0] = stale_root;
    let stale_space = ProcessAddressSpace::new(&stale_hierarchy, &first_mappings);
    let mut stale_expected = first_space;
    stale_expected.hierarchy[0] = stale_root;
    if init.pid != 1
        || rejected_child.pid != 2
        || table.create_init(first_space, init_context).is_some()
        || table.commit_fork(rejected_child, init_space, init_context.fork_child())
        || stale_space.is_some()
        || address_space::verify_linux_process_root_activation(stale_root)
        || table.wait(init, rejected_child.pid).is_some()
        || table.exit(rejected_child, 1)
        || table.commit_cloned_fork(rejected_child, &frames[0..4], &frames[4..6])
        || !init_space.is_valid()
        || table.commit_fork(rejected_child, first_space, init_context.fork_child())
        || table.abort_fork(rejected_child)
    {
        return false;
    }
    let Some(first_child) = table.prepare_fork(init) else {
        return false;
    };
    let (Some(parent_page), Some(parent_stack), Some(child_page), Some(child_stack)) = (
        frames[4].direct_map_address(),
        frames[5].direct_map_address(),
        frames[10].direct_map_address(),
        frames[11].direct_map_address(),
    ) else {
        return false;
    };
    unsafe {
        core::ptr::copy_nonoverlapping(
            PROCESS_CHILD_PROGRAM.as_ptr(),
            parent_page as *mut u8,
            PROCESS_CHILD_PROGRAM.len(),
        );
        (parent_page as *mut u8).add(PAGE_SIZE - 1).write(0x32);
    }
    if first_child.pid != 3
        || !table.commit_cloned_fork(first_child, &frames[6..10], &frames[10..12])
        || unsafe {
            core::slice::from_raw_parts(child_page as *const u8, PROCESS_CHILD_PROGRAM.len())
        } != PROCESS_CHILD_PROGRAM
        || unsafe { (child_page as *const u8).add(PAGE_SIZE - 1).read() } != 0x32
        || table.commit_fork(first_child, second_space, init_context.fork_child())
        || table.wait(init, first_child.pid).is_some()
        || table.exit(first_child, 42)
        || table.begin_exit(first_child, second_space).is_some()
        || table.begin_exit(first_child, stale_expected).is_some()
    {
        return false;
    }
    let Some(child_context) = table
        .process(first_child)
        .and_then(|process| process.context)
    else {
        return false;
    };
    if child_context != init_context.fork_child()
        || child_context.general[PROCESS_RAX_INDEX] != 0
        || table.process(init).and_then(|process| process.context) != Some(init_context)
    {
        return false;
    }
    let Some(child_scheduler_context) = child_context.scheduler_context(first_space) else {
        return false;
    };
    if !table.admit_to_scheduler(first_child, &mut run_queue, 1)
        || table.admit_to_scheduler(first_child, &mut run_queue, 1)
    {
        return false;
    }
    let (Some(init_thread), Some(child_thread)) = (
        table
            .process(init)
            .and_then(|process| process.scheduler_thread),
        table
            .process(first_child)
            .and_then(|process| process.scheduler_thread),
    ) else {
        return false;
    };
    let Some(initial) = run_queue.schedule_next() else {
        return false;
    };
    let Some(selected_child) = run_queue.yield_current(init_thread, init_scheduler_context) else {
        return false;
    };
    if selected_child.handle() != child_thread
        || selected_child.context() != child_scheduler_context
    {
        return false;
    }
    let Some(child_exit_status) = address_space::execute_linux_process_child(
        first_space.root(),
        child_context.rip,
        child_context.rsp,
        child_context.fs_base,
        child_context.general,
    ) else {
        return false;
    };
    if child_exit_status != 42
        || unsafe { (child_stack as *const u8).read() } != 0x5a
        || unsafe { (parent_stack as *const u8).read() } != 0
    {
        return false;
    }
    let Some(resumed_parent) = run_queue.yield_current(child_thread, child_scheduler_context)
    else {
        return false;
    };
    if initial.handle() != init_thread
        || initial.context() != init_scheduler_context
        || resumed_parent.handle() != init_thread
        || resumed_parent.context() != init_scheduler_context
        || table.begin_exit(first_child, first_space).is_some()
        || !table.withdraw_from_scheduler(first_child, &mut run_queue)
        || !table.withdraw_from_scheduler(init, &mut run_queue)
        || run_queue.retire(child_thread)
        || table.withdraw_from_scheduler(first_child, &mut run_queue)
    {
        return false;
    }
    if unsafe { (parent_page as *const u8).read() } != PROCESS_CHILD_PROGRAM[0] {
        return false;
    }
    let Some(first_resources) = table.begin_exit(first_child, first_space) else {
        return false;
    };
    let (first_leases, first_lease_count) = first_resources.leases();
    if table.exit(first_child, child_exit_status)
        || !memory::scrub_and_release_frames(memory::FrameOwner::KERNEL, &first_leases[..1])
        || table.exit(first_child, child_exit_status)
        || !memory::scrub_and_release_frames(
            memory::FrameOwner::KERNEL,
            &first_leases[1..first_lease_count],
        )
        || !table.exit(first_child, child_exit_status)
        || table.wait(first_child, first_child.pid).is_some()
        || table.wait(init, first_child.pid)
            != Some(WaitResult {
                pid: 3,
                status: child_exit_status,
            })
        || table.exit(first_child, child_exit_status)
    {
        return false;
    }
    let Some(second_child) = table.prepare_fork(init) else {
        return false;
    };
    if second_child.slot != first_child.slot
        || second_child.generation <= first_child.generation
        || second_child.pid != 4
        || table.begin_exit(first_child, second_space).is_some()
        || table.commit_fork(second_child, first_space, init_context.fork_child())
        || table.commit_fork(second_child, second_space, invalid_context)
        || !table.commit_fork(second_child, second_space, init_context.fork_child())
    {
        return false;
    }
    let Some(second_resources) = table.begin_exit(second_child, second_space) else {
        return false;
    };
    if !second_resources.scrub_and_release()
        || !table.exit(second_child, 7)
        || table.wait(init, second_child.pid) != Some(WaitResult { pid: 4, status: 7 })
    {
        return false;
    }
    let Some(init_resources) = table.begin_exit(init, init_space) else {
        return false;
    };
    init_resources.scrub_and_release()
        && table.exit(init, 0)
        && memory::owner_live_frame_count(memory::FrameOwner::KERNEL) == Some(before)
}

pub(crate) fn install_live_process_registry(
    snapshot: &cpu::LinuxProcessAddressSpaceSnapshot,
    general: [u64; 15],
    rip: u64,
    rsp: u64,
    rflags: u64,
    fs_base: u64,
) -> bool {
    if LIVE_PROCESS_TABLE_INITIALIZED.load(Ordering::Acquire) {
        return false;
    }
    let Some(address_space) = ProcessAddressSpace::new(snapshot.hierarchy(), snapshot.mappings())
    else {
        return false;
    };
    let context = ProcessExecutionContext {
        general,
        rip,
        rsp,
        rflags,
        fs_base,
    };
    let mut invalid = context;
    invalid.rip = 0;
    if invalid.is_valid(address_space) || !context.is_valid(address_space) {
        return false;
    }
    let table = core::ptr::addr_of_mut!(LIVE_PROCESS_TABLE).cast::<ProcessTable>();
    unsafe {
        let processes = core::ptr::addr_of_mut!((*table).processes).cast::<Option<Process>>();
        let generations = core::ptr::addr_of_mut!((*table).generations).cast::<u32>();
        for index in 0..PROCESS_LIMIT {
            processes.add(index).write(None);
            generations.add(index).write(0);
        }
        core::ptr::addr_of_mut!((*table).next_pid).write(1);
        core::ptr::addr_of_mut!((*table).init).write(None);
    }
    let table = unsafe { &mut *table };
    if table.create_init(address_space, context).is_none() {
        return false;
    }
    LIVE_PROCESS_REFRESH_COUNT.store(0, Ordering::Release);
    LIVE_PROCESS_TABLE_INITIALIZED.store(true, Ordering::Release);
    true
}

pub(crate) fn refresh_live_process_registry(
    snapshot: &cpu::LinuxProcessAddressSpaceSnapshot,
    general: [u64; 15],
    rip: u64,
    rsp: u64,
    rflags: u64,
    fs_base: u64,
) -> bool {
    let Some(table) = live_process_table_mut() else {
        return false;
    };
    let Some(address_space) = ProcessAddressSpace::new(snapshot.hierarchy(), snapshot.mappings())
    else {
        return false;
    };
    let refreshed = table.refresh_init(
        address_space,
        ProcessExecutionContext {
            general,
            rip,
            rsp,
            rflags,
            fs_base,
        },
    );
    if refreshed {
        LIVE_PROCESS_REFRESH_COUNT.fetch_add(1, Ordering::SeqCst);
    }
    refreshed
}

pub(crate) fn finish_live_process_registry() -> bool {
    if !LIVE_PROCESS_TABLE_INITIALIZED.load(Ordering::Acquire) {
        return true;
    }
    let Some(table) = live_process_table_mut() else {
        return false;
    };
    let valid = table.init.is_some()
        && LIVE_PROCESS_REFRESH_COUNT.load(Ordering::Acquire) >= 2
        && table.processes.iter().flatten().count() == 1
        && table.processes.iter().flatten().all(|process| {
            process.parent.is_none()
                && process.state == ProcessState::Running
                && process.retiring_address_space.is_none()
                && process.scheduler_thread.is_none()
        });
    if !valid {
        return false;
    }
    LIVE_PROCESS_TABLE_INITIALIZED.store(false, Ordering::Release);
    LIVE_PROCESS_REFRESH_COUNT.store(0, Ordering::Release);
    true
}

#[inline(never)]
pub(crate) fn verify_live_fork_clone_transaction() -> bool {
    let Some(before) = memory::owner_live_frame_count(memory::FrameOwner::KERNEL) else {
        return false;
    };
    let Some(table) = live_process_table_mut() else {
        return false;
    };
    let Some(parent_handle) = table.init else {
        return false;
    };
    let Some(parent) = table.process(parent_handle) else {
        return false;
    };
    let (Some(parent_space), Some(parent_context)) = (parent.address_space, parent.context) else {
        return false;
    };
    if parent.state != ProcessState::Running || !parent_space.is_valid() {
        return false;
    }
    let Some(child_handle) = table.prepare_fork(parent_handle) else {
        return false;
    };
    let Some(child_hierarchy) =
        memory::allocate_frames::<PROCESS_HIERARCHY_LIMIT>(memory::FrameOwner::KERNEL)
    else {
        let _ = table.abort_fork(child_handle);
        return false;
    };
    let Some(child_pages) =
        memory::allocate_frames::<PROCESS_PRIVATE_PAGE_LIMIT>(memory::FrameOwner::KERNEL)
    else {
        let _released =
            memory::scrub_and_release_frames(memory::FrameOwner::KERNEL, &child_hierarchy);
        let _aborted = table.abort_fork(child_handle);
        return false;
    };
    let hierarchy_count = parent_space.hierarchy().len();
    let mapping_count = parent_space.mappings().len();
    let published = table.commit_cloned_fork(
        child_handle,
        &child_hierarchy[..hierarchy_count],
        &child_pages[..mapping_count],
    );
    let hierarchy_extras_released = hierarchy_count == child_hierarchy.len()
        || memory::scrub_and_release_frames(
            memory::FrameOwner::KERNEL,
            &child_hierarchy[hierarchy_count..],
        );
    let page_extras_released = mapping_count == child_pages.len()
        || memory::scrub_and_release_frames(
            memory::FrameOwner::KERNEL,
            &child_pages[mapping_count..],
        );
    if !published {
        return false;
    }
    let Some(child) = table.process(child_handle) else {
        return false;
    };
    let Some(child_space) = child.address_space else {
        return false;
    };
    let child_valid = child.parent == Some(parent_handle)
        && child.state == ProcessState::Running
        && child.context == Some(parent_context.fork_child())
        && child_space.is_valid()
        && !child_space.overlaps(parent_space)
        && child_space.hierarchy().len() == hierarchy_count
        && child_space.mappings().len() == mapping_count
        && child_space
            .mappings()
            .iter()
            .zip(parent_space.mappings())
            .all(|(child, parent)| {
                child.address == parent.address
                    && child.writable == parent.writable
                    && child.executable == parent.executable
                    && child.frame != parent.frame
                    && child
                        .frame
                        .direct_map_address()
                        .zip(parent.frame.direct_map_address())
                        .is_some_and(|(child_address, parent_address)| {
                            // SAFETY: Both leases remain owned by the process table for
                            // the duration of this comparison, and a direct-map lookup
                            // validates the complete physical page range.
                            unsafe {
                                core::slice::from_raw_parts(child_address as *const u8, PAGE_SIZE)
                                    == core::slice::from_raw_parts(
                                        parent_address as *const u8,
                                        PAGE_SIZE,
                                    )
                            }
                        })
            });
    let Some(retiring) = table.begin_exit(child_handle, child_space) else {
        return false;
    };
    let resources_released = retiring.scrub_and_release();
    let exited = resources_released && table.exit(child_handle, 42);
    let waited = exited
        && table.wait(parent_handle, child_handle.pid)
            == Some(WaitResult {
                pid: child_handle.pid,
                status: 42,
            });
    let retired = child_valid
        && waited
        && table.processes.iter().flatten().count() == 1
        && table.process(parent_handle).is_some_and(|process| {
            process.address_space == Some(parent_space)
                && process.context == Some(parent_context)
                && process.state == ProcessState::Running
        })
        && memory::owner_live_frame_count(memory::FrameOwner::KERNEL) == Some(before);
    hierarchy_extras_released && page_extras_released && retired
}

fn live_process_table_mut() -> Option<&'static mut ProcessTable> {
    if !LIVE_PROCESS_TABLE_INITIALIZED.load(Ordering::Acquire) {
        return None;
    }
    Some(unsafe { &mut *core::ptr::addr_of_mut!(LIVE_PROCESS_TABLE).cast::<ProcessTable>() })
}

fn scrub_candidate_resources(
    hierarchy: &[memory::FrameLease],
    pages: &[memory::FrameLease],
) -> bool {
    if hierarchy.len() + pages.len() > PROCESS_LEASE_LIMIT {
        return false;
    }
    let mut leases = [memory::FrameLease::EMPTY; PROCESS_LEASE_LIMIT];
    leases[..hierarchy.len()].copy_from_slice(hierarchy);
    leases[hierarchy.len()..hierarchy.len() + pages.len()].copy_from_slice(pages);
    memory::scrub_and_release_frames(
        memory::FrameOwner::KERNEL,
        &leases[..hierarchy.len() + pages.len()],
    )
}

pub(crate) fn copy_private_pages(
    source: &[memory::FrameLease],
    destination: &[memory::FrameLease],
) -> bool {
    if source.is_empty()
        || source.len() != destination.len()
        || source.len() > PROCESS_PRIVATE_PAGE_LIMIT
        || !memory::frames_owned_by(memory::FrameOwner::KERNEL, source)
        || !memory::frames_owned_by(memory::FrameOwner::KERNEL, destination)
        || source
            .iter()
            .enumerate()
            .any(|(index, frame)| source[..index].contains(frame) || destination.contains(frame))
        || destination
            .iter()
            .enumerate()
            .any(|(index, frame)| destination[..index].contains(frame))
    {
        return false;
    }
    if source.iter().zip(destination).any(|(source, destination)| {
        source.direct_map_address().is_none() || destination.direct_map_address().is_none()
    }) {
        return false;
    }
    for (source_frame, destination_frame) in source.iter().zip(destination) {
        let (Some(source_address), Some(destination_address)) = (
            source_frame.direct_map_address(),
            destination_frame.direct_map_address(),
        ) else {
            return false;
        };
        // SAFETY: all leases and direct-map extents were validated before the
        // first write, and source/destination frames are live and disjoint.
        unsafe {
            core::ptr::copy_nonoverlapping(
                source_address as *const u8,
                destination_address as *mut u8,
                PAGE_SIZE,
            );
        }
    }
    true
}

pub(crate) fn verify_private_page_copy_contract() -> bool {
    let Some(before) = memory::owner_live_frame_count(memory::FrameOwner::KERNEL) else {
        return false;
    };
    let Some(source) =
        memory::allocate_frames::<FORK_PRIVATE_PAGE_LIMIT>(memory::FrameOwner::KERNEL)
    else {
        return false;
    };
    let Some(destination) =
        memory::allocate_frames::<FORK_PRIVATE_PAGE_LIMIT>(memory::FrameOwner::KERNEL)
    else {
        let _ = memory::scrub_and_release_frames(memory::FrameOwner::KERNEL, &source);
        return false;
    };

    let result = (|| {
        let source_addresses = source.map(memory::FrameLease::direct_map_address);
        let destination_addresses = destination.map(memory::FrameLease::direct_map_address);
        if source_addresses.iter().any(Option::is_none)
            || destination_addresses.iter().any(Option::is_none)
        {
            return None;
        }
        for (index, source_address) in source_addresses.iter().copied().enumerate() {
            let source_address = source_address?;
            // SAFETY: allocation returned a private zeroed frame with a valid
            // complete direct-map extent.
            unsafe {
                (source_address as *mut u8).write(0x41 + index as u8);
                (source_address as *mut u8)
                    .add(PAGE_SIZE - 1)
                    .write(0x61 + index as u8);
            }
        }

        let stale_generation = destination[0].generation().checked_add(1)?;
        let stale = memory::FrameLease::from_parts(destination[0].address(), stale_generation)?;
        let mut stale_destination = destination;
        stale_destination[0] = stale;
        if copy_private_pages(&source, &stale_destination)
            || copy_private_pages(&source, &source)
            || destination_addresses.into_iter().any(|address| {
                address.is_none_or(|address| unsafe {
                    core::slice::from_raw_parts(address as *const u8, PAGE_SIZE)
                        .iter()
                        .any(|byte| *byte != 0)
                })
            })
            || !copy_private_pages(&source, &destination)
        {
            return None;
        }

        for (source_address, destination_address) in source_addresses
            .iter()
            .copied()
            .zip(destination_addresses.iter().copied())
        {
            let source_address = source_address?;
            let destination_address = destination_address?;
            // SAFETY: both complete direct-map extents remain live.
            let (source_page, destination_page) = unsafe {
                (
                    core::slice::from_raw_parts(source_address as *const u8, PAGE_SIZE),
                    core::slice::from_raw_parts(destination_address as *const u8, PAGE_SIZE),
                )
            };
            if source_page != destination_page {
                return None;
            }
        }

        let first_source = source_addresses[0]?;
        let first_destination = destination_addresses[0]?;
        // SAFETY: the destination is a distinct, live, private frame.
        unsafe { (first_destination as *mut u8).write(0xff) };
        Some(unsafe { (first_source as *const u8).read() == 0x41 })
    })()
    .unwrap_or(false);

    let destination_released =
        memory::scrub_and_release_frames(memory::FrameOwner::KERNEL, &destination);
    let source_released = memory::scrub_and_release_frames(memory::FrameOwner::KERNEL, &source);
    result
        && destination_released
        && source_released
        && memory::owner_live_frame_count(memory::FrameOwner::KERNEL) == Some(before)
}
