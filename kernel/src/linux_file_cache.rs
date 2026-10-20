use crate::{arch::x86_64::cpu, control_abi, memory};

const PAGE_SIZE: usize = 4096;
const INITIAL_CAPACITY: usize = 64;

#[derive(Clone, Copy)]
#[repr(C)]
struct SourceEntry {
    source_base: u64,
    source_offset: u64,
    frame_address: u64,
    frame_generation: u32,
    reserved: u32,
}

#[derive(Clone, Copy)]
#[repr(C)]
struct FrameEntry {
    address: u64,
    generation: u32,
    reserved: u32,
}

#[derive(Clone, Copy)]
struct CacheTable {
    first: memory::FrameLease,
    frames: usize,
    capacity: usize,
    entries: usize,
}

impl CacheTable {
    const EMPTY: Self = Self {
        first: memory::FrameLease::EMPTY,
        frames: 0,
        capacity: 0,
        entries: 0,
    };
}

static mut CACHE_TABLES: [CacheTable; control_abi::DOMAIN_LIMIT] =
    [CacheTable::EMPTY; control_abi::DOMAIN_LIMIT];

fn current_slot() -> Option<usize> {
    cpu::current_container_slot().filter(|slot| *slot < control_abi::DOMAIN_LIMIT)
}

fn table(slot: usize) -> CacheTable {
    unsafe {
        *core::ptr::addr_of!(CACHE_TABLES)
            .cast::<CacheTable>()
            .add(slot)
    }
}

fn set_table(slot: usize, table: CacheTable) {
    unsafe {
        core::ptr::addr_of_mut!(CACHE_TABLES)
            .cast::<CacheTable>()
            .add(slot)
            .write(table)
    };
}

fn frames_for_capacity(capacity: usize) -> Option<usize> {
    capacity
        .checked_mul(core::mem::size_of::<SourceEntry>() + core::mem::size_of::<FrameEntry>())
        .map(|bytes| bytes.div_ceil(PAGE_SIZE))
}

fn source_entries(table: CacheTable) -> Option<*mut SourceEntry> {
    Some(table.first.direct_map_address()? as *mut SourceEntry)
}

fn frame_entries(table: CacheTable) -> Option<*mut FrameEntry> {
    let base = table.first.direct_map_address()?;
    let offset = table
        .capacity
        .checked_mul(core::mem::size_of::<SourceEntry>())?;
    Some(base.checked_add(offset)? as *mut FrameEntry)
}

fn source_slot(source_base: u64, source_offset: u64, capacity: usize) -> usize {
    (source_base ^ source_offset.rotate_left(23)).wrapping_mul(0x9e37_79b9_7f4a_7c15) as usize
        & (capacity - 1)
}

fn frame_slot(frame: memory::FrameLease, capacity: usize) -> usize {
    ((frame.address() >> 12) ^ u64::from(frame.generation()).rotate_left(19))
        .wrapping_mul(0x9e37_79b9_7f4a_7c15) as usize
        & (capacity - 1)
}

fn insert_into(
    table: CacheTable,
    source_base: u64,
    source_offset: u64,
    frame: memory::FrameLease,
) -> bool {
    let (Some(sources), Some(frames)) = (source_entries(table), frame_entries(table)) else {
        return false;
    };
    let mut source_index = source_slot(source_base, source_offset, table.capacity);
    for _ in 0..table.capacity {
        let entry = unsafe { sources.add(source_index).read() };
        if entry.source_base == 0 {
            unsafe {
                sources.add(source_index).write(SourceEntry {
                    source_base,
                    source_offset,
                    frame_address: frame.address(),
                    frame_generation: frame.generation(),
                    reserved: 0,
                })
            };
            break;
        }
        if entry.source_base == source_base && entry.source_offset == source_offset {
            return false;
        }
        source_index = (source_index + 1) & (table.capacity - 1);
    }
    let mut frame_index = frame_slot(frame, table.capacity);
    for _ in 0..table.capacity {
        let entry = unsafe { frames.add(frame_index).read() };
        if entry.address == 0 {
            unsafe {
                frames.add(frame_index).write(FrameEntry {
                    address: frame.address(),
                    generation: frame.generation(),
                    reserved: 0,
                })
            };
            return true;
        }
        if entry.address == frame.address() && entry.generation == frame.generation() {
            return false;
        }
        frame_index = (frame_index + 1) & (table.capacity - 1);
    }
    false
}

fn allocate_table(capacity: usize) -> Option<CacheTable> {
    if !capacity.is_power_of_two() || capacity < INITIAL_CAPACITY {
        return None;
    }
    let frames = frames_for_capacity(capacity)?;
    let (first, allocated) =
        memory::allocate_contiguous_extent(memory::FrameOwner::KERNEL, frames)?;
    Some(CacheTable {
        first,
        frames: allocated,
        capacity,
        entries: 0,
    })
}

fn grow(slot: usize, old: CacheTable) -> Option<CacheTable> {
    let capacity = if old.capacity == 0 {
        INITIAL_CAPACITY
    } else {
        old.capacity.checked_mul(2)?
    };
    let mut replacement = allocate_table(capacity)?;
    if old.capacity != 0 {
        let sources = source_entries(old)?;
        for index in 0..old.capacity {
            let entry = unsafe { sources.add(index).read() };
            if entry.source_base == 0 {
                continue;
            }
            let frame =
                memory::FrameLease::from_parts(entry.frame_address, entry.frame_generation)?;
            if !insert_into(replacement, entry.source_base, entry.source_offset, frame) {
                let _ = memory::scrub_and_release_contiguous_extent(
                    memory::FrameOwner::KERNEL,
                    replacement.first,
                    replacement.frames,
                );
                return None;
            }
            replacement.entries += 1;
        }
        if !memory::scrub_and_release_contiguous_extent(
            memory::FrameOwner::KERNEL,
            old.first,
            old.frames,
        ) {
            return None;
        }
    }
    set_table(slot, replacement);
    Some(replacement)
}

pub(crate) fn lookup(source_base: u64, source_offset: u64) -> Option<memory::FrameLease> {
    if source_base == 0 {
        return None;
    }
    let table = table(current_slot()?);
    if table.capacity == 0 {
        return None;
    }
    let sources = source_entries(table)?;
    let mut index = source_slot(source_base, source_offset, table.capacity);
    for _ in 0..table.capacity {
        let entry = unsafe { sources.add(index).read() };
        if entry.source_base == source_base && entry.source_offset == source_offset {
            return memory::FrameLease::from_parts(entry.frame_address, entry.frame_generation);
        }
        if entry.source_base == 0 {
            return None;
        }
        index = (index + 1) & (table.capacity - 1);
    }
    None
}

pub(crate) fn insert(source_base: u64, source_offset: u64, frame: memory::FrameLease) -> bool {
    if source_base == 0 || lookup(source_base, source_offset).is_some() {
        return false;
    }
    let Some(slot) = current_slot() else {
        return false;
    };
    let mut current = table(slot);
    if current.capacity == 0
        || current
            .entries
            .checked_add(1)
            .is_none_or(|entries| entries * 10 >= current.capacity * 7)
    {
        let Some(grown) = grow(slot, current) else {
            return false;
        };
        current = grown;
    }
    if !insert_into(current, source_base, source_offset, frame) {
        return false;
    }
    current.entries += 1;
    set_table(slot, current);
    true
}

pub(crate) fn contains(frame: memory::FrameLease) -> bool {
    let Some(slot) = current_slot() else {
        return false;
    };
    let table = table(slot);
    if table.capacity == 0 {
        return false;
    }
    let Some(frames) = frame_entries(table) else {
        return false;
    };
    let mut index = frame_slot(frame, table.capacity);
    for _ in 0..table.capacity {
        let entry = unsafe { frames.add(index).read() };
        if entry.address == frame.address() && entry.generation == frame.generation() {
            return true;
        }
        if entry.address == 0 {
            return false;
        }
        index = (index + 1) & (table.capacity - 1);
    }
    false
}

pub(crate) fn reset_current() -> bool {
    let Some(slot) = current_slot() else {
        return true;
    };
    let current = table(slot);
    if current.capacity == 0 {
        return true;
    }
    set_table(slot, CacheTable::EMPTY);
    let Some(sources) = source_entries(current) else {
        return false;
    };
    let mut batch = [memory::FrameLease::EMPTY; 64];
    let mut count = 0;
    for index in 0..current.capacity {
        let entry = unsafe { sources.add(index).read() };
        if entry.source_base == 0 {
            continue;
        }
        let Some(frame) =
            memory::FrameLease::from_parts(entry.frame_address, entry.frame_generation)
        else {
            return false;
        };
        batch[count] = frame;
        count += 1;
        if count == batch.len() {
            if !memory::scrub_and_release_frames(memory::FrameOwner::KERNEL, &batch) {
                return false;
            }
            count = 0;
        }
    }
    if count != 0 && !memory::scrub_and_release_frames(memory::FrameOwner::KERNEL, &batch[..count])
    {
        return false;
    }
    memory::scrub_and_release_contiguous_extent(
        memory::FrameOwner::KERNEL,
        current.first,
        current.frames,
    )
}
