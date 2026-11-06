#![no_std]

pub const INDEX_MAGIC: &[u8; 8] = b"GINUXVFS";
pub const INDEX_VERSION: u32 = 1;
pub const HEADER_SIZE: usize = 64;
pub const RECORD_SIZE: usize = 424;
pub const MAX_PATH: usize = 256;
pub const MAX_LINK: usize = 128;
pub const O_READ: u32 = 1;
pub const O_WRITE: u32 = 2;
pub const O_CREATE: u32 = 4;
pub const O_TRUNC: u32 = 8;
pub const O_CLOEXEC: u32 = 16;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum NodeKind {
    File = 1,
    Directory = 2,
    Symlink = 3,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VfsError {
    InvalidIndex,
    InvalidPath,
    NotFound,
    NotDirectory,
    IsDirectory,
    SymlinkLoop,
    PermissionDenied,
    InvalidDescriptor,
    DescriptorLimit,
    OverlayLimit,
    FileTooLarge,
    AlreadyExists,
    Io,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Entry<'a> {
    pub path: &'a [u8],
    pub kind: NodeKind,
    pub mode: u32,
    pub uid: u32,
    pub gid: u32,
    pub size: u64,
    pub data_offset: u64,
    pub link: &'a [u8],
}

pub struct Index<'a> {
    bytes: &'a [u8],
    count: usize,
}

impl<'a> Index<'a> {
    pub fn new(bytes: &'a [u8]) -> Result<Self, VfsError> {
        if bytes.len() < HEADER_SIZE || &bytes[..8] != INDEX_MAGIC {
            return Err(VfsError::InvalidIndex);
        }
        if read_u32(bytes, 8)? != INDEX_VERSION || read_u32(bytes, 12)? as usize != RECORD_SIZE {
            return Err(VfsError::InvalidIndex);
        }
        let count = read_u32(bytes, 16)? as usize;
        let expected = count
            .checked_mul(RECORD_SIZE)
            .and_then(|size| size.checked_add(HEADER_SIZE))
            .ok_or(VfsError::InvalidIndex)?;
        if expected != bytes.len() {
            return Err(VfsError::InvalidIndex);
        }
        let index = Self { bytes, count };
        let mut previous: Option<&[u8]> = None;
        for offset in 0..count {
            let entry = index.entry(offset)?;
            if entry.path.is_empty()
                || entry.path[0] != b'/'
                || previous.is_some_and(|value| value >= entry.path)
            {
                return Err(VfsError::InvalidIndex);
            }
            previous = Some(entry.path);
        }
        Ok(index)
    }

    pub const fn len(&self) -> usize {
        self.count
    }

    pub const fn is_empty(&self) -> bool {
        self.count == 0
    }

    pub fn entry(&self, index: usize) -> Result<Entry<'a>, VfsError> {
        if index >= self.count {
            return Err(VfsError::InvalidIndex);
        }
        let start = HEADER_SIZE + index * RECORD_SIZE;
        decode_record(&self.bytes[start..start + RECORD_SIZE])
    }

    pub fn lookup_exact(&self, path: &[u8]) -> Result<Entry<'a>, VfsError> {
        let mut low = 0usize;
        let mut high = self.count;
        while low < high {
            let middle = low + (high - low) / 2;
            let entry = self.entry(middle)?;
            match entry.path.cmp(path) {
                core::cmp::Ordering::Less => low = middle + 1,
                core::cmp::Ordering::Greater => high = middle,
                core::cmp::Ordering::Equal => return Ok(entry),
            }
        }
        Err(VfsError::NotFound)
    }

    pub fn resolve(&self, path: &[u8]) -> Result<Entry<'a>, VfsError> {
        let path = resolve_components(path, true, false, |name, link| {
            let entry = self.lookup_exact(name)?;
            if entry.link.len() > link.len() {
                return Err(VfsError::InvalidPath);
            }
            link[..entry.link.len()].copy_from_slice(entry.link);
            Ok((entry.kind, entry.link.len()))
        })?;
        self.lookup_exact(path.as_bytes())
    }
}

fn decode_record(bytes: &[u8]) -> Result<Entry<'_>, VfsError> {
    if bytes.len() != RECORD_SIZE {
        return Err(VfsError::InvalidIndex);
    }
    let path_len = read_u16(bytes, 0)? as usize;
    let kind = match bytes[2] {
        1 => NodeKind::File,
        2 => NodeKind::Directory,
        3 => NodeKind::Symlink,
        _ => return Err(VfsError::InvalidIndex),
    };
    let link_len = read_u16(bytes, 32)? as usize;
    if path_len == 0 || path_len > MAX_PATH || link_len > MAX_LINK {
        return Err(VfsError::InvalidIndex);
    }
    Ok(Entry {
        path: &bytes[40..40 + path_len],
        kind,
        mode: read_u32(bytes, 4)?,
        uid: read_u32(bytes, 8)?,
        gid: read_u32(bytes, 12)?,
        size: read_u64(bytes, 16)?,
        data_offset: read_u64(bytes, 24)?,
        link: &bytes[40 + MAX_PATH..40 + MAX_PATH + link_len],
    })
}

pub fn encode_header(count: u32, output: &mut [u8; HEADER_SIZE]) {
    output.fill(0);
    output[..8].copy_from_slice(INDEX_MAGIC);
    output[8..12].copy_from_slice(&INDEX_VERSION.to_le_bytes());
    output[12..16].copy_from_slice(&(RECORD_SIZE as u32).to_le_bytes());
    output[16..20].copy_from_slice(&count.to_le_bytes());
}

pub fn encode_record(entry: Entry<'_>, output: &mut [u8; RECORD_SIZE]) -> Result<(), VfsError> {
    if entry.path.is_empty()
        || entry.path.len() > MAX_PATH
        || entry.link.len() > MAX_LINK
        || entry.path[0] != b'/'
    {
        return Err(VfsError::InvalidPath);
    }
    output.fill(0);
    output[..2].copy_from_slice(&(entry.path.len() as u16).to_le_bytes());
    output[2] = entry.kind as u8;
    output[4..8].copy_from_slice(&entry.mode.to_le_bytes());
    output[8..12].copy_from_slice(&entry.uid.to_le_bytes());
    output[12..16].copy_from_slice(&entry.gid.to_le_bytes());
    output[16..24].copy_from_slice(&entry.size.to_le_bytes());
    output[24..32].copy_from_slice(&entry.data_offset.to_le_bytes());
    output[32..34].copy_from_slice(&(entry.link.len() as u16).to_le_bytes());
    output[40..40 + entry.path.len()].copy_from_slice(entry.path);
    output[40 + MAX_PATH..40 + MAX_PATH + entry.link.len()].copy_from_slice(entry.link);
    Ok(())
}

#[derive(Clone, Copy)]
struct Path {
    bytes: [u8; MAX_PATH],
    len: usize,
}

impl Path {
    const fn new() -> Self {
        Self {
            bytes: [0; MAX_PATH],
            len: 0,
        }
    }

    fn extend(&mut self, value: &[u8]) -> Result<(), VfsError> {
        let end = self
            .len
            .checked_add(value.len())
            .ok_or(VfsError::InvalidPath)?;
        if end > MAX_PATH {
            return Err(VfsError::InvalidPath);
        }
        self.bytes[self.len..end].copy_from_slice(value);
        self.len = end;
        Ok(())
    }

    fn as_bytes(&self) -> &[u8] {
        &self.bytes[..self.len]
    }
}

// Walk original components: collapsing `..` first can skip a link or erase
// a missing/non-directory component before it has been checked.
fn resolve_components(
    input: &[u8],
    follow_final: bool,
    allow_missing: bool,
    mut lookup: impl FnMut(&[u8], &mut [u8; MAX_PATH]) -> Result<(NodeKind, usize), VfsError>,
) -> Result<Path, VfsError> {
    if input.first() != Some(&b'/') || input.contains(&0) {
        return Err(VfsError::InvalidPath);
    }
    let mut pending = Path::new();
    pending.extend(input)?;
    let mut current = Path::new();
    current.extend(b"/")?;
    let mut position = 1;
    let mut followed = 0;
    while position < pending.len {
        let end = pending.bytes[position..pending.len]
            .iter()
            .position(|byte| *byte == b'/')
            .map_or(pending.len, |offset| position + offset);
        let component = &pending.bytes[position..end];
        if component.is_empty() || component == b"." {
            position = end + 1;
            continue;
        }
        if component == b".." {
            if current.len == 1 {
                return Err(VfsError::InvalidPath);
            }
            current.len = current
                .as_bytes()
                .iter()
                .rposition(|byte| *byte == b'/')
                .unwrap()
                .max(1);
            position = end + 1;
            continue;
        }
        let parent_len = current.len;
        if current.len > 1 {
            current.extend(b"/")?;
        }
        current.extend(component)?;
        let last = end == pending.len;
        let mut link = [0u8; MAX_PATH];
        let (kind, length) = match lookup(current.as_bytes(), &mut link) {
            Ok(entry) => entry,
            Err(VfsError::NotFound) if last && allow_missing => return Ok(current),
            Err(error) => return Err(error),
        };
        if kind == NodeKind::Symlink && (follow_final || !last) {
            if followed == 16 {
                return Err(VfsError::SymlinkLoop);
            }
            followed += 1;
            if length == 0 {
                return Err(VfsError::NotFound);
            }
            let target = link.get(..length).ok_or(VfsError::InvalidPath)?;
            if target.contains(&0) {
                return Err(VfsError::InvalidPath);
            }
            current.len = if target[0] == b'/' { 1 } else { parent_len };
            let mut replacement = Path::new();
            replacement.extend(target)?;
            replacement.extend(&pending.bytes[end..pending.len])?;
            pending = replacement;
            position = 0;
            continue;
        }
        if !last && kind != NodeKind::Directory {
            return Err(VfsError::NotDirectory);
        }
        position = end + 1;
    }
    Ok(current)
}

pub trait BaseReader {
    fn read_at(&mut self, offset: u64, output: &mut [u8]) -> Result<(), VfsError>;
}

#[derive(Clone, Copy)]
struct OverlayFile<const FILE_SIZE: usize> {
    state: u8,
    path: Path,
    mode: u32,
    len: usize,
    data: [u8; FILE_SIZE],
}

impl<const FILE_SIZE: usize> OverlayFile<FILE_SIZE> {
    const fn empty() -> Self {
        Self {
            state: 0,
            path: Path::new(),
            mode: 0,
            len: 0,
            data: [0; FILE_SIZE],
        }
    }
}

#[derive(Clone, Copy)]
struct Descriptor {
    open: bool,
    overlay: bool,
    object: usize,
    description: usize,
    readable: bool,
    writable: bool,
    close_on_exec: bool,
}

impl Descriptor {
    const fn empty() -> Self {
        Self {
            open: false,
            overlay: false,
            object: 0,
            description: 0,
            readable: false,
            writable: false,
            close_on_exec: false,
        }
    }
}

pub struct VirtualFs<'a, R, const FILES: usize, const FILE_SIZE: usize, const FDS: usize> {
    index: Index<'a>,
    reader: R,
    overlay: [OverlayFile<FILE_SIZE>; FILES],
    descriptors: [Descriptor; FDS],
    offsets: [usize; FDS],
}

impl<'a, R: BaseReader, const FILES: usize, const FILE_SIZE: usize, const FDS: usize>
    VirtualFs<'a, R, FILES, FILE_SIZE, FDS>
{
    pub fn new(index: Index<'a>, reader: R) -> Self {
        Self {
            index,
            reader,
            overlay: [OverlayFile::empty(); FILES],
            descriptors: [Descriptor::empty(); FDS],
            offsets: [0; FDS],
        }
    }

    pub fn open(&mut self, path: &[u8], flags: u32) -> Result<i32, VfsError> {
        let path = self.resolved_path(path, true, flags & O_CREATE != 0)?;
        let writable = flags & O_WRITE != 0;
        let readable = flags & O_READ != 0;
        if !writable && !readable {
            return Err(VfsError::PermissionDenied);
        }
        if flags & O_TRUNC != 0 && !writable {
            return Err(VfsError::PermissionDenied);
        }
        // Reserve capacity before any operation can create, copy up or truncate.
        let slot = self
            .descriptors
            .iter()
            .position(|descriptor| !descriptor.open)
            .ok_or(VfsError::DescriptorLimit)?;
        // Each open creates one open-file description; duplicates share it, and
        // with it the offset, until their last close. A free descriptor slot
        // means at most FDS - 1 descriptions are in use, so one is always free.
        let description = (0..FDS)
            .find(|description| {
                !self
                    .descriptors
                    .iter()
                    .any(|fd| fd.open && fd.description == *description)
            })
            .ok_or(VfsError::DescriptorLimit)?;
        let overlay_index = self.find_overlay(path.as_bytes());
        let object: usize;
        let overlay: bool;
        match overlay_index {
            Some(index) if self.overlay[index].state == 2 => {
                if flags & O_CREATE == 0 {
                    return Err(VfsError::NotFound);
                }
                object = self.create_overlay(path, 0o600)?;
                overlay = true;
            }
            Some(index) => {
                object = index;
                overlay = true;
            }
            None => match self.index.resolve(path.as_bytes()) {
                Ok(entry) => {
                    if entry.kind == NodeKind::Directory {
                        return Err(VfsError::IsDirectory);
                    }
                    if writable {
                        if entry.mode & 0o222 == 0 {
                            return Err(VfsError::PermissionDenied);
                        }
                        object = self.copy_to_overlay(path, entry)?;
                        overlay = true;
                    } else {
                        object = self
                            .find_base_record(entry.path)
                            .ok_or(VfsError::InvalidIndex)?;
                        overlay = false;
                    }
                }
                Err(VfsError::NotFound) if flags & O_CREATE != 0 => {
                    self.require_parent(path.as_bytes())?;
                    object = self.create_overlay(path, 0o600)?;
                    overlay = true;
                }
                Err(error) => return Err(error),
            },
        }
        if overlay && flags & O_TRUNC != 0 {
            self.overlay[object].len = 0;
        }

        self.offsets[description] = 0;
        self.descriptors[slot] = Descriptor {
            open: true,
            overlay,
            object,
            description,
            readable,
            writable,
            close_on_exec: flags & O_CLOEXEC != 0,
        };
        Ok(slot as i32 + 3)
    }

    pub fn read(&mut self, descriptor: i32, output: &mut [u8]) -> Result<usize, VfsError> {
        let slot = self.descriptor(descriptor)?;
        if !self.descriptors[slot].readable {
            return Err(VfsError::PermissionDenied);
        }
        let state = self.descriptors[slot];
        let offset = self.offsets[state.description];
        let available = if state.overlay {
            self.overlay[state.object].len.saturating_sub(offset)
        } else {
            let entry = self.index.entry(state.object)?;
            usize::try_from(entry.size)
                .map_err(|_| VfsError::FileTooLarge)?
                .saturating_sub(offset)
        };
        let length = available.min(output.len());
        if state.overlay {
            output[..length]
                .copy_from_slice(&self.overlay[state.object].data[offset..offset + length]);
        } else {
            let entry = self.index.entry(state.object)?;
            self.reader
                .read_at(entry.data_offset + offset as u64, &mut output[..length])?;
        }
        self.offsets[state.description] += length;
        Ok(length)
    }

    pub fn write(&mut self, descriptor: i32, input: &[u8]) -> Result<usize, VfsError> {
        let slot = self.descriptor(descriptor)?;
        let state = self.descriptors[slot];
        let offset = self.offsets[state.description];
        if !state.writable || !state.overlay {
            return Err(VfsError::PermissionDenied);
        }
        let end = offset
            .checked_add(input.len())
            .ok_or(VfsError::FileTooLarge)?;
        if end > FILE_SIZE {
            return Err(VfsError::FileTooLarge);
        }
        self.overlay[state.object].data[offset..end].copy_from_slice(input);
        self.overlay[state.object].len = self.overlay[state.object].len.max(end);
        self.offsets[state.description] = end;
        Ok(input.len())
    }

    /// Share the open-file description, and with it the offset; close-on-exec
    /// stays per descriptor.
    pub fn duplicate(&mut self, descriptor: i32, close_on_exec: bool) -> Result<i32, VfsError> {
        let source = self.descriptor(descriptor)?;
        let target = self
            .descriptors
            .iter()
            .position(|descriptor| !descriptor.open)
            .ok_or(VfsError::DescriptorLimit)?;
        self.descriptors[target] = self.descriptors[source];
        self.descriptors[target].close_on_exec = close_on_exec;
        Ok(target as i32 + 3)
    }

    pub fn close(&mut self, descriptor: i32) -> Result<(), VfsError> {
        let slot = self.descriptor(descriptor)?;
        let state = self.descriptors[slot];
        self.descriptors[slot] = Descriptor::empty();
        if state.overlay {
            self.release_unlinked(state.object);
        }
        Ok(())
    }

    pub fn close_on_exec(&mut self) {
        for slot in 0..FDS {
            if self.descriptors[slot].open && self.descriptors[slot].close_on_exec {
                let _ = self.close(slot as i32 + 3);
            }
        }
    }

    fn referenced(&self, object: usize) -> bool {
        self.descriptors
            .iter()
            .any(|fd| fd.open && fd.overlay && fd.object == object)
    }

    fn release_unlinked(&mut self, object: usize) {
        if self.overlay[object].state == 2 && !self.referenced(object) {
            self.overlay[object].data.fill(0);
            self.overlay[object].len = 0;
        }
    }

    pub fn unlink(&mut self, path: &[u8]) -> Result<(), VfsError> {
        let path = self.resolved_path(path, false, false)?;
        if let Some(index) = self.find_overlay(path.as_bytes()) {
            if self.overlay[index].state == 2 {
                return Err(VfsError::NotFound);
            }
            self.overlay[index].state = 2;
            self.release_unlinked(index);
            return Ok(());
        }
        let entry = self.index.lookup_exact(path.as_bytes())?;
        if entry.kind == NodeKind::Directory {
            return Err(VfsError::IsDirectory);
        }
        let index = self.allocate_overlay()?;
        self.overlay[index].state = 2;
        self.overlay[index].path = path;
        Ok(())
    }

    pub fn teardown(&mut self) {
        self.overlay.fill(OverlayFile::empty());
        self.descriptors.fill(Descriptor::empty());
    }

    pub fn overlay_files(&self) -> usize {
        self.overlay.iter().filter(|entry| entry.state == 1).count()
    }

    fn descriptor(&self, descriptor: i32) -> Result<usize, VfsError> {
        let slot = descriptor
            .checked_sub(3)
            .ok_or(VfsError::InvalidDescriptor)? as usize;
        if slot >= FDS || !self.descriptors[slot].open {
            return Err(VfsError::InvalidDescriptor);
        }
        Ok(slot)
    }

    fn find_overlay(&self, path: &[u8]) -> Option<usize> {
        self.overlay
            .iter()
            .position(|entry| entry.state == 1 && entry.path.as_bytes() == path)
            .or_else(|| {
                self.overlay
                    .iter()
                    .position(|entry| entry.state == 2 && entry.path.as_bytes() == path)
            })
    }

    fn allocate_overlay(&self) -> Result<usize, VfsError> {
        self.overlay
            .iter()
            .position(|entry| entry.state == 0)
            .or_else(|| {
                self.overlay.iter().enumerate().position(|(slot, entry)| {
                    entry.state == 2
                        && !self.referenced(slot)
                        && (self.index.resolve(entry.path.as_bytes()) == Err(VfsError::NotFound)
                            || self.overlay.iter().enumerate().any(|(other, candidate)| {
                                other != slot
                                    && candidate.state != 0
                                    && candidate.path.as_bytes() == entry.path.as_bytes()
                            }))
                })
            })
            .ok_or(VfsError::OverlayLimit)
    }

    fn create_overlay(&mut self, path: Path, mode: u32) -> Result<usize, VfsError> {
        // A whiteout can become the same live name again only after its old
        // descriptions close. Whiteouts for other image names remain reserved.
        let index = match self.find_overlay(path.as_bytes()) {
            Some(slot) if self.overlay[slot].state == 2 && !self.referenced(slot) => slot,
            _ => self.allocate_overlay()?,
        };
        self.overlay[index].data.fill(0);
        self.overlay[index].state = 1;
        self.overlay[index].path = path;
        self.overlay[index].mode = mode;
        self.overlay[index].len = 0;
        Ok(index)
    }

    fn copy_to_overlay(&mut self, path: Path, entry: Entry<'_>) -> Result<usize, VfsError> {
        let size = usize::try_from(entry.size).map_err(|_| VfsError::FileTooLarge)?;
        if size > FILE_SIZE {
            return Err(VfsError::FileTooLarge);
        }
        let index = self.create_overlay(path, entry.mode)?;
        if let Err(error) = self
            .reader
            .read_at(entry.data_offset, &mut self.overlay[index].data[..size])
        {
            self.overlay[index] = OverlayFile::empty();
            return Err(error);
        }
        self.overlay[index].len = size;
        Ok(index)
    }

    fn find_base_record(&self, path: &[u8]) -> Option<usize> {
        let mut low = 0usize;
        let mut high = self.index.count;
        while low < high {
            let middle = low + (high - low) / 2;
            let entry = self.index.entry(middle).ok()?;
            match entry.path.cmp(path) {
                core::cmp::Ordering::Less => low = middle + 1,
                core::cmp::Ordering::Greater => high = middle,
                core::cmp::Ordering::Equal => return Some(middle),
            }
        }
        None
    }

    fn resolved_path(
        &self,
        path: &[u8],
        follow_final: bool,
        allow_missing: bool,
    ) -> Result<Path, VfsError> {
        resolve_components(path, follow_final, allow_missing, |name, target| {
            if let Some(slot) = self.find_overlay(name) {
                return if self.overlay[slot].state == 2 {
                    Err(VfsError::NotFound)
                } else {
                    Ok((NodeKind::File, 0))
                };
            }
            let entry = self.index.lookup_exact(name)?;
            target[..entry.link.len()].copy_from_slice(entry.link);
            Ok((entry.kind, entry.link.len()))
        })
    }

    fn require_parent(&self, path: &[u8]) -> Result<(), VfsError> {
        let split = path
            .iter()
            .rposition(|byte| *byte == b'/')
            .ok_or(VfsError::InvalidPath)?;
        let parent = if split == 0 {
            b"/".as_slice()
        } else {
            &path[..split]
        };
        if self.index.resolve(parent)?.kind != NodeKind::Directory {
            return Err(VfsError::NotDirectory);
        }
        Ok(())
    }
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, VfsError> {
    let value = bytes
        .get(offset..offset + 2)
        .ok_or(VfsError::InvalidIndex)?;
    Ok(u16::from_le_bytes(value.try_into().unwrap()))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, VfsError> {
    let value = bytes
        .get(offset..offset + 4)
        .ok_or(VfsError::InvalidIndex)?;
    Ok(u32::from_le_bytes(value.try_into().unwrap()))
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64, VfsError> {
    let value = bytes
        .get(offset..offset + 8)
        .ok_or(VfsError::InvalidIndex)?;
    Ok(u64::from_le_bytes(value.try_into().unwrap()))
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use std::vec::Vec;

    fn index_bytes() -> Vec<u8> {
        let records = [
            Entry {
                path: b"/",
                kind: NodeKind::Directory,
                mode: 0o755,
                uid: 0,
                gid: 0,
                size: 0,
                data_offset: 0,
                link: b"",
            },
            Entry {
                path: b"/base",
                kind: NodeKind::File,
                mode: 0o644,
                uid: 0,
                gid: 0,
                size: 4,
                data_offset: 0,
                link: b"",
            },
            Entry {
                path: b"/bin",
                kind: NodeKind::Symlink,
                mode: 0o777,
                uid: 0,
                gid: 0,
                size: 0,
                data_offset: 0,
                link: b"usr/bin",
            },
            Entry {
                path: b"/loop",
                kind: NodeKind::Symlink,
                mode: 0o777,
                uid: 0,
                gid: 0,
                size: 0,
                data_offset: 0,
                link: b"loop",
            },
            Entry {
                path: b"/usr",
                kind: NodeKind::Directory,
                mode: 0o755,
                uid: 0,
                gid: 0,
                size: 0,
                data_offset: 0,
                link: b"",
            },
            Entry {
                path: b"/usr/bin",
                kind: NodeKind::Directory,
                mode: 0o755,
                uid: 0,
                gid: 0,
                size: 0,
                data_offset: 0,
                link: b"",
            },
            Entry {
                path: b"/usr/bin/sh",
                kind: NodeKind::Symlink,
                mode: 0o777,
                uid: 0,
                gid: 0,
                size: 0,
                data_offset: 0,
                link: b"tool",
            },
            Entry {
                path: b"/usr/bin/tool",
                kind: NodeKind::File,
                mode: 0o755,
                uid: 0,
                gid: 0,
                size: 4,
                data_offset: 4,
                link: b"",
            },
        ];
        let mut bytes = Vec::new();
        let mut header = [0u8; HEADER_SIZE];
        encode_header(records.len() as u32, &mut header);
        bytes.extend_from_slice(&header);
        for entry in records {
            let mut record = [0u8; RECORD_SIZE];
            encode_record(entry, &mut record).unwrap();
            bytes.extend_from_slice(&record);
        }
        bytes
    }

    struct Reader([u8; 8]);
    impl BaseReader for Reader {
        fn read_at(&mut self, offset: u64, output: &mut [u8]) -> Result<(), VfsError> {
            let offset = offset as usize;
            output.copy_from_slice(&self.0[offset..offset + output.len()]);
            Ok(())
        }
    }

    #[test]
    fn resolves_links_before_parent_components_and_checks_each_directory() {
        let bytes = index_bytes();
        let index = Index::new(&bytes).unwrap();
        assert_eq!(index.resolve(b"/bin/..").unwrap().path, b"/usr");
        assert_eq!(index.resolve(b"/bin/../base"), Err(VfsError::NotFound));
        assert_eq!(index.resolve(b"/base/.."), Err(VfsError::NotDirectory));
        assert_eq!(index.resolve(b"/absent/.."), Err(VfsError::NotFound));
        assert_eq!(index.resolve(b"/base/"), Err(VfsError::NotDirectory));
        assert_eq!(index.resolve(b"/bin//./sh").unwrap().path, b"/usr/bin/tool");
        assert_eq!(index.resolve(b"/bin/../..").unwrap().path, b"/");
        assert_eq!(index.resolve(b"/bin/../../.."), Err(VfsError::InvalidPath));
        assert_eq!(index.resolve(b"/base\0"), Err(VfsError::InvalidPath));
        let mut vfs: VirtualFs<'_, Reader, 4, 32, 4> =
            VirtualFs::new(Index::new(&bytes).unwrap(), Reader(*b"BASETOOL"));
        assert_eq!(vfs.open(b"/bin/../base", O_READ), Err(VfsError::NotFound));
        assert_eq!(vfs.open(b"/base/..", O_READ), Err(VfsError::NotDirectory));
        let fd = vfs.open(b"/bin/../new", O_WRITE | O_CREATE).unwrap();
        vfs.write(fd, b"new").unwrap();
        vfs.close(fd).unwrap();
        let fd = vfs.open(b"/usr/new", O_READ).unwrap();
        let mut output = [0; 3];
        assert_eq!(vfs.read(fd, &mut output), Ok(3));
        assert_eq!(&output, b"new");
        // Remove the link entry, preserving its target directory and children.
        vfs.unlink(b"/bin").unwrap();
        assert_eq!(vfs.open(b"/bin/sh", O_READ), Err(VfsError::NotFound));
        assert!(vfs.open(b"/usr/bin/tool", O_READ).is_ok());
    }

    #[test]
    fn validates_lookup_and_symlink_components() {
        let bytes = index_bytes();
        let index = Index::new(&bytes).unwrap();
        assert_eq!(index.resolve(b"/bin/./sh").unwrap().path, b"/usr/bin/tool");
        assert_eq!(index.resolve(b"/loop"), Err(VfsError::SymlinkLoop));
        assert_eq!(index.resolve(b"/../../base"), Err(VfsError::InvalidPath));
    }

    #[test]
    fn descriptors_never_equal_base_authority_and_overlay_is_disposable() {
        let bytes = index_bytes();
        let index = Index::new(&bytes).unwrap();
        let mut vfs: VirtualFs<'_, Reader, 4, 32, 4> = VirtualFs::new(index, Reader(*b"BASETOOL"));
        let descriptor = vfs.open(b"/base", O_READ).unwrap();
        assert_eq!(descriptor, 3);
        let mut output = [0u8; 4];
        assert_eq!(vfs.read(descriptor, &mut output).unwrap(), 4);
        assert_eq!(&output, b"BASE");
        vfs.close(descriptor).unwrap();
        let descriptor = vfs.open(b"/base", O_READ | O_WRITE).unwrap();
        vfs.write(descriptor, b"edit").unwrap();
        assert_eq!(vfs.overlay_files(), 1);
        assert_eq!(&vfs.reader.0[..4], b"BASE");
        let duplicate = vfs.duplicate(descriptor, true).unwrap();
        vfs.close_on_exec();
        assert_eq!(vfs.close(duplicate), Err(VfsError::InvalidDescriptor));
        vfs.teardown();
        assert_eq!(vfs.overlay_files(), 0);
    }

    #[test]
    fn descriptor_exhaustion_does_not_create_copy_up_or_truncate() {
        let bytes = index_bytes();
        let mut vfs: VirtualFs<'_, Reader, 4, 32, 1> =
            VirtualFs::new(Index::new(&bytes).unwrap(), Reader(*b"BASETOOL"));
        let fd = vfs.open(b"/usr/product", O_WRITE | O_CREATE).unwrap();
        vfs.write(fd, b"keep").unwrap();
        vfs.close(fd).unwrap();
        let occupied = vfs.open(b"/base", O_READ).unwrap();
        assert_eq!(
            vfs.open(b"/usr/product", O_WRITE | O_TRUNC),
            Err(VfsError::DescriptorLimit)
        );
        assert_eq!(
            vfs.open(b"/usr/absent", O_WRITE | O_CREATE),
            Err(VfsError::DescriptorLimit)
        );
        assert_eq!(vfs.open(b"/base", O_WRITE), Err(VfsError::DescriptorLimit));
        assert_eq!(vfs.overlay_files(), 1);
        vfs.close(occupied).unwrap();
        assert_eq!(vfs.open(b"/usr/absent", O_READ), Err(VfsError::NotFound));
        let fd = vfs.open(b"/usr/product", O_READ).unwrap();
        let mut bytes = [0; 4];
        assert_eq!(vfs.read(fd, &mut bytes), Ok(4));
        assert_eq!(&bytes, b"keep");
    }

    #[test]
    fn failed_copy_up_preserves_image_contents_and_overlay_capacity() {
        struct FailingReader(bool);
        impl BaseReader for FailingReader {
            fn read_at(&mut self, offset: u64, output: &mut [u8]) -> Result<(), VfsError> {
                if self.0 {
                    output.fill(b'x');
                    return Err(VfsError::Io);
                }
                Reader(*b"BASETOOL").read_at(offset, output)
            }
        }
        let bytes = index_bytes();
        let mut vfs: VirtualFs<'_, FailingReader, 1, 32, 1> =
            VirtualFs::new(Index::new(&bytes).unwrap(), FailingReader(true));
        for _ in 0..3 {
            assert_eq!(vfs.open(b"/base", O_WRITE), Err(VfsError::Io));
            assert_eq!(vfs.overlay_files(), 0);
        }
        vfs.reader.0 = false;
        let fd = vfs.open(b"/base", O_READ).unwrap();
        let mut output = [0; 4];
        assert_eq!(vfs.read(fd, &mut output), Ok(4));
        assert_eq!(&output, b"BASE");
        vfs.close(fd).unwrap();
        let fd = vfs.open(b"/base", O_READ | O_WRITE).unwrap();
        assert_eq!(vfs.read(fd, &mut output), Ok(4));
        assert_eq!(&output, b"BASE");
    }

    #[test]
    fn readonly_truncate_fails_without_changing_contents() {
        let bytes = index_bytes();
        let mut vfs: VirtualFs<'_, Reader, 2, 32, 2> =
            VirtualFs::new(Index::new(&bytes).unwrap(), Reader(*b"BASETOOL"));
        let fd = vfs.open(b"/base", O_READ | O_WRITE).unwrap();
        vfs.write(fd, b"keep").unwrap();
        vfs.close(fd).unwrap();
        assert_eq!(
            vfs.open(b"/base", O_READ | O_TRUNC),
            Err(VfsError::PermissionDenied)
        );
        let fd = vfs.open(b"/base", O_READ).unwrap();
        let mut output = [0; 4];
        assert_eq!(vfs.read(fd, &mut output), Ok(4));
        assert_eq!(&output, b"keep");
    }

    #[test]
    fn create_unlink_and_failures_are_deterministic() {
        let bytes = index_bytes();
        let index = Index::new(&bytes).unwrap();
        let mut vfs: VirtualFs<'_, Reader, 4, 8, 4> = VirtualFs::new(index, Reader(*b"BASETOOL"));
        let descriptor = vfs.open(b"/usr/new", O_WRITE | O_CREATE).unwrap();
        vfs.write(descriptor, b"new").unwrap();
        assert_eq!(
            vfs.read(descriptor, &mut [0u8; 1]),
            Err(VfsError::PermissionDenied)
        );
        assert_eq!(vfs.close(99), Err(VfsError::InvalidDescriptor));
        vfs.unlink(b"/base").unwrap();
        assert_eq!(vfs.open(b"/base", O_READ), Err(VfsError::NotFound));
        assert_eq!(
            vfs.open(b"/missing/file", O_WRITE | O_CREATE),
            Err(VfsError::NotFound)
        );
    }

    #[test]
    fn unlink_retains_open_bytes_and_recreated_names_are_independent() {
        let bytes = index_bytes();
        let mut vfs: VirtualFs<'_, Reader, 3, 32, 4> =
            VirtualFs::new(Index::new(&bytes).unwrap(), Reader(*b"BASETOOL"));
        let writer = vfs.open(b"/usr/new", O_WRITE | O_CREATE).unwrap();
        vfs.write(writer, b"old").unwrap();
        vfs.close(writer).unwrap();
        let old = vfs.open(b"/usr/new", O_READ).unwrap();
        let alias = vfs.duplicate(old, true).unwrap();
        vfs.unlink(b"/usr/new").unwrap();
        assert_eq!(vfs.unlink(b"/usr/new"), Err(VfsError::NotFound));
        assert_eq!(vfs.open(b"/usr/new", O_READ), Err(VfsError::NotFound));
        let fresh = vfs.open(b"/usr/new", O_WRITE | O_CREATE).unwrap();
        vfs.write(fresh, b"new").unwrap();
        vfs.close(fresh).unwrap();
        let fresh = vfs.open(b"/usr/new", O_READ).unwrap();
        let mut output = [0; 3];
        assert_eq!(vfs.read(fresh, &mut output), Ok(3));
        assert_eq!(&output, b"new");
        assert_eq!(vfs.read(old, &mut output[..1]), Ok(1));
        assert_eq!(&output[..1], b"o");
        vfs.close(old).unwrap();
        assert_eq!(vfs.read(alias, &mut output), Ok(2));
        assert_eq!(&output[..2], b"ld");
        assert_eq!(vfs.read(alias, &mut output), Ok(0));
        vfs.close_on_exec();
        assert_eq!(vfs.close(alias), Err(VfsError::InvalidDescriptor));
        vfs.close(fresh).unwrap();
    }

    #[test]
    fn closed_unlinked_slots_are_reusable_without_losing_image_whiteouts() {
        let bytes = index_bytes();
        let mut vfs: VirtualFs<'_, Reader, 2, 32, 2> =
            VirtualFs::new(Index::new(&bytes).unwrap(), Reader(*b"BASETOOL"));
        vfs.unlink(b"/base").unwrap();
        for _ in 0..8 {
            let fd = vfs.open(b"/usr/new", O_WRITE | O_CREATE).unwrap();
            vfs.write(fd, b"fresh").unwrap();
            vfs.unlink(b"/usr/new").unwrap();
            // Open references keep the deleted slot occupied.
            assert_eq!(
                vfs.open(b"/usr/other", O_WRITE | O_CREATE),
                Err(VfsError::OverlayLimit)
            );
            vfs.close(fd).unwrap();
            let fd = vfs.open(b"/usr/other", O_WRITE | O_CREATE).unwrap();
            vfs.close(fd).unwrap();
            vfs.unlink(b"/usr/other").unwrap();
            assert_eq!(vfs.open(b"/base", O_READ), Err(VfsError::NotFound));
        }
        assert_eq!(vfs.overlay_files(), 0);
    }

    #[test]
    fn recycling_slots_preserves_whiteouts_below_image_symlinks() {
        let bytes = index_bytes();
        let mut vfs: VirtualFs<'_, Reader, 2, 32, 2> =
            VirtualFs::new(Index::new(&bytes).unwrap(), Reader(*b"BASETOOL"));
        let fd = vfs.open(b"/bin/tool", O_READ | O_WRITE).unwrap();
        vfs.unlink(b"/bin/tool").unwrap();
        vfs.close(fd).unwrap();
        for path in [b"/usr/one", b"/usr/two", b"/usr/end"] {
            let fd = vfs.open(path, O_CREATE | O_WRITE).unwrap();
            vfs.close(fd).unwrap();
            assert_eq!(vfs.open(b"/bin/tool", O_READ), Err(VfsError::NotFound));
            vfs.unlink(path).unwrap();
        }
    }

    #[test]
    fn replacing_an_open_image_copy_does_not_leak_redundant_whiteouts() {
        let bytes = index_bytes();
        let mut vfs: VirtualFs<'_, Reader, 2, 32, 3> =
            VirtualFs::new(Index::new(&bytes).unwrap(), Reader(*b"BASETOOL"));
        let old = vfs.open(b"/base", O_READ | O_WRITE).unwrap();
        vfs.write(old, b"old!").unwrap();
        let reader = vfs.open(b"/base", O_READ).unwrap();
        vfs.unlink(b"/base").unwrap();
        let replacement = vfs.open(b"/base", O_WRITE | O_CREATE).unwrap();
        vfs.write(replacement, b"new!").unwrap();
        vfs.close(replacement).unwrap();
        let mut output = [0; 4];
        assert_eq!(vfs.read(reader, &mut output), Ok(4));
        assert_eq!(&output, b"old!");
        vfs.close(reader).unwrap();
        vfs.close(old).unwrap();
        // The old image-name whiteout is redundant while its replacement exists.
        let extra = vfs.open(b"/usr/extra", O_CREATE | O_WRITE).unwrap();
        vfs.close(extra).unwrap();
        vfs.unlink(b"/usr/extra").unwrap();
        vfs.unlink(b"/base").unwrap();
        assert_eq!(vfs.open(b"/base", O_READ), Err(VfsError::NotFound));
        for _ in 0..4 {
            let fd = vfs.open(b"/base", O_CREATE | O_WRITE).unwrap();
            vfs.close(fd).unwrap();
            vfs.unlink(b"/base").unwrap();
        }
        let extra = vfs.open(b"/usr/extra", O_CREATE | O_WRITE).unwrap();
        vfs.close(extra).unwrap();
        assert_eq!(vfs.open(b"/base", O_READ), Err(VfsError::NotFound));
    }

    #[test]
    fn overlays_are_isolated_between_domain_instances() {
        let bytes = index_bytes();
        let mut first: VirtualFs<'_, Reader, 4, 32, 4> =
            VirtualFs::new(Index::new(&bytes).unwrap(), Reader(*b"BASETOOL"));
        let mut second: VirtualFs<'_, Reader, 4, 32, 4> =
            VirtualFs::new(Index::new(&bytes).unwrap(), Reader(*b"BASETOOL"));

        let first_base = first.open(b"/base", O_READ | O_WRITE).unwrap();
        first.write(first_base, b"edit").unwrap();
        let first_new = first.open(b"/usr/new", O_WRITE | O_CREATE).unwrap();
        first.write(first_new, b"private").unwrap();

        let second_base = second.open(b"/base", O_READ).unwrap();
        let mut output = [0u8; 4];
        assert_eq!(second.read(second_base, &mut output), Ok(4));
        assert_eq!(&output, b"BASE");
        assert_eq!(second.open(b"/usr/new", O_READ), Err(VfsError::NotFound));

        first.teardown();
        assert_eq!(first.overlay_files(), 0);
        assert_eq!(second.overlay_files(), 0);
    }

    #[test]
    fn duplicated_reads_share_offset_while_separate_opens_do_not() {
        let bytes = index_bytes();
        let mut vfs: VirtualFs<'_, Reader, 4, 32, 4> =
            VirtualFs::new(Index::new(&bytes).unwrap(), Reader(*b"BASETOOL"));
        let original = vfs.open(b"/base", O_READ).unwrap();
        let alias = vfs.duplicate(original, false).unwrap();
        let separate = vfs.open(b"/base", O_READ).unwrap();
        let mut byte = [0];
        assert_eq!(vfs.read(original, &mut byte), Ok(1));
        assert_eq!(&byte, b"B");
        assert_eq!(vfs.read(alias, &mut byte), Ok(1));
        assert_eq!(&byte, b"A");
        assert_eq!(vfs.read(separate, &mut byte), Ok(1));
        assert_eq!(&byte, b"B");
        assert_eq!(vfs.read(original, &mut byte), Ok(1));
        assert_eq!(&byte, b"S");
    }

    #[test]
    fn duplicated_writes_and_reads_advance_the_same_offset() {
        let bytes = index_bytes();
        let mut vfs: VirtualFs<'_, Reader, 4, 32, 4> =
            VirtualFs::new(Index::new(&bytes).unwrap(), Reader(*b"BASETOOL"));
        let original = vfs.open(b"/base", O_READ | O_WRITE).unwrap();
        let alias = vfs.duplicate(original, false).unwrap();
        assert_eq!(vfs.write(alias, b"X"), Ok(1));
        let mut byte = [0];
        assert_eq!(vfs.read(original, &mut byte), Ok(1));
        assert_eq!(&byte, b"A");
        assert_eq!(vfs.write(original, b"Y"), Ok(1));
        assert_eq!(vfs.read(alias, &mut byte), Ok(1));
        assert_eq!(&byte, b"E");
        let separate = vfs.open(b"/base", O_READ).unwrap();
        let mut contents = [0; 4];
        assert_eq!(vfs.read(separate, &mut contents), Ok(4));
        assert_eq!(&contents, b"XAYE");
    }

    #[test]
    fn duplicated_offset_survives_close_slot_reuse_and_cloexec() {
        let bytes = index_bytes();
        let mut vfs: VirtualFs<'_, Reader, 4, 32, 4> =
            VirtualFs::new(Index::new(&bytes).unwrap(), Reader(*b"BASETOOL"));
        let original = vfs.open(b"/base", O_READ).unwrap();
        let alias = vfs.duplicate(original, true).unwrap();
        let mut byte = [0];
        assert_eq!(vfs.read(original, &mut byte), Ok(1));
        vfs.close(original).unwrap();
        // The reused slot opens a new description; the alias keeps the old one.
        let reused = vfs.open(b"/base", O_READ).unwrap();
        assert_eq!(reused, original);
        assert_eq!(vfs.read(reused, &mut byte), Ok(1));
        assert_eq!(&byte, b"B");
        assert_eq!(vfs.read(alias, &mut byte), Ok(1));
        assert_eq!(&byte, b"A");
        vfs.close_on_exec();
        assert_eq!(vfs.read(alias, &mut byte), Err(VfsError::InvalidDescriptor));
        assert_eq!(vfs.read(reused, &mut byte), Ok(1));
        assert_eq!(&byte, b"A");
    }

    #[test]
    fn closing_every_alias_releases_its_description() {
        let bytes = index_bytes();
        let mut vfs: VirtualFs<'_, Reader, 4, 32, 4> =
            VirtualFs::new(Index::new(&bytes).unwrap(), Reader(*b"BASETOOL"));
        for _ in 0..8 {
            let original = vfs.open(b"/base", O_READ).unwrap();
            let alias = vfs.duplicate(original, false).unwrap();
            let mut byte = [0];
            assert_eq!(vfs.read(alias, &mut byte), Ok(1));
            vfs.close(original).unwrap();
            vfs.close(alias).unwrap();
            let separate = vfs.open(b"/base", O_READ).unwrap();
            assert_eq!(vfs.read(separate, &mut byte), Ok(1));
            assert_eq!(&byte, b"B");
            vfs.close(separate).unwrap();
        }
    }
}
