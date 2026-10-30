use core::{
    cell::UnsafeCell,
    mem::{size_of, MaybeUninit},
    sync::atomic::{AtomicBool, AtomicU32, AtomicU8, Ordering},
};

use crate::{address_space, arch::x86_64::cpu, control_abi, linux_user, memory, scheduler};

pub(crate) const PROCESS_LIMIT: usize = 16;
pub(crate) const PROCESS_RUNTIME_LIMIT: usize = 6;
const PROCESS_HIERARCHY_LIMIT: usize = cpu::LINUX_PROCESS_HIERARCHY_LIMIT;
const PROCESS_PRIVATE_PAGE_LIMIT: usize = cpu::LINUX_PROCESS_MAPPING_LIMIT;
const PROCESS_PRIVATE_COPY_LIMIT: usize = cpu::LINUX_PROCESS_MAPPING_LIMIT;
const PAGE_SIZE: usize = 4096;
const FORK_PRIVATE_PAGE_LIMIT: usize = 2;
const PRESENT: u64 = 1 << 0;
const WRITABLE: u64 = 1 << 1;
const USER: u64 = 1 << 2;
const NO_EXECUTE: u64 = 1 << 63;
const TEST_USER_ADDRESS: usize = 1usize << 39;
const PROCESS_RAX_INDEX: usize = 0;
const HOST_PROCESS_INSTANCE: usize = control_abi::DOMAIN_LIMIT;
const PROCESS_INSTANCE_LIMIT: usize = control_abi::DOMAIN_LIMIT + 1;
const PROCESS_INSTANCE_NONE: u8 = u8::MAX;
const LIVE_FRAME_REFERENCE_SET_CAPACITY: usize = 32_768;

struct LiveForkScratch {
    child_mappings: [cpu::LinuxActiveImageMapping; PROCESS_PRIVATE_PAGE_LIMIT],
    parent_pages: [memory::FrameLease; PROCESS_PRIVATE_PAGE_LIMIT],
}

struct ProcessRuntimeOwner(UnsafeCell<ProcessRuntimeState>);

unsafe impl Sync for ProcessRuntimeOwner {}

struct ProcessRuntimeState {
    table_initialized: [AtomicBool; PROCESS_INSTANCE_LIMIT],
    refresh_count: [AtomicU32; PROCESS_INSTANCE_LIMIT],
    active_instance: AtomicU8,
    fork_child_executed: AtomicBool,
    fork_execution_stage: AtomicU32,
    fork_child_diagnostic: AtomicU32,
    tables: UnsafeCell<[ProcessTable; PROCESS_INSTANCE_LIMIT]>,
    frame_reference_set: UnsafeCell<[u64; LIVE_FRAME_REFERENCE_SET_CAPACITY]>,
    fork_scratch: UnsafeCell<LiveForkScratch>,
    fork_allocation_scratch: UnsafeCell<[memory::FrameLease; PROCESS_PRIVATE_COPY_LIMIT]>,
    fork_hierarchy_allocation_scratch: UnsafeCell<[memory::FrameLease; PROCESS_HIERARCHY_LIMIT]>,
    address_space_storage:
        UnsafeCell<[[ProcessAddressSpaceStorage; PROCESS_RUNTIME_LIMIT]; PROCESS_INSTANCE_LIMIT]>,
    test_table: UnsafeCell<MaybeUninit<ProcessTable>>,
}

static PROCESS_RUNTIME: ProcessRuntimeOwner =
    ProcessRuntimeOwner(UnsafeCell::new(ProcessRuntimeState {
        table_initialized: [const { AtomicBool::new(false) }; PROCESS_INSTANCE_LIMIT],
        refresh_count: [const { AtomicU32::new(0) }; PROCESS_INSTANCE_LIMIT],
        active_instance: AtomicU8::new(PROCESS_INSTANCE_NONE),
        fork_child_executed: AtomicBool::new(false),
        fork_execution_stage: AtomicU32::new(0),
        fork_child_diagnostic: AtomicU32::new(0),
        tables: UnsafeCell::new([const { ProcessTable::new() }; PROCESS_INSTANCE_LIMIT]),
        frame_reference_set: UnsafeCell::new([0; LIVE_FRAME_REFERENCE_SET_CAPACITY]),
        fork_scratch: UnsafeCell::new(LiveForkScratch {
            child_mappings: [cpu::LinuxActiveImageMapping::EMPTY; PROCESS_PRIVATE_PAGE_LIMIT],
            parent_pages: [memory::FrameLease::EMPTY; PROCESS_PRIVATE_PAGE_LIMIT],
        }),
        fork_allocation_scratch: UnsafeCell::new(
            [memory::FrameLease::EMPTY; PROCESS_PRIVATE_COPY_LIMIT],
        ),
        fork_hierarchy_allocation_scratch: UnsafeCell::new(
            [memory::FrameLease::EMPTY; PROCESS_HIERARCHY_LIMIT],
        ),
        address_space_storage: UnsafeCell::new(
            [[ProcessAddressSpaceStorage::EMPTY; PROCESS_RUNTIME_LIMIT]; PROCESS_INSTANCE_LIMIT],
        ),
        test_table: UnsafeCell::new(MaybeUninit::uninit()),
    }));

fn process_runtime() -> &'static ProcessRuntimeState {
    // SAFETY: The owner has boot lifetime. Mutable fields use atomics or
    // UnsafeCell and retain their existing single-CPU lifecycle serialization
    // until Gate 8 adds cross-CPU locking.
    unsafe { &*PROCESS_RUNTIME.0.get() }
}

fn mapping_requires_fork_copy(mapping: &cpu::LinuxActiveImageMapping) -> bool {
    !mapping.dontfork
        && !mapping.shared
        && (mapping.writable || !memory::frame_lease_is_runtime(mapping.frame))
}

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
#[allow(dead_code)]
const LIVE_PROCESS_CHILD_PROGRAM: [u8; 16] = [
    0xcd, 0x80, // validate the complete fork-child context
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
    Stopped { signal: u8 },
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
    fn is_valid(self, address_space: &ProcessAddressSpace) -> bool {
        self.validation_error(address_space).is_none()
    }

    fn validation_error(self, address_space: &ProcessAddressSpace) -> Option<u8> {
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
                        .is_some_and(|end| self.rsp >= start && self.rsp <= end)
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
        address_space: &ProcessAddressSpace,
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
    storage_slot: u8,
    hierarchy_count: u16,
    mapping_count: u16,
}

#[derive(Clone, Copy)]
struct ProcessAddressSpaceStorage {
    hierarchy: [memory::FrameLease; PROCESS_HIERARCHY_LIMIT],
    mappings: [cpu::LinuxActiveImageMapping; PROCESS_PRIVATE_PAGE_LIMIT],
}

impl ProcessAddressSpaceStorage {
    const EMPTY: Self = Self {
        hierarchy: [memory::FrameLease::EMPTY; PROCESS_HIERARCHY_LIMIT],
        mappings: [cpu::LinuxActiveImageMapping::EMPTY; PROCESS_PRIVATE_PAGE_LIMIT],
    };
}

impl ProcessAddressSpace {
    fn new(
        storage_slot: usize,
        hierarchy: &[memory::FrameLease],
        mappings: &[cpu::LinuxActiveImageMapping],
    ) -> Option<Self> {
        if storage_slot >= PROCESS_RUNTIME_LIMIT
            || !(4..=PROCESS_HIERARCHY_LIMIT).contains(&hierarchy.len())
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
                    || !(memory::frames_owned_by(memory::FrameOwner::KERNEL, &[mapping.frame])
                        || memory::frame_lease_is_runtime(mapping.frame))
                    || hierarchy.contains(&mapping.frame)
                    || mappings[..index].iter().any(|prior| {
                        prior.address == mapping.address
                            || prior.frame == mapping.frame && !(mapping.shared && prior.shared)
                    })
            })
            || !address_space::validate_linux_process_address_space(hierarchy, mappings)
        {
            return None;
        }
        let storage = unsafe {
            &mut *process_runtime()
                .address_space_storage
                .get()
                .cast::<ProcessAddressSpaceStorage>()
                .add(current_process_instance() * PROCESS_RUNTIME_LIMIT + storage_slot)
        };
        // The published counts bound every reader. Overwrite only the live
        // prefixes instead of clearing the full elastic capacity on every
        // registry refresh.
        storage.hierarchy[..hierarchy.len()].copy_from_slice(hierarchy);
        storage.mappings[..mappings.len()].copy_from_slice(mappings);
        Some(Self {
            storage_slot: storage_slot as u8,
            hierarchy_count: u16::try_from(hierarchy.len()).ok()?,
            mapping_count: u16::try_from(mappings.len()).ok()?,
        })
    }

    fn from_validated_snapshot(
        storage_slot: usize,
        hierarchy: &[memory::FrameLease],
        mappings: &[cpu::LinuxActiveImageMapping],
    ) -> Option<Self> {
        if storage_slot >= PROCESS_RUNTIME_LIMIT
            || !(4..=PROCESS_HIERARCHY_LIMIT).contains(&hierarchy.len())
            || mappings.is_empty()
            || mappings.len() > PROCESS_PRIVATE_PAGE_LIMIT
        {
            return None;
        }
        let storage = unsafe {
            &mut *process_runtime()
                .address_space_storage
                .get()
                .cast::<ProcessAddressSpaceStorage>()
                .add(current_process_instance() * PROCESS_RUNTIME_LIMIT + storage_slot)
        };
        // Stale tail entries are unreachable because access is count-bounded.
        storage.hierarchy[..hierarchy.len()].copy_from_slice(hierarchy);
        storage.mappings[..mappings.len()].copy_from_slice(mappings);
        Some(Self {
            storage_slot: storage_slot as u8,
            hierarchy_count: u16::try_from(hierarchy.len()).ok()?,
            mapping_count: u16::try_from(mappings.len()).ok()?,
        })
    }

    fn storage(&self) -> &'static ProcessAddressSpaceStorage {
        unsafe {
            &*process_runtime()
                .address_space_storage
                .get()
                .cast::<ProcessAddressSpaceStorage>()
                .add(
                    current_process_instance() * PROCESS_RUNTIME_LIMIT
                        + usize::from(self.storage_slot),
                )
        }
    }

    fn hierarchy(&self) -> &[memory::FrameLease] {
        &self.storage().hierarchy[..usize::from(self.hierarchy_count)]
    }

    fn root(&self) -> memory::FrameLease {
        self.storage().hierarchy[0]
    }

    fn mappings(&self) -> &[cpu::LinuxActiveImageMapping] {
        &self.storage().mappings[..usize::from(self.mapping_count)]
    }

    fn is_valid(&self) -> bool {
        self.is_owned()
            && address_space::validate_linux_process_address_space(
                self.hierarchy(),
                self.mappings(),
            )
    }

    fn is_owned(&self) -> bool {
        memory::frames_owned_by(memory::FrameOwner::KERNEL, self.hierarchy())
            && self.mappings().iter().all(|mapping| {
                memory::frame_lease_is_runtime(mapping.frame)
                    || memory::frames_owned_by(memory::FrameOwner::KERNEL, &[mapping.frame])
            })
    }

    fn is_live(&self) -> bool {
        memory::any_frame_lease_live(self.hierarchy())
            || self.mappings().iter().any(|mapping| {
                !mapping.shared
                    && !memory::frame_lease_is_runtime(mapping.frame)
                    && memory::any_frame_lease_live(&[mapping.frame])
            })
    }

    fn overlaps(&self, other: &Self) -> bool {
        self.hierarchy()
            .iter()
            .chain(
                self.mappings()
                    .iter()
                    .filter(|mapping| {
                        !mapping.shared && !memory::frame_lease_is_runtime(mapping.frame)
                    })
                    .map(|mapping| &mapping.frame),
            )
            .any(|left| {
                other
                    .hierarchy()
                    .iter()
                    .chain(
                        other
                            .mappings()
                            .iter()
                            .filter(|mapping| {
                                !mapping.shared && !memory::frame_lease_is_runtime(mapping.frame)
                            })
                            .map(|mapping| &mapping.frame),
                    )
                    .any(|right| left.address() == right.address())
            })
    }

    fn scrub_and_release(&self) -> bool {
        if !memory::frame_leases_releasable(memory::FrameOwner::KERNEL, self.hierarchy())
            || self.mappings().iter().any(|mapping| {
                !mapping.shared
                    && !memory::frame_lease_is_runtime(mapping.frame)
                    && !memory::frame_leases_releasable(
                        memory::FrameOwner::KERNEL,
                        &[mapping.frame],
                    )
            })
        {
            return false;
        }
        let mut released =
            memory::scrub_and_release_frames(memory::FrameOwner::KERNEL, self.hierarchy());
        for mapping in self.mappings() {
            if !mapping.shared && !memory::frame_lease_is_runtime(mapping.frame) {
                released &=
                    memory::scrub_and_release_frames(memory::FrameOwner::KERNEL, &[mapping.frame]);
            }
        }
        released
    }
}

#[derive(Clone, Copy)]
struct Process {
    handle: ProcessHandle,
    parent: Option<ProcessHandle>,
    process_group: u32,
    session: u32,
    executed: bool,
    user_namespace: linux_user::UserNamespaceHandle,
    runtime_slot: Option<u8>,
    address_space: Option<ProcessAddressSpace>,
    retiring_address_space: Option<ProcessAddressSpace>,
    context: Option<ProcessExecutionContext>,
    scheduler_thread: Option<scheduler::ProcessThreadHandle>,
    state: ProcessState,
    wait_event: Option<ProcessWaitEvent>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ProcessWaitEvent {
    Stopped { signal: u8 },
    Continued,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct WaitResult {
    pid: u32,
    status: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ChildWaitState {
    Running,
    Stopped { pid: u32, signal: u8 },
    Continued { pid: u32 },
    Exited { pid: u32, status: u8 },
}

#[derive(Clone, Copy)]
pub(crate) struct LiveProcessDispatch {
    pub(crate) pid: u32,
    pub(crate) root: memory::FrameLease,
    pub(crate) general: [u64; 15],
    pub(crate) rip: u64,
    pub(crate) rsp: u64,
    pub(crate) rflags: u64,
    pub(crate) fs_base: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum LiveProcessState {
    Running,
    Stopped,
    Zombie,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct LiveProcessStatus {
    pub(crate) pid: u32,
    pub(crate) parent_pid: u32,
    pub(crate) state: LiveProcessState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProcessGroupError {
    Access,
    NoSuchProcess,
    Permission,
}

struct ProcessTable {
    owner_domain: u64,
    processes: [Option<Process>; PROCESS_LIMIT],
    generations: [u32; PROCESS_LIMIT],
    next_pid: u32,
    init: Option<ProcessHandle>,
}

impl ProcessTable {
    const fn new() -> Self {
        Self {
            owner_domain: 0,
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
            || !context.is_valid(&address_space)
        {
            return None;
        }
        let user_namespace = linux_user::initial()?;
        let handle = self.allocate(
            None,
            user_namespace,
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

    fn scheduler_domain(&self) -> u64 {
        if self.owner_domain == 0 {
            1
        } else {
            self.owner_domain
        }
    }

    fn frame_referenced_by_other_process(
        &self,
        excluded: ProcessHandle,
        frame: memory::FrameLease,
    ) -> bool {
        self.processes.iter().flatten().any(|process| {
            process.handle != excluded
                && process.address_space.is_some_and(|space| {
                    space
                        .mappings()
                        .iter()
                        .any(|mapping| mapping.frame == frame)
                })
        })
    }

    fn promote_exclusive_cow_reference(
        &self,
        excluded: ProcessHandle,
        frame: memory::FrameLease,
    ) -> bool {
        let mut reference = None;
        for process in self.processes.iter().flatten() {
            if process.handle == excluded {
                continue;
            }
            let Some(space) = process.address_space else {
                continue;
            };
            for mapping in space
                .mappings()
                .iter()
                .filter(|mapping| mapping.frame == frame)
            {
                if reference.is_some() {
                    return true;
                }
                reference = Some((space.root(), *mapping));
            }
        }
        let Some((root, mapping)) = reference else {
            return true;
        };
        if !mapping.writable || mapping.shared {
            return true;
        }
        match address_space::linux_cow_page_in_root(root, mapping.address as u64) {
            Some(candidate) if candidate == frame => address_space::promote_linux_cow_page_in_root(
                root,
                mapping.address as u64,
                frame,
                mapping.executable,
            ),
            None => true,
            Some(_) => false,
        }
    }

    fn free_runtime_slot(&self) -> Option<usize> {
        (0..PROCESS_RUNTIME_LIMIT).find(|slot| {
            self.processes
                .iter()
                .flatten()
                .all(|process| process.runtime_slot != u8::try_from(*slot).ok())
        })
    }

    fn prepare_fork(&mut self, parent: ProcessHandle) -> Option<ProcessHandle> {
        let parent_process = self.process(parent)?;
        if parent_process.state != ProcessState::Running
            || parent_process.address_space.is_none()
            || parent_process.context.is_none()
        {
            return None;
        }
        let user_namespace = parent_process.user_namespace;
        linux_user::credentials(user_namespace)?;
        self.allocate(
            Some(parent),
            user_namespace,
            None,
            None,
            ProcessState::Preparing,
        )
    }

    fn refresh_process_inner(
        &mut self,
        handle: ProcessHandle,
        address_space: ProcessAddressSpace,
        context: ProcessExecutionContext,
        validate_address_space: bool,
    ) -> bool {
        if validate_address_space && !address_space.is_valid() || !context.is_valid(&address_space)
        {
            return false;
        }
        let Some(process) = self.process_mut(handle) else {
            return false;
        };
        if linux_user::credentials(process.user_namespace).is_none()
            || process.retiring_address_space.is_some()
            || process.state != ProcessState::Running
        {
            return false;
        }
        process.address_space = Some(address_space);
        process.runtime_slot = Some(address_space.storage_slot);
        process.context = Some(context);
        true
    }

    fn commit_fork(
        &mut self,
        handle: ProcessHandle,
        address_space: ProcessAddressSpace,
        context: ProcessExecutionContext,
    ) -> bool {
        self.commit_fork_inner(handle, address_space, context, true)
    }

    fn commit_fork_inner(
        &mut self,
        handle: ProcessHandle,
        address_space: ProcessAddressSpace,
        context: ProcessExecutionContext,
        validate_address_space: bool,
    ) -> bool {
        if !context.is_valid(&address_space)
            || validate_address_space
                && (!address_space.is_valid()
                    || self.processes.iter().flatten().any(|process| {
                        process
                            .address_space
                            .is_some_and(|existing| existing.overlaps(&address_space))
                            || process
                                .retiring_address_space
                                .is_some_and(|existing| existing.overlaps(&address_space))
                    }))
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
        process.runtime_slot = Some(address_space.storage_slot);
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
        let Some(scheduler_context) = context.scheduler_context(&address_space) else {
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
            crate::serial_write(
                "ginux: process withdraw missing scheduler thread pid/slot/generation: ",
            );
            crate::serial_write_u64(u64::from(handle.pid));
            crate::serial_write(" ");
            crate::serial_write_u64(u64::from(handle.slot));
            crate::serial_write(" ");
            crate::serial_write_u64(u64::from(handle.generation));
            crate::serial_write("\r\n");
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
        share_vm: bool,
    ) -> bool {
        process_runtime()
            .fork_execution_stage
            .store(1131, Ordering::Release);
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
        let Some(parent_space) = parent.address_space.as_ref() else {
            let _ = self.abort_fork(handle);
            return false;
        };
        let Some(parent_context) = parent.context else {
            let _ = self.abort_fork(handle);
            return false;
        };
        process_runtime()
            .fork_execution_stage
            .store(1132, Ordering::Release);
        if parent.state != ProcessState::Running
            || !parent_space.is_owned()
            || child_hierarchy.len() != parent_space.hierarchy().len()
            || (!child_pages.is_empty()
                && child_pages.len()
                    != parent_space
                        .mappings()
                        .iter()
                        .filter(|mapping| !mapping.dontfork && mapping_requires_fork_copy(mapping))
                        .count())
            || !self.resources_available(child_hierarchy, child_pages)
        {
            let _ = self.abort_fork(handle);
            return false;
        }

        let scratch = unsafe { &mut *process_runtime().fork_scratch.get() };
        process_runtime()
            .fork_execution_stage
            .store(1133, Ordering::Release);
        let child_mappings = &mut scratch.child_mappings;
        let parent_pages = &mut scratch.parent_pages;
        let mut private_index = 0;
        let mut child_mapping_count = 0;
        for parent in parent_space.mappings() {
            if parent.dontfork && !share_vm {
                continue;
            }
            let frame = if child_pages.is_empty() || !mapping_requires_fork_copy(parent) {
                parent.frame
            } else {
                let frame = *child_pages
                    .get(private_index)
                    .unwrap_or(&memory::FrameLease::EMPTY);
                private_index += 1;
                frame
            };
            child_mappings[child_mapping_count] = cpu::LinuxActiveImageMapping {
                address: parent.address,
                frame,
                writable: parent.writable,
                executable: parent.executable,
                shared: share_vm || parent.shared,
                dontfork: false,
            };
            child_mapping_count += 1;
        }
        let mut parent_page_count = 0;
        if !child_pages.is_empty() {
            for mapping in parent_space
                .mappings()
                .iter()
                .filter(|mapping| !mapping.dontfork && mapping_requires_fork_copy(mapping))
            {
                parent_pages[parent_page_count] = mapping.frame;
                parent_page_count += 1;
            }
        }
        process_runtime()
            .fork_execution_stage
            .store(1134, Ordering::Release);
        let published = child_pages.is_empty()
            || private_index == child_pages.len()
                && parent_page_count == child_pages.len()
                && copy_validated_private_pages(
                    parent_space.mappings(),
                    parent_space.root(),
                    &parent_pages[..parent_page_count],
                    child_pages,
                );
        process_runtime()
            .fork_execution_stage
            .store(1135, Ordering::Release);
        let published = published
            && address_space::clone_validated_linux_process_hierarchy(
                parent_space.mappings(),
                parent_space.hierarchy(),
                child_hierarchy,
                child_pages,
                share_vm,
            );
        process_runtime()
            .fork_execution_stage
            .store(1136, Ordering::Release);
        let child_space = if published {
            process_runtime()
                .fork_execution_stage
                .store(11361, Ordering::Release);
            let Some(storage_slot) = self.free_runtime_slot() else {
                let _ = scrub_candidate_resources(child_hierarchy, child_pages);
                let _ = self.abort_fork(handle);
                return false;
            };
            ProcessAddressSpace::from_validated_snapshot(
                storage_slot,
                child_hierarchy,
                &child_mappings[..child_mapping_count],
            )
        } else {
            None
        };
        process_runtime()
            .fork_execution_stage
            .store(1137, Ordering::Release);
        if child_space.is_none() {
            crate::serial_write("ginux: runtime fork clone state published/hierarchy/mappings: ");
            crate::serial_write_u64(u64::from(published));
            crate::serial_write(" ");
            crate::serial_write_u64(child_hierarchy.len() as u64);
            crate::serial_write(" ");
            crate::serial_write_u64(child_mapping_count as u64);
            crate::serial_write("\r\n");
        }
        if let Some(child_space) = child_space {
            process_runtime()
                .fork_execution_stage
                .store(1138, Ordering::Release);
            let attachments_inherited =
                crate::linux_sysv_ipc::inherit_process_attachments(parent_handle.pid, handle.pid)
                    .is_ok();
            if attachments_inherited
                && self.commit_fork_inner(handle, child_space, parent_context.fork_child(), false)
            {
                process_runtime()
                    .fork_execution_stage
                    .store(1139, Ordering::Release);
                return true;
            }
            if attachments_inherited {
                let _ = crate::linux_sysv_ipc::drop_process_attachments(handle.pid);
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
        let unique = |frames: &[memory::FrameLease]| {
            frames
                .iter()
                .enumerate()
                .all(|(index, frame)| !frames[index + 1..].contains(frame))
        };
        let unpublished = hierarchy.iter().chain(pages).all(|candidate| {
            self.processes.iter().flatten().all(|process| {
                process.address_space.into_iter().all(|space| {
                    !space.hierarchy().contains(candidate)
                        && !space.mappings().iter().any(|mapping| {
                            !mapping.shared
                                && !memory::frame_lease_is_runtime(mapping.frame)
                                && mapping.frame == *candidate
                        })
                })
            })
        });
        (4..=PROCESS_HIERARCHY_LIMIT).contains(&hierarchy.len())
            && pages.len() <= PROCESS_PRIVATE_PAGE_LIMIT
            && memory::frames_owned_by(memory::FrameOwner::KERNEL, hierarchy)
            && (pages.is_empty() || memory::frames_owned_by(memory::FrameOwner::KERNEL, pages))
            && unique(hierarchy)
            && unique(pages)
            && hierarchy.iter().all(|frame| !pages.contains(frame))
            && unpublished
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
        process.wait_event = None;
        process.state = ProcessState::Zombie { status };
        true
    }

    fn exit_after_current_address_space_retired(
        &mut self,
        handle: ProcessHandle,
        status: u8,
    ) -> bool {
        let Some(process) = self.process_mut(handle) else {
            return false;
        };
        if process.state != ProcessState::Running || process.scheduler_thread.is_some() {
            return false;
        }
        process.address_space = None;
        process.retiring_address_space = None;
        process.context = None;
        process.wait_event = None;
        process.state = ProcessState::Zombie { status };
        true
    }

    fn exit_after_retiring_address_space_released(
        &mut self,
        handle: ProcessHandle,
        expected_space: ProcessAddressSpace,
        status: u8,
    ) -> bool {
        let Some(process) = self.process(handle) else {
            return false;
        };
        if process.state != ProcessState::Exiting
            || process.address_space.is_some()
            || process.retiring_address_space != Some(expected_space)
            || process.context.is_some()
            || process.scheduler_thread.is_some()
            || expected_space
                .hierarchy()
                .iter()
                .any(|frame| memory::frames_owned_by(memory::FrameOwner::KERNEL, &[*frame]))
            || expected_space.mappings().iter().any(|mapping| {
                !mapping.shared
                    && !memory::frame_lease_is_runtime(mapping.frame)
                    && memory::frames_owned_by(memory::FrameOwner::KERNEL, &[mapping.frame])
                    && !self.frame_referenced_by_other_process(handle, mapping.frame)
            })
        {
            return false;
        }
        let Some(process) = self.process_mut(handle) else {
            return false;
        };
        process.retiring_address_space = None;
        process.wait_event = None;
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
        if self
            .processes
            .iter()
            .flatten()
            .all(|candidate| candidate.user_namespace != process.user_namespace)
        {
            let _ = linux_user::retire(process.user_namespace);
        }
        Some(WaitResult { pid, status })
    }

    fn child_wait_state(
        &self,
        parent: ProcessHandle,
        pid: Option<u32>,
        report_stopped: bool,
        report_continued: bool,
    ) -> Option<ChildWaitState> {
        self.child_wait_state_matching(
            parent,
            |process| pid.is_none_or(|pid| process.handle.pid == pid),
            report_stopped,
            report_continued,
        )
    }

    fn child_group_wait_state(
        &self,
        parent: ProcessHandle,
        process_group: u32,
        report_stopped: bool,
        report_continued: bool,
    ) -> Option<ChildWaitState> {
        self.child_wait_state_matching(
            parent,
            |process| process.process_group == process_group,
            report_stopped,
            report_continued,
        )
    }

    fn child_wait_state_matching(
        &self,
        parent: ProcessHandle,
        matches: impl Fn(&Process) -> bool,
        report_stopped: bool,
        report_continued: bool,
    ) -> Option<ChildWaitState> {
        if self.process(parent)?.state != ProcessState::Running {
            return None;
        }
        let mut children = self
            .processes
            .iter()
            .flatten()
            .filter(|process| process.parent == Some(parent) && matches(process));
        let child = children
            .clone()
            .find(|process| matches!(process.state, ProcessState::Zombie { .. }))
            .or_else(|| {
                children.clone().find(|process| {
                    report_stopped
                        && matches!(process.wait_event, Some(ProcessWaitEvent::Stopped { .. }))
                        || report_continued
                            && process.wait_event == Some(ProcessWaitEvent::Continued)
                })
            })
            .or_else(|| children.next())?;
        if report_stopped {
            if let Some(ProcessWaitEvent::Stopped { signal }) = child.wait_event {
                return Some(ChildWaitState::Stopped {
                    pid: child.handle.pid,
                    signal,
                });
            }
        }
        if report_continued && child.wait_event == Some(ProcessWaitEvent::Continued) {
            return Some(ChildWaitState::Continued {
                pid: child.handle.pid,
            });
        }
        match child.state {
            ProcessState::Running
            | ProcessState::Stopped { .. }
            | ProcessState::Preparing
            | ProcessState::Exiting => Some(ChildWaitState::Running),
            ProcessState::Zombie { status } => Some(ChildWaitState::Exited {
                pid: child.handle.pid,
                status,
            }),
        }
    }

    fn stop(&mut self, handle: ProcessHandle, signal: u8) -> bool {
        let Some(process) = self.process_mut(handle) else {
            return false;
        };
        if process.state != ProcessState::Running || signal == 0 {
            return false;
        }
        process.state = ProcessState::Stopped { signal };
        process.wait_event = Some(ProcessWaitEvent::Stopped { signal });
        true
    }

    fn continue_process(&mut self, handle: ProcessHandle) -> bool {
        let Some(process) = self.process_mut(handle) else {
            return false;
        };
        if !matches!(process.state, ProcessState::Stopped { .. }) {
            return false;
        }
        process.state = ProcessState::Running;
        process.wait_event = Some(ProcessWaitEvent::Continued);
        true
    }

    fn consume_wait_event(&mut self, parent: ProcessHandle, pid: u32) -> bool {
        let Some(handle) = self.handle_by_pid(pid) else {
            return false;
        };
        let Some(process) = self.process_mut(handle) else {
            return false;
        };
        if process.parent != Some(parent) || process.wait_event.is_none() {
            return false;
        }
        process.wait_event = None;
        true
    }

    fn reap_init_zombies(&mut self) -> bool {
        let Some(init) = self.init else {
            return false;
        };
        loop {
            let Some(pid) = self
                .processes
                .iter()
                .flatten()
                .find(|process| {
                    process.parent == Some(init)
                        && matches!(process.state, ProcessState::Zombie { .. })
                })
                .map(|process| process.handle.pid)
            else {
                return true;
            };
            if self.wait(init, pid).is_none() {
                return false;
            }
        }
    }

    fn reap_all_zombies(&mut self) -> bool {
        for process in &mut self.processes {
            let Some(candidate) = process.as_ref() else {
                continue;
            };
            if matches!(candidate.state, ProcessState::Zombie { .. }) {
                if candidate.address_space.is_some()
                    || candidate.retiring_address_space.is_some()
                    || candidate.scheduler_thread.is_some()
                {
                    return false;
                }
                *process = None;
            }
        }
        true
    }

    fn handle_by_pid(&self, pid: u32) -> Option<ProcessHandle> {
        self.processes
            .iter()
            .flatten()
            .find(|process| process.handle.pid == pid)
            .map(|process| process.handle)
    }

    fn set_return_value(&mut self, handle: ProcessHandle, value: u64) -> bool {
        let Some(process) = self.process_mut(handle) else {
            return false;
        };
        let Some(mut context) = process.context else {
            return false;
        };
        if process.state != ProcessState::Running {
            return false;
        }
        context.general[PROCESS_RAX_INDEX] = value;
        process.context = Some(context);
        true
    }

    fn set_stack_pointer(&mut self, handle: ProcessHandle, stack: u64) -> bool {
        let Some(process) = self.process_mut(handle) else {
            return false;
        };
        let Some(mut context) = process.context else {
            return false;
        };
        if process.state != ProcessState::Running || stack == 0 {
            return false;
        }
        context.rsp = stack;
        process.context = Some(context);
        true
    }

    fn dispatch(&self, handle: ProcessHandle) -> Option<LiveProcessDispatch> {
        let process = self.process(handle)?;
        let address_space = process.address_space.as_ref()?;
        let context = process.context?;
        if process.state != ProcessState::Running || !context.is_valid(address_space) {
            return None;
        }
        Some(LiveProcessDispatch {
            pid: handle.pid,
            root: address_space.root(),
            general: context.general,
            rip: context.rip,
            rsp: context.rsp,
            rflags: context.rflags,
            fs_base: context.fs_base,
        })
    }

    fn allocate(
        &mut self,
        parent: Option<ProcessHandle>,
        user_namespace: linux_user::UserNamespaceHandle,
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
        let (process_group, session) = parent
            .and_then(|parent| self.process(parent))
            .map_or((pid, pid), |parent| (parent.process_group, parent.session));
        self.generations[slot] = generation;
        self.processes[slot] = Some(Process {
            handle,
            parent,
            process_group,
            session,
            executed: false,
            user_namespace,
            runtime_slot: address_space.map(|space| space.storage_slot),
            address_space,
            retiring_address_space: None,
            context,
            scheduler_thread: None,
            state,
            wait_event: None,
        });
        Some(handle)
    }

    fn process_group(&self, pid: u32) -> Option<(u32, u32)> {
        let process = self.process(self.handle_by_pid(pid)?)?;
        matches!(
            process.state,
            ProcessState::Running | ProcessState::Stopped { .. } | ProcessState::Zombie { .. }
        )
        .then_some((process.process_group, process.session))
    }

    fn set_process_group(
        &mut self,
        caller_pid: u32,
        target_pid: u32,
        process_group: u32,
    ) -> Result<(), ProcessGroupError> {
        let caller = self
            .handle_by_pid(caller_pid)
            .ok_or(ProcessGroupError::NoSuchProcess)?;
        let target = self
            .handle_by_pid(target_pid)
            .ok_or(ProcessGroupError::NoSuchProcess)?;
        let caller_process = self
            .process(caller)
            .filter(|process| process.state == ProcessState::Running)
            .ok_or(ProcessGroupError::NoSuchProcess)?;
        let target_process = self
            .process(target)
            .filter(|process| process.state == ProcessState::Running)
            .ok_or(ProcessGroupError::NoSuchProcess)?;
        if target != caller && target_process.parent != Some(caller) {
            return Err(ProcessGroupError::NoSuchProcess);
        }
        if target != caller && target_process.executed {
            return Err(ProcessGroupError::Access);
        }
        if target_process.session != caller_process.session
            || target_process.session == target_process.handle.pid
        {
            return Err(ProcessGroupError::Permission);
        }
        let requested = if process_group == 0 {
            target_process.handle.pid
        } else {
            process_group
        };
        if requested != target_process.handle.pid
            && !self.processes.iter().flatten().any(|process| {
                process.process_group == requested && process.session == caller_process.session
            })
        {
            return Err(ProcessGroupError::Permission);
        }
        self.process_mut(target)
            .ok_or(ProcessGroupError::NoSuchProcess)?
            .process_group = requested;
        Ok(())
    }

    fn create_session(&mut self, pid: u32) -> Result<u32, ProcessGroupError> {
        let handle = self
            .handle_by_pid(pid)
            .ok_or(ProcessGroupError::NoSuchProcess)?;
        if self
            .processes
            .iter()
            .flatten()
            .any(|process| process.process_group == pid)
        {
            return Err(ProcessGroupError::Permission);
        }
        let process = self
            .process_mut(handle)
            .filter(|process| process.state == ProcessState::Running)
            .ok_or(ProcessGroupError::NoSuchProcess)?;
        process.process_group = pid;
        process.session = pid;
        Ok(pid)
    }

    fn mark_executed(&mut self, pid: u32) -> bool {
        let Some(handle) = self.handle_by_pid(pid) else {
            return false;
        };
        let Some(process) = self
            .process_mut(handle)
            .filter(|process| process.state == ProcessState::Running)
        else {
            return false;
        };
        process.executed = true;
        true
    }

    fn set_exec_context(&mut self, pid: u32, rip: u64, rsp: u64) -> bool {
        let Some(handle) = self.handle_by_pid(pid) else {
            return false;
        };
        let Some(process) = self
            .process_mut(handle)
            .filter(|process| process.state == ProcessState::Running)
        else {
            return false;
        };
        let context = ProcessExecutionContext {
            general: [0; 15],
            rip,
            rsp,
            rflags: (1 << 1) | (1 << 9),
            fs_base: 0,
        };
        if process
            .address_space
            .as_ref()
            .is_none_or(|space| !context.is_valid(space))
        {
            return false;
        }
        process.context = Some(context);
        process.executed = true;
        true
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
        shared: false,
        dontfork: false,
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
    if !write_test_hierarchy(&frames[0..4], &init_mappings) {
        crate::serial_write("ginux: lifecycle stage hierarchy failed\r\n");
        return false;
    }
    if !address_space::set_linux_mapping_wipe_on_fork(
        frames[0],
        TEST_USER_ADDRESS + PAGE_SIZE,
        true,
    ) {
        crate::serial_write("ginux: lifecycle stage wipe policy failed\r\n");
        return false;
    }
    if address_space::verify_linux_process_root_activation(frames[0])
        || !address_space::inherit_linux_process_kernel_mappings(frames[0])
    {
        crate::serial_write("ginux: lifecycle stage kernel inheritance failed\r\n");
        return false;
    }
    let Some(init_space) = ProcessAddressSpace::new(0, &frames[0..4], &init_mappings) else {
        return false;
    };
    let init_context = test_context();
    let table = process_runtime().test_table.get().cast::<ProcessTable>();
    unsafe { table.write(ProcessTable::new()) };
    let table = unsafe { &mut *table };
    let mut invalid_context = init_context;
    invalid_context.rip = (TEST_USER_ADDRESS + PAGE_SIZE) as u64;
    if table.create_init(init_space, invalid_context).is_some() {
        return false;
    }
    invalid_context = init_context;
    invalid_context.rsp = TEST_USER_ADDRESS as u64;
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
    let Some(init_scheduler_context) = init_context.scheduler_context(&init_space) else {
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
    let Some(shared_page_space) =
        ProcessAddressSpace::new(1, &frames[6..10], &shared_page_mappings)
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
    if ProcessAddressSpace::new(4, &frames[6..10], &wrong_permission)
        .is_some_and(|space| space.is_valid())
        || ProcessAddressSpace::new(5, &frames[6..10], &wrong_address)
            .is_some_and(|space| space.is_valid())
    {
        return false;
    }
    let (Some(first_space), Some(second_space)) = (
        ProcessAddressSpace::new(1, &frames[6..10], &first_mappings),
        ProcessAddressSpace::new(2, &frames[12..16], &second_mappings),
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
    let stale_space = ProcessAddressSpace::new(3, &stale_hierarchy, &first_mappings);
    let stale_expected = ProcessAddressSpace {
        storage_slot: 3,
        hierarchy_count: first_space.hierarchy_count,
        mapping_count: first_space.mapping_count,
    };
    if init.pid != 1
        || rejected_child.pid != 2
        || table.process_group(init.pid) != Some((1, 1))
        || table.create_session(init.pid) != Err(ProcessGroupError::Permission)
        || table.create_init(first_space, init_context).is_some()
        || table.commit_fork(rejected_child, init_space, init_context.fork_child())
        || stale_space.is_some_and(|space| space.is_valid())
        || address_space::verify_linux_process_root_activation(stale_root)
        || table.wait(init, rejected_child.pid).is_some()
        || table.exit(rejected_child, 1)
        || table.commit_cloned_fork(rejected_child, &frames[0..4], &frames[4..6], false)
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
        (parent_stack as *mut u8).write(0x44);
    }
    if first_child.pid != 3 {
        crate::serial_write("ginux: lifecycle stage child identity failed\r\n");
        return false;
    }
    if !table.commit_cloned_fork(first_child, &frames[6..10], &frames[10..12], false) {
        crate::serial_write("ginux: lifecycle stage cloned fork failed\r\n");
        return false;
    }
    if unsafe { core::slice::from_raw_parts(child_page as *const u8, PROCESS_CHILD_PROGRAM.len()) }
        != PROCESS_CHILD_PROGRAM
        || unsafe { (child_page as *const u8).add(PAGE_SIZE - 1).read() } != 0x32
        || table.commit_fork(first_child, second_space, init_context.fork_child())
        || table.wait(init, first_child.pid).is_some()
        || table.exit(first_child, 42)
        || table.begin_exit(first_child, second_space).is_some()
        || table.begin_exit(first_child, stale_expected).is_some()
    {
        crate::serial_write("ginux: lifecycle stage cloned fork state failed\r\n");
        return false;
    }
    if unsafe { (child_stack as *const u8).read() } != 0 {
        crate::serial_write("ginux: lifecycle stage wipe contents failed\r\n");
        return false;
    }
    if table.process_group(first_child.pid) != Some((1, 1))
        || table.child_wait_state(init, Some(first_child.pid), false, false)
            != Some(ChildWaitState::Running)
        || table.set_process_group(init.pid, first_child.pid, first_child.pid) != Ok(())
        || table.process_group(first_child.pid) != Some((3, 1))
        || table.child_group_wait_state(init, first_child.pid, false, false)
            != Some(ChildWaitState::Running)
        || table
            .child_group_wait_state(init, init.pid, false, false)
            .is_some()
        || table.set_process_group(init.pid, first_child.pid, init.pid) != Ok(())
        || !table.mark_executed(first_child.pid)
        || table.set_process_group(init.pid, first_child.pid, first_child.pid)
            != Err(ProcessGroupError::Access)
        || table.create_session(first_child.pid) != Ok(first_child.pid)
        || table.process_group(first_child.pid) != Some((3, 3))
        || table.set_process_group(init.pid, first_child.pid, init.pid)
            != Err(ProcessGroupError::Access)
        || table.set_process_group(first_child.pid, init.pid, init.pid)
            != Err(ProcessGroupError::NoSuchProcess)
    {
        return false;
    }
    if !table.stop(first_child, 19)
        || table.child_wait_state(init, Some(first_child.pid), false, false)
            != Some(ChildWaitState::Running)
        || table.child_wait_state(init, Some(first_child.pid), true, false)
            != Some(ChildWaitState::Stopped {
                pid: first_child.pid,
                signal: 19,
            })
        || !table.consume_wait_event(init, first_child.pid)
        || !table.continue_process(first_child)
        || table.child_wait_state(init, Some(first_child.pid), false, true)
            != Some(ChildWaitState::Continued {
                pid: first_child.pid,
            })
        || !table.consume_wait_event(init, first_child.pid)
        || table.child_wait_state(init, Some(first_child.pid), true, true)
            != Some(ChildWaitState::Running)
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
    let Some(child_scheduler_context) = child_context.scheduler_context(&first_space) else {
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
        || unsafe { (parent_stack as *const u8).read() } != 0x44
    {
        crate::serial_write("ginux: lifecycle stage wipe execution failed\r\n");
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
    let first_root = first_resources.hierarchy()[0];
    if table.exit(first_child, child_exit_status)
        || !memory::scrub_and_release_frames(memory::FrameOwner::KERNEL, &[first_root])
        || table.exit(first_child, child_exit_status)
        || !memory::scrub_and_release_frames(
            memory::FrameOwner::KERNEL,
            &first_resources.hierarchy()[1..],
        )
        || first_resources.mappings().iter().any(|mapping| {
            !mapping.shared
                && !memory::frame_lease_is_runtime(mapping.frame)
                && !memory::scrub_and_release_frames(memory::FrameOwner::KERNEL, &[mapping.frame])
        })
        || !table.exit(first_child, child_exit_status)
        || table.child_wait_state(init, Some(first_child.pid), false, false)
            != Some(ChildWaitState::Exited {
                pid: first_child.pid,
                status: child_exit_status,
            })
        || table.child_group_wait_state(init, first_child.pid, false, false)
            != Some(ChildWaitState::Exited {
                pid: first_child.pid,
                status: child_exit_status,
            })
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
    let instance = current_process_instance();
    if process_runtime().table_initialized[instance].load(Ordering::Acquire)
        || process_runtime().active_instance.load(Ordering::Acquire) != PROCESS_INSTANCE_NONE
    {
        return false;
    }
    let Some(address_space) =
        ProcessAddressSpace::new(0, snapshot.hierarchy(), snapshot.mappings())
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
    if invalid.is_valid(&address_space) || !context.is_valid(&address_space) {
        return false;
    }
    let table = unsafe {
        process_runtime()
            .tables
            .get()
            .cast::<ProcessTable>()
            .add(instance)
    };
    unsafe {
        core::ptr::addr_of_mut!((*table).owner_domain).write(
            cpu::current_container_slot()
                .and_then(cpu::container_domain_id)
                .unwrap_or(0),
        );
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
    process_runtime().refresh_count[instance].store(0, Ordering::Release);
    process_runtime().table_initialized[instance].store(true, Ordering::Release);
    process_runtime()
        .active_instance
        .store(instance as u8, Ordering::Release);
    true
}

pub(crate) fn refresh_live_process_registry(
    pid: u32,
    snapshot: &cpu::LinuxProcessAddressSpaceSnapshot,
    general: [u64; 15],
    rip: u64,
    rsp: u64,
    rflags: u64,
    fs_base: u64,
) -> bool {
    refresh_live_process_registry_inner(pid, snapshot, general, rip, rsp, rflags, fs_base, true)
}

pub(crate) fn refresh_validated_runtime_process_registry(
    pid: u32,
    snapshot: &cpu::LinuxProcessAddressSpaceSnapshot,
    general: [u64; 15],
    rip: u64,
    rsp: u64,
    rflags: u64,
    fs_base: u64,
) -> bool {
    refresh_live_process_registry_inner(pid, snapshot, general, rip, rsp, rflags, fs_base, false)
}

#[allow(clippy::too_many_arguments)]
fn refresh_live_process_registry_inner(
    pid: u32,
    snapshot: &cpu::LinuxProcessAddressSpaceSnapshot,
    general: [u64; 15],
    rip: u64,
    rsp: u64,
    rflags: u64,
    fs_base: u64,
    validate_address_space: bool,
) -> bool {
    let Some(table) = live_process_table_mut() else {
        crate::serial_write("ginux: live registry refresh has no table\r\n");
        return false;
    };
    let Some(handle) = table.handle_by_pid(pid) else {
        crate::serial_write("ginux: live registry refresh has no process\r\n");
        return false;
    };
    let Some(storage_slot) = table
        .process(handle)
        .and_then(|process| process.address_space)
        .map(|space| usize::from(space.storage_slot))
    else {
        crate::serial_write("ginux: live registry refresh has no address space\r\n");
        return false;
    };
    let address_space = if validate_address_space {
        ProcessAddressSpace::new(storage_slot, snapshot.hierarchy(), snapshot.mappings())
    } else {
        ProcessAddressSpace::from_validated_snapshot(
            storage_slot,
            snapshot.hierarchy(),
            snapshot.mappings(),
        )
    };
    let Some(address_space) = address_space else {
        crate::serial_write("ginux: live registry refresh rejected address space\r\n");
        return false;
    };
    let refreshed = table.refresh_process_inner(
        handle,
        address_space,
        ProcessExecutionContext {
            general,
            rip,
            rsp,
            rflags,
            fs_base,
        },
        validate_address_space,
    );
    if refreshed {
        let instance = active_process_instance().unwrap_or(HOST_PROCESS_INSTANCE);
        process_runtime().refresh_count[instance].fetch_add(1, Ordering::SeqCst);
    } else {
        crate::serial_write("ginux: live registry refresh rejected process context\r\n");
    }
    refreshed
}

pub(crate) fn finish_live_process_registry(refresh_floor: u32) -> bool {
    finish_live_process_registry_with_refresh_floor(refresh_floor)
}

pub(crate) fn finish_runtime_process_registry() -> bool {
    finish_live_process_registry_with_refresh_floor(0)
}

pub(crate) fn force_stop_runtime_process_registry() -> bool {
    let Some(instance) = active_process_instance() else {
        return true;
    };
    if !process_runtime().table_initialized[instance].load(Ordering::Acquire) {
        return false;
    }
    let Some(table) = live_process_table_mut() else {
        return false;
    };
    let Some(init) = table.init else {
        return false;
    };
    let Some(run_queue) = scheduler::active_process_scheduler_workspace() else {
        return false;
    };
    run_queue.retire_all();
    for process in table.processes.iter_mut().flatten() {
        process.scheduler_thread = None;
    }
    if !scheduler::release_process_scheduler_workspace() {
        return false;
    }

    for slot in 0..PROCESS_LIMIT {
        let Some(process) = table.processes[slot] else {
            continue;
        };
        if process.handle == init {
            continue;
        }
        if let Some(space) = process.address_space {
            if !release_live_process_address_space(table, process.handle, space) {
                return false;
            }
        }
        if let Some(space) = process.retiring_address_space {
            if !release_live_process_address_space(table, process.handle, space) {
                return false;
            }
        }
        if crate::linux_sysv_ipc::drop_process_attachments(process.handle.pid).is_err() {
            return false;
        }
        table.processes[slot] = None;
        if table
            .processes
            .iter()
            .flatten()
            .all(|candidate| candidate.user_namespace != process.user_namespace)
        {
            let _ = linux_user::retire(process.user_namespace);
        }
    }
    let Some(process) = table.process_mut(init) else {
        return false;
    };
    process.parent = None;
    process.retiring_address_space = None;
    process.scheduler_thread = None;
    process.state = ProcessState::Running;
    process.wait_event = None;
    true
}

pub(crate) fn suspend_runtime_process_registry() -> bool {
    let Some(instance) = active_process_instance() else {
        return false;
    };
    if instance != current_process_instance()
        || !process_runtime().table_initialized[instance].load(Ordering::Acquire)
    {
        return false;
    }
    process_runtime()
        .active_instance
        .store(PROCESS_INSTANCE_NONE, Ordering::Release);
    true
}

pub(crate) fn resume_runtime_process_registry() -> bool {
    let instance = current_process_instance();
    if !process_runtime().table_initialized[instance].load(Ordering::Acquire)
        || process_runtime()
            .active_instance
            .compare_exchange(
                PROCESS_INSTANCE_NONE,
                instance as u8,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_err()
    {
        return false;
    }
    let valid = live_process_table_mut().is_some_and(|table| {
        table.owner_domain
            == cpu::current_container_slot()
                .and_then(cpu::container_domain_id)
                .unwrap_or(0)
            && table.init.is_some()
    });
    if !valid {
        process_runtime()
            .active_instance
            .store(PROCESS_INSTANCE_NONE, Ordering::Release);
    }
    valid
}

pub(crate) fn reset_runtime_scheduler_bindings() -> bool {
    if let Some(run_queue) = scheduler::active_process_scheduler_workspace() {
        run_queue.retire_all();
        if !scheduler::release_process_scheduler_workspace() {
            return false;
        }
    }
    let Some(table) = live_process_table_mut() else {
        return false;
    };
    for process in table.processes.iter_mut().flatten() {
        process.scheduler_thread = None;
    }
    true
}

fn finish_live_process_registry_with_refresh_floor(refresh_floor: u32) -> bool {
    let Some(instance) = active_process_instance() else {
        return true;
    };
    if !process_runtime().table_initialized[instance].load(Ordering::Acquire) {
        return false;
    }
    let Some(table) = live_process_table_mut() else {
        return false;
    };
    let valid = table.reap_init_zombies()
        && table.reap_all_zombies()
        && table.init.is_some()
        && process_runtime().refresh_count[instance].load(Ordering::Acquire) >= refresh_floor
        && table.processes.iter().flatten().count() == 1
        && table.processes.iter().flatten().all(|process| {
            process.parent.is_none()
                && linux_user::credentials(process.user_namespace).is_some()
                && process.state == ProcessState::Running
                && process.retiring_address_space.is_none()
                && process.scheduler_thread.is_none()
        });
    if !valid {
        crate::serial_write("ginux: live process registry finish state: ");
        crate::serial_write_u64(u64::from(table.init.is_some()));
        crate::serial_write(" ");
        crate::serial_write_u64(u64::from(
            process_runtime().refresh_count[instance].load(Ordering::Acquire),
        ));
        crate::serial_write(" ");
        crate::serial_write_u64(table.processes.iter().flatten().count() as u64);
        crate::serial_write("\r\n");
        return false;
    }
    process_runtime().table_initialized[instance].store(false, Ordering::Release);
    process_runtime().refresh_count[instance].store(0, Ordering::Release);
    process_runtime()
        .active_instance
        .store(PROCESS_INSTANCE_NONE, Ordering::Release);
    true
}

pub(crate) fn live_process_user_namespace(pid: u32) -> Option<linux_user::UserNamespaceHandle> {
    let table = live_process_table_mut()?;
    let handle = table.handle_by_pid(pid)?;
    let namespace = table.process(handle)?.user_namespace;
    linux_user::credentials(namespace).map(|_| namespace)
}

pub(crate) fn set_live_process_user_namespace(
    pid: u32,
    namespace: linux_user::UserNamespaceHandle,
) -> bool {
    if linux_user::credentials(namespace).is_none() {
        return false;
    }
    let Some(table) = live_process_table_mut() else {
        return false;
    };
    let Some(handle) = table.handle_by_pid(pid) else {
        return false;
    };
    let Some(process) = table.process_mut(handle) else {
        return false;
    };
    if process.state != ProcessState::Running {
        return false;
    }
    process.user_namespace = namespace;
    true
}

pub(crate) fn live_process_status(pid: u32) -> Option<LiveProcessStatus> {
    let table = live_process_table_mut()?;
    let handle = table.handle_by_pid(pid)?;
    let process = table.process(handle)?;
    let state = match process.state {
        ProcessState::Running => LiveProcessState::Running,
        ProcessState::Stopped { .. } => LiveProcessState::Stopped,
        ProcessState::Zombie { .. } => LiveProcessState::Zombie,
        ProcessState::Preparing | ProcessState::Exiting => return None,
    };
    Some(LiveProcessStatus {
        pid: process.handle.pid,
        parent_pid: process.parent.map_or(0, |parent| parent.pid),
        state,
    })
}

pub(crate) fn set_live_process_scheduler_nice(pid: u32, nice: u8) -> bool {
    if nice > 39 {
        return false;
    }
    let Some(table) = live_process_table_mut() else {
        return false;
    };
    let Some(handle) = table.handle_by_pid(pid) else {
        return false;
    };
    let Some(thread) = table
        .process(handle)
        .and_then(|process| process.scheduler_thread)
    else {
        return true;
    };
    let Some(run_queue) = scheduler::active_process_scheduler_workspace() else {
        return false;
    };
    run_queue.set_priority(thread, 39_i8 - nice as i8)
}

pub(crate) fn live_process_parent_pid(pid: u32) -> Option<u32> {
    let table = live_process_table_mut()?;
    let handle = table.handle_by_pid(pid)?;
    let process = table.process(handle)?;
    Some(
        process
            .parent
            .and_then(|parent| table.process(parent).map(|parent| parent.handle.pid))
            .unwrap_or(0),
    )
}

pub(crate) fn live_running_child_pid(parent_pid: u32) -> Option<u32> {
    let table = live_process_table_mut()?;
    let parent = table.handle_by_pid(parent_pid)?;
    table
        .processes
        .iter()
        .flatten()
        .find(|process| process.parent == Some(parent) && process.state == ProcessState::Running)
        .map(|process| process.handle.pid)
}

pub(crate) fn live_process_group(pid: u32) -> Option<u32> {
    live_process_table_mut()?
        .process_group(pid)
        .map(|group| group.0)
}

pub(crate) fn live_process_session(pid: u32) -> Option<u32> {
    live_process_table_mut()?
        .process_group(pid)
        .map(|group| group.1)
}

pub(crate) fn set_live_process_group(
    caller_pid: u32,
    target_pid: u32,
    process_group: u32,
) -> Result<(), ProcessGroupError> {
    live_process_table_mut()
        .ok_or(ProcessGroupError::NoSuchProcess)?
        .set_process_group(caller_pid, target_pid, process_group)
}

pub(crate) fn create_live_process_session(pid: u32) -> Result<u32, ProcessGroupError> {
    live_process_table_mut()
        .ok_or(ProcessGroupError::NoSuchProcess)?
        .create_session(pid)
}

pub(crate) fn mark_live_process_executed(pid: u32) -> bool {
    live_process_table_mut().is_some_and(|table| table.mark_executed(pid))
}

pub(crate) fn set_live_process_exec_context(pid: u32, rip: u64, rsp: u64) -> bool {
    live_process_table_mut().is_some_and(|table| table.set_exec_context(pid, rip, rsp))
}

pub(crate) fn live_process_slot(pid: u32) -> Option<usize> {
    let table = live_process_table_mut()?;
    let handle = table.handle_by_pid(pid)?;
    Some(usize::from(handle.slot))
}

pub(crate) fn live_process_runtime_slot(pid: u32) -> Option<usize> {
    let table = live_process_table_mut()?;
    let handle = table.handle_by_pid(pid)?;
    table.process(handle)?.runtime_slot.map(usize::from)
}

pub(crate) fn release_live_process_runtime_slot(pid: u32) -> bool {
    let Some(table) = live_process_table_mut() else {
        return false;
    };
    let Some(handle) = table.handle_by_pid(pid) else {
        return false;
    };
    let Some(process) = table.process_mut(handle) else {
        return false;
    };
    if !matches!(process.state, ProcessState::Zombie { .. }) || process.runtime_slot.is_none() {
        return false;
    }
    process.runtime_slot = None;
    true
}

pub(crate) fn live_process_generation(pid: u32) -> Option<u32> {
    let table = live_process_table_mut()?;
    Some(table.handle_by_pid(pid)?.generation)
}

pub(crate) fn live_process_pid_at(slot: usize) -> Option<u32> {
    let table = live_process_table_mut()?;
    table
        .processes
        .get(slot)?
        .as_ref()
        .map(|process| process.handle.pid)
}

pub(crate) fn live_process_count() -> Option<u16> {
    let table = live_process_table_mut()?;
    table
        .processes
        .iter()
        .filter(|process| process.is_some())
        .count()
        .try_into()
        .ok()
}

pub(crate) fn host_live_process_count() -> Option<u16> {
    let mut count = 0_usize;
    for (instance, initialized) in process_runtime().table_initialized.iter().enumerate() {
        if initialized.load(Ordering::Acquire) {
            // Linux process-table mutation and host inspection are serialized
            // on the current single-processor syscall path.
            let table = unsafe {
                &*process_runtime()
                    .tables
                    .get()
                    .cast::<ProcessTable>()
                    .add(instance)
            };
            count = count.checked_add(
                table
                    .processes
                    .iter()
                    .filter(|process| process.is_some())
                    .count(),
            )?;
        }
    }
    count.try_into().ok()
}

pub(crate) fn live_process_mapping(
    pid: u32,
    address: usize,
) -> Option<cpu::LinuxActiveImageMapping> {
    let table = live_process_table_mut()?;
    let handle = table.handle_by_pid(pid)?;
    table
        .process(handle)?
        .address_space?
        .mappings()
        .iter()
        .copied()
        .find(|mapping| mapping.address == address)
}

pub(crate) fn live_mapping_referenced_by_other_process(
    pid: u32,
    frame: memory::FrameLease,
) -> bool {
    let Some(table) = live_process_table_mut() else {
        return true;
    };
    let Some(excluded) = table.handle_by_pid(pid) else {
        return true;
    };
    table.frame_referenced_by_other_process(excluded, frame)
}

pub(crate) fn prepare_live_other_process_frame_references(pid: u32) -> bool {
    let Some(table) = live_process_table_mut() else {
        return false;
    };
    let Some(excluded) = table.handle_by_pid(pid) else {
        return false;
    };
    build_live_frame_reference_set(table, excluded)
}

pub(crate) fn prepared_live_frame_referenced_by_other_process(frame: memory::FrameLease) -> bool {
    live_frame_reference_set_contains(frame)
}

pub(crate) fn replace_live_process_mapping(
    pid: u32,
    address: usize,
    expected: memory::FrameLease,
    replacement: memory::FrameLease,
    writable: bool,
    executable: bool,
) -> bool {
    replace_live_process_mapping_inner(
        pid,
        address,
        expected,
        replacement,
        writable,
        executable,
        false,
    )
}

pub(crate) fn replace_live_process_mapping_for_exec(
    pid: u32,
    address: usize,
    expected: memory::FrameLease,
    replacement: memory::FrameLease,
    writable: bool,
    executable: bool,
) -> bool {
    replace_live_process_mapping_inner(
        pid,
        address,
        expected,
        replacement,
        writable,
        executable,
        true,
    )
}

fn replace_live_process_mapping_inner(
    pid: u32,
    address: usize,
    expected: memory::FrameLease,
    replacement: memory::FrameLease,
    writable: bool,
    executable: bool,
    private_exec_mapping: bool,
) -> bool {
    if writable && executable
        || !memory::frames_owned_by(memory::FrameOwner::KERNEL, &[replacement])
    {
        return false;
    }
    let Some(table) = live_process_table_mut() else {
        return false;
    };
    let Some(handle) = table.handle_by_pid(pid) else {
        return false;
    };
    let Some(process) = table.process_mut(handle) else {
        return false;
    };
    let Some(space) = process.address_space.as_mut() else {
        return false;
    };
    let storage = unsafe {
        &mut *process_runtime()
            .address_space_storage
            .get()
            .cast::<ProcessAddressSpaceStorage>()
            .add(
                current_process_instance() * PROCESS_RUNTIME_LIMIT
                    + usize::from(space.storage_slot),
            )
    };
    let Some(mapping) = storage.mappings[..usize::from(space.mapping_count)]
        .iter_mut()
        .find(|mapping| mapping.address == address && mapping.frame == expected)
    else {
        return false;
    };
    mapping.frame = replacement;
    mapping.writable = writable;
    mapping.executable = executable;
    if private_exec_mapping {
        mapping.shared = false;
        mapping.dontfork = false;
    }
    true
}

pub(crate) fn add_live_process_mapping(pid: u32, mapping: cpu::LinuxActiveImageMapping) -> bool {
    if mapping.address == 0
        || !mapping.address.is_multiple_of(PAGE_SIZE)
        || mapping.writable && mapping.executable
        || !(memory::frames_owned_by(memory::FrameOwner::KERNEL, &[mapping.frame])
            || memory::frame_lease_is_runtime(mapping.frame))
    {
        return false;
    }
    let Some(table) = live_process_table_mut() else {
        return false;
    };
    let Some(handle) = table.handle_by_pid(pid) else {
        return false;
    };
    let Some(process) = table.process_mut(handle) else {
        return false;
    };
    let Some(space) = process.address_space.as_mut() else {
        return false;
    };
    let storage = unsafe {
        &mut *process_runtime()
            .address_space_storage
            .get()
            .cast::<ProcessAddressSpaceStorage>()
            .add(
                current_process_instance() * PROCESS_RUNTIME_LIMIT
                    + usize::from(space.storage_slot),
            )
    };
    let count = usize::from(space.mapping_count);
    if count >= storage.mappings.len()
        || storage.mappings[..count]
            .iter()
            .any(|existing| existing.address == mapping.address)
    {
        return false;
    }
    storage.mappings[count] = mapping;
    space.mapping_count += 1;
    true
}

pub(crate) fn live_process_hierarchy_frame(pid: u32, index: usize) -> Option<memory::FrameLease> {
    let table = live_process_table_mut()?;
    let handle = table.handle_by_pid(pid)?;
    table
        .process(handle)?
        .address_space?
        .hierarchy()
        .get(index)
        .copied()
}

pub(crate) fn write_live_process_u32(pid: u32, address: u64, value: u32) -> bool {
    let Ok(address) = usize::try_from(address) else {
        return false;
    };
    let page = address & !(PAGE_SIZE - 1);
    let offset = address & (PAGE_SIZE - 1);
    if offset + size_of::<u32>() > PAGE_SIZE {
        return false;
    }
    let Some(mapping) = live_process_mapping(pid, page).filter(|mapping| mapping.writable) else {
        return false;
    };
    let Some(frame) = mapping.frame.direct_map_address() else {
        return false;
    };
    unsafe {
        (frame as *mut u8)
            .add(offset)
            .cast::<u32>()
            .write_unaligned(value)
    };
    true
}

pub(crate) fn copy_from_live_process(pid: u32, address: u64, output: &mut [u8]) -> bool {
    copy_live_process_bytes(pid, address, output.as_mut_ptr(), output.len(), false)
}

pub(crate) fn copy_to_live_process(pid: u32, address: u64, input: &[u8]) -> bool {
    copy_live_process_bytes(pid, address, input.as_ptr().cast_mut(), input.len(), true)
}

pub(crate) fn live_process_range_mapped(
    pid: u32,
    address: u64,
    length: usize,
    writable: bool,
) -> bool {
    let Ok(address) = usize::try_from(address) else {
        return false;
    };
    let Some(end) = address.checked_add(length) else {
        return false;
    };
    if length == 0 {
        return true;
    }
    let mut page = address & !(PAGE_SIZE - 1);
    while page < end {
        if live_process_mapping(pid, page).is_none_or(|mapping| writable && !mapping.writable) {
            return false;
        }
        let Some(next) = page.checked_add(PAGE_SIZE) else {
            return false;
        };
        page = next;
    }
    true
}

fn copy_live_process_bytes(
    pid: u32,
    address: u64,
    buffer: *mut u8,
    buffer_length: usize,
    write: bool,
) -> bool {
    let Ok(mut address) = usize::try_from(address) else {
        return false;
    };
    let mut copied = 0;
    while copied < buffer_length {
        let page = address & !(PAGE_SIZE - 1);
        let offset = address & (PAGE_SIZE - 1);
        let length = (buffer_length - copied).min(PAGE_SIZE - offset);
        if write && !prepare_live_process_write(pid, page) {
            return false;
        }
        let Some(mapping) =
            live_process_mapping(pid, page).filter(|mapping| !write || mapping.writable)
        else {
            return false;
        };
        let Some(frame) = mapping.frame.direct_map_address() else {
            return false;
        };
        // SAFETY: the generation-bound process mapping owns a live page. The
        // bounds above keep each copy within that page, and syscall execution
        // serializes live-process table access on the current CPU.
        unsafe {
            let process = (frame as *mut u8).add(offset);
            if write {
                core::ptr::copy_nonoverlapping(buffer.add(copied), process, length);
            } else {
                core::ptr::copy_nonoverlapping(process, buffer.add(copied), length);
            }
        }
        copied += length;
        address += length;
    }
    true
}

pub(crate) fn prepare_live_process_write(pid: u32, address: usize) -> bool {
    let Some(mapping) =
        live_process_mapping(pid, address).filter(|mapping| mapping.writable && !mapping.shared)
    else {
        return live_process_mapping(pid, address).is_some_and(|mapping| mapping.writable);
    };
    let Some(root) = live_process_root(pid) else {
        return false;
    };
    if address_space::linux_cow_page_in_root(root, address as u64) != Some(mapping.frame) {
        return true;
    }
    let Some([replacement]) = memory::allocate_frames::<1>(memory::FrameOwner::KERNEL) else {
        return false;
    };
    if !address_space::copy_linux_frame(mapping.frame, replacement)
        || !replace_live_process_mapping(
            pid,
            address,
            mapping.frame,
            replacement,
            true,
            mapping.executable,
        )
        || !address_space::replace_linux_cow_page_in_root(
            root,
            address as u64,
            mapping.frame,
            replacement,
            mapping.executable,
        )
    {
        let _ = replace_live_process_mapping(
            pid,
            address,
            replacement,
            mapping.frame,
            true,
            mapping.executable,
        );
        let _ = memory::scrub_and_release_frames(memory::FrameOwner::KERNEL, &[replacement]);
        return false;
    }
    if !live_mapping_referenced_by_other_process(pid, mapping.frame)
        && !memory::frame_lease_is_runtime(mapping.frame)
        && !memory::scrub_and_release_frames(memory::FrameOwner::KERNEL, &[mapping.frame])
    {
        return false;
    }
    true
}

fn live_process_root(pid: u32) -> Option<memory::FrameLease> {
    let table = live_process_table_mut()?;
    let handle = table.handle_by_pid(pid)?;
    Some(table.process(handle)?.address_space?.root())
}

pub(crate) fn dispatch_live_init_with_return(value: u64) -> Option<LiveProcessDispatch> {
    let table = live_process_table_mut()?;
    let init = table.init?;
    if !table.set_return_value(init, value) {
        return None;
    }
    table.dispatch(init)
}

pub(crate) fn dispatch_live_process(pid: u32) -> Option<LiveProcessDispatch> {
    let table = live_process_table_mut()?;
    let handle = table.handle_by_pid(pid)?;
    table.dispatch(handle)
}

pub(crate) fn yield_live_process(pid: u32) -> Option<LiveProcessDispatch> {
    let table = live_process_table_mut()?;
    let handle = table.handle_by_pid(pid)?;
    let process = table.process(handle)?;
    let address_space = process.address_space.as_ref()?;
    let context = process.context?;
    let thread = process.scheduler_thread?;
    let scheduler_context = context.scheduler_context(address_space)?;
    let run_queue = scheduler::active_process_scheduler_workspace()?;
    let selected = run_queue.yield_current(thread, scheduler_context)?;
    let selected_handle = table
        .processes
        .iter()
        .flatten()
        .find(|process| process.scheduler_thread == Some(selected.handle()))?
        .handle;
    (table
        .process(selected_handle)?
        .context?
        .scheduler_context(table.process(selected_handle)?.address_space.as_ref()?)?
        == selected.context())
    .then(|| table.dispatch(selected_handle))
    .flatten()
}

#[inline(never)]
pub(crate) fn begin_live_fork(
    parent_pid: u32,
    eager_private_copy: bool,
    share_vm: bool,
    child_stack: Option<u64>,
) -> Option<LiveProcessDispatch> {
    process_runtime()
        .fork_execution_stage
        .store(10, Ordering::Release);
    let Some(table) = live_process_table_mut() else {
        process_runtime()
            .fork_execution_stage
            .store(101, Ordering::Release);
        return None;
    };
    let Some(parent_handle) = table.handle_by_pid(parent_pid) else {
        process_runtime()
            .fork_execution_stage
            .store(102, Ordering::Release);
        return None;
    };
    let Some(parent) = table.process(parent_handle) else {
        process_runtime()
            .fork_execution_stage
            .store(103, Ordering::Release);
        return None;
    };
    let Some(parent_space) = parent.address_space.as_ref() else {
        process_runtime()
            .fork_execution_stage
            .store(104, Ordering::Release);
        return None;
    };
    if parent.state != ProcessState::Running || !parent_space.is_owned() {
        process_runtime().fork_execution_stage.store(
            if parent.state != ProcessState::Running {
                105
            } else {
                106
            },
            Ordering::Release,
        );
        return None;
    }
    let hierarchy_count = parent_space.hierarchy().len();
    let private_count = (eager_private_copy && !share_vm).then(|| {
        parent_space
            .mappings()
            .iter()
            .filter(|mapping| !mapping.dontfork && mapping_requires_fork_copy(mapping))
            .count()
    });
    process_runtime()
        .fork_execution_stage
        .store(11, Ordering::Release);
    let child_handle = table.prepare_fork(parent_handle)?;
    process_runtime()
        .fork_execution_stage
        .store(111, Ordering::Release);
    let child_hierarchy =
        unsafe { &mut *process_runtime().fork_hierarchy_allocation_scratch.get() };
    if !memory::allocate_frame_prefix_into(
        memory::FrameOwner::KERNEL,
        child_hierarchy,
        hierarchy_count,
    ) {
        let _ = table.abort_fork(child_handle);
        return None;
    }
    process_runtime()
        .fork_execution_stage
        .store(112, Ordering::Release);
    let child_pages = unsafe { &mut *process_runtime().fork_allocation_scratch.get() };
    let private_count = private_count.unwrap_or(0);
    if private_count != 0
        && !memory::allocate_frame_prefix_into(
            memory::FrameOwner::KERNEL,
            child_pages,
            private_count,
        )
    {
        let _ = memory::scrub_and_release_frames(
            memory::FrameOwner::KERNEL,
            &child_hierarchy[..hierarchy_count],
        );
        let _ = table.abort_fork(child_handle);
        return None;
    }
    process_runtime()
        .fork_execution_stage
        .store(113, Ordering::Release);
    let published = table.commit_cloned_fork(
        child_handle,
        &child_hierarchy[..hierarchy_count],
        &child_pages[..private_count],
        share_vm,
    );
    if !published {
        return None;
    }
    process_runtime()
        .fork_execution_stage
        .store(12, Ordering::Release);
    if !table.set_return_value(parent_handle, u64::from(child_handle.pid)) {
        process_runtime()
            .fork_execution_stage
            .store(121, Ordering::Release);
        let _ = discard_live_fork(table, parent_handle, child_handle);
        return None;
    }
    if child_stack.is_some_and(|stack| !table.set_stack_pointer(child_handle, stack)) {
        process_runtime()
            .fork_execution_stage
            .store(122, Ordering::Release);
        let _ = discard_live_fork(table, parent_handle, child_handle);
        return None;
    }
    let Some(parent_context) = table
        .process(parent_handle)
        .and_then(|process| process.context)
    else {
        process_runtime()
            .fork_execution_stage
            .store(123, Ordering::Release);
        let _ = discard_live_fork(table, parent_handle, child_handle);
        return None;
    };
    let Some(parent_space) = table
        .process(parent_handle)
        .and_then(|process| process.address_space.as_ref())
    else {
        process_runtime()
            .fork_execution_stage
            .store(124, Ordering::Release);
        let _ = discard_live_fork(table, parent_handle, child_handle);
        return None;
    };
    let Some(parent_scheduler_context) = parent_context.scheduler_context(parent_space) else {
        process_runtime()
            .fork_execution_stage
            .store(125, Ordering::Release);
        let _ = discard_live_fork(table, parent_handle, child_handle);
        return None;
    };
    let Some(child) = table.process(child_handle) else {
        let _ = discard_live_fork(table, parent_handle, child_handle);
        return None;
    };
    let (Some(child_space), Some(child_context)) = (child.address_space.as_ref(), child.context)
    else {
        process_runtime()
            .fork_execution_stage
            .store(126, Ordering::Release);
        let _ = discard_live_fork(table, parent_handle, child_handle);
        return None;
    };
    let Some(child_scheduler_context) = child_context.scheduler_context(child_space) else {
        process_runtime()
            .fork_execution_stage
            .store(127, Ordering::Release);
        crate::serial_write("ginux: fork child context validation error/rip/rsp/fs/mappings: ");
        crate::serial_write_u64(u64::from(
            child_context.validation_error(child_space).unwrap_or(0),
        ));
        crate::serial_write(" ");
        crate::serial_write_u64(child_context.rip);
        crate::serial_write(" ");
        crate::serial_write_u64(child_context.rsp);
        crate::serial_write(" ");
        crate::serial_write_u64(child_context.fs_base);
        crate::serial_write(" ");
        crate::serial_write_u64(child_space.mappings().len() as u64);
        crate::serial_write("\r\n");
        let _ = discard_live_fork(table, parent_handle, child_handle);
        return None;
    };
    process_runtime()
        .fork_execution_stage
        .store(13, Ordering::Release);
    let scheduler_domain = table.scheduler_domain();
    let nested = table
        .process(parent_handle)
        .is_some_and(|process| process.scheduler_thread.is_some());
    let run_queue = if nested {
        scheduler::active_process_scheduler_workspace()
    } else {
        scheduler::acquire_process_scheduler_workspace()
    }?;
    if !nested && !table.admit_to_scheduler(parent_handle, run_queue, scheduler_domain) {
        let _ = scheduler::release_process_scheduler_workspace();
        let _ = discard_live_fork(table, parent_handle, child_handle);
        return None;
    }
    if !table.admit_to_scheduler(child_handle, run_queue, scheduler_domain) {
        if !nested {
            let _ = table.withdraw_from_scheduler(parent_handle, run_queue);
            let _ = scheduler::release_process_scheduler_workspace();
        }
        let _ = discard_live_fork(table, parent_handle, child_handle);
        return None;
    }
    let scheduled = (|| {
        let parent_thread = table.process(parent_handle)?.scheduler_thread?;
        let child_thread = table.process(child_handle)?.scheduler_thread?;
        let selected = if nested {
            run_queue.switch_current(parent_thread, parent_scheduler_context, child_thread)?
        } else {
            let initial = run_queue.schedule_next()?;
            (initial.handle() == parent_thread && initial.context() == parent_scheduler_context)
                .then(|| {
                    run_queue.switch_current(parent_thread, parent_scheduler_context, child_thread)
                })
                .flatten()?
        };
        (selected.handle() == child_thread && selected.context() == child_scheduler_context)
            .then_some(())
    })()
    .is_some();
    if !scheduled {
        let _ = table.withdraw_from_scheduler(child_handle, run_queue);
        if !nested {
            let _ = table.withdraw_from_scheduler(parent_handle, run_queue);
            let _ = scheduler::release_process_scheduler_workspace();
        }
        let _ = discard_live_fork(table, parent_handle, child_handle);
        return None;
    }
    process_runtime()
        .fork_execution_stage
        .store(14, Ordering::Release);
    table.dispatch(child_handle)
}

fn discard_live_fork(
    table: &mut ProcessTable,
    parent_handle: ProcessHandle,
    child_handle: ProcessHandle,
) -> bool {
    let Some(child_space) = table
        .process(child_handle)
        .and_then(|process| process.address_space)
    else {
        return false;
    };
    let return_restored = table.set_return_value(parent_handle, 0);
    let Some(retiring) = table.begin_exit(child_handle, child_space) else {
        return false;
    };
    return_restored
        && release_live_process_address_space(table, child_handle, retiring)
        && crate::linux_sysv_ipc::drop_process_attachments(child_handle.pid).is_ok()
        && table.exit_after_retiring_address_space_released(child_handle, retiring, 0)
        && table.wait(parent_handle, child_handle.pid)
            == Some(WaitResult {
                pid: child_handle.pid,
                status: 0,
            })
}

fn live_frame_reference_set_slot(address: u64) -> usize {
    ((address >> 12).wrapping_mul(0x9e37_79b9_7f4a_7c15) as usize)
        & (LIVE_FRAME_REFERENCE_SET_CAPACITY - 1)
}

fn build_live_frame_reference_set(table: &ProcessTable, excluded: ProcessHandle) -> bool {
    let set = process_runtime().frame_reference_set.get().cast::<u64>();
    for index in 0..LIVE_FRAME_REFERENCE_SET_CAPACITY {
        unsafe { set.add(index).write(0) };
    }
    for process in &table.processes {
        let Some(process) = process
            .as_ref()
            .filter(|process| process.handle != excluded)
        else {
            continue;
        };
        let Some(address_space) = process.address_space else {
            continue;
        };
        for mapping in address_space.mappings() {
            let address = mapping.frame.address();
            let mut slot = live_frame_reference_set_slot(address);
            let mut inserted = false;
            for _ in 0..LIVE_FRAME_REFERENCE_SET_CAPACITY {
                let entry = unsafe { set.add(slot).read() };
                if entry == address {
                    inserted = true;
                    break;
                }
                if entry == 0 {
                    unsafe { set.add(slot).write(address) };
                    inserted = true;
                    break;
                }
                slot = (slot + 1) & (LIVE_FRAME_REFERENCE_SET_CAPACITY - 1);
            }
            if !inserted {
                return false;
            }
        }
    }
    true
}

fn live_frame_reference_set_contains(frame: memory::FrameLease) -> bool {
    let set = process_runtime().frame_reference_set.get().cast::<u64>();
    let address = frame.address();
    let mut slot = live_frame_reference_set_slot(address);
    for _ in 0..LIVE_FRAME_REFERENCE_SET_CAPACITY {
        let entry = unsafe { set.add(slot).read() };
        if entry == address {
            return true;
        }
        if entry == 0 {
            return false;
        }
        slot = (slot + 1) & (LIVE_FRAME_REFERENCE_SET_CAPACITY - 1);
    }
    false
}

fn release_live_process_address_space(
    table: &ProcessTable,
    excluded: ProcessHandle,
    space: ProcessAddressSpace,
) -> bool {
    const RELEASE_BATCH_SIZE: usize = 64;
    let page_table_started = crate::arch::x86_64::timer::timestamp_counter();
    if !address_space::retire_linux_child_address_space(space.root()) {
        return false;
    }
    if cfg!(feature = "linux-boot-profile") {
        crate::arch::x86_64::cpu::linux_boot_profile_record_lifecycle_phase(7, page_table_started);
    }
    let frame_release_started = crate::arch::x86_64::timer::timestamp_counter();
    let reference_set_ready = build_live_frame_reference_set(table, excluded);
    let mut release_batch = [memory::FrameLease::EMPTY; RELEASE_BATCH_SIZE];
    let mut release_count = 0;
    for mapping in space.mappings() {
        if !mapping.shared {
            let reference_started = crate::arch::x86_64::timer::timestamp_counter();
            let referenced = if reference_set_ready {
                live_frame_reference_set_contains(mapping.frame)
            } else {
                table.frame_referenced_by_other_process(excluded, mapping.frame)
            };
            if cfg!(feature = "linux-boot-profile") {
                crate::arch::x86_64::cpu::linux_boot_profile_record_lifecycle_phase(
                    9,
                    reference_started,
                );
            }
            if referenced {
                if crate::linux_file_cache::contains(mapping.frame) {
                    continue;
                }
                if !table.promote_exclusive_cow_reference(excluded, mapping.frame) {
                    return false;
                }
            } else if crate::linux_file_cache::contains(mapping.frame) {
                continue;
            } else {
                let ownership_started = crate::arch::x86_64::timer::timestamp_counter();
                let owned = memory::frames_owned_by(
                    memory::FrameOwner::KERNEL,
                    core::slice::from_ref(&mapping.frame),
                );
                if cfg!(feature = "linux-boot-profile") {
                    crate::arch::x86_64::cpu::linux_boot_profile_record_lifecycle_phase(
                        10,
                        ownership_started,
                    );
                }
                if owned && !release_batch[..release_count].contains(&mapping.frame) {
                    release_batch[release_count] = mapping.frame;
                    release_count += 1;
                    if release_count == release_batch.len() {
                        let scrub_started = crate::arch::x86_64::timer::timestamp_counter();
                        if !memory::scrub_and_release_frames(
                            memory::FrameOwner::KERNEL,
                            &release_batch,
                        ) {
                            return false;
                        }
                        if cfg!(feature = "linux-boot-profile") {
                            crate::arch::x86_64::cpu::linux_boot_profile_record_lifecycle_phase(
                                11,
                                scrub_started,
                            );
                        }
                        release_count = 0;
                    }
                }
            }
        }
    }
    for frame in space.hierarchy() {
        let ownership_started = crate::arch::x86_64::timer::timestamp_counter();
        let owned = memory::frames_owned_by(memory::FrameOwner::KERNEL, &[*frame]);
        if cfg!(feature = "linux-boot-profile") {
            crate::arch::x86_64::cpu::linux_boot_profile_record_lifecycle_phase(
                10,
                ownership_started,
            );
        }
        if owned && !release_batch[..release_count].contains(frame) {
            release_batch[release_count] = *frame;
            release_count += 1;
            if release_count == release_batch.len() {
                let scrub_started = crate::arch::x86_64::timer::timestamp_counter();
                if !memory::scrub_and_release_frames(memory::FrameOwner::KERNEL, &release_batch) {
                    return false;
                }
                if cfg!(feature = "linux-boot-profile") {
                    crate::arch::x86_64::cpu::linux_boot_profile_record_lifecycle_phase(
                        11,
                        scrub_started,
                    );
                }
                release_count = 0;
            }
        }
    }
    let scrub_started = crate::arch::x86_64::timer::timestamp_counter();
    let released = release_count == 0
        || memory::scrub_and_release_frames(
            memory::FrameOwner::KERNEL,
            &release_batch[..release_count],
        );
    if cfg!(feature = "linux-boot-profile") && release_count != 0 {
        crate::arch::x86_64::cpu::linux_boot_profile_record_lifecycle_phase(11, scrub_started);
    }
    if cfg!(feature = "linux-boot-profile") {
        crate::arch::x86_64::cpu::linux_boot_profile_record_lifecycle_phase(
            8,
            frame_release_started,
        );
    }
    released
}

#[inline(never)]
pub(crate) fn complete_live_child_exit(pid: u32, status: u8) -> Option<LiveProcessDispatch> {
    process_runtime()
        .fork_execution_stage
        .store(15, Ordering::Release);
    let table = live_process_table_mut()?;
    process_runtime()
        .fork_execution_stage
        .store(151, Ordering::Release);
    let child_handle = table.handle_by_pid(pid)?;
    let child = table.process(child_handle)?;
    let parent_handle = child.parent?;
    let parent = table.process(parent_handle)?;
    if child.parent != Some(parent_handle) || child.state != ProcessState::Running {
        return None;
    }
    let parent_space = parent.address_space.as_ref()?;
    let parent_context = parent.context?;
    let child_space = child.address_space?;
    let child_context = child.context?;
    process_runtime()
        .fork_execution_stage
        .store(152, Ordering::Release);
    let parent_scheduler_context = parent_context.scheduler_context(parent_space)?;
    let child_scheduler_context = child_context.scheduler_context(&child_space)?;
    let parent_thread = parent.scheduler_thread?;
    let child_thread = child.scheduler_thread?;
    let run_queue = scheduler::active_process_scheduler_workspace()?;
    process_runtime()
        .fork_execution_stage
        .store(153, Ordering::Release);
    let resumed = run_queue.switch_current(child_thread, child_scheduler_context, parent_thread)?;
    process_runtime()
        .fork_execution_stage
        .store(154, Ordering::Release);
    if resumed.handle() != parent_thread || resumed.context() != parent_scheduler_context {
        return None;
    }
    process_runtime()
        .fork_execution_stage
        .store(16, Ordering::Release);
    let nested = parent.parent.is_some();
    let standalone_transaction = table
        .processes
        .iter()
        .flatten()
        .filter(|process| process.scheduler_thread.is_some())
        .count()
        == 2;
    if !table.withdraw_from_scheduler(child_handle, run_queue) {
        return None;
    }
    if !nested
        && standalone_transaction
        && (!table.withdraw_from_scheduler(parent_handle, run_queue)
            || !scheduler::release_process_scheduler_workspace())
    {
        return None;
    }
    process_runtime()
        .fork_execution_stage
        .store(161, Ordering::Release);
    if !release_live_process_address_space(table, child_handle, child_space) {
        process_runtime()
            .fork_execution_stage
            .store(162, Ordering::Release);
        return None;
    }
    process_runtime()
        .fork_execution_stage
        .store(163, Ordering::Release);
    if crate::linux_sysv_ipc::drop_process_attachments(pid).is_err()
        || !table.exit_after_current_address_space_retired(child_handle, status)
    {
        process_runtime()
            .fork_execution_stage
            .store(164, Ordering::Release);
        return None;
    }
    process_runtime()
        .fork_execution_stage
        .store(17, Ordering::Release);
    table.dispatch(parent_handle)
}

pub(crate) fn resume_live_parent_after_child_exec(pid: u32) -> Option<LiveProcessDispatch> {
    let table = live_process_table_mut()?;
    let child_handle = table.handle_by_pid(pid)?;
    let child = table.process(child_handle)?;
    let parent_handle = child.parent?;
    let parent = table.process(parent_handle)?;
    let child_space = child.address_space.as_ref()?;
    let parent_space = parent.address_space.as_ref()?;
    let child_context = child.context?;
    let parent_context = parent.context?;
    let child_thread = child.scheduler_thread?;
    let parent_thread = parent.scheduler_thread?;
    let child_scheduler_context = child_context.scheduler_context(child_space)?;
    let parent_scheduler_context = parent_context.scheduler_context(parent_space)?;
    let run_queue = scheduler::active_process_scheduler_workspace()?;
    let resumed = run_queue.switch_current(child_thread, child_scheduler_context, parent_thread)?;
    if resumed.handle() != parent_thread || resumed.context() != parent_scheduler_context {
        return None;
    }
    table.dispatch(parent_handle)
}

pub(crate) fn switch_live_process(from_pid: u32, to_pid: u32) -> Option<LiveProcessDispatch> {
    let table = live_process_table_mut()?;
    let from_handle = table.handle_by_pid(from_pid)?;
    let to_handle = table.handle_by_pid(to_pid)?;
    let from = table.process(from_handle)?;
    let to = table.process(to_handle)?;
    let from_space = from.address_space.as_ref()?;
    let to_space = to.address_space.as_ref()?;
    let from_context = from.context?;
    let to_context = to.context?;
    let from_thread = from.scheduler_thread?;
    let to_thread = to.scheduler_thread?;
    let from_scheduler_context = from_context.scheduler_context(from_space)?;
    let to_scheduler_context = to_context.scheduler_context(to_space)?;
    let run_queue = scheduler::active_process_scheduler_workspace()?;
    let resumed = run_queue.switch_current(from_thread, from_scheduler_context, to_thread)?;
    if resumed.handle() != to_thread || resumed.context() != to_scheduler_context {
        return None;
    }
    table.dispatch(to_handle)
}

pub(crate) fn wait_live_child(parent_pid: u32, child_pid: u32) -> Option<(u32, u8)> {
    let table = live_process_table_mut()?;
    let parent = table.handle_by_pid(parent_pid)?;
    table
        .wait(parent, child_pid)
        .map(|result| (result.pid, result.status))
}

pub(crate) fn live_child_wait_state(
    parent_pid: u32,
    child_pid: Option<u32>,
    report_stopped: bool,
    report_continued: bool,
) -> Option<ChildWaitState> {
    let table = live_process_table_mut()?;
    let parent = table.handle_by_pid(parent_pid)?;
    table.child_wait_state(parent, child_pid, report_stopped, report_continued)
}

pub(crate) fn live_child_group_wait_state(
    parent_pid: u32,
    process_group: u32,
    report_stopped: bool,
    report_continued: bool,
) -> Option<ChildWaitState> {
    let table = live_process_table_mut()?;
    let parent = table.handle_by_pid(parent_pid)?;
    table.child_group_wait_state(parent, process_group, report_stopped, report_continued)
}

pub(crate) fn consume_live_child_wait_event(parent_pid: u32, child_pid: u32) -> bool {
    let Some(table) = live_process_table_mut() else {
        return false;
    };
    let Some(parent) = table.handle_by_pid(parent_pid) else {
        return false;
    };
    table.consume_wait_event(parent, child_pid)
}

#[inline(never)]
#[allow(dead_code)]
pub(crate) fn verify_live_fork_clone_transaction() -> bool {
    process_runtime()
        .fork_execution_stage
        .store(0, Ordering::Release);
    process_runtime()
        .fork_child_diagnostic
        .store(0, Ordering::Release);
    cpu::reset_process_child_execution_diagnostic();
    if process_runtime()
        .fork_child_executed
        .load(Ordering::Acquire)
    {
        verify_live_fork_clone_only_transaction()
    } else {
        verify_live_fork_clone_and_execution_transaction()
    }
}

#[inline(never)]
#[allow(dead_code)]
fn verify_live_fork_clone_and_execution_transaction() -> bool {
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
    let parent_user_namespace = parent.user_namespace;
    if parent.state != ProcessState::Running || !parent_space.is_valid() {
        return false;
    }
    let Some(child_handle) = table.prepare_fork(parent_handle) else {
        return false;
    };
    let hierarchy_count = parent_space.hierarchy().len();
    let mapping_count = parent_space.mappings().len();
    let private_count = parent_space
        .mappings()
        .iter()
        .filter(|mapping| mapping_requires_fork_copy(mapping))
        .count();
    let child_hierarchy =
        unsafe { &mut *process_runtime().fork_hierarchy_allocation_scratch.get() };
    if !memory::allocate_frame_prefix_into(
        memory::FrameOwner::KERNEL,
        child_hierarchy,
        hierarchy_count,
    ) {
        let _ = table.abort_fork(child_handle);
        return false;
    }
    let child_pages = unsafe { &mut *process_runtime().fork_allocation_scratch.get() };
    if !memory::allocate_frame_prefix_into(memory::FrameOwner::KERNEL, child_pages, private_count) {
        let _released = memory::scrub_and_release_frames(
            memory::FrameOwner::KERNEL,
            &child_hierarchy[..hierarchy_count],
        );
        let _aborted = table.abort_fork(child_handle);
        return false;
    }
    let published = table.commit_cloned_fork(
        child_handle,
        &child_hierarchy[..hierarchy_count],
        &child_pages[..private_count],
        false,
    );
    let hierarchy_extras_released = true;
    let page_extras_released = true;
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
        && child.user_namespace == parent_user_namespace
        && linux_user::credentials(child.user_namespace).is_some()
        && child.state == ProcessState::Running
        && child.context == Some(parent_context.fork_child())
        && child_space.is_valid()
        && !child_space.overlaps(&parent_space)
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
                    && child.shared == parent.shared
                    && (child.frame != parent.frame) == mapping_requires_fork_copy(parent)
                    && (!mapping_requires_fork_copy(parent)
                        || child
                            .frame
                            .direct_map_address()
                            .zip(parent.frame.direct_map_address())
                            .is_some_and(|(child_address, parent_address)| {
                                // SAFETY: Both leases remain owned by the process table for
                                // the duration of this comparison, and a direct-map lookup
                                // validates the complete physical page range.
                                unsafe {
                                    core::slice::from_raw_parts(
                                        child_address as *const u8,
                                        PAGE_SIZE,
                                    ) == core::slice::from_raw_parts(
                                        parent_address as *const u8,
                                        PAGE_SIZE,
                                    )
                                }
                            }))
            });
    let execute_child = child_valid
        && !process_runtime()
            .fork_child_executed
            .load(Ordering::Acquire);
    let exit_status = if execute_child {
        execute_cloned_live_child(table, parent_handle, child_handle)
    } else {
        child_valid.then_some(42)
    };
    let Some(retiring) = table.begin_exit(child_handle, child_space) else {
        return false;
    };
    let resources_released = retiring.scrub_and_release();
    let exited = resources_released && table.exit(child_handle, exit_status.unwrap_or(0));
    let waited = exited
        && table.wait(parent_handle, child_handle.pid)
            == Some(WaitResult {
                pid: child_handle.pid,
                status: 42,
            });
    let valid = child_valid
        && hierarchy_extras_released
        && page_extras_released
        && exit_status == Some(42)
        && waited
        && table.processes.iter().flatten().count() == 1
        && table.process(parent_handle).is_some_and(|process| {
            process.address_space == Some(parent_space)
                && process.context == Some(parent_context)
                && process.state == ProcessState::Running
                && process.scheduler_thread.is_none()
        })
        && memory::owner_live_frame_count(memory::FrameOwner::KERNEL) == Some(before);
    if valid && execute_child {
        process_runtime()
            .fork_child_executed
            .store(true, Ordering::Release);
    }
    valid
}

#[inline(never)]
#[allow(dead_code)]
fn verify_live_fork_clone_only_transaction() -> bool {
    macro_rules! reject {
        ($stage:expr) => {{
            process_runtime()
                .fork_execution_stage
                .store($stage, Ordering::Release);
            return false;
        }};
    }
    process_runtime()
        .fork_execution_stage
        .store(200, Ordering::Release);
    let Some(before) = memory::owner_live_frame_count(memory::FrameOwner::KERNEL) else {
        reject!(201);
    };
    let Some(table) = live_process_table_mut() else {
        reject!(202);
    };
    let Some(parent_handle) = table.init else {
        reject!(203);
    };
    let Some(parent) = table.process(parent_handle) else {
        reject!(204);
    };
    let (Some(parent_space), Some(parent_context)) = (parent.address_space, parent.context) else {
        reject!(205);
    };
    let parent_user_namespace = parent.user_namespace;
    if parent.state != ProcessState::Running || !parent_space.is_valid() {
        crate::serial_write("ginux: clone-only parent state/owned/validated: ");
        crate::serial_write_u64(u64::from(parent.state == ProcessState::Running));
        crate::serial_write(" ");
        crate::serial_write_u64(u64::from(parent_space.is_owned()));
        crate::serial_write(" ");
        crate::serial_write_u64(u64::from(
            address_space::validate_linux_process_address_space(
                parent_space.hierarchy(),
                parent_space.mappings(),
            ),
        ));
        crate::serial_write("\r\n");
        reject!(206);
    }
    let Some(child_handle) = table.prepare_fork(parent_handle) else {
        reject!(207);
    };
    let hierarchy_count = parent_space.hierarchy().len();
    let mapping_count = parent_space.mappings().len();
    let private_count = parent_space
        .mappings()
        .iter()
        .filter(|mapping| mapping_requires_fork_copy(mapping))
        .count();
    let child_hierarchy =
        unsafe { &mut *process_runtime().fork_hierarchy_allocation_scratch.get() };
    if !memory::allocate_frame_prefix_into(
        memory::FrameOwner::KERNEL,
        child_hierarchy,
        hierarchy_count,
    ) {
        let _ = table.abort_fork(child_handle);
        reject!(208);
    }
    let child_pages = unsafe { &mut *process_runtime().fork_allocation_scratch.get() };
    if !memory::allocate_frame_prefix_into(memory::FrameOwner::KERNEL, child_pages, private_count) {
        let _ = memory::scrub_and_release_frames(
            memory::FrameOwner::KERNEL,
            &child_hierarchy[..hierarchy_count],
        );
        let _ = table.abort_fork(child_handle);
        reject!(209);
    }
    let published = table.commit_cloned_fork(
        child_handle,
        &child_hierarchy[..hierarchy_count],
        &child_pages[..private_count],
        false,
    );
    let hierarchy_extras_released = true;
    let page_extras_released = true;
    if !published {
        reject!(210);
    }
    let Some(child) = table.process(child_handle) else {
        reject!(211);
    };
    let Some(child_space) = child.address_space else {
        reject!(212);
    };
    let child_valid = child.parent == Some(parent_handle)
        && child.user_namespace == parent_user_namespace
        && linux_user::credentials(child.user_namespace).is_some()
        && child.state == ProcessState::Running
        && child.context == Some(parent_context.fork_child())
        && child_space.is_valid()
        && !child_space.overlaps(&parent_space)
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
                    && child.shared == parent.shared
                    && (child.frame != parent.frame) == mapping_requires_fork_copy(parent)
                    && (!mapping_requires_fork_copy(parent)
                        || child
                            .frame
                            .direct_map_address()
                            .zip(parent.frame.direct_map_address())
                            .is_some_and(|(child_address, parent_address)| {
                                // SAFETY: Both live, generation-checked leases map
                                // complete pages for this serialized comparison.
                                unsafe {
                                    core::slice::from_raw_parts(
                                        child_address as *const u8,
                                        PAGE_SIZE,
                                    ) == core::slice::from_raw_parts(
                                        parent_address as *const u8,
                                        PAGE_SIZE,
                                    )
                                }
                            }))
            });
    let Some(retiring) = table.begin_exit(child_handle, child_space) else {
        reject!(213);
    };
    let resources_released = retiring.scrub_and_release();
    let exited = resources_released && table.exit(child_handle, 42);
    let waited = exited
        && table.wait(parent_handle, child_handle.pid)
            == Some(WaitResult {
                pid: child_handle.pid,
                status: 42,
            });
    let parent_restored = table.process(parent_handle).is_some_and(|process| {
        process.address_space == Some(parent_space)
            && process.context == Some(parent_context)
            && process.state == ProcessState::Running
            && process.scheduler_thread.is_none()
    });
    let frame_count_restored =
        memory::owner_live_frame_count(memory::FrameOwner::KERNEL) == Some(before);
    let valid = child_valid
        && hierarchy_extras_released
        && page_extras_released
        && waited
        && table.processes.iter().flatten().count() == 1
        && parent_restored
        && frame_count_restored;
    if !valid {
        crate::serial_write("ginux: clone-only verification state: ");
        for state in [
            child_valid,
            resources_released,
            exited,
            waited,
            table.processes.iter().flatten().count() == 1,
            parent_restored,
            frame_count_restored,
        ] {
            crate::serial_write_u64(u64::from(state));
            crate::serial_write(" ");
        }
        crate::serial_write_u64(before as u64);
        crate::serial_write(" ");
        crate::serial_write_u64(
            memory::owner_live_frame_count(memory::FrameOwner::KERNEL).unwrap_or(usize::MAX) as u64,
        );
        crate::serial_write("\r\n");
    }
    valid
}

#[inline(never)]
#[allow(dead_code)]
fn execute_cloned_live_child(
    table: &mut ProcessTable,
    parent_handle: ProcessHandle,
    child_handle: ProcessHandle,
) -> Option<u8> {
    process_runtime()
        .fork_execution_stage
        .store(1, Ordering::Release);
    let parent = table.process(parent_handle)?;
    let parent_space = parent.address_space.as_ref()?;
    let parent_context = parent.context?;
    let parent_scheduler_context = parent_context.scheduler_context(parent_space)?;
    let child = table.process(child_handle)?;
    let child_space = child.address_space.as_ref()?;
    let child_context = child.context?;
    let child_scheduler_context = child_context.scheduler_context(child_space)?;
    let child_root = child_space.root();
    let child_code = child_space.mappings().iter().find(|mapping| {
        mapping.executable
            && child_context.rip >= mapping.address as u64
            && child_context.rip < mapping.address as u64 + PAGE_SIZE as u64
    })?;
    let parent_code = parent_space
        .mappings()
        .iter()
        .find(|mapping| mapping.address == child_code.address)?;
    let child_code_frame = child_code.frame;
    let parent_code_frame = parent_code.frame;
    let offset = usize::try_from(child_context.rip - child_code.address as u64).ok()?;
    if offset.checked_add(LIVE_PROCESS_CHILD_PROGRAM.len())? > PAGE_SIZE {
        return None;
    }
    let child_address = child_code_frame.direct_map_address()?.checked_add(offset)?;
    let parent_address = parent_code_frame
        .direct_map_address()?
        .checked_add(offset)?;
    let mut original_parent = [0_u8; LIVE_PROCESS_CHILD_PROGRAM.len()];
    // SAFETY: Both generation-checked pages remain exclusively owned by their
    // process records. The validated offset covers the complete bounded probe.
    unsafe {
        core::ptr::copy_nonoverlapping(
            parent_address as *const u8,
            original_parent.as_mut_ptr(),
            original_parent.len(),
        );
        core::ptr::copy_nonoverlapping(
            LIVE_PROCESS_CHILD_PROGRAM.as_ptr(),
            child_address as *mut u8,
            LIVE_PROCESS_CHILD_PROGRAM.len(),
        );
    }
    process_runtime()
        .fork_execution_stage
        .store(2, Ordering::Release);
    let run_queue = scheduler::acquire_process_scheduler_workspace()?;
    let scheduler_domain = table.scheduler_domain();
    if !table.admit_to_scheduler(parent_handle, run_queue, scheduler_domain) {
        let _ = scheduler::release_process_scheduler_workspace();
        return None;
    }
    if !table.admit_to_scheduler(child_handle, run_queue, scheduler_domain) {
        let _ = table.withdraw_from_scheduler(parent_handle, run_queue);
        let _ = scheduler::release_process_scheduler_workspace();
        return None;
    }
    process_runtime()
        .fork_execution_stage
        .store(3, Ordering::Release);
    let parent_thread = table.process(parent_handle)?.scheduler_thread?;
    let child_thread = table.process(child_handle)?.scheduler_thread?;
    let initial = run_queue.schedule_next();
    let selected_child = initial
        .and_then(|selected| {
            (selected.handle() == parent_thread && selected.context() == parent_scheduler_context)
                .then_some(())
        })
        .and_then(|()| run_queue.yield_current(parent_thread, parent_scheduler_context));
    let selected = selected_child.is_some_and(|selected| {
        selected.handle() == child_thread && selected.context() == child_scheduler_context
    });
    if selected {
        process_runtime()
            .fork_execution_stage
            .store(4, Ordering::Release);
    }
    cpu::reset_process_child_execution_diagnostic();
    let exit_status = selected
        .then(|| {
            address_space::execute_linux_process_child(
                child_root,
                child_context.rip,
                child_context.rsp,
                child_context.fs_base,
                child_context.general,
            )
        })
        .flatten();
    if exit_status == Some(42) {
        process_runtime()
            .fork_execution_stage
            .store(5, Ordering::Release);
    }
    let resumed_parent =
        exit_status.and_then(|_| run_queue.yield_current(child_thread, child_scheduler_context));
    let resumed = resumed_parent.is_some_and(|selected| {
        selected.handle() == parent_thread && selected.context() == parent_scheduler_context
    });
    if resumed {
        process_runtime()
            .fork_execution_stage
            .store(6, Ordering::Release);
    }
    let child_withdrawn = table.withdraw_from_scheduler(child_handle, run_queue);
    let parent_withdrawn = table.withdraw_from_scheduler(parent_handle, run_queue);
    let scheduler_released = scheduler::release_process_scheduler_workspace();
    let parent_unchanged = parent_code_frame
        .direct_map_address()
        .and_then(|address| address.checked_add(offset))
        .is_some_and(|address| {
            // SAFETY: The parent lease remains live and the bounded range was
            // validated above; the child executed under a disjoint root.
            unsafe {
                core::slice::from_raw_parts(address as *const u8, original_parent.len())
                    == original_parent
            }
        });
    if child_withdrawn && parent_withdrawn && parent_unchanged {
        process_runtime()
            .fork_execution_stage
            .store(7, Ordering::Release);
    }
    let valid = selected
        && resumed
        && child_withdrawn
        && parent_withdrawn
        && scheduler_released
        && parent_unchanged
        && exit_status == Some(42);
    let diagnostic = u32::from(selected)
        | (u32::from(exit_status.is_some()) << 1)
        | (u32::from(exit_status == Some(42)) << 2)
        | (u32::from(resumed) << 3)
        | (u32::from(child_withdrawn) << 4)
        | (u32::from(parent_withdrawn) << 5)
        | (u32::from(parent_unchanged) << 6)
        | (u32::from(scheduler_released) << 7);
    process_runtime()
        .fork_child_diagnostic
        .store(diagnostic, Ordering::Release);
    valid.then_some(42)
}

pub(crate) fn live_fork_execution_stage() -> u32 {
    process_runtime()
        .fork_execution_stage
        .load(Ordering::Acquire)
}

pub(crate) fn live_fork_child_diagnostic() -> u32 {
    process_runtime()
        .fork_child_diagnostic
        .load(Ordering::Acquire)
}

fn live_process_table_mut() -> Option<&'static mut ProcessTable> {
    let instance = active_process_instance()?;
    if !process_runtime().table_initialized[instance].load(Ordering::Acquire) {
        return None;
    }
    let table = unsafe {
        process_runtime()
            .tables
            .get()
            .cast::<ProcessTable>()
            .add(instance)
    };
    Some(unsafe { &mut *table })
}

fn current_process_instance() -> usize {
    cpu::current_container_slot().unwrap_or(HOST_PROCESS_INSTANCE)
}

pub(crate) fn current_ipc_instance() -> usize {
    current_process_instance()
}

fn active_process_instance() -> Option<usize> {
    let instance = usize::from(process_runtime().active_instance.load(Ordering::Acquire));
    (instance < PROCESS_INSTANCE_LIMIT).then_some(instance)
}

fn scrub_candidate_resources(
    hierarchy: &[memory::FrameLease],
    pages: &[memory::FrameLease],
) -> bool {
    if !memory::frame_leases_releasable(memory::FrameOwner::KERNEL, hierarchy)
        || !memory::frame_leases_releasable(memory::FrameOwner::KERNEL, pages)
    {
        return false;
    }
    memory::scrub_and_release_frames(memory::FrameOwner::KERNEL, hierarchy)
        && memory::scrub_and_release_frames(memory::FrameOwner::KERNEL, pages)
}

pub(crate) fn copy_private_pages(
    source: &[memory::FrameLease],
    destination: &[memory::FrameLease],
) -> bool {
    if source.is_empty()
        || source.len() != destination.len()
        || source.len() > PROCESS_PRIVATE_PAGE_LIMIT
        || !source.iter().all(|frame| {
            memory::frames_owned_by(memory::FrameOwner::KERNEL, &[*frame])
                || memory::frame_lease_is_runtime(*frame)
        })
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

fn copy_validated_private_pages(
    mappings: &[cpu::LinuxActiveImageMapping],
    root: memory::FrameLease,
    source: &[memory::FrameLease],
    destination: &[memory::FrameLease],
) -> bool {
    if source.is_empty()
        || source.len() != destination.len()
        || source.len() > PROCESS_PRIVATE_PAGE_LIMIT
        || mappings
            .iter()
            .filter(|mapping| mapping_requires_fork_copy(mapping))
            .count()
            != source.len()
        || !memory::frames_owned_by(memory::FrameOwner::KERNEL, destination)
    {
        return false;
    }
    for ((mapping, source_frame), destination_frame) in mappings
        .iter()
        .filter(|mapping| mapping_requires_fork_copy(mapping))
        .zip(source)
        .zip(destination)
    {
        let (Some(source_address), Some(destination_address)) = (
            source_frame.direct_map_address(),
            destination_frame.direct_map_address(),
        ) else {
            return false;
        };
        let Some(wipeonfork) = address_space::linux_mapping_wipe_on_fork(root, mapping.address)
        else {
            return false;
        };
        unsafe {
            if wipeonfork {
                core::ptr::write_bytes(destination_address as *mut u8, 0, PAGE_SIZE);
            } else {
                core::ptr::copy_nonoverlapping(
                    source_address as *const u8,
                    destination_address as *mut u8,
                    PAGE_SIZE,
                );
            }
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
