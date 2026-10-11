use crate::memory;
use core::mem::MaybeUninit;
use core::sync::atomic::{AtomicBool, Ordering};

const MAGIC: &[u8; 6] = b"070701";
const HEADER_SIZE: usize = 110;
const TRAILER: &[u8] = b"TRAILER!!!";
const MAX_ARCHIVE_SIZE: usize = 16 * 1024 * 1024;
const MAX_ENTRIES: usize = 256;
const MAX_NAME_SIZE: usize = 256;
const MAX_FILE_SIZE: usize = 8 * 1024 * 1024;
const PAGE_SIZE: usize = 4096;
const MAX_INDEXED_ENTRIES: usize = 16;
const ROOT_FRAME_COUNT: usize = 32;
const ROOT_CAPACITY: usize = ROOT_FRAME_COUNT * PAGE_SIZE;
const TYPE_MASK: u32 = 0o170000;
const DIRECTORY: u32 = 0o040000;
const REGULAR: u32 = 0o100000;
const SYMLINK: u32 = 0o120000;

static ROOT_PUBLISHED: AtomicBool = AtomicBool::new(false);
static mut PUBLISHED_ROOT: MaybeUninit<ReadOnlyRoot> = MaybeUninit::uninit();

#[derive(Clone, Copy)]
pub(crate) struct Entry<'a> {
    pub(crate) name: &'a [u8],
    pub(crate) data: &'a [u8],
    pub(crate) mode: u32,
}

pub(crate) struct Archive<'a> {
    bytes: &'a [u8],
    entry_count: usize,
}

impl<'a> Archive<'a> {
    pub(crate) const fn entry_count(&self) -> usize {
        self.entry_count
    }

    pub(crate) fn find(&self, wanted: &[u8]) -> Option<Entry<'a>> {
        let mut offset = 0;
        while let Some((entry, next, trailer)) = parse_entry(self.bytes, offset) {
            if trailer {
                return None;
            }
            if entry.name == wanted {
                return Some(entry);
            }
            offset = next;
        }
        None
    }
}

pub(crate) fn parse(bytes: &[u8]) -> Option<Archive<'_>> {
    if bytes.is_empty() || bytes.len() > MAX_ARCHIVE_SIZE {
        return None;
    }
    let mut offset = 0;
    let mut entry_count = 0;
    let mut init_found = false;
    loop {
        let (entry, next, trailer) = parse_entry(bytes, offset)?;
        if trailer {
            if next != bytes.len() || !init_found || entry_count == 0 {
                return None;
            }
            return Some(Archive { bytes, entry_count });
        }
        entry_count = entry_count.checked_add(1)?;
        if entry_count > MAX_ENTRIES || duplicate_name(bytes, offset, entry.name) {
            return None;
        }
        if entry.name == b"init" {
            if init_found
                || entry.mode & TYPE_MASK != REGULAR
                || entry.mode & 0o111 == 0
                || entry.data.is_empty()
            {
                return None;
            }
            init_found = true;
        }
        offset = next;
    }
}

fn parse_entry(bytes: &[u8], offset: usize) -> Option<(Entry<'_>, usize, bool)> {
    let header = bytes.get(offset..offset.checked_add(HEADER_SIZE)?)?;
    if header.get(..6)? != MAGIC {
        return None;
    }
    let inode = read_hex_u32(header, 6)?;
    let mode = read_hex_u32(header, 14)?;
    let uid = read_hex_u32(header, 22)?;
    let gid = read_hex_u32(header, 30)?;
    let links = read_hex_u32(header, 38)?;
    let modified = read_hex_u32(header, 46)?;
    let file_size = usize::try_from(read_hex_u32(header, 54)?).ok()?;
    let device_major = read_hex_u32(header, 62)?;
    let device_minor = read_hex_u32(header, 70)?;
    let special_major = read_hex_u32(header, 78)?;
    let special_minor = read_hex_u32(header, 86)?;
    let name_size = usize::try_from(read_hex_u32(header, 94)?).ok()?;
    let checksum = read_hex_u32(header, 102)?;
    if inode == 0
        || !(2..=MAX_NAME_SIZE).contains(&name_size)
        || file_size > MAX_FILE_SIZE
        || uid != 0
        || gid != 0
        || links != 1
        || modified != 0
        || device_major != 0
        || device_minor != 0
        || special_major != 0
        || special_minor != 0
        || checksum != 0
    {
        return None;
    }
    let name_start = offset.checked_add(HEADER_SIZE)?;
    let name_end = name_start.checked_add(name_size)?;
    let encoded_name = bytes.get(name_start..name_end)?;
    if encoded_name.last() != Some(&0) || encoded_name[..name_size - 1].contains(&0) {
        return None;
    }
    let name = &encoded_name[..name_size - 1];
    let data_start = align4(name_end)?;
    if bytes
        .get(name_end..data_start)?
        .iter()
        .any(|byte| *byte != 0)
    {
        return None;
    }
    let data_end = data_start.checked_add(file_size)?;
    let next = align4(data_end)?;
    let data = bytes.get(data_start..data_end)?;
    if bytes.get(data_end..next)?.iter().any(|byte| *byte != 0) {
        return None;
    }
    let trailer = name == TRAILER;
    if trailer {
        if mode != 0 || file_size != 0 {
            return None;
        }
    } else if !valid_name(name)
        || !matches!(mode & TYPE_MASK, DIRECTORY | REGULAR | SYMLINK)
        || (mode & TYPE_MASK == DIRECTORY && file_size != 0)
        || (mode & TYPE_MASK == SYMLINK && file_size == 0)
    {
        return None;
    }
    Some((Entry { name, data, mode }, next, trailer))
}

fn duplicate_name(bytes: &[u8], current: usize, name: &[u8]) -> bool {
    let mut offset = 0;
    while offset < current {
        let Some((entry, next, trailer)) = parse_entry(bytes, offset) else {
            return true;
        };
        if trailer || entry.name == name {
            return true;
        }
        offset = next;
    }
    offset != current
}

fn valid_name(name: &[u8]) -> bool {
    if name.is_empty() || name[0] == b'/' || name.last() == Some(&b'/') {
        return false;
    }
    let mut component_start = 0;
    for (index, byte) in name.iter().copied().enumerate() {
        if !(byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b'/')) {
            return false;
        }
        if byte == b'/' {
            if invalid_component(&name[component_start..index]) {
                return false;
            }
            component_start = index + 1;
        }
    }
    !invalid_component(&name[component_start..])
}

fn invalid_component(component: &[u8]) -> bool {
    component.is_empty() || component == b"." || component == b".."
}

fn align4(value: usize) -> Option<usize> {
    value.checked_add(3).map(|value| value & !3)
}

fn read_hex_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    let mut value = 0_u32;
    for byte in bytes.get(offset..offset.checked_add(8)?)? {
        value = value.checked_mul(16)?;
        value = value.checked_add(u32::from(match byte {
            b'0'..=b'9' => *byte - b'0',
            b'a'..=b'f' => *byte - b'a' + 10,
            _ => return None,
        }))?;
    }
    Some(value)
}

pub(crate) fn verify_pinned_image(bytes: &[u8]) -> bool {
    let Some(archive) = parse(bytes) else {
        return false;
    };
    archive.entry_count() == 14
        && archive.find(b"init").is_some_and(|entry| {
            entry.mode == 0o100555 && entry.data == b"GINUX_INITRAMFS_INIT_V0\n"
        })
        && [b"dev" as &[u8], b"etc", b"proc", b"sys", b"tmp", b"var"]
            .iter()
            .all(|name| {
                archive
                    .find(name)
                    .is_some_and(|entry| entry.mode & TYPE_MASK == DIRECTORY)
            })
        && archive
            .find(b"etc/hostname")
            .is_some_and(|entry| entry.mode == 0o100555 && entry.data == b"ginux\n")
        && archive
            .find(b"etc/os-release")
            .is_some_and(|entry| entry.mode == 0o100555 && entry.data == b"NAME=Ginux\nID=ginux\n")
        && archive
            .find(b"etc/motd")
            .is_some_and(|entry| entry.mode == 0o100555 && entry.data == b"Welcome to Ginux\n")
        && archive.find(b"linux/init").is_some_and(|entry| {
            entry.mode == 0o100555 && entry.data == crate::linux_elf::runtime_main_bytes()
        })
        && archive.find(b"linux/ld-ginux.so").is_some_and(|entry| {
            entry.mode == 0o100555 && entry.data == crate::linux_elf::runtime_interpreter_bytes()
        })
        && archive.find(b"linux/libginux.so").is_some_and(|entry| {
            entry.mode == 0o100555 && entry.data == crate::linux_elf::runtime_dependency_bytes()
        })
        && archive.find(b"linux/libginux-aux.so").is_some_and(|entry| {
            entry.mode == 0o100555
                && entry.data == crate::linux_elf::runtime_auxiliary_dependency_bytes()
        })
        && archive.find(b"missing").is_none()
}

fn mutation_rejected(
    candidate: &mut [u8],
    source: &[u8],
    offset: usize,
    replacement: &[u8],
) -> Option<bool> {
    let end = offset.checked_add(replacement.len())?;
    candidate.get_mut(offset..end)?.copy_from_slice(replacement);
    let rejected = parse(candidate).is_none();
    candidate
        .get_mut(offset..end)?
        .copy_from_slice(source.get(offset..end)?);
    Some(rejected)
}

pub(crate) fn verify_rejections(bytes: &[u8]) -> bool {
    if bytes.is_empty() || bytes.len() > ROOT_CAPACITY {
        return false;
    }
    let Some(init) = header_offset(bytes, b"init") else {
        return false;
    };
    let Some(etc) = name_offset(bytes, b"etc") else {
        return false;
    };
    let Some(proc_name) = name_offset(bytes, b"proc") else {
        return false;
    };
    let Some(init_name) = name_offset(bytes, b"init") else {
        return false;
    };
    let Some(trailer) = header_offset(bytes, TRAILER) else {
        return false;
    };
    let Some(mut image) = OwnedRoot::materialize(bytes) else {
        return false;
    };
    let rejected = (|| {
        let candidate = image.bytes_mut()?;
        if !mutation_rejected(candidate, bytes, 0, b"1")?
            || !mutation_rejected(candidate, bytes, 6, b"G")?
        {
            return Some(false);
        }
        for (offset, replacement) in [
            (init + 14, &b"00008124"[..]),
            (etc, &b"dev"[..]),
            (etc, &b"../"[..]),
            (init + 54, &b"ffffffff"[..]),
            (init_name, &b"inix"[..]),
        ] {
            if !mutation_rejected(candidate, bytes, offset, replacement)? {
                return Some(false);
            }
        }
        Some(
            mutation_rejected(candidate, bytes, proc_name + 5, &[1])?
                && mutation_rejected(candidate, bytes, trailer + HEADER_SIZE, b"X")?
                && parse(&bytes[..bytes.len() - 1]).is_none(),
        )
    })()
    .unwrap_or(false);
    rejected && image.release()
}

fn header_offset(bytes: &[u8], wanted: &[u8]) -> Option<usize> {
    let mut offset = 0;
    loop {
        let (entry, next, trailer) = parse_entry(bytes, offset)?;
        if entry.name == wanted {
            return Some(offset);
        }
        if trailer {
            return None;
        }
        offset = next;
    }
}

fn name_offset(bytes: &[u8], wanted: &[u8]) -> Option<usize> {
    header_offset(bytes, wanted)?.checked_add(HEADER_SIZE)
}

struct OwnedRoot {
    frames: [memory::FrameLease; ROOT_FRAME_COUNT],
    size: usize,
}

impl OwnedRoot {
    fn materialize(bytes: &[u8]) -> Option<Self> {
        if bytes.is_empty() || bytes.len() > ROOT_CAPACITY || parse(bytes).is_none() {
            return None;
        }
        let frames =
            memory::allocate_contiguous_frames::<ROOT_FRAME_COUNT>(memory::FrameOwner::KERNEL)?;
        let Some(address) = frames[0].direct_map_address() else {
            let _ = memory::release_frames(memory::FrameOwner::KERNEL, frames);
            return None;
        };
        if frames.iter().enumerate().any(|(index, frame)| {
            frame.address() != frames[0].address() + index as u64 * PAGE_SIZE as u64
                || frame.direct_map_address() != Some(address + index * PAGE_SIZE)
        }) {
            let _ = memory::release_frames(memory::FrameOwner::KERNEL, frames);
            return None;
        }
        // SAFETY: the allocator returned a private zeroed frame owned by the
        // kernel. The source was measured and is disjoint from this new lease.
        unsafe {
            core::ptr::copy_nonoverlapping(bytes.as_ptr(), address as *mut u8, bytes.len());
        }
        Some(Self {
            frames,
            size: bytes.len(),
        })
    }

    fn bytes(&self) -> Option<&[u8]> {
        let address = self.frames[0].direct_map_address()?;
        // SAFETY: these contiguous leases remain live and kernel-owned for
        // `self`'s lifetime, and materialization validated their direct map.
        Some(unsafe { core::slice::from_raw_parts(address as *const u8, self.size) })
    }

    fn bytes_mut(&mut self) -> Option<&mut [u8]> {
        let address = self.frames[0].direct_map_address()?;
        // SAFETY: `&mut self` uniquely borrows every live frame in the owned
        // contiguous allocation.
        Some(unsafe { core::slice::from_raw_parts_mut(address as *mut u8, self.size) })
    }

    fn tail_is_zero(&self) -> bool {
        let Some(address) = self.frames[0].direct_map_address() else {
            return false;
        };
        // SAFETY: the complete contiguous allocation is reachable through its
        // validated direct-map aliases.
        unsafe {
            core::slice::from_raw_parts(
                (address + self.size) as *const u8,
                ROOT_CAPACITY - self.size,
            )
            .iter()
            .all(|byte| *byte == 0)
        }
    }

    fn release(self) -> bool {
        let Some(address) = self.frames[0].direct_map_address() else {
            return false;
        };
        // SAFETY: the leases are still live, contiguous, and exclusively
        // kernel-owned.
        unsafe {
            core::ptr::write_bytes(address as *mut u8, 0, ROOT_CAPACITY);
        }
        let scrubbed = unsafe {
            core::slice::from_raw_parts(address as *const u8, ROOT_CAPACITY)
                .iter()
                .all(|byte| *byte == 0)
        };
        scrubbed && memory::release_frames(memory::FrameOwner::KERNEL, self.frames)
    }
}

pub(crate) fn verify_owned_materialization(source: &[u8]) -> bool {
    let Some(before) = memory::owner_live_frame_count(memory::FrameOwner::KERNEL) else {
        return false;
    };
    let Some(root) = OwnedRoot::materialize(source) else {
        return false;
    };
    let Some(address) = root.frames[0].direct_map_address() else {
        let _ = root.release();
        return false;
    };
    let source_start = source.as_ptr() as usize;
    let source_end = source_start.checked_add(source.len());
    let destination_end = address.checked_add(root.size);
    let disjoint = source_end.is_some_and(|end| end <= address)
        || destination_end.is_some_and(|end| end <= source_start);
    let exact = root.bytes() == Some(source);
    let owned = memory::frames_owned_by(memory::FrameOwner::KERNEL, &root.frames);
    let tail_zero = root.tail_is_zero();
    let source_first = source[0];
    // SAFETY: this is a deliberate private-copy mutation before publication.
    unsafe {
        (address as *mut u8).write(source_first ^ 1);
    }
    let independent =
        source[0] == source_first && root.bytes().is_some_and(|bytes| bytes != source);
    // SAFETY: restore the measured byte before validating and releasing the copy.
    unsafe {
        (address as *mut u8).write(source_first);
    }
    let restored = root.bytes() == Some(source);
    let released = root.release();
    disjoint
        && exact
        && owned
        && tail_zero
        && independent
        && restored
        && released
        && memory::owner_live_frame_count(memory::FrameOwner::KERNEL) == Some(before)
}

#[derive(Clone, Copy)]
struct RootNode {
    name_offset: u32,
    name_size: u32,
    data_offset: u32,
    data_size: u32,
    mode: u32,
}

const EMPTY_ROOT_NODE: RootNode = RootNode {
    name_offset: 0,
    name_size: 0,
    data_offset: 0,
    data_size: 0,
    mode: 0,
};

struct ReadOnlyRoot {
    image: OwnedRoot,
    nodes: [RootNode; MAX_INDEXED_ENTRIES],
    node_count: usize,
}

impl ReadOnlyRoot {
    fn build(image: OwnedRoot) -> Option<Self> {
        let Some((nodes, node_count)) = Self::build_nodes(&image) else {
            let _ = image.release();
            return None;
        };
        Some(Self {
            image,
            nodes,
            node_count,
        })
    }

    fn build_nodes(image: &OwnedRoot) -> Option<([RootNode; MAX_INDEXED_ENTRIES], usize)> {
        let bytes = image.bytes()?;
        let archive = parse(bytes)?;
        if archive.entry_count() > MAX_INDEXED_ENTRIES {
            return None;
        }
        let base = bytes.as_ptr() as usize;
        let mut nodes = [EMPTY_ROOT_NODE; MAX_INDEXED_ENTRIES];
        let mut offset = 0;
        let mut node_count = 0;
        loop {
            let (entry, next, trailer) = parse_entry(bytes, offset)?;
            if trailer {
                break;
            }
            let name_offset = (entry.name.as_ptr() as usize).checked_sub(base)?;
            let data_offset = (entry.data.as_ptr() as usize).checked_sub(base)?;
            nodes[node_count] = RootNode {
                name_offset: u32::try_from(name_offset).ok()?,
                name_size: u32::try_from(entry.name.len()).ok()?,
                data_offset: u32::try_from(data_offset).ok()?,
                data_size: u32::try_from(entry.data.len()).ok()?,
                mode: entry.mode,
            };
            node_count += 1;
            offset = next;
        }
        (node_count == archive.entry_count()).then_some((nodes, node_count))
    }

    fn lookup(&self, name: &[u8]) -> Option<Entry<'_>> {
        if !valid_name(name) {
            return None;
        }
        let bytes = self.image.bytes()?;
        for node in &self.nodes[..self.node_count] {
            let name_start = usize::try_from(node.name_offset).ok()?;
            let name_end = name_start.checked_add(usize::try_from(node.name_size).ok()?)?;
            let stored_name = bytes.get(name_start..name_end)?;
            if stored_name == name {
                let data_start = usize::try_from(node.data_offset).ok()?;
                let data_end = data_start.checked_add(usize::try_from(node.data_size).ok()?)?;
                return Some(Entry {
                    name: stored_name,
                    data: bytes.get(data_start..data_end)?,
                    mode: node.mode,
                });
            }
        }
        None
    }

    fn release(self) -> bool {
        self.image.release()
    }
}

fn published_root() -> Option<&'static ReadOnlyRoot> {
    if !ROOT_PUBLISHED.load(Ordering::Acquire) {
        return None;
    }
    // SAFETY: publication writes the root exactly once before the release
    // store. Bootstrap is single-core and never mutates or releases it.
    Some(unsafe { &*core::ptr::addr_of!(PUBLISHED_ROOT).cast::<ReadOnlyRoot>() })
}

pub(crate) fn publish_read_only_root(source: &[u8]) -> bool {
    if ROOT_PUBLISHED.load(Ordering::Acquire) {
        return false;
    }
    let Some(image) = OwnedRoot::materialize(source) else {
        return false;
    };
    let Some(root) = ReadOnlyRoot::build(image) else {
        return false;
    };
    if root.node_count == 0 || !root.image.tail_is_zero() {
        let _ = root.release();
        return false;
    }
    // SAFETY: the publication flag is still false and bootstrap is single-core.
    unsafe {
        core::ptr::addr_of_mut!(PUBLISHED_ROOT).write(MaybeUninit::new(root));
    }
    ROOT_PUBLISHED.store(true, Ordering::Release);
    true
}

pub(crate) fn published_root_is_live() -> bool {
    published_root().is_some_and(|root| {
        root.node_count != 0
            && root.image.tail_is_zero()
            && memory::frames_owned_by(memory::FrameOwner::KERNEL, &root.image.frames)
    })
}

pub(crate) fn published_lookup(name: &[u8]) -> Option<(u32, usize)> {
    let entry = published_root()?.lookup(name)?;
    Some((entry.mode, entry.data.len()))
}

fn published_node(name: &[u8], expected_type: u32) -> Option<u8> {
    let root = published_root()?;
    let entry = root.lookup(name)?;
    if entry.mode & TYPE_MASK != expected_type {
        return None;
    }
    root.nodes[..root.node_count]
        .iter()
        .position(|node| {
            let Ok(offset) = usize::try_from(node.name_offset) else {
                return false;
            };
            let Ok(size) = usize::try_from(node.name_size) else {
                return false;
            };
            let Some(end) = offset.checked_add(size) else {
                return false;
            };
            root.image.bytes().and_then(|bytes| bytes.get(offset..end)) == Some(name)
        })
        .and_then(|index| u8::try_from(index + 1).ok())
}

pub(crate) fn published_regular_file_node(name: &[u8]) -> Option<u8> {
    published_node(name, REGULAR)
}

pub(crate) fn published_directory_node(name: &[u8]) -> Option<u8> {
    published_node(name, DIRECTORY)
}

pub(crate) fn published_directory_by_node(node: u8) -> Option<&'static [u8]> {
    let root = published_root()?;
    let index = usize::from(node.checked_sub(1)?);
    let node = root.nodes.get(index).filter(|_| index < root.node_count)?;
    if node.mode & TYPE_MASK != DIRECTORY {
        return None;
    }
    let start = usize::try_from(node.name_offset).ok()?;
    let end = start.checked_add(usize::try_from(node.name_size).ok()?)?;
    root.image.bytes()?.get(start..end)
}

pub(crate) fn published_file_by_node(node: u8) -> Option<&'static [u8]> {
    let root = published_root()?;
    let index = usize::from(node.checked_sub(1)?);
    let node = root.nodes.get(index).filter(|_| index < root.node_count)?;
    if node.mode & TYPE_MASK != REGULAR {
        return None;
    }
    let start = usize::try_from(node.data_offset).ok()?;
    let end = start.checked_add(usize::try_from(node.data_size).ok()?)?;
    root.image.bytes()?.get(start..end)
}

pub(crate) fn published_read(name: &[u8], offset: usize, output: &mut [u8]) -> Option<usize> {
    let entry = published_root()?.lookup(name)?;
    let available = entry.data.get(offset..)?;
    let size = output.len().min(available.len());
    output
        .get_mut(..size)?
        .copy_from_slice(available.get(..size)?);
    Some(size)
}

pub(crate) struct PublishedDirectoryEntry<'a> {
    pub(crate) name: &'a [u8],
    pub(crate) mode: u32,
    pub(crate) data_size: usize,
    pub(crate) next_cursor: usize,
}

pub(crate) fn published_next_child(
    directory: &[u8],
    cursor: usize,
) -> Option<Option<PublishedDirectoryEntry<'static>>> {
    let root = published_root()?;
    if cursor > root.node_count
        || (!directory.is_empty()
            && root
                .lookup(directory)
                .is_none_or(|entry| entry.mode & TYPE_MASK != DIRECTORY))
    {
        return None;
    }
    let bytes = root.image.bytes()?;
    for (index, node) in root.nodes[cursor..root.node_count].iter().enumerate() {
        let name_start = usize::try_from(node.name_offset).ok()?;
        let name_end = name_start.checked_add(usize::try_from(node.name_size).ok()?)?;
        let name = bytes.get(name_start..name_end)?;
        let child = if directory.is_empty() {
            (!name.contains(&b'/')).then_some(name)
        } else {
            name.strip_prefix(directory)
                .and_then(|suffix| suffix.strip_prefix(core::slice::from_ref(&b'/')))
                .filter(|suffix| !suffix.is_empty() && !suffix.contains(&b'/'))
        };
        if let Some(child) = child {
            return Some(Some(PublishedDirectoryEntry {
                name: child,
                mode: node.mode,
                data_size: usize::try_from(node.data_size).ok()?,
                next_cursor: cursor + index + 1,
            }));
        }
    }
    Some(None)
}

pub(crate) fn verify_read_only_index(source: &[u8]) -> bool {
    let Some(before) = memory::owner_live_frame_count(memory::FrameOwner::KERNEL) else {
        return false;
    };
    let Some(image) = OwnedRoot::materialize(source) else {
        return false;
    };
    let Some(root) = ReadOnlyRoot::build(image) else {
        return false;
    };
    let base = root.image.frames[0].direct_map_address();
    let init = root.lookup(b"init");
    let valid = root.node_count == 14
        && init.is_some_and(|entry| {
            entry.mode == 0o100555
                && entry.data == b"GINUX_INITRAMFS_INIT_V0\n"
                && base.is_some_and(|start| {
                    let address = entry.data.as_ptr() as usize;
                    (start..start + PAGE_SIZE).contains(&address)
                })
        })
        && [b"dev" as &[u8], b"etc", b"proc", b"sys", b"tmp", b"var"]
            .iter()
            .all(|name| {
                root.lookup(name)
                    .is_some_and(|entry| entry.mode & TYPE_MASK == DIRECTORY)
            })
        && root
            .lookup(b"etc/hostname")
            .is_some_and(|entry| entry.mode == 0o100555 && entry.data == b"ginux\n")
        && root
            .lookup(b"etc/os-release")
            .is_some_and(|entry| entry.mode == 0o100555 && entry.data == b"NAME=Ginux\nID=ginux\n")
        && root
            .lookup(b"etc/motd")
            .is_some_and(|entry| entry.mode == 0o100555 && entry.data == b"Welcome to Ginux\n")
        && root.lookup(b"linux/init").is_some_and(|entry| {
            entry.mode == 0o100555 && entry.data == crate::linux_elf::runtime_main_bytes()
        })
        && root.lookup(b"linux/ld-ginux.so").is_some_and(|entry| {
            entry.mode == 0o100555 && entry.data == crate::linux_elf::runtime_interpreter_bytes()
        })
        && root.lookup(b"linux/libginux.so").is_some_and(|entry| {
            entry.mode == 0o100555 && entry.data == crate::linux_elf::runtime_dependency_bytes()
        })
        && root.lookup(b"linux/libginux-aux.so").is_some_and(|entry| {
            entry.mode == 0o100555
                && entry.data == crate::linux_elf::runtime_auxiliary_dependency_bytes()
        })
        && root.lookup(b"missing").is_none()
        && root.lookup(b"../init").is_none()
        && root.lookup(b"/init").is_none();
    valid
        && root.release()
        && memory::owner_live_frame_count(memory::FrameOwner::KERNEL) == Some(before)
}
