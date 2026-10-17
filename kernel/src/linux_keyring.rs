use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

use crate::{control_abi, linux_process, memory};

const PAGE_SIZE: usize = 4096;
const INSTANCE_LIMIT: usize = control_abi::DOMAIN_LIMIT + 1;
const WALK_LIMIT: usize = 262_144;
pub(crate) const TYPE_CAPACITY: usize = 32;
pub(crate) const DESCRIPTION_CAPACITY: usize = 256;
pub(crate) const PAYLOAD_CAPACITY: usize = 3760;
pub(crate) const PROCESS_KEYRING: i32 = 1;
pub(crate) const SESSION_KEYRING: i32 = 2;
pub(crate) const USER_KEYRING: i32 = 3;

static LOCKS: [AtomicBool; INSTANCE_LIMIT] = [const { AtomicBool::new(false) }; INSTANCE_LIMIT];
static ROOT_ADDRESSES: [AtomicU64; INSTANCE_LIMIT] = [const { AtomicU64::new(0) }; INSTANCE_LIMIT];
static ROOT_GENERATIONS: [AtomicU32; INSTANCE_LIMIT] =
    [const { AtomicU32::new(0) }; INSTANCE_LIMIT];
static NEXT_SERIALS: [AtomicU32; INSTANCE_LIMIT] = [const { AtomicU32::new(100) }; INSTANCE_LIMIT];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Error {
    Invalid,
    NoKey,
    NoMemory,
    Revoked,
    TooBig,
}

#[derive(Clone, Copy)]
pub(crate) struct KeyInfo {
    pub(crate) type_length: usize,
    pub(crate) description_length: usize,
}

#[repr(C)]
struct KeyPage {
    next: memory::FrameLease,
    serial: i32,
    type_length: u16,
    description_length: u16,
    payload_length: u32,
    revoked: u8,
    _padding: [u8; 3],
    key_type: [u8; TYPE_CAPACITY],
    description: [u8; DESCRIPTION_CAPACITY],
    payload: [u8; PAYLOAD_CAPACITY],
}

const _: () = assert!(core::mem::size_of::<KeyPage>() <= PAGE_SIZE);

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

fn root(instance: usize) -> Option<memory::FrameLease> {
    memory::FrameLease::from_parts(
        ROOT_ADDRESSES[instance].load(Ordering::Acquire),
        ROOT_GENERATIONS[instance].load(Ordering::Acquire),
    )
}

fn publish_root(instance: usize, root: Option<memory::FrameLease>) {
    let root = root.unwrap_or(memory::FrameLease::EMPTY);
    ROOT_GENERATIONS[instance].store(root.generation(), Ordering::Release);
    ROOT_ADDRESSES[instance].store(root.address(), Ordering::Release);
}

fn page(frame: memory::FrameLease) -> Option<&'static KeyPage> {
    Some(unsafe { &*(frame.direct_map_address()? as *const KeyPage) })
}

fn page_mut(frame: memory::FrameLease) -> Option<&'static mut KeyPage> {
    Some(unsafe { &mut *(frame.direct_map_address()? as *mut KeyPage) })
}

fn find(instance: usize, serial: i32) -> Option<memory::FrameLease> {
    let mut current = root(instance);
    for _ in 0..WALK_LIMIT {
        let frame = current?;
        let key = page(frame)?;
        if key.serial == serial {
            return Some(frame);
        }
        current = (key.next != memory::FrameLease::EMPTY).then_some(key.next);
    }
    None
}

fn matches(key: &KeyPage, key_type: &[u8], description: &[u8]) -> bool {
    key.revoked == 0
        && usize::from(key.type_length) == key_type.len()
        && usize::from(key.description_length) == description.len()
        && &key.key_type[..key_type.len()] == key_type
        && &key.description[..description.len()] == description
}

pub(crate) fn add(key_type: &[u8], description: &[u8], payload: &[u8]) -> Result<i32, Error> {
    if key_type.is_empty()
        || key_type.len() > TYPE_CAPACITY
        || description.is_empty()
        || description.len() > DESCRIPTION_CAPACITY
    {
        return Err(Error::Invalid);
    }
    if payload.len() > PAYLOAD_CAPACITY {
        return Err(Error::TooBig);
    }
    let guard = Guard::acquire().ok_or(Error::Invalid)?;
    let [frame] =
        memory::allocate_frames::<1>(memory::FrameOwner::KERNEL).ok_or(Error::NoMemory)?;
    let Some(address) = frame.direct_map_address() else {
        let _ = memory::scrub_and_release_frames(memory::FrameOwner::KERNEL, &[frame]);
        return Err(Error::NoMemory);
    };
    unsafe { core::ptr::write_bytes(address as *mut u8, 0, PAGE_SIZE) };
    let serial = NEXT_SERIALS[guard.instance]
        .fetch_add(1, Ordering::SeqCst)
        .max(100) as i32;
    let key = page_mut(frame).ok_or(Error::NoMemory)?;
    key.next = root(guard.instance).unwrap_or(memory::FrameLease::EMPTY);
    key.serial = serial;
    key.type_length = key_type.len() as u16;
    key.description_length = description.len() as u16;
    key.payload_length = payload.len() as u32;
    key.key_type[..key_type.len()].copy_from_slice(key_type);
    key.description[..description.len()].copy_from_slice(description);
    key.payload[..payload.len()].copy_from_slice(payload);
    publish_root(guard.instance, Some(frame));
    Ok(serial)
}

pub(crate) fn search(key_type: &[u8], description: &[u8]) -> Result<i32, Error> {
    let guard = Guard::acquire().ok_or(Error::Invalid)?;
    let mut current = root(guard.instance);
    for _ in 0..WALK_LIMIT {
        let frame = current.ok_or(Error::NoKey)?;
        let key = page(frame).ok_or(Error::Invalid)?;
        if matches(key, key_type, description) {
            return Ok(key.serial);
        }
        current = (key.next != memory::FrameLease::EMPTY).then_some(key.next);
    }
    Err(Error::Invalid)
}

pub(crate) fn read(serial: i32, output: &mut [u8]) -> Result<usize, Error> {
    let guard = Guard::acquire().ok_or(Error::Invalid)?;
    let key = page(find(guard.instance, serial).ok_or(Error::NoKey)?).ok_or(Error::Invalid)?;
    if key.revoked != 0 {
        return Err(Error::Revoked);
    }
    let length = key.payload_length as usize;
    let copied = length.min(output.len());
    output[..copied].copy_from_slice(&key.payload[..copied]);
    Ok(length)
}

pub(crate) fn describe(
    serial: i32,
    key_type: &mut [u8],
    description: &mut [u8],
) -> Result<KeyInfo, Error> {
    let guard = Guard::acquire().ok_or(Error::Invalid)?;
    let key = page(find(guard.instance, serial).ok_or(Error::NoKey)?).ok_or(Error::Invalid)?;
    if key.revoked != 0 {
        return Err(Error::Revoked);
    }
    let info = KeyInfo {
        type_length: usize::from(key.type_length),
        description_length: usize::from(key.description_length),
    };
    key_type[..info.type_length].copy_from_slice(&key.key_type[..info.type_length]);
    description[..info.description_length]
        .copy_from_slice(&key.description[..info.description_length]);
    Ok(info)
}

pub(crate) fn update(serial: i32, payload: &[u8]) -> Result<(), Error> {
    if payload.len() > PAYLOAD_CAPACITY {
        return Err(Error::TooBig);
    }
    let guard = Guard::acquire().ok_or(Error::Invalid)?;
    let key = page_mut(find(guard.instance, serial).ok_or(Error::NoKey)?).ok_or(Error::Invalid)?;
    if key.revoked != 0 {
        return Err(Error::Revoked);
    }
    key.payload.fill(0);
    key.payload[..payload.len()].copy_from_slice(payload);
    key.payload_length = payload.len() as u32;
    Ok(())
}

pub(crate) fn revoke(serial: i32) -> Result<(), Error> {
    let guard = Guard::acquire().ok_or(Error::Invalid)?;
    let key = page_mut(find(guard.instance, serial).ok_or(Error::NoKey)?).ok_or(Error::Invalid)?;
    key.revoked = 1;
    Ok(())
}

pub(crate) fn clear() -> bool {
    let Some(guard) = Guard::acquire() else {
        return false;
    };
    let mut current = root(guard.instance);
    publish_root(guard.instance, None);
    for _ in 0..WALK_LIMIT {
        let Some(frame) = current else { return true };
        let Some(key) = page(frame) else { return false };
        let next = (key.next != memory::FrameLease::EMPTY).then_some(key.next);
        if !memory::scrub_and_release_frames(memory::FrameOwner::KERNEL, &[frame]) {
            return false;
        }
        current = next;
    }
    false
}

pub(crate) fn teardown() -> bool {
    let released = clear();
    if released {
        let instance = linux_process::current_ipc_instance();
        NEXT_SERIALS[instance].store(100, Ordering::SeqCst);
    }
    released
}
