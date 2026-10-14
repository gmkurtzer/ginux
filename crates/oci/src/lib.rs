#![no_std]

#[cfg(test)]
extern crate std;

use sha2::{Digest, Sha256};

const TAR_BLOCK_SIZE: usize = 512;
const TAR_HEADER_SIZE: usize = 512;
const TAR_MAX_ENTRIES: usize = 4096;
const TAR_NAME_SIZE: usize = 100;
const TAR_PREFIX_SIZE: usize = 155;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EntryKind {
    File,
    Directory,
    Symlink,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Entry<'a> {
    pub path: &'a [u8],
    pub prefix: &'a [u8],
    pub data: &'a [u8],
    pub link: &'a [u8],
    pub kind: EntryKind,
    pub mode: u32,
}

pub struct Rootfs<'a> {
    bytes: &'a [u8],
    entry_count: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Child<'a> {
    pub index: usize,
    pub next_cursor: usize,
    pub entry: Entry<'a>,
    pub name: &'a [u8],
}

impl<'a> Rootfs<'a> {
    pub fn parse(bytes: &'a [u8]) -> Option<Self> {
        let mut offset = 0_usize;
        let mut entry_count = 0;
        loop {
            let header = bytes.get(offset..offset.checked_add(TAR_HEADER_SIZE)?)?;
            if header.iter().all(|byte| *byte == 0) {
                let second =
                    bytes.get(offset + TAR_BLOCK_SIZE..offset.checked_add(TAR_BLOCK_SIZE * 2)?)?;
                if !second.iter().all(|byte| *byte == 0)
                    || bytes[offset + TAR_BLOCK_SIZE * 2..]
                        .iter()
                        .any(|byte| *byte != 0)
                {
                    return None;
                }
                return (entry_count != 0).then_some(Self { bytes, entry_count });
            }
            let (_, next) = parse_entry(bytes, offset)?;
            entry_count = entry_count.checked_add(1)?;
            if entry_count > TAR_MAX_ENTRIES {
                return None;
            }
            offset = next;
        }
    }

    pub const fn entry_count(&self) -> usize {
        self.entry_count
    }

    pub fn find(&self, path: &[u8]) -> Option<Entry<'a>> {
        self.find_index(path).map(|(_, entry)| entry)
    }

    pub fn entry(&self, wanted: usize) -> Option<Entry<'a>> {
        if wanted >= self.entry_count {
            return None;
        }
        let mut offset = 0;
        for index in 0..self.entry_count {
            let (entry, next) = parse_entry(self.bytes, offset)?;
            if index == wanted {
                return Some(entry);
            }
            offset = next;
        }
        None
    }

    pub fn find_index(&self, path: &[u8]) -> Option<(usize, Entry<'a>)> {
        if !valid_path(path) {
            return None;
        }
        let mut offset = 0;
        for index in 0..self.entry_count {
            let (entry, next) = parse_entry(self.bytes, offset)?;
            if entry_path_matches(entry, path) {
                return Some((index, entry));
            }
            offset = next;
        }
        None
    }

    pub fn next_child(&self, parent: &[u8], cursor: usize) -> Option<Option<Child<'a>>> {
        if !parent.is_empty() && !valid_path(parent) || cursor > self.entry_count {
            return None;
        }
        let mut offset = 0;
        for index in 0..self.entry_count {
            let (entry, next) = parse_entry(self.bytes, offset)?;
            offset = next;
            if index < cursor {
                continue;
            }
            let mut full_path = [0_u8; TAR_PREFIX_SIZE + 1 + TAR_NAME_SIZE];
            let length = copy_entry_path(entry, &mut full_path)?;
            let path = &full_path[..length];
            let remainder = if parent.is_empty() {
                path
            } else {
                let Some(remainder) = path.strip_prefix(parent) else {
                    continue;
                };
                let Some(remainder) = remainder.strip_prefix(b"/") else {
                    continue;
                };
                remainder
            };
            if remainder.is_empty() || remainder.contains(&b'/') {
                continue;
            }
            let name = entry.path.rsplit(|byte| *byte == b'/').next()?;
            return Some(Some(Child {
                index,
                next_cursor: index + 1,
                entry,
                name,
            }));
        }
        Some(None)
    }

    pub fn copy_path(&self, index: usize, output: &mut [u8]) -> Option<usize> {
        copy_entry_path(self.entry(index)?, output)
    }
}

pub fn verify_sha256(bytes: &[u8], expected: &[u8; 32]) -> bool {
    let actual: [u8; 32] = Sha256::digest(bytes).into();
    actual == *expected
}

fn parse_entry(bytes: &[u8], offset: usize) -> Option<(Entry<'_>, usize)> {
    let header = bytes.get(offset..offset.checked_add(TAR_HEADER_SIZE)?)?;
    if &header[257..263] != b"ustar\0" || checksum(header)? != stored_checksum(header)? {
        return None;
    }
    let stored_path = terminated(&header[..TAR_NAME_SIZE])?;
    let prefix = terminated(&header[345..345 + TAR_PREFIX_SIZE])?;
    let size = usize::try_from(octal(&header[124..136])?).ok()?;
    let mode = u32::try_from(octal(&header[100..108])?).ok()?;
    let kind = match header[156] {
        0 | b'0' => EntryKind::File,
        b'5' if size == 0 => EntryKind::Directory,
        b'2' if size == 0 => EntryKind::Symlink,
        _ => return None,
    };
    let path = if kind == EntryKind::Directory {
        stored_path.strip_suffix(b"/").unwrap_or(stored_path)
    } else {
        stored_path
    };
    if path.is_empty() || !valid_entry_path(prefix, path) {
        return None;
    }
    let link = terminated(&header[157..257])?;
    if kind == EntryKind::Symlink && link.is_empty() {
        return None;
    }
    let data_start = offset.checked_add(TAR_HEADER_SIZE)?;
    let data_end = data_start.checked_add(size)?;
    let data = bytes.get(data_start..data_end)?;
    let padded = size.checked_add(TAR_BLOCK_SIZE - 1)? / TAR_BLOCK_SIZE * TAR_BLOCK_SIZE;
    let next = data_start.checked_add(padded)?;
    bytes.get(offset..next)?;
    Some((
        Entry {
            path,
            prefix,
            data,
            link,
            kind,
            mode,
        },
        next,
    ))
}

fn entry_path_matches(entry: Entry<'_>, wanted: &[u8]) -> bool {
    if entry.prefix.is_empty() {
        entry.path == wanted
    } else {
        wanted.len() == entry.prefix.len() + 1 + entry.path.len()
            && wanted.starts_with(entry.prefix)
            && wanted[entry.prefix.len()] == b'/'
            && &wanted[entry.prefix.len() + 1..] == entry.path
    }
}

fn copy_entry_path(entry: Entry<'_>, output: &mut [u8]) -> Option<usize> {
    let length = entry.prefix.len() + usize::from(!entry.prefix.is_empty()) + entry.path.len();
    let output = output.get_mut(..length)?;
    let mut used = 0;
    if !entry.prefix.is_empty() {
        output[..entry.prefix.len()].copy_from_slice(entry.prefix);
        used = entry.prefix.len();
        output[used] = b'/';
        used += 1;
    }
    output[used..].copy_from_slice(entry.path);
    Some(length)
}

fn terminated(field: &[u8]) -> Option<&[u8]> {
    let end = field
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(field.len());
    field[end..]
        .iter()
        .all(|byte| *byte == 0)
        .then_some(&field[..end])
}

fn octal(field: &[u8]) -> Option<u64> {
    let mut value = 0_u64;
    let mut digit = false;
    let mut ended = false;
    for byte in field {
        match *byte {
            b'0'..=b'7' if !ended => {
                value = value.checked_mul(8)?.checked_add(u64::from(*byte - b'0'))?;
                digit = true;
            }
            0 | b' ' if digit => ended = true,
            b' ' if !digit => {}
            _ => return None,
        }
    }
    digit.then_some(value)
}

fn stored_checksum(header: &[u8]) -> Option<u64> {
    octal(header.get(148..156)?)
}

fn checksum(header: &[u8]) -> Option<u64> {
    (header.len() == TAR_HEADER_SIZE).then(|| {
        header
            .iter()
            .enumerate()
            .map(|(index, byte)| {
                if (148..156).contains(&index) {
                    u64::from(b' ')
                } else {
                    u64::from(*byte)
                }
            })
            .sum()
    })
}

fn valid_entry_path(prefix: &[u8], path: &[u8]) -> bool {
    valid_path(path) && (prefix.is_empty() || valid_path(prefix)) && !prefix.ends_with(b"/")
}

fn valid_path(path: &[u8]) -> bool {
    !path.is_empty()
        && path[0] != b'/'
        && path.iter().all(|byte| byte.is_ascii_graphic())
        && path
            .split(|byte| *byte == b'/')
            .all(|component| !component.is_empty() && component != b"." && component != b"..")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::format;
    use std::vec;
    use std::vec::Vec;

    fn archive(path: &[u8], data: &[u8], kind: u8, link: &[u8]) -> Vec<u8> {
        let mut output = vec![0_u8; TAR_BLOCK_SIZE];
        output[..path.len()].copy_from_slice(path);
        output[100..108].copy_from_slice(b"0000755\0");
        output[108..116].copy_from_slice(b"0000000\0");
        output[116..124].copy_from_slice(b"0000000\0");
        let size = format!("{:011o}\0", data.len());
        output[124..136].copy_from_slice(size.as_bytes());
        output[136..148].copy_from_slice(b"00000000000\0");
        output[148..156].fill(b' ');
        output[156] = kind;
        output[157..157 + link.len()].copy_from_slice(link);
        output[257..263].copy_from_slice(b"ustar\0");
        output[263..265].copy_from_slice(b"00");
        let sum: u64 = output.iter().map(|byte| u64::from(*byte)).sum();
        output[148..156].copy_from_slice(format!("{:06o}\0 ", sum).as_bytes());
        output.extend_from_slice(data);
        output.resize(output.len().div_ceil(TAR_BLOCK_SIZE) * TAR_BLOCK_SIZE, 0);
        output.resize(output.len() + TAR_BLOCK_SIZE * 2, 0);
        output
    }

    #[test]
    fn indexes_regular_file_and_symlink() {
        let bytes = archive(b"bin/busybox", b"ELF", b'0', b"");
        let root = Rootfs::parse(&bytes).unwrap();
        assert_eq!(root.entry_count(), 1);
        assert_eq!(root.find(b"bin/busybox").unwrap().data, b"ELF");

        let bytes = archive(b"bin/sh", b"", b'2', b"/bin/busybox");
        let root = Rootfs::parse(&bytes).unwrap();
        assert_eq!(root.find(b"bin/sh").unwrap().link, b"/bin/busybox");
    }

    #[test]
    fn rejects_traversal_corruption_and_trailing_data() {
        assert!(Rootfs::parse(&archive(b"../escape", b"x", b'0', b"")).is_none());
        let mut corrupt = archive(b"bin/sh", b"x", b'0', b"");
        corrupt[0] ^= 1;
        assert!(Rootfs::parse(&corrupt).is_none());
        let mut trailing = archive(b"bin/sh", b"x", b'0', b"");
        trailing.push(1);
        assert!(Rootfs::parse(&trailing).is_none());
    }
}
