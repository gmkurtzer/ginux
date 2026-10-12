use core::{
    mem::MaybeUninit,
    sync::atomic::{AtomicBool, Ordering},
};

pub(crate) const FILE_CAPACITY: usize = 64;
const FILE_NAME: &[u8] = b"ginux-ready";
const FILE_INODE: u64 = 18;

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
    length: u8,
    data: [u8; FILE_CAPACITY],
}

struct Tmpfs {
    generation: u32,
    file: Option<File>,
}

impl Tmpfs {
    const fn new() -> Self {
        Self {
            generation: 0,
            file: None,
        }
    }

    fn create(&mut self) -> Option<FileHandle> {
        if self.file.is_some() {
            return None;
        }
        let generation = self.generation.checked_add(1)?;
        let handle = FileHandle {
            generation,
            inode: FILE_INODE,
        };
        self.generation = generation;
        self.file = Some(File {
            handle,
            length: 0,
            data: [0; FILE_CAPACITY],
        });
        Some(handle)
    }

    fn open(&self) -> Option<FileHandle> {
        self.file.map(|file| file.handle)
    }

    fn file(&self, handle: FileHandle) -> Option<&File> {
        self.file.as_ref().filter(|file| file.handle == handle)
    }

    fn file_mut(&mut self, handle: FileHandle) -> Option<&mut File> {
        self.file.as_mut().filter(|file| file.handle == handle)
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
        let file = self.file_mut(handle)?;
        file.data[offset..end].copy_from_slice(input);
        file.length = file.length.max(u8::try_from(end).ok()?);
        Some(input.len())
    }

    fn unlink(&mut self, handle: FileHandle) -> bool {
        let Some(file) = self.file.as_mut() else {
            return false;
        };
        if file.handle != handle {
            return false;
        }
        for byte in &mut file.data {
            unsafe { core::ptr::write_volatile(byte, 0) };
        }
        unsafe { core::ptr::write_volatile(&mut file.length, 0) };
        self.file = None;
        true
    }

    fn clean(&self) -> bool {
        self.file.is_none()
    }
}

pub(crate) fn verify_contract() -> bool {
    let mut tmpfs = Tmpfs::new();
    let Some(first) = tmpfs.create() else {
        return false;
    };
    let mut output = [0_u8; FILE_CAPACITY];
    if tmpfs.create().is_some()
        || tmpfs.write(first, 0, b"ready\n") != Some(6)
        || tmpfs.read(first, 0, &mut output[..6]) != Some(6)
        || &output[..6] != b"ready\n"
        || tmpfs.write(first, FILE_CAPACITY, b"x").is_some()
        || !tmpfs.unlink(first)
        || tmpfs.read(first, 0, &mut output[..1]).is_some()
    {
        return false;
    }
    let Some(second) = tmpfs.create() else {
        return false;
    };
    second.generation > first.generation
        && second.inode == first.inode
        && tmpfs.read(second, 0, &mut output) == Some(0)
        && !tmpfs.unlink(first)
        && tmpfs.unlink(second)
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
    (name == FILE_NAME).then_some(())?;
    live_mut()?.create()
}

pub(crate) fn open(name: &[u8]) -> Option<FileHandle> {
    (name == FILE_NAME).then_some(())?;
    live()?.open()
}

pub(crate) fn from_generation(generation: u32) -> Option<FileHandle> {
    let handle = live()?.open()?;
    (handle.generation == generation).then_some(handle)
}

pub(crate) fn generation(handle: FileHandle) -> u32 {
    handle.generation
}

pub(crate) fn inode(handle: FileHandle) -> u64 {
    handle.inode
}

pub(crate) fn entry() -> Option<(FileHandle, &'static [u8])> {
    live()?.open().map(|handle| (handle, FILE_NAME))
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
    name == FILE_NAME && live_mut().is_some_and(|tmpfs| tmpfs.unlink(handle))
}

pub(crate) fn teardown() -> bool {
    let Some(tmpfs) = live_mut() else {
        return false;
    };
    if let Some(handle) = tmpfs.open() {
        if !tmpfs.unlink(handle) {
            return false;
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
