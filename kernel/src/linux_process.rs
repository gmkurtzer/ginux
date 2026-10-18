use core::{
    cell::UnsafeCell,
    mem::{size_of, MaybeUninit},
    sync::atomic::{AtomicBool, AtomicU32, AtomicU8, Ordering},
};

use crate::{address_space, arch::x86_64::cpu, control_abi, linux_user, memory, scheduler};

pub(crate) const PROCESS_LIMIT: usize = 6;
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
static LIVE_PROCESS_TABLE_INITIALIZED: [AtomicBool; PROCESS_INSTANCE_LIMIT] =
    [const { AtomicBool::new(false) }; PROCESS_INSTANCE_LIMIT];
static LIVE_PROCESS_REFRESH_COUNT: [AtomicU32; PROCESS_INSTANCE_LIMIT] =
    [const { AtomicU32::new(0) }; PROCESS_INSTANCE_LIMIT];
static LIVE_PROCESS_ACTIVE_INSTANCE: AtomicU8 = AtomicU8::new(PROCESS_INSTANCE_NONE);
#[allow(dead_code)]
static LIVE_FORK_CHILD_EXECUTED: AtomicBool = AtomicBool::new(false);
static LIVE_FORK_EXECUTION_STAGE: AtomicU32 = AtomicU32::new(0);
static LIVE_FORK_CHILD_DIAGNOSTIC: AtomicU32 = AtomicU32::new(0);
static mut LIVE_PROCESS_TABLES: [ProcessTable; PROCESS_INSTANCE_LIMIT] =
    [const { ProcessTable::new() }; PROCESS_INSTANCE_LIMIT];

struct LiveForkScratch {
    child_mappings: [cpu::LinuxActiveImageMapping; PROCESS_PRIVATE_PAGE_LIMIT],
    parent_pages: [memory::FrameLease; PROCESS_PRIVATE_PAGE_LIMIT],
}

struct LiveForkScratchCell(UnsafeCell<LiveForkScratch>);

unsafe impl Sync for LiveForkScratchCell {}

static LIVE_FORK_SCRATCH: LiveForkScratchCell =
    LiveForkScratchCell(UnsafeCell::new(LiveForkScratch {
        child_mappings: [cpu::LinuxActiveImageMapping::EMPTY; PROCESS_PRIVATE_PAGE_LIMIT],
        parent_pages: [memory::FrameLease::EMPTY; PROCESS_PRIVATE_PAGE_LIMIT],
    }));
struct LiveForkAllocationScratch(UnsafeCell<[memory::FrameLease; PROCESS_PRIVATE_COPY_LIMIT]>);

unsafe impl Sync for LiveForkAllocationScratch {}

static LIVE_FORK_ALLOCATION_SCRATCH: LiveForkAllocationScratch = LiveForkAllocationScratch(
    UnsafeCell::new([memory::FrameLease::EMPTY; PROCESS_PRIVATE_COPY_LIMIT]),
);
struct LiveForkHierarchyAllocationScratch(
    UnsafeCell<[memory::FrameLease; PROCESS_HIERARCHY_LIMIT]>,
);

unsafe impl Sync for LiveForkHierarchyAllocationScratch {}

static LIVE_FORK_HIERARCHY_ALLOCATION_SCRATCH: LiveForkHierarchyAllocationScratch =
    LiveForkHierarchyAllocationScratch(UnsafeCell::new(
        [memory::FrameLease::EMPTY; PROCESS_HIERARCHY_LIMIT],
    ));
static mut TEST_PROCESS_TABLE: MaybeUninit<ProcessTable> = MaybeUninit::uninit();

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

static mut PROCESS_ADDRESS_SPACE_STORAGE: [[ProcessAddressSpaceStorage; PROCESS_LIMIT];
    PROCESS_INSTANCE_LIMIT] =
    [[ProcessAddressSpaceStorage::EMPTY; PROCESS_LIMIT]; PROCESS_INSTANCE_LIMIT];

impl ProcessAddressSpace {
    fn new(
        storage_slot: usize,
        hierarchy: &[memory::FrameLease],
        mappings: &[cpu::LinuxActiveImageMapping],
    ) -> Option<Self> {
        if storage_slot >= PROCESS_LIMIT
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
            &mut *core::ptr::addr_of_mut!(PROCESS_ADDRESS_SPACE_STORAGE)
                .cast::<ProcessAddressSpaceStorage>()
                .add(current_process_instance() * PROCESS_LIMIT + storage_slot)
        };
        storage.hierarchy.fill(memory::FrameLease::EMPTY);
        storage.mappings.fill(cpu::LinuxActiveImageMapping::EMPTY);
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
        if storage_slot >= PROCESS_LIMIT
            || !(4..=PROCESS_HIERARCHY_LIMIT).contains(&hierarchy.len())
            || mappings.is_empty()
            || mappings.len() > PROCESS_PRIVATE_PAGE_LIMIT
        {
            return None;
        }
        let storage = unsafe {
            &mut *core::ptr::addr_of_mut!(PROCESS_ADDRESS_SPACE_STORAGE)
                .cast::<ProcessAddressSpaceStorage>()
                .add(current_process_instance() * PROCESS_LIMIT + storage_slot)
        };
        storage.hierarchy.fill(memory::FrameLease::EMPTY);
        storage.mappings.fill(cpu::LinuxActiveImageMapping::EMPTY);
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
            &*core::ptr::addr_of!(PROCESS_ADDRESS_SPACE_STORAGE)
                .cast::<ProcessAddressSpaceStorage>()
                .add(current_process_instance() * PROCESS_LIMIT + usize::from(self.storage_slot))
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ChildWaitState {
    Running,
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
        LIVE_FORK_EXECUTION_STAGE.store(1131, Ordering::Release);
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
        LIVE_FORK_EXECUTION_STAGE.store(1132, Ordering::Release);
        if parent.state != ProcessState::Running
            || !parent_space.is_owned()
            || child_hierarchy.len() != parent_space.hierarchy().len()
            || child_pages.len()
                != parent_space
                    .mappings()
                    .iter()
                    .filter(|mapping| mapping_requires_fork_copy(mapping))
                    .count()
            || !self.resources_available(child_hierarchy, child_pages)
        {
            let _ = self.abort_fork(handle);
            return false;
        }

        let scratch = unsafe { &mut *LIVE_FORK_SCRATCH.0.get() };
        LIVE_FORK_EXECUTION_STAGE.store(1133, Ordering::Release);
        let child_mappings = &mut scratch.child_mappings;
        let parent_pages = &mut scratch.parent_pages;
        child_mappings.fill(cpu::LinuxActiveImageMapping::EMPTY);
        parent_pages.fill(memory::FrameLease::EMPTY);
        let mut private_index = 0;
        let mut child_mapping_count = 0;
        for parent in parent_space.mappings() {
            if parent.dontfork {
                continue;
            }
            let frame = if mapping_requires_fork_copy(parent) {
                let frame = *child_pages
                    .get(private_index)
                    .unwrap_or(&memory::FrameLease::EMPTY);
                private_index += 1;
                frame
            } else {
                parent.frame
            };
            child_mappings[child_mapping_count] = cpu::LinuxActiveImageMapping {
                address: parent.address,
                frame,
                writable: parent.writable,
                executable: parent.executable,
                shared: parent.shared,
                dontfork: false,
            };
            child_mapping_count += 1;
        }
        let mut parent_page_count = 0;
        for mapping in parent_space
            .mappings()
            .iter()
            .filter(|mapping| mapping_requires_fork_copy(mapping))
        {
            parent_pages[parent_page_count] = mapping.frame;
            parent_page_count += 1;
        }
        LIVE_FORK_EXECUTION_STAGE.store(1134, Ordering::Release);
        let published = private_index == child_pages.len()
            && parent_page_count == child_pages.len()
            && copy_validated_private_pages(
                parent_space.mappings(),
                parent_space.root(),
                &parent_pages[..parent_page_count],
                child_pages,
            );
        LIVE_FORK_EXECUTION_STAGE.store(1135, Ordering::Release);
        let published = published
            && address_space::clone_validated_linux_process_hierarchy(
                parent_space.mappings(),
                parent_space.hierarchy(),
                child_hierarchy,
                child_pages,
            );
        LIVE_FORK_EXECUTION_STAGE.store(1136, Ordering::Release);
        let child_space = if published {
            LIVE_FORK_EXECUTION_STAGE.store(11361, Ordering::Release);
            ProcessAddressSpace::from_validated_snapshot(
                usize::from(handle.slot),
                child_hierarchy,
                &child_mappings[..child_mapping_count],
            )
        } else {
            None
        };
        LIVE_FORK_EXECUTION_STAGE.store(1137, Ordering::Release);
        if let Some(child_space) = child_space {
            LIVE_FORK_EXECUTION_STAGE.store(1138, Ordering::Release);
            let attachments_inherited =
                crate::linux_sysv_ipc::inherit_process_attachments(parent_handle.pid, handle.pid)
                    .is_ok();
            if attachments_inherited
                && self.commit_fork_inner(handle, child_space, parent_context.fork_child(), false)
            {
                LIVE_FORK_EXECUTION_STAGE.store(1139, Ordering::Release);
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
            && !pages.is_empty()
            && pages.len() <= PROCESS_PRIVATE_PAGE_LIMIT
            && memory::frames_owned_by(memory::FrameOwner::KERNEL, hierarchy)
            && memory::frames_owned_by(memory::FrameOwner::KERNEL, pages)
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

    fn child_wait_state(&self, parent: ProcessHandle, pid: Option<u32>) -> Option<ChildWaitState> {
        self.child_wait_state_matching(parent, |process| {
            pid.is_none_or(|pid| process.handle.pid == pid)
        })
    }

    fn child_group_wait_state(
        &self,
        parent: ProcessHandle,
        process_group: u32,
    ) -> Option<ChildWaitState> {
        self.child_wait_state_matching(parent, |process| process.process_group == process_group)
    }

    fn child_wait_state_matching(
        &self,
        parent: ProcessHandle,
        matches: impl Fn(&Process) -> bool,
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
            .or_else(|| children.next())?;
        match child.state {
            ProcessState::Running | ProcessState::Preparing | ProcessState::Exiting => {
                Some(ChildWaitState::Running)
            }
            ProcessState::Zombie { status } => Some(ChildWaitState::Exited {
                pid: child.handle.pid,
                status,
            }),
        }
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
            address_space,
            retiring_address_space: None,
            context,
            scheduler_thread: None,
            state,
        });
        Some(handle)
    }

    fn process_group(&self, pid: u32) -> Option<(u32, u32)> {
        let process = self.process(self.handle_by_pid(pid)?)?;
        matches!(
            process.state,
            ProcessState::Running | ProcessState::Zombie { .. }
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
    let table = core::ptr::addr_of_mut!(TEST_PROCESS_TABLE).cast::<ProcessTable>();
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
        (parent_stack as *mut u8).write(0x44);
    }
    if first_child.pid != 3 {
        crate::serial_write("ginux: lifecycle stage child identity failed\r\n");
        return false;
    }
    if !table.commit_cloned_fork(first_child, &frames[6..10], &frames[10..12]) {
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
        || table.child_wait_state(init, Some(first_child.pid)) != Some(ChildWaitState::Running)
        || table.set_process_group(init.pid, first_child.pid, first_child.pid) != Ok(())
        || table.process_group(first_child.pid) != Some((3, 1))
        || table.child_group_wait_state(init, first_child.pid) != Some(ChildWaitState::Running)
        || table.child_group_wait_state(init, init.pid).is_some()
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
        || table.child_wait_state(init, Some(first_child.pid))
            != Some(ChildWaitState::Exited {
                pid: first_child.pid,
                status: child_exit_status,
            })
        || table.child_group_wait_state(init, first_child.pid)
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
    if LIVE_PROCESS_TABLE_INITIALIZED[instance].load(Ordering::Acquire)
        || LIVE_PROCESS_ACTIVE_INSTANCE.load(Ordering::Acquire) != PROCESS_INSTANCE_NONE
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
        core::ptr::addr_of_mut!(LIVE_PROCESS_TABLES)
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
    LIVE_PROCESS_REFRESH_COUNT[instance].store(0, Ordering::Release);
    LIVE_PROCESS_TABLE_INITIALIZED[instance].store(true, Ordering::Release);
    LIVE_PROCESS_ACTIVE_INSTANCE.store(instance as u8, Ordering::Release);
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
    let address_space = if validate_address_space {
        ProcessAddressSpace::new(
            usize::from(handle.slot),
            snapshot.hierarchy(),
            snapshot.mappings(),
        )
    } else {
        ProcessAddressSpace::from_validated_snapshot(
            usize::from(handle.slot),
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
        LIVE_PROCESS_REFRESH_COUNT[instance].fetch_add(1, Ordering::SeqCst);
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

pub(crate) fn suspend_runtime_process_registry() -> bool {
    let Some(instance) = active_process_instance() else {
        return false;
    };
    if instance != current_process_instance()
        || !LIVE_PROCESS_TABLE_INITIALIZED[instance].load(Ordering::Acquire)
    {
        return false;
    }
    LIVE_PROCESS_ACTIVE_INSTANCE.store(PROCESS_INSTANCE_NONE, Ordering::Release);
    true
}

pub(crate) fn resume_runtime_process_registry() -> bool {
    let instance = current_process_instance();
    if !LIVE_PROCESS_TABLE_INITIALIZED[instance].load(Ordering::Acquire)
        || LIVE_PROCESS_ACTIVE_INSTANCE
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
        LIVE_PROCESS_ACTIVE_INSTANCE.store(PROCESS_INSTANCE_NONE, Ordering::Release);
    }
    valid
}

fn finish_live_process_registry_with_refresh_floor(refresh_floor: u32) -> bool {
    let Some(instance) = active_process_instance() else {
        return true;
    };
    if !LIVE_PROCESS_TABLE_INITIALIZED[instance].load(Ordering::Acquire) {
        return false;
    }
    let Some(table) = live_process_table_mut() else {
        return false;
    };
    let valid = table.reap_init_zombies()
        && table.reap_all_zombies()
        && table.init.is_some()
        && LIVE_PROCESS_REFRESH_COUNT[instance].load(Ordering::Acquire) >= refresh_floor
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
            LIVE_PROCESS_REFRESH_COUNT[instance].load(Ordering::Acquire),
        ));
        crate::serial_write(" ");
        crate::serial_write_u64(table.processes.iter().flatten().count() as u64);
        crate::serial_write("\r\n");
        return false;
    }
    LIVE_PROCESS_TABLE_INITIALIZED[instance].store(false, Ordering::Release);
    LIVE_PROCESS_REFRESH_COUNT[instance].store(0, Ordering::Release);
    LIVE_PROCESS_ACTIVE_INSTANCE.store(PROCESS_INSTANCE_NONE, Ordering::Release);
    true
}

pub(crate) fn live_process_user_namespace(pid: u32) -> Option<linux_user::UserNamespaceHandle> {
    let table = live_process_table_mut()?;
    let handle = table.handle_by_pid(pid)?;
    let namespace = table.process(handle)?.user_namespace;
    linux_user::credentials(namespace).map(|_| namespace)
}

pub(crate) fn live_process_status(pid: u32) -> Option<LiveProcessStatus> {
    let table = live_process_table_mut()?;
    let handle = table.handle_by_pid(pid)?;
    let process = table.process(handle)?;
    let state = match process.state {
        ProcessState::Running => LiveProcessState::Running,
        ProcessState::Zombie { .. } => LiveProcessState::Zombie,
        ProcessState::Preparing | ProcessState::Exiting => return None,
    };
    Some(LiveProcessStatus {
        pid: process.handle.pid,
        parent_pid: process.parent.map_or(0, |parent| parent.pid),
        state,
    })
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

pub(crate) fn live_process_slot(pid: u32) -> Option<usize> {
    let table = live_process_table_mut()?;
    let handle = table.handle_by_pid(pid)?;
    Some(usize::from(handle.slot))
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
    for (instance, initialized) in LIVE_PROCESS_TABLE_INITIALIZED.iter().enumerate() {
        if initialized.load(Ordering::Acquire) {
            // Linux process-table mutation and host inspection are serialized
            // on the current single-processor syscall path.
            let table = unsafe {
                &*core::ptr::addr_of!(LIVE_PROCESS_TABLES[instance]).cast::<ProcessTable>()
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

pub(crate) fn replace_live_process_mapping(
    pid: u32,
    address: usize,
    expected: memory::FrameLease,
    replacement: memory::FrameLease,
    writable: bool,
    executable: bool,
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
        &mut *core::ptr::addr_of_mut!(PROCESS_ADDRESS_SPACE_STORAGE)
            .cast::<ProcessAddressSpaceStorage>()
            .add(current_process_instance() * PROCESS_LIMIT + usize::from(space.storage_slot))
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

pub(crate) fn dispatch_live_init_with_return(value: u64) -> Option<LiveProcessDispatch> {
    let table = live_process_table_mut()?;
    let init = table.init?;
    if !table.set_return_value(init, value) {
        return None;
    }
    table.dispatch(init)
}

#[inline(never)]
pub(crate) fn begin_live_fork(parent_pid: u32) -> Option<LiveProcessDispatch> {
    LIVE_FORK_EXECUTION_STAGE.store(10, Ordering::Release);
    let Some(table) = live_process_table_mut() else {
        LIVE_FORK_EXECUTION_STAGE.store(101, Ordering::Release);
        return None;
    };
    let Some(parent_handle) = table.handle_by_pid(parent_pid) else {
        LIVE_FORK_EXECUTION_STAGE.store(102, Ordering::Release);
        return None;
    };
    let Some(parent) = table.process(parent_handle) else {
        LIVE_FORK_EXECUTION_STAGE.store(103, Ordering::Release);
        return None;
    };
    let Some(parent_space) = parent.address_space.as_ref() else {
        LIVE_FORK_EXECUTION_STAGE.store(104, Ordering::Release);
        return None;
    };
    if parent.state != ProcessState::Running || !parent_space.is_owned() {
        LIVE_FORK_EXECUTION_STAGE.store(
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
    let private_count = parent_space
        .mappings()
        .iter()
        .filter(|mapping| mapping_requires_fork_copy(mapping))
        .count();
    LIVE_FORK_EXECUTION_STAGE.store(11, Ordering::Release);
    let child_handle = table.prepare_fork(parent_handle)?;
    LIVE_FORK_EXECUTION_STAGE.store(111, Ordering::Release);
    let child_hierarchy = unsafe { &mut *LIVE_FORK_HIERARCHY_ALLOCATION_SCRATCH.0.get() };
    if !memory::allocate_frame_prefix_into(
        memory::FrameOwner::KERNEL,
        child_hierarchy,
        hierarchy_count,
    ) {
        let _ = table.abort_fork(child_handle);
        return None;
    }
    LIVE_FORK_EXECUTION_STAGE.store(112, Ordering::Release);
    if private_count == 0 || private_count > PROCESS_PRIVATE_COPY_LIMIT {
        let _ = memory::scrub_and_release_frames(
            memory::FrameOwner::KERNEL,
            &child_hierarchy[..hierarchy_count],
        );
        let _ = table.abort_fork(child_handle);
        return None;
    }
    let child_pages = unsafe { &mut *LIVE_FORK_ALLOCATION_SCRATCH.0.get() };
    if !memory::allocate_frame_prefix_into(memory::FrameOwner::KERNEL, child_pages, private_count) {
        let _ = memory::scrub_and_release_frames(
            memory::FrameOwner::KERNEL,
            &child_hierarchy[..hierarchy_count],
        );
        let _ = table.abort_fork(child_handle);
        return None;
    }
    LIVE_FORK_EXECUTION_STAGE.store(113, Ordering::Release);
    let published = table.commit_cloned_fork(
        child_handle,
        &child_hierarchy[..hierarchy_count],
        &child_pages[..private_count],
    );
    if !published {
        return None;
    }
    LIVE_FORK_EXECUTION_STAGE.store(12, Ordering::Release);
    if !table.set_return_value(parent_handle, u64::from(child_handle.pid)) {
        let _ = discard_live_fork(table, parent_handle, child_handle);
        return None;
    }
    let Some(parent_context) = table
        .process(parent_handle)
        .and_then(|process| process.context)
    else {
        let _ = discard_live_fork(table, parent_handle, child_handle);
        return None;
    };
    let Some(parent_space) = table
        .process(parent_handle)
        .and_then(|process| process.address_space.as_ref())
    else {
        let _ = discard_live_fork(table, parent_handle, child_handle);
        return None;
    };
    let Some(parent_scheduler_context) = parent_context.scheduler_context(parent_space) else {
        let _ = discard_live_fork(table, parent_handle, child_handle);
        return None;
    };
    let Some(child) = table.process(child_handle) else {
        let _ = discard_live_fork(table, parent_handle, child_handle);
        return None;
    };
    let (Some(child_space), Some(child_context)) = (child.address_space.as_ref(), child.context)
    else {
        let _ = discard_live_fork(table, parent_handle, child_handle);
        return None;
    };
    let Some(child_scheduler_context) = child_context.scheduler_context(child_space) else {
        let _ = discard_live_fork(table, parent_handle, child_handle);
        return None;
    };
    LIVE_FORK_EXECUTION_STAGE.store(13, Ordering::Release);
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
    LIVE_FORK_EXECUTION_STAGE.store(14, Ordering::Release);
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
        && retiring.scrub_and_release()
        && crate::linux_sysv_ipc::drop_process_attachments(child_handle.pid).is_ok()
        && table.exit(child_handle, 0)
        && table.wait(parent_handle, child_handle.pid)
            == Some(WaitResult {
                pid: child_handle.pid,
                status: 0,
            })
}

#[inline(never)]
pub(crate) fn complete_live_child_exit(pid: u32, status: u8) -> Option<LiveProcessDispatch> {
    LIVE_FORK_EXECUTION_STAGE.store(15, Ordering::Release);
    let table = live_process_table_mut()?;
    LIVE_FORK_EXECUTION_STAGE.store(151, Ordering::Release);
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
    LIVE_FORK_EXECUTION_STAGE.store(152, Ordering::Release);
    let parent_scheduler_context = parent_context.scheduler_context(parent_space)?;
    let child_scheduler_context = child_context.scheduler_context(&child_space)?;
    let parent_thread = parent.scheduler_thread?;
    let child_thread = child.scheduler_thread?;
    let run_queue = scheduler::active_process_scheduler_workspace()?;
    LIVE_FORK_EXECUTION_STAGE.store(153, Ordering::Release);
    let resumed = run_queue.switch_current(child_thread, child_scheduler_context, parent_thread)?;
    LIVE_FORK_EXECUTION_STAGE.store(154, Ordering::Release);
    if resumed.handle() != parent_thread || resumed.context() != parent_scheduler_context {
        return None;
    }
    LIVE_FORK_EXECUTION_STAGE.store(16, Ordering::Release);
    let nested = parent.parent.is_some();
    if !table.withdraw_from_scheduler(child_handle, run_queue) {
        return None;
    }
    if !nested
        && (!table.withdraw_from_scheduler(parent_handle, run_queue)
            || !scheduler::release_process_scheduler_workspace())
    {
        return None;
    }
    LIVE_FORK_EXECUTION_STAGE.store(161, Ordering::Release);
    if !address_space::retire_linux_child_address_space(child_space.root()) {
        LIVE_FORK_EXECUTION_STAGE.store(162, Ordering::Release);
        return None;
    }
    for mapping in child_space.mappings() {
        if !mapping.shared
            && memory::frames_owned_by(memory::FrameOwner::KERNEL, &[mapping.frame])
            && !memory::scrub_and_release_frames(
                memory::FrameOwner::KERNEL,
                core::slice::from_ref(&mapping.frame),
            )
        {
            return None;
        }
    }
    for frame in child_space.hierarchy() {
        if memory::frames_owned_by(memory::FrameOwner::KERNEL, &[*frame])
            && !memory::scrub_and_release_frames(
                memory::FrameOwner::KERNEL,
                core::slice::from_ref(frame),
            )
        {
            return None;
        }
    }
    LIVE_FORK_EXECUTION_STAGE.store(163, Ordering::Release);
    if crate::linux_sysv_ipc::drop_process_attachments(pid).is_err()
        || !table.exit_after_current_address_space_retired(child_handle, status)
    {
        LIVE_FORK_EXECUTION_STAGE.store(164, Ordering::Release);
        return None;
    }
    LIVE_FORK_EXECUTION_STAGE.store(17, Ordering::Release);
    table.dispatch(parent_handle)
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
) -> Option<ChildWaitState> {
    let table = live_process_table_mut()?;
    let parent = table.handle_by_pid(parent_pid)?;
    table.child_wait_state(parent, child_pid)
}

pub(crate) fn live_child_group_wait_state(
    parent_pid: u32,
    process_group: u32,
) -> Option<ChildWaitState> {
    let table = live_process_table_mut()?;
    let parent = table.handle_by_pid(parent_pid)?;
    table.child_group_wait_state(parent, process_group)
}

#[inline(never)]
#[allow(dead_code)]
pub(crate) fn verify_live_fork_clone_transaction() -> bool {
    LIVE_FORK_EXECUTION_STAGE.store(0, Ordering::Release);
    LIVE_FORK_CHILD_DIAGNOSTIC.store(0, Ordering::Release);
    cpu::reset_process_child_execution_diagnostic();
    if LIVE_FORK_CHILD_EXECUTED.load(Ordering::Acquire) {
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
    let child_hierarchy = unsafe { &mut *LIVE_FORK_HIERARCHY_ALLOCATION_SCRATCH.0.get() };
    if !memory::allocate_frame_prefix_into(
        memory::FrameOwner::KERNEL,
        child_hierarchy,
        hierarchy_count,
    ) {
        let _ = table.abort_fork(child_handle);
        return false;
    }
    let child_pages = unsafe { &mut *LIVE_FORK_ALLOCATION_SCRATCH.0.get() };
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
    let execute_child = child_valid && !LIVE_FORK_CHILD_EXECUTED.load(Ordering::Acquire);
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
        LIVE_FORK_CHILD_EXECUTED.store(true, Ordering::Release);
    }
    valid
}

#[inline(never)]
#[allow(dead_code)]
fn verify_live_fork_clone_only_transaction() -> bool {
    macro_rules! reject {
        ($stage:expr) => {{
            LIVE_FORK_EXECUTION_STAGE.store($stage, Ordering::Release);
            return false;
        }};
    }
    LIVE_FORK_EXECUTION_STAGE.store(200, Ordering::Release);
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
    let child_hierarchy = unsafe { &mut *LIVE_FORK_HIERARCHY_ALLOCATION_SCRATCH.0.get() };
    if !memory::allocate_frame_prefix_into(
        memory::FrameOwner::KERNEL,
        child_hierarchy,
        hierarchy_count,
    ) {
        let _ = table.abort_fork(child_handle);
        reject!(208);
    }
    let child_pages = unsafe { &mut *LIVE_FORK_ALLOCATION_SCRATCH.0.get() };
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
    LIVE_FORK_EXECUTION_STAGE.store(1, Ordering::Release);
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
    LIVE_FORK_EXECUTION_STAGE.store(2, Ordering::Release);
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
    LIVE_FORK_EXECUTION_STAGE.store(3, Ordering::Release);
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
        LIVE_FORK_EXECUTION_STAGE.store(4, Ordering::Release);
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
        LIVE_FORK_EXECUTION_STAGE.store(5, Ordering::Release);
    }
    let resumed_parent =
        exit_status.and_then(|_| run_queue.yield_current(child_thread, child_scheduler_context));
    let resumed = resumed_parent.is_some_and(|selected| {
        selected.handle() == parent_thread && selected.context() == parent_scheduler_context
    });
    if resumed {
        LIVE_FORK_EXECUTION_STAGE.store(6, Ordering::Release);
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
        LIVE_FORK_EXECUTION_STAGE.store(7, Ordering::Release);
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
    LIVE_FORK_CHILD_DIAGNOSTIC.store(diagnostic, Ordering::Release);
    valid.then_some(42)
}

pub(crate) fn live_fork_execution_stage() -> u32 {
    LIVE_FORK_EXECUTION_STAGE.load(Ordering::Acquire)
}

pub(crate) fn live_fork_child_diagnostic() -> u32 {
    LIVE_FORK_CHILD_DIAGNOSTIC.load(Ordering::Acquire)
}

fn live_process_table_mut() -> Option<&'static mut ProcessTable> {
    let instance = active_process_instance()?;
    if !LIVE_PROCESS_TABLE_INITIALIZED[instance].load(Ordering::Acquire) {
        return None;
    }
    let table = unsafe {
        core::ptr::addr_of_mut!(LIVE_PROCESS_TABLES)
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
    let instance = usize::from(LIVE_PROCESS_ACTIVE_INSTANCE.load(Ordering::Acquire));
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
