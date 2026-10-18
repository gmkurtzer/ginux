#![allow(static_mut_refs)]

use core::{
    ptr::{read_volatile, write_volatile},
    sync::atomic::{compiler_fence, Ordering},
};

use smoltcp::{
    iface::{Config as InterfaceConfig, Interface, SocketHandle, SocketSet, SocketStorage},
    phy::{Device, DeviceCapabilities, Medium, PacketMeta, RxToken, TxToken},
    socket::{tcp, udp},
    time::Instant,
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
struct Slot {
    occupied: bool,
    references: u16,
    kind: Kind,
    handle: Option<SocketHandle>,
    remote: Option<IpEndpoint>,
    local_port: u16,
    tcp_buffer: Option<u8>,
    listening: bool,
    peer: Option<u8>,
    read_shutdown: bool,
    write_shutdown: bool,
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
        peer: None,
        read_shutdown: false,
        write_shutdown: false,
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
            peer: None,
            read_shutdown: false,
            write_shutdown: false,
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
            peer: Some(second as u8),
            read_shutdown: false,
            write_shutdown: false,
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
            peer: Some(first as u8),
            read_shutdown: false,
            write_shutdown: false,
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
        *state = Slot::EMPTY;
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

pub(crate) fn listen(slot: u8) -> Result<(), Error> {
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
    if let Some(client_slot) = listener.peer {
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
                peer: Some(client_slot),
                read_shutdown: false,
                write_shutdown: false,
                local_first: memory::FrameLease::EMPTY,
                local_last: memory::FrameLease::EMPTY,
                local_head: 0,
                local_length: 0,
            };
            SLOTS[client_index].peer = Some(accepted_index as u8);
            SLOTS[listener_index].peer = None;
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
            peer: None,
            read_shutdown: false,
            write_shutdown: false,
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
                    && candidate.peer.is_none()
            })
        };
        if let Some(listener) = listener {
            unsafe {
                SLOTS[index].local_port = local_port;
                SLOTS[index].remote = Some(remote);
                SLOTS[index].peer = Some(listener as u8);
                SLOTS[listener].peer = Some(slot);
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
    for _ in 0..BLOCKING_POLL_LIMIT {
        match send(slot, input, remote) {
            Err(Error::WouldBlock | Error::InProgress) => core::hint::spin_loop(),
            result => return result,
        }
    }
    Err(Error::WouldBlock)
}

pub(crate) fn recv(slot: u8, output: &mut [u8]) -> Result<(usize, Option<IpEndpoint>), Error> {
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

pub(crate) fn recv_blocking(
    slot: u8,
    output: &mut [u8],
) -> Result<(usize, Option<IpEndpoint>), Error> {
    for _ in 0..BLOCKING_POLL_LIMIT {
        match recv(slot, output) {
            Err(Error::WouldBlock | Error::InProgress) => core::hint::spin_loop(),
            result => return result,
        }
    }
    report_timeout("receive");
    Err(Error::WouldBlock)
}

pub(crate) fn complete_connect(slot: u8) -> Result<(), Error> {
    for _ in 0..BLOCKING_POLL_LIMIT {
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
    if state.listening && state.peer.is_some() {
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
                    socket.can_recv() || !socket.may_recv(),
                    socket.can_send(),
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
