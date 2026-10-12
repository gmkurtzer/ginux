#![no_std]
#![no_main]

use core::arch::{asm, global_asm};

use ginux_native_abi::{
    native_control_abi_is_compatible, NativeControlDescriptorV4, NativeNetworkBootV0,
    NATIVE_CONTROL_DESCRIPTOR_OPERATION, NATIVE_CONTROL_DESCRIPTOR_SIZE,
    NATIVE_CONTROL_DISCOVERY_OPERATION, NATIVE_NETWORK_DEVICE_DESCRIPTOR_WORD,
    NATIVE_NETWORK_DEVICE_OPERATION, NATIVE_NETWORK_DEVICE_PROBE, NATIVE_SERVICE_EXIT_COMPLETED,
    NATIVE_SERVICE_EXIT_OPERATION,
};

static SERVICE_IDENTITY: &[u8] = b"GINUX_NETWORK_SERVICE_V0";

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
    if identity != b"GINUX_NETWORK_SERVICE_V0"
        || !native_control_abi_is_compatible(discover_control_abi())
        || !discover_control_descriptor().is_compatible()
    {
        fail();
    }
    if !info.is_null() {
        let Some(info) = (unsafe { info.as_ref() }) else {
            fail();
        };
        if !info.is_valid_virtio_network()
            || info.mac_address != 0x5254_0012_3456
            || probe_network_device(info.device_capability) != NATIVE_NETWORK_DEVICE_DESCRIPTOR_WORD
        {
            fail();
        }
    }
    exit(NATIVE_SERVICE_EXIT_COMPLETED)
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
