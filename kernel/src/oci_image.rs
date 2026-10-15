use core::sync::atomic::{AtomicUsize, Ordering};
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

pub(crate) struct LaunchArguments<'a> {
    pub(crate) values: [&'a [u8]; 3],
    pub(crate) count: usize,
    pub(crate) execfn: usize,
}

static ADDRESSES: [AtomicUsize; crate::control_abi::CONTAINER_LIMIT] =
    [const { AtomicUsize::new(0) }; crate::control_abi::CONTAINER_LIMIT];
static SIZES: [AtomicUsize; crate::control_abi::CONTAINER_LIMIT] =
    [const { AtomicUsize::new(0) }; crate::control_abi::CONTAINER_LIMIT];

pub(crate) fn is_installed(slot: usize) -> bool {
    slot < crate::control_abi::CONTAINER_LIMIT && SIZES[slot].load(Ordering::Acquire) != 0
}

pub(crate) fn install_runtime_rootfs(slot: usize, bytes: &'static [u8]) -> bool {
    if slot >= crate::control_abi::CONTAINER_LIMIT || is_installed(slot) {
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
    ADDRESSES[slot].store(bytes.as_ptr() as usize, Ordering::Relaxed);
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
    true
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
    Rootfs::parse(bytes)
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
    let (index, entry) = rootfs.find_index(&resolved[..length])?;
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
        if let Some((index, entry)) = rootfs.find_index(&current[..length]) {
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
            let Some((_, entry)) = rootfs.find_index(&source[..prefix_end]) else {
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
