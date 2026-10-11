const MAGIC: &[u8; 6] = b"070701";
const HEADER_SIZE: usize = 110;
const TRAILER: &[u8] = b"TRAILER!!!";
const MAX_ARCHIVE_SIZE: usize = 16 * 1024 * 1024;
const MAX_ENTRIES: usize = 256;
const MAX_NAME_SIZE: usize = 256;
const MAX_FILE_SIZE: usize = 8 * 1024 * 1024;
const TYPE_MASK: u32 = 0o170000;
const DIRECTORY: u32 = 0o040000;
const REGULAR: u32 = 0o100000;
const SYMLINK: u32 = 0o120000;

const PINNED_INITRAMFS: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/initramfs-v0.cpio"));

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

pub(crate) fn verify_pinned_image() -> bool {
    let Some(archive) = parse(PINNED_INITRAMFS) else {
        return false;
    };
    archive.entry_count() == 6
        && archive.find(b"init").is_some_and(|entry| {
            entry.mode == 0o100555 && entry.data == b"GINUX_INITRAMFS_INIT_V0\n"
        })
        && [b"dev" as &[u8], b"etc", b"proc", b"sys", b"tmp"]
            .iter()
            .all(|name| {
                archive
                    .find(name)
                    .is_some_and(|entry| entry.mode & TYPE_MASK == DIRECTORY)
            })
        && archive.find(b"missing").is_none()
}

pub(crate) fn verify_rejections() -> bool {
    let mut candidate = [0_u8; PINNED_INITRAMFS.len()];
    candidate.copy_from_slice(PINNED_INITRAMFS);
    candidate[0] = b'1';
    if parse(&candidate).is_some() {
        return false;
    }
    candidate.copy_from_slice(PINNED_INITRAMFS);
    candidate[6] = b'G';
    if parse(&candidate).is_some() {
        return false;
    }
    candidate.copy_from_slice(PINNED_INITRAMFS);
    let Some(init) = header_offset(PINNED_INITRAMFS, b"init") else {
        return false;
    };
    candidate[init + 14..init + 22].copy_from_slice(b"00008124");
    if parse(&candidate).is_some() {
        return false;
    }
    candidate.copy_from_slice(PINNED_INITRAMFS);
    let Some(etc) = name_offset(PINNED_INITRAMFS, b"etc") else {
        return false;
    };
    candidate[etc..etc + 3].copy_from_slice(b"dev");
    if parse(&candidate).is_some() {
        return false;
    }
    candidate.copy_from_slice(PINNED_INITRAMFS);
    candidate[etc..etc + 3].copy_from_slice(b"../");
    if parse(&candidate).is_some() {
        return false;
    }
    candidate.copy_from_slice(PINNED_INITRAMFS);
    let Some(proc_name) = name_offset(PINNED_INITRAMFS, b"proc") else {
        return false;
    };
    candidate[proc_name + 5] = 1;
    if parse(&candidate).is_some() {
        return false;
    }
    candidate.copy_from_slice(PINNED_INITRAMFS);
    candidate[init + 54..init + 62].copy_from_slice(b"ffffffff");
    if parse(&candidate).is_some() {
        return false;
    }
    candidate.copy_from_slice(PINNED_INITRAMFS);
    let Some(init_name) = name_offset(PINNED_INITRAMFS, b"init") else {
        return false;
    };
    candidate[init_name..init_name + 4].copy_from_slice(b"inix");
    if parse(&candidate).is_some() {
        return false;
    }
    candidate.copy_from_slice(PINNED_INITRAMFS);
    let Some(trailer) = header_offset(PINNED_INITRAMFS, TRAILER) else {
        return false;
    };
    candidate[trailer + HEADER_SIZE] = b'X';
    parse(&candidate).is_none() && parse(&PINNED_INITRAMFS[..PINNED_INITRAMFS.len() - 1]).is_none()
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
