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
        let mut current = normalize(path)?;
        for _ in 0..16 {
            let mut position = 1usize;
            let mut followed = false;
            while position <= current.len {
                let component_end = current.bytes[position..current.len]
                    .iter()
                    .position(|byte| *byte == b'/')
                    .map_or(current.len, |offset| position + offset);
                let prefix = &current.bytes[..component_end];
                let entry = self.lookup_exact(prefix)?;
                if entry.kind == NodeKind::Symlink {
                    let mut replacement = Path::new();
                    if entry.link.first() == Some(&b'/') {
                        replacement.extend(entry.link)?;
                    } else {
                        let parent_end = prefix.iter().rposition(|byte| *byte == b'/').unwrap_or(0);
                        if parent_end == 0 {
                            replacement.extend(b"/")?;
                        } else {
                            replacement.extend(&prefix[..parent_end])?;
                            replacement.extend(b"/")?;
                        }
                        replacement.extend(entry.link)?;
                    }
                    if component_end < current.len {
                        replacement.extend(&current.bytes[component_end..current.len])?;
                    }
                    current = normalize(replacement.as_bytes())?;
                    followed = true;
                    break;
                }
                if component_end < current.len && entry.kind != NodeKind::Directory {
                    return Err(VfsError::NotDirectory);
                }
                if component_end == current.len {
                    return Ok(entry);
                }
                position = component_end + 1;
            }
            if !followed {
                break;
            }
        }
        Err(VfsError::SymlinkLoop)
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

fn normalize(input: &[u8]) -> Result<Path, VfsError> {
    if input.first() != Some(&b'/') || input.contains(&0) {
        return Err(VfsError::InvalidPath);
    }
    let mut output = Path::new();
    output.extend(b"/")?;
    let mut starts = [0usize; 64];
    let mut components = 0usize;
    for component in input.split(|byte| *byte == b'/') {
        if component.is_empty() || component == b"." {
            continue;
        }
        if component == b".." {
            if components == 0 {
                return Err(VfsError::InvalidPath);
            }
            components -= 1;
            output.len = starts[components];
            continue;
        }
        if components == starts.len() {
            return Err(VfsError::InvalidPath);
        }
        if output.len > 1 {
            output.extend(b"/")?;
        }
        starts[components] = if output.len == 1 { 1 } else { output.len - 1 };
        components += 1;
        output.extend(component)?;
    }
    Ok(output)
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
    offset: usize,
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
            offset: 0,
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
        }
    }

    pub fn open(&mut self, path: &[u8], flags: u32) -> Result<i32, VfsError> {
        let path = normalize(path)?;
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

        self.descriptors[slot] = Descriptor {
            open: true,
            overlay,
            object,
            offset: 0,
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
        let available = if state.overlay {
            self.overlay[state.object].len.saturating_sub(state.offset)
        } else {
            let entry = self.index.entry(state.object)?;
            usize::try_from(entry.size)
                .map_err(|_| VfsError::FileTooLarge)?
                .saturating_sub(state.offset)
        };
        let length = available.min(output.len());
        if state.overlay {
            output[..length].copy_from_slice(
                &self.overlay[state.object].data[state.offset..state.offset + length],
            );
        } else {
            let entry = self.index.entry(state.object)?;
            self.reader.read_at(
                entry.data_offset + state.offset as u64,
                &mut output[..length],
            )?;
        }
        self.descriptors[slot].offset += length;
        Ok(length)
    }

    pub fn write(&mut self, descriptor: i32, input: &[u8]) -> Result<usize, VfsError> {
        let slot = self.descriptor(descriptor)?;
        let state = self.descriptors[slot];
        if !state.writable || !state.overlay {
            return Err(VfsError::PermissionDenied);
        }
        let end = state
            .offset
            .checked_add(input.len())
            .ok_or(VfsError::FileTooLarge)?;
        if end > FILE_SIZE {
            return Err(VfsError::FileTooLarge);
        }
        self.overlay[state.object].data[state.offset..end].copy_from_slice(input);
        self.overlay[state.object].len = self.overlay[state.object].len.max(end);
        self.descriptors[slot].offset = end;
        Ok(input.len())
    }

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
        self.descriptors[slot] = Descriptor::empty();
        Ok(())
    }

    pub fn close_on_exec(&mut self) {
        for descriptor in &mut self.descriptors {
            if descriptor.open && descriptor.close_on_exec {
                *descriptor = Descriptor::empty();
            }
        }
    }

    pub fn unlink(&mut self, path: &[u8]) -> Result<(), VfsError> {
        let path = normalize(path)?;
        if let Some(index) = self.find_overlay(path.as_bytes()) {
            self.overlay[index].state = 2;
            self.overlay[index].len = 0;
            return Ok(());
        }
        let entry = self.index.resolve(path.as_bytes())?;
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
            .position(|entry| entry.state != 0 && entry.path.as_bytes() == path)
    }

    fn allocate_overlay(&self) -> Result<usize, VfsError> {
        self.overlay
            .iter()
            .position(|entry| entry.state == 0)
            .ok_or(VfsError::OverlayLimit)
    }

    fn create_overlay(&mut self, path: Path, mode: u32) -> Result<usize, VfsError> {
        let index = self.allocate_overlay()?;
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
}
