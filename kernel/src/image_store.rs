use core::{
    mem::MaybeUninit,
    sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
};

use ginux_image_store::{Digest, ImageStore, ObjectBackend, StagingHandle, StoreError};

use crate::memory::{self, FrameLease, FrameOwner};

const PAGE_SIZE: usize = 4096;
const OBJECT_LIMIT: usize = ginux_oci::OCI_LAYER_LIMIT;
pub(crate) const CACHE_QUOTA: usize = usize::MAX;

static INSTALLED: AtomicBool = AtomicBool::new(false);
static mut STORE: MaybeUninit<ImageStore<RamBackend, OBJECT_LIMIT>> = MaybeUninit::uninit();
static mut PUBLISHED_LAYERS: [Digest; ginux_oci::OCI_LAYER_LIMIT] =
    [Digest([0; 32]); ginux_oci::OCI_LAYER_LIMIT];
static PUBLISHED_LAYER_COUNT: AtomicUsize = AtomicUsize::new(0);
static ROOTFS_FRAME_STARTS: [AtomicU64; crate::control_abi::DOMAIN_LIMIT] =
    [const { AtomicU64::new(0) }; crate::control_abi::DOMAIN_LIMIT];
static ROOTFS_FRAME_COUNTS: [AtomicUsize; crate::control_abi::DOMAIN_LIMIT] =
    [const { AtomicUsize::new(0) }; crate::control_abi::DOMAIN_LIMIT];

#[derive(Clone, Copy)]
struct RamObject {
    live: bool,
    length: usize,
    page_count: usize,
    first: FrameLease,
}

impl RamObject {
    const EMPTY: Self = Self {
        live: false,
        length: 0,
        page_count: 0,
        first: FrameLease::EMPTY,
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
        if page_count == 0 {
            return None;
        }
        let slot = self.objects.iter().position(|object| !object.live)?;
        let object = &mut self.objects[slot];
        let (first, _) = memory::allocate_contiguous_extent(FrameOwner::KERNEL, page_count)?;
        object.live = true;
        object.length = expected_length;
        object.page_count = page_count;
        object.first = first;
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
            let Some(address) = object
                .first
                .address()
                .checked_add((page * PAGE_SIZE) as u64)
                .and_then(|address| FrameLease::from_parts(address, object.first.generation()))
                .and_then(FrameLease::direct_map_address)
            else {
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
            let Some(address) = object
                .first
                .address()
                .checked_add((page * PAGE_SIZE) as u64)
                .and_then(|address| FrameLease::from_parts(address, object.first.generation()))
                .and_then(FrameLease::direct_map_address)
            else {
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
        if !memory::scrub_and_release_contiguous_extent(
            FrameOwner::KERNEL,
            object.first,
            object.page_count,
        ) {
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
        let index = PUBLISHED_LAYER_COUNT.fetch_add(1, Ordering::AcqRel);
        if index >= ginux_oci::OCI_LAYER_LIMIT {
            PUBLISHED_LAYER_COUNT.store(ginux_oci::OCI_LAYER_LIMIT, Ordering::Release);
            return Err(StoreError::QuotaExceeded);
        }
        unsafe {
            core::ptr::addr_of_mut!(PUBLISHED_LAYERS)
                .cast::<Digest>()
                .add(index)
                .write(digest);
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

pub(crate) fn begin_layer_pull() -> bool {
    discard_published_layers()
}

pub(crate) fn discard_published_layers() -> bool {
    let count = PUBLISHED_LAYER_COUNT.swap(0, Ordering::AcqRel);
    if count > ginux_oci::OCI_LAYER_LIMIT {
        return false;
    }
    with_store(|store| {
        let mut valid = true;
        for index in 0..count {
            let digest = unsafe {
                core::ptr::addr_of!(PUBLISHED_LAYERS)
                    .cast::<Digest>()
                    .add(index)
                    .read()
            };
            valid &= digest != Digest([0; 32]) && store.remove(digest).is_ok();
            unsafe {
                core::ptr::addr_of_mut!(PUBLISHED_LAYERS)
                    .cast::<Digest>()
                    .add(index)
                    .write(Digest([0; 32]));
            }
        }
        valid
    })
    .unwrap_or(false)
}

pub(crate) fn install_published_layers(slot: usize) -> bool {
    if slot >= crate::control_abi::DOMAIN_LIMIT || crate::published_rootfs::is_installed(slot) {
        return false;
    }
    let layer_count = PUBLISHED_LAYER_COUNT.load(Ordering::Acquire);
    if layer_count == 0 || layer_count > ginux_oci::OCI_LAYER_LIMIT {
        return false;
    }
    with_store(|store| {
        let mut digests = [Digest([0; 32]); ginux_oci::OCI_LAYER_LIMIT];
        let mut objects = [0_u8; ginux_oci::OCI_LAYER_LIMIT];
        let mut compressed_lengths = [0_usize; ginux_oci::OCI_LAYER_LIMIT];
        let mut headers = [0_usize; ginux_oci::OCI_LAYER_LIMIT];
        let mut expected_lengths = [0_usize; ginux_oci::OCI_LAYER_LIMIT];
        let mut allocation_length = 0_usize;
        for index in 0..layer_count {
            let digest = unsafe {
                core::ptr::addr_of!(PUBLISHED_LAYERS)
                    .cast::<Digest>()
                    .add(index)
                    .read()
            };
            let Some((object, compressed_length)) = store.published_object(digest) else {
                return false;
            };
            let Some(header) = gzip_header_length(store.backend(), object, compressed_length)
            else {
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
            if expected_length == 0 {
                return false;
            }
            allocation_length = match allocation_length.checked_add(expected_length) {
                Some(length) => length,
                None => return false,
            };
            digests[index] = digest;
            objects[index] = object;
            compressed_lengths[index] = compressed_length;
            headers[index] = header;
            expected_lengths[index] = expected_length;
        }
        let page_count = allocation_length.div_ceil(PAGE_SIZE);
        let Some((first_frame, _)) =
            memory::allocate_contiguous_extent(FrameOwner::KERNEL, page_count)
        else {
            crate::serial_write_bytes(b"ginux: OCI rootfs memory allocation failed: ");
            crate::serial_write_u64(allocation_length as u64);
            crate::serial_write_bytes(b" bytes\r\n");
            return false;
        };
        let Some(address) = first_frame.direct_map_address() else {
            let _ = memory::scrub_and_release_contiguous_extent(
                FrameOwner::KERNEL,
                first_frame,
                page_count,
            );
            return false;
        };
        let output =
            unsafe { core::slice::from_raw_parts_mut(address as *mut u8, page_count * PAGE_SIZE) };
        output.fill(0);
        let mut rootfs_length = 0_usize;
        for index in 0..layer_count {
            let expected_length = expected_lengths[index];
            let Some(layer_end) = rootfs_length.checked_add(expected_length) else {
                let _ = memory::scrub_and_release_contiguous_extent(
                    FrameOwner::KERNEL,
                    first_frame,
                    page_count,
                );
                return false;
            };
            let input = ObjectSlices::new(
                store.backend(),
                objects[index],
                headers[index],
                compressed_lengths[index] - headers[index] - 8,
            );
            let Ok(written) = miniz_oxide::inflate::decompress_slice_iter_to_slice(
                &mut output[rootfs_length..layer_end],
                input,
                false,
                true,
            ) else {
                let _ = memory::scrub_and_release_contiguous_extent(
                    FrameOwner::KERNEL,
                    first_frame,
                    page_count,
                );
                return false;
            };
            if written != expected_length {
                let _ = memory::scrub_and_release_contiguous_extent(
                    FrameOwner::KERNEL,
                    first_frame,
                    page_count,
                );
                return false;
            }
            let Some(layer) = ginux_oci::Rootfs::parse(&output[rootfs_length..layer_end]) else {
                let _ = memory::scrub_and_release_contiguous_extent(
                    FrameOwner::KERNEL,
                    first_frame,
                    page_count,
                );
                return false;
            };
            rootfs_length += layer.content_length();
        }
        let Some(rootfs_end) = rootfs_length.checked_add(1024) else {
            let _ = memory::scrub_and_release_contiguous_extent(
                FrameOwner::KERNEL,
                first_frame,
                page_count,
            );
            return false;
        };
        if rootfs_end > output.len() {
            let _ = memory::scrub_and_release_contiguous_extent(
                FrameOwner::KERNEL,
                first_frame,
                page_count,
            );
            return false;
        }
        output[rootfs_length..rootfs_end].fill(0);
        let bytes = unsafe { core::slice::from_raw_parts(address as *const u8, rootfs_end) };
        if !crate::published_rootfs::install_published_rootfs(slot, bytes) {
            crate::serial_write_bytes(b"ginux: OCI rootfs publication failed\r\n");
            let _ = memory::scrub_and_release_contiguous_extent(
                FrameOwner::KERNEL,
                first_frame,
                page_count,
            );
            return false;
        }
        for digest in &digests[..layer_count] {
            if store.remove(*digest).is_err() {
                let _ = crate::published_rootfs::uninstall_published_rootfs(slot);
                let _ = memory::scrub_and_release_contiguous_extent(
                    FrameOwner::KERNEL,
                    first_frame,
                    page_count,
                );
                return false;
            }
        }
        ROOTFS_FRAME_STARTS[slot].store(first_frame.address(), Ordering::Release);
        ROOTFS_FRAME_COUNTS[slot].store(page_count, Ordering::Release);
        for index in 0..layer_count {
            unsafe {
                core::ptr::addr_of_mut!(PUBLISHED_LAYERS)
                    .cast::<Digest>()
                    .add(index)
                    .write(Digest([0; 32]));
            }
        }
        PUBLISHED_LAYER_COUNT.store(0, Ordering::Release);
        true
    })
    .unwrap_or(false)
}

pub(crate) fn uninstall_rootfs(slot: usize) -> bool {
    if slot >= crate::control_abi::DOMAIN_LIMIT
        || !crate::published_rootfs::uninstall_published_rootfs(slot)
    {
        return false;
    }
    let start = ROOTFS_FRAME_STARTS[slot].load(Ordering::Acquire);
    let count = ROOTFS_FRAME_COUNTS[slot].load(Ordering::Acquire);
    if count == 0 || start == 0 {
        return true;
    }
    let Some(first_frame) = FrameLease::from_parts(start, 1) else {
        return false;
    };
    let released =
        memory::scrub_and_release_contiguous_extent(FrameOwner::KERNEL, first_frame, count);
    if released {
        ROOTFS_FRAME_STARTS[slot].store(0, Ordering::Release);
        ROOTFS_FRAME_COUNTS[slot].store(0, Ordering::Release);
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
        if page >= object.page_count {
            return None;
        }
        let address = FrameLease::from_parts(
            object
                .first
                .address()
                .checked_add((page * PAGE_SIZE) as u64)?,
            object.first.generation(),
        )?
        .direct_map_address()?
            + page_offset;
        let bytes = unsafe { core::slice::from_raw_parts(address as *const u8, length) };
        self.offset += length;
        self.remaining -= length;
        Some(bytes)
    }
}
