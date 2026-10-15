use crate::{
    arch::x86_64::paging, BootInfo, UefiMemoryDescriptor, EFI_BOOT_SERVICES_CODE,
    EFI_BOOT_SERVICES_DATA, EFI_CONVENTIONAL_MEMORY,
};
use core::mem::MaybeUninit;
use core::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

const PAGE_SIZE: u64 = 4096;
const LOW_MEMORY_LIMIT: u64 = 1024 * 1024;
const MAX_RANGES: usize = 128;
const MAX_RECYCLED_FRAMES: usize = 262_144;
const MAX_LIVE_FRAMES: usize = 262_144;
const DOMAIN_FRAME_LIMIT: usize = 192;

static INITIALIZED: AtomicBool = AtomicBool::new(false);
static RUNTIME_EXTENT_STARTS: [AtomicU64; crate::control_abi::DOMAIN_LIMIT] =
    [const { AtomicU64::new(0) }; crate::control_abi::DOMAIN_LIMIT];
static RUNTIME_EXTENT_COUNTS: [AtomicUsize; crate::control_abi::DOMAIN_LIMIT] =
    [const { AtomicUsize::new(0) }; crate::control_abi::DOMAIN_LIMIT];
static mut GLOBAL_ALLOCATOR: MaybeUninit<FrameAllocator> = MaybeUninit::uninit();

#[derive(Clone, Copy)]
struct Range {
    next: u64,
    end: u64,
}

const EMPTY_RANGE: Range = Range { next: 0, end: 0 };

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct FrameLease {
    address: u64,
    generation: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct FrameOwner(u64);

impl FrameOwner {
    pub(crate) const KERNEL: Self = Self(0);

    pub(crate) const fn domain(raw: u64) -> Option<Self> {
        if raw == 0 || raw >> 48 != 0 {
            None
        } else {
            Some(Self(raw))
        }
    }

    const fn is_kernel(self) -> bool {
        self.0 == 0
    }
}

impl FrameLease {
    pub(crate) const EMPTY: Self = Self {
        address: 0,
        generation: 0,
    };

    pub(crate) const fn address(self) -> u64 {
        self.address
    }

    pub(crate) const fn generation(self) -> u32 {
        self.generation
    }

    pub(crate) const fn from_parts(address: u64, generation: u32) -> Option<Self> {
        if address < LOW_MEMORY_LIMIT || !address.is_multiple_of(PAGE_SIZE) || generation == 0 {
            None
        } else {
            Some(Self {
                address,
                generation,
            })
        }
    }

    pub(crate) fn direct_map_address(self) -> Option<usize> {
        paging::direct_map_range_address(self.address, PAGE_SIZE as usize)
    }
}

const EMPTY_LEASE: FrameLease = FrameLease {
    address: 0,
    generation: 0,
};

#[derive(Clone, Copy)]
pub(crate) struct FrameAllocator<
    const LIVE_LIMIT: usize = MAX_LIVE_FRAMES,
    const RECYCLED_LIMIT: usize = MAX_RECYCLED_FRAMES,
    const OWNER_LIMIT: usize = DOMAIN_FRAME_LIMIT,
> {
    ranges: [Range; MAX_RANGES],
    range_count: usize,
    active_range: usize,
    allocated_frames: u64,
    reclaimed_boot_service_frames: u64,
    live_frames: [FrameLease; LIVE_LIMIT],
    live_owners: [FrameOwner; LIVE_LIMIT],
    live_count: usize,
    recycled: [FrameLease; RECYCLED_LIMIT],
    recycled_count: usize,
}

impl<const LIVE_LIMIT: usize, const RECYCLED_LIMIT: usize, const OWNER_LIMIT: usize>
    FrameAllocator<LIVE_LIMIT, RECYCLED_LIMIT, OWNER_LIMIT>
{
    fn populate_uefi_map(&mut self, boot_info: &BootInfo, ceiling: u64) -> Option<()> {
        let count = boot_info.memory_map_size / boot_info.memory_descriptor_size;

        for index in 0..count {
            let descriptor = unsafe {
                let address = boot_info
                    .memory_map
                    .add(index * boot_info.memory_descriptor_size);
                core::ptr::read_unaligned(address.cast::<UefiMemoryDescriptor>())
            };
            let boot_service = matches!(
                descriptor.memory_type,
                EFI_BOOT_SERVICES_CODE | EFI_BOOT_SERVICES_DATA
            );
            if descriptor.memory_type != EFI_CONVENTIONAL_MEMORY && !boot_service {
                continue;
            }
            let start = descriptor.physical_start.max(LOW_MEMORY_LIMIT);
            let end = descriptor
                .page_count
                .checked_mul(PAGE_SIZE)
                .and_then(|size| descriptor.physical_start.checked_add(size))?
                .min(ceiling);
            let start = start.div_ceil(PAGE_SIZE) * PAGE_SIZE;
            let end = end / PAGE_SIZE * PAGE_SIZE;
            if start < end {
                self.insert_range(Range { next: start, end })?;
                if boot_service {
                    self.reclaimed_boot_service_frames = self
                        .reclaimed_boot_service_frames
                        .checked_add((end - start) / PAGE_SIZE)?;
                }
            }
        }
        self.sort_and_merge();
        (self.range_count != 0).then_some(())
    }

    pub(crate) fn reserve<const N: usize>(&mut self, owner: FrameOwner) -> Option<[FrameLease; N]> {
        if N == 0
            || self.live_count.checked_add(N)? > LIVE_LIMIT
            || (!owner.is_kernel() && self.owner_live_count(owner).checked_add(N)? > OWNER_LIMIT)
            || self.allocated_frames.checked_add(N as u64).is_none()
        {
            return None;
        }
        let recycled = N.min(self.recycled_count);
        if self.recycled[self.recycled_count - recycled..self.recycled_count]
            .iter()
            .any(|frame| frame.generation == u32::MAX)
        {
            return None;
        }
        let available_ranges = self.ranges[self.active_range..self.range_count]
            .iter()
            .try_fold(0_u64, |total, range| {
                total.checked_add(range.end.saturating_sub(range.next) / PAGE_SIZE)
            })?;
        if available_ranges < (N - recycled) as u64 {
            return None;
        }
        let mut frames = [EMPTY_LEASE; N];
        for frame in &mut frames {
            *frame = self.allocate_one(owner)?;
        }
        Some(frames)
    }

    pub(crate) fn reserve_contiguous<const N: usize>(
        &mut self,
        owner: FrameOwner,
    ) -> Option<[FrameLease; N]> {
        if N == 0
            || self.live_count.checked_add(N)? > LIVE_LIMIT
            || (!owner.is_kernel() && self.owner_live_count(owner).checked_add(N)? > OWNER_LIMIT)
        {
            return None;
        }
        let recycled_start = (self.recycled_count >= N)
            .then(|| {
                (0..=self.recycled_count - N).find(|start| {
                    self.recycled[*start..*start + N]
                        .iter()
                        .enumerate()
                        .all(|(index, frame)| {
                            frame.address
                                == self.recycled[*start].address + index as u64 * PAGE_SIZE
                        })
                })
            })
            .flatten();
        if let Some(recycled_start) = recycled_start {
            if self.recycled[recycled_start..recycled_start + N]
                .iter()
                .any(|frame| frame.generation == u32::MAX)
            {
                return None;
            }
            let mut frames = [EMPTY_LEASE; N];
            for (index, frame) in frames.iter_mut().enumerate() {
                let prior = self.recycled[recycled_start + index];
                *frame = FrameLease {
                    address: prior.address,
                    generation: prior.generation.checked_add(1)?,
                };
                self.record_live(*frame, owner)?;
            }
            self.recycled
                .copy_within(recycled_start + N..self.recycled_count, recycled_start);
            let prior_count = self.recycled_count;
            self.recycled_count -= N;
            self.recycled[self.recycled_count..prior_count].fill(EMPTY_LEASE);
            return Some(frames);
        }
        let size = (N as u64).checked_mul(PAGE_SIZE)?;
        let range_index = (self.active_range..self.range_count).find(|index| {
            self.ranges[*index]
                .end
                .checked_sub(self.ranges[*index].next)
                .is_some_and(|available| available >= size)
        })?;
        let start = self.ranges[range_index].next;
        self.ranges[range_index].next = start.checked_add(size)?;
        let mut frames = [EMPTY_LEASE; N];
        for (index, frame) in frames.iter_mut().enumerate() {
            *frame = FrameLease {
                address: start.checked_add((index as u64).checked_mul(PAGE_SIZE)?)?,
                generation: 1,
            };
            self.record_live(*frame, owner)?;
        }
        Some(frames)
    }

    fn reserve_contiguous_extent(&mut self, owner: FrameOwner, count: usize) -> Option<FrameLease> {
        if count == 0
            || self.live_count.checked_add(count)? > LIVE_LIMIT
            || (!owner.is_kernel()
                && self.owner_live_count(owner).checked_add(count)? > OWNER_LIMIT)
        {
            return None;
        }
        let size = (count as u64).checked_mul(PAGE_SIZE)?;
        let range_index = (self.active_range..self.range_count).find(|index| {
            self.ranges[*index]
                .end
                .checked_sub(self.ranges[*index].next)
                .is_some_and(|available| available >= size)
        })?;
        let start = self.ranges[range_index].next;
        self.ranges[range_index].next = start.checked_add(size)?;
        for index in 0..count {
            self.record_live(
                FrameLease {
                    address: start.checked_add((index as u64).checked_mul(PAGE_SIZE)?)?,
                    generation: 1,
                },
                owner,
            )?;
        }
        Some(FrameLease {
            address: start,
            generation: 1,
        })
    }

    fn release_contiguous_extent(
        &mut self,
        owner: FrameOwner,
        start: FrameLease,
        count: usize,
    ) -> bool {
        let Some(end) = (count as u64)
            .checked_mul(PAGE_SIZE)
            .and_then(|size| start.address.checked_add(size))
        else {
            return false;
        };
        if count == 0
            || start.address < LOW_MEMORY_LIMIT
            || !start.address.is_multiple_of(PAGE_SIZE)
            || start.generation == 0
            || self
                .recycled_count
                .checked_add(count)
                .is_none_or(|value| value > RECYCLED_LIMIT)
            || self.allocated_frames < count as u64
            || self.live_count < count
            || self.live_frames[..self.live_count]
                .iter()
                .zip(&self.live_owners[..self.live_count])
                .filter(|(frame, frame_owner)| {
                    **frame_owner == owner
                        && frame.generation == start.generation
                        && (start.address..end).contains(&frame.address)
                        && frame.address.is_multiple_of(PAGE_SIZE)
                })
                .count()
                != count
        {
            return false;
        }
        let mut index = 0;
        while index < self.live_count {
            let frame = self.live_frames[index];
            if self.live_owners[index] == owner
                && frame.generation == start.generation
                && (start.address..end).contains(&frame.address)
            {
                self.live_count -= 1;
                self.live_frames[index] = self.live_frames[self.live_count];
                self.live_owners[index] = self.live_owners[self.live_count];
                self.live_frames[self.live_count] = EMPTY_LEASE;
                self.live_owners[self.live_count] = FrameOwner::KERNEL;
                self.recycled[self.recycled_count] = frame;
                self.recycled_count += 1;
            } else {
                index += 1;
            }
        }
        self.allocated_frames -= count as u64;
        true
    }

    fn allocate_one(&mut self, owner: FrameOwner) -> Option<FrameLease> {
        if self.live_count == LIVE_LIMIT
            || (!owner.is_kernel() && self.owner_live_count(owner) >= OWNER_LIMIT)
        {
            return None;
        }
        if self.recycled_count != 0 {
            self.recycled_count -= 1;
            let prior = self.recycled[self.recycled_count];
            self.recycled[self.recycled_count] = EMPTY_LEASE;
            let lease = FrameLease {
                address: prior.address,
                generation: prior.generation.checked_add(1)?,
            };
            self.record_live(lease, owner)?;
            return Some(lease);
        }
        while self.active_range < self.range_count {
            let range = &mut self.ranges[self.active_range];
            if range.next < range.end {
                let address = range.next;
                range.next += PAGE_SIZE;
                let lease = FrameLease {
                    address,
                    generation: 1,
                };
                self.record_live(lease, owner)?;
                return Some(lease);
            }
            self.active_range += 1;
        }
        None
    }

    fn can_release(&self, owner: FrameOwner, frames: &[FrameLease]) -> bool {
        if frames.is_empty()
            || self
                .recycled_count
                .checked_add(frames.len())
                .is_none_or(|count| count > RECYCLED_LIMIT)
            || self.allocated_frames < frames.len() as u64
            || self.live_count < frames.len()
        {
            return false;
        }
        for (index, frame) in frames.iter().enumerate() {
            if frame.address < LOW_MEMORY_LIMIT
                || frame.address % PAGE_SIZE != 0
                || frame.generation == 0
                || frames[..index]
                    .iter()
                    .any(|prior| prior.address == frame.address)
                || self.recycled[..self.recycled_count]
                    .iter()
                    .any(|recycled| recycled.address == frame.address)
                || !self.live_frames[..self.live_count]
                    .iter()
                    .zip(&self.live_owners[..self.live_count])
                    .any(|(live, live_owner)| live == frame && *live_owner == owner)
            {
                return false;
            }
        }
        true
    }

    fn release_slice(&mut self, owner: FrameOwner, frames: &[FrameLease]) -> bool {
        if !self.can_release(owner, frames) {
            return false;
        }
        for frame in frames.iter().copied() {
            let Some(index) = self.live_frames[..self.live_count]
                .iter()
                .position(|live| *live == frame)
            else {
                return false;
            };
            self.live_count -= 1;
            self.live_frames[index] = self.live_frames[self.live_count];
            self.live_owners[index] = self.live_owners[self.live_count];
            self.live_frames[self.live_count] = EMPTY_LEASE;
            self.live_owners[self.live_count] = FrameOwner::KERNEL;
            self.recycled[self.recycled_count] = frame;
            self.recycled_count += 1;
        }
        self.allocated_frames -= frames.len() as u64;
        true
    }

    pub(crate) fn release<const N: usize>(
        &mut self,
        owner: FrameOwner,
        frames: [FrameLease; N],
    ) -> bool {
        self.release_slice(owner, &frames)
    }

    fn record_live(&mut self, frame: FrameLease, owner: FrameOwner) -> Option<()> {
        if self.live_count == LIVE_LIMIT
            || self.live_frames[..self.live_count]
                .iter()
                .any(|live| live.address == frame.address)
        {
            return None;
        }
        let allocated = self.allocated_frames.checked_add(1)?;
        self.live_frames[self.live_count] = frame;
        self.live_owners[self.live_count] = owner;
        self.live_count += 1;
        self.allocated_frames = allocated;
        Some(())
    }

    fn transfer_owner(&mut self, from: FrameOwner, to: FrameOwner) -> Option<usize> {
        if from == to || from == FrameOwner::KERNEL || to == FrameOwner::KERNEL {
            return None;
        }
        let count = self.live_owners[..self.live_count]
            .iter()
            .filter(|owner| **owner == from)
            .count();
        if count == 0 {
            return None;
        }
        if self
            .owner_live_count(to)
            .checked_add(count)
            .is_none_or(|total| total > OWNER_LIMIT)
        {
            return None;
        }
        for owner in &mut self.live_owners[..self.live_count] {
            if *owner == from {
                *owner = to;
            }
        }
        Some(count)
    }

    fn owner_live_count(&self, owner: FrameOwner) -> usize {
        self.live_owners[..self.live_count]
            .iter()
            .filter(|live_owner| **live_owner == owner)
            .count()
    }

    fn owns(&self, owner: FrameOwner, frames: &[FrameLease]) -> bool {
        frames.iter().all(|frame| {
            self.live_frames[..self.live_count]
                .iter()
                .zip(&self.live_owners[..self.live_count])
                .any(|(live, live_owner)| live == frame && *live_owner == owner)
        })
    }

    fn tracks_any(&self, frames: &[FrameLease]) -> bool {
        !frames.is_empty()
            && frames.iter().any(|frame| {
                self.live_frames[..self.live_count]
                    .iter()
                    .any(|live| live == frame)
            })
    }

    fn owns_addresses(&self, owner: FrameOwner, addresses: &[u64]) -> bool {
        !addresses.is_empty()
            && addresses.iter().all(|address| {
                self.live_frames[..self.live_count]
                    .iter()
                    .zip(&self.live_owners[..self.live_count])
                    .any(|(live, live_owner)| live.address == *address && *live_owner == owner)
            })
    }

    fn available_frames(&self) -> u64 {
        self.ranges[..self.range_count]
            .iter()
            .map(|range| (range.end - range.next) / PAGE_SIZE)
            .sum()
    }

    fn insert_range(&mut self, range: Range) -> Option<()> {
        if self.range_count == MAX_RANGES {
            return None;
        }
        self.ranges[self.range_count] = range;
        self.range_count += 1;
        Some(())
    }

    fn sort_and_merge(&mut self) {
        self.ranges[..self.range_count].sort_unstable_by_key(|range| range.next);
        let mut output = 0;
        for input in 0..self.range_count {
            let range = self.ranges[input];
            if output != 0 && range.next <= self.ranges[output - 1].end {
                self.ranges[output - 1].end = self.ranges[output - 1].end.max(range.end);
            } else {
                self.ranges[output] = range;
                output += 1;
            }
        }
        self.range_count = output;
    }
}

pub(crate) fn initialize(boot_info: &BootInfo, ceiling: u64) -> bool {
    if INITIALIZED.load(Ordering::Acquire) {
        return false;
    }
    // SAFETY: Bootstrap initialization is single-processor and interrupts do
    // not access the allocator. Constructing the large tracking tables in
    // their final static location avoids consuming the bounded kernel stack.
    unsafe {
        let allocator = core::ptr::addr_of_mut!(GLOBAL_ALLOCATOR).cast::<FrameAllocator>();
        core::ptr::write_bytes(
            allocator.cast::<u8>(),
            0,
            core::mem::size_of::<FrameAllocator>(),
        );
        if (*allocator).populate_uefi_map(boot_info, ceiling).is_none() {
            return false;
        }
    }
    INITIALIZED.store(true, Ordering::Release);
    true
}

pub(crate) fn allocate_frames<const N: usize>(owner: FrameOwner) -> Option<[FrameLease; N]> {
    if !INITIALIZED.load(Ordering::Acquire) {
        return None;
    }
    // SAFETY: Initialization published a valid allocator. Bootstrap is
    // single-processor and callers keep interrupts disabled, so access is
    // serialized until an SMP-safe allocator replaces this path.
    unsafe {
        let allocator = core::ptr::addr_of_mut!(GLOBAL_ALLOCATOR).cast::<FrameAllocator>();
        let frames = (*allocator).reserve::<N>(owner)?;
        for frame in &frames {
            let Some(address) = frame.direct_map_address() else {
                let _ = (*allocator).release(owner, frames);
                return None;
            };
            core::ptr::write_bytes(address as *mut u8, 0, PAGE_SIZE as usize);
        }
        Some(frames)
    }
}

pub(crate) fn allocate_frame_prefix<const N: usize>(
    owner: FrameOwner,
    count: usize,
) -> Option<[FrameLease; N]> {
    if !INITIALIZED.load(Ordering::Acquire) || count == 0 || count > N {
        return None;
    }
    // SAFETY: As with `allocate_frames`, bootstrap and syscall execution keep
    // allocator access serialized. A partial reservation is rolled back
    // atomically before returning failure.
    unsafe {
        let allocator = core::ptr::addr_of_mut!(GLOBAL_ALLOCATOR).cast::<FrameAllocator>();
        let mut frames = [FrameLease::EMPTY; N];
        for index in 0..count {
            let Some(frame) = (*allocator).allocate_one(owner) else {
                if index != 0 {
                    let _ = (*allocator).release_slice(owner, &frames[..index]);
                }
                return None;
            };
            let Some(address) = frame.direct_map_address() else {
                let _ = (*allocator).release_slice(owner, core::slice::from_ref(&frame));
                if index != 0 {
                    let _ = (*allocator).release_slice(owner, &frames[..index]);
                }
                return None;
            };
            core::ptr::write_bytes(address as *mut u8, 0, PAGE_SIZE as usize);
            frames[index] = frame;
        }
        Some(frames)
    }
}

pub(crate) fn allocate_contiguous_frames<const N: usize>(
    owner: FrameOwner,
) -> Option<[FrameLease; N]> {
    if !INITIALIZED.load(Ordering::Acquire) {
        return None;
    }
    // SAFETY: Initialization published a valid allocator. Bootstrap is
    // single-processor and callers keep interrupts disabled.
    unsafe {
        let allocator = core::ptr::addr_of_mut!(GLOBAL_ALLOCATOR).cast::<FrameAllocator>();
        let frames = (*allocator).reserve_contiguous::<N>(owner)?;
        for frame in &frames {
            let Some(address) = frame.direct_map_address() else {
                let _ = (*allocator).release(owner, frames);
                return None;
            };
            core::ptr::write_bytes(address as *mut u8, 0, PAGE_SIZE as usize);
        }
        Some(frames)
    }
}

pub(crate) fn allocate_contiguous_extent(
    owner: FrameOwner,
    count: usize,
) -> Option<(FrameLease, usize)> {
    if !INITIALIZED.load(Ordering::Acquire) || count == 0 {
        return None;
    }
    unsafe {
        let allocator = core::ptr::addr_of_mut!(GLOBAL_ALLOCATOR).cast::<FrameAllocator>();
        let first = (*allocator).reserve_contiguous_extent(owner, count)?;
        let address = first.direct_map_address()?;
        core::ptr::write_bytes(
            address as *mut u8,
            0,
            count.checked_mul(PAGE_SIZE as usize)?,
        );
        Some((first, count))
    }
}

pub(crate) fn scrub_and_release_contiguous_extent(
    owner: FrameOwner,
    first: FrameLease,
    count: usize,
) -> bool {
    if !INITIALIZED.load(Ordering::Acquire) || count == 0 {
        return false;
    }
    let Some(address) = first.direct_map_address() else {
        return false;
    };
    let Some(length) = count.checked_mul(PAGE_SIZE as usize) else {
        return false;
    };
    unsafe { core::ptr::write_bytes(address as *mut u8, 0, length) };
    unsafe {
        let allocator = core::ptr::addr_of_mut!(GLOBAL_ALLOCATOR).cast::<FrameAllocator>();
        (*allocator).release_contiguous_extent(owner, first, count)
    }
}

pub(crate) fn allocate_runtime_extent<const N: usize>() -> Option<[FrameLease; N]> {
    if !INITIALIZED.load(Ordering::Acquire) || N == 0 {
        return None;
    }
    let slot = crate::arch::x86_64::cpu::current_container_slot()?;
    let existing = RUNTIME_EXTENT_STARTS[slot].load(Ordering::Acquire);
    let count = RUNTIME_EXTENT_COUNTS[slot].load(Ordering::Acquire);
    let start = if existing != 0 {
        (count == N).then_some(existing)?
    } else {
        let size = (N as u64).checked_mul(PAGE_SIZE)?;
        let start = unsafe {
            let allocator = core::ptr::addr_of_mut!(GLOBAL_ALLOCATOR).cast::<FrameAllocator>();
            let index = ((*allocator).active_range..(*allocator).range_count).find(|index| {
                (*allocator).ranges[*index]
                    .end
                    .checked_sub((*allocator).ranges[*index].next)
                    .is_some_and(|available| available >= size)
            })?;
            let start = (*allocator).ranges[index].next;
            (*allocator).ranges[index].next = start.checked_add(size)?;
            start
        };
        RUNTIME_EXTENT_COUNTS[slot].store(N, Ordering::Release);
        RUNTIME_EXTENT_STARTS[slot].store(start, Ordering::Release);
        start
    };
    let frames = core::array::from_fn(|index| FrameLease {
        address: start + index as u64 * PAGE_SIZE,
        generation: 1,
    });
    for frame in &frames {
        let address = frame.direct_map_address()?;
        unsafe { core::ptr::write_bytes(address as *mut u8, 0, PAGE_SIZE as usize) };
    }
    Some(frames)
}

pub(crate) fn scrub_runtime_extent<const N: usize>(frames: [FrameLease; N]) -> bool {
    if !runtime_extent_owns(&frames) {
        return false;
    }
    frames.iter().all(|frame| {
        frame.direct_map_address().is_some_and(|address| {
            unsafe { core::ptr::write_bytes(address as *mut u8, 0, PAGE_SIZE as usize) };
            true
        })
    })
}

fn runtime_extent_owns(frames: &[FrameLease]) -> bool {
    !frames.is_empty()
        && RUNTIME_EXTENT_STARTS
            .iter()
            .zip(&RUNTIME_EXTENT_COUNTS)
            .any(|(start, count)| {
                let start = start.load(Ordering::Acquire);
                let count = count.load(Ordering::Acquire);
                let end = start.saturating_add(count as u64 * PAGE_SIZE);
                start != 0
                    && frames.iter().all(|frame| {
                        frame.generation == 1
                            && frame.address >= start
                            && frame.address < end
                            && frame.address.is_multiple_of(PAGE_SIZE)
                    })
            })
}

pub(crate) fn frame_lease_is_runtime(frame: FrameLease) -> bool {
    runtime_extent_owns(&[frame])
}

pub(crate) fn frame_address_is_runtime(address: u64) -> bool {
    FrameLease::from_parts(address, 1).is_some_and(frame_lease_is_runtime)
}

pub(crate) fn release_frames<const N: usize>(owner: FrameOwner, frames: [FrameLease; N]) -> bool {
    if !INITIALIZED.load(Ordering::Acquire) {
        return false;
    }
    unsafe {
        let allocator = core::ptr::addr_of_mut!(GLOBAL_ALLOCATOR).cast::<FrameAllocator>();
        (*allocator).release(owner, frames)
    }
}

pub(crate) fn frame_leases_releasable(owner: FrameOwner, frames: &[FrameLease]) -> bool {
    if !INITIALIZED.load(Ordering::Acquire) || frames.is_empty() {
        return false;
    }
    unsafe {
        let allocator = core::ptr::addr_of!(GLOBAL_ALLOCATOR).cast::<FrameAllocator>();
        (*allocator).can_release(owner, frames)
            && frames
                .iter()
                .all(|frame| frame.direct_map_address().is_some())
    }
}

pub(crate) fn live_frame_lease(owner: FrameOwner, address: u64) -> Option<FrameLease> {
    if !INITIALIZED.load(Ordering::Acquire) || !address.is_multiple_of(PAGE_SIZE) {
        return None;
    }
    unsafe {
        let allocator = core::ptr::addr_of!(GLOBAL_ALLOCATOR).cast::<FrameAllocator>();
        (&(*allocator).live_frames)[..(*allocator).live_count]
            .iter()
            .copied()
            .zip(
                (&(*allocator).live_owners)[..(*allocator).live_count]
                    .iter()
                    .copied(),
            )
            .find_map(|(frame, live_owner)| {
                (frame.address == address && live_owner == owner).then_some(frame)
            })
    }
}

pub(crate) fn scrub_and_release_frames(owner: FrameOwner, frames: &[FrameLease]) -> bool {
    if !INITIALIZED.load(Ordering::Acquire) || frames.is_empty() {
        return false;
    }
    // SAFETY: The allocator and direct map are serialized by the same
    // single-processor bootstrap contract as allocation. All ownership and
    // release preconditions are checked before any payload is destroyed, so a
    // stale or duplicate lease cannot scrub a newer allocation.
    unsafe {
        let allocator = core::ptr::addr_of_mut!(GLOBAL_ALLOCATOR).cast::<FrameAllocator>();
        if !(*allocator).can_release(owner, frames)
            || frames
                .iter()
                .any(|frame| frame.direct_map_address().is_none())
        {
            return false;
        }
        for frame in frames {
            let Some(address) = frame.direct_map_address() else {
                return false;
            };
            core::ptr::write_bytes(address as *mut u8, 0, PAGE_SIZE as usize);
        }
        (*allocator).release_slice(owner, frames)
    }
}

pub(crate) fn transfer_frame_owner(from: FrameOwner, to: FrameOwner) -> Option<usize> {
    if !INITIALIZED.load(Ordering::Acquire) {
        return None;
    }
    unsafe {
        let allocator = core::ptr::addr_of_mut!(GLOBAL_ALLOCATOR).cast::<FrameAllocator>();
        (*allocator).transfer_owner(from, to)
    }
}

pub(crate) fn frames_owned_by(owner: FrameOwner, frames: &[FrameLease]) -> bool {
    if !INITIALIZED.load(Ordering::Acquire) {
        return false;
    }
    unsafe {
        let allocator = core::ptr::addr_of!(GLOBAL_ALLOCATOR).cast::<FrameAllocator>();
        (*allocator).owns(owner, frames) || owner.is_kernel() && runtime_extent_owns(frames)
    }
}

pub(crate) fn any_frame_lease_live(frames: &[FrameLease]) -> bool {
    if !INITIALIZED.load(Ordering::Acquire) {
        return false;
    }
    unsafe {
        let allocator = core::ptr::addr_of!(GLOBAL_ALLOCATOR).cast::<FrameAllocator>();
        (*allocator).tracks_any(frames)
    }
}

pub(crate) fn frame_addresses_owned_by(owner: FrameOwner, addresses: &[u64]) -> bool {
    if !INITIALIZED.load(Ordering::Acquire) {
        return false;
    }
    unsafe {
        let allocator = core::ptr::addr_of!(GLOBAL_ALLOCATOR).cast::<FrameAllocator>();
        (*allocator).owns_addresses(owner, addresses)
            || owner.is_kernel()
                && !addresses.is_empty()
                && RUNTIME_EXTENT_STARTS
                    .iter()
                    .zip(&RUNTIME_EXTENT_COUNTS)
                    .any(|(start, count)| {
                        let start = start.load(Ordering::Acquire);
                        let count = count.load(Ordering::Acquire);
                        addresses.iter().all(|address| {
                            start != 0
                                && *address >= start
                                && *address < start.saturating_add(count as u64 * PAGE_SIZE)
                                && address.is_multiple_of(PAGE_SIZE)
                        })
                    })
    }
}

pub(crate) fn owner_live_frame_count(owner: FrameOwner) -> Option<usize> {
    if !INITIALIZED.load(Ordering::Acquire) {
        return None;
    }
    unsafe {
        let allocator = core::ptr::addr_of!(GLOBAL_ALLOCATOR).cast::<FrameAllocator>();
        Some((*allocator).owner_live_count(owner))
    }
}

pub(crate) fn reclaimed_boot_service_frame_count() -> Option<u64> {
    if !INITIALIZED.load(Ordering::Acquire) {
        return None;
    }
    unsafe {
        let allocator = core::ptr::addr_of!(GLOBAL_ALLOCATOR).cast::<FrameAllocator>();
        Some((*allocator).reclaimed_boot_service_frames)
    }
}

pub(crate) fn verify_reservation_invariants() -> bool {
    const TEST_FRAME_LIMIT: usize = 64;
    type TestFrameAllocator = FrameAllocator<TEST_FRAME_LIMIT, TEST_FRAME_LIMIT, 16>;
    let mut allocator = TestFrameAllocator {
        ranges: [EMPTY_RANGE; MAX_RANGES],
        range_count: 2,
        active_range: 0,
        allocated_frames: 0,
        reclaimed_boot_service_frames: 0,
        live_frames: [EMPTY_LEASE; TEST_FRAME_LIMIT],
        live_owners: [FrameOwner::KERNEL; TEST_FRAME_LIMIT],
        live_count: 0,
        recycled: [EMPTY_LEASE; TEST_FRAME_LIMIT],
        recycled_count: 0,
    };
    allocator.ranges[0] = Range {
        next: 0x10_0000,
        end: 0x10_2000,
    };
    allocator.ranges[1] = Range {
        next: 0x20_0000,
        end: 0x20_1000,
    };
    let owner = FrameOwner(1);
    let other_owner = FrameOwner(2);
    let Some(first) = allocator.reserve::<2>(owner) else {
        return false;
    };
    let before_failure = allocator;
    let failed = allocator.reserve::<2>(owner).is_none();
    let failure_was_atomic = allocator.active_range == before_failure.active_range
        && allocator.ranges[0].next == before_failure.ranges[0].next
        && allocator.ranges[1].next == before_failure.ranges[1].next
        && allocator.allocated_frames == before_failure.allocated_frames;
    let Some(last) = allocator.reserve::<1>(owner) else {
        return false;
    };
    let before_forged_release = allocator;
    let forged_release_rejected = !allocator.release(
        owner,
        [FrameLease {
            address: 0x30_0000,
            generation: 1,
        }],
    ) && allocator.allocated_frames
        == before_forged_release.allocated_frames
        && allocator.live_count == before_forged_release.live_count
        && allocator.recycled_count == before_forged_release.recycled_count;
    let wrong_owner_rejected = !allocator.release(other_owner, first);
    let released = allocator.release(owner, first);
    let duplicate_rejected = !allocator.release(owner, first);
    let reused = allocator.reserve::<2>(other_owner);
    let stale_release_rejected = !allocator.release(other_owner, first);
    let ownership_transferred = allocator.transfer_owner(owner, other_owner) == Some(1)
        && !allocator.owns(owner, &last)
        && allocator.owns(other_owner, &last)
        && allocator.owner_live_count(owner) == 0
        && allocator.owner_live_count(other_owner) == 3;
    let mut budgeted = TestFrameAllocator {
        ranges: [EMPTY_RANGE; MAX_RANGES],
        range_count: 1,
        active_range: 0,
        allocated_frames: 0,
        reclaimed_boot_service_frames: 0,
        live_frames: [EMPTY_LEASE; TEST_FRAME_LIMIT],
        live_owners: [FrameOwner::KERNEL; TEST_FRAME_LIMIT],
        live_count: 0,
        recycled: [EMPTY_LEASE; TEST_FRAME_LIMIT],
        recycled_count: 0,
    };
    budgeted.ranges[0] = Range {
        next: 0x40_0000,
        end: 0x40_0000 + 33 * PAGE_SIZE,
    };
    let before_over_budget = budgeted;
    let over_budget_atomic = budgeted.reserve::<17>(owner).is_none()
        && budgeted.live_count == before_over_budget.live_count
        && budgeted.ranges[0].next == before_over_budget.ranges[0].next;
    let budget_filled = budgeted.reserve::<16>(owner).is_some()
        && budgeted.reserve::<1>(owner).is_none()
        && budgeted.reserve::<1>(other_owner).is_some();
    let third_owner = FrameOwner(3);
    let transfer_over_budget_rejected = budgeted.transfer_owner(owner, other_owner).is_none()
        && budgeted.owner_live_count(owner) == 16
        && budgeted.owner_live_count(other_owner) == 1;
    let bounded_transfer_completed = budgeted.transfer_owner(owner, third_owner) == Some(16)
        && budgeted.owner_live_count(owner) == 0
        && budgeted.owner_live_count(third_owner) == 16;
    let mut contiguous = TestFrameAllocator {
        ranges: [EMPTY_RANGE; MAX_RANGES],
        range_count: 2,
        active_range: 0,
        allocated_frames: 0,
        reclaimed_boot_service_frames: 0,
        live_frames: [EMPTY_LEASE; TEST_FRAME_LIMIT],
        live_owners: [FrameOwner::KERNEL; TEST_FRAME_LIMIT],
        live_count: 0,
        recycled: [EMPTY_LEASE; TEST_FRAME_LIMIT],
        recycled_count: 0,
    };
    contiguous.ranges[0] = Range {
        next: 0x50_0000,
        end: 0x50_2000,
    };
    contiguous.ranges[1] = Range {
        next: 0x60_0000,
        end: 0x60_4000,
    };
    let before_contiguous_failure = contiguous;
    let contiguous_failure_atomic = contiguous.reserve_contiguous::<5>(owner).is_none()
        && contiguous.live_count == before_contiguous_failure.live_count
        && contiguous.ranges[0].next == before_contiguous_failure.ranges[0].next
        && contiguous.ranges[1].next == before_contiguous_failure.ranges[1].next;
    let contiguous_three = contiguous.reserve_contiguous::<3>(owner);
    let contiguous_two = contiguous.reserve_contiguous::<2>(owner);
    let contiguous_ranges_selected = contiguous_three.map(|frames| frames.map(FrameLease::address))
        == Some([0x60_0000, 0x60_1000, 0x60_2000])
        && contiguous_two.map(|frames| frames.map(FrameLease::address))
            == Some([0x50_0000, 0x50_1000]);
    let contiguous_released = contiguous_three
        .is_some_and(|frames| contiguous.release(owner, frames))
        && contiguous_two.is_some_and(|frames| contiguous.release(owner, frames));
    let contiguous_reused = contiguous.reserve_contiguous::<3>(other_owner);
    let contiguous_recycle_preserved = contiguous_reused.is_some_and(|frames| {
        frames.map(FrameLease::address) == [0x60_0000, 0x60_1000, 0x60_2000]
            && frames.iter().all(|frame| frame.generation == 2)
            && contiguous.release(other_owner, frames)
    });

    first.map(FrameLease::address) == [0x10_0000, 0x10_1000]
        && failed
        && failure_was_atomic
        && last.map(FrameLease::address) == [0x20_0000]
        && forged_release_rejected
        && wrong_owner_rejected
        && released
        && duplicate_rejected
        && reused.map(|leases| leases.map(FrameLease::address)) == Some([0x10_1000, 0x10_0000])
        && reused.is_some_and(|leases| leases.iter().all(|lease| lease.generation == 2))
        && stale_release_rejected
        && ownership_transferred
        && allocator.allocated_frames == 3
        && allocator.live_count == 3
        && allocator.available_frames() == 0
        && allocator.reserve::<0>(owner).is_none()
        && over_budget_atomic
        && budget_filled
        && transfer_over_budget_rejected
        && bounded_transfer_completed
        && contiguous_failure_atomic
        && contiguous_ranges_selected
        && contiguous_released
        && contiguous_recycle_preserved
}
