use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use ginux_oci::{Entry, EntryKind, Rootfs};

pub(crate) const ROOT_HANDLE: u16 = 1;
const ENTRY_HANDLE_BASE: usize = 2;
const PATH_CAPACITY: usize = 256;

pub(crate) struct Child {
    pub(crate) next_cursor: usize,
    pub(crate) handle: u16,
    pub(crate) entry: Entry<'static>,
    pub(crate) name: &'static [u8],
}

const PINNED_ROOTFS_SHA256: [u8; 32] = [
    0x74, 0xd9, 0x7c, 0x42, 0x8c, 0x51, 0xa8, 0x28, 0xf9, 0x05, 0x1a, 0x7a, 0x40, 0xa5, 0x3f, 0xf1,
    0xfc, 0x99, 0xe5, 0x4f, 0xc3, 0x03, 0x23, 0xce, 0x36, 0x76, 0x07, 0x01, 0xb0, 0xb7, 0xf7, 0x11,
];

static INSTALLED: AtomicBool = AtomicBool::new(false);
static ADDRESS: AtomicUsize = AtomicUsize::new(0);
static SIZE: AtomicUsize = AtomicUsize::new(0);

pub(crate) fn is_installed() -> bool {
    INSTALLED.load(Ordering::Acquire)
}

pub(crate) fn install_pinned_rootfs(bytes: &'static [u8]) -> bool {
    if INSTALLED.load(Ordering::Acquire) || !ginux_oci::verify_sha256(bytes, &PINNED_ROOTFS_SHA256)
    {
        return false;
    }
    let Some(rootfs) = Rootfs::parse(bytes) else {
        return false;
    };
    let valid = process_plan_in(&rootfs, b"/bin/sh").is_some_and(|plan| {
        plan.main.segment_count == 4
            && plan.interpreter.segment_count == 4
            && plan.main.program_header_count == 12
            && plan.interpreter.program_header_count == 11
            && plan.main.program_headers == 0x40_0040
            && plan.interpreter.program_headers == 0x60_0040
            && plan.main.load_bias == 0x40_0000
            && plan.interpreter.load_bias == 0x60_0000
            && plan.initial_entry == plan.interpreter.entry
            && plan.main.segments[..plan.main.segment_count]
                .iter()
                .flatten()
                .all(|segment| segment.page_count != 0)
    });
    if !valid {
        return false;
    }
    ADDRESS.store(bytes.as_ptr() as usize, Ordering::Relaxed);
    SIZE.store(bytes.len(), Ordering::Relaxed);
    INSTALLED.store(true, Ordering::Release);
    process_plan(b"/bin/sh").is_some()
}

fn rootfs() -> Option<Rootfs<'static>> {
    if !INSTALLED.load(Ordering::Acquire) {
        return None;
    }
    let address = ADDRESS.load(Ordering::Relaxed);
    let size = SIZE.load(Ordering::Relaxed);
    if address == 0 || size == 0 {
        return None;
    }
    // SAFETY: installation records an immutable slice embedded in the loaded
    // EFI image. The image remains mapped and read-only for the kernel lifetime.
    let bytes = unsafe { core::slice::from_raw_parts(address as *const u8, size) };
    Rootfs::parse(bytes)
}

pub(crate) fn lookup(path: &[u8]) -> Option<(u16, Entry<'static>)> {
    let rootfs = rootfs()?;
    let (index, entry) = lookup_in(&rootfs, path)?;
    let handle = u16::try_from(index.checked_add(ENTRY_HANDLE_BASE)?).ok()?;
    Some((handle, entry))
}

pub(crate) fn lookup_metadata(path: &[u8]) -> Option<(u16, Entry<'static>)> {
    let rootfs = rootfs()?;
    let path = path.strip_prefix(b"/").unwrap_or(path);
    let path = path.strip_suffix(b"/").unwrap_or(path);
    let (index, entry) = rootfs.find_index(path)?;
    let handle = u16::try_from(index.checked_add(ENTRY_HANDLE_BASE)?).ok()?;
    Some((handle, entry))
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
    for _ in 0..8 {
        let (index, entry) = rootfs.find_index(&current[..length])?;
        if entry.kind != EntryKind::Symlink {
            return Some((index, entry));
        }
        let mut source = [0_u8; PATH_CAPACITY];
        source[..length].copy_from_slice(&current[..length]);
        length = resolve_link(&source[..length], entry.link, &mut current)?;
    }
    None
}

pub(crate) fn entry(handle: u16) -> Option<Entry<'static>> {
    let index = usize::from(handle).checked_sub(ENTRY_HANDLE_BASE)?;
    rootfs()?.entry(index)
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

fn process_plan_in<'a>(
    rootfs: &Rootfs<'a>,
    init: &[u8],
) -> Option<crate::linux_elf::RelocatableProcessPlan<'a, 'a>> {
    let (_, executable) = lookup_in(rootfs, init)?;
    if executable.kind != EntryKind::File || executable.mode & 0o111 == 0 {
        return None;
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
