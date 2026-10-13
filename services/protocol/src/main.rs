#![no_std]
#![no_main]

use core::arch::{asm, global_asm};

use ginux_native_abi::{
    native_control_abi_is_compatible, native_control_abi_supports_packet_handoff,
    NativeControlDescriptorV4, NativePacketRequestV0, NativeProtocolBootV0, NATIVE_CALL_FAILED,
    NATIVE_CONTROL_DESCRIPTOR_OPERATION, NATIVE_CONTROL_DESCRIPTOR_SIZE,
    NATIVE_CONTROL_DISCOVERY_OPERATION, NATIVE_NETWORK_DEVICE_OPERATION,
    NATIVE_NETWORK_DEVICE_PROBE, NATIVE_PACKET_CONSUME_OPERATION, NATIVE_PACKET_MAX_SIZE,
    NATIVE_PACKET_PUBLISH_OPERATION, NATIVE_SERVICE_EXIT_COMPLETED, NATIVE_SERVICE_EXIT_OPERATION,
};

static SERVICE_IDENTITY: &[u8] = b"GINUX_PROTOCOL_SERVICE_V0";

global_asm!(
    r#"
    .text
    .global _start
_start:
    mov rdi, rbx
    call ginux_protocol_service_main
    int3
"#
);

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo<'_>) -> ! {
    fail()
}

#[no_mangle]
extern "C" fn ginux_protocol_service_main(info: *const NativeProtocolBootV0) -> ! {
    let identity = unsafe { core::ptr::read_volatile(&SERVICE_IDENTITY) };
    let discovery = discover_control_abi();
    if identity != b"GINUX_PROTOCOL_SERVICE_V0"
        || !native_control_abi_is_compatible(discovery)
        || !native_control_abi_supports_packet_handoff(discovery)
        || !discover_control_descriptor().is_compatible()
    {
        fail();
    }
    let Some(info) = (unsafe { info.as_ref() }) else {
        fail();
    };
    if !info.is_valid_arp_consumer() {
        fail();
    }

    let mut packet = [0_u8; NATIVE_PACKET_MAX_SIZE as usize];
    let publish = NativePacketRequestV0::publish(packet.as_ptr() as u64, 42);
    if packet_publish(info.packet_capability, &publish) != NATIVE_CALL_FAILED
        || network_probe(info.packet_capability) != NATIVE_CALL_FAILED
    {
        fail();
    }

    let consume = NativePacketRequestV0::consume(
        packet.as_mut_ptr() as u64,
        NATIVE_PACKET_MAX_SIZE,
        info.expected_sequence,
    );
    if packet_consume(info.packet_capability, &consume) != 42
        || !valid_arp_reply(
            &packet[..42],
            info.local_mac,
            info.local_ipv4,
            info.peer_ipv4,
        )
    {
        fail();
    }

    packet[21] = 1;
    if valid_arp_reply(
        &packet[..42],
        info.local_mac,
        info.local_ipv4,
        info.peer_ipv4,
    ) || packet_consume(info.packet_capability, &consume) != NATIVE_CALL_FAILED
    {
        fail();
    }
    exit(NATIVE_SERVICE_EXIT_COMPLETED)
}

fn valid_arp_reply(frame: &[u8], local_mac: u64, local_ipv4: u32, peer_ipv4: u32) -> bool {
    if frame.len() != 42 || local_mac >> 48 != 0 {
        return false;
    }
    let local_mac = local_mac.to_be_bytes();
    let source = &frame[6..12];
    frame[0..6] == local_mac[2..8]
        && frame[12..14] == [0x08, 0x06]
        && frame[14..22] == [0x00, 0x01, 0x08, 0x00, 0x06, 0x04, 0x00, 0x02]
        && source == &frame[22..28]
        && source != [0; 6]
        && source != [0xff; 6]
        && source[0] & 1 == 0
        && frame[28..32] == peer_ipv4.to_be_bytes()
        && frame[32..38] == local_mac[2..8]
        && frame[38..42] == local_ipv4.to_be_bytes()
}

fn packet_publish(capability: u64, request: &NativePacketRequestV0) -> u64 {
    native_call(
        NATIVE_PACKET_PUBLISH_OPERATION,
        capability,
        request as *const _ as u64,
        0,
    )
}

fn packet_consume(capability: u64, request: &NativePacketRequestV0) -> u64 {
    native_call(
        NATIVE_PACKET_CONSUME_OPERATION,
        capability,
        request as *const _ as u64,
        0,
    )
}

fn network_probe(capability: u64) -> u64 {
    native_call(
        NATIVE_NETWORK_DEVICE_OPERATION,
        capability,
        NATIVE_NETWORK_DEVICE_PROBE,
        0,
    )
}

fn native_call(operation: u64, capability: u64, first: u64, second: u64) -> u64 {
    let mut result = operation;
    unsafe {
        asm!(
            "int 0x80",
            inlateout("rax") result,
            in("rdi") capability,
            in("rsi") first,
            in("rdx") second,
            options(nostack),
        );
    }
    result
}

fn discover_control_abi() -> u64 {
    native_call(NATIVE_CONTROL_DISCOVERY_OPERATION, 0, 0, 0)
}

fn discover_control_descriptor() -> NativeControlDescriptorV4 {
    let mut descriptor = NativeControlDescriptorV4::zeroed();
    let result = native_call(
        NATIVE_CONTROL_DESCRIPTOR_OPERATION,
        0,
        &mut descriptor as *mut NativeControlDescriptorV4 as u64,
        u64::from(NATIVE_CONTROL_DESCRIPTOR_SIZE),
    );
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
        );
    }
}

fn fail() -> ! {
    unsafe { asm!("int3", options(noreturn)) }
}
