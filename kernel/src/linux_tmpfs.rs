use crate::{
    arch::x86_64::cpu,
    control_abi,
    memory::{self, FrameLease, FrameOwner},
};
use core::sync::atomic::{AtomicBool, Ordering};

pub(crate) const FILE_CAPACITY: usize = 64;
const PAGE_SIZE: usize = 4096;
const FILE_LIMIT: usize = 4;
const NAME_CAPACITY: usize = 64;
const OBJECT_PAGE_LIMIT: usize = 4;
const TOTAL_PAGE_LIMIT: usize = 8;
const OBJECT_CAPACITY: usize = OBJECT_PAGE_LIMIT * PAGE_SIZE;
const TOTAL_CAPACITY: usize = TOTAL_PAGE_LIMIT * PAGE_SIZE;
const FILE_NAME: &[u8] = b"ginux-ready";
const FIRST_FILE_INODE: u64 = 18;
const HOST_INSTANCE: usize = control_abi::CONTAINER_LIMIT;
const INSTANCE_LIMIT: usize = control_abi::CONTAINER_LIMIT + 1;

static INSTALLED: AtomicBool = AtomicBool::new(false);
static mut LIVE_TMPFS: [Tmpfs; INSTANCE_LIMIT] = [const { Tmpfs::new() }; INSTANCE_LIMIT];

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
    length: usize,
    page_count: u8,
    pages: [FrameLease; OBJECT_PAGE_LIMIT],
    published: bool,
}

struct Tmpfs {
    generation: u32,
    used: usize,
    allocated_pages: usize,
    files: [Option<File>; FILE_LIMIT],
}

impl Tmpfs {
    const fn new() -> Self {
        Self {
            generation: 0,
            used: 0,
            allocated_pages: 0,
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
            page_count: 0,
            pages: [FrameLease::EMPTY; OBJECT_PAGE_LIMIT],
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
        let start = offset.min(file.length);
        let end = start.saturating_add(output.len()).min(file.length);
        let mut copied = 0;
        while start + copied < end {
            let position = start + copied;
            let page_index = position / PAGE_SIZE;
            let page_offset = position % PAGE_SIZE;
            let length = (end - position).min(PAGE_SIZE - page_offset);
            let address = file.pages.get(page_index)?.direct_map_address()?;
            let source = unsafe {
                core::slice::from_raw_parts((address + page_offset) as *const u8, length)
            };
            output[copied..copied + length].copy_from_slice(source);
            copied += length;
        }
        Some(copied)
    }

    fn write(&mut self, handle: FileHandle, offset: usize, input: &[u8]) -> Option<usize> {
        let slot = self
            .files
            .iter()
            .position(|file| file.is_some_and(|file| file.handle == handle))?;
        if input.is_empty() {
            return Some(0);
        }
        let end = offset.checked_add(input.len())?;
        if end > OBJECT_CAPACITY {
            return None;
        }
        let file = self.files[slot].as_ref()?;
        if file.published && file.name() != FILE_NAME {
            return None;
        }
        let old_length = file.length;
        let old_page_count = usize::from(file.page_count);
        let required_page_count = end.div_ceil(PAGE_SIZE);
        let additional_page_count = required_page_count.saturating_sub(old_page_count);
        if self.allocated_pages.checked_add(additional_page_count)? > TOTAL_PAGE_LIMIT {
            return None;
        }

        let mut additions = [FrameLease::EMPTY; OBJECT_PAGE_LIMIT];
        let mut addition_count = 0;
        while addition_count < additional_page_count {
            let Some([frame]) = memory::allocate_frames::<1>(FrameOwner::KERNEL) else {
                if addition_count != 0 {
                    let _ = memory::scrub_and_release_frames(
                        FrameOwner::KERNEL,
                        &additions[..addition_count],
                    );
                }
                return None;
            };
            additions[addition_count] = frame;
            addition_count += 1;
        }

        let mut pages = file.pages;
        pages[old_page_count..required_page_count].copy_from_slice(&additions[..addition_count]);
        if pages[..required_page_count]
            .iter()
            .any(|frame| frame.direct_map_address().is_none())
        {
            if addition_count != 0 {
                let _ = memory::scrub_and_release_frames(
                    FrameOwner::KERNEL,
                    &additions[..addition_count],
                );
            }
            return None;
        }

        let mut copied = 0;
        while copied < input.len() {
            let position = offset + copied;
            let page_index = position / PAGE_SIZE;
            let page_offset = position % PAGE_SIZE;
            let length = (input.len() - copied).min(PAGE_SIZE - page_offset);
            let address = pages[page_index].direct_map_address()?;
            let output = unsafe {
                core::slice::from_raw_parts_mut((address + page_offset) as *mut u8, length)
            };
            output.copy_from_slice(&input[copied..copied + length]);
            copied += length;
        }

        let file = self.files[slot].as_mut()?;
        file.pages = pages;
        file.page_count = required_page_count as u8;
        file.length = old_length.max(end);
        self.allocated_pages += addition_count;
        self.used += end.saturating_sub(old_length);
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
        let page_count = usize::from(file.page_count);
        if page_count != 0
            && !memory::scrub_and_release_frames(FrameOwner::KERNEL, &file.pages[..page_count])
        {
            return false;
        }
        self.used -= file.length;
        self.allocated_pages -= page_count;
        for byte in &mut file.name {
            unsafe { core::ptr::write_volatile(byte, 0) };
        }
        unsafe {
            core::ptr::write_volatile(&mut file.length, 0);
            core::ptr::write_volatile(&mut file.name_length, 0);
            core::ptr::write_volatile(&mut file.page_count, 0);
            core::ptr::write_volatile(&mut file.published, false);
        }
        file.pages.fill(FrameLease::EMPTY);
        self.files[slot] = None;
        true
    }

    fn clean(&self) -> bool {
        self.used == 0 && self.allocated_pages == 0 && self.files.iter().all(Option::is_none)
    }
}

impl File {
    fn name(&self) -> &[u8] {
        &self.name[..usize::from(self.name_length)]
    }
}

pub(crate) fn verify_contract() -> bool {
    let Some(frames_before) = memory::owner_live_frame_count(FrameOwner::KERNEL) else {
        return false;
    };
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
        || tmpfs.write(staged, PAGE_SIZE - 2, b"abcd") != Some(4)
        || tmpfs.read(staged, PAGE_SIZE - 2, &mut output[..4]) != Some(4)
        || &output[..4] != b"abcd"
        || !tmpfs.publish(staged)
        || tmpfs.publish(staged)
        || tmpfs.find(staged_name, true) != Some(staged)
        || tmpfs.write(staged, 0, b"x").is_some()
    {
        return false;
    }

    let aborted_name = b"sha256:bbbbbbbb";
    let Some(aborted) = tmpfs.create(aborted_name, false) else {
        return false;
    };
    if tmpfs.write(aborted, OBJECT_CAPACITY - 1, b"z") != Some(1) {
        return false;
    }
    let quota_name = b"sha256:cccccccc";
    let Some(quota) = tmpfs.create(quota_name, false) else {
        return false;
    };
    if tmpfs.write(quota, PAGE_SIZE - 1, b"q") != Some(1)
        || tmpfs.write(quota, PAGE_SIZE, b"x").is_some()
        || tmpfs.find(aborted_name, true).is_some()
        || !tmpfs.unlink(quota)
        || !tmpfs.unlink(aborted)
        || tmpfs.find(aborted_name, false).is_some()
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
        && memory::owner_live_frame_count(FrameOwner::KERNEL) == Some(frames_before)
}

pub(crate) fn install() -> bool {
    if INSTALLED.load(Ordering::Acquire) {
        return false;
    }
    let mut index = 0;
    while index < INSTANCE_LIMIT {
        unsafe {
            core::ptr::addr_of_mut!(LIVE_TMPFS[index]).write(Tmpfs::new());
        }
        index += 1;
    }
    INSTALLED.store(true, Ordering::Release);
    (0..INSTANCE_LIMIT).all(|index| live_at(index).is_some_and(Tmpfs::clean))
}

pub(crate) fn create(name: &[u8]) -> Option<FileHandle> {
    (name == FILE_NAME).then_some(())?;
    live_mut()?.create(name, true)
}

pub(crate) fn open(name: &[u8]) -> Option<FileHandle> {
    live()?.find(name, true)
}

pub(crate) fn stage_object(name: &[u8]) -> Option<FileHandle> {
    (name != FILE_NAME).then_some(())?;
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
    let Some(frames_before) = memory::owner_live_frame_count(FrameOwner::KERNEL) else {
        return false;
    };
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
        || write(first, PAGE_SIZE - 2, b"abcd") != Some(4)
        || !publish_object(first)
        || open(first_name) != Some(first)
        || abort_object(first)
        || write(first, 0, b"x").is_some()
    {
        return false;
    }
    let Some(second) = stage_object(second_name) else {
        return false;
    };
    let Some(overflow) = stage_object(overflow_name) else {
        return false;
    };
    if write(second, OBJECT_CAPACITY - 1, b"s") != Some(1)
        || write(overflow, 2 * PAGE_SIZE - 1, b"o") != Some(1)
        || write(overflow, 2 * PAGE_SIZE, b"x").is_some()
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
        && memory::owner_live_frame_count(FrameOwner::KERNEL) == Some(frames_before)
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
    live()?.file(handle).map(|file| file.length)
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
    live_at(current_instance())
}

fn live_at(index: usize) -> Option<&'static Tmpfs> {
    if !INSTALLED.load(Ordering::Acquire) {
        return None;
    }
    let tmpfs = unsafe { core::ptr::addr_of!(LIVE_TMPFS).cast::<Tmpfs>().add(index) };
    Some(unsafe { &*tmpfs })
}

fn live_mut() -> Option<&'static mut Tmpfs> {
    if !INSTALLED.load(Ordering::Acquire) {
        return None;
    }
    let index = current_instance();
    let tmpfs = unsafe {
        core::ptr::addr_of_mut!(LIVE_TMPFS)
            .cast::<Tmpfs>()
            .add(index)
    };
    Some(unsafe { &mut *tmpfs })
}

fn current_instance() -> usize {
    cpu::current_container_slot().unwrap_or(HOST_INSTANCE)
}
