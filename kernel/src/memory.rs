use crate::{BootInfo, UefiMemoryDescriptor, EFI_CONVENTIONAL_MEMORY};
use core::mem::MaybeUninit;
use core::sync::atomic::{AtomicBool, Ordering};

const PAGE_SIZE: u64 = 4096;
const LOW_MEMORY_LIMIT: u64 = 1024 * 1024;
const MAX_RANGES: usize = 64;
const MAX_RECYCLED_FRAMES: usize = 128;
const MAX_LIVE_FRAMES: usize = 128;
const DOMAIN_FRAME_LIMIT: usize = 16;

static INITIALIZED: AtomicBool = AtomicBool::new(false);
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
    pub(crate) const fn address(self) -> u64 {
        self.address
    }
}

const EMPTY_LEASE: FrameLease = FrameLease {
    address: 0,
    generation: 0,
};

#[derive(Clone, Copy)]
pub(crate) struct FrameAllocator {
    ranges: [Range; MAX_RANGES],
    range_count: usize,
    active_range: usize,
    allocated_frames: u64,
    live_frames: [FrameLease; MAX_LIVE_FRAMES],
    live_owners: [FrameOwner; MAX_LIVE_FRAMES],
    live_count: usize,
    recycled: [FrameLease; MAX_RECYCLED_FRAMES],
    recycled_count: usize,
}

impl FrameAllocator {
    pub(crate) fn from_uefi_map(boot_info: &BootInfo, ceiling: u64) -> Option<Self> {
        let mut allocator = Self {
            ranges: [EMPTY_RANGE; MAX_RANGES],
            range_count: 0,
            active_range: 0,
            allocated_frames: 0,
            live_frames: [EMPTY_LEASE; MAX_LIVE_FRAMES],
            live_owners: [FrameOwner::KERNEL; MAX_LIVE_FRAMES],
            live_count: 0,
            recycled: [EMPTY_LEASE; MAX_RECYCLED_FRAMES],
            recycled_count: 0,
        };
        let count = boot_info.memory_map_size / boot_info.memory_descriptor_size;

        for index in 0..count {
            let descriptor = unsafe {
                let address = boot_info
                    .memory_map
                    .add(index * boot_info.memory_descriptor_size);
                core::ptr::read_unaligned(address.cast::<UefiMemoryDescriptor>())
            };
            if descriptor.memory_type != EFI_CONVENTIONAL_MEMORY {
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
                allocator.insert_range(Range { next: start, end })?;
            }
        }
        allocator.sort_and_merge();
        (allocator.range_count != 0).then_some(allocator)
    }

    pub(crate) fn reserve<const N: usize>(&mut self, owner: FrameOwner) -> Option<[FrameLease; N]> {
        if N == 0 {
            return None;
        }
        let mut staged = *self;
        let mut frames = [EMPTY_LEASE; N];
        for frame in &mut frames {
            *frame = staged.allocate_one(owner)?;
        }
        *self = staged;
        Some(frames)
    }

    fn allocate_one(&mut self, owner: FrameOwner) -> Option<FrameLease> {
        if self.live_count == MAX_LIVE_FRAMES
            || (!owner.is_kernel() && self.owner_live_count(owner) >= DOMAIN_FRAME_LIMIT)
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

    pub(crate) fn release<const N: usize>(
        &mut self,
        owner: FrameOwner,
        frames: [FrameLease; N],
    ) -> bool {
        if N == 0
            || self
                .recycled_count
                .checked_add(N)
                .is_none_or(|count| count > MAX_RECYCLED_FRAMES)
            || self.allocated_frames < N as u64
            || self.live_count < N
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
        let mut staged = *self;
        for frame in frames {
            let Some(index) = staged.live_frames[..staged.live_count]
                .iter()
                .position(|live| *live == frame)
            else {
                return false;
            };
            staged.live_count -= 1;
            staged.live_frames[index] = staged.live_frames[staged.live_count];
            staged.live_owners[index] = staged.live_owners[staged.live_count];
            staged.live_frames[staged.live_count] = EMPTY_LEASE;
            staged.live_owners[staged.live_count] = FrameOwner::KERNEL;
            staged.recycled[staged.recycled_count] = frame;
            staged.recycled_count += 1;
        }
        staged.allocated_frames -= N as u64;
        *self = staged;
        true
    }

    fn record_live(&mut self, frame: FrameLease, owner: FrameOwner) -> Option<()> {
        if self.live_count == MAX_LIVE_FRAMES
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
            .is_none_or(|total| total > DOMAIN_FRAME_LIMIT)
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

    fn owns<const N: usize>(&self, owner: FrameOwner, frames: &[FrameLease; N]) -> bool {
        frames.iter().all(|frame| {
            self.live_frames[..self.live_count]
                .iter()
                .zip(&self.live_owners[..self.live_count])
                .any(|(live, live_owner)| live == frame && *live_owner == owner)
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
    let Some(allocator) = FrameAllocator::from_uefi_map(boot_info, ceiling) else {
        return false;
    };
    // SAFETY: Bootstrap initialization is single-processor and interrupts do
    // not access the allocator. Publication follows the complete write.
    unsafe {
        core::ptr::addr_of_mut!(GLOBAL_ALLOCATOR).write(MaybeUninit::new(allocator));
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
        (*allocator).reserve::<N>(owner)
    }
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

pub(crate) fn transfer_frame_owner(from: FrameOwner, to: FrameOwner) -> Option<usize> {
    if !INITIALIZED.load(Ordering::Acquire) {
        return None;
    }
    unsafe {
        let allocator = core::ptr::addr_of_mut!(GLOBAL_ALLOCATOR).cast::<FrameAllocator>();
        (*allocator).transfer_owner(from, to)
    }
}

pub(crate) fn frames_owned_by<const N: usize>(owner: FrameOwner, frames: &[FrameLease; N]) -> bool {
    if !INITIALIZED.load(Ordering::Acquire) {
        return false;
    }
    unsafe {
        let allocator = core::ptr::addr_of!(GLOBAL_ALLOCATOR).cast::<FrameAllocator>();
        (*allocator).owns(owner, frames)
    }
}

pub(crate) fn verify_reservation_invariants() -> bool {
    let mut allocator = FrameAllocator {
        ranges: [EMPTY_RANGE; MAX_RANGES],
        range_count: 2,
        active_range: 0,
        allocated_frames: 0,
        live_frames: [EMPTY_LEASE; MAX_LIVE_FRAMES],
        live_owners: [FrameOwner::KERNEL; MAX_LIVE_FRAMES],
        live_count: 0,
        recycled: [EMPTY_LEASE; MAX_RECYCLED_FRAMES],
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
        && allocator.owns(other_owner, &last);
    let mut budgeted = FrameAllocator {
        ranges: [EMPTY_RANGE; MAX_RANGES],
        range_count: 1,
        active_range: 0,
        allocated_frames: 0,
        live_frames: [EMPTY_LEASE; MAX_LIVE_FRAMES],
        live_owners: [FrameOwner::KERNEL; MAX_LIVE_FRAMES],
        live_count: 0,
        recycled: [EMPTY_LEASE; MAX_RECYCLED_FRAMES],
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
}
