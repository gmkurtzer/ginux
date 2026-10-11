use crate::{BootInfo, UefiMemoryDescriptor, EFI_CONVENTIONAL_MEMORY};
use core::mem::MaybeUninit;
use core::sync::atomic::{AtomicBool, Ordering};

const PAGE_SIZE: u64 = 4096;
const LOW_MEMORY_LIMIT: u64 = 1024 * 1024;
const MAX_RANGES: usize = 64;

static INITIALIZED: AtomicBool = AtomicBool::new(false);
static mut GLOBAL_ALLOCATOR: MaybeUninit<FrameAllocator> = MaybeUninit::uninit();

#[derive(Clone, Copy)]
struct Range {
    next: u64,
    end: u64,
}

const EMPTY_RANGE: Range = Range { next: 0, end: 0 };

pub(crate) struct FrameAllocator {
    ranges: [Range; MAX_RANGES],
    range_count: usize,
    active_range: usize,
}

impl FrameAllocator {
    pub(crate) fn from_uefi_map(boot_info: &BootInfo, ceiling: u64) -> Option<Self> {
        let mut allocator = Self {
            ranges: [EMPTY_RANGE; MAX_RANGES],
            range_count: 0,
            active_range: 0,
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

    pub(crate) fn allocate(&mut self) -> Option<u64> {
        while self.active_range < self.range_count {
            let range = &mut self.ranges[self.active_range];
            if range.next < range.end {
                let frame = range.next;
                range.next += PAGE_SIZE;
                return Some(frame);
            }
            self.active_range += 1;
        }
        None
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

pub(crate) fn allocate_frame() -> Option<u64> {
    if !INITIALIZED.load(Ordering::Acquire) {
        return None;
    }
    // SAFETY: Initialization published a valid allocator. Bootstrap is
    // single-processor and callers keep interrupts disabled, so access is
    // serialized until an SMP-safe allocator replaces this path.
    unsafe {
        let allocator = core::ptr::addr_of_mut!(GLOBAL_ALLOCATOR).cast::<FrameAllocator>();
        (*allocator).allocate()
    }
}
