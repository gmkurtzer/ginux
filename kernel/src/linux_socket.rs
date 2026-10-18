#![allow(static_mut_refs)]

use core::{
    ptr::{read_volatile, write_volatile},
    sync::atomic::{compiler_fence, Ordering},
};

use smoltcp::{
    iface::{Config as InterfaceConfig, Interface, SocketHandle, SocketSet, SocketStorage},
    phy::{Device, DeviceCapabilities, Medium, PacketMeta, RxToken, TxToken},
    socket::{tcp, udp},
    time::{Duration, Instant},
    wire::{EthernetAddress, IpAddress, IpCidr, IpEndpoint, Ipv4Address},
};

use crate::{
    arch::x86_64::{
        iommu::{DeviceTranslation, DeviceTranslationProfile},
        paging::MmioMapping,
        pci::{self, VirtioNetworkDevice, VirtioNetworkInitialization},
    },
    memory, network,
};

const SLOT_COUNT: usize = 12;
pub(crate) const PASSED_RIGHT_LIMIT: usize = SLOT_COUNT;
const TCP_BUFFER_SIZE: usize = 64 * 1024;
const UDP_BUFFER_SIZE: usize = 16 * 1024;
const UDP_PACKET_COUNT: usize = 16;
const QUEUE_SIZE: u16 = 8;
const RX_DESC_OFFSET: usize = 0x000;
const RX_AVAIL_OFFSET: usize = 0x080;
const RX_USED_OFFSET: usize = 0x100;
const TX_DESC_OFFSET: usize = 0x180;
const TX_AVAIL_OFFSET: usize = 0x200;
const TX_USED_OFFSET: usize = 0x280;
const RX_BUFFER_OFFSET: usize = 0x400;
const TX_BUFFER_OFFSET: usize = 0xa00;
const RX_DESCRIPTOR_LENGTH: usize = 0x5ff;
const VIRTIO_HEADER_LENGTH: usize = 12;
const BLOCKING_POLL_LIMIT: usize = 20_000;
const PAGE_SIZE: usize = 4096;
const LOCAL_DATA_CAPACITY: usize =
    PAGE_SIZE - core::mem::size_of::<memory::FrameLease>() - core::mem::size_of::<u32>();

#[repr(C)]
struct LocalDataPage {
    next: memory::FrameLease,
    used: u32,
    bytes: [u8; LOCAL_DATA_CAPACITY],
}

const _: () = assert!(core::mem::size_of::<LocalDataPage>() == PAGE_SIZE);

#[derive(Clone, Copy)]
struct Platform {
    device: VirtioNetworkDevice,
    initialization: VirtioNetworkInitialization,
    common: MmioMapping,
    notification: MmioMapping,
    iommu: DeviceTranslationProfile,
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum Kind {
    Tcp,
    Udp,
    Unix,
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum Error {
    Unavailable,
    Exhausted,
    AddressInUse,
    AddressNotAvailable,
    Invalid,
    InProgress,
    WouldBlock,
    NotConnected,
    BrokenPipe,
    Reset,
}

#[derive(Clone, Copy)]
pub(crate) enum BooleanOption {
    ReuseAddress,
    KeepAlive,
    ReceiveErrors,
    NoDelay,
    PassCredentials,
}

#[derive(Clone, Copy)]
pub(crate) enum TimeoutOption {
    Receive,
    Send,
}

#[derive(Clone, Copy)]
struct Slot {
    occupied: bool,
    references: u16,
    kind: Kind,
    handle: Option<SocketHandle>,
    remote: Option<IpEndpoint>,
    local_port: u16,
    tcp_buffer: Option<u8>,
    listening: bool,
    backlog: u8,
    pending_length: u8,
    pending: [u8; SLOT_COUNT],
    peer: Option<u8>,
    read_shutdown: bool,
    write_shutdown: bool,
    reuse_address: bool,
    keep_alive: bool,
    receive_errors: bool,
    no_delay: bool,
    pass_credentials: bool,
    receive_timeout_us: u64,
    send_timeout_us: u64,
    pending_right_count: u8,
    pending_right_slots: [u8; SLOT_COUNT],
    pending_right_flags: [u32; SLOT_COUNT],
    pending_credentials: Option<(u32, u32, u32)>,
    peer_credentials: Option<(u32, u32, u32)>,
    local_first: memory::FrameLease,
    local_last: memory::FrameLease,
    local_head: usize,
    local_length: usize,
}

impl Slot {
    const EMPTY: Self = Self {
        occupied: false,
        references: 0,
        kind: Kind::Tcp,
        handle: None,
        remote: None,
        local_port: 0,
        tcp_buffer: None,
        listening: false,
        backlog: 0,
        pending_length: 0,
        pending: [0; SLOT_COUNT],
        peer: None,
        read_shutdown: false,
        write_shutdown: false,
        reuse_address: false,
        keep_alive: false,
        receive_errors: false,
        no_delay: false,
        pass_credentials: false,
        receive_timeout_us: 0,
        send_timeout_us: 0,
        pending_right_count: 0,
        pending_right_slots: [0; SLOT_COUNT],
        pending_right_flags: [0; SLOT_COUNT],
        pending_credentials: None,
        peer_credentials: None,
        local_first: memory::FrameLease::EMPTY,
        local_last: memory::FrameLease::EMPTY,
        local_head: 0,
        local_length: 0,
    };
}

struct Runtime {
    interface: Interface,
    device: VirtioNet,
    sockets: SocketSet<'static>,
    _dma: memory::FrameLease,
    _translation: DeviceTranslation,
    tick: i64,
    next_port: u16,
    local_address: u32,
}

static mut PLATFORM: Option<Platform> = None;
static mut RUNTIME: Option<Runtime> = None;
static mut SLOTS: [Slot; SLOT_COUNT] = [Slot::EMPTY; SLOT_COUNT];
static mut UNIX_INFLIGHT_RIGHTS: usize = 0;
static mut SOCKET_STORAGE: [SocketStorage<'static>; SLOT_COUNT] =
    [SocketStorage::EMPTY; SLOT_COUNT];
static mut TCP_RX: [[u8; TCP_BUFFER_SIZE]; SLOT_COUNT] = [[0; TCP_BUFFER_SIZE]; SLOT_COUNT];
static mut TCP_TX: [[u8; TCP_BUFFER_SIZE]; SLOT_COUNT] = [[0; TCP_BUFFER_SIZE]; SLOT_COUNT];
static mut UDP_RX: [[u8; UDP_BUFFER_SIZE]; SLOT_COUNT] = [[0; UDP_BUFFER_SIZE]; SLOT_COUNT];
static mut UDP_TX: [[u8; UDP_BUFFER_SIZE]; SLOT_COUNT] = [[0; UDP_BUFFER_SIZE]; SLOT_COUNT];
static mut UDP_RX_META: [[udp::PacketMetadata; UDP_PACKET_COUNT]; SLOT_COUNT] =
    [[udp::PacketMetadata::EMPTY; UDP_PACKET_COUNT]; SLOT_COUNT];
static mut UDP_TX_META: [[udp::PacketMetadata; UDP_PACKET_COUNT]; SLOT_COUNT] =
    [[udp::PacketMetadata::EMPTY; UDP_PACKET_COUNT]; SLOT_COUNT];

fn free_tcp_buffer() -> Option<usize> {
    (0..SLOT_COUNT).find(|candidate| unsafe {
        SLOTS
            .iter()
            .all(|slot| !slot.occupied || slot.tcp_buffer.map(usize::from) != Some(*candidate))
    })
}

fn local_page(frame: memory::FrameLease) -> Option<&'static LocalDataPage> {
    let address = frame.direct_map_address()?;
    Some(unsafe { &*(address as *const LocalDataPage) })
}

fn local_page_mut(frame: memory::FrameLease) -> Option<&'static mut LocalDataPage> {
    let address = frame.direct_map_address()?;
    Some(unsafe { &mut *(address as *mut LocalDataPage) })
}

fn release_local_pages(mut page: memory::FrameLease) -> bool {
    while page != memory::FrameLease::EMPTY {
        let Some(next) = local_page(page).map(|data| data.next) else {
            return false;
        };
        if !memory::scrub_and_release_frames(
            memory::FrameOwner::KERNEL,
            core::slice::from_ref(&page),
        ) {
            return false;
        }
        page = next;
    }
    true
}

fn allocate_local_pages(input: &[u8]) -> Option<(memory::FrameLease, memory::FrameLease)> {
    let mut first = memory::FrameLease::EMPTY;
    let mut last = memory::FrameLease::EMPTY;
    for chunk in input.chunks(LOCAL_DATA_CAPACITY) {
        let Some([page]) = memory::allocate_frames::<1>(memory::FrameOwner::KERNEL) else {
            let _ = release_local_pages(first);
            return None;
        };
        let Some(data) = local_page_mut(page) else {
            let _ = memory::scrub_and_release_frames(memory::FrameOwner::KERNEL, &[page]);
            let _ = release_local_pages(first);
            return None;
        };
        data.next = memory::FrameLease::EMPTY;
        data.used = chunk.len() as u32;
        data.bytes[..chunk.len()].copy_from_slice(chunk);
        if last == memory::FrameLease::EMPTY {
            first = page;
        } else {
            local_page_mut(last)?.next = page;
        }
        last = page;
    }
    (first != memory::FrameLease::EMPTY).then_some((first, last))
}

pub(crate) fn configure(
    device: VirtioNetworkDevice,
    initialization: VirtioNetworkInitialization,
    common: MmioMapping,
    notification: MmioMapping,
    iommu: DeviceTranslationProfile,
) -> bool {
    unsafe {
        if PLATFORM.is_some() {
            return false;
        }
        PLATFORM = Some(Platform {
            device,
            initialization,
            common,
            notification,
            iommu,
        });
    }
    true
}

pub(crate) fn open(kind: Kind) -> Result<u8, Error> {
    let runtime = runtime()?;
    let slot = unsafe { SLOTS.iter().position(|slot| !slot.occupied) }.ok_or(Error::Exhausted)?;
    let tcp_buffer = if kind == Kind::Tcp {
        Some(free_tcp_buffer().ok_or(Error::Exhausted)? as u8)
    } else {
        None
    };
    let handle = match kind {
        Kind::Tcp => {
            let buffer = usize::from(tcp_buffer.ok_or(Error::Exhausted)?);
            let rx = tcp::SocketBuffer::new(unsafe {
                core::slice::from_raw_parts_mut(TCP_RX[buffer].as_mut_ptr(), TCP_BUFFER_SIZE)
            });
            let tx = tcp::SocketBuffer::new(unsafe {
                core::slice::from_raw_parts_mut(TCP_TX[buffer].as_mut_ptr(), TCP_BUFFER_SIZE)
            });
            runtime.sockets.add(tcp::Socket::new(rx, tx))
        }
        Kind::Udp => {
            let rx = udp::PacketBuffer::new(
                unsafe {
                    core::slice::from_raw_parts_mut(
                        UDP_RX_META[slot].as_mut_ptr(),
                        UDP_PACKET_COUNT,
                    )
                },
                unsafe {
                    core::slice::from_raw_parts_mut(UDP_RX[slot].as_mut_ptr(), UDP_BUFFER_SIZE)
                },
            );
            let tx = udp::PacketBuffer::new(
                unsafe {
                    core::slice::from_raw_parts_mut(
                        UDP_TX_META[slot].as_mut_ptr(),
                        UDP_PACKET_COUNT,
                    )
                },
                unsafe {
                    core::slice::from_raw_parts_mut(UDP_TX[slot].as_mut_ptr(), UDP_BUFFER_SIZE)
                },
            );
            runtime.sockets.add(udp::Socket::new(rx, tx))
        }
        Kind::Unix => return Err(Error::Invalid),
    };
    unsafe {
        SLOTS[slot] = Slot {
            occupied: true,
            references: 1,
            kind,
            handle: Some(handle),
            remote: None,
            local_port: 0,
            tcp_buffer,
            listening: false,
            backlog: 0,
            pending_length: 0,
            pending: [0; SLOT_COUNT],
            peer: None,
            read_shutdown: false,
            write_shutdown: false,
            reuse_address: false,
            keep_alive: false,
            receive_errors: false,
            no_delay: false,
            pass_credentials: false,
            receive_timeout_us: 0,
            send_timeout_us: 0,
            pending_right_count: 0,
            pending_right_slots: [0; SLOT_COUNT],
            pending_right_flags: [0; SLOT_COUNT],
            pending_credentials: None,
            peer_credentials: None,
            local_first: memory::FrameLease::EMPTY,
            local_last: memory::FrameLease::EMPTY,
            local_head: 0,
            local_length: 0,
        };
    }
    Ok(slot as u8)
}

pub(crate) fn open_pair() -> Result<(u8, u8), Error> {
    let first = unsafe { SLOTS.iter().position(|slot| !slot.occupied) }.ok_or(Error::Exhausted)?;
    let second = unsafe {
        SLOTS
            .iter()
            .enumerate()
            .find(|(index, slot)| *index != first && !slot.occupied)
            .map(|(index, _)| index)
    }
    .ok_or(Error::Exhausted)?;
    unsafe {
        SLOTS[first] = Slot {
            occupied: true,
            references: 1,
            kind: Kind::Unix,
            handle: None,
            remote: None,
            local_port: 0,
            tcp_buffer: None,
            listening: false,
            backlog: 0,
            pending_length: 0,
            pending: [0; SLOT_COUNT],
            peer: Some(second as u8),
            read_shutdown: false,
            write_shutdown: false,
            reuse_address: false,
            keep_alive: false,
            receive_errors: false,
            no_delay: false,
            pass_credentials: false,
            receive_timeout_us: 0,
            send_timeout_us: 0,
            pending_right_count: 0,
            pending_right_slots: [0; SLOT_COUNT],
            pending_right_flags: [0; SLOT_COUNT],
            pending_credentials: None,
            peer_credentials: None,
            local_first: memory::FrameLease::EMPTY,
            local_last: memory::FrameLease::EMPTY,
            local_head: 0,
            local_length: 0,
        };
        SLOTS[second] = Slot {
            occupied: true,
            references: 1,
            kind: Kind::Unix,
            handle: None,
            remote: None,
            local_port: 0,
            tcp_buffer: None,
            listening: false,
            backlog: 0,
            pending_length: 0,
            pending: [0; SLOT_COUNT],
            peer: Some(first as u8),
            read_shutdown: false,
            write_shutdown: false,
            reuse_address: false,
            keep_alive: false,
            receive_errors: false,
            no_delay: false,
            pass_credentials: false,
            receive_timeout_us: 0,
            send_timeout_us: 0,
            pending_right_count: 0,
            pending_right_slots: [0; SLOT_COUNT],
            pending_right_flags: [0; SLOT_COUNT],
            pending_credentials: None,
            peer_credentials: None,
            local_first: memory::FrameLease::EMPTY,
            local_last: memory::FrameLease::EMPTY,
            local_head: 0,
            local_length: 0,
        };
    }
    Ok((first as u8, second as u8))
}

pub(crate) fn close(slot: u8) -> bool {
    let index = usize::from(slot);
    let Some(state) = (unsafe { SLOTS.get_mut(index) }).filter(|slot| slot.occupied) else {
        return false;
    };
    if state.references > 1 {
        state.references -= 1;
        return true;
    }
    if state.kind == Kind::Unix || state.peer.is_some() && !state.listening {
        let pending_right_count = usize::from(state.pending_right_count);
        if pending_right_count > unsafe { UNIX_INFLIGHT_RIGHTS } {
            return false;
        }
        if !release_local_pages(state.local_first) {
            return false;
        }
        if state.kind == Kind::Tcp {
            if let Some(handle) = state.handle.take() {
                let Some(runtime) = runtime_existing() else {
                    return false;
                };
                runtime.sockets.remove(handle);
            }
        }
        let pending_rights = state.pending_right_slots;
        *state = Slot::EMPTY;
        unsafe { UNIX_INFLIGHT_RIGHTS -= pending_right_count };
        for right in pending_rights.into_iter().take(pending_right_count) {
            let _ = close(right);
        }
        return true;
    }
    let Some(runtime) = runtime_existing() else {
        return false;
    };
    let Some(handle) = state.handle.take() else {
        return false;
    };
    runtime.sockets.remove(handle);
    *state = Slot::EMPTY;
    true
}

pub(crate) fn shutdown_socket(slot: u8, how: u8) -> Result<(), Error> {
    if how > 2 {
        return Err(Error::Invalid);
    }
    let index = usize::from(slot);
    let state = unsafe { SLOTS.get_mut(index) }
        .filter(|state| state.occupied && !state.listening)
        .ok_or(Error::NotConnected)?;
    let peer_connected = state.peer.is_some_and(|peer| {
        unsafe { SLOTS.get(usize::from(peer)) }.is_some_and(|peer| peer.occupied)
    });
    if state.remote.is_none() && !peer_connected {
        return Err(Error::NotConnected);
    }
    if matches!(how, 0 | 2) {
        if !release_local_pages(state.local_first) {
            return Err(Error::Invalid);
        }
        state.local_first = memory::FrameLease::EMPTY;
        state.local_last = memory::FrameLease::EMPTY;
        state.local_head = 0;
        state.local_length = 0;
        state.read_shutdown = true;
    }
    if matches!(how, 1 | 2) {
        state.write_shutdown = true;
        if state.kind == Kind::Tcp && state.peer.is_none() {
            let runtime = runtime_existing().ok_or(Error::Unavailable)?;
            let handle = state.handle.ok_or(Error::Invalid)?;
            runtime.sockets.get_mut::<tcp::Socket>(handle).close();
        }
    }
    Ok(())
}

pub(crate) fn shutdown() -> bool {
    if unsafe { SLOTS.iter().any(|slot| slot.occupied) } {
        return false;
    }
    unsafe { UNIX_INFLIGHT_RIGHTS = 0 };
    if unsafe { RUNTIME.is_none() } {
        return true;
    }
    teardown()
}

pub(crate) fn duplicate(slot: u8) -> bool {
    unsafe { SLOTS.get_mut(usize::from(slot)) }
        .filter(|state| state.occupied)
        .and_then(|state| {
            state.references = state.references.checked_add(1)?;
            Some(())
        })
        .is_some()
}

fn port_in_use(kind: Kind, port: u16, except: usize) -> bool {
    unsafe {
        SLOTS.iter().enumerate().any(|(index, slot)| {
            index != except
                && slot.occupied
                && slot.kind == kind
                && slot.local_port == port
                && port != 0
        })
    }
}

fn allocate_ephemeral_port(runtime: &mut Runtime, kind: Kind, except: usize) -> Option<u16> {
    for _ in 0..(u16::MAX - 49_152) {
        let port = runtime.next_port.max(49_152);
        runtime.next_port = port.wrapping_add(1).max(49_152);
        if !port_in_use(kind, port, except) {
            return Some(port);
        }
    }
    None
}

pub(crate) fn kind(slot: u8) -> Option<Kind> {
    unsafe { SLOTS.get(usize::from(slot)) }
        .filter(|state| state.occupied)
        .map(|state| state.kind)
}

pub(crate) fn is_listening(slot: u8) -> bool {
    unsafe { SLOTS.get(usize::from(slot)) }.is_some_and(|state| state.occupied && state.listening)
}

pub(crate) fn set_boolean_option(
    slot: u8,
    option: BooleanOption,
    enabled: bool,
) -> Result<(), Error> {
    let state = unsafe { SLOTS.get_mut(usize::from(slot)) }
        .filter(|state| state.occupied)
        .ok_or(Error::Invalid)?;
    match option {
        BooleanOption::ReuseAddress => state.reuse_address = enabled,
        BooleanOption::KeepAlive => state.keep_alive = enabled,
        BooleanOption::ReceiveErrors => state.receive_errors = enabled,
        BooleanOption::NoDelay => state.no_delay = enabled,
        BooleanOption::PassCredentials => state.pass_credentials = enabled,
    }
    if state.kind == Kind::Tcp {
        if let Some(handle) = state.handle {
            let runtime = runtime_existing().ok_or(Error::Unavailable)?;
            let socket = runtime.sockets.get_mut::<tcp::Socket>(handle);
            match option {
                BooleanOption::KeepAlive => {
                    socket.set_keep_alive(enabled.then_some(Duration::from_secs(75)))
                }
                BooleanOption::NoDelay => socket.set_nagle_enabled(!enabled),
                BooleanOption::ReuseAddress
                | BooleanOption::ReceiveErrors
                | BooleanOption::PassCredentials => {}
            }
        }
    }
    Ok(())
}

pub(crate) fn boolean_option(slot: u8, option: BooleanOption) -> Result<bool, Error> {
    let state = unsafe { SLOTS.get(usize::from(slot)) }
        .filter(|state| state.occupied)
        .ok_or(Error::Invalid)?;
    Ok(match option {
        BooleanOption::ReuseAddress => state.reuse_address,
        BooleanOption::KeepAlive => state.keep_alive,
        BooleanOption::ReceiveErrors => state.receive_errors,
        BooleanOption::NoDelay => state.no_delay,
        BooleanOption::PassCredentials => state.pass_credentials,
    })
}

pub(crate) fn set_timeout_option(
    slot: u8,
    option: TimeoutOption,
    microseconds: u64,
) -> Result<(), Error> {
    let state = unsafe { SLOTS.get_mut(usize::from(slot)) }
        .filter(|state| state.occupied)
        .ok_or(Error::Invalid)?;
    match option {
        TimeoutOption::Receive => state.receive_timeout_us = microseconds,
        TimeoutOption::Send => state.send_timeout_us = microseconds,
    }
    Ok(())
}

pub(crate) fn timeout_option(slot: u8, option: TimeoutOption) -> Result<u64, Error> {
    let state = unsafe { SLOTS.get(usize::from(slot)) }
        .filter(|state| state.occupied)
        .ok_or(Error::Invalid)?;
    Ok(match option {
        TimeoutOption::Receive => state.receive_timeout_us,
        TimeoutOption::Send => state.send_timeout_us,
    })
}

fn timeout_poll_limit(slot: u8, option: TimeoutOption) -> Option<usize> {
    timeout_option(slot, option)
        .ok()
        .filter(|microseconds| *microseconds != 0)
        .map(|microseconds| {
            usize::try_from(microseconds.div_ceil(1_000))
                .unwrap_or(usize::MAX)
                .max(1)
        })
}

pub(crate) fn bind(slot: u8, address: u32, requested_port: u16) -> Result<u16, Error> {
    poll();
    let runtime = runtime()?;
    let index = usize::from(slot);
    let state = unsafe { SLOTS.get(index).copied() }
        .filter(|state| state.occupied)
        .ok_or(Error::Invalid)?;
    if state.kind == Kind::Unix || state.local_port != 0 || state.remote.is_some() {
        return Err(Error::Invalid);
    }
    if address != 0 && address != runtime.local_address {
        return Err(Error::AddressNotAvailable);
    }
    let port = if requested_port == 0 {
        allocate_ephemeral_port(runtime, state.kind, index).ok_or(Error::Exhausted)?
    } else {
        requested_port
    };
    if port_in_use(state.kind, port, index) {
        return Err(Error::AddressInUse);
    }
    if state.kind == Kind::Udp {
        let handle = state.handle.ok_or(Error::Invalid)?;
        runtime
            .sockets
            .get_mut::<udp::Socket>(handle)
            .bind(port)
            .map_err(|_| Error::AddressInUse)?;
    }
    unsafe { SLOTS[index].local_port = port };
    Ok(port)
}

pub(crate) fn listen(slot: u8, requested_backlog: usize) -> Result<(), Error> {
    poll();
    let runtime = runtime()?;
    let index = usize::from(slot);
    let state = unsafe { SLOTS.get(index).copied() }
        .filter(|state| state.occupied && state.kind == Kind::Tcp)
        .ok_or(Error::Invalid)?;
    if state.remote.is_some() {
        return Err(Error::Invalid);
    }
    let local_port = if state.local_port == 0 {
        allocate_ephemeral_port(runtime, Kind::Tcp, index).ok_or(Error::Exhausted)?
    } else {
        state.local_port
    };
    let handle = state.handle.ok_or(Error::Invalid)?;
    runtime
        .sockets
        .get_mut::<tcp::Socket>(handle)
        .listen(local_port)
        .map_err(|_| Error::Invalid)?;
    unsafe {
        SLOTS[index].local_port = local_port;
        SLOTS[index].listening = true;
        SLOTS[index].backlog = requested_backlog.clamp(1, SLOT_COUNT) as u8;
    }
    Ok(())
}

fn add_tcp_socket(runtime: &mut Runtime, buffer: usize) -> SocketHandle {
    let rx = tcp::SocketBuffer::new(unsafe {
        core::slice::from_raw_parts_mut(TCP_RX[buffer].as_mut_ptr(), TCP_BUFFER_SIZE)
    });
    let tx = tcp::SocketBuffer::new(unsafe {
        core::slice::from_raw_parts_mut(TCP_TX[buffer].as_mut_ptr(), TCP_BUFFER_SIZE)
    });
    runtime.sockets.add(tcp::Socket::new(rx, tx))
}

pub(crate) fn accept(slot: u8) -> Result<(u8, IpEndpoint), Error> {
    poll();
    let runtime = runtime()?;
    let listener_index = usize::from(slot);
    let listener = unsafe { SLOTS.get(listener_index).copied() }
        .filter(|state| state.occupied && state.kind == Kind::Tcp && state.listening)
        .ok_or(Error::Invalid)?;
    if listener.pending_length != 0 {
        let client_slot = listener.pending[0];
        let client_index = usize::from(client_slot);
        let client = unsafe { SLOTS.get(client_index).copied() }
            .filter(|state| state.occupied && state.kind == Kind::Tcp && !state.listening)
            .ok_or(Error::Invalid)?;
        let accepted_index =
            unsafe { SLOTS.iter().position(|state| !state.occupied) }.ok_or(Error::Exhausted)?;
        let remote = endpoint(runtime.local_address, client.local_port);
        unsafe {
            SLOTS[accepted_index] = Slot {
                occupied: true,
                references: 1,
                kind: Kind::Tcp,
                handle: None,
                remote: Some(remote),
                local_port: listener.local_port,
                tcp_buffer: None,
                listening: false,
                backlog: 0,
                pending_length: 0,
                pending: [0; SLOT_COUNT],
                peer: Some(client_slot),
                read_shutdown: false,
                write_shutdown: false,
                reuse_address: listener.reuse_address,
                keep_alive: listener.keep_alive,
                receive_errors: listener.receive_errors,
                no_delay: listener.no_delay,
                pass_credentials: listener.pass_credentials,
                receive_timeout_us: listener.receive_timeout_us,
                send_timeout_us: listener.send_timeout_us,
                pending_right_count: 0,
                pending_right_slots: [0; SLOT_COUNT],
                pending_right_flags: [0; SLOT_COUNT],
                pending_credentials: None,
                peer_credentials: None,
                local_first: memory::FrameLease::EMPTY,
                local_last: memory::FrameLease::EMPTY,
                local_head: 0,
                local_length: 0,
            };
            SLOTS[client_index].peer = Some(accepted_index as u8);
            SLOTS[listener_index]
                .pending
                .copy_within(1..usize::from(listener.pending_length), 0);
            SLOTS[listener_index].pending_length -= 1;
        }
        return Ok((accepted_index as u8, remote));
    }
    let handle = listener.handle.ok_or(Error::Invalid)?;
    let socket = runtime.sockets.get::<tcp::Socket>(handle);
    if !matches!(
        socket.state(),
        tcp::State::Established | tcp::State::CloseWait
    ) {
        return Err(Error::WouldBlock);
    }
    let remote = socket.remote_endpoint().ok_or(Error::Invalid)?;
    let accepted_index =
        unsafe { SLOTS.iter().position(|state| !state.occupied) }.ok_or(Error::Exhausted)?;
    let replacement_buffer = free_tcp_buffer().ok_or(Error::Exhausted)?;
    let replacement = add_tcp_socket(runtime, replacement_buffer);
    if runtime
        .sockets
        .get_mut::<tcp::Socket>(replacement)
        .listen(listener.local_port)
        .is_err()
    {
        runtime.sockets.remove(replacement);
        return Err(Error::Invalid);
    }
    {
        let accepted_socket = runtime.sockets.get_mut::<tcp::Socket>(handle);
        accepted_socket.set_nagle_enabled(!listener.no_delay);
        accepted_socket.set_keep_alive(listener.keep_alive.then_some(Duration::from_secs(75)));
    }
    unsafe {
        SLOTS[accepted_index] = Slot {
            occupied: true,
            references: 1,
            kind: Kind::Tcp,
            handle: Some(handle),
            remote: Some(remote),
            local_port: listener.local_port,
            tcp_buffer: listener.tcp_buffer,
            listening: false,
            backlog: 0,
            pending_length: 0,
            pending: [0; SLOT_COUNT],
            peer: None,
            read_shutdown: false,
            write_shutdown: false,
            reuse_address: listener.reuse_address,
            keep_alive: listener.keep_alive,
            receive_errors: listener.receive_errors,
            no_delay: listener.no_delay,
            pass_credentials: listener.pass_credentials,
            receive_timeout_us: listener.receive_timeout_us,
            send_timeout_us: listener.send_timeout_us,
            pending_right_count: 0,
            pending_right_slots: [0; SLOT_COUNT],
            pending_right_flags: [0; SLOT_COUNT],
            pending_credentials: None,
            peer_credentials: None,
            local_first: memory::FrameLease::EMPTY,
            local_last: memory::FrameLease::EMPTY,
            local_head: 0,
            local_length: 0,
        };
        SLOTS[listener_index].handle = Some(replacement);
        SLOTS[listener_index].remote = None;
        SLOTS[listener_index].tcp_buffer = Some(replacement_buffer as u8);
        SLOTS[listener_index].listening = true;
    }
    Ok((accepted_index as u8, remote))
}

pub(crate) fn accept_blocking(slot: u8) -> Result<(u8, IpEndpoint), Error> {
    if let Some(limit) = timeout_poll_limit(slot, TimeoutOption::Receive) {
        for _ in 0..limit {
            match accept(slot) {
                Err(Error::WouldBlock | Error::InProgress) => core::hint::spin_loop(),
                result => return result,
            }
        }
        return Err(Error::WouldBlock);
    }
    loop {
        match accept(slot) {
            Err(Error::WouldBlock | Error::InProgress) => core::hint::spin_loop(),
            result => return result,
        }
    }
}

fn teardown() -> bool {
    let Some(platform) = (unsafe { PLATFORM }) else {
        return false;
    };
    let Some(runtime) = (unsafe { RUNTIME.take() }) else {
        return false;
    };
    pci::quiesce_isolated_network(platform.device, platform.common)
        && runtime._translation.finish()
        && memory::scrub_and_release_frames(memory::FrameOwner::KERNEL, &[runtime._dma])
}

pub(crate) fn connect(slot: u8, remote: IpEndpoint) -> Result<(), Error> {
    poll();
    let runtime = runtime()?;
    let index = usize::from(slot);
    let state = unsafe { SLOTS.get(index).copied() }
        .filter(|slot| slot.occupied)
        .ok_or(Error::Invalid)?;
    let handle = state.handle.ok_or(Error::Invalid)?;
    if state.listening {
        return Err(Error::Invalid);
    }
    let local_port = if state.local_port == 0 {
        allocate_ephemeral_port(runtime, state.kind, index).ok_or(Error::Exhausted)?
    } else {
        state.local_port
    };
    if state.kind == Kind::Tcp
        && remote.addr
            == IpAddress::Ipv4(Ipv4Address::from_octets(
                runtime.local_address.to_be_bytes(),
            ))
    {
        let listener = unsafe {
            SLOTS.iter().position(|candidate| {
                candidate.occupied
                    && candidate.kind == Kind::Tcp
                    && candidate.listening
                    && candidate.local_port == remote.port
                    && candidate.pending_length < candidate.backlog
            })
        };
        if let Some(listener) = listener {
            unsafe {
                SLOTS[index].local_port = local_port;
                SLOTS[index].remote = Some(remote);
                SLOTS[index].peer = Some(listener as u8);
                let pending = usize::from(SLOTS[listener].pending_length);
                SLOTS[listener].pending[pending] = slot;
                SLOTS[listener].pending_length += 1;
            }
            return Ok(());
        }
    }
    match state.kind {
        Kind::Tcp => {
            let socket = runtime.sockets.get_mut::<tcp::Socket>(handle);
            if socket.is_active() {
                return if socket.may_send() {
                    Ok(())
                } else {
                    Err(Error::InProgress)
                };
            }
            socket
                .connect(runtime.interface.context(), remote, local_port)
                .map_err(|_| Error::Invalid)?;
            unsafe {
                SLOTS[index].local_port = local_port;
                SLOTS[index].remote = Some(remote);
            }
            Err(Error::InProgress)
        }
        Kind::Udp => {
            let socket = runtime.sockets.get_mut::<udp::Socket>(handle);
            if !socket.is_open() {
                socket.bind(local_port).map_err(|_| Error::Invalid)?;
            }
            unsafe {
                SLOTS[index].local_port = local_port;
                SLOTS[index].remote = Some(remote);
            }
            Ok(())
        }
        Kind::Unix => Err(Error::Invalid),
    }
}

pub(crate) fn send(slot: u8, input: &[u8], remote: Option<IpEndpoint>) -> Result<usize, Error> {
    poll();
    let state = unsafe { SLOTS.get(usize::from(slot)) }
        .filter(|slot| slot.occupied)
        .ok_or(Error::Invalid)?;
    if state.write_shutdown {
        return Err(Error::BrokenPipe);
    }
    if state.kind == Kind::Unix || state.peer.is_some() && !state.listening {
        if remote.is_some() {
            return Err(Error::Invalid);
        }
        let peer = usize::from(state.peer.ok_or(Error::NotConnected)?);
        let Some(peer_state) = (unsafe { SLOTS.get_mut(peer) })
            .filter(|slot| slot.occupied && slot.kind == state.kind && !slot.listening)
        else {
            crate::serial_write(
                "ginux: local socket peer unavailable slot/peer/occupied/references ",
            );
            crate::serial_write_u64(u64::from(slot));
            crate::serial_write(" ");
            crate::serial_write_u64(peer as u64);
            crate::serial_write(" ");
            let observed = unsafe { SLOTS.get(peer).copied().unwrap_or(Slot::EMPTY) };
            crate::serial_write_u64(u64::from(observed.occupied));
            crate::serial_write(" ");
            crate::serial_write_u64(u64::from(observed.references));
            crate::serial_write("\r\n");
            return Err(Error::NotConnected);
        };
        if input.is_empty() {
            return Ok(0);
        }
        let new_length = peer_state
            .local_length
            .checked_add(input.len())
            .ok_or(Error::Exhausted)?;
        let (first, last) = allocate_local_pages(input).ok_or(Error::Exhausted)?;
        if peer_state.local_last == memory::FrameLease::EMPTY {
            peer_state.local_first = first;
        } else if let Some(tail) = local_page_mut(peer_state.local_last) {
            tail.next = first;
        } else {
            let _ = release_local_pages(first);
            return Err(Error::Invalid);
        }
        peer_state.local_last = last;
        peer_state.local_length = new_length;
        return Ok(input.len());
    }
    let runtime = runtime()?;
    let handle = state.handle.ok_or(Error::Invalid)?;
    match state.kind {
        Kind::Tcp => {
            let socket = runtime.sockets.get_mut::<tcp::Socket>(handle);
            if socket.can_send() {
                socket.send_slice(input).map_err(|_| Error::Reset)
            } else if socket.may_send() {
                Err(Error::WouldBlock)
            } else {
                Err(Error::NotConnected)
            }
        }
        Kind::Udp => {
            let endpoint = remote.or(state.remote).ok_or(Error::NotConnected)?;
            runtime
                .sockets
                .get_mut::<udp::Socket>(handle)
                .send_slice(input, endpoint)
                .map(|_| input.len())
                .map_err(|_| Error::WouldBlock)
        }
        Kind::Unix => unreachable!(),
    }
}

pub(crate) fn send_blocking(
    slot: u8,
    input: &[u8],
    remote: Option<IpEndpoint>,
) -> Result<usize, Error> {
    let limit = timeout_poll_limit(slot, TimeoutOption::Send).unwrap_or(BLOCKING_POLL_LIMIT);
    for _ in 0..limit {
        match send(slot, input, remote) {
            Err(Error::WouldBlock | Error::InProgress) => core::hint::spin_loop(),
            result => return result,
        }
    }
    Err(Error::WouldBlock)
}

pub(crate) fn recv(slot: u8, output: &mut [u8]) -> Result<(usize, Option<IpEndpoint>), Error> {
    if kind(slot) == Some(Kind::Udp) {
        return recv_message(slot, output, false)
            .map(|(copied, _message_length, endpoint)| (copied, endpoint));
    }
    poll();
    let state = unsafe { SLOTS.get_mut(usize::from(slot)) }
        .filter(|slot| slot.occupied)
        .ok_or(Error::Invalid)?;
    if state.read_shutdown {
        return Ok((0, None));
    }
    if state.kind == Kind::Unix || state.peer.is_some() && !state.listening {
        let received = output.len().min(state.local_length);
        let mut copied = 0_usize;
        while copied < received {
            let page = state.local_first;
            let data = local_page(page).ok_or(Error::Invalid)?;
            let used = usize::try_from(data.used)
                .ok()
                .filter(|used| *used <= LOCAL_DATA_CAPACITY)
                .ok_or(Error::Invalid)?;
            if state.local_head >= used {
                return Err(Error::Invalid);
            }
            let amount = (received - copied).min(used - state.local_head);
            output[copied..copied + amount]
                .copy_from_slice(&data.bytes[state.local_head..state.local_head + amount]);
            copied += amount;
            state.local_head += amount;
            if state.local_head == used {
                let next = data.next;
                if !memory::scrub_and_release_frames(
                    memory::FrameOwner::KERNEL,
                    core::slice::from_ref(&page),
                ) {
                    return Err(Error::Invalid);
                }
                state.local_first = next;
                state.local_head = 0;
                if next == memory::FrameLease::EMPTY {
                    state.local_last = memory::FrameLease::EMPTY;
                }
            }
        }
        state.local_length -= received;
        if received != 0 {
            return Ok((received, None));
        }
        let peer = state
            .peer
            .and_then(|peer| unsafe { SLOTS.get(usize::from(peer)) });
        let peer_open = peer.is_some_and(|peer| peer.occupied);
        return if peer_open && !peer.is_some_and(|peer| peer.write_shutdown) {
            Err(Error::WouldBlock)
        } else {
            Ok((0, None))
        };
    }
    let runtime = runtime()?;
    let handle = state.handle.ok_or(Error::Invalid)?;
    match state.kind {
        Kind::Tcp => {
            let socket = runtime.sockets.get_mut::<tcp::Socket>(handle);
            if socket.can_recv() {
                socket
                    .recv_slice(output)
                    .map(|size| (size, None))
                    .map_err(|_| Error::Reset)
            } else if socket.may_recv() {
                Err(Error::WouldBlock)
            } else {
                Ok((0, None))
            }
        }
        Kind::Udp => runtime
            .sockets
            .get_mut::<udp::Socket>(handle)
            .recv_slice(output)
            .map(|(size, metadata)| (size, Some(metadata.endpoint)))
            .map_err(|_| Error::WouldBlock),
        Kind::Unix => unreachable!(),
    }
}

pub(crate) fn peek_local(slot: u8, output: &mut [u8]) -> Result<usize, Error> {
    let state = unsafe { SLOTS.get(usize::from(slot)) }
        .filter(|slot| {
            slot.occupied && (slot.kind == Kind::Unix || slot.peer.is_some() && !slot.listening)
        })
        .ok_or(Error::Invalid)?;
    let returned = output.len().min(state.local_length);
    let mut page = state.local_first;
    let mut head = state.local_head;
    let mut copied = 0_usize;
    while copied < returned {
        let data = local_page(page).ok_or(Error::Invalid)?;
        let used = usize::try_from(data.used)
            .ok()
            .filter(|used| *used <= LOCAL_DATA_CAPACITY)
            .ok_or(Error::Invalid)?;
        if head >= used {
            return Err(Error::Invalid);
        }
        let amount = (returned - copied).min(used - head);
        output[copied..copied + amount].copy_from_slice(&data.bytes[head..head + amount]);
        copied += amount;
        head += amount;
        if head == used {
            page = data.next;
            head = 0;
        }
    }
    if returned != 0 {
        return Ok(returned);
    }
    let peer = state
        .peer
        .and_then(|peer| unsafe { SLOTS.get(usize::from(peer)) });
    let peer_open = peer.is_some_and(|peer| peer.occupied);
    if peer_open && !peer.is_some_and(|peer| peer.write_shutdown) {
        Err(Error::WouldBlock)
    } else {
        Ok(0)
    }
}

pub(crate) fn peek(slot: u8, output: &mut [u8]) -> Result<(usize, Option<IpEndpoint>), Error> {
    if kind(slot) == Some(Kind::Udp) {
        return recv_message(slot, output, true)
            .map(|(copied, _message_length, endpoint)| (copied, endpoint));
    }
    poll();
    let state = unsafe { SLOTS.get(usize::from(slot)) }
        .filter(|slot| slot.occupied)
        .ok_or(Error::Invalid)?;
    if state.read_shutdown {
        return Ok((0, None));
    }
    if state.kind == Kind::Unix || state.peer.is_some() && !state.listening {
        return peek_local(slot, output).map(|received| (received, None));
    }
    let runtime = runtime()?;
    let handle = state.handle.ok_or(Error::Invalid)?;
    match state.kind {
        Kind::Tcp => {
            let socket = runtime.sockets.get_mut::<tcp::Socket>(handle);
            if socket.can_recv() {
                socket
                    .peek_slice(output)
                    .map(|size| (size, None))
                    .map_err(|_| Error::Reset)
            } else if socket.may_recv() {
                Err(Error::WouldBlock)
            } else {
                Ok((0, None))
            }
        }
        Kind::Udp => runtime
            .sockets
            .get_mut::<udp::Socket>(handle)
            .peek_slice(output)
            .map(|(size, metadata)| (size, Some(metadata.endpoint)))
            .map_err(|_| Error::WouldBlock),
        Kind::Unix => unreachable!(),
    }
}

pub(crate) fn recv_message(
    slot: u8,
    output: &mut [u8],
    peek_only: bool,
) -> Result<(usize, usize, Option<IpEndpoint>), Error> {
    if kind(slot) != Some(Kind::Udp) {
        let (copied, endpoint) = if peek_only {
            peek(slot, output)?
        } else {
            recv(slot, output)?
        };
        return Ok((copied, copied, endpoint));
    }
    poll();
    let state = unsafe { SLOTS.get(usize::from(slot)) }
        .filter(|slot| slot.occupied && slot.kind == Kind::Udp)
        .ok_or(Error::Invalid)?;
    if state.read_shutdown {
        return Ok((0, 0, None));
    }
    let runtime = runtime()?;
    let handle = state.handle.ok_or(Error::Invalid)?;
    let socket = runtime.sockets.get_mut::<udp::Socket>(handle);
    let (payload, endpoint) = if peek_only {
        let (payload, metadata) = socket.peek().map_err(|_| Error::WouldBlock)?;
        (payload, metadata.endpoint)
    } else {
        let (payload, metadata) = socket.recv().map_err(|_| Error::WouldBlock)?;
        (payload, metadata.endpoint)
    };
    let copied = output.len().min(payload.len());
    output[..copied].copy_from_slice(&payload[..copied]);
    Ok((copied, payload.len(), Some(endpoint)))
}

pub(crate) fn recv_message_blocking(
    slot: u8,
    output: &mut [u8],
    peek_only: bool,
    wait_all: bool,
) -> Result<(usize, usize, Option<IpEndpoint>), Error> {
    if output.is_empty() {
        return Ok((0, 0, None));
    }
    if !wait_all || kind(slot) == Some(Kind::Udp) {
        let limit = timeout_poll_limit(slot, TimeoutOption::Receive).unwrap_or(BLOCKING_POLL_LIMIT);
        for _ in 0..limit {
            match recv_message(slot, output, peek_only) {
                Err(Error::WouldBlock | Error::InProgress) => core::hint::spin_loop(),
                result => return result,
            }
        }
        report_timeout(if peek_only { "peek" } else { "receive" });
        return Err(Error::WouldBlock);
    }
    if peek_only {
        let mut observed = 0;
        let limit = timeout_poll_limit(slot, TimeoutOption::Receive).unwrap_or(BLOCKING_POLL_LIMIT);
        for _ in 0..limit {
            match recv_message(slot, output, true) {
                Ok((copied, _, endpoint)) if copied == output.len() || copied == 0 => {
                    return Ok((copied, copied, endpoint));
                }
                Ok((copied, _, _)) => observed = copied,
                Err(Error::WouldBlock | Error::InProgress) => core::hint::spin_loop(),
                Err(error) => {
                    return if observed == 0 {
                        Err(error)
                    } else {
                        Ok((observed, observed, None))
                    }
                }
            }
        }
        return if observed == 0 {
            report_timeout("peek");
            Err(Error::WouldBlock)
        } else {
            Ok((observed, observed, None))
        };
    }
    let mut received = 0;
    let limit = timeout_poll_limit(slot, TimeoutOption::Receive).unwrap_or(BLOCKING_POLL_LIMIT);
    for _ in 0..limit {
        match recv_message(slot, &mut output[received..], false) {
            Ok((0, _, endpoint)) => return Ok((received, received, endpoint)),
            Ok((copied, _, endpoint)) => {
                received += copied;
                if received == output.len() {
                    return Ok((received, received, endpoint));
                }
            }
            Err(Error::WouldBlock | Error::InProgress) => core::hint::spin_loop(),
            Err(error) => {
                return if received == 0 {
                    Err(error)
                } else {
                    Ok((received, received, None))
                };
            }
        }
    }
    if received == 0 {
        report_timeout("receive");
        Err(Error::WouldBlock)
    } else {
        Ok((received, received, None))
    }
}

pub(crate) fn recv_blocking(
    slot: u8,
    output: &mut [u8],
) -> Result<(usize, Option<IpEndpoint>), Error> {
    let limit = timeout_poll_limit(slot, TimeoutOption::Receive).unwrap_or(BLOCKING_POLL_LIMIT);
    for _ in 0..limit {
        match recv(slot, output) {
            Err(Error::WouldBlock | Error::InProgress) => core::hint::spin_loop(),
            result => return result,
        }
    }
    report_timeout("receive");
    Err(Error::WouldBlock)
}

pub(crate) fn complete_connect(slot: u8) -> Result<(), Error> {
    let limit = timeout_poll_limit(slot, TimeoutOption::Send).unwrap_or(BLOCKING_POLL_LIMIT);
    for _ in 0..limit {
        let Some((_, writable, error)) = readiness(slot) else {
            return Err(Error::Invalid);
        };
        if writable {
            return Ok(());
        }
        if error {
            return Err(Error::Reset);
        }
        core::hint::spin_loop();
    }
    report_timeout("connect");
    Err(Error::WouldBlock)
}

fn report_timeout(operation: &str) {
    let Some(runtime) = runtime_existing() else {
        return;
    };
    crate::serial_write("ginux: socket ");
    crate::serial_write(operation);
    crate::serial_write(" timeout rx/tx available/used: ");
    crate::serial_write_u64(u64::from(runtime.device.rx_avail));
    crate::serial_write("/");
    crate::serial_write_u64(u64::from(unsafe {
        read_u16(runtime.device.dma + RX_USED_OFFSET + 2)
    }));
    crate::serial_write(" ");
    crate::serial_write_u64(u64::from(runtime.device.tx_avail));
    crate::serial_write("/");
    crate::serial_write_u64(u64::from(unsafe {
        read_u16(runtime.device.dma + TX_USED_OFFSET + 2)
    }));
    crate::serial_write("\r\n");
}

pub(crate) fn readiness(slot: u8) -> Option<(bool, bool, bool)> {
    poll();
    let state = unsafe { SLOTS.get(usize::from(slot)) }?
        .occupied
        .then(|| unsafe { SLOTS[usize::from(slot)] })?;
    if state.listening && state.pending_length != 0 {
        return Some((true, false, false));
    }
    if state.kind == Kind::Unix || state.peer.is_some() {
        let peer = state
            .peer
            .and_then(|peer| unsafe { SLOTS.get(usize::from(peer)) });
        let peer_open = peer.is_some_and(|peer| peer.occupied);
        let writable = !state.write_shutdown && peer.is_some_and(|peer| peer.occupied);
        let readable = state.read_shutdown
            || state.local_length != 0
            || !peer_open
            || peer.is_some_and(|peer| peer.write_shutdown);
        return Some((readable, writable, false));
    }
    let runtime = runtime_existing()?;
    let handle = state.handle?;
    Some(match state.kind {
        Kind::Tcp => {
            let socket = runtime.sockets.get::<tcp::Socket>(handle);
            if state.listening {
                (
                    matches!(
                        socket.state(),
                        tcp::State::Established | tcp::State::CloseWait
                    ),
                    false,
                    false,
                )
            } else {
                (
                    state.read_shutdown || socket.can_recv() || !socket.may_recv(),
                    !state.write_shutdown && socket.can_send(),
                    !socket.is_open() && state.remote.is_some(),
                )
            }
        }
        Kind::Udp => {
            let socket = runtime.sockets.get::<udp::Socket>(handle);
            (socket.can_recv(), socket.can_send(), false)
        }
        Kind::Unix => unreachable!(),
    })
}

pub(crate) fn names(slot: u8) -> Option<(IpEndpoint, Option<IpEndpoint>)> {
    let state = unsafe { *SLOTS.get(usize::from(slot))? };
    if !state.occupied || state.kind == Kind::Unix {
        return None;
    }
    let runtime = runtime_existing()?;
    Some((
        endpoint(runtime.local_address, state.local_port),
        state.remote,
    ))
}

pub(crate) fn unix_peer_connected(slot: u8) -> bool {
    let Some(state) = (unsafe { SLOTS.get(usize::from(slot)).copied() })
        .filter(|state| state.occupied && state.kind == Kind::Unix)
    else {
        return false;
    };
    state.peer.is_some_and(|peer| {
        unsafe { SLOTS.get(usize::from(peer)).copied() }
            .is_some_and(|peer| peer.occupied && peer.kind == Kind::Unix)
    })
}

pub(crate) fn set_unix_pair_credentials(
    first: u8,
    second: u8,
    credentials: (u32, u32, u32),
) -> bool {
    let first_valid = unsafe { SLOTS.get(usize::from(first)) }.is_some_and(|state| {
        state.occupied && state.kind == Kind::Unix && state.peer == Some(second)
    });
    let second_valid = unsafe { SLOTS.get(usize::from(second)) }.is_some_and(|state| {
        state.occupied && state.kind == Kind::Unix && state.peer == Some(first)
    });
    if !first_valid || !second_valid {
        return false;
    }
    unsafe {
        SLOTS[usize::from(first)].peer_credentials = Some(credentials);
        SLOTS[usize::from(second)].peer_credentials = Some(credentials);
    }
    true
}

pub(crate) fn unix_peer_credentials(slot: u8) -> Option<(u32, u32, u32)> {
    unsafe { SLOTS.get(usize::from(slot)) }
        .filter(|state| state.occupied && state.kind == Kind::Unix)
        .and_then(|state| state.peer_credentials)
}

pub(crate) fn queue_unix_rights(sender: u8, rights: &[(u8, u32)]) -> Result<(), Error> {
    if rights.is_empty() || rights.len() > PASSED_RIGHT_LIMIT {
        return Err(Error::Invalid);
    }
    let sender_state = unsafe { SLOTS.get(usize::from(sender)).copied() }
        .filter(|state| state.occupied && state.kind == Kind::Unix)
        .ok_or(Error::Invalid)?;
    let peer = sender_state.peer.ok_or(Error::NotConnected)?;
    if rights.iter().any(|(right, _)| *right == peer) {
        return Err(Error::Invalid);
    }
    let peer_state = unsafe { SLOTS.get_mut(usize::from(peer)) }
        .filter(|state| state.occupied && state.kind == Kind::Unix)
        .ok_or(Error::NotConnected)?;
    if peer_state.pending_right_count != 0 {
        return Err(Error::WouldBlock);
    }
    let next_inflight = unsafe { UNIX_INFLIGHT_RIGHTS }
        .checked_add(rights.len())
        .ok_or(Error::Exhausted)?;
    for (index, (right, flags)) in rights.iter().copied().enumerate() {
        peer_state.pending_right_slots[index] = right;
        peer_state.pending_right_flags[index] = flags;
    }
    peer_state.pending_right_count = rights.len() as u8;
    unsafe { UNIX_INFLIGHT_RIGHTS = next_inflight };
    Ok(())
}

pub(crate) fn unix_inflight_rights() -> usize {
    unsafe { UNIX_INFLIGHT_RIGHTS }
}

pub(crate) fn unix_right_queue_available(sender: u8, rights: &[u8]) -> Result<(), Error> {
    if rights.is_empty() || rights.len() > PASSED_RIGHT_LIMIT {
        return Err(Error::Invalid);
    }
    let sender_state = unsafe { SLOTS.get(usize::from(sender)).copied() }
        .filter(|state| state.occupied && state.kind == Kind::Unix)
        .ok_or(Error::Invalid)?;
    let peer = sender_state.peer.ok_or(Error::NotConnected)?;
    if rights.contains(&peer) {
        return Err(Error::Invalid);
    }
    let peer_state = unsafe { SLOTS.get(usize::from(peer)) }
        .filter(|state| state.occupied && state.kind == Kind::Unix)
        .ok_or(Error::NotConnected)?;
    if peer_state.pending_right_count != 0 || peer_state.local_length != 0 {
        return Err(Error::WouldBlock);
    }
    Ok(())
}

pub(crate) fn unix_credentials_queue_available(sender: u8) -> Result<(), Error> {
    let sender_state = unsafe { SLOTS.get(usize::from(sender)).copied() }
        .filter(|state| state.occupied && state.kind == Kind::Unix)
        .ok_or(Error::Invalid)?;
    let peer = sender_state.peer.ok_or(Error::NotConnected)?;
    let peer_state = unsafe { SLOTS.get(usize::from(peer)) }
        .filter(|state| state.occupied && state.kind == Kind::Unix)
        .ok_or(Error::NotConnected)?;
    if peer_state.pending_credentials.is_some() || peer_state.local_length != 0 {
        return Err(Error::WouldBlock);
    }
    Ok(())
}

pub(crate) fn receive_unix_rights(
    receiver: u8,
    output: &mut [(u8, u32)],
    peek_only: bool,
) -> Result<usize, Error> {
    let state = unsafe { SLOTS.get_mut(usize::from(receiver)) }
        .filter(|state| state.occupied && state.kind == Kind::Unix)
        .ok_or(Error::Invalid)?;
    let count = usize::from(state.pending_right_count);
    if output.len() < count {
        return Err(Error::Invalid);
    }
    if peek_only {
        for (duplicated, right) in state
            .pending_right_slots
            .into_iter()
            .take(count)
            .enumerate()
        {
            if !duplicate(right) {
                for rollback in state.pending_right_slots.into_iter().take(duplicated) {
                    let _ = close(rollback);
                }
                return Err(Error::Exhausted);
            }
        }
    } else {
        if count > unsafe { UNIX_INFLIGHT_RIGHTS } {
            return Err(Error::Invalid);
        }
        state.pending_right_count = 0;
        unsafe { UNIX_INFLIGHT_RIGHTS -= count };
    }
    for (index, entry) in output.iter_mut().enumerate().take(count) {
        *entry = (
            state.pending_right_slots[index],
            state.pending_right_flags[index],
        );
    }
    Ok(count)
}

pub(crate) fn unix_right_count(receiver: u8) -> usize {
    unsafe { SLOTS.get(usize::from(receiver)) }
        .filter(|state| state.occupied && state.kind == Kind::Unix)
        .map_or(0, |state| usize::from(state.pending_right_count))
}

pub(crate) fn unix_peer_passes_credentials(sender: u8) -> Result<bool, Error> {
    let sender_state = unsafe { SLOTS.get(usize::from(sender)).copied() }
        .filter(|state| state.occupied && state.kind == Kind::Unix)
        .ok_or(Error::Invalid)?;
    let peer = sender_state.peer.ok_or(Error::NotConnected)?;
    let peer_state = unsafe { SLOTS.get(usize::from(peer)) }
        .filter(|state| state.occupied && state.kind == Kind::Unix)
        .ok_or(Error::NotConnected)?;
    Ok(peer_state.pass_credentials)
}

pub(crate) fn queue_unix_credentials(
    sender: u8,
    credentials: (u32, u32, u32),
) -> Result<(), Error> {
    let sender_state = unsafe { SLOTS.get(usize::from(sender)).copied() }
        .filter(|state| state.occupied && state.kind == Kind::Unix)
        .ok_or(Error::Invalid)?;
    let peer = sender_state.peer.ok_or(Error::NotConnected)?;
    let peer_state = unsafe { SLOTS.get_mut(usize::from(peer)) }
        .filter(|state| state.occupied && state.kind == Kind::Unix)
        .ok_or(Error::NotConnected)?;
    if peer_state.pending_credentials.is_some() {
        return Err(Error::WouldBlock);
    }
    peer_state.pending_credentials = Some(credentials);
    Ok(())
}

pub(crate) fn receive_unix_credentials(
    receiver: u8,
    peek_only: bool,
) -> Result<Option<(u32, u32, u32)>, Error> {
    let state = unsafe { SLOTS.get_mut(usize::from(receiver)) }
        .filter(|state| state.occupied && state.kind == Kind::Unix)
        .ok_or(Error::Invalid)?;
    let credentials = state.pending_credentials;
    if !peek_only {
        state.pending_credentials = None;
    }
    Ok(credentials)
}

pub(crate) fn unix_credentials_pending(receiver: u8) -> bool {
    unsafe { SLOTS.get(usize::from(receiver)) }.is_some_and(|state| {
        state.occupied && state.kind == Kind::Unix && state.pending_credentials.is_some()
    })
}

pub(crate) fn discard_unix_ancillary(receiver: u8) -> Result<bool, Error> {
    let state = unsafe { SLOTS.get_mut(usize::from(receiver)) }
        .filter(|state| state.occupied && state.kind == Kind::Unix)
        .ok_or(Error::Invalid)?;
    state.pending_credentials = None;
    let count = usize::from(state.pending_right_count);
    if count > unsafe { UNIX_INFLIGHT_RIGHTS } {
        return Err(Error::Invalid);
    }
    state.pending_right_count = 0;
    unsafe { UNIX_INFLIGHT_RIGHTS -= count };
    let rights = state.pending_right_slots;
    let mut closed = count != 0;
    for right in rights.into_iter().take(count) {
        closed &= close(right);
    }
    Ok(closed)
}

pub(crate) fn discard_unix_peer_ancillary(sender: u8) -> Result<bool, Error> {
    let sender_state = unsafe { SLOTS.get(usize::from(sender)).copied() }
        .filter(|state| state.occupied && state.kind == Kind::Unix)
        .ok_or(Error::Invalid)?;
    discard_unix_ancillary(sender_state.peer.ok_or(Error::NotConnected)?)
}

pub(crate) fn discard_unix_rights(receiver: u8) -> Result<bool, Error> {
    let state = unsafe { SLOTS.get_mut(usize::from(receiver)) }
        .filter(|state| state.occupied && state.kind == Kind::Unix)
        .ok_or(Error::Invalid)?;
    let count = usize::from(state.pending_right_count);
    if count > unsafe { UNIX_INFLIGHT_RIGHTS } {
        return Err(Error::Invalid);
    }
    state.pending_right_count = 0;
    unsafe { UNIX_INFLIGHT_RIGHTS -= count };
    let rights = state.pending_right_slots;
    let mut closed = count != 0;
    for right in rights.into_iter().take(count) {
        closed &= close(right);
    }
    Ok(closed)
}

pub(crate) fn endpoint(address: u32, port: u16) -> IpEndpoint {
    IpEndpoint::new(
        IpAddress::Ipv4(Ipv4Address::from_octets(address.to_be_bytes())),
        port,
    )
}

fn poll() {
    if !network::link_is_up() {
        return;
    }
    let Some(runtime) = runtime_existing() else {
        return;
    };
    runtime.tick = runtime.tick.wrapping_add(1);
    let _ = runtime.interface.poll(
        Instant::from_millis(runtime.tick),
        &mut runtime.device,
        &mut runtime.sockets,
    );
    drain_shutdown_tcp_receivers(runtime);
}

fn drain_shutdown_tcp_receivers(runtime: &mut Runtime) {
    for state in unsafe { SLOTS.iter() }.filter(|state| {
        state.occupied
            && state.kind == Kind::Tcp
            && !state.listening
            && state.peer.is_none()
            && state.read_shutdown
    }) {
        let Some(handle) = state.handle else {
            continue;
        };
        let socket = runtime.sockets.get_mut::<tcp::Socket>(handle);
        while socket.can_recv() {
            if socket.recv(|bytes| (bytes.len(), ())).is_err() {
                break;
            }
        }
    }
}

fn runtime() -> Result<&'static mut Runtime, Error> {
    if let Some(runtime) = runtime_existing() {
        return Ok(runtime);
    }
    let platform = unsafe { PLATFORM }.ok_or(Error::Unavailable)?;
    let configuration = network::ipv4_configuration();
    let ipv4 = configuration as u32;
    let prefix = (configuration >> 32) as u8;
    let gateway = network::ipv4_gateway() as u32;
    if ipv4 == 0 || prefix > 32 || gateway == 0 {
        return Err(Error::Unavailable);
    }
    let receive = platform
        .notification
        .address(platform.initialization.receive_notification, 2)
        .ok_or(Error::Unavailable)?;
    let transmit = platform
        .notification
        .address(platform.initialization.transmit_notification, 2)
        .ok_or(Error::Unavailable)?;
    let [dma] =
        memory::allocate_frames::<1>(memory::FrameOwner::KERNEL).ok_or(Error::Unavailable)?;
    let Some(dma_virtual) = dma.direct_map_address() else {
        let _ = memory::scrub_and_release_frames(memory::FrameOwner::KERNEL, &[dma]);
        return Err(Error::Unavailable);
    };
    let Some(translation) = platform.iommu.activate(dma) else {
        let _ = memory::scrub_and_release_frames(memory::FrameOwner::KERNEL, &[dma]);
        return Err(Error::Unavailable);
    };
    if !translation.is_active()
        || !(unsafe {
            pci::activate_kernel_network_queues(
                platform.device,
                platform.common,
                dma.address(),
                dma_virtual,
            )
        })
    {
        let _ = translation.finish();
        let _ = memory::scrub_and_release_frames(memory::FrameOwner::KERNEL, &[dma]);
        return Err(Error::Unavailable);
    }
    let mut device = unsafe { VirtioNet::new(dma_virtual, dma.address(), receive, transmit) };
    let mut config = InterfaceConfig::new(EthernetAddress(platform.initialization.mac).into());
    config.random_seed = u64::from(ipv4) << 32 | u64::from(gateway);
    let mut interface = Interface::new(config, &mut device, Instant::from_millis(0));
    interface.update_ip_addrs(|addresses| {
        let _ = addresses.push(IpCidr::new(
            IpAddress::Ipv4(Ipv4Address::from_octets(ipv4.to_be_bytes())),
            prefix,
        ));
    });
    if interface
        .routes_mut()
        .add_default_ipv4_route(Ipv4Address::from_octets(gateway.to_be_bytes()))
        .is_err()
    {
        let _ = pci::quiesce_isolated_network(platform.device, platform.common);
        let _ = translation.finish();
        let _ = memory::scrub_and_release_frames(memory::FrameOwner::KERNEL, &[dma]);
        return Err(Error::Unavailable);
    }
    let sockets = SocketSet::new(unsafe {
        core::slice::from_raw_parts_mut(SOCKET_STORAGE.as_mut_ptr(), SLOT_COUNT)
    });
    unsafe {
        RUNTIME = Some(Runtime {
            interface,
            device,
            sockets,
            _dma: dma,
            _translation: translation,
            tick: 0,
            next_port: 49_152,
            local_address: ipv4,
        });
    }
    runtime_existing().ok_or(Error::Unavailable)
}

fn runtime_existing() -> Option<&'static mut Runtime> {
    unsafe { core::ptr::addr_of_mut!(RUNTIME).as_mut()?.as_mut() }
}

struct VirtioNet {
    dma: usize,
    dma_device: u64,
    rx_notify: usize,
    tx_notify: usize,
    rx_avail: u16,
    rx_used: u16,
    tx_avail: u16,
    tx_used: u16,
}

impl VirtioNet {
    unsafe fn new(dma: usize, dma_device: u64, rx_notify: usize, tx_notify: usize) -> Self {
        let mut device = Self {
            dma,
            dma_device,
            rx_notify,
            tx_notify,
            rx_avail: 0,
            rx_used: 0,
            tx_avail: 0,
            tx_used: 0,
        };
        device.post_receive();
        device
    }

    unsafe fn post_receive(&mut self) {
        write_u64(
            self.dma + RX_DESC_OFFSET,
            self.dma_device + RX_BUFFER_OFFSET as u64,
        );
        write_u32(self.dma + RX_DESC_OFFSET + 8, RX_DESCRIPTOR_LENGTH as u32);
        write_u16(self.dma + RX_DESC_OFFSET + 12, 2);
        write_u16(self.dma + RX_DESC_OFFSET + 14, 0);
        write_u16(
            self.dma + RX_AVAIL_OFFSET + 4 + usize::from(self.rx_avail % QUEUE_SIZE) * 2,
            0,
        );
        self.rx_avail = self.rx_avail.wrapping_add(1);
        compiler_fence(Ordering::SeqCst);
        write_u16(self.dma + RX_AVAIL_OFFSET + 2, self.rx_avail);
        write_u16(self.rx_notify, 0);
    }
}

struct VirtioRxToken(*mut VirtioNet, usize);
struct VirtioTxToken(*mut VirtioNet);

impl RxToken for VirtioRxToken {
    fn consume<R, F>(self, callback: F) -> R
    where
        F: FnOnce(&[u8]) -> R,
    {
        unsafe {
            let device = &mut *self.0;
            let bytes = core::slice::from_raw_parts(
                (device.dma + RX_BUFFER_OFFSET + VIRTIO_HEADER_LENGTH) as *const u8,
                self.1,
            );
            let result = callback(bytes);
            device.rx_used = device.rx_used.wrapping_add(1);
            device.post_receive();
            result
        }
    }

    fn meta(&self) -> PacketMeta {
        PacketMeta::default()
    }
}

impl TxToken for VirtioTxToken {
    fn consume<R, F>(self, length: usize, callback: F) -> R
    where
        F: FnOnce(&mut [u8]) -> R,
    {
        unsafe {
            let device = &mut *self.0;
            core::slice::from_raw_parts_mut(
                (device.dma + TX_BUFFER_OFFSET) as *mut u8,
                VIRTIO_HEADER_LENGTH,
            )
            .fill(0);
            let bytes = core::slice::from_raw_parts_mut(
                (device.dma + TX_BUFFER_OFFSET + VIRTIO_HEADER_LENGTH) as *mut u8,
                length,
            );
            let result = callback(bytes);
            write_u64(
                device.dma + TX_DESC_OFFSET,
                device.dma_device + TX_BUFFER_OFFSET as u64,
            );
            write_u32(
                device.dma + TX_DESC_OFFSET + 8,
                (length + VIRTIO_HEADER_LENGTH) as u32,
            );
            write_u16(device.dma + TX_DESC_OFFSET + 12, 0);
            write_u16(device.dma + TX_DESC_OFFSET + 14, 0);
            write_u16(
                device.dma + TX_AVAIL_OFFSET + 4 + usize::from(device.tx_avail % QUEUE_SIZE) * 2,
                0,
            );
            device.tx_avail = device.tx_avail.wrapping_add(1);
            compiler_fence(Ordering::SeqCst);
            write_u16(device.dma + TX_AVAIL_OFFSET + 2, device.tx_avail);
            write_u16(device.tx_notify, 1);
            result
        }
    }
}

impl Device for VirtioNet {
    type RxToken<'a>
        = VirtioRxToken
    where
        Self: 'a;
    type TxToken<'a>
        = VirtioTxToken
    where
        Self: 'a;

    fn receive(&mut self, _timestamp: Instant) -> Option<(Self::RxToken<'_>, Self::TxToken<'_>)> {
        let used = unsafe { read_u16(self.dma + RX_USED_OFFSET + 2) };
        if used == self.rx_used {
            return None;
        }
        let element = self.dma + RX_USED_OFFSET + 4 + usize::from(self.rx_used % QUEUE_SIZE) * 8;
        let length = unsafe { read_u32(element + 4) as usize }.checked_sub(VIRTIO_HEADER_LENGTH)?;
        let pointer = self as *mut Self;
        Some((VirtioRxToken(pointer, length), VirtioTxToken(pointer)))
    }

    fn transmit(&mut self, _timestamp: Instant) -> Option<Self::TxToken<'_>> {
        let used = unsafe { read_u16(self.dma + TX_USED_OFFSET + 2) };
        if used != self.tx_used {
            self.tx_used = used;
        }
        (self.tx_used == self.tx_avail).then_some(VirtioTxToken(self as *mut Self))
    }

    fn capabilities(&self) -> DeviceCapabilities {
        let mut capabilities = DeviceCapabilities::default();
        capabilities.medium = Medium::Ethernet;
        capabilities.max_transmission_unit = 1514;
        capabilities
    }
}

unsafe fn write_u16(address: usize, value: u16) {
    write_volatile(address as *mut u16, value);
}

unsafe fn write_u32(address: usize, value: u32) {
    write_volatile(address as *mut u32, value);
}

unsafe fn write_u64(address: usize, value: u64) {
    write_volatile(address as *mut u64, value);
}

unsafe fn read_u16(address: usize) -> u16 {
    read_volatile(address as *const u16)
}

unsafe fn read_u32(address: usize) -> u32 {
    read_volatile(address as *const u32)
}
