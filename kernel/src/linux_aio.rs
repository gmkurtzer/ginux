use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

use crate::{control_abi, linux_process, memory};

const INSTANCE_LIMIT: usize = control_abi::DOMAIN_LIMIT + 1;
const WALK_LIMIT: usize = 262_144;

struct AioRuntimeOwner {
    locks: [AtomicBool; INSTANCE_LIMIT],
    root_addresses: [AtomicU64; INSTANCE_LIMIT],
    root_generations: [AtomicU32; INSTANCE_LIMIT],
}

static AIO_RUNTIME: AioRuntimeOwner = AioRuntimeOwner {
    locks: [const { AtomicBool::new(false) }; INSTANCE_LIMIT],
    root_addresses: [const { AtomicU64::new(0) }; INSTANCE_LIMIT],
    root_generations: [const { AtomicU32::new(0) }; INSTANCE_LIMIT],
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Error {
    Invalid,
    NoMemory,
    Full,
}

#[derive(Clone, Copy)]
pub(crate) struct Event {
    pub(crate) data: u64,
    pub(crate) object: u64,
    pub(crate) result: i64,
    pub(crate) result2: i64,
}

#[repr(C)]
struct Context {
    next: memory::FrameLease,
    id: u64,
    maximum: u64,
    pending: u64,
    ring_pages: u64,
    head: memory::FrameLease,
    tail: memory::FrameLease,
}

#[repr(C)]
struct EventPage {
    next: memory::FrameLease,
    event: Event,
}

struct Guard {
    instance: usize,
}
impl Guard {
    fn acquire() -> Option<Self> {
        let instance = linux_process::current_ipc_instance();
        let lock = AIO_RUNTIME.locks.get(instance)?;
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
        AIO_RUNTIME.locks[self.instance].store(false, Ordering::Release);
    }
}

fn lease() -> Option<memory::FrameLease> {
    memory::allocate_frames::<1>(memory::FrameOwner::KERNEL).map(|frames| frames[0])
}
fn zero(frame: memory::FrameLease) -> bool {
    frame.direct_map_address().is_some_and(|address| {
        unsafe { core::ptr::write_bytes(address as *mut u8, 0, 4096) };
        true
    })
}
fn root(instance: usize) -> Option<memory::FrameLease> {
    memory::FrameLease::from_parts(
        AIO_RUNTIME.root_addresses[instance].load(Ordering::Acquire),
        AIO_RUNTIME.root_generations[instance].load(Ordering::Acquire),
    )
}
fn publish(instance: usize, value: Option<memory::FrameLease>) {
    let value = value.unwrap_or(memory::FrameLease::EMPTY);
    AIO_RUNTIME.root_generations[instance].store(value.generation(), Ordering::Release);
    AIO_RUNTIME.root_addresses[instance].store(value.address(), Ordering::Release);
}
fn context(frame: memory::FrameLease) -> Option<&'static Context> {
    Some(unsafe { &*(frame.direct_map_address()? as *const Context) })
}
fn context_mut(frame: memory::FrameLease) -> Option<&'static mut Context> {
    Some(unsafe { &mut *(frame.direct_map_address()? as *mut Context) })
}
fn event(frame: memory::FrameLease) -> Option<&'static EventPage> {
    Some(unsafe { &*(frame.direct_map_address()? as *const EventPage) })
}
fn event_mut(frame: memory::FrameLease) -> Option<&'static mut EventPage> {
    Some(unsafe { &mut *(frame.direct_map_address()? as *mut EventPage) })
}

fn find(instance: usize, id: u64) -> Option<(memory::FrameLease, Option<memory::FrameLease>)> {
    let mut current = root(instance);
    let mut previous = None;
    for _ in 0..WALK_LIMIT {
        let frame = current?;
        let item = context(frame)?;
        if item.id == id {
            return Some((frame, previous));
        }
        previous = Some(frame);
        current = (item.next != memory::FrameLease::EMPTY).then_some(item.next);
    }
    None
}

pub(crate) fn create(id: u64, maximum: u64, ring_pages: u64) -> Result<(), Error> {
    if id == 0 || maximum == 0 || ring_pages == 0 {
        return Err(Error::Invalid);
    }
    let guard = Guard::acquire().ok_or(Error::Invalid)?;
    if find(guard.instance, id).is_some() {
        return Err(Error::Invalid);
    }
    let frame = lease().ok_or(Error::NoMemory)?;
    if !zero(frame) {
        let _ = memory::scrub_and_release_frames(memory::FrameOwner::KERNEL, &[frame]);
        return Err(Error::NoMemory);
    }
    let old = root(guard.instance);
    let item = context_mut(frame).ok_or(Error::NoMemory)?;
    item.next = old.unwrap_or(memory::FrameLease::EMPTY);
    item.id = id;
    item.maximum = maximum;
    item.ring_pages = ring_pages;
    publish(guard.instance, Some(frame));
    Ok(())
}

pub(crate) fn push(id: u64, value: Event) -> Result<(), Error> {
    let guard = Guard::acquire().ok_or(Error::Invalid)?;
    let (frame, _) = find(guard.instance, id).ok_or(Error::Invalid)?;
    let item = context(frame).ok_or(Error::Invalid)?;
    if item.pending >= item.maximum {
        return Err(Error::Full);
    }
    let node = lease().ok_or(Error::NoMemory)?;
    if !zero(node) {
        let _ = memory::scrub_and_release_frames(memory::FrameOwner::KERNEL, &[node]);
        return Err(Error::NoMemory);
    }
    event_mut(node).ok_or(Error::NoMemory)?.event = value;
    let item = context_mut(frame).ok_or(Error::Invalid)?;
    if item.tail != memory::FrameLease::EMPTY {
        event_mut(item.tail).ok_or(Error::Invalid)?.next = node;
    } else {
        item.head = node;
    }
    item.tail = node;
    item.pending += 1;
    Ok(())
}

pub(crate) fn pop(id: u64) -> Result<Option<Event>, Error> {
    let guard = Guard::acquire().ok_or(Error::Invalid)?;
    let (frame, _) = find(guard.instance, id).ok_or(Error::Invalid)?;
    let item = context_mut(frame).ok_or(Error::Invalid)?;
    if item.head == memory::FrameLease::EMPTY {
        return Ok(None);
    }
    let node = item.head;
    let page = event(node).ok_or(Error::Invalid)?;
    let value = page.event;
    item.head = page.next;
    if item.head == memory::FrameLease::EMPTY {
        item.tail = memory::FrameLease::EMPTY;
    }
    item.pending = item.pending.checked_sub(1).ok_or(Error::Invalid)?;
    memory::scrub_and_release_frames(memory::FrameOwner::KERNEL, &[node])
        .then_some(Some(value))
        .ok_or(Error::Invalid)
}

pub(crate) fn info(id: u64) -> Result<(u64, u64), Error> {
    let guard = Guard::acquire().ok_or(Error::Invalid)?;
    let (frame, _) = find(guard.instance, id).ok_or(Error::Invalid)?;
    let item = context(frame).ok_or(Error::Invalid)?;
    Ok((item.pending, item.ring_pages))
}

pub(crate) fn can_submit(id: u64) -> Result<bool, Error> {
    let guard = Guard::acquire().ok_or(Error::Invalid)?;
    let (frame, _) = find(guard.instance, id).ok_or(Error::Invalid)?;
    let item = context(frame).ok_or(Error::Invalid)?;
    Ok(item.pending < item.maximum)
}

pub(crate) fn destroy(id: u64) -> Result<u64, Error> {
    let guard = Guard::acquire().ok_or(Error::Invalid)?;
    let (frame, previous) = find(guard.instance, id).ok_or(Error::Invalid)?;
    let item = context(frame).ok_or(Error::Invalid)?;
    let next = item.next;
    let pages = item.ring_pages;
    let mut node = item.head;
    for _ in 0..WALK_LIMIT {
        if node == memory::FrameLease::EMPTY {
            break;
        }
        let following = event(node).ok_or(Error::Invalid)?.next;
        if !memory::scrub_and_release_frames(memory::FrameOwner::KERNEL, &[node]) {
            return Err(Error::Invalid);
        }
        node = following;
    }
    if node != memory::FrameLease::EMPTY {
        return Err(Error::Invalid);
    }
    if let Some(previous) = previous {
        context_mut(previous).ok_or(Error::Invalid)?.next = next;
    } else {
        publish(
            guard.instance,
            (next != memory::FrameLease::EMPTY).then_some(next),
        );
    }
    memory::scrub_and_release_frames(memory::FrameOwner::KERNEL, &[frame])
        .then_some(pages)
        .ok_or(Error::Invalid)
}

pub(crate) fn teardown() -> bool {
    let Some(guard) = Guard::acquire() else {
        return false;
    };
    let mut current = root(guard.instance);
    publish(guard.instance, None);
    for _ in 0..WALK_LIMIT {
        let Some(frame) = current else { return true };
        let Ok(item) = context(frame).ok_or(Error::Invalid) else {
            return false;
        };
        current = (item.next != memory::FrameLease::EMPTY).then_some(item.next);
        let mut node = item.head;
        for _ in 0..WALK_LIMIT {
            let Some(page) = (node != memory::FrameLease::EMPTY)
                .then(|| event(node))
                .flatten()
            else {
                break;
            };
            let next = page.next;
            if !memory::scrub_and_release_frames(memory::FrameOwner::KERNEL, &[node]) {
                return false;
            }
            node = next;
        }
        if !memory::scrub_and_release_frames(memory::FrameOwner::KERNEL, &[frame]) {
            return false;
        }
    }
    false
}
