use crate::{BootInfo, UefiMemoryDescriptor, EFI_CONVENTIONAL_MEMORY};
use core::mem::MaybeUninit;
use core::sync::atomic::{AtomicBool, Ordering};

const PAGE_SIZE: u64 = 4096;
const LOW_MEMORY_LIMIT: u64 = 1024 * 1024;
const MAX_RANGES: usize = 64;
const MAX_RECYCLED_FRAMES: usize = 128;
const MAX_LIVE_FRAMES: usize = 128;

static INITIALIZED: AtomicBool = AtomicBool::new(false);
static mut GLOBAL_ALLOCATOR: MaybeUninit<FrameAllocator> = MaybeUninit::uninit();

#[derive(Clone, Copy)]
struct Range {
    next: u64,
    end: u64,
}

const EMPTY_RANGE: Range = Range { next: 0, end: 0 };

#[derive(Clone, Copy)]
pub(crate) struct FrameAllocator {
    ranges: [Range; MAX_RANGES],
    range_count: usize,
    active_range: usize,
    allocated_frames: u64,
    live_frames: [u64; MAX_LIVE_FRAMES],
    live_count: usize,
    recycled: [u64; MAX_RECYCLED_FRAMES],
    recycled_count: usize,
}

impl FrameAllocator {
    pub(crate) fn from_uefi_map(boot_info: &BootInfo, ceiling: u64) -> Option<Self> {
        let mut allocator = Self {
            ranges: [EMPTY_RANGE; MAX_RANGES],
            range_count: 0,
            active_range: 0,
            allocated_frames: 0,
            live_frames: [0; MAX_LIVE_FRAMES],
            live_count: 0,
            recycled: [0; MAX_RECYCLED_FRAMES],
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

    pub(crate) fn reserve<const N: usize>(&mut self) -> Option<[u64; N]> {
        if N == 0 {
            return None;
        }
        let mut staged = *self;
        let mut frames = [0_u64; N];
        for frame in &mut frames {
            *frame = staged.allocate_one()?;
        }
        *self = staged;
        Some(frames)
    }

    fn allocate_one(&mut self) -> Option<u64> {
        if self.live_count == MAX_LIVE_FRAMES {
            return None;
        }
        if self.recycled_count != 0 {
            self.recycled_count -= 1;
            let frame = self.recycled[self.recycled_count];
            self.recycled[self.recycled_count] = 0;
            self.record_live(frame)?;
            return Some(frame);
        }
        while self.active_range < self.range_count {
            let range = &mut self.ranges[self.active_range];
            if range.next < range.end {
                let frame = range.next;
                range.next += PAGE_SIZE;
                self.record_live(frame)?;
                return Some(frame);
            }
            self.active_range += 1;
        }
        None
    }

    pub(crate) fn release<const N: usize>(&mut self, frames: [u64; N]) -> bool {
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
            if *frame < LOW_MEMORY_LIMIT
                || *frame % PAGE_SIZE != 0
                || frames[..index].contains(frame)
                || self.recycled[..self.recycled_count].contains(frame)
                || !self.live_frames[..self.live_count].contains(frame)
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
            staged.live_frames[staged.live_count] = 0;
            staged.recycled[staged.recycled_count] = frame;
            staged.recycled_count += 1;
        }
        staged.allocated_frames -= N as u64;
        *self = staged;
        true
    }

    fn record_live(&mut self, frame: u64) -> Option<()> {
        if self.live_count == MAX_LIVE_FRAMES
            || self.live_frames[..self.live_count].contains(&frame)
        {
            return None;
        }
        let allocated = self.allocated_frames.checked_add(1)?;
        self.live_frames[self.live_count] = frame;
        self.live_count += 1;
        self.allocated_frames = allocated;
        Some(())
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

pub(crate) fn allocate_frames<const N: usize>() -> Option<[u64; N]> {
    if !INITIALIZED.load(Ordering::Acquire) {
        return None;
    }
    // SAFETY: Initialization published a valid allocator. Bootstrap is
    // single-processor and callers keep interrupts disabled, so access is
    // serialized until an SMP-safe allocator replaces this path.
    unsafe {
        let allocator = core::ptr::addr_of_mut!(GLOBAL_ALLOCATOR).cast::<FrameAllocator>();
        (*allocator).reserve::<N>()
    }
}

pub(crate) fn release_frames<const N: usize>(frames: [u64; N]) -> bool {
    if !INITIALIZED.load(Ordering::Acquire) {
        return false;
    }
    unsafe {
        let allocator = core::ptr::addr_of_mut!(GLOBAL_ALLOCATOR).cast::<FrameAllocator>();
        (*allocator).release(frames)
    }
}

pub(crate) fn verify_reservation_invariants() -> bool {
    let mut allocator = FrameAllocator {
        ranges: [EMPTY_RANGE; MAX_RANGES],
        range_count: 2,
        active_range: 0,
        allocated_frames: 0,
        live_frames: [0; MAX_LIVE_FRAMES],
        live_count: 0,
        recycled: [0; MAX_RECYCLED_FRAMES],
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
    let Some(first) = allocator.reserve::<2>() else {
        return false;
    };
    let before_failure = allocator;
    let failed = allocator.reserve::<2>().is_none();
    let failure_was_atomic = allocator.active_range == before_failure.active_range
        && allocator.ranges[0].next == before_failure.ranges[0].next
        && allocator.ranges[1].next == before_failure.ranges[1].next
        && allocator.allocated_frames == before_failure.allocated_frames;
    let Some(last) = allocator.reserve::<1>() else {
        return false;
    };
    let before_forged_release = allocator;
    let forged_release_rejected = !allocator.release([0x30_0000])
        && allocator.allocated_frames == before_forged_release.allocated_frames
        && allocator.live_count == before_forged_release.live_count
        && allocator.recycled_count == before_forged_release.recycled_count;
    let released = allocator.release(first);
    let duplicate_rejected = !allocator.release(first);
    let reused = allocator.reserve::<2>();
    first == [0x10_0000, 0x10_1000]
        && failed
        && failure_was_atomic
        && last == [0x20_0000]
        && forged_release_rejected
        && released
        && duplicate_rejected
        && reused == Some([0x10_1000, 0x10_0000])
        && allocator.allocated_frames == 3
        && allocator.live_count == 3
        && allocator.available_frames() == 0
        && allocator.reserve::<0>().is_none()
}
