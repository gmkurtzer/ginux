use core::sync::atomic::{AtomicU16, AtomicU32, AtomicUsize, Ordering};
pub(crate) use ginux_oci::{Entry, EntryKind};
use ginux_oci::{Rootfs, RootfsMetadata};

use crate::memory::{self, FrameLease, FrameOwner};

// This module is the source-neutral, immutable root-filesystem boundary used
// by Linux VFS and ELF execution. OCI ingestion may publish an artifact here,
// but no registry, manifest, layer, or transport state crosses this boundary.

pub(crate) const ROOT_HANDLE: u16 = 1;
const ENTRY_HANDLE_BASE: usize = 2;
const PATH_CAPACITY: usize = 256;
const ENTRY_CACHE_SLOTS: usize = 64;
const PAGE_SIZE: usize = 4096;
const INDEX_EMPTY: u64 = 0;
const INDEX_TOMBSTONE: u64 = u64::MAX;

#[derive(Clone, Copy)]
#[repr(C)]
struct PathIndexEntry {
    hash: u64,
    offset: usize,
    index: usize,
}

pub(crate) struct Child {
    pub(crate) next_cursor: usize,
    pub(crate) handle: u16,
    pub(crate) entry: Entry<'static>,
    pub(crate) name: &'static [u8],
}

pub(crate) struct LaunchArguments<'a> {
    pub(crate) values: [&'a [u8]; 3],
    pub(crate) count: usize,
    pub(crate) execfn: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProcessPlanFailure<'a> {
    RootfsUnavailable,
    ExecutableNotFound,
    NotRegularFile,
    NotExecutable,
    ScriptInterpreterNotFound(&'a [u8]),
    ScriptInterpreterNotExecutable(&'a [u8]),
    UnsupportedExecutable,
    DynamicLoaderNotFound(&'a [u8]),
    DynamicLoaderNotExecutable(&'a [u8]),
    UnsupportedLoadLayout,
}

static ADDRESSES: [AtomicUsize; crate::control_abi::DOMAIN_LIMIT] =
    [const { AtomicUsize::new(0) }; crate::control_abi::DOMAIN_LIMIT];
static SIZES: [AtomicUsize; crate::control_abi::DOMAIN_LIMIT] =
    [const { AtomicUsize::new(0) }; crate::control_abi::DOMAIN_LIMIT];
static ENTRY_COUNTS: [AtomicUsize; crate::control_abi::DOMAIN_LIMIT] =
    [const { AtomicUsize::new(0) }; crate::control_abi::DOMAIN_LIMIT];
static CONTENT_LENGTHS: [AtomicUsize; crate::control_abi::DOMAIN_LIMIT] =
    [const { AtomicUsize::new(0) }; crate::control_abi::DOMAIN_LIMIT];
static INDEX_STARTS: [AtomicUsize; crate::control_abi::DOMAIN_LIMIT] =
    [const { AtomicUsize::new(0) }; crate::control_abi::DOMAIN_LIMIT];
static ENTRY_OFFSET_STARTS: [AtomicUsize; crate::control_abi::DOMAIN_LIMIT] =
    [const { AtomicUsize::new(0) }; crate::control_abi::DOMAIN_LIMIT];
static CHILD_HEAD_STARTS: [AtomicUsize; crate::control_abi::DOMAIN_LIMIT] =
    [const { AtomicUsize::new(0) }; crate::control_abi::DOMAIN_LIMIT];
static CHILD_NEXT_STARTS: [AtomicUsize; crate::control_abi::DOMAIN_LIMIT] =
    [const { AtomicUsize::new(0) }; crate::control_abi::DOMAIN_LIMIT];
static INDEX_FRAME_STARTS: [AtomicUsize; crate::control_abi::DOMAIN_LIMIT] =
    [const { AtomicUsize::new(0) }; crate::control_abi::DOMAIN_LIMIT];
static INDEX_GENERATIONS: [AtomicU32; crate::control_abi::DOMAIN_LIMIT] =
    [const { AtomicU32::new(0) }; crate::control_abi::DOMAIN_LIMIT];
static INDEX_FRAME_COUNTS: [AtomicUsize; crate::control_abi::DOMAIN_LIMIT] =
    [const { AtomicUsize::new(0) }; crate::control_abi::DOMAIN_LIMIT];
static INDEX_CAPACITIES: [AtomicUsize; crate::control_abi::DOMAIN_LIMIT] =
    [const { AtomicUsize::new(0) }; crate::control_abi::DOMAIN_LIMIT];
static ENTRY_CACHE_HANDLES: [[AtomicU16; ENTRY_CACHE_SLOTS]; crate::control_abi::DOMAIN_LIMIT] =
    [const { [const { AtomicU16::new(0) }; ENTRY_CACHE_SLOTS] }; crate::control_abi::DOMAIN_LIMIT];
static ENTRY_CACHE_OFFSETS: [[AtomicUsize; ENTRY_CACHE_SLOTS]; crate::control_abi::DOMAIN_LIMIT] =
    [const { [const { AtomicUsize::new(0) }; ENTRY_CACHE_SLOTS] };
        crate::control_abi::DOMAIN_LIMIT];

pub(crate) fn is_installed(slot: usize) -> bool {
    slot < crate::control_abi::DOMAIN_LIMIT && SIZES[slot].load(Ordering::Acquire) != 0
}

pub(crate) fn install_published_rootfs(slot: usize, bytes: &'static [u8]) -> bool {
    if slot >= crate::control_abi::DOMAIN_LIMIT || is_installed(slot) {
        return false;
    }
    let Some(rootfs) = Rootfs::parse(bytes) else {
        crate::serial_write_bytes(b"ginux: published rootfs validation failed\r\n");
        return false;
    };
    let metadata = rootfs.metadata();
    if !install_path_index(slot, &rootfs) {
        crate::serial_write_bytes(b"ginux: published rootfs pathname index allocation failed\r\n");
        return false;
    }
    clear_entry_cache(slot);
    ADDRESSES[slot].store(bytes.as_ptr() as usize, Ordering::Relaxed);
    ENTRY_COUNTS[slot].store(metadata.entry_count(), Ordering::Relaxed);
    CONTENT_LENGTHS[slot].store(metadata.content_length(), Ordering::Relaxed);
    SIZES[slot].store(bytes.len(), Ordering::Release);
    true
}

pub(crate) fn uninstall_published_rootfs(slot: usize) -> bool {
    let Some(size) = SIZES.get(slot) else {
        return false;
    };
    if size.swap(0, Ordering::AcqRel) == 0 {
        return true;
    }
    ADDRESSES[slot].store(0, Ordering::Release);
    ENTRY_COUNTS[slot].store(0, Ordering::Release);
    CONTENT_LENGTHS[slot].store(0, Ordering::Release);
    clear_entry_cache(slot);
    release_path_index(slot)
}

fn install_path_index(slot: usize, rootfs: &Rootfs<'_>) -> bool {
    let Some(capacity) = rootfs
        .entry_count()
        .checked_mul(2)
        .and_then(usize::checked_next_power_of_two)
    else {
        return false;
    };
    let Some(index_bytes) = capacity.checked_mul(core::mem::size_of::<PathIndexEntry>()) else {
        return false;
    };
    let Some(offset_bytes) = rootfs
        .entry_count()
        .checked_mul(core::mem::size_of::<usize>())
    else {
        return false;
    };
    let Some(child_head_bytes) = rootfs
        .entry_count()
        .checked_add(1)
        .and_then(|count| count.checked_mul(core::mem::size_of::<usize>()))
    else {
        return false;
    };
    let child_next_bytes = offset_bytes;
    let Some(bytes) = index_bytes
        .checked_add(offset_bytes)
        .and_then(|bytes| bytes.checked_add(child_head_bytes))
        .and_then(|bytes| bytes.checked_add(child_next_bytes))
    else {
        return false;
    };
    let frame_count = bytes.div_ceil(PAGE_SIZE);
    let Some((first, allocated)) =
        memory::allocate_contiguous_extent(FrameOwner::KERNEL, frame_count)
    else {
        return false;
    };
    let Some(address) = first.direct_map_address() else {
        let _ = memory::scrub_and_release_contiguous_extent(FrameOwner::KERNEL, first, allocated);
        return false;
    };
    let table =
        unsafe { core::slice::from_raw_parts_mut(address as *mut PathIndexEntry, capacity) };
    let Some(offset_address) = address.checked_add(index_bytes) else {
        let _ = memory::scrub_and_release_contiguous_extent(FrameOwner::KERNEL, first, allocated);
        return false;
    };
    let offsets = unsafe {
        core::slice::from_raw_parts_mut(offset_address as *mut usize, rootfs.entry_count())
    };
    let Some(child_head_address) = offset_address.checked_add(offset_bytes) else {
        let _ = memory::scrub_and_release_contiguous_extent(FrameOwner::KERNEL, first, allocated);
        return false;
    };
    let child_heads = unsafe {
        core::slice::from_raw_parts_mut(child_head_address as *mut usize, rootfs.entry_count() + 1)
    };
    let Some(child_next_address) = child_head_address.checked_add(child_head_bytes) else {
        let _ = memory::scrub_and_release_contiguous_extent(FrameOwner::KERNEL, first, allocated);
        return false;
    };
    let child_next = unsafe {
        core::slice::from_raw_parts_mut(child_next_address as *mut usize, rootfs.entry_count())
    };
    table.fill(PathIndexEntry {
        hash: INDEX_EMPTY,
        offset: 0,
        index: 0,
    });
    child_heads.fill(usize::MAX);
    child_next.fill(usize::MAX);
    let mut path = [0_u8; PATH_CAPACITY];
    let mut offset = 0_usize;
    for (index, stored_offset) in offsets.iter_mut().enumerate().take(rootfs.entry_count()) {
        let entry_offset = offset;
        *stored_offset = entry_offset;
        let Some((entry, next)) = rootfs.entry_and_next_at_offset(entry_offset) else {
            let _ =
                memory::scrub_and_release_contiguous_extent(FrameOwner::KERNEL, first, allocated);
            return false;
        };
        offset = next;
        let length = entry.prefix.len() + usize::from(!entry.prefix.is_empty()) + entry.path.len();
        if length > path.len() {
            let _ =
                memory::scrub_and_release_contiguous_extent(FrameOwner::KERNEL, first, allocated);
            return false;
        }
        let mut used = 0;
        if !entry.prefix.is_empty() {
            path[..entry.prefix.len()].copy_from_slice(entry.prefix);
            used = entry.prefix.len();
            path[used] = b'/';
            used += 1;
        }
        path[used..length].copy_from_slice(entry.path);
        let visible = &path[..length];
        let name_start = visible
            .iter()
            .rposition(|byte| *byte == b'/')
            .map_or(0, |at| at + 1);
        let name = &visible[name_start..];
        if let Some(removed) = name.strip_prefix(b".wh.") {
            if removed == b".wh..opq" {
                remove_index_children(rootfs, table, &visible[..name_start.saturating_sub(1)]);
            } else {
                let mut target = [0_u8; PATH_CAPACITY];
                let target_length = name_start
                    .checked_add(removed.len())
                    .unwrap_or(PATH_CAPACITY + 1);
                if target_length > PATH_CAPACITY {
                    let _ = memory::scrub_and_release_contiguous_extent(
                        FrameOwner::KERNEL,
                        first,
                        allocated,
                    );
                    return false;
                }
                target[..name_start].copy_from_slice(&visible[..name_start]);
                target[name_start..target_length].copy_from_slice(removed);
                remove_index_path(rootfs, table, &target[..target_length]);
            }
            continue;
        }
        if !insert_index_path(rootfs, table, visible, entry_offset, index) {
            let _ =
                memory::scrub_and_release_contiguous_extent(FrameOwner::KERNEL, first, allocated);
            return false;
        }
    }
    for index in (0..rootfs.entry_count()).rev() {
        let Some(entry) = rootfs.entry_at_offset(offsets[index]) else {
            let _ =
                memory::scrub_and_release_contiguous_extent(FrameOwner::KERNEL, first, allocated);
            return false;
        };
        let Some(length) = copy_entry_path(entry, &mut path) else {
            continue;
        };
        let visible = &path[..length];
        if find_indexed_in(rootfs, table, visible)
            .is_none_or(|(visible_index, _)| visible_index != index)
        {
            continue;
        }
        let parent = if let Some(separator) = visible.iter().rposition(|byte| *byte == b'/') {
            let Some((parent, entry)) = find_indexed_in(rootfs, table, &visible[..separator])
            else {
                continue;
            };
            if entry.kind != EntryKind::Directory {
                continue;
            }
            parent
        } else {
            rootfs.entry_count()
        };
        child_next[index] = child_heads[parent];
        child_heads[parent] = index;
    }
    INDEX_STARTS[slot].store(address, Ordering::Relaxed);
    ENTRY_OFFSET_STARTS[slot].store(address + index_bytes, Ordering::Relaxed);
    CHILD_HEAD_STARTS[slot].store(child_head_address, Ordering::Relaxed);
    CHILD_NEXT_STARTS[slot].store(child_next_address, Ordering::Relaxed);
    INDEX_FRAME_STARTS[slot].store(first.address() as usize, Ordering::Relaxed);
    INDEX_GENERATIONS[slot].store(first.generation(), Ordering::Relaxed);
    INDEX_FRAME_COUNTS[slot].store(allocated, Ordering::Relaxed);
    INDEX_CAPACITIES[slot].store(capacity, Ordering::Release);
    true
}

fn release_path_index(slot: usize) -> bool {
    let capacity = INDEX_CAPACITIES[slot].swap(0, Ordering::AcqRel);
    if capacity == 0 {
        return true;
    }
    INDEX_STARTS[slot].store(0, Ordering::Release);
    ENTRY_OFFSET_STARTS[slot].store(0, Ordering::Release);
    CHILD_HEAD_STARTS[slot].store(0, Ordering::Release);
    CHILD_NEXT_STARTS[slot].store(0, Ordering::Release);
    let start = INDEX_FRAME_STARTS[slot].swap(0, Ordering::AcqRel) as u64;
    let generation = INDEX_GENERATIONS[slot].swap(0, Ordering::AcqRel);
    let count = INDEX_FRAME_COUNTS[slot].swap(0, Ordering::AcqRel);
    FrameLease::from_parts(start, generation).is_some_and(|first| {
        memory::scrub_and_release_contiguous_extent(FrameOwner::KERNEL, first, count)
    })
}

fn path_hash(path: &[u8]) -> u64 {
    let hash = path.iter().fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x100_0000_01b3)
    });
    match hash {
        INDEX_EMPTY | INDEX_TOMBSTONE => hash ^ 0x8000_0000_0000_0000,
        _ => hash,
    }
}

fn entry_path_matches(entry: Entry<'_>, path: &[u8]) -> bool {
    if entry.prefix.is_empty() {
        entry.path == path
    } else {
        path.len() == entry.prefix.len() + 1 + entry.path.len()
            && path.starts_with(entry.prefix)
            && path[entry.prefix.len()] == b'/'
            && &path[entry.prefix.len() + 1..] == entry.path
    }
}

fn index_table(slot: usize) -> Option<&'static [PathIndexEntry]> {
    let capacity = INDEX_CAPACITIES.get(slot)?.load(Ordering::Acquire);
    let start = INDEX_STARTS.get(slot)?.load(Ordering::Relaxed);
    (capacity != 0 && start != 0)
        .then(|| unsafe { core::slice::from_raw_parts(start as *const PathIndexEntry, capacity) })
}

fn entry_offsets(slot: usize) -> Option<&'static [usize]> {
    let count = ENTRY_COUNTS.get(slot)?.load(Ordering::Acquire);
    let start = ENTRY_OFFSET_STARTS.get(slot)?.load(Ordering::Relaxed);
    (count != 0 && start != 0)
        .then(|| unsafe { core::slice::from_raw_parts(start as *const usize, count) })
}

fn entry_offset(slot: usize, index: usize) -> Option<usize> {
    entry_offsets(slot)?.get(index).copied()
}

fn find_indexed<'a>(slot: usize, rootfs: &Rootfs<'a>, path: &[u8]) -> Option<(usize, Entry<'a>)> {
    let table = index_table(slot)?;
    find_indexed_in(rootfs, table, path)
}

fn find_indexed_in<'a>(
    rootfs: &Rootfs<'a>,
    table: &[PathIndexEntry],
    path: &[u8],
) -> Option<(usize, Entry<'a>)> {
    let hash = path_hash(path);
    for probe in 0..table.len() {
        let indexed = table[(hash as usize + probe) & (table.len() - 1)];
        if indexed.hash == INDEX_EMPTY {
            return None;
        }
        if indexed.hash == hash {
            let entry = rootfs.entry_at_offset(indexed.offset)?;
            if entry_path_matches(entry, path) {
                return Some((indexed.index, entry));
            }
        }
    }
    None
}

fn find_visible<'a>(rootfs: &Rootfs<'a>, path: &[u8]) -> Option<(usize, Entry<'a>)> {
    if let Some(slot) = crate::arch::x86_64::cpu::current_container_slot() {
        if index_table(slot).is_some() {
            return find_indexed(slot, rootfs, path);
        }
    }
    rootfs.find_index(path)
}

fn insert_index_path(
    rootfs: &Rootfs<'_>,
    table: &mut [PathIndexEntry],
    path: &[u8],
    offset: usize,
    index: usize,
) -> bool {
    let hash = path_hash(path);
    let mut available = None;
    for probe in 0..table.len() {
        let slot = (hash as usize + probe) & (table.len() - 1);
        let indexed = table[slot];
        if indexed.hash == INDEX_TOMBSTONE {
            available.get_or_insert(slot);
        } else if indexed.hash == INDEX_EMPTY {
            table[available.unwrap_or(slot)] = PathIndexEntry {
                hash,
                offset,
                index,
            };
            return true;
        } else if indexed.hash == hash
            && rootfs
                .entry_at_offset(indexed.offset)
                .is_some_and(|entry| entry_path_matches(entry, path))
        {
            table[slot] = PathIndexEntry {
                hash,
                offset,
                index,
            };
            return true;
        }
    }
    false
}

fn remove_index_path(rootfs: &Rootfs<'_>, table: &mut [PathIndexEntry], path: &[u8]) {
    let hash = path_hash(path);
    for probe in 0..table.len() {
        let slot = (hash as usize + probe) & (table.len() - 1);
        let indexed = table[slot];
        if indexed.hash == INDEX_EMPTY {
            return;
        }
        if indexed.hash == hash
            && rootfs
                .entry_at_offset(indexed.offset)
                .is_some_and(|entry| entry_path_matches(entry, path))
        {
            table[slot].hash = INDEX_TOMBSTONE;
            return;
        }
    }
}

fn remove_index_children(rootfs: &Rootfs<'_>, table: &mut [PathIndexEntry], parent: &[u8]) {
    for indexed in table {
        if matches!(indexed.hash, INDEX_EMPTY | INDEX_TOMBSTONE) {
            continue;
        }
        let Some(entry) = rootfs.entry_at_offset(indexed.offset) else {
            continue;
        };
        let descendant = if parent.is_empty() {
            true
        } else if entry.prefix.is_empty() {
            entry.path.starts_with(parent) && entry.path.get(parent.len()) == Some(&b'/')
        } else {
            entry.prefix.starts_with(parent)
                && (entry.prefix.len() == parent.len()
                    || entry.prefix.get(parent.len()) == Some(&b'/'))
        };
        if descendant {
            indexed.hash = INDEX_TOMBSTONE;
        }
    }
}

fn rootfs() -> Option<Rootfs<'static>> {
    rootfs_for(crate::arch::x86_64::cpu::current_container_slot()?)
}

fn rootfs_for(slot: usize) -> Option<Rootfs<'static>> {
    let size = SIZES.get(slot)?.load(Ordering::Acquire);
    let address = ADDRESSES.get(slot)?.load(Ordering::Relaxed);
    if address == 0 || size == 0 {
        return None;
    }
    // SAFETY: installation records an immutable slice embedded in the loaded
    // EFI image. The image remains mapped and read-only for the kernel lifetime.
    let bytes = unsafe { core::slice::from_raw_parts(address as *const u8, size) };
    Rootfs::from_metadata(
        bytes,
        RootfsMetadata::new(
            ENTRY_COUNTS.get(slot)?.load(Ordering::Relaxed),
            CONTENT_LENGTHS.get(slot)?.load(Ordering::Relaxed),
        )?,
    )
}

pub(crate) fn lookup_at(directory: u16, path: &[u8]) -> Option<(u16, Option<Entry<'static>>)> {
    let rootfs = rootfs()?;
    let (resolved, length) = resolve_at_in(&rootfs, directory, path)?;
    if length == 0 {
        return Some((ROOT_HANDLE, None));
    }
    let (index, entry) = lookup_in(&rootfs, &resolved[..length])?;
    let handle = u16::try_from(index.checked_add(ENTRY_HANDLE_BASE)?).ok()?;
    Some((handle, Some(entry)))
}

pub(crate) fn lookup_metadata_at(
    directory: u16,
    path: &[u8],
) -> Option<(u16, Option<Entry<'static>>)> {
    let rootfs = rootfs()?;
    let (resolved, length) = resolve_at_in(&rootfs, directory, path)?;
    if length == 0 {
        return Some((ROOT_HANDLE, None));
    }
    let (index, entry) = lookup_metadata_in(&rootfs, &resolved[..length])?;
    if entry.kind == EntryKind::Hardlink {
        let target = entry.link.strip_prefix(b"/").unwrap_or(entry.link);
        let (index, entry) = lookup_in(&rootfs, target)?;
        let handle = u16::try_from(index.checked_add(ENTRY_HANDLE_BASE)?).ok()?;
        return Some((handle, Some(entry)));
    }
    let handle = u16::try_from(index.checked_add(ENTRY_HANDLE_BASE)?).ok()?;
    Some((handle, Some(entry)))
}

fn lookup_metadata_in<'a>(rootfs: &Rootfs<'a>, path: &[u8]) -> Option<(usize, Entry<'a>)> {
    let path = path.strip_prefix(b"/").unwrap_or(path);
    let path = path.strip_suffix(b"/").unwrap_or(path);
    if path.is_empty() {
        return None;
    }
    let mut current = [0_u8; PATH_CAPACITY];
    current.get_mut(..path.len())?.copy_from_slice(path);
    let mut length = path.len();
    for _ in 0..16 {
        if let Some(entry) = find_visible(rootfs, &current[..length]) {
            return Some(entry);
        }
        let mut source = [0_u8; PATH_CAPACITY];
        source[..length].copy_from_slice(&current[..length]);
        let mut replaced = false;
        for prefix_end in source[..length]
            .iter()
            .enumerate()
            .filter_map(|(index, byte)| (*byte == b'/').then_some(index))
        {
            let Some((_, entry)) = find_visible(rootfs, &source[..prefix_end]) else {
                continue;
            };
            if !matches!(entry.kind, EntryKind::Symlink | EntryKind::Hardlink) {
                continue;
            }
            length = replace_link_path(
                &source[..prefix_end],
                entry.kind,
                entry.link,
                &source[prefix_end + 1..length],
                &mut current,
            )?;
            replaced = true;
            break;
        }
        if !replaced {
            return None;
        }
    }
    None
}

fn resolve_at_in(
    rootfs: &Rootfs<'_>,
    directory: u16,
    path: &[u8],
) -> Option<([u8; PATH_CAPACITY], usize)> {
    let absolute = path.starts_with(b"/");
    let mut resolved = [0_u8; PATH_CAPACITY];
    let mut length = if absolute || directory == ROOT_HANDLE {
        0
    } else {
        let index = usize::from(directory).checked_sub(ENTRY_HANDLE_BASE)?;
        let slot = crate::arch::x86_64::cpu::current_container_slot()?;
        let entry = rootfs.entry_at_offset(entry_offset(slot, index)?)?;
        if entry.kind != EntryKind::Directory {
            return None;
        }
        copy_entry_path(entry, &mut resolved)?
    };
    for component in path.split(|byte| *byte == b'/') {
        if component.is_empty() || component == b"." {
            continue;
        }
        if component == b".." {
            length = resolved[..length]
                .iter()
                .rposition(|byte| *byte == b'/')
                .unwrap_or(0);
            continue;
        }
        if length != 0 {
            *resolved.get_mut(length)? = b'/';
            length += 1;
        }
        resolved
            .get_mut(length..length.checked_add(component.len())?)?
            .copy_from_slice(component);
        length += component.len();
    }
    Some((resolved, length))
}

pub(crate) fn copy_path(handle: u16, output: &mut [u8]) -> Option<usize> {
    if handle == ROOT_HANDLE {
        return Some(0);
    }
    let index = usize::from(handle).checked_sub(ENTRY_HANDLE_BASE)?;
    let slot = crate::arch::x86_64::cpu::current_container_slot()?;
    let entry = rootfs_for(slot)?.entry_at_offset(entry_offset(slot, index)?)?;
    copy_entry_path(entry, output)
}

fn lookup_in<'a>(rootfs: &Rootfs<'a>, path: &[u8]) -> Option<(usize, Entry<'a>)> {
    let path = path.strip_prefix(b"/").unwrap_or(path);
    let path = path.strip_suffix(b"/").unwrap_or(path);
    if path.is_empty() {
        return None;
    }
    let mut current = [0_u8; PATH_CAPACITY];
    current.get_mut(..path.len())?.copy_from_slice(path);
    let mut length = path.len();
    for _ in 0..16 {
        if let Some((index, entry)) = find_visible(rootfs, &current[..length]) {
            match entry.kind {
                EntryKind::Symlink | EntryKind::Hardlink => {
                    let mut source = [0_u8; PATH_CAPACITY];
                    source[..length].copy_from_slice(&current[..length]);
                    length = replace_link_path(
                        &source[..length],
                        entry.kind,
                        entry.link,
                        b"",
                        &mut current,
                    )?;
                }
                _ => return Some((index, entry)),
            }
            continue;
        }
        let mut source = [0_u8; PATH_CAPACITY];
        source[..length].copy_from_slice(&current[..length]);
        let mut replaced = false;
        for prefix_end in source[..length]
            .iter()
            .enumerate()
            .filter_map(|(index, byte)| (*byte == b'/').then_some(index))
        {
            let Some((_, entry)) = find_visible(rootfs, &source[..prefix_end]) else {
                continue;
            };
            if !matches!(entry.kind, EntryKind::Symlink | EntryKind::Hardlink) {
                continue;
            }
            length = replace_link_path(
                &source[..prefix_end],
                entry.kind,
                entry.link,
                &source[prefix_end + 1..length],
                &mut current,
            )?;
            replaced = true;
            break;
        }
        if !replaced {
            return None;
        }
    }
    None
}

fn replace_link_path(
    source: &[u8],
    kind: EntryKind,
    link: &[u8],
    remainder: &[u8],
    output: &mut [u8; PATH_CAPACITY],
) -> Option<usize> {
    let mut length = if kind == EntryKind::Hardlink {
        let target = link.strip_prefix(b"/").unwrap_or(link);
        output.get_mut(..target.len())?.copy_from_slice(target);
        target.len()
    } else {
        resolve_link(source, link, output)?
    };
    if !remainder.is_empty() {
        *output.get_mut(length)? = b'/';
        length += 1;
        output
            .get_mut(length..length.checked_add(remainder.len())?)?
            .copy_from_slice(remainder);
        length += remainder.len();
    }
    Some(length)
}

pub(crate) fn entry(handle: u16) -> Option<Entry<'static>> {
    let index = usize::from(handle).checked_sub(ENTRY_HANDLE_BASE)?;
    let slot = crate::arch::x86_64::cpu::current_container_slot()?;
    let cache = usize::from(handle) % ENTRY_CACHE_SLOTS;
    let rootfs = rootfs_for(slot)?;
    if ENTRY_CACHE_HANDLES[slot][cache].load(Ordering::Acquire) == handle {
        let offset = ENTRY_CACHE_OFFSETS[slot][cache].load(Ordering::Relaxed);
        return rootfs.entry_at_offset(offset);
    }
    let offset = entry_offset(slot, index)?;
    let entry = rootfs.entry_at_offset(offset)?;
    ENTRY_CACHE_OFFSETS[slot][cache].store(offset, Ordering::Relaxed);
    ENTRY_CACHE_HANDLES[slot][cache].store(handle, Ordering::Release);
    Some(entry)
}

fn clear_entry_cache(slot: usize) {
    for handle in &ENTRY_CACHE_HANDLES[slot] {
        handle.store(0, Ordering::Release);
    }
}

pub(crate) fn next_child(directory: u16, cursor: usize) -> Option<Option<Child>> {
    let slot = crate::arch::x86_64::cpu::current_container_slot()?;
    let rootfs = rootfs()?;
    let parent_index = if directory == ROOT_HANDLE {
        rootfs.entry_count()
    } else {
        let index = usize::from(directory).checked_sub(ENTRY_HANDLE_BASE)?;
        let entry = rootfs.entry_at_offset(entry_offset(slot, index)?)?;
        if entry.kind != EntryKind::Directory {
            return None;
        }
        index
    };
    let offsets = entry_offsets(slot)?;
    let head_start = CHILD_HEAD_STARTS.get(slot)?.load(Ordering::Acquire);
    let next_start = CHILD_NEXT_STARTS.get(slot)?.load(Ordering::Acquire);
    if head_start == 0 || next_start == 0 {
        return None;
    }
    let end_cursor = offsets.len().checked_add(1)?;
    if cursor == end_cursor {
        return Some(None);
    }
    let heads = unsafe {
        core::slice::from_raw_parts(head_start as *const usize, offsets.len().checked_add(1)?)
    };
    let next = unsafe { core::slice::from_raw_parts(next_start as *const usize, offsets.len()) };
    let index = if cursor == 0 {
        *heads.get(parent_index)?
    } else {
        cursor.checked_sub(1)?
    };
    if index == usize::MAX {
        return Some(None);
    }
    let entry = rootfs.entry_at_offset(*offsets.get(index)?)?;
    let handle = u16::try_from(index.checked_add(ENTRY_HANDLE_BASE)?).ok()?;
    let name = entry.path.rsplit(|byte| *byte == b'/').next()?;
    Some(Some(Child {
        next_cursor: next[index].checked_add(1).unwrap_or(end_cursor),
        handle,
        entry,
        name,
    }))
}

fn copy_entry_path(entry: Entry<'_>, output: &mut [u8]) -> Option<usize> {
    let length = entry.prefix.len() + usize::from(!entry.prefix.is_empty()) + entry.path.len();
    if length > output.len() {
        return None;
    }
    let mut used = 0;
    if !entry.prefix.is_empty() {
        output[..entry.prefix.len()].copy_from_slice(entry.prefix);
        used = entry.prefix.len();
        output[used] = b'/';
        used += 1;
    }
    output[used..length].copy_from_slice(entry.path);
    Some(length)
}

fn resolve_link(path: &[u8], link: &[u8], output: &mut [u8]) -> Option<usize> {
    let mut length = if link.starts_with(b"/") {
        0
    } else {
        path.iter().rposition(|byte| *byte == b'/').unwrap_or(0)
    };
    if length != 0 {
        output.get_mut(..length)?.copy_from_slice(&path[..length]);
    }
    for component in link.split(|byte| *byte == b'/') {
        if component.is_empty() || component == b"." {
            continue;
        }
        if component == b".." {
            length = output[..length]
                .iter()
                .rposition(|byte| *byte == b'/')
                .unwrap_or(0);
            continue;
        }
        if length != 0 {
            *output.get_mut(length)? = b'/';
            length += 1;
        }
        output
            .get_mut(length..length.checked_add(component.len())?)?
            .copy_from_slice(component);
        length += component.len();
    }
    (length != 0).then_some(length)
}

pub(crate) fn process_plan(
    init: &[u8],
) -> Option<crate::linux_elf::RelocatableExecutablePlan<'static, 'static>> {
    process_plan_in(&rootfs()?, init)
}

pub(crate) fn process_plan_failure(init: &[u8]) -> ProcessPlanFailure<'static> {
    let Some(rootfs) = rootfs() else {
        return ProcessPlanFailure::RootfsUnavailable;
    };
    let Some((_, mut executable)) = lookup_in(&rootfs, init) else {
        return ProcessPlanFailure::ExecutableNotFound;
    };
    if executable.kind != EntryKind::File {
        return ProcessPlanFailure::NotRegularFile;
    }
    if executable.mode & 0o111 == 0 {
        return ProcessPlanFailure::NotExecutable;
    }
    if let Some((interpreter, _)) = shebang_command(executable.data) {
        let Some((_, script_interpreter)) = lookup_in(&rootfs, interpreter) else {
            return ProcessPlanFailure::ScriptInterpreterNotFound(interpreter);
        };
        if script_interpreter.kind != EntryKind::File || script_interpreter.mode & 0o111 == 0 {
            return ProcessPlanFailure::ScriptInterpreterNotExecutable(interpreter);
        }
        executable = script_interpreter;
    }
    let Some(interpreter_path) = crate::linux_elf::relocatable_interpreter(executable.data) else {
        return ProcessPlanFailure::UnsupportedExecutable;
    };
    let Some((_, interpreter)) = lookup_in(&rootfs, interpreter_path) else {
        return ProcessPlanFailure::DynamicLoaderNotFound(interpreter_path);
    };
    if interpreter.kind != EntryKind::File || interpreter.mode & 0o111 == 0 {
        return ProcessPlanFailure::DynamicLoaderNotExecutable(interpreter_path);
    }
    ProcessPlanFailure::UnsupportedLoadLayout
}

pub(crate) fn executable_diagnostic(init: &[u8]) -> Option<(usize, u64, u16, u16)> {
    let rootfs = rootfs()?;
    let (_, executable) = lookup_in(&rootfs, init)?;
    let head = u64::from_le_bytes(executable.data.get(..8)?.try_into().ok()?);
    let kind = u16::from_le_bytes(executable.data.get(16..18)?.try_into().ok()?);
    let headers = u16::from_le_bytes(executable.data.get(56..58)?.try_into().ok()?);
    Some((executable.data.len(), head, kind, headers))
}

pub(crate) fn executable_program_headers(init: &[u8]) -> Option<([u64; 16], usize)> {
    let rootfs = rootfs()?;
    let (_, executable) = lookup_in(&rootfs, init)?;
    let count = usize::from(u16::from_le_bytes(
        executable.data.get(56..58)?.try_into().ok()?,
    ));
    if count > 16 {
        return None;
    }
    let mut headers = [0_u64; 16];
    for (index, output) in headers[..count].iter_mut().enumerate() {
        let offset = 64 + index * 56;
        let kind = u32::from_le_bytes(executable.data.get(offset..offset + 4)?.try_into().ok()?);
        let flags = u32::from_le_bytes(
            executable
                .data
                .get(offset + 4..offset + 8)?
                .try_into()
                .ok()?,
        );
        *output = u64::from(kind) | (u64::from(flags) << 32);
    }
    Some((headers, count))
}

pub(crate) fn launch_arguments<'a>(init: &'a [u8]) -> Option<LaunchArguments<'a>> {
    let rootfs = rootfs()?;
    let (_, executable) = lookup_in(&rootfs, init)?;
    if let Some((interpreter, argument)) = shebang_command(executable.data) {
        let mut values = [b"" as &[u8]; 3];
        values[0] = interpreter;
        let count = if let Some(argument) = argument {
            values[1] = argument;
            values[2] = init;
            3
        } else {
            values[1] = init;
            2
        };
        Some(LaunchArguments {
            values,
            count,
            execfn: count - 1,
        })
    } else {
        Some(LaunchArguments {
            values: [init, b"", b""],
            count: 1,
            execfn: 0,
        })
    }
}

fn process_plan_in<'a>(
    rootfs: &Rootfs<'a>,
    init: &[u8],
) -> Option<crate::linux_elf::RelocatableExecutablePlan<'a, 'a>> {
    let (_, mut executable) = lookup_in(rootfs, init)?;
    if executable.kind != EntryKind::File || executable.mode & 0o111 == 0 {
        return None;
    }
    if let Some((interpreter, _)) = shebang_command(executable.data) {
        let (_, script_interpreter) = lookup_in(rootfs, interpreter)?;
        if script_interpreter.kind != EntryKind::File || script_interpreter.mode & 0o111 == 0 {
            return None;
        }
        executable = script_interpreter;
    }
    let Some(interpreter_path) = crate::linux_elf::relocatable_interpreter(executable.data) else {
        return crate::linux_elf::plan_relocatable_static(executable.data, 0x40_0000)
            .map(crate::linux_elf::RelocatableExecutablePlan::Static)
            .or_else(|| {
                crate::linux_elf::plan_static_exec(executable.data)
                    .map(crate::linux_elf::RelocatableExecutablePlan::Fixed)
            });
    };
    let (_, interpreter) = lookup_in(rootfs, interpreter_path)?;
    if interpreter.kind != EntryKind::File || interpreter.mode & 0o111 == 0 {
        return None;
    }
    crate::linux_elf::plan_relocatable_process_auto(
        executable.data,
        0x40_0000,
        interpreter_path,
        interpreter.data,
    )
    .map(crate::linux_elf::RelocatableExecutablePlan::Interpreted)
}

fn shebang_command(bytes: &[u8]) -> Option<(&[u8], Option<&[u8]>)> {
    let line = bytes.strip_prefix(b"#!")?;
    let end = line.iter().position(|byte| *byte == b'\n')?;
    let line = &line[..end];
    let start = line.iter().position(|byte| !byte.is_ascii_whitespace())?;
    let path = &line[start..];
    let end = path
        .iter()
        .position(u8::is_ascii_whitespace)
        .unwrap_or(path.len());
    let interpreter = &path[..end];
    if interpreter.is_empty() || !interpreter.starts_with(b"/") {
        return None;
    }
    let argument = path[end..]
        .iter()
        .position(|byte| !byte.is_ascii_whitespace())
        .map(|start| &path[end + start..])
        .filter(|argument| !argument.is_empty());
    Some((interpreter, argument))
}
