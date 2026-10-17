use core::sync::atomic::{AtomicU16, AtomicU32, AtomicUsize, Ordering};
use ginux_oci::{Entry, EntryKind, Rootfs, RootfsMetadata};

use crate::memory::{self, FrameLease, FrameOwner};

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

pub(crate) fn install_runtime_rootfs(slot: usize, bytes: &'static [u8]) -> bool {
    if slot >= crate::control_abi::DOMAIN_LIMIT || is_installed(slot) {
        return false;
    }
    let Some(rootfs) = Rootfs::parse(bytes) else {
        crate::serial_write_bytes(b"ginux: OCI rootfs tar validation failed\r\n");
        return false;
    };
    if process_plan_in(&rootfs, b"/bin/sh").is_none() {
        crate::serial_write_bytes(b"ginux: OCI rootfs /bin/sh execution plan failed\r\n");
        return false;
    }
    let metadata = rootfs.metadata();
    if !install_path_index(slot, &rootfs) {
        crate::serial_write_bytes(b"ginux: OCI rootfs pathname index allocation failed\r\n");
        return false;
    }
    clear_entry_cache(slot);
    ADDRESSES[slot].store(bytes.as_ptr() as usize, Ordering::Relaxed);
    ENTRY_COUNTS[slot].store(metadata.entry_count(), Ordering::Relaxed);
    CONTENT_LENGTHS[slot].store(metadata.content_length(), Ordering::Relaxed);
    SIZES[slot].store(bytes.len(), Ordering::Release);
    true
}

pub(crate) fn uninstall_runtime_rootfs(slot: usize) -> bool {
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
    let Some(bytes) = capacity.checked_mul(core::mem::size_of::<PathIndexEntry>()) else {
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
    table.fill(PathIndexEntry {
        hash: INDEX_EMPTY,
        offset: 0,
        index: 0,
    });
    let mut path = [0_u8; PATH_CAPACITY];
    let mut offset = 0_usize;
    for index in 0..rootfs.entry_count() {
        let entry_offset = offset;
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
    INDEX_STARTS[slot].store(address, Ordering::Relaxed);
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

fn find_indexed<'a>(slot: usize, rootfs: &Rootfs<'a>, path: &[u8]) -> Option<(usize, Entry<'a>)> {
    let table = index_table(slot)?;
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
    let (index, entry) = find_visible(&rootfs, &resolved[..length])?;
    if entry.kind == EntryKind::Hardlink {
        let target = entry.link.strip_prefix(b"/").unwrap_or(entry.link);
        let (index, entry) = lookup_in(&rootfs, target)?;
        let handle = u16::try_from(index.checked_add(ENTRY_HANDLE_BASE)?).ok()?;
        return Some((handle, Some(entry)));
    }
    let handle = u16::try_from(index.checked_add(ENTRY_HANDLE_BASE)?).ok()?;
    Some((handle, Some(entry)))
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
        let entry = rootfs.entry(index)?;
        if entry.kind != EntryKind::Directory {
            return None;
        }
        rootfs.copy_path(index, &mut resolved)?
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
    rootfs()?.copy_path(index, output)
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
    let (offset, entry) = rootfs.entry_indexed(index)?;
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
    let rootfs = rootfs()?;
    let mut parent = [0_u8; PATH_CAPACITY];
    let parent_length = if directory == ROOT_HANDLE {
        0
    } else {
        let index = usize::from(directory).checked_sub(ENTRY_HANDLE_BASE)?;
        rootfs.copy_path(index, &mut parent)?
    };
    let child = rootfs.next_child(&parent[..parent_length], cursor)?;
    Some(child.and_then(|child| {
        let handle = u16::try_from(child.index.checked_add(ENTRY_HANDLE_BASE)?).ok()?;
        Some(Child {
            next_cursor: child.next_cursor,
            handle,
            entry: child.entry,
            name: child.name,
        })
    }))
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
) -> Option<crate::linux_elf::RelocatableProcessPlan<'static, 'static>> {
    process_plan_in(&rootfs()?, init)
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
) -> Option<crate::linux_elf::RelocatableProcessPlan<'a, 'a>> {
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
    let interpreter_path = crate::linux_elf::relocatable_interpreter(executable.data)?;
    let (_, interpreter) = lookup_in(rootfs, interpreter_path)?;
    if interpreter.kind != EntryKind::File || interpreter.mode & 0o111 == 0 {
        return None;
    }
    crate::linux_elf::plan_relocatable_process(
        executable.data,
        0x40_0000,
        interpreter_path,
        interpreter.data,
        0x60_0000,
    )
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
