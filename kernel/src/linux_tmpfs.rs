use core::{
    mem::MaybeUninit,
    sync::atomic::{AtomicBool, Ordering},
};

pub(crate) const FILE_CAPACITY: usize = 64;
const FILE_LIMIT: usize = 4;
const NAME_CAPACITY: usize = 64;
const TOTAL_CAPACITY: usize = FILE_CAPACITY * 2;
const FILE_NAME: &[u8] = b"ginux-ready";
const FIRST_FILE_INODE: u64 = 18;

static INSTALLED: AtomicBool = AtomicBool::new(false);
static mut LIVE_TMPFS: MaybeUninit<Tmpfs> = MaybeUninit::uninit();

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct FileHandle {
    generation: u32,
    inode: u64,
}

#[derive(Clone, Copy)]
struct File {
    handle: FileHandle,
    name_length: u8,
    name: [u8; NAME_CAPACITY],
    length: u8,
    data: [u8; FILE_CAPACITY],
    published: bool,
}

struct Tmpfs {
    generation: u32,
    used: usize,
    files: [Option<File>; FILE_LIMIT],
}

impl Tmpfs {
    const fn new() -> Self {
        Self {
            generation: 0,
            used: 0,
            files: [None; FILE_LIMIT],
        }
    }

    fn valid_name(name: &[u8]) -> bool {
        !name.is_empty()
            && name.len() <= NAME_CAPACITY
            && name
                .iter()
                .all(|byte| byte.is_ascii_graphic() && *byte != b'/')
    }

    fn create(&mut self, name: &[u8], published: bool) -> Option<FileHandle> {
        if !Self::valid_name(name) || self.find(name, false).is_some() {
            return None;
        }
        let slot = self.files.iter().position(Option::is_none)?;
        let generation = self.generation.checked_add(1)?;
        let mut stored_name = [0_u8; NAME_CAPACITY];
        stored_name[..name.len()].copy_from_slice(name);
        let handle = FileHandle {
            generation,
            inode: FIRST_FILE_INODE + slot as u64,
        };
        self.generation = generation;
        self.files[slot] = Some(File {
            handle,
            name_length: name.len() as u8,
            name: stored_name,
            length: 0,
            data: [0; FILE_CAPACITY],
            published,
        });
        Some(handle)
    }

    fn find(&self, name: &[u8], published_only: bool) -> Option<FileHandle> {
        self.files.iter().flatten().find_map(|file| {
            (!published_only || file.published)
                .then_some(())
                .filter(|_| file.name() == name)
                .map(|_| file.handle)
        })
    }

    fn file(&self, handle: FileHandle) -> Option<&File> {
        self.files
            .iter()
            .flatten()
            .find(|file| file.handle == handle)
    }

    fn file_mut(&mut self, handle: FileHandle) -> Option<&mut File> {
        self.files
            .iter_mut()
            .flatten()
            .find(|file| file.handle == handle)
    }

    fn read(&self, handle: FileHandle, offset: usize, output: &mut [u8]) -> Option<usize> {
        let file = self.file(handle)?;
        let start = offset.min(usize::from(file.length));
        let end = start
            .saturating_add(output.len())
            .min(usize::from(file.length));
        output[..end - start].copy_from_slice(&file.data[start..end]);
        Some(end - start)
    }

    fn write(&mut self, handle: FileHandle, offset: usize, input: &[u8]) -> Option<usize> {
        let end = offset.checked_add(input.len())?;
        if end > FILE_CAPACITY {
            return None;
        }
        let old_length = usize::from(self.file(handle)?.length);
        let growth = end.saturating_sub(old_length);
        if self.used.checked_add(growth)? > TOTAL_CAPACITY {
            return None;
        }
        let file = self.file_mut(handle)?;
        file.data[offset..end].copy_from_slice(input);
        file.length = file.length.max(u8::try_from(end).ok()?);
        self.used += growth;
        Some(input.len())
    }

    fn publish(&mut self, handle: FileHandle) -> bool {
        let Some(file) = self.file_mut(handle) else {
            return false;
        };
        if file.published {
            return false;
        }
        file.published = true;
        true
    }

    fn unlink(&mut self, handle: FileHandle) -> bool {
        let Some(slot) = self
            .files
            .iter()
            .position(|file| file.is_some_and(|file| file.handle == handle))
        else {
            return false;
        };
        let Some(file) = self.files[slot].as_mut() else {
            return false;
        };
        self.used -= usize::from(file.length);
        for byte in &mut file.data {
            unsafe { core::ptr::write_volatile(byte, 0) };
        }
        for byte in &mut file.name {
            unsafe { core::ptr::write_volatile(byte, 0) };
        }
        unsafe {
            core::ptr::write_volatile(&mut file.length, 0);
            core::ptr::write_volatile(&mut file.name_length, 0);
            core::ptr::write_volatile(&mut file.published, false);
        }
        self.files[slot] = None;
        true
    }

    fn clean(&self) -> bool {
        self.used == 0 && self.files.iter().all(Option::is_none)
    }
}

impl File {
    fn name(&self) -> &[u8] {
        &self.name[..usize::from(self.name_length)]
    }
}

pub(crate) fn verify_contract() -> bool {
    let mut tmpfs = Tmpfs::new();
    let Some(first) = tmpfs.create(FILE_NAME, true) else {
        return false;
    };
    let mut output = [0_u8; FILE_CAPACITY];
    if tmpfs.write(first, 0, b"ready\n") != Some(6)
        || tmpfs.read(first, 0, &mut output[..6]) != Some(6)
        || &output[..6] != b"ready\n"
        || tmpfs.create(FILE_NAME, true).is_some()
        || tmpfs.create(b"bad/name", true).is_some()
    {
        return false;
    }

    let staged_name = b"sha256:aaaaaaaa";
    let Some(staged) = tmpfs.create(staged_name, false) else {
        return false;
    };
    if tmpfs.find(staged_name, true).is_some()
        || tmpfs.write(staged, 0, &[0x5a; FILE_CAPACITY]) != Some(FILE_CAPACITY)
        || !tmpfs.publish(staged)
        || tmpfs.publish(staged)
        || tmpfs.find(staged_name, true) != Some(staged)
    {
        return false;
    }

    let aborted_name = b"sha256:bbbbbbbb";
    let Some(aborted) = tmpfs.create(aborted_name, false) else {
        return false;
    };
    let remaining = TOTAL_CAPACITY - 6 - FILE_CAPACITY;
    if tmpfs.write(aborted, 0, &[0x33; FILE_CAPACITY][..remaining]) != Some(remaining)
        || tmpfs.write(aborted, remaining, b"x").is_some()
        || tmpfs.find(aborted_name, true).is_some()
        || !tmpfs.unlink(aborted)
        || tmpfs.find(aborted_name, false).is_some()
        || tmpfs.used != 6 + FILE_CAPACITY
    {
        return false;
    }

    let stale = first;
    if !tmpfs.unlink(first) || tmpfs.read(stale, 0, &mut output[..1]).is_some() {
        return false;
    }
    let Some(replacement) = tmpfs.create(FILE_NAME, true) else {
        return false;
    };
    replacement.generation > stale.generation
        && replacement.inode == stale.inode
        && tmpfs.read(replacement, 0, &mut output) == Some(0)
        && !tmpfs.unlink(stale)
        && tmpfs.unlink(replacement)
        && tmpfs.unlink(staged)
        && tmpfs.clean()
}

pub(crate) fn install() -> bool {
    if INSTALLED.load(Ordering::Acquire) {
        return false;
    }
    unsafe {
        core::ptr::addr_of_mut!(LIVE_TMPFS).write(MaybeUninit::new(Tmpfs::new()));
    }
    INSTALLED.store(true, Ordering::Release);
    live().is_some_and(Tmpfs::clean)
}

pub(crate) fn create(name: &[u8]) -> Option<FileHandle> {
    live_mut()?.create(name, true)
}

pub(crate) fn open(name: &[u8]) -> Option<FileHandle> {
    live()?.find(name, true)
}

pub(crate) fn stage_object(name: &[u8]) -> Option<FileHandle> {
    live_mut()?.create(name, false)
}

pub(crate) fn publish_object(handle: FileHandle) -> bool {
    live_mut().is_some_and(|tmpfs| tmpfs.publish(handle))
}

pub(crate) fn abort_object(handle: FileHandle) -> bool {
    let staged = live()
        .and_then(|tmpfs| tmpfs.file(handle))
        .is_some_and(|file| !file.published);
    staged && live_mut().is_some_and(|tmpfs| tmpfs.unlink(handle))
}

pub(crate) fn usage() -> (usize, usize) {
    live().map_or((0, TOTAL_CAPACITY), |tmpfs| (tmpfs.used, TOTAL_CAPACITY))
}

pub(crate) fn verify_live_object_contract() -> bool {
    let first_name = b"sha256:live-a";
    let second_name = b"sha256:live-b";
    let overflow_name = b"sha256:overflow";
    if usage() != (0, TOTAL_CAPACITY) {
        return false;
    }
    let Some(first) = stage_object(first_name) else {
        return false;
    };
    if open(first_name).is_some()
        || write(first, 0, &[0xa5; FILE_CAPACITY]) != Some(FILE_CAPACITY)
        || !publish_object(first)
        || open(first_name) != Some(first)
        || abort_object(first)
    {
        return false;
    }
    let Some(second) = stage_object(second_name) else {
        return false;
    };
    let Some(overflow) = stage_object(overflow_name) else {
        return false;
    };
    if write(second, 0, &[0x5a; FILE_CAPACITY]) != Some(FILE_CAPACITY)
        || write(overflow, 0, b"x").is_some()
        || !abort_object(overflow)
        || !abort_object(second)
        || !unlink(first_name, first)
    {
        return false;
    }
    usage() == (0, TOTAL_CAPACITY)
        && open(first_name).is_none()
        && open(second_name).is_none()
        && open(overflow_name).is_none()
}

pub(crate) fn from_generation(generation: u32) -> Option<FileHandle> {
    live()?
        .files
        .iter()
        .flatten()
        .find(|file| file.published && file.handle.generation == generation)
        .map(|file| file.handle)
}

pub(crate) fn generation(handle: FileHandle) -> u32 {
    handle.generation
}

pub(crate) fn inode(handle: FileHandle) -> u64 {
    handle.inode
}

pub(crate) fn entry() -> Option<(FileHandle, &'static [u8])> {
    open(FILE_NAME).map(|handle| (handle, FILE_NAME))
}

pub(crate) fn size(handle: FileHandle) -> Option<usize> {
    live()?.file(handle).map(|file| usize::from(file.length))
}

pub(crate) fn read(handle: FileHandle, offset: usize, output: &mut [u8]) -> Option<usize> {
    live()?.read(handle, offset, output)
}

pub(crate) fn write(handle: FileHandle, offset: usize, input: &[u8]) -> Option<usize> {
    live_mut()?.write(handle, offset, input)
}

pub(crate) fn unlink(name: &[u8], handle: FileHandle) -> bool {
    live()
        .and_then(|tmpfs| tmpfs.file(handle))
        .is_some_and(|file| file.published && file.name() == name)
        && live_mut().is_some_and(|tmpfs| tmpfs.unlink(handle))
}

pub(crate) fn teardown() -> bool {
    let Some(tmpfs) = live_mut() else {
        return false;
    };
    for slot in 0..FILE_LIMIT {
        if let Some(handle) = tmpfs.files[slot].map(|file| file.handle) {
            if !tmpfs.unlink(handle) {
                return false;
            }
        }
    }
    tmpfs.clean()
}

fn live() -> Option<&'static Tmpfs> {
    if !INSTALLED.load(Ordering::Acquire) {
        return None;
    }
    Some(unsafe { &*core::ptr::addr_of!(LIVE_TMPFS).cast::<Tmpfs>() })
}

fn live_mut() -> Option<&'static mut Tmpfs> {
    if !INSTALLED.load(Ordering::Acquire) {
        return None;
    }
    Some(unsafe { &mut *core::ptr::addr_of_mut!(LIVE_TMPFS).cast::<Tmpfs>() })
}
