use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

use crate::{control_abi, linux_process, memory};

const PAGE_SIZE: usize = 4096;
const INSTANCE_LIMIT: usize = control_abi::DOMAIN_LIMIT + 1;
const FRAME_CAPACITY: usize = 320;
const WALK_LIMIT: usize = 262_144;

static LOCKS: [AtomicBool; INSTANCE_LIMIT] = [const { AtomicBool::new(false) }; INSTANCE_LIMIT];
static ROOT_ADDRESSES: [AtomicU64; INSTANCE_LIMIT] = [const { AtomicU64::new(0) }; INSTANCE_LIMIT];
static ROOT_GENERATIONS: [AtomicU32; INSTANCE_LIMIT] =
    [const { AtomicU32::new(0) }; INSTANCE_LIMIT];
static ATTACHMENT_ROOT_ADDRESSES: [AtomicU64; INSTANCE_LIMIT] =
    [const { AtomicU64::new(0) }; INSTANCE_LIMIT];
static ATTACHMENT_ROOT_GENERATIONS: [AtomicU32; INSTANCE_LIMIT] =
    [const { AtomicU32::new(0) }; INSTANCE_LIMIT];
static NEXT_IDS: [AtomicU32; INSTANCE_LIMIT] = [const { AtomicU32::new(1) }; INSTANCE_LIMIT];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Error {
    Exists,
    Invalid,
    NoEntry,
    NoMemory,
}

#[derive(Clone, Copy)]
pub(crate) struct SegmentInfo {
    pub(crate) key: i32,
    pub(crate) size: u64,
    pub(crate) mode: u16,
    pub(crate) creator_pid: u32,
    pub(crate) last_pid: u32,
    pub(crate) attachments: u64,
}

#[repr(C)]
struct SegmentPage {
    next_segment: memory::FrameLease,
    next_page: memory::FrameLease,
    id: u32,
    key: i32,
    size: u64,
    mode: u16,
    marked_for_removal: u8,
    _reserved: u8,
    creator_pid: u32,
    last_pid: u32,
    attachments: u64,
    frames_used: u16,
    _padding: [u8; 6],
    frames: [memory::FrameLease; FRAME_CAPACITY],
}

#[repr(C)]
struct Attachment {
    next: memory::FrameLease,
    segment: memory::FrameLease,
    address: u64,
    length: u64,
    pid: u32,
}

const _: () = assert!(core::mem::size_of::<SegmentPage>() <= PAGE_SIZE);
const _: () = assert!(core::mem::size_of::<Attachment>() <= PAGE_SIZE);

struct Guard {
    instance: usize,
}

impl Guard {
    fn acquire() -> Option<Self> {
        let instance = linux_process::current_ipc_instance();
        let lock = LOCKS.get(instance)?;
        while lock
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            core::hint::spin_loop();
        }
        Some(Self { instance })
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        LOCKS[self.instance].store(false, Ordering::Release);
    }
}

fn root(
    addresses: &[AtomicU64; INSTANCE_LIMIT],
    generations: &[AtomicU32; INSTANCE_LIMIT],
    instance: usize,
) -> Option<memory::FrameLease> {
    memory::FrameLease::from_parts(
        addresses[instance].load(Ordering::Acquire),
        generations[instance].load(Ordering::Acquire),
    )
}

fn publish_root(
    addresses: &[AtomicU64; INSTANCE_LIMIT],
    generations: &[AtomicU32; INSTANCE_LIMIT],
    instance: usize,
    value: Option<memory::FrameLease>,
) {
    let value = value.unwrap_or(memory::FrameLease::EMPTY);
    generations[instance].store(value.generation(), Ordering::Release);
    addresses[instance].store(value.address(), Ordering::Release);
}

fn segment_page(frame: memory::FrameLease) -> Option<&'static SegmentPage> {
    Some(unsafe { &*(frame.direct_map_address()? as *const SegmentPage) })
}

fn segment_page_mut(frame: memory::FrameLease) -> Option<&'static mut SegmentPage> {
    Some(unsafe { &mut *(frame.direct_map_address()? as *mut SegmentPage) })
}

fn attachment(frame: memory::FrameLease) -> Option<&'static Attachment> {
    Some(unsafe { &*(frame.direct_map_address()? as *const Attachment) })
}

fn attachment_mut(frame: memory::FrameLease) -> Option<&'static mut Attachment> {
    Some(unsafe { &mut *(frame.direct_map_address()? as *mut Attachment) })
}

fn allocate_zeroed() -> Option<memory::FrameLease> {
    let [frame] = memory::allocate_frames::<1>(memory::FrameOwner::KERNEL)?;
    let Some(address) = frame.direct_map_address() else {
        let _ = memory::scrub_and_release_frames(memory::FrameOwner::KERNEL, &[frame]);
        return None;
    };
    unsafe { core::ptr::write_bytes(address as *mut u8, 0, PAGE_SIZE) };
    Some(frame)
}

fn find_segment(
    instance: usize,
    id: u32,
) -> Option<(memory::FrameLease, Option<memory::FrameLease>)> {
    let mut current = root(&ROOT_ADDRESSES, &ROOT_GENERATIONS, instance);
    let mut previous = None;
    for _ in 0..WALK_LIMIT {
        let frame = current?;
        let page = segment_page(frame)?;
        if page.id == id {
            return Some((frame, previous));
        }
        previous = current;
        current = (page.next_segment != memory::FrameLease::EMPTY).then_some(page.next_segment);
    }
    None
}

fn find_key(instance: usize, key: i32) -> Option<memory::FrameLease> {
    let mut current = root(&ROOT_ADDRESSES, &ROOT_GENERATIONS, instance);
    for _ in 0..WALK_LIMIT {
        let frame = current?;
        let page = segment_page(frame)?;
        if page.key == key && page.marked_for_removal == 0 {
            return Some(frame);
        }
        current = (page.next_segment != memory::FrameLease::EMPTY).then_some(page.next_segment);
    }
    None
}

fn info(page: &SegmentPage) -> SegmentInfo {
    SegmentInfo {
        key: page.key,
        size: page.size,
        mode: page.mode,
        creator_pid: page.creator_pid,
        last_pid: page.last_pid,
        attachments: page.attachments,
    }
}

fn release_segment_chain(first: memory::FrameLease) -> bool {
    let mut current = Some(first);
    for _ in 0..WALK_LIMIT {
        let Some(frame) = current else { return true };
        let Some(page) = segment_page(frame) else {
            return false;
        };
        let next = (page.next_page != memory::FrameLease::EMPTY).then_some(page.next_page);
        for data in page.frames[..usize::from(page.frames_used)].iter().copied() {
            if !memory::scrub_and_release_frames(memory::FrameOwner::KERNEL, &[data]) {
                return false;
            }
        }
        if !memory::scrub_and_release_frames(memory::FrameOwner::KERNEL, &[frame]) {
            return false;
        }
        current = next;
    }
    false
}

fn unlink_and_release(
    instance: usize,
    frame: memory::FrameLease,
    previous: Option<memory::FrameLease>,
) -> bool {
    let Some(page) = segment_page(frame) else {
        return false;
    };
    let next = (page.next_segment != memory::FrameLease::EMPTY).then_some(page.next_segment);
    if let Some(previous) = previous {
        let Some(previous) = segment_page_mut(previous) else {
            return false;
        };
        previous.next_segment = next.unwrap_or(memory::FrameLease::EMPTY);
    } else {
        publish_root(&ROOT_ADDRESSES, &ROOT_GENERATIONS, instance, next);
    }
    release_segment_chain(frame)
}

pub(crate) fn get_or_create(
    key: i32,
    size: u64,
    mode: u16,
    create: bool,
    exclusive: bool,
    pid: u32,
) -> Result<u32, Error> {
    let guard = Guard::acquire().ok_or(Error::Invalid)?;
    if key != 0 {
        if let Some(frame) = find_key(guard.instance, key) {
            let page = segment_page(frame).ok_or(Error::Invalid)?;
            if create && exclusive {
                return Err(Error::Exists);
            }
            if size > page.size {
                return Err(Error::Invalid);
            }
            return Ok(page.id);
        }
        if !create {
            return Err(Error::NoEntry);
        }
    }
    if size == 0 {
        return Err(Error::Invalid);
    }
    let pages = usize::try_from(size.div_ceil(PAGE_SIZE as u64)).map_err(|_| Error::Invalid)?;
    let first = allocate_zeroed().ok_or(Error::NoMemory)?;
    let id = NEXT_IDS[guard.instance].fetch_add(1, Ordering::AcqRel);
    if id == 0 {
        let _ = memory::scrub_and_release_frames(memory::FrameOwner::KERNEL, &[first]);
        return Err(Error::NoMemory);
    }
    {
        let page = segment_page_mut(first).ok_or(Error::NoMemory)?;
        page.id = id;
        page.key = key;
        page.size = size;
        page.mode = mode & 0o777;
        page.creator_pid = pid;
        page.last_pid = pid;
    }
    let mut metadata = first;
    for index in 0..pages {
        if index != 0 && index.is_multiple_of(FRAME_CAPACITY) {
            let Some(next) = allocate_zeroed() else {
                let _ = release_segment_chain(first);
                return Err(Error::NoMemory);
            };
            let Some(page) = segment_page_mut(metadata) else {
                let _ = memory::scrub_and_release_frames(memory::FrameOwner::KERNEL, &[next]);
                let _ = release_segment_chain(first);
                return Err(Error::NoMemory);
            };
            page.next_page = next;
            metadata = next;
        }
        let Some(data) = allocate_zeroed() else {
            let _ = release_segment_chain(first);
            return Err(Error::NoMemory);
        };
        let Some(page) = segment_page_mut(metadata) else {
            let _ = memory::scrub_and_release_frames(memory::FrameOwner::KERNEL, &[data]);
            let _ = release_segment_chain(first);
            return Err(Error::NoMemory);
        };
        let slot = usize::from(page.frames_used);
        page.frames[slot] = data;
        page.frames_used += 1;
    }
    let old_root = root(&ROOT_ADDRESSES, &ROOT_GENERATIONS, guard.instance);
    segment_page_mut(first).ok_or(Error::NoMemory)?.next_segment =
        old_root.unwrap_or(memory::FrameLease::EMPTY);
    publish_root(
        &ROOT_ADDRESSES,
        &ROOT_GENERATIONS,
        guard.instance,
        Some(first),
    );
    Ok(id)
}

pub(crate) fn segment(id: u32) -> Result<SegmentInfo, Error> {
    let guard = Guard::acquire().ok_or(Error::Invalid)?;
    let (frame, _) = find_segment(guard.instance, id).ok_or(Error::Invalid)?;
    segment_page(frame).map(info).ok_or(Error::Invalid)
}

pub(crate) fn frame(id: u32, index: usize) -> Result<memory::FrameLease, Error> {
    let guard = Guard::acquire().ok_or(Error::Invalid)?;
    let (mut metadata, _) = find_segment(guard.instance, id).ok_or(Error::Invalid)?;
    let mut remaining = index;
    for _ in 0..WALK_LIMIT {
        let page = segment_page(metadata).ok_or(Error::Invalid)?;
        if remaining < usize::from(page.frames_used) {
            return Ok(page.frames[remaining]);
        }
        remaining -= usize::from(page.frames_used);
        if page.next_page == memory::FrameLease::EMPTY {
            return Err(Error::Invalid);
        }
        metadata = page.next_page;
    }
    Err(Error::Invalid)
}

pub(crate) fn attach(id: u32, address: u64, pid: u32) -> Result<(), Error> {
    let guard = Guard::acquire().ok_or(Error::Invalid)?;
    let (segment, _) = find_segment(guard.instance, id).ok_or(Error::Invalid)?;
    if segment_page(segment).ok_or(Error::Invalid)?.attachments == u64::MAX {
        return Err(Error::NoMemory);
    }
    let record = allocate_zeroed().ok_or(Error::NoMemory)?;
    let length = segment_page(segment).ok_or(Error::Invalid)?.size;
    let old_root = root(
        &ATTACHMENT_ROOT_ADDRESSES,
        &ATTACHMENT_ROOT_GENERATIONS,
        guard.instance,
    );
    let entry = attachment_mut(record).ok_or(Error::NoMemory)?;
    entry.next = old_root.unwrap_or(memory::FrameLease::EMPTY);
    entry.segment = segment;
    entry.address = address;
    entry.length = length;
    entry.pid = pid;
    let page = segment_page_mut(segment).ok_or(Error::Invalid)?;
    page.attachments = page.attachments.checked_add(1).ok_or(Error::NoMemory)?;
    page.last_pid = pid;
    publish_root(
        &ATTACHMENT_ROOT_ADDRESSES,
        &ATTACHMENT_ROOT_GENERATIONS,
        guard.instance,
        Some(record),
    );
    Ok(())
}

pub(crate) fn attachment_at(address: u64, pid: u32) -> Result<(u32, u64), Error> {
    let guard = Guard::acquire().ok_or(Error::Invalid)?;
    let mut current = root(
        &ATTACHMENT_ROOT_ADDRESSES,
        &ATTACHMENT_ROOT_GENERATIONS,
        guard.instance,
    );
    for _ in 0..WALK_LIMIT {
        let frame = current.ok_or(Error::Invalid)?;
        let entry = attachment(frame).ok_or(Error::Invalid)?;
        if entry.address == address && entry.pid == pid {
            let page = segment_page(entry.segment).ok_or(Error::Invalid)?;
            return Ok((page.id, entry.length));
        }
        current = (entry.next != memory::FrameLease::EMPTY).then_some(entry.next);
    }
    Err(Error::Invalid)
}

fn detach_matching(instance: usize, pid: u32, address: Option<u64>) -> Result<usize, Error> {
    let mut current = root(
        &ATTACHMENT_ROOT_ADDRESSES,
        &ATTACHMENT_ROOT_GENERATIONS,
        instance,
    );
    let mut previous = None;
    let mut removed = 0;
    for _ in 0..WALK_LIMIT {
        let Some(record) = current else {
            return Ok(removed);
        };
        let entry = attachment(record).ok_or(Error::Invalid)?;
        if entry.pid == pid && address.is_none_or(|address| entry.address == address) {
            let next = (entry.next != memory::FrameLease::EMPTY).then_some(entry.next);
            let segment = entry.segment;
            if let Some(previous) = previous {
                attachment_mut(previous).ok_or(Error::Invalid)?.next =
                    next.unwrap_or(memory::FrameLease::EMPTY);
            } else {
                publish_root(
                    &ATTACHMENT_ROOT_ADDRESSES,
                    &ATTACHMENT_ROOT_GENERATIONS,
                    instance,
                    next,
                );
            }
            let page = segment_page_mut(segment).ok_or(Error::Invalid)?;
            page.attachments = page.attachments.checked_sub(1).ok_or(Error::Invalid)?;
            page.last_pid = pid;
            let remove = page.marked_for_removal != 0 && page.attachments == 0;
            let segment_id = page.id;
            if !memory::scrub_and_release_frames(memory::FrameOwner::KERNEL, &[record]) {
                return Err(Error::Invalid);
            }
            if remove {
                let (_, segment_previous) =
                    find_segment(instance, segment_id).ok_or(Error::Invalid)?;
                if !unlink_and_release(instance, segment, segment_previous) {
                    return Err(Error::Invalid);
                }
            }
            removed += 1;
            current = next;
            if address.is_some() {
                return Ok(removed);
            }
            continue;
        }
        previous = current;
        current = (entry.next != memory::FrameLease::EMPTY).then_some(entry.next);
    }
    Err(Error::Invalid)
}

pub(crate) fn detach(address: u64, pid: u32) -> Result<(), Error> {
    let guard = Guard::acquire().ok_or(Error::Invalid)?;
    (detach_matching(guard.instance, pid, Some(address))? == 1)
        .then_some(())
        .ok_or(Error::Invalid)
}

pub(crate) fn inherit_process_attachments(parent: u32, child: u32) -> Result<(), Error> {
    let guard = Guard::acquire().ok_or(Error::Invalid)?;
    if parent == child {
        return Err(Error::Invalid);
    }
    let mut current = root(
        &ATTACHMENT_ROOT_ADDRESSES,
        &ATTACHMENT_ROOT_GENERATIONS,
        guard.instance,
    );
    for _ in 0..WALK_LIMIT {
        let Some(record) = current else { return Ok(()) };
        let source = attachment(record).ok_or(Error::Invalid)?;
        let next = (source.next != memory::FrameLease::EMPTY).then_some(source.next);
        if source.pid == child {
            let _ = detach_matching(guard.instance, child, None);
            return Err(Error::Invalid);
        }
        if source.pid == parent {
            let Some(clone) = allocate_zeroed() else {
                let _ = detach_matching(guard.instance, child, None);
                return Err(Error::NoMemory);
            };
            let root = root(
                &ATTACHMENT_ROOT_ADDRESSES,
                &ATTACHMENT_ROOT_GENERATIONS,
                guard.instance,
            );
            let Some(entry) = attachment_mut(clone) else {
                let _ = memory::scrub_and_release_frames(memory::FrameOwner::KERNEL, &[clone]);
                let _ = detach_matching(guard.instance, child, None);
                return Err(Error::NoMemory);
            };
            entry.next = root.unwrap_or(memory::FrameLease::EMPTY);
            entry.segment = source.segment;
            entry.address = source.address;
            entry.length = source.length;
            entry.pid = child;
            let Some(segment) = segment_page_mut(source.segment) else {
                let _ = memory::scrub_and_release_frames(memory::FrameOwner::KERNEL, &[clone]);
                let _ = detach_matching(guard.instance, child, None);
                return Err(Error::Invalid);
            };
            let Some(attachments) = segment.attachments.checked_add(1) else {
                let _ = memory::scrub_and_release_frames(memory::FrameOwner::KERNEL, &[clone]);
                let _ = detach_matching(guard.instance, child, None);
                return Err(Error::NoMemory);
            };
            segment.attachments = attachments;
            segment.last_pid = child;
            publish_root(
                &ATTACHMENT_ROOT_ADDRESSES,
                &ATTACHMENT_ROOT_GENERATIONS,
                guard.instance,
                Some(clone),
            );
        }
        current = next;
    }
    let _ = detach_matching(guard.instance, child, None);
    Err(Error::Invalid)
}

pub(crate) fn drop_process_attachments(pid: u32) -> Result<(), Error> {
    let guard = Guard::acquire().ok_or(Error::Invalid)?;
    detach_matching(guard.instance, pid, None).map(|_| ())
}

pub(crate) fn teardown() -> bool {
    let Some(guard) = Guard::acquire() else {
        return false;
    };
    let mut attachments = root(
        &ATTACHMENT_ROOT_ADDRESSES,
        &ATTACHMENT_ROOT_GENERATIONS,
        guard.instance,
    );
    for _ in 0..WALK_LIMIT {
        let Some(record) = attachments else { break };
        let Some(entry) = attachment(record) else {
            return false;
        };
        attachments = (entry.next != memory::FrameLease::EMPTY).then_some(entry.next);
        if !memory::scrub_and_release_frames(memory::FrameOwner::KERNEL, &[record]) {
            return false;
        }
    }
    if attachments.is_some() {
        return false;
    }
    publish_root(
        &ATTACHMENT_ROOT_ADDRESSES,
        &ATTACHMENT_ROOT_GENERATIONS,
        guard.instance,
        None,
    );
    let mut segments = root(&ROOT_ADDRESSES, &ROOT_GENERATIONS, guard.instance);
    publish_root(&ROOT_ADDRESSES, &ROOT_GENERATIONS, guard.instance, None);
    for _ in 0..WALK_LIMIT {
        let Some(segment) = segments else { return true };
        let Some(page) = segment_page(segment) else {
            return false;
        };
        segments = (page.next_segment != memory::FrameLease::EMPTY).then_some(page.next_segment);
        if !release_segment_chain(segment) {
            return false;
        }
    }
    false
}

pub(crate) fn mark_for_removal(id: u32, pid: u32) -> Result<(), Error> {
    let guard = Guard::acquire().ok_or(Error::Invalid)?;
    let (frame, previous) = find_segment(guard.instance, id).ok_or(Error::Invalid)?;
    let page = segment_page_mut(frame).ok_or(Error::Invalid)?;
    page.marked_for_removal = 1;
    page.last_pid = pid;
    if page.attachments == 0 && !unlink_and_release(guard.instance, frame, previous) {
        return Err(Error::Invalid);
    }
    Ok(())
}

pub(crate) fn set_mode(id: u32, mode: u16, pid: u32) -> Result<(), Error> {
    let guard = Guard::acquire().ok_or(Error::Invalid)?;
    let (frame, _) = find_segment(guard.instance, id).ok_or(Error::Invalid)?;
    let page = segment_page_mut(frame).ok_or(Error::Invalid)?;
    page.mode = mode & 0o777;
    page.last_pid = pid;
    Ok(())
}

pub(crate) fn verify_shared_memory_lifecycle() -> bool {
    let Some(before) = memory::owner_live_frame_count(memory::FrameOwner::KERNEL) else {
        return false;
    };
    let Ok(id) = get_or_create(0x4749_4e55, 2 * PAGE_SIZE as u64, 0o600, true, true, 1) else {
        return false;
    };
    let verified = (|| {
        let first = frame(id, 0).ok()?;
        let second = frame(id, 1).ok()?;
        let first_address = first.direct_map_address()?;
        let second_address = second.direct_map_address()?;
        unsafe {
            (first_address as *mut u8).write(0x47);
            (second_address as *mut u8).write(0x58);
        }
        attach(id, 0x4000_0000, 1).ok()?;
        inherit_process_attachments(1, 2).ok()?;
        let state = segment(id).ok()?;
        if state.size != 2 * PAGE_SIZE as u64
            || state.mode != 0o600
            || state.attachments != 2
            || attachment_at(0x4000_0000, 1).ok()? != (id, 2 * PAGE_SIZE as u64)
            || attachment_at(0x4000_0000, 2).ok()? != (id, 2 * PAGE_SIZE as u64)
            || unsafe { (first_address as *const u8).read() } != 0x47
            || unsafe { (second_address as *const u8).read() } != 0x58
        {
            return None;
        }
        mark_for_removal(id, 1).ok()?;
        if segment(id).ok()?.attachments != 2 {
            return None;
        }
        detach(0x4000_0000, 1).ok()?;
        if segment(id).ok()?.attachments != 1 {
            return None;
        }
        drop_process_attachments(2).ok()?;
        matches!(segment(id), Err(Error::Invalid)).then_some(())
    })()
    .is_some();
    verified
        && memory::owner_live_frame_count(memory::FrameOwner::KERNEL) == Some(before)
        && get_or_create(0x4749_4e55, 0, 0, false, false, 1) == Err(Error::NoEntry)
}
