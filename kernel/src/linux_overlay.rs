use crate::{
    arch::x86_64::cpu,
    control_abi,
    memory::{self, FrameLease, FrameOwner},
};
use core::cell::UnsafeCell;

const PAGE_SIZE: usize = 4096;
pub(crate) const PATH_CAPACITY: usize = 256;
const NODE_COUNT: usize = 10;
const MAPPING_COUNT: usize = 100;
const DATA_CAPACITY: usize = PAGE_SIZE - core::mem::size_of::<FrameLease>();
const XATTR_NAME_CAPACITY: usize = 255;
const HOST_INSTANCE: usize = control_abi::DOMAIN_LIMIT;
const INSTANCE_LIMIT: usize = control_abi::DOMAIN_LIMIT + 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum NodeKind {
    File,
    Directory,
    Symlink,
    Fifo,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Handle {
    generation: u32,
    inode: u64,
}

impl Handle {
    pub(crate) const fn inode(self) -> u64 {
        self.inode
    }
}

#[derive(Clone, Copy)]
pub(crate) struct Metadata {
    pub(crate) handle: Handle,
    pub(crate) kind: NodeKind,
    pub(crate) mode: u32,
    pub(crate) size: usize,
    pub(crate) atime: (i64, i64),
    pub(crate) mtime: (i64, i64),
    pub(crate) ctime: (i64, i64),
}

pub(crate) struct Child {
    pub(crate) next_cursor: usize,
    pub(crate) metadata: Metadata,
    name_length: usize,
    name: [u8; PATH_CAPACITY],
}

impl Child {
    pub(crate) fn name(&self) -> &[u8] {
        &self.name[..self.name_length]
    }
}

#[repr(C)]
struct DataPage {
    next: FrameLease,
    bytes: [u8; DATA_CAPACITY],
}

#[repr(C)]
struct XattrPage {
    previous: FrameLease,
    value_first: FrameLease,
    value_size: u32,
    name_length: u16,
    removed: bool,
    name: [u8; XATTR_NAME_CAPACITY],
}

#[derive(Clone, Copy)]
struct SharedMapping {
    active: bool,
    pid: u32,
    address: u64,
    handle: Handle,
    offset: usize,
}

impl SharedMapping {
    const EMPTY: Self = Self {
        active: false,
        pid: 0,
        address: 0,
        handle: Handle {
            generation: 0,
            inode: 0,
        },
        offset: 0,
    };
}

#[repr(C)]
struct MappingPage {
    next: FrameLease,
    mappings: [SharedMapping; MAPPING_COUNT],
}

#[derive(Clone, Copy)]
struct Node {
    active: bool,
    linked: bool,
    whiteout: bool,
    memfd: bool,
    kind: NodeKind,
    path_length: u16,
    mode: u32,
    seals: u32,
    fifo_socket: u8,
    handle: Handle,
    size: usize,
    first_page: FrameLease,
    last_page: FrameLease,
    page_count: usize,
    cached_page: FrameLease,
    cached_page_index: usize,
    xattr_first: FrameLease,
    atime: (i64, i64),
    mtime: (i64, i64),
    ctime: (i64, i64),
    path: [u8; PATH_CAPACITY],
}

impl Node {
    const EMPTY: Self = Self {
        active: false,
        linked: false,
        whiteout: false,
        memfd: false,
        kind: NodeKind::File,
        path_length: 0,
        mode: 0,
        seals: 0,
        fifo_socket: 0,
        handle: Handle {
            generation: 0,
            inode: 0,
        },
        size: 0,
        first_page: FrameLease::EMPTY,
        last_page: FrameLease::EMPTY,
        page_count: 0,
        cached_page: FrameLease::EMPTY,
        cached_page_index: 0,
        xattr_first: FrameLease::EMPTY,
        atime: (0, 0),
        mtime: (0, 0),
        ctime: (0, 0),
        path: [0; PATH_CAPACITY],
    };

    fn path(&self) -> &[u8] {
        &self.path[..usize::from(self.path_length)]
    }

    fn metadata(&self) -> Metadata {
        Metadata {
            handle: self.handle,
            kind: self.kind,
            mode: self.mode,
            size: self.size,
            atime: self.atime,
            mtime: self.mtime,
            ctime: self.ctime,
        }
    }
}

#[repr(C)]
struct NodePage {
    next: FrameLease,
    nodes: [Node; NODE_COUNT],
}

const _: () = assert!(core::mem::size_of::<DataPage>() == PAGE_SIZE);
const _: () = assert!(core::mem::size_of::<XattrPage>() <= PAGE_SIZE);
const _: () = assert!(core::mem::size_of::<NodePage>() <= PAGE_SIZE);
const _: () = assert!(core::mem::size_of::<MappingPage>() <= PAGE_SIZE);

#[derive(Clone, Copy)]
struct Overlay {
    first_node_page: FrameLease,
    first_mapping_page: FrameLease,
    next_generation: u32,
    next_inode: u64,
}

impl Overlay {
    const EMPTY: Self = Self {
        first_node_page: FrameLease::EMPTY,
        first_mapping_page: FrameLease::EMPTY,
        next_generation: 0x8000_0000,
        next_inode: 0x10_000,
    };
}

pub(crate) fn register_shared_mapping(
    pid: u32,
    address: u64,
    handle: Handle,
    offset: usize,
) -> bool {
    if pid == 0 || !address.is_multiple_of(PAGE_SIZE as u64) || metadata(handle).is_none() {
        return false;
    }
    let overlay = live_mut();
    let mut page = overlay.first_mapping_page;
    let mut last = FrameLease::EMPTY;
    while page != FrameLease::EMPTY {
        let Some(mappings) = mapping_page_mut(page) else {
            return false;
        };
        if mappings
            .mappings
            .iter()
            .any(|mapping| mapping.active && mapping.pid == pid && mapping.address == address)
        {
            return false;
        }
        if let Some(mapping) = mappings.mappings.iter_mut().find(|mapping| !mapping.active) {
            *mapping = SharedMapping {
                active: true,
                pid,
                address,
                handle,
                offset,
            };
            return true;
        }
        last = page;
        page = mappings.next;
    }
    let Some(page) = allocate_mapping_page() else {
        return false;
    };
    if last == FrameLease::EMPTY {
        overlay.first_mapping_page = page;
    } else {
        let Some(previous) = mapping_page_mut(last) else {
            let _ = memory::scrub_and_release_frames(FrameOwner::KERNEL, &[page]);
            return false;
        };
        previous.next = page;
    }
    let Some(mapping) = mapping_page_mut(page).and_then(|page| page.mappings.first_mut()) else {
        return false;
    };
    *mapping = SharedMapping {
        active: true,
        pid,
        address,
        handle,
        offset,
    };
    true
}

pub(crate) fn shared_mapping(pid: u32, address: u64) -> Option<(Handle, usize)> {
    let overlay = live()?;
    let mut page = overlay.first_mapping_page;
    while page != FrameLease::EMPTY {
        let mappings = mapping_page(page)?;
        if let Some(mapping) = mappings
            .mappings
            .iter()
            .find(|mapping| mapping.active && mapping.pid == pid && mapping.address == address)
        {
            return Some((mapping.handle, mapping.offset));
        }
        page = mappings.next;
    }
    None
}

pub(crate) fn next_shared_mapping(pid: u32, after: Option<u64>) -> Option<(u64, Handle, usize)> {
    let overlay = live()?;
    let mut selected: Option<(u64, Handle, usize)> = None;
    let mut page = overlay.first_mapping_page;
    while page != FrameLease::EMPTY {
        let mappings = mapping_page(page)?;
        for mapping in &mappings.mappings {
            if mapping.active
                && mapping.pid == pid
                && after.is_none_or(|after| mapping.address > after)
                && selected.is_none_or(|selected| mapping.address < selected.0)
            {
                selected = Some((mapping.address, mapping.handle, mapping.offset));
            }
        }
        page = mappings.next;
    }
    selected
}

pub(crate) fn remove_shared_mapping(pid: u32, address: u64) -> bool {
    let overlay = live_mut();
    let mut page = overlay.first_mapping_page;
    while page != FrameLease::EMPTY {
        let Some(mappings) = mapping_page_mut(page) else {
            return false;
        };
        if let Some(mapping) = mappings
            .mappings
            .iter_mut()
            .find(|mapping| mapping.active && mapping.pid == pid && mapping.address == address)
        {
            *mapping = SharedMapping::EMPTY;
            return true;
        }
        page = mappings.next;
    }
    false
}

pub(crate) fn clone_shared_mappings(parent: u32, child: u32) -> bool {
    let mut count = 0;
    let Some(overlay) = live() else {
        return false;
    };
    let mut page = overlay.first_mapping_page;
    while page != FrameLease::EMPTY {
        let Some(mappings) = mapping_page(page) else {
            return false;
        };
        count += mappings
            .mappings
            .iter()
            .filter(|mapping| mapping.active && mapping.pid == parent)
            .count();
        page = mappings.next;
    }
    for wanted in 0..count {
        let mut found = 0;
        let mut selected = None;
        let Some(overlay) = live() else {
            return false;
        };
        let mut page = overlay.first_mapping_page;
        while page != FrameLease::EMPTY && selected.is_none() {
            let Some(mappings) = mapping_page(page) else {
                return false;
            };
            for mapping in &mappings.mappings {
                if mapping.active && mapping.pid == parent {
                    if found == wanted {
                        selected = Some(*mapping);
                        break;
                    }
                    found += 1;
                }
            }
            page = mappings.next;
        }
        let Some(mapping) = selected else {
            return false;
        };
        if !register_shared_mapping(child, mapping.address, mapping.handle, mapping.offset) {
            remove_process_shared_mappings(child);
            return false;
        }
    }
    true
}

pub(crate) fn remove_process_shared_mappings(pid: u32) {
    let overlay = live_mut();
    let mut page = overlay.first_mapping_page;
    while page != FrameLease::EMPTY {
        let Some(mappings) = mapping_page_mut(page) else {
            return;
        };
        for mapping in &mut mappings.mappings {
            if mapping.active && mapping.pid == pid {
                *mapping = SharedMapping::EMPTY;
            }
        }
        page = mappings.next;
    }
}

struct OverlayRuntimeOwner(UnsafeCell<[Overlay; INSTANCE_LIMIT]>);

unsafe impl Sync for OverlayRuntimeOwner {}

static OVERLAY_RUNTIME: OverlayRuntimeOwner =
    OverlayRuntimeOwner(UnsafeCell::new([Overlay::EMPTY; INSTANCE_LIMIT]));

pub(crate) fn lookup(path: &[u8]) -> Option<Metadata> {
    valid_path(path)?;
    let overlay = live()?;
    for_each_node(overlay, |node| {
        (node.linked && !node.whiteout && node.path() == path).then_some(node.metadata())
    })
}

pub(crate) fn hidden(path: &[u8]) -> bool {
    valid_path(path).is_some()
        && live().is_some_and(|overlay| {
            for_each_node(overlay, |node| {
                (node.linked
                    && node.whiteout
                    && (node.path() == path
                        || path.starts_with(node.path())
                            && path.get(node.path().len()) == Some(&b'/')))
                .then_some(())
            })
            .is_some()
        })
}

pub(crate) fn from_generation(generation: u32) -> Option<Handle> {
    let overlay = live()?;
    for_each_node(overlay, |node| {
        (node.handle.generation == generation).then_some(node.handle)
    })
}

pub(crate) fn generation(handle: Handle) -> u32 {
    handle.generation
}

pub(crate) fn metadata(handle: Handle) -> Option<Metadata> {
    let overlay = live()?;
    for_each_node(overlay, |node| {
        (node.handle == handle).then_some(node.metadata())
    })
}

pub(crate) fn configure_memfd(handle: Handle, allow_sealing: bool) -> bool {
    let Some(node) = node_mut(handle) else {
        return false;
    };
    if node.kind != NodeKind::File {
        return false;
    }
    node.memfd = true;
    node.seals = if allow_sealing { 0 } else { 1 };
    true
}

pub(crate) fn file_seals(handle: Handle) -> Option<u32> {
    let node = node(handle)?;
    node.memfd.then_some(node.seals)
}

pub(crate) fn add_file_seals(handle: Handle, seals: u32) -> Option<bool> {
    let node = node_mut(handle)?;
    if !node.memfd {
        return None;
    }
    let added = seals & !node.seals;
    if added != 0 && node.seals & 1 != 0 {
        return Some(false);
    }
    node.seals |= seals;
    Some(true)
}

pub(crate) fn copy_path(handle: Handle, output: &mut [u8]) -> Option<usize> {
    let overlay = live()?;
    for_each_node(overlay, |node| {
        if node.handle != handle {
            return None;
        }
        output
            .get_mut(..node.path().len())?
            .copy_from_slice(node.path());
        Some(node.path().len())
    })
}

pub(crate) fn path_hash(handle: Handle) -> Option<u64> {
    let node = node(handle)?;
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in node.path() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100_0000_01b3);
    }
    Some(hash.max(1))
}

pub(crate) fn next_child(directory: Handle, cursor: usize) -> Option<Option<Child>> {
    let mut parent = [0_u8; PATH_CAPACITY];
    let parent_length = copy_path(directory, &mut parent)?;
    let mut prefix_length = parent_length;
    if prefix_length != 0 {
        parent[prefix_length] = b'/';
        prefix_length += 1;
    }
    let mut observed = 0;
    let overlay = live()?;
    Some(for_each_node(overlay, |node| {
        if !node.linked || node.whiteout {
            return None;
        }
        let remainder = node.path().strip_prefix(&parent[..prefix_length])?;
        if remainder.is_empty() || remainder.contains(&b'/') {
            return None;
        }
        if observed != cursor {
            observed += 1;
            return None;
        }
        let mut name = [0_u8; PATH_CAPACITY];
        name[..remainder.len()].copy_from_slice(remainder);
        Some(Child {
            next_cursor: cursor + 1,
            metadata: node.metadata(),
            name_length: remainder.len(),
            name,
        })
    }))
}

pub(crate) fn create_file(path: &[u8], mode: u32, initial: &[u8]) -> Option<Handle> {
    create(path, NodeKind::File, mode, initial)
}

pub(crate) fn create_directory(path: &[u8], mode: u32) -> Option<Handle> {
    create(path, NodeKind::Directory, mode, &[])
}

pub(crate) fn create_symlink(path: &[u8], target: &[u8]) -> Option<Handle> {
    create(path, NodeKind::Symlink, 0o777, target)
}

pub(crate) fn create_fifo(path: &[u8], mode: u32) -> Option<Handle> {
    create(path, NodeKind::Fifo, mode, &[])
}

pub(crate) fn fifo_socket(handle: Handle) -> Option<u8> {
    let node = node(handle)?;
    (node.kind == NodeKind::Fifo && node.fifo_socket != 0).then_some(node.fifo_socket - 1)
}

pub(crate) fn set_fifo_socket(handle: Handle, slot: u8) -> bool {
    let Some(node) = node_mut(handle).filter(|node| node.kind == NodeKind::Fifo) else {
        return false;
    };
    let Some(encoded) = slot.checked_add(1) else {
        return false;
    };
    node.fifo_socket = encoded;
    true
}

fn create(path: &[u8], kind: NodeKind, mode: u32, initial: &[u8]) -> Option<Handle> {
    valid_path(path)?;
    if lookup(path).is_some() {
        return None;
    }
    let replace_whiteout = hidden(path);
    let overlay = live_mut();
    let generation = overlay.next_generation.checked_add(1)?;
    let inode = overlay.next_inode.checked_add(1)?;
    let node = if replace_whiteout {
        node_by_path_mut(overlay, path)?
    } else {
        empty_node_slot(overlay)?
    };
    let mut stored_path = [0_u8; PATH_CAPACITY];
    stored_path[..path.len()].copy_from_slice(path);
    let handle = Handle { generation, inode };
    *node = Node {
        active: true,
        linked: true,
        whiteout: false,
        memfd: false,
        kind,
        path_length: path.len() as u16,
        mode: mode & 0o7777,
        seals: 0,
        fifo_socket: 0,
        handle,
        size: 0,
        first_page: FrameLease::EMPTY,
        last_page: FrameLease::EMPTY,
        page_count: 0,
        cached_page: FrameLease::EMPTY,
        cached_page_index: 0,
        xattr_first: FrameLease::EMPTY,
        atime: (0, 0),
        mtime: (0, 0),
        ctime: (0, 0),
        path: stored_path,
    };
    overlay.next_generation = generation;
    overlay.next_inode = inode;
    if !initial.is_empty() && write(handle, 0, initial) != Some(initial.len()) {
        let _ = unlink(handle);
        return None;
    }
    Some(handle)
}

pub(crate) fn whiteout(path: &[u8]) -> bool {
    if valid_path(path).is_none() {
        return false;
    }
    let has_node = lookup(path).is_some() || hidden(path);
    let overlay = live_mut();
    if has_node {
        let Some(node) = node_by_path_mut(overlay, path) else {
            return false;
        };
        if !release_data_pages(node.first_page) || !release_xattr_pages(node.xattr_first) {
            return false;
        }
        node.first_page = FrameLease::EMPTY;
        node.last_page = FrameLease::EMPTY;
        node.page_count = 0;
        node.cached_page = FrameLease::EMPTY;
        node.xattr_first = FrameLease::EMPTY;
        node.size = 0;
        node.linked = true;
        node.whiteout = true;
        return true;
    }
    let generation = match overlay.next_generation.checked_add(1) {
        Some(generation) => generation,
        None => return false,
    };
    let inode = match overlay.next_inode.checked_add(1) {
        Some(inode) => inode,
        None => return false,
    };
    let Some(node) = empty_node_slot(overlay) else {
        return false;
    };
    let mut stored_path = [0_u8; PATH_CAPACITY];
    stored_path[..path.len()].copy_from_slice(path);
    *node = Node {
        active: true,
        linked: true,
        whiteout: true,
        memfd: false,
        kind: NodeKind::File,
        path_length: path.len() as u16,
        mode: 0,
        seals: 0,
        fifo_socket: 0,
        handle: Handle { generation, inode },
        size: 0,
        first_page: FrameLease::EMPTY,
        last_page: FrameLease::EMPTY,
        page_count: 0,
        cached_page: FrameLease::EMPTY,
        cached_page_index: 0,
        xattr_first: FrameLease::EMPTY,
        atime: (0, 0),
        mtime: (0, 0),
        ctime: (0, 0),
        path: stored_path,
    };
    overlay.next_generation = generation;
    overlay.next_inode = inode;
    true
}

pub(crate) fn read(handle: Handle, offset: usize, output: &mut [u8]) -> Option<usize> {
    let node = node_mut(handle)?;
    if matches!(node.kind, NodeKind::Directory | NodeKind::Fifo) {
        return None;
    }
    let start = offset.min(node.size);
    let end = start.saturating_add(output.len()).min(node.size);
    let target_page = start / DATA_CAPACITY;
    let (mut page, mut page_index) =
        if node.cached_page != FrameLease::EMPTY && node.cached_page_index <= target_page {
            (node.cached_page, node.cached_page_index)
        } else {
            (node.first_page, 0)
        };
    while page_index < target_page {
        page = data_page(page)?.next;
        page_index += 1;
    }
    node.cached_page = page;
    node.cached_page_index = page_index;
    let mut skipped = page_index * DATA_CAPACITY;
    let mut copied = 0;
    while start + copied < end {
        let data = data_page(page)?;
        let page_offset = start + copied - skipped;
        let length = (end - start - copied).min(DATA_CAPACITY - page_offset);
        output[copied..copied + length]
            .copy_from_slice(&data.bytes[page_offset..page_offset + length]);
        copied += length;
        skipped += DATA_CAPACITY;
        page = data.next;
    }
    Some(copied)
}

pub(crate) fn write(handle: Handle, offset: usize, input: &[u8]) -> Option<usize> {
    let end = offset.checked_add(input.len())?;
    if input.is_empty() {
        return Some(0);
    }
    let node = node_mut(handle)?;
    if matches!(node.kind, NodeKind::Directory | NodeKind::Fifo) {
        return None;
    }
    ensure_data_pages(node, end.div_ceil(DATA_CAPACITY))?;
    let target_page = offset / DATA_CAPACITY;
    let (mut page, mut page_index) =
        if node.cached_page != FrameLease::EMPTY && node.cached_page_index <= target_page {
            (node.cached_page, node.cached_page_index)
        } else {
            (node.first_page, 0)
        };
    while page_index < target_page {
        page = data_page(page)?.next;
        page_index += 1;
    }
    node.cached_page = page;
    node.cached_page_index = page_index;
    let mut skipped = page_index * DATA_CAPACITY;
    let mut copied = 0;
    while copied < input.len() {
        let data = data_page_mut(page)?;
        let page_offset = offset + copied - skipped;
        let length = (input.len() - copied).min(DATA_CAPACITY - page_offset);
        data.bytes[page_offset..page_offset + length]
            .copy_from_slice(&input[copied..copied + length]);
        copied += length;
        skipped += DATA_CAPACITY;
        page = data.next;
    }
    node.size = node.size.max(end);
    Some(input.len())
}

pub(crate) fn truncate(handle: Handle, size: usize) -> bool {
    let Some(node) = node_mut(handle) else {
        return false;
    };
    if node.kind != NodeKind::File {
        return false;
    }
    if size > node.size {
        if ensure_data_pages(node, size.div_ceil(DATA_CAPACITY)).is_none() {
            return false;
        }
    } else if size == 0 {
        let first = core::mem::replace(&mut node.first_page, FrameLease::EMPTY);
        if !release_data_pages(first) {
            node.first_page = first;
            return false;
        }
        node.last_page = FrameLease::EMPTY;
        node.page_count = 0;
        node.cached_page = FrameLease::EMPTY;
        node.cached_page_index = 0;
    }
    node.size = size;
    true
}

pub(crate) fn set_mode(handle: Handle, mode: u32) -> bool {
    let Some(node) = node_mut(handle) else {
        return false;
    };
    node.mode = mode & 0o7777;
    true
}

pub(crate) fn set_times(handle: Handle, atime: (i64, i64), mtime: (i64, i64)) -> bool {
    let Some(node) = node_mut(handle) else {
        return false;
    };
    node.atime = atime;
    node.mtime = mtime;
    node.ctime = mtime;
    true
}

pub(crate) fn set_xattr_from<F>(
    handle: Handle,
    name: &[u8],
    value_size: usize,
    create_only: bool,
    replace_only: bool,
    mut fill: F,
) -> bool
where
    F: FnMut(usize, &mut [u8]) -> bool,
{
    if name.is_empty()
        || name.len() > XATTR_NAME_CAPACITY
        || name.contains(&0)
        || u32::try_from(value_size).is_err()
    {
        return false;
    }
    let exists = xattr_metadata(handle, name).is_some_and(|(_, removed)| !removed);
    if create_only && exists || replace_only && !exists {
        return false;
    }
    let mut offset = 0;
    let value_first = match allocate_data_pages_with(value_size, |bytes| {
        let accepted = fill(offset, bytes);
        offset += bytes.len();
        accepted
    }) {
        Some(first) => first,
        None => return false,
    };
    append_xattr_pages(handle, name, value_first, value_size, false)
}

pub(crate) fn get_xattr_chunk(
    handle: Handle,
    name: &[u8],
    offset: usize,
    output: &mut [u8],
) -> Option<(usize, usize)> {
    let (page, removed) = xattr_metadata(handle, name)?;
    if removed {
        return None;
    }
    let record = xattr_page(page)?;
    let size = record.value_size as usize;
    if offset >= size || output.is_empty() {
        return Some((size, 0));
    }
    let mut value_page = record.value_first;
    let mut page_index = offset / DATA_CAPACITY;
    while page_index != 0 {
        value_page = data_page(value_page)?.next;
        page_index -= 1;
    }
    let mut within_page = offset % DATA_CAPACITY;
    let mut copied = 0;
    let wanted = output.len().min(size - offset);
    while copied < wanted {
        let data = data_page(value_page)?;
        let length = (wanted - copied).min(DATA_CAPACITY - within_page);
        output[copied..copied + length]
            .copy_from_slice(&data.bytes[within_page..within_page + length]);
        copied += length;
        within_page = 0;
        value_page = data.next;
    }
    Some((size, copied))
}

pub(crate) fn list_xattrs_chunk(
    handle: Handle,
    offset: usize,
    output: &mut [u8],
) -> Option<(usize, usize)> {
    let node = node(handle)?;
    let mut current = node.xattr_first;
    let mut used: usize = 0;
    let mut copied: usize = 0;
    while current != FrameLease::EMPTY {
        let record = xattr_page(current)?;
        if !record.removed && !xattr_superseded(node.xattr_first, current, record.name())? {
            let length = usize::from(record.name_length).checked_add(1)?;
            let end = used.checked_add(length)?;
            let copy_start = offset.max(used);
            let copy_end = end.min(offset.saturating_add(output.len()));
            if copy_start < copy_end {
                for source_index in copy_start - used..copy_end - used {
                    output[copied] = if source_index < record.name().len() {
                        record.name()[source_index]
                    } else {
                        0
                    };
                    copied += 1;
                }
            }
            used = end;
        }
        current = record.previous;
    }
    Some((used, copied))
}

pub(crate) fn remove_xattr(handle: Handle, name: &[u8]) -> bool {
    if !xattr_metadata(handle, name).is_some_and(|(_, removed)| !removed) {
        return false;
    }
    append_xattr(handle, name, &[], true)
}

pub(crate) fn rename(handle: Handle, path: &[u8]) -> bool {
    if valid_path(path).is_none() || lookup(path).is_some() {
        return false;
    }
    let Some(source) = node(handle) else {
        return false;
    };
    if !source.linked {
        return false;
    }
    let mut old_path = [0_u8; PATH_CAPACITY];
    let old_length = source.path().len();
    old_path[..old_length].copy_from_slice(source.path());
    let directory = source.kind == NodeKind::Directory;
    let Some(overlay) = live() else {
        return false;
    };
    let valid = for_each_node(overlay, |candidate| {
        if !candidate.linked
            || candidate.handle == handle
            || !directory
            || !candidate.path().starts_with(&old_path[..old_length])
            || candidate.path().get(old_length) != Some(&b'/')
        {
            return None;
        }
        let suffix = &candidate.path()[old_length..];
        let Some(new_length) = path.len().checked_add(suffix.len()) else {
            return Some(false);
        };
        if new_length >= PATH_CAPACITY {
            return Some(false);
        }
        let mut destination = [0_u8; PATH_CAPACITY];
        destination[..path.len()].copy_from_slice(path);
        destination[path.len()..new_length].copy_from_slice(suffix);
        if lookup(&destination[..new_length]).is_some() {
            return Some(false);
        }
        None
    });
    if valid == Some(false) {
        return false;
    }
    let overlay = live_mut();
    let mut page = overlay.first_node_page;
    while page != FrameLease::EMPTY {
        let Some(nodes) = node_page_mut(page) else {
            return false;
        };
        for candidate in &mut nodes.nodes {
            if !candidate.active || !candidate.linked {
                continue;
            }
            let suffix = if candidate.handle == handle {
                &[][..]
            } else if directory
                && candidate.path().starts_with(&old_path[..old_length])
                && candidate.path().get(old_length) == Some(&b'/')
            {
                &candidate.path()[old_length..]
            } else {
                continue;
            };
            let new_length = path.len() + suffix.len();
            let mut destination = [0_u8; PATH_CAPACITY];
            destination[..path.len()].copy_from_slice(path);
            destination[path.len()..new_length].copy_from_slice(suffix);
            candidate.path = destination;
            candidate.path_length = new_length as u16;
        }
        page = nodes.next;
    }
    true
}

pub(crate) fn unlink(handle: Handle) -> bool {
    let Some(metadata) = metadata(handle) else {
        return false;
    };
    if metadata.kind == NodeKind::Directory {
        let mut prefix = [0_u8; PATH_CAPACITY];
        let Some(length) = copy_path(handle, &mut prefix) else {
            return false;
        };
        prefix[length] = b'/';
        if for_each_node(live().unwrap_or(&Overlay::EMPTY), |candidate| {
            (candidate.linked
                && !candidate.whiteout
                && candidate.path().starts_with(&prefix[..length + 1]))
            .then_some(())
        })
        .is_some()
        {
            return false;
        }
    }
    let Some(node) = node_mut(handle) else {
        return false;
    };
    node.linked = false;
    true
}

pub(crate) fn teardown() -> bool {
    let overlay = live_mut();
    let mut page = core::mem::replace(&mut overlay.first_node_page, FrameLease::EMPTY);
    while page != FrameLease::EMPTY {
        let Some(nodes) = node_page_mut(page) else {
            return false;
        };
        let next = nodes.next;
        for node in &mut nodes.nodes {
            if node.active
                && (!release_data_pages(node.first_page) || !release_xattr_pages(node.xattr_first))
            {
                return false;
            }
            *node = Node::EMPTY;
        }
        if !memory::scrub_and_release_frames(FrameOwner::KERNEL, core::slice::from_ref(&page)) {
            return false;
        }
        page = next;
    }
    let mut page = core::mem::replace(&mut overlay.first_mapping_page, FrameLease::EMPTY);
    while page != FrameLease::EMPTY {
        let Some(mappings) = mapping_page(page) else {
            return false;
        };
        let next = mappings.next;
        if !memory::scrub_and_release_frames(FrameOwner::KERNEL, core::slice::from_ref(&page)) {
            return false;
        }
        page = next;
    }
    overlay.next_generation = 0x8000_0000;
    overlay.next_inode = 0x10_000;
    true
}

fn valid_path(path: &[u8]) -> Option<()> {
    (!path.is_empty()
        && path.len() < PATH_CAPACITY
        && !path.starts_with(b"/")
        && !path.ends_with(b"/")
        && path.split(|byte| *byte == b'/').all(|component| {
            !component.is_empty()
                && component != b"."
                && component != b".."
                && !component.contains(&0)
        }))
    .then_some(())
}

fn node_by_path_mut<'a>(overlay: &'a mut Overlay, path: &[u8]) -> Option<&'a mut Node> {
    let mut page = overlay.first_node_page;
    while page != FrameLease::EMPTY {
        let nodes = node_page_mut(page)?;
        if let Some(index) = nodes
            .nodes
            .iter()
            .position(|node| node.active && node.linked && node.path() == path)
        {
            return nodes.nodes.get_mut(index);
        }
        page = nodes.next;
    }
    None
}

fn empty_node_slot(overlay: &mut Overlay) -> Option<&mut Node> {
    let mut page = overlay.first_node_page;
    let mut last = FrameLease::EMPTY;
    while page != FrameLease::EMPTY {
        let nodes = node_page_mut(page)?;
        if let Some(index) = nodes.nodes.iter().position(|node| !node.active) {
            return nodes.nodes.get_mut(index);
        }
        last = page;
        page = nodes.next;
    }
    let page = allocate_node_page()?;
    if last == FrameLease::EMPTY {
        overlay.first_node_page = page;
    } else {
        node_page_mut(last)?.next = page;
    }
    node_page_mut(page)?.nodes.first_mut()
}

fn allocate_node_page() -> Option<FrameLease> {
    let [page] = memory::allocate_frames::<1>(FrameOwner::KERNEL)?;
    let address = page.direct_map_address()?;
    unsafe {
        core::ptr::write_bytes(address as *mut u8, 0, PAGE_SIZE);
        core::ptr::write(
            address as *mut NodePage,
            NodePage {
                next: FrameLease::EMPTY,
                nodes: [Node::EMPTY; NODE_COUNT],
            },
        );
    }
    Some(page)
}

fn allocate_mapping_page() -> Option<FrameLease> {
    let [page] = memory::allocate_frames::<1>(FrameOwner::KERNEL)?;
    let address = page.direct_map_address()?;
    unsafe {
        core::ptr::write_bytes(address as *mut u8, 0, PAGE_SIZE);
        core::ptr::write(
            address as *mut MappingPage,
            MappingPage {
                next: FrameLease::EMPTY,
                mappings: [SharedMapping::EMPTY; MAPPING_COUNT],
            },
        );
    }
    Some(page)
}

fn allocate_data_page() -> Option<FrameLease> {
    let [page] = memory::allocate_frames::<1>(FrameOwner::KERNEL)?;
    let address = page.direct_map_address()?;
    unsafe {
        core::ptr::write_bytes(address as *mut u8, 0, PAGE_SIZE);
        core::ptr::write(
            address as *mut DataPage,
            DataPage {
                next: FrameLease::EMPTY,
                bytes: [0; DATA_CAPACITY],
            },
        );
    }
    Some(page)
}

impl XattrPage {
    fn name(&self) -> &[u8] {
        &self.name[..usize::from(self.name_length)]
    }
}

fn append_xattr(handle: Handle, name: &[u8], value: &[u8], removed: bool) -> bool {
    let value_first = if value.is_empty() {
        FrameLease::EMPTY
    } else {
        let Some(first) = allocate_data_pages(value) else {
            return false;
        };
        first
    };
    append_xattr_pages(handle, name, value_first, value.len(), removed)
}

fn append_xattr_pages(
    handle: Handle,
    name: &[u8],
    value_first: FrameLease,
    value_size: usize,
    removed: bool,
) -> bool {
    let Some([page]) = memory::allocate_frames::<1>(FrameOwner::KERNEL) else {
        let _ = release_data_pages(value_first);
        return false;
    };
    let Some(address) = page.direct_map_address() else {
        let _ = release_data_pages(value_first);
        let _ = memory::scrub_and_release_frames(FrameOwner::KERNEL, &[page]);
        return false;
    };
    let Some(node) = node_mut(handle) else {
        let _ = release_data_pages(value_first);
        let _ = memory::scrub_and_release_frames(FrameOwner::KERNEL, &[page]);
        return false;
    };
    let mut stored_name = [0_u8; XATTR_NAME_CAPACITY];
    stored_name[..name.len()].copy_from_slice(name);
    unsafe {
        core::ptr::write_bytes(address as *mut u8, 0, PAGE_SIZE);
        core::ptr::write(
            address as *mut XattrPage,
            XattrPage {
                previous: node.xattr_first,
                value_first,
                value_size: value_size as u32,
                name_length: name.len() as u16,
                removed,
                name: stored_name,
            },
        );
    }
    node.xattr_first = page;
    true
}

fn allocate_data_pages(input: &[u8]) -> Option<FrameLease> {
    let mut offset = 0;
    allocate_data_pages_with(input.len(), |output| {
        output.copy_from_slice(&input[offset..offset + output.len()]);
        offset += output.len();
        true
    })
}

fn allocate_data_pages_with<F>(size: usize, mut fill: F) -> Option<FrameLease>
where
    F: FnMut(&mut [u8]) -> bool,
{
    let mut first = FrameLease::EMPTY;
    let mut last = FrameLease::EMPTY;
    let mut remaining = size;
    while remaining != 0 {
        let Some(page) = allocate_data_page() else {
            let _ = release_data_pages(first);
            return None;
        };
        let Some(data) = data_page_mut(page) else {
            let _ = memory::scrub_and_release_frames(FrameOwner::KERNEL, &[page]);
            let _ = release_data_pages(first);
            return None;
        };
        let length = remaining.min(DATA_CAPACITY);
        if !fill(&mut data.bytes[..length]) {
            let _ = memory::scrub_and_release_frames(FrameOwner::KERNEL, &[page]);
            let _ = release_data_pages(first);
            return None;
        }
        if first == FrameLease::EMPTY {
            first = page;
        } else {
            let Some(tail) = data_page_mut(last) else {
                let _ = memory::scrub_and_release_frames(FrameOwner::KERNEL, &[page]);
                let _ = release_data_pages(first);
                return None;
            };
            tail.next = page;
        }
        last = page;
        remaining -= length;
    }
    Some(first)
}

fn xattr_metadata(handle: Handle, name: &[u8]) -> Option<(FrameLease, bool)> {
    let node = node(handle)?;
    let mut page = node.xattr_first;
    while page != FrameLease::EMPTY {
        let record = xattr_page(page)?;
        if record.name() == name {
            return Some((page, record.removed));
        }
        page = record.previous;
    }
    None
}

fn xattr_superseded(first: FrameLease, target: FrameLease, name: &[u8]) -> Option<bool> {
    let mut page = first;
    while page != target {
        let record = xattr_page(page)?;
        if record.name() == name {
            return Some(true);
        }
        page = record.previous;
    }
    Some(false)
}

fn release_xattr_pages(mut page: FrameLease) -> bool {
    while page != FrameLease::EMPTY {
        let Some(record) = xattr_page(page) else {
            return false;
        };
        let previous = record.previous;
        if !release_data_pages(record.value_first)
            || !memory::scrub_and_release_frames(FrameOwner::KERNEL, &[page])
        {
            return false;
        }
        page = previous;
    }
    true
}

fn ensure_data_pages(node: &mut Node, required: usize) -> Option<()> {
    while node.page_count < required {
        let next = allocate_data_page()?;
        if node.first_page == FrameLease::EMPTY {
            node.first_page = next;
            node.cached_page = next;
            node.cached_page_index = 0;
        } else {
            data_page_mut(node.last_page)?.next = next;
        }
        node.last_page = next;
        node.page_count += 1;
    }
    Some(())
}

fn release_data_pages(mut page: FrameLease) -> bool {
    while page != FrameLease::EMPTY {
        let Some(data) = data_page(page) else {
            return false;
        };
        let next = data.next;
        if !memory::scrub_and_release_frames(FrameOwner::KERNEL, core::slice::from_ref(&page)) {
            return false;
        }
        page = next;
    }
    true
}

fn node(handle: Handle) -> Option<&'static Node> {
    let overlay = live()?;
    for_each_node(overlay, |node| (node.handle == handle).then_some(node))
}

fn node_mut(handle: Handle) -> Option<&'static mut Node> {
    let overlay = live_mut();
    let mut page = overlay.first_node_page;
    while page != FrameLease::EMPTY {
        let nodes = node_page_mut(page)?;
        if let Some(index) = nodes
            .nodes
            .iter()
            .position(|node| node.active && node.handle == handle)
        {
            return nodes.nodes.get_mut(index);
        }
        page = nodes.next;
    }
    None
}

fn for_each_node<T>(
    overlay: &Overlay,
    mut select: impl FnMut(&'static Node) -> Option<T>,
) -> Option<T> {
    let mut page = overlay.first_node_page;
    while page != FrameLease::EMPTY {
        let nodes = node_page(page)?;
        for node in &nodes.nodes {
            if node.active {
                if let Some(value) = select(node) {
                    return Some(value);
                }
            }
        }
        page = nodes.next;
    }
    None
}

fn node_page(page: FrameLease) -> Option<&'static NodePage> {
    let address = page.direct_map_address()?;
    Some(unsafe { &*(address as *const NodePage) })
}

fn node_page_mut(page: FrameLease) -> Option<&'static mut NodePage> {
    let address = page.direct_map_address()?;
    Some(unsafe { &mut *(address as *mut NodePage) })
}

fn mapping_page(page: FrameLease) -> Option<&'static MappingPage> {
    (page != FrameLease::EMPTY).then_some(())?;
    let address = page.direct_map_address()?;
    Some(unsafe { &*(address as *const MappingPage) })
}

fn mapping_page_mut(page: FrameLease) -> Option<&'static mut MappingPage> {
    (page != FrameLease::EMPTY).then_some(())?;
    let address = page.direct_map_address()?;
    Some(unsafe { &mut *(address as *mut MappingPage) })
}

fn data_page(page: FrameLease) -> Option<&'static DataPage> {
    (page != FrameLease::EMPTY).then_some(())?;
    let address = page.direct_map_address()?;
    Some(unsafe { &*(address as *const DataPage) })
}

fn data_page_mut(page: FrameLease) -> Option<&'static mut DataPage> {
    (page != FrameLease::EMPTY).then_some(())?;
    let address = page.direct_map_address()?;
    Some(unsafe { &mut *(address as *mut DataPage) })
}

fn xattr_page(page: FrameLease) -> Option<&'static XattrPage> {
    (page != FrameLease::EMPTY).then_some(())?;
    let address = page.direct_map_address()?;
    Some(unsafe { &*(address as *const XattrPage) })
}

fn live() -> Option<&'static Overlay> {
    let index = current_instance();
    Some(unsafe { &*OVERLAY_RUNTIME.0.get().cast::<Overlay>().add(index) })
}

fn live_mut() -> &'static mut Overlay {
    let index = current_instance();
    unsafe { &mut *OVERLAY_RUNTIME.0.get().cast::<Overlay>().add(index) }
}

fn current_instance() -> usize {
    cpu::current_container_slot().unwrap_or(HOST_INSTANCE)
}
