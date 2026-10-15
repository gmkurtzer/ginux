use core::{
    mem::MaybeUninit,
    sync::atomic::{AtomicBool, Ordering},
};

use ginux_image_store::{Digest, ImageStore, ObjectBackend, StagingHandle, StoreError};

use crate::memory::{self, FrameLease, FrameOwner};

const PAGE_SIZE: usize = 4096;
const OBJECT_LIMIT: usize = 4;
const OBJECT_PAGE_LIMIT: usize = 2048;
const ROOTFS_PAGE_LIMIT: usize = 2560;
pub(crate) const OBJECT_SIZE_LIMIT: usize = OBJECT_PAGE_LIMIT * PAGE_SIZE;
pub(crate) const CACHE_QUOTA: usize = OBJECT_SIZE_LIMIT;

static INSTALLED: AtomicBool = AtomicBool::new(false);
static mut STORE: MaybeUninit<ImageStore<RamBackend, OBJECT_LIMIT>> = MaybeUninit::uninit();
static mut LAST_PUBLISHED: Digest = Digest([0; 32]);
static mut ROOTFS_FRAMES: [[FrameLease; ROOTFS_PAGE_LIMIT]; crate::control_abi::CONTAINER_LIMIT] =
    [const { [FrameLease::EMPTY; ROOTFS_PAGE_LIMIT] }; crate::control_abi::CONTAINER_LIMIT];

#[derive(Clone, Copy)]
struct RamObject {
    live: bool,
    length: usize,
    page_count: usize,
    pages: [FrameLease; OBJECT_PAGE_LIMIT],
}

impl RamObject {
    const EMPTY: Self = Self {
        live: false,
        length: 0,
        page_count: 0,
        pages: [FrameLease::EMPTY; OBJECT_PAGE_LIMIT],
    };
}

struct RamBackend {
    objects: [RamObject; OBJECT_LIMIT],
}

impl RamBackend {
    const fn new() -> Self {
        Self {
            objects: [RamObject::EMPTY; OBJECT_LIMIT],
        }
    }
}

impl ObjectBackend for RamBackend {
    type Object = u8;

    fn create(&mut self, expected_length: usize) -> Option<Self::Object> {
        let page_count = expected_length.div_ceil(PAGE_SIZE);
        if page_count == 0 || page_count > OBJECT_PAGE_LIMIT {
            return None;
        }
        let slot = self.objects.iter().position(|object| !object.live)?;
        let object = &mut self.objects[slot];
        for index in 0..page_count {
            let Some([frame]) = memory::allocate_frames::<1>(FrameOwner::KERNEL) else {
                if index != 0 {
                    let _ = memory::scrub_and_release_frames(
                        FrameOwner::KERNEL,
                        &object.pages[..index],
                    );
                }
                object.pages.fill(FrameLease::EMPTY);
                return None;
            };
            object.pages[index] = frame;
        }
        object.live = true;
        object.length = expected_length;
        object.page_count = page_count;
        Some(slot as u8)
    }

    fn append(&mut self, object: Self::Object, offset: usize, bytes: &[u8]) -> bool {
        let Some(object) = self
            .objects
            .get_mut(usize::from(object))
            .filter(|object| object.live)
        else {
            return false;
        };
        if offset
            .checked_add(bytes.len())
            .is_none_or(|end| end > object.length)
        {
            return false;
        }
        let mut copied = 0;
        while copied < bytes.len() {
            let position = offset + copied;
            let page = position / PAGE_SIZE;
            let page_offset = position % PAGE_SIZE;
            let length = (bytes.len() - copied).min(PAGE_SIZE - page_offset);
            let Some(address) = object.pages[page].direct_map_address() else {
                return false;
            };
            let output = unsafe {
                core::slice::from_raw_parts_mut((address + page_offset) as *mut u8, length)
            };
            output.copy_from_slice(&bytes[copied..copied + length]);
            copied += length;
        }
        true
    }

    fn read(&self, object: Self::Object, offset: usize, output: &mut [u8]) -> bool {
        let Some(object) = self
            .objects
            .get(usize::from(object))
            .filter(|object| object.live)
        else {
            return false;
        };
        if offset
            .checked_add(output.len())
            .is_none_or(|end| end > object.length)
        {
            return false;
        }
        let mut copied = 0;
        while copied < output.len() {
            let position = offset + copied;
            let page = position / PAGE_SIZE;
            let page_offset = position % PAGE_SIZE;
            let length = (output.len() - copied).min(PAGE_SIZE - page_offset);
            let Some(address) = object.pages[page].direct_map_address() else {
                return false;
            };
            let input = unsafe {
                core::slice::from_raw_parts((address + page_offset) as *const u8, length)
            };
            output[copied..copied + length].copy_from_slice(input);
            copied += length;
        }
        true
    }

    fn destroy(&mut self, object: Self::Object) -> bool {
        let Some(object) = self
            .objects
            .get_mut(usize::from(object))
            .filter(|object| object.live)
        else {
            return false;
        };
        if !memory::scrub_and_release_frames(FrameOwner::KERNEL, &object.pages[..object.page_count])
        {
            return false;
        }
        *object = RamObject::EMPTY;
        true
    }
}

pub(crate) fn install() -> bool {
    if INSTALLED.swap(true, Ordering::AcqRel) {
        return false;
    }
    unsafe {
        core::ptr::addr_of_mut!(STORE).write(MaybeUninit::new(ImageStore::new(
            RamBackend::new(),
            CACHE_QUOTA,
        )));
    }
    true
}

pub(crate) fn stage(digest: [u8; 32], length: usize) -> Result<StagingHandle, StoreError> {
    with_store(|store| store.stage(Digest(digest), length)).unwrap_or(Err(StoreError::Backend))
}

pub(crate) fn append(handle: StagingHandle, bytes: &[u8]) -> Result<usize, StoreError> {
    with_store(|store| store.append(handle, bytes)).unwrap_or(Err(StoreError::Backend))
}

pub(crate) fn publish(handle: StagingHandle) -> Result<Digest, StoreError> {
    let result = with_store(|store| store.publish(handle)).unwrap_or(Err(StoreError::Backend));
    if let Ok(digest) = result {
        unsafe {
            core::ptr::addr_of_mut!(LAST_PUBLISHED).write(digest);
        }
    }
    result
}

pub(crate) fn abort(handle: StagingHandle) -> Result<(), StoreError> {
    with_store(|store| store.abort(handle)).unwrap_or(Err(StoreError::Backend))
}

fn with_store<T>(
    operation: impl FnOnce(&mut ImageStore<RamBackend, OBJECT_LIMIT>) -> T,
) -> Option<T> {
    if !INSTALLED.load(Ordering::Acquire) {
        return None;
    }
    let store = unsafe {
        &mut *core::ptr::addr_of_mut!(STORE).cast::<ImageStore<RamBackend, OBJECT_LIMIT>>()
    };
    Some(operation(store))
}

pub(crate) fn install_latest_layer(slot: usize) -> bool {
    if slot >= crate::control_abi::CONTAINER_LIMIT || crate::oci_image::is_installed(slot) {
        return false;
    }
    let digest = unsafe { core::ptr::addr_of!(LAST_PUBLISHED).read() };
    if digest == Digest([0; 32]) {
        return false;
    }
    with_store(|store| {
        let Some((object, compressed_length)) = store.published_object(digest) else {
            return false;
        };
        let Some(header) = gzip_header_length(store.backend(), object, compressed_length) else {
            return false;
        };
        if compressed_length < header + 8 {
            return false;
        }
        let mut trailer = [0; 4];
        if !store
            .backend()
            .read(object, compressed_length - 4, &mut trailer)
        {
            return false;
        }
        let expected_length = u32::from_le_bytes(trailer) as usize;
        if expected_length == 0 || expected_length > ROOTFS_PAGE_LIMIT * PAGE_SIZE {
            return false;
        }
        let Some(frames) =
            memory::allocate_contiguous_frames::<ROOTFS_PAGE_LIMIT>(FrameOwner::KERNEL)
        else {
            return false;
        };
        let Some(address) = frames[0].direct_map_address() else {
            let _ = memory::scrub_and_release_frames(FrameOwner::KERNEL, &frames);
            return false;
        };
        let output = unsafe {
            core::slice::from_raw_parts_mut(address as *mut u8, ROOTFS_PAGE_LIMIT * PAGE_SIZE)
        };
        let input = ObjectSlices::new(
            store.backend(),
            object,
            header,
            compressed_length - header - 8,
        );
        let Ok(written) =
            miniz_oxide::inflate::decompress_slice_iter_to_slice(output, input, false, true)
        else {
            let _ = memory::scrub_and_release_frames(FrameOwner::KERNEL, &frames);
            return false;
        };
        if written != expected_length {
            let _ = memory::scrub_and_release_frames(FrameOwner::KERNEL, &frames);
            return false;
        }
        let bytes = unsafe { core::slice::from_raw_parts(address as *const u8, written) };
        if !crate::oci_image::install_runtime_rootfs(slot, bytes) {
            let _ = memory::scrub_and_release_frames(FrameOwner::KERNEL, &frames);
            return false;
        }
        unsafe {
            core::ptr::addr_of_mut!(ROOTFS_FRAMES[slot]).write(frames);
        }
        true
    })
    .unwrap_or(false)
}

pub(crate) fn uninstall_rootfs(slot: usize) -> bool {
    if slot >= crate::control_abi::CONTAINER_LIMIT
        || !crate::oci_image::uninstall_runtime_rootfs(slot)
    {
        return false;
    }
    let frames = unsafe { core::ptr::addr_of!(ROOTFS_FRAMES[slot]).read() };
    if frames[0] == FrameLease::EMPTY {
        return true;
    }
    let released = memory::scrub_and_release_frames(FrameOwner::KERNEL, &frames);
    if released {
        unsafe {
            core::ptr::addr_of_mut!(ROOTFS_FRAMES[slot])
                .write([FrameLease::EMPTY; ROOTFS_PAGE_LIMIT]);
        }
    }
    released
}

fn gzip_header_length(backend: &RamBackend, object: u8, length: usize) -> Option<usize> {
    let mut header = [0; PAGE_SIZE];
    let read = length.min(header.len());
    if !backend.read(object, 0, &mut header[..read]) || read < 10 || header[..3] != [0x1f, 0x8b, 8]
    {
        return None;
    }
    let flags = header[3];
    if flags & 0xe0 != 0 {
        return None;
    }
    let mut offset = 10;
    if flags & 4 != 0 {
        let extra = u16::from_le_bytes([*header.get(offset)?, *header.get(offset + 1)?]) as usize;
        offset = offset.checked_add(2 + extra)?;
    }
    for flag in [8, 16] {
        if flags & flag != 0 {
            offset = header.get(offset..)?.iter().position(|byte| *byte == 0)? + offset + 1;
        }
    }
    if flags & 2 != 0 {
        offset = offset.checked_add(2)?;
    }
    (offset < read).then_some(offset)
}

struct ObjectSlices<'a> {
    backend: &'a RamBackend,
    object: u8,
    offset: usize,
    remaining: usize,
}

impl<'a> ObjectSlices<'a> {
    fn new(backend: &'a RamBackend, object: u8, offset: usize, remaining: usize) -> Self {
        Self {
            backend,
            object,
            offset,
            remaining,
        }
    }
}

impl<'a> Iterator for ObjectSlices<'a> {
    type Item = &'a [u8];

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }
        let object = self.backend.objects.get(usize::from(self.object))?;
        let page = self.offset / PAGE_SIZE;
        let page_offset = self.offset % PAGE_SIZE;
        let length = self.remaining.min(PAGE_SIZE - page_offset);
        let address = object.pages.get(page)?.direct_map_address()? + page_offset;
        let bytes = unsafe { core::slice::from_raw_parts(address as *const u8, length) };
        self.offset += length;
        self.remaining -= length;
        Some(bytes)
    }
}
