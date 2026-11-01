#![no_std]
#![no_main]

use core::arch::{asm, global_asm};

use ginux_native_abi::{
    native_control_abi_is_compatible, NativeControlDescriptorV4, NativeGraphicsBootV0,
    NATIVE_CONSOLE_WRITE_OPERATION, NATIVE_CONTROL_DESCRIPTOR_OPERATION,
    NATIVE_CONTROL_DESCRIPTOR_SIZE, NATIVE_CONTROL_DISCOVERY_OPERATION,
    NATIVE_GRAPHICS_COMMAND_COMPLETE, NATIVE_GRAPHICS_COMMAND_FAULT, NATIVE_SERVICE_EXIT_COMPLETED,
    NATIVE_SERVICE_EXIT_OPERATION,
};

const READY: &[u8] = b"ginux: graphical controller service ready\r\n";
static SERVICE_IDENTITY: &[u8] = b"GINUX_GRAPHICS_SERVICE_V0";

global_asm!(
    r#"
    .text
    .global _start
_start:
    mov rdi, rbx
    call ginux_graphics_service_main
    int3
"#
);

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo<'_>) -> ! {
    fail()
}

#[no_mangle]
extern "C" fn ginux_graphics_service_main(info: *const NativeGraphicsBootV0) -> ! {
    let identity = unsafe { core::ptr::read_volatile(&SERVICE_IDENTITY) };
    if identity != b"GINUX_GRAPHICS_SERVICE_V0"
        || !native_control_abi_is_compatible(call(NATIVE_CONTROL_DISCOVERY_OPERATION, 0, 0, 0))
        || !descriptor().is_compatible()
    {
        fail();
    }
    let Some(info) = (unsafe { info.as_ref() }) else {
        fail();
    };
    if !info.is_valid() {
        fail();
    }
    if call(
        NATIVE_CONSOLE_WRITE_OPERATION,
        info.console_capability,
        READY.as_ptr() as u64,
        READY.len() as u64,
    ) != READY.len() as u64
    {
        fail();
    }
    match info.command {
        NATIVE_GRAPHICS_COMMAND_COMPLETE => exit(NATIVE_SERVICE_EXIT_COMPLETED),
        NATIVE_GRAPHICS_COMMAND_FAULT => fail(),
        _ => fail(),
    }
}

fn descriptor() -> NativeControlDescriptorV4 {
    let mut descriptor = NativeControlDescriptorV4::zeroed();
    if call(
        NATIVE_CONTROL_DESCRIPTOR_OPERATION,
        0,
        (&mut descriptor as *mut NativeControlDescriptorV4) as u64,
        u64::from(NATIVE_CONTROL_DESCRIPTOR_SIZE),
    ) == u64::from(NATIVE_CONTROL_DESCRIPTOR_SIZE)
    {
        descriptor
    } else {
        NativeControlDescriptorV4::zeroed()
    }
}

fn call(operation: u64, capability: u64, first: u64, second: u64) -> u64 {
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
