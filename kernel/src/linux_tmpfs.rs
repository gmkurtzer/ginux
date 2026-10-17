use crate::{
    arch::x86_64::cpu,
    control_abi,
    memory::{self, FrameLease, FrameOwner},
};
use core::sync::atomic::{AtomicBool, Ordering};

pub(crate) const FILE_CAPACITY: usize = 64;
const PAGE_SIZE: usize = 4096;
const NAME_CAPACITY: usize = 64;
const DATA_CAPACITY: usize = PAGE_SIZE - core::mem::size_of::<FrameLease>();
const FILE_NAME: &[u8] = b"ginux-ready";
const FIRST_FILE_INODE: u64 = 18;
const HOST_INSTANCE: usize = control_abi::DOMAIN_LIMIT;
const INSTANCE_LIMIT: usize = control_abi::DOMAIN_LIMIT + 1;

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
    page_count: usize,
    first_page: FrameLease,
    last_page: FrameLease,
    cached_page: FrameLease,
    cached_page_index: usize,
    published: bool,
}

#[repr(C)]
struct DataPage {
    next: FrameLease,
    bytes: [u8; DATA_CAPACITY],
}

const FILES_PER_PAGE: usize =
    (PAGE_SIZE - core::mem::size_of::<FrameLease>()) / core::mem::size_of::<Option<File>>();

#[repr(C)]
struct FilePage {
    next: FrameLease,
    files: [Option<File>; FILES_PER_PAGE],
}

struct Tmpfs {
    generation: u32,
    used: usize,
    allocated_pages: usize,
    first_file_page: FrameLease,
    file_page_count: usize,
}

impl Tmpfs {
    const fn new() -> Self {
        Self {
            generation: 0,
            used: 0,
            allocated_pages: 0,
            first_file_page: FrameLease::EMPTY,
            file_page_count: 0,
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
        let (page, slot) = self.free_file_slot()?;
        let generation = self.generation.checked_add(1)?;
        let mut stored_name = [0_u8; NAME_CAPACITY];
        stored_name[..name.len()].copy_from_slice(name);
        let handle = FileHandle {
            generation,
            inode: FIRST_FILE_INODE.checked_add(u64::from(generation) - 1)?,
        };
        self.generation = generation;
        file_page_mut(page)?.files[slot] = Some(File {
            handle,
            name_length: name.len() as u8,
            name: stored_name,
            length: 0,
            page_count: 0,
            first_page: FrameLease::EMPTY,
            last_page: FrameLease::EMPTY,
            cached_page: FrameLease::EMPTY,
            cached_page_index: 0,
            published,
        });
        Some(handle)
    }

    fn find(&self, name: &[u8], published_only: bool) -> Option<FileHandle> {
        self.find_file(|file| (!published_only || file.published) && file.name() == name)
            .and_then(|(page, slot)| file_page(page)?.files[slot].map(|file| file.handle))
    }

    fn file(&self, handle: FileHandle) -> Option<&File> {
        let (page, slot) = self.find_file(|file| file.handle == handle)?;
        file_page(page)?.files[slot].as_ref()
    }

    fn file_mut(&mut self, handle: FileHandle) -> Option<&mut File> {
        let (page, slot) = self.find_file(|file| file.handle == handle)?;
        file_page_mut(page)?.files[slot].as_mut()
    }

    fn find_file(&self, predicate: impl Fn(&File) -> bool) -> Option<(FrameLease, usize)> {
        let mut page = self.first_file_page;
        while page != FrameLease::EMPTY {
            let metadata = file_page(page)?;
            if let Some(slot) = metadata
                .files
                .iter()
                .position(|file| file.as_ref().is_some_and(&predicate))
            {
                return Some((page, slot));
            }
            page = metadata.next;
        }
        None
    }

    fn free_file_slot(&mut self) -> Option<(FrameLease, usize)> {
        let mut page = self.first_file_page;
        let mut last = FrameLease::EMPTY;
        while page != FrameLease::EMPTY {
            let metadata = file_page(page)?;
            if let Some(slot) = metadata.files.iter().position(Option::is_none) {
                return Some((page, slot));
            }
            last = page;
            page = metadata.next;
        }
        let page = allocate_file_page()?;
        if last == FrameLease::EMPTY {
            self.first_file_page = page;
        } else {
            file_page_mut(last)?.next = page;
        }
        self.file_page_count += 1;
        Some((page, 0))
    }

    fn read(&mut self, handle: FileHandle, offset: usize, output: &mut [u8]) -> Option<usize> {
        let file = self.file_mut(handle)?;
        let start = offset.min(file.length);
        let end = start.saturating_add(output.len()).min(file.length);
        let target_page = start / DATA_CAPACITY;
        let (mut page, mut page_index) =
            if file.cached_page != FrameLease::EMPTY && file.cached_page_index <= target_page {
                (file.cached_page, file.cached_page_index)
            } else {
                (file.first_page, 0)
            };
        while page_index < target_page {
            page = data_page(page)?.next;
            page_index += 1;
        }
        file.cached_page = page;
        file.cached_page_index = page_index;
        let mut skipped = page_index * DATA_CAPACITY;
        let mut copied = 0;
        while start + copied < end {
            let position = start + copied;
            let data = data_page(page)?;
            let page_offset = position - skipped;
            let length = (end - position).min(DATA_CAPACITY - page_offset);
            output[copied..copied + length]
                .copy_from_slice(&data.bytes[page_offset..page_offset + length]);
            copied += length;
            skipped += DATA_CAPACITY;
            page = data.next;
        }
        Some(copied)
    }

    fn write(&mut self, handle: FileHandle, offset: usize, input: &[u8]) -> Option<usize> {
        let (metadata_page, slot) = self.find_file(|file| file.handle == handle)?;
        if input.is_empty() {
            return Some(0);
        }
        let end = offset.checked_add(input.len())?;
        let file = file_page(metadata_page)?.files[slot].as_ref()?;
        if file.published && file.name() != FILE_NAME {
            return None;
        }
        let old_length = file.length;
        let old_page_count = file.page_count;
        let required_page_count = end.div_ceil(DATA_CAPACITY);
        let additional_page_count = required_page_count.saturating_sub(old_page_count);
        let first_page = file.first_page;
        let old_last_page = file.last_page;
        let additions = allocate_data_pages(additional_page_count)?;
        let first_page = if first_page == FrameLease::EMPTY {
            additions
        } else {
            if additions != FrameLease::EMPTY {
                data_page_mut(old_last_page)?.next = additions;
            }
            first_page
        };
        let last_page = if additions == FrameLease::EMPTY {
            old_last_page
        } else {
            data_page_at(additions, additional_page_count.checked_sub(1)?)?
        };

        let mut page = data_page_at(first_page, offset / DATA_CAPACITY)?;
        let mut page_start = offset / DATA_CAPACITY * DATA_CAPACITY;
        let mut copied = 0;
        while copied < input.len() {
            let position = offset + copied;
            let page_offset = position - page_start;
            let length = (input.len() - copied).min(DATA_CAPACITY - page_offset);
            let data = data_page_mut(page)?;
            data.bytes[page_offset..page_offset + length]
                .copy_from_slice(&input[copied..copied + length]);
            copied += length;
            page_start += DATA_CAPACITY;
            page = data.next;
        }

        let file = file_page_mut(metadata_page)?.files[slot].as_mut()?;
        file.first_page = first_page;
        file.last_page = last_page;
        file.page_count = required_page_count;
        file.cached_page = page;
        file.cached_page_index = page_start / DATA_CAPACITY;
        file.length = old_length.max(end);
        self.allocated_pages += additional_page_count;
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
        let Some((metadata_page, slot)) = self.find_file(|file| file.handle == handle) else {
            return false;
        };
        let Some(file) = file_page_mut(metadata_page).and_then(|page| page.files[slot].as_mut())
        else {
            return false;
        };
        let page_count = file.page_count;
        if !release_data_pages(file.first_page) {
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
        file.first_page = FrameLease::EMPTY;
        file.last_page = FrameLease::EMPTY;
        file.cached_page = FrameLease::EMPTY;
        file.cached_page_index = 0;
        let Some(page) = file_page_mut(metadata_page) else {
            return false;
        };
        page.files[slot] = None;
        self.release_empty_file_pages()
    }

    fn clean(&self) -> bool {
        self.used == 0
            && self.allocated_pages == 0
            && self.file_page_count == 0
            && self.first_file_page == FrameLease::EMPTY
    }

    fn release_empty_file_pages(&mut self) -> bool {
        let mut previous = FrameLease::EMPTY;
        let mut current = self.first_file_page;
        while current != FrameLease::EMPTY {
            let Some(page) = file_page(current) else {
                return false;
            };
            let next = page.next;
            if page.files.iter().all(Option::is_none) {
                if !memory::scrub_and_release_frames(FrameOwner::KERNEL, &[current]) {
                    return false;
                }
                if previous == FrameLease::EMPTY {
                    self.first_file_page = next;
                } else {
                    let Some(previous_page) = file_page_mut(previous) else {
                        return false;
                    };
                    previous_page.next = next;
                }
                self.file_page_count -= 1;
            } else {
                previous = current;
            }
            current = next;
        }
        true
    }
}

impl File {
    fn name(&self) -> &[u8] {
        &self.name[..usize::from(self.name_length)]
    }
}

fn allocate_file_page() -> Option<FrameLease> {
    let [page] = memory::allocate_frames::<1>(FrameOwner::KERNEL)?;
    let Some(address) = page.direct_map_address() else {
        let _ = memory::scrub_and_release_frames(FrameOwner::KERNEL, &[page]);
        return None;
    };
    unsafe {
        core::ptr::write_bytes(address as *mut u8, 0, PAGE_SIZE);
        core::ptr::write(
            address as *mut FilePage,
            FilePage {
                next: FrameLease::EMPTY,
                files: [None; FILES_PER_PAGE],
            },
        );
    }
    Some(page)
}

fn file_page(page: FrameLease) -> Option<&'static FilePage> {
    let address = page.direct_map_address()?;
    Some(unsafe { &*(address as *const FilePage) })
}

fn file_page_mut(page: FrameLease) -> Option<&'static mut FilePage> {
    let address = page.direct_map_address()?;
    Some(unsafe { &mut *(address as *mut FilePage) })
}

fn allocate_data_pages(count: usize) -> Option<FrameLease> {
    let mut first = FrameLease::EMPTY;
    let mut last = FrameLease::EMPTY;
    for _ in 0..count {
        let Some([page]) = memory::allocate_frames::<1>(FrameOwner::KERNEL) else {
            let _ = release_data_pages(first);
            return None;
        };
        let Some(address) = page.direct_map_address() else {
            let _ = memory::scrub_and_release_frames(FrameOwner::KERNEL, &[page]);
            let _ = release_data_pages(first);
            return None;
        };
        unsafe {
            core::ptr::write_bytes(address as *mut u8, 0, PAGE_SIZE);
            core::ptr::write(
                address as *mut DataPage,
                DataPage {
                    next: FrameLease::EMPTY,
                    bytes: [0; DATA_CAPACITY],
                },
            );
        }
        if first == FrameLease::EMPTY {
            first = page;
        } else {
            data_page_mut(last)?.next = page;
        }
        last = page;
    }
    Some(first)
}

fn data_page_at(mut page: FrameLease, index: usize) -> Option<FrameLease> {
    for _ in 0..index {
        page = data_page(page)?.next;
    }
    (page != FrameLease::EMPTY).then_some(page)
}

fn data_page(page: FrameLease) -> Option<&'static DataPage> {
    (page != FrameLease::EMPTY).then_some(())?;
    let address = page.direct_map_address()?;
    Some(unsafe { &*(address as *const DataPage) })
}

fn data_page_mut(page: FrameLease) -> Option<&'static mut DataPage> {
    (page != FrameLease::EMPTY).then_some(())?;
    let address = page.direct_map_address()?;
    Some(unsafe { &mut *(address as *mut DataPage) })
}

fn release_data_pages(mut page: FrameLease) -> bool {
    while page != FrameLease::EMPTY {
        let Some(next) = data_page(page).map(|data| data.next) else {
            return false;
        };
        if !memory::scrub_and_release_frames(FrameOwner::KERNEL, &[page]) {
            return false;
        }
        page = next;
    }
    true
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
        || tmpfs.write(staged, DATA_CAPACITY - 2, b"abcd") != Some(4)
        || tmpfs.read(staged, DATA_CAPACITY - 2, &mut output[..4]) != Some(4)
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
    let beyond_old_quota = 8 * PAGE_SIZE + 17;
    if tmpfs.write(aborted, beyond_old_quota, b"z") != Some(1)
        || tmpfs.read(aborted, beyond_old_quota, &mut output[..1]) != Some(1)
        || output[0] != b'z'
    {
        return false;
    }
    let second_name = b"sha256:cccccccc";
    let Some(second) = tmpfs.create(second_name, false) else {
        return false;
    };
    if tmpfs.write(second, beyond_old_quota, b"q") != Some(1)
        || tmpfs.find(aborted_name, true).is_some()
        || !tmpfs.unlink(second)
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
    if replacement.generation <= stale.generation
        || replacement.inode <= stale.inode
        || tmpfs.read(replacement, 0, &mut output) != Some(0)
        || tmpfs.unlink(stale)
        || !tmpfs.unlink(replacement)
        || !tmpfs.unlink(staged)
    {
        return false;
    }
    for index in 0..=FILES_PER_PAGE {
        let name = [
            b'm',
            b'a' + u8::try_from(index / 26).unwrap_or(25),
            b'a' + u8::try_from(index % 26).unwrap_or(25),
        ];
        if tmpfs.create(&name, false).is_none() {
            return false;
        }
    }
    if tmpfs.file_page_count < 2 {
        return false;
    }
    for index in 0..=FILES_PER_PAGE {
        let name = [
            b'm',
            b'a' + u8::try_from(index / 26).unwrap_or(25),
            b'a' + u8::try_from(index % 26).unwrap_or(25),
        ];
        let Some(handle) = tmpfs.find(&name, false) else {
            return false;
        };
        if !tmpfs.unlink(handle) {
            return false;
        }
    }
    tmpfs.clean() && memory::owner_live_frame_count(FrameOwner::KERNEL) == Some(frames_before)
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

pub(crate) fn usage() -> usize {
    live().map_or(0, |tmpfs| tmpfs.used)
}

pub(crate) fn verify_live_object_contract() -> bool {
    let Some(frames_before) = memory::owner_live_frame_count(FrameOwner::KERNEL) else {
        return false;
    };
    let first_name = b"sha256:live-a";
    let second_name = b"sha256:live-b";
    let overflow_name = b"sha256:overflow";
    if usage() != 0 {
        return false;
    }
    let Some(first) = stage_object(first_name) else {
        return false;
    };
    if open(first_name).is_some()
        || write(first, DATA_CAPACITY - 2, b"abcd") != Some(4)
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
    let beyond_old_quota = 8 * PAGE_SIZE + 17;
    if write(second, beyond_old_quota, b"s") != Some(1)
        || write(overflow, beyond_old_quota, b"o") != Some(1)
        || !abort_object(overflow)
        || !abort_object(second)
        || !unlink(first_name, first)
    {
        return false;
    }
    usage() == 0
        && open(first_name).is_none()
        && open(second_name).is_none()
        && open(overflow_name).is_none()
        && memory::owner_live_frame_count(FrameOwner::KERNEL) == Some(frames_before)
}

pub(crate) fn from_generation(generation: u32) -> Option<FileHandle> {
    let tmpfs = live()?;
    let (page, slot) =
        tmpfs.find_file(|file| file.published && file.handle.generation == generation)?;
    file_page(page)?.files[slot].map(|file| file.handle)
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
    live_mut()?.read(handle, offset, output)
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
    while let Some((page, slot)) = tmpfs.find_file(|_| true) {
        let Some(handle) =
            file_page(page).and_then(|page| page.files[slot].map(|file| file.handle))
        else {
            return false;
        };
        if !tmpfs.unlink(handle) {
            return false;
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
