#![no_std]
#![no_main]

use core::{
    arch::{asm, global_asm},
    ptr::{read_volatile, write_volatile},
    sync::atomic::{compiler_fence, Ordering},
};

use ginux_native_abi::{
    native_control_abi_is_compatible, native_control_abi_supports_packet_handoff,
    NativeControlDescriptorV4, NativeNetworkBootV0, NativeNetworkBootV1, NativeNetworkBootV2,
    NativeNetworkBootV3, NativeNetworkBootV4, NativePacketRequestV0,
    NATIVE_CONTROL_DESCRIPTOR_OPERATION, NATIVE_CONTROL_DESCRIPTOR_SIZE,
    NATIVE_CONTROL_DISCOVERY_OPERATION, NATIVE_NETWORK_DEVICE_ACTIVATE_EMPTY_QUEUES,
    NATIVE_NETWORK_DEVICE_DESCRIPTOR_WORD, NATIVE_NETWORK_DEVICE_OPERATION,
    NATIVE_NETWORK_DEVICE_PROBE, NATIVE_NETWORK_WAIT_CANCELLED, NATIVE_NETWORK_WAIT_FAILED,
    NATIVE_NETWORK_WAIT_OPERATION, NATIVE_NETWORK_WAIT_RX_POSTED, NATIVE_NETWORK_WAIT_TX_NOTIFIED,
    NATIVE_PACKET_EGRESS_ACK_OPERATION, NATIVE_PACKET_EGRESS_CLAIM_OPERATION,
    NATIVE_PACKET_EGRESS_WAIT_OPERATION, NATIVE_PACKET_PUBLISH_OPERATION, NATIVE_PACKET_STALE,
    NATIVE_SERVICE_EXIT_COMPLETED, NATIVE_SERVICE_EXIT_OPERATION,
    NATIVE_SERVICE_EXIT_RESTART_REQUIRED,
};

static SERVICE_IDENTITY: &[u8] = b"GINUX_NETWORK_SERVICE_V0";

const DMA_PAGE_SIZE: usize = 4096;
const QUEUE_SIZE: u16 = 8;
const RX_DESC_OFFSET: u64 = 0x000;
const RX_AVAIL_OFFSET: u64 = 0x080;
const RX_USED_OFFSET: u64 = 0x100;
const TX_DESC_OFFSET: u64 = 0x180;
const TX_AVAIL_OFFSET: u64 = 0x200;
const TX_USED_OFFSET: u64 = 0x280;
const TX_BUFFER_OFFSET: u64 = 0xa00;
const RX_BUFFER_OFFSET: u64 = 0x400;
const RX_DESCRIPTOR_LENGTH: usize = 0x5ff;
const TX_HEADER_LENGTH: usize = 12;
const TX_FRAME: [u8; 19] = [
    0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x52, 0x54, 0x00, 0x12, 0x34, 0x56, 0x88, 0xb5, b'G', b'I',
    b'N', b'U', b'X',
];
const COMMON_DEVICE_FEATURE_SELECT: usize = 0x00;
const COMMON_DEVICE_FEATURE: usize = 0x04;
const COMMON_DRIVER_FEATURE_SELECT: usize = 0x08;
const COMMON_DRIVER_FEATURE: usize = 0x0c;
const COMMON_NUM_QUEUES: usize = 0x12;
const COMMON_DEVICE_STATUS: usize = 0x14;
const COMMON_QUEUE_SELECT: usize = 0x16;
const COMMON_QUEUE_SIZE: usize = 0x18;
const COMMON_QUEUE_MSIX_VECTOR: usize = 0x1a;
const COMMON_QUEUE_ENABLE: usize = 0x1c;
const COMMON_QUEUE_DESC: usize = 0x20;
const COMMON_QUEUE_DRIVER: usize = 0x28;
const COMMON_QUEUE_DEVICE: usize = 0x30;
const STATUS_ACKNOWLEDGE: u8 = 1;
const STATUS_DRIVER: u8 = 2;
const STATUS_DRIVER_OK: u8 = 4;
const STATUS_FEATURES_OK: u8 = 8;
const FEATURE_NET_MAC: u32 = 1 << 5;
const FEATURE_NET_MRG_RXBUF: u32 = 1 << 15;
const FEATURE_VERSION_1: u32 = 1;
const FEATURE_ACCESS_PLATFORM: u32 = 1 << 1;
const RESET_POLL_LIMIT: usize = 100_000;
const COMPLETION_POLL_LIMIT: usize = 10_000_000;
const TX_INTERRUPT_REQUEST: u64 = 1;
const ECHO_REPLY: [u8; 42] = [
    0x52, 0x55, 0x0a, 0x00, 0x02, 0x02, 0x52, 0x54, 0x00, 0x12, 0x34, 0x56, 0x08, 0x00, 0x45, 0x00,
    0x00, 0x1c, 0x12, 0x34, 0x40, 0x00, 0x40, 0x01, 0x10, 0x9d, 10, 0, 2, 15, 10, 0, 2, 2, 0x00,
    0x00, 0x41, 0x0f, 0xbe, 0xef, 0x00, 0x01,
];

global_asm!(
    r#"
    .text
    .global _start
_start:
    mov rdi, rbx
    call ginux_network_service_main
    int3
"#
);

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo<'_>) -> ! {
    fail()
}

#[no_mangle]
extern "C" fn ginux_network_service_main(info: *const NativeNetworkBootV0) -> ! {
    let identity = unsafe { core::ptr::read_volatile(&SERVICE_IDENTITY) };
    let discovery = discover_control_abi();
    if identity != b"GINUX_NETWORK_SERVICE_V0"
        || !native_control_abi_is_compatible(discovery)
        || !discover_control_descriptor().is_compatible()
    {
        fail();
    }
    if !info.is_null() {
        let Some(info) = (unsafe { info.as_ref() }) else {
            fail();
        };
        let (info, packet_capability, fault_after_packet, packet_batch, egress) =
            if info.version == 0 {
                if !info.is_valid_virtio_network() {
                    fail();
                }
                (*info, None, false, false, None)
            } else if info.version == 1 {
                let Some(v1) = (unsafe {
                    (info as *const NativeNetworkBootV0 as *const NativeNetworkBootV1).as_ref()
                }) else {
                    fail();
                };
                if !v1.is_valid_packet_handoff()
                    || !native_control_abi_supports_packet_handoff(discovery)
                {
                    fail();
                }
                (
                    v1.v0_prefix(),
                    Some(v1.packet_capability),
                    v1.requests_packet_fault(),
                    v1.requests_packet_batch(),
                    None,
                )
            } else if info.version == 2 {
                let Some(v2) = (unsafe {
                    (info as *const NativeNetworkBootV0 as *const NativeNetworkBootV2).as_ref()
                }) else {
                    fail();
                };
                if !v2.is_valid_egress_transmit()
                    || !native_control_abi_supports_packet_handoff(discovery)
                {
                    fail();
                }
                (
                    v2.v0_prefix(),
                    None,
                    false,
                    false,
                    Some((
                        v2.egress_capability,
                        v2.expected_egress_sequence,
                        false,
                        false,
                        false,
                    )),
                )
            } else if info.version == 3 {
                let Some(v3) = (unsafe {
                    (info as *const NativeNetworkBootV0 as *const NativeNetworkBootV3).as_ref()
                }) else {
                    fail();
                };
                if !v3.is_valid_reliable_egress_transmit()
                    || !native_control_abi_supports_packet_handoff(discovery)
                {
                    fail();
                }
                (
                    v3.v0_prefix(),
                    None,
                    false,
                    false,
                    Some((
                        v3.egress_capability,
                        v3.expected_egress_sequence,
                        true,
                        v3.requests_fault_after_claim(),
                        false,
                    )),
                )
            } else if info.version == 4 {
                let Some(v4) = (unsafe {
                    (info as *const NativeNetworkBootV0 as *const NativeNetworkBootV4).as_ref()
                }) else {
                    fail();
                };
                if !v4.is_valid_waiting_reliable_egress_transmit()
                    || !native_control_abi_supports_packet_handoff(discovery)
                {
                    fail();
                }
                (
                    v4.v0_prefix(),
                    None,
                    false,
                    false,
                    Some((
                        v4.egress_capability,
                        v4.expected_egress_sequence,
                        true,
                        false,
                        true,
                    )),
                )
            } else {
                fail();
            };
        if info.mac_address != 0x5254_0012_3456
            || probe_network_device(info.device_capability) != NATIVE_NETWORK_DEVICE_DESCRIPTOR_WORD
        {
            fail();
        }
        if info.requests_empty_queues()
            && (!unsafe { configure_empty_queues(&info) }
                || activate_empty_queues(info.device_capability) != 0)
        {
            fail();
        }
        if info.requests_transmit_probe() {
            let status =
                if let Some((capability, sequence, reliable, fault_after_claim, wait)) = egress {
                    unsafe {
                        transmit_egress(
                            &info,
                            capability,
                            sequence,
                            reliable,
                            fault_after_claim,
                            wait,
                        )
                    }
                } else if info.requests_arp_probe() {
                    unsafe { arp_probe(&info, packet_capability, fault_after_packet, packet_batch) }
                } else {
                    unsafe { transmit_probe(&info) }
                };
            let Some(status) = status else {
                fail();
            };
            if status != NATIVE_SERVICE_EXIT_COMPLETED {
                exit(status);
            }
        }
    }
    exit(NATIVE_SERVICE_EXIT_COMPLETED)
}

unsafe fn transmit_probe(info: &NativeNetworkBootV0) -> Option<u8> {
    let Ok(dma) = usize::try_from(info.dma_virtual_address) else {
        return None;
    };
    let Ok(notification) = usize::try_from(info.transmit_notification_address) else {
        return None;
    };
    let Ok(common) = usize::try_from(info.common_config_address) else {
        return None;
    };
    let payload_length = TX_HEADER_LENGTH + TX_FRAME.len();
    write_u64(
        dma + TX_DESC_OFFSET as usize,
        info.dma_device_address + TX_BUFFER_OFFSET,
    );
    write_u32(dma + TX_DESC_OFFSET as usize + 8, payload_length as u32);
    write_u16(dma + TX_DESC_OFFSET as usize + 12, 0);
    write_u16(dma + TX_DESC_OFFSET as usize + 14, 0);
    for offset in 0..TX_HEADER_LENGTH {
        write_u8(dma + TX_BUFFER_OFFSET as usize + offset, 0);
    }
    for (offset, byte) in TX_FRAME.into_iter().enumerate() {
        write_u8(
            dma + TX_BUFFER_OFFSET as usize + TX_HEADER_LENGTH + offset,
            byte,
        );
    }
    write_u8(dma + TX_BUFFER_OFFSET as usize - 1, 0x3c);
    write_u8(dma + TX_BUFFER_OFFSET as usize + payload_length, 0xc3);
    write_u16(dma + TX_AVAIL_OFFSET as usize, 0);
    write_u16(dma + TX_AVAIL_OFFSET as usize + 4, 0);
    compiler_fence(Ordering::SeqCst);
    write_u16(dma + TX_AVAIL_OFFSET as usize + 2, 1);
    compiler_fence(Ordering::SeqCst);
    write_u16(notification, 1);
    if info.requests_tx_interrupt() {
        match wait_for_tx_interrupt(info.device_capability, TX_INTERRUPT_REQUEST) {
            0 => {}
            NATIVE_NETWORK_WAIT_CANCELLED => {
                return Some(NATIVE_SERVICE_EXIT_RESTART_REQUIRED);
            }
            _ => return None,
        }
        if wait_for_tx_interrupt(info.device_capability, TX_INTERRUPT_REQUEST)
            != NATIVE_NETWORK_WAIT_FAILED
        {
            return None;
        }
    } else {
        let mut completed = false;
        for _ in 0..COMPLETION_POLL_LIMIT {
            if read_u16(dma + TX_USED_OFFSET as usize + 2) == 1 {
                completed = true;
                break;
            }
            core::hint::spin_loop();
        }
        if !completed {
            return None;
        }
    }
    compiler_fence(Ordering::SeqCst);
    (read_u16(dma + TX_USED_OFFSET as usize + 2) == 1
        && read_u32(dma + TX_USED_OFFSET as usize + 4) == 0
        && read_u8(common + COMMON_DEVICE_STATUS)
            == STATUS_ACKNOWLEDGE | STATUS_DRIVER | STATUS_FEATURES_OK | STATUS_DRIVER_OK)
        .then_some(NATIVE_SERVICE_EXIT_COMPLETED)
}

unsafe fn transmit_egress(
    info: &NativeNetworkBootV0,
    egress_capability: u64,
    expected_sequence: u64,
    reliable: bool,
    fault_after_claim: bool,
    wait: bool,
) -> Option<u8> {
    let dma = usize::try_from(info.dma_virtual_address).ok()?;
    let notification = usize::try_from(info.transmit_notification_address).ok()?;
    let common = usize::try_from(info.common_config_address).ok()?;
    let frame = dma + TX_BUFFER_OFFSET as usize + TX_HEADER_LENGTH;
    let future_sequence = expected_sequence.checked_add(1)?;
    for offset in 0..ECHO_REPLY.len() {
        write_u8(frame + offset, 0xcc);
    }
    let publish = NativePacketRequestV0::publish(frame as u64, ECHO_REPLY.len() as u32);
    let future =
        NativePacketRequestV0::consume(frame as u64, ECHO_REPLY.len() as u32, future_sequence);
    let future_wait = NativePacketRequestV0::consume(0, 0, future_sequence);
    let exact_wait = NativePacketRequestV0::consume(0, 0, expected_sequence);
    if publish_packet(egress_capability, &publish) != NATIVE_NETWORK_WAIT_FAILED {
        return None;
    }
    if !wait && consume_packet(egress_capability, &future) != NATIVE_PACKET_STALE {
        return None;
    }
    if wait {
        if wait_for_egress_packet(egress_capability, &future_wait) != NATIVE_PACKET_STALE {
            return None;
        }
        if wait_for_egress_packet(info.device_capability, &exact_wait) != NATIVE_NETWORK_WAIT_FAILED
        {
            return None;
        }
        if wait_for_egress_packet(egress_capability, &exact_wait) != expected_sequence {
            return None;
        }
    }
    if (0..ECHO_REPLY.len()).any(|offset| read_u8(frame + offset) != 0xcc) {
        return None;
    }
    let transfer =
        NativePacketRequestV0::consume(frame as u64, ECHO_REPLY.len() as u32, expected_sequence);
    let transferred = if reliable {
        claim_egress_packet(egress_capability, &transfer)
    } else {
        consume_packet(egress_capability, &transfer)
    };
    if transferred != ECHO_REPLY.len() as u64 || !bytes_match(frame, &ECHO_REPLY) {
        return None;
    }
    if reliable {
        let acknowledge = NativePacketRequestV0::consume(0, 0, expected_sequence);
        if acknowledge_egress_packet(egress_capability, &acknowledge) != NATIVE_NETWORK_WAIT_FAILED
            || claim_egress_packet(egress_capability, &transfer) != NATIVE_NETWORK_WAIT_FAILED
        {
            return None;
        }
        if fault_after_claim {
            fail();
        }
    } else if consume_packet(egress_capability, &transfer) != NATIVE_NETWORK_WAIT_FAILED {
        return None;
    }

    let payload_length = TX_HEADER_LENGTH + ECHO_REPLY.len();
    write_u64(
        dma + TX_DESC_OFFSET as usize,
        info.dma_device_address + TX_BUFFER_OFFSET,
    );
    write_u32(dma + TX_DESC_OFFSET as usize + 8, payload_length as u32);
    write_u16(dma + TX_DESC_OFFSET as usize + 12, 0);
    write_u16(dma + TX_DESC_OFFSET as usize + 14, 0);
    for offset in 0..TX_HEADER_LENGTH {
        write_u8(dma + TX_BUFFER_OFFSET as usize + offset, 0);
    }
    write_u8(dma + TX_BUFFER_OFFSET as usize - 1, 0x3c);
    write_u8(dma + TX_BUFFER_OFFSET as usize + payload_length, 0xc3);
    write_u16(dma + TX_AVAIL_OFFSET as usize, 0);
    write_u16(dma + TX_AVAIL_OFFSET as usize + 4, 0);
    compiler_fence(Ordering::SeqCst);
    write_u16(dma + TX_AVAIL_OFFSET as usize + 2, 1);
    compiler_fence(Ordering::SeqCst);
    write_u16(notification, 1);
    match wait_for_tx_interrupt(info.device_capability, TX_INTERRUPT_REQUEST) {
        0 => {}
        NATIVE_NETWORK_WAIT_CANCELLED => return Some(NATIVE_SERVICE_EXIT_RESTART_REQUIRED),
        _ => return None,
    }
    if wait_for_tx_interrupt(info.device_capability, TX_INTERRUPT_REQUEST)
        != NATIVE_NETWORK_WAIT_FAILED
    {
        return None;
    }
    compiler_fence(Ordering::SeqCst);
    let complete = read_u16(dma + TX_USED_OFFSET as usize + 2) == 1
        && read_u32(dma + TX_USED_OFFSET as usize + 4) == 0
        && read_u8(common + COMMON_DEVICE_STATUS)
            == STATUS_ACKNOWLEDGE | STATUS_DRIVER | STATUS_FEATURES_OK | STATUS_DRIVER_OK;
    if !complete {
        return None;
    }
    if reliable {
        let acknowledge = NativePacketRequestV0::consume(0, 0, expected_sequence);
        if acknowledge_egress_packet(egress_capability, &acknowledge) != 0
            || claim_egress_packet(egress_capability, &transfer) != NATIVE_NETWORK_WAIT_FAILED
        {
            return None;
        }
    }
    Some(NATIVE_SERVICE_EXIT_COMPLETED)
}

unsafe fn arp_probe(
    info: &NativeNetworkBootV0,
    packet_capability: Option<u64>,
    fault_after_packet: bool,
    packet_batch: bool,
) -> Option<u8> {
    const ARP_REQUEST: [u8; 60] = [
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x52, 0x54, 0x00, 0x12, 0x34, 0x56, 0x08, 0x06, 0x00,
        0x01, 0x08, 0x00, 0x06, 0x04, 0x00, 0x01, 0x52, 0x54, 0x00, 0x12, 0x34, 0x56, 10, 0, 2, 15,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 10, 0, 2, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0,
    ];
    let dma = usize::try_from(info.dma_virtual_address).ok()?;
    let receive_notification = usize::try_from(info.receive_notification_address).ok()?;
    let transmit_notification = usize::try_from(info.transmit_notification_address).ok()?;
    let common = usize::try_from(info.common_config_address).ok()?;
    let tx_length = TX_HEADER_LENGTH + ARP_REQUEST.len();

    write_u64(
        dma + RX_DESC_OFFSET as usize,
        info.dma_device_address + RX_BUFFER_OFFSET,
    );
    write_u32(
        dma + RX_DESC_OFFSET as usize + 8,
        RX_DESCRIPTOR_LENGTH as u32,
    );
    write_u16(dma + RX_DESC_OFFSET as usize + 12, 2);
    write_u16(dma + RX_DESC_OFFSET as usize + 14, 0);
    write_u8(dma + RX_BUFFER_OFFSET as usize - 1, 0x3c);
    write_u8(dma + RX_BUFFER_OFFSET as usize + RX_DESCRIPTOR_LENGTH, 0xc3);
    write_u16(dma + RX_AVAIL_OFFSET as usize, 0);
    write_u16(dma + RX_AVAIL_OFFSET as usize + 4, 0);
    compiler_fence(Ordering::SeqCst);
    write_u16(dma + RX_AVAIL_OFFSET as usize + 2, 1);

    write_u64(
        dma + TX_DESC_OFFSET as usize,
        info.dma_device_address + TX_BUFFER_OFFSET,
    );
    write_u32(dma + TX_DESC_OFFSET as usize + 8, tx_length as u32);
    write_u16(dma + TX_DESC_OFFSET as usize + 12, 0);
    write_u16(dma + TX_DESC_OFFSET as usize + 14, 0);
    for offset in 0..TX_HEADER_LENGTH {
        write_u8(dma + TX_BUFFER_OFFSET as usize + offset, 0);
    }
    for (offset, byte) in ARP_REQUEST.into_iter().enumerate() {
        write_u8(
            dma + TX_BUFFER_OFFSET as usize + TX_HEADER_LENGTH + offset,
            byte,
        );
    }
    write_u8(dma + TX_BUFFER_OFFSET as usize + tx_length, 0xa5);
    write_u16(dma + TX_AVAIL_OFFSET as usize, 0);
    write_u16(dma + TX_AVAIL_OFFSET as usize + 4, 0);
    compiler_fence(Ordering::SeqCst);
    write_u16(dma + TX_AVAIL_OFFSET as usize + 2, 1);
    compiler_fence(Ordering::SeqCst);
    write_u16(receive_notification, 0);
    write_u16(transmit_notification, 1);

    match wait_for_network_interrupt(info.device_capability, NATIVE_NETWORK_WAIT_TX_NOTIFIED, 1) {
        0 => {}
        NATIVE_NETWORK_WAIT_CANCELLED => return Some(NATIVE_SERVICE_EXIT_RESTART_REQUIRED),
        _ => return None,
    }
    if read_u16(dma + TX_USED_OFFSET as usize + 2) != 1
        || read_u32(dma + TX_USED_OFFSET as usize + 4) != 0
    {
        return None;
    }
    match wait_for_network_interrupt(info.device_capability, NATIVE_NETWORK_WAIT_RX_POSTED, 2) {
        0 => {}
        NATIVE_NETWORK_WAIT_CANCELLED => return Some(NATIVE_SERVICE_EXIT_RESTART_REQUIRED),
        _ => return None,
    }
    if wait_for_network_interrupt(info.device_capability, NATIVE_NETWORK_WAIT_RX_POSTED, 2)
        != NATIVE_NETWORK_WAIT_FAILED
    {
        return None;
    }
    if !arp_reply_is_valid(dma, common, tx_length, 1) {
        return None;
    }
    if packet_batch {
        let capability = packet_capability?;
        let frame = (dma + RX_BUFFER_OFFSET as usize + TX_HEADER_LENGTH) as u64;
        let request = NativePacketRequestV0::publish(frame, 42);
        if matches!(
            publish_packet(capability, &request),
            0 | NATIVE_NETWORK_WAIT_FAILED
        ) {
            return None;
        }
    }
    if !info.requests_rx_reuse() {
        return Some(NATIVE_SERVICE_EXIT_COMPLETED);
    }

    for offset in 0..TX_HEADER_LENGTH + 42 {
        write_u8(dma + RX_BUFFER_OFFSET as usize + offset, 0xcc);
    }
    write_u16(dma + RX_AVAIL_OFFSET as usize + 6, 0);
    compiler_fence(Ordering::SeqCst);
    write_u16(dma + RX_AVAIL_OFFSET as usize + 2, 2);
    write_u16(dma + TX_AVAIL_OFFSET as usize + 6, 0);
    compiler_fence(Ordering::SeqCst);
    write_u16(dma + TX_AVAIL_OFFSET as usize + 2, 2);
    compiler_fence(Ordering::SeqCst);
    write_u16(receive_notification, 0);
    write_u16(transmit_notification, 1);

    match wait_for_network_interrupt(info.device_capability, NATIVE_NETWORK_WAIT_TX_NOTIFIED, 3) {
        0 => {}
        NATIVE_NETWORK_WAIT_CANCELLED => return Some(NATIVE_SERVICE_EXIT_RESTART_REQUIRED),
        _ => return None,
    }
    if read_u16(dma + TX_USED_OFFSET as usize + 2) != 2
        || read_u32(dma + TX_USED_OFFSET as usize + 12) != 0
    {
        return None;
    }
    match wait_for_network_interrupt(info.device_capability, NATIVE_NETWORK_WAIT_RX_POSTED, 4) {
        0 => {}
        NATIVE_NETWORK_WAIT_CANCELLED => return Some(NATIVE_SERVICE_EXIT_RESTART_REQUIRED),
        _ => return None,
    }
    if wait_for_network_interrupt(info.device_capability, NATIVE_NETWORK_WAIT_RX_POSTED, 4)
        != NATIVE_NETWORK_WAIT_FAILED
    {
        return None;
    }
    if !arp_reply_is_valid(dma, common, tx_length, 2) {
        return None;
    }
    if let Some(capability) = packet_capability {
        let frame = (dma + RX_BUFFER_OFFSET as usize + TX_HEADER_LENGTH) as u64;
        let request = NativePacketRequestV0::publish(frame, 42);
        let read_request = NativePacketRequestV0 {
            expected_sequence: 1,
            ..request
        };
        let sequence = publish_packet(capability, &request);
        if consume_packet(capability, &read_request) != NATIVE_NETWORK_WAIT_FAILED
            || matches!(sequence, 0 | NATIVE_NETWORK_WAIT_FAILED)
        {
            return None;
        }
        if fault_after_packet {
            fail();
        }
    }
    Some(NATIVE_SERVICE_EXIT_COMPLETED)
}

unsafe fn arp_reply_is_valid(dma: usize, common: usize, tx_length: usize, used_index: u16) -> bool {
    const GUEST_MAC: [u8; 6] = [0x52, 0x54, 0x00, 0x12, 0x34, 0x56];
    const GATEWAY_MAC: [u8; 6] = [0x52, 0x55, 0x0a, 0x00, 0x02, 0x02];
    const GUEST_IP: [u8; 4] = [10, 0, 2, 15];
    const GATEWAY_IP: [u8; 4] = [10, 0, 2, 2];
    if !(1..=2).contains(&used_index) {
        return false;
    }
    let used_element = RX_USED_OFFSET as usize + 4 + (usize::from(used_index) - 1) * 8;
    let used_length = read_u32(dma + used_element + 4) as usize;
    let buffer = dma + RX_BUFFER_OFFSET as usize;
    let frame = buffer + TX_HEADER_LENGTH;
    read_u16(dma + RX_USED_OFFSET as usize + 2) == used_index
        && read_u32(dma + used_element) == 0
        && (TX_HEADER_LENGTH + 42..=RX_DESCRIPTOR_LENGTH).contains(&used_length)
        && (0..10).all(|offset| read_u8(buffer + offset) == 0)
        && read_u16(buffer + 10) == 1
        && bytes_match(frame, &GUEST_MAC)
        && bytes_match(frame + 6, &GATEWAY_MAC)
        && bytes_match(frame + 12, &[0x08, 0x06])
        && bytes_match(
            frame + 14,
            &[0x00, 0x01, 0x08, 0x00, 0x06, 0x04, 0x00, 0x02],
        )
        && bytes_match(frame + 22, &GATEWAY_MAC)
        && bytes_match(frame + 28, &GATEWAY_IP)
        && bytes_match(frame + 32, &GUEST_MAC)
        && bytes_match(frame + 38, &GUEST_IP)
        && read_u8(dma + RX_BUFFER_OFFSET as usize - 1) == 0x3c
        && read_u8(dma + RX_BUFFER_OFFSET as usize + RX_DESCRIPTOR_LENGTH) == 0xc3
        && read_u8(dma + TX_BUFFER_OFFSET as usize + tx_length) == 0xa5
        && read_u8(common + COMMON_DEVICE_STATUS)
            == STATUS_ACKNOWLEDGE | STATUS_DRIVER | STATUS_FEATURES_OK | STATUS_DRIVER_OK
}

unsafe fn bytes_match(address: usize, expected: &[u8]) -> bool {
    expected
        .iter()
        .enumerate()
        .all(|(offset, byte)| read_u8(address + offset) == *byte)
}

unsafe fn configure_empty_queues(info: &NativeNetworkBootV0) -> bool {
    let Ok(common) = usize::try_from(info.common_config_address) else {
        return false;
    };
    let Ok(dma) = usize::try_from(info.dma_virtual_address) else {
        return false;
    };
    if !dma.is_multiple_of(DMA_PAGE_SIZE)
        || !info.dma_device_address.is_multiple_of(DMA_PAGE_SIZE as u64)
    {
        return false;
    }
    for offset in 0..DMA_PAGE_SIZE {
        write_u8(dma + offset, 0);
    }
    write_u8(common + COMMON_DEVICE_STATUS, 0);
    let mut reset = false;
    for _ in 0..RESET_POLL_LIMIT {
        if read_u8(common + COMMON_DEVICE_STATUS) == 0 {
            reset = true;
            break;
        }
        core::hint::spin_loop();
    }
    if !reset {
        return false;
    }
    write_u8(common + COMMON_DEVICE_STATUS, STATUS_ACKNOWLEDGE);
    write_u8(
        common + COMMON_DEVICE_STATUS,
        STATUS_ACKNOWLEDGE | STATUS_DRIVER,
    );
    write_u32(common + COMMON_DEVICE_FEATURE_SELECT, 0);
    if read_u32(common + COMMON_DEVICE_FEATURE) & (FEATURE_NET_MAC | FEATURE_NET_MRG_RXBUF)
        != FEATURE_NET_MAC | FEATURE_NET_MRG_RXBUF
    {
        return false;
    }
    write_u32(common + COMMON_DEVICE_FEATURE_SELECT, 1);
    if read_u32(common + COMMON_DEVICE_FEATURE) & (FEATURE_VERSION_1 | FEATURE_ACCESS_PLATFORM)
        != FEATURE_VERSION_1 | FEATURE_ACCESS_PLATFORM
    {
        return false;
    }
    write_u32(common + COMMON_DRIVER_FEATURE_SELECT, 0);
    write_u32(
        common + COMMON_DRIVER_FEATURE,
        FEATURE_NET_MAC | FEATURE_NET_MRG_RXBUF,
    );
    write_u32(common + COMMON_DRIVER_FEATURE_SELECT, 1);
    write_u32(
        common + COMMON_DRIVER_FEATURE,
        FEATURE_VERSION_1 | FEATURE_ACCESS_PLATFORM,
    );
    write_u8(
        common + COMMON_DEVICE_STATUS,
        STATUS_ACKNOWLEDGE | STATUS_DRIVER | STATUS_FEATURES_OK,
    );
    if read_u8(common + COMMON_DEVICE_STATUS)
        != STATUS_ACKNOWLEDGE | STATUS_DRIVER | STATUS_FEATURES_OK
        || read_u16(common + COMMON_NUM_QUEUES) < 2
    {
        return false;
    }
    for (queue, descriptor, available, used, vector) in [
        (
            0_u16,
            RX_DESC_OFFSET,
            RX_AVAIL_OFFSET,
            RX_USED_OFFSET,
            if info.requests_arp_probe() {
                0
            } else {
                u16::MAX
            },
        ),
        (
            1_u16,
            TX_DESC_OFFSET,
            TX_AVAIL_OFFSET,
            TX_USED_OFFSET,
            if info.requests_tx_interrupt() {
                1
            } else {
                u16::MAX
            },
        ),
    ] {
        write_u16(common + COMMON_QUEUE_SELECT, queue);
        let maximum = read_u16(common + COMMON_QUEUE_SIZE);
        if maximum < QUEUE_SIZE
            || !maximum.is_power_of_two()
            || read_u16(common + COMMON_QUEUE_ENABLE) != 0
            || read_u16(common + COMMON_QUEUE_MSIX_VECTOR) != u16::MAX
        {
            return false;
        }
        write_u16(common + COMMON_QUEUE_SIZE, QUEUE_SIZE);
        write_u16(common + COMMON_QUEUE_MSIX_VECTOR, vector);
        write_u64(
            common + COMMON_QUEUE_DESC,
            info.dma_device_address + descriptor,
        );
        write_u64(
            common + COMMON_QUEUE_DRIVER,
            info.dma_device_address + available,
        );
        write_u64(common + COMMON_QUEUE_DEVICE, info.dma_device_address + used);
        write_u16(common + COMMON_QUEUE_ENABLE, 1);
        if read_u16(common + COMMON_QUEUE_SIZE) != QUEUE_SIZE
            || read_u16(common + COMMON_QUEUE_ENABLE) != 1
            || read_u16(common + COMMON_QUEUE_MSIX_VECTOR) != vector
            || read_u64(common + COMMON_QUEUE_DESC) != info.dma_device_address + descriptor
            || read_u64(common + COMMON_QUEUE_DRIVER) != info.dma_device_address + available
            || read_u64(common + COMMON_QUEUE_DEVICE) != info.dma_device_address + used
        {
            return false;
        }
    }
    write_u8(
        common + COMMON_DEVICE_STATUS,
        STATUS_ACKNOWLEDGE | STATUS_DRIVER | STATUS_FEATURES_OK | STATUS_DRIVER_OK,
    );
    read_u8(common + COMMON_DEVICE_STATUS)
        == STATUS_ACKNOWLEDGE | STATUS_DRIVER | STATUS_FEATURES_OK | STATUS_DRIVER_OK
}

fn wait_for_tx_interrupt(capability: u64, request: u64) -> u64 {
    wait_for_network_interrupt(capability, NATIVE_NETWORK_WAIT_TX_NOTIFIED, request)
}

fn wait_for_network_interrupt(capability: u64, command: u64, request: u64) -> u64 {
    let mut result = NATIVE_NETWORK_WAIT_OPERATION;
    unsafe {
        asm!(
            "int 0x80",
            inlateout("rax") result,
            in("rdi") capability,
            in("rsi") command,
            in("rdx") request,
            options(nostack),
        );
    }
    result
}

fn activate_empty_queues(capability: u64) -> u64 {
    let mut result = NATIVE_NETWORK_DEVICE_OPERATION;
    unsafe {
        asm!(
            "int 0x80",
            inlateout("rax") result,
            in("rdi") capability,
            in("rsi") NATIVE_NETWORK_DEVICE_ACTIVATE_EMPTY_QUEUES,
            in("rdx") 0_u64,
            options(nostack),
        );
    }
    result
}

fn publish_packet(capability: u64, request: &NativePacketRequestV0) -> u64 {
    let mut result = NATIVE_PACKET_PUBLISH_OPERATION;
    unsafe {
        asm!(
            "int 0x80",
            inlateout("rax") result,
            in("rdi") capability,
            in("rsi") request as *const NativePacketRequestV0 as u64,
            options(nostack),
        );
    }
    result
}

fn consume_packet(capability: u64, request: &NativePacketRequestV0) -> u64 {
    let mut result = 22_u64;
    unsafe {
        asm!(
            "int 0x80",
            inlateout("rax") result,
            in("rdi") capability,
            in("rsi") request as *const NativePacketRequestV0 as u64,
            options(nostack),
        );
    }
    result
}

fn claim_egress_packet(capability: u64, request: &NativePacketRequestV0) -> u64 {
    let mut result = NATIVE_PACKET_EGRESS_CLAIM_OPERATION;
    unsafe {
        asm!(
            "int 0x80",
            inlateout("rax") result,
            in("rdi") capability,
            in("rsi") request as *const NativePacketRequestV0 as u64,
            options(nostack),
        );
    }
    result
}

fn acknowledge_egress_packet(capability: u64, request: &NativePacketRequestV0) -> u64 {
    let mut result = NATIVE_PACKET_EGRESS_ACK_OPERATION;
    unsafe {
        asm!(
            "int 0x80",
            inlateout("rax") result,
            in("rdi") capability,
            in("rsi") request as *const NativePacketRequestV0 as u64,
            options(nostack),
        );
    }
    result
}

fn wait_for_egress_packet(capability: u64, request: &NativePacketRequestV0) -> u64 {
    let mut result = NATIVE_PACKET_EGRESS_WAIT_OPERATION;
    unsafe {
        asm!(
            "int 0x80",
            inlateout("rax") result,
            in("rdi") capability,
            in("rsi") request as *const NativePacketRequestV0 as u64,
            options(nostack),
        );
    }
    result
}

unsafe fn read_u8(address: usize) -> u8 {
    compiler_fence(Ordering::SeqCst);
    let value = read_volatile(address as *const u8);
    compiler_fence(Ordering::SeqCst);
    value
}

unsafe fn read_u16(address: usize) -> u16 {
    compiler_fence(Ordering::SeqCst);
    let value = read_volatile(address as *const u16);
    compiler_fence(Ordering::SeqCst);
    value
}

unsafe fn read_u32(address: usize) -> u32 {
    compiler_fence(Ordering::SeqCst);
    let value = read_volatile(address as *const u32);
    compiler_fence(Ordering::SeqCst);
    value
}

unsafe fn read_u64(address: usize) -> u64 {
    compiler_fence(Ordering::SeqCst);
    let value = read_volatile(address as *const u64);
    compiler_fence(Ordering::SeqCst);
    value
}

unsafe fn write_u8(address: usize, value: u8) {
    compiler_fence(Ordering::SeqCst);
    write_volatile(address as *mut u8, value);
    compiler_fence(Ordering::SeqCst);
}

unsafe fn write_u16(address: usize, value: u16) {
    compiler_fence(Ordering::SeqCst);
    write_volatile(address as *mut u16, value);
    compiler_fence(Ordering::SeqCst);
}

unsafe fn write_u32(address: usize, value: u32) {
    compiler_fence(Ordering::SeqCst);
    write_volatile(address as *mut u32, value);
    compiler_fence(Ordering::SeqCst);
}

unsafe fn write_u64(address: usize, value: u64) {
    compiler_fence(Ordering::SeqCst);
    write_volatile(address as *mut u64, value);
    compiler_fence(Ordering::SeqCst);
}

fn probe_network_device(capability: u64) -> u64 {
    let mut result = NATIVE_NETWORK_DEVICE_OPERATION;
    unsafe {
        asm!(
            "int 0x80",
            inlateout("rax") result,
            in("rdi") capability,
            in("rsi") NATIVE_NETWORK_DEVICE_PROBE,
            in("rdx") 0_u64,
            options(nostack),
        );
    }
    result
}

fn discover_control_abi() -> u64 {
    let mut result = NATIVE_CONTROL_DISCOVERY_OPERATION;
    unsafe {
        asm!(
            "int 0x80",
            inlateout("rax") result,
            in("rdi") 0_u64,
            in("rsi") 0_u64,
            in("rdx") 0_u64,
            options(nostack),
        );
    }
    result
}

fn discover_control_descriptor() -> NativeControlDescriptorV4 {
    let mut descriptor = NativeControlDescriptorV4::zeroed();
    let mut result = NATIVE_CONTROL_DESCRIPTOR_OPERATION;
    unsafe {
        asm!(
            "int 0x80",
            inlateout("rax") result,
            in("rdi") 0_u64,
            in("rsi") &mut descriptor as *mut NativeControlDescriptorV4 as u64,
            in("rdx") u64::from(NATIVE_CONTROL_DESCRIPTOR_SIZE),
            options(nostack),
        );
    }
    if result == u64::from(NATIVE_CONTROL_DESCRIPTOR_SIZE) {
        descriptor
    } else {
        NativeControlDescriptorV4::zeroed()
    }
}

fn exit(status: u8) -> ! {
    unsafe {
        asm!(
            "int 0x80",
            in("rax") NATIVE_SERVICE_EXIT_OPERATION,
            in("rdi") 0_u64,
            in("rsi") u64::from(status),
            options(noreturn),
        )
    }
}

fn fail() -> ! {
    unsafe { asm!("hlt", options(noreturn)) }
}
