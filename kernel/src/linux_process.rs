use crate::memory;

const PROCESS_LIMIT: usize = 4;
const PROCESS_HIERARCHY_LIMIT: usize = 6;
const PROCESS_PRIVATE_PAGE_LIMIT: usize = 16;
const PROCESS_LEASE_LIMIT: usize = PROCESS_HIERARCHY_LIMIT + PROCESS_PRIVATE_PAGE_LIMIT;
const PAGE_SIZE: usize = 4096;
const FORK_PRIVATE_PAGE_LIMIT: usize = 2;

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
struct ProcessAddressSpace {
    hierarchy: [memory::FrameLease; PROCESS_HIERARCHY_LIMIT],
    hierarchy_count: u8,
    pages: [memory::FrameLease; PROCESS_PRIVATE_PAGE_LIMIT],
    page_count: u8,
}

impl ProcessAddressSpace {
    fn new(hierarchy: &[memory::FrameLease], pages: &[memory::FrameLease]) -> Option<Self> {
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
            return None;
        }
        let mut space = Self {
            hierarchy: [memory::FrameLease::EMPTY; PROCESS_HIERARCHY_LIMIT],
            hierarchy_count: u8::try_from(hierarchy.len()).ok()?,
            pages: [memory::FrameLease::EMPTY; PROCESS_PRIVATE_PAGE_LIMIT],
            page_count: u8::try_from(pages.len()).ok()?,
        };
        space.hierarchy[..hierarchy.len()].copy_from_slice(hierarchy);
        space.pages[..pages.len()].copy_from_slice(pages);
        Some(space)
    }

    fn leases(self) -> ([memory::FrameLease; PROCESS_LEASE_LIMIT], usize) {
        let hierarchy_count = usize::from(self.hierarchy_count);
        let page_count = usize::from(self.page_count);
        let mut leases = [memory::FrameLease::EMPTY; PROCESS_LEASE_LIMIT];
        leases[..hierarchy_count].copy_from_slice(&self.hierarchy[..hierarchy_count]);
        leases[hierarchy_count..hierarchy_count + page_count]
            .copy_from_slice(&self.pages[..page_count]);
        (leases, hierarchy_count + page_count)
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

    fn create_init(&mut self, address_space: ProcessAddressSpace) -> Option<ProcessHandle> {
        if self.init.is_some() || self.next_pid != 1 || !address_space.is_owned() {
            return None;
        }
        let handle = self.allocate(None, Some(address_space), ProcessState::Running)?;
        if handle.pid != 1 {
            return None;
        }
        self.init = Some(handle);
        Some(handle)
    }

    fn prepare_fork(&mut self, parent: ProcessHandle) -> Option<ProcessHandle> {
        if self.process(parent)?.state != ProcessState::Running {
            return None;
        }
        self.allocate(Some(parent), None, ProcessState::Preparing)
    }

    fn commit_fork(&mut self, handle: ProcessHandle, address_space: ProcessAddressSpace) -> bool {
        if !address_space.is_owned()
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
        {
            return false;
        }
        process.address_space = Some(address_space);
        process.state = ProcessState::Running;
        true
    }

    fn abort_fork(&mut self, handle: ProcessHandle) -> bool {
        let Some(process) = self.process(handle) else {
            return false;
        };
        if process.state != ProcessState::Preparing
            || process.address_space.is_some()
            || process.retiring_address_space.is_some()
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
            || !expected_space.is_owned()
        {
            return None;
        }
        process.address_space = None;
        process.retiring_address_space = Some(expected_space);
        process.state = ProcessState::Exiting;
        Some(expected_space)
    }

    fn exit(&mut self, handle: ProcessHandle, status: u8) -> bool {
        let Some(process) = self.process_mut(handle) else {
            return false;
        };
        if process.state != ProcessState::Exiting || process.address_space.is_some() {
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

pub(crate) fn verify_lifecycle_contract() -> bool {
    let Some(before) = memory::owner_live_frame_count(memory::FrameOwner::KERNEL) else {
        return false;
    };
    let Some(frames) = memory::allocate_frames::<15>(memory::FrameOwner::KERNEL) else {
        return false;
    };
    let (Some(init_space), Some(first_space), Some(second_space), Some(shared_page_space)) = (
        ProcessAddressSpace::new(&frames[0..4], &frames[4..5]),
        ProcessAddressSpace::new(&frames[5..9], &frames[9..10]),
        ProcessAddressSpace::new(&frames[10..14], &frames[14..15]),
        ProcessAddressSpace::new(&frames[5..9], &frames[4..5]),
    ) else {
        return false;
    };
    let mut table = ProcessTable::new();
    let Some(init) = table.create_init(init_space) else {
        return false;
    };
    let Some(rejected_child) = table.prepare_fork(init) else {
        return false;
    };
    let Some(stale_generation) = frames[5].generation().checked_add(1) else {
        return false;
    };
    let Some(stale_root) = memory::FrameLease::from_parts(frames[5].address(), stale_generation)
    else {
        return false;
    };
    let mut stale_hierarchy: [memory::FrameLease; 4] =
        core::array::from_fn(|index| frames[5 + index]);
    stale_hierarchy[0] = stale_root;
    let stale_space = ProcessAddressSpace::new(&stale_hierarchy, &frames[9..10]);
    let mut stale_expected = first_space;
    stale_expected.hierarchy[0] = stale_root;
    if init.pid != 1
        || rejected_child.pid != 2
        || table.create_init(first_space).is_some()
        || table.commit_fork(rejected_child, init_space)
        || table.commit_fork(rejected_child, shared_page_space)
        || stale_space.is_some()
        || table.wait(init, rejected_child.pid).is_some()
        || table.exit(rejected_child, 1)
        || !table.abort_fork(rejected_child)
        || table.commit_fork(rejected_child, first_space)
        || table.abort_fork(rejected_child)
    {
        return false;
    }
    let Some(first_child) = table.prepare_fork(init) else {
        return false;
    };
    if first_child.pid != 3
        || !table.commit_fork(first_child, first_space)
        || table.commit_fork(first_child, second_space)
        || table.wait(init, first_child.pid).is_some()
        || table.exit(first_child, 42)
        || table.begin_exit(first_child, second_space).is_some()
        || table.begin_exit(first_child, stale_expected).is_some()
    {
        return false;
    }
    let Some(first_resources) = table.begin_exit(first_child, first_space) else {
        return false;
    };
    let (first_leases, first_lease_count) = first_resources.leases();
    if table.exit(first_child, 42)
        || !memory::scrub_and_release_frames(memory::FrameOwner::KERNEL, &first_leases[..1])
        || table.exit(first_child, 42)
        || !memory::scrub_and_release_frames(
            memory::FrameOwner::KERNEL,
            &first_leases[1..first_lease_count],
        )
        || !table.exit(first_child, 42)
        || table.wait(first_child, first_child.pid).is_some()
        || table.wait(init, first_child.pid) != Some(WaitResult { pid: 3, status: 42 })
        || table.exit(first_child, 42)
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
        || table.commit_fork(second_child, first_space)
        || !table.commit_fork(second_child, second_space)
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

pub(crate) fn copy_private_pages(
    source: &[memory::FrameLease; FORK_PRIVATE_PAGE_LIMIT],
    destination: &[memory::FrameLease; FORK_PRIVATE_PAGE_LIMIT],
) -> bool {
    if !memory::frames_owned_by(memory::FrameOwner::KERNEL, source)
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
    let mut source_addresses = [0_usize; FORK_PRIVATE_PAGE_LIMIT];
    let mut destination_addresses = [0_usize; FORK_PRIVATE_PAGE_LIMIT];
    for (((source_frame, destination_frame), source_address), destination_address) in source
        .iter()
        .zip(destination)
        .zip(&mut source_addresses)
        .zip(&mut destination_addresses)
    {
        let (Some(source_direct), Some(destination_direct)) = (
            source_frame.direct_map_address(),
            destination_frame.direct_map_address(),
        ) else {
            return false;
        };
        *source_address = source_direct;
        *destination_address = destination_direct;
    }
    for (source_address, destination_address) in
        source_addresses.into_iter().zip(destination_addresses)
    {
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
