#![no_std]
#![no_main]

use core::arch::{asm, global_asm};
use ginux_native_abi::{
    native_control_abi_is_compatible, NativeControlDescriptorV4, NATIVE_CALL_FAILED,
    NATIVE_CONSOLE_READ_LINE_OPERATION, NATIVE_CONSOLE_WRITE_OPERATION,
    NATIVE_CONTROL_DESCRIPTOR_OPERATION, NATIVE_CONTROL_DESCRIPTOR_SIZE,
    NATIVE_CONTROL_DISCOVERY_OPERATION, NATIVE_SERVICE_EXIT_COMPLETED,
    NATIVE_SERVICE_EXIT_OPERATION, NATIVE_SERVICE_EXIT_REJECTED,
};

const MAX_LINE: usize = 32;
static CONTROLLER_IDENTITY: &[u8] = b"GINUX_CONTROLLER_V0";

global_asm!(
    r#"
    .text
    .global _start
_start:
    mov rdi, rbx
    call ginux_controller_main
    ud2
"#
);

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo<'_>) -> ! {
    exit(NATIVE_SERVICE_EXIT_REJECTED)
}

#[no_mangle]
extern "C" fn ginux_controller_main(console: u64) -> ! {
    let identity = unsafe { core::ptr::read_volatile(&CONTROLLER_IDENTITY) };
    if identity != b"GINUX_CONTROLLER_V0"
        || console == 0
        || !native_control_abi_is_compatible(call(NATIVE_CONTROL_DISCOVERY_OPERATION, 0, 0, 0))
    {
        exit(NATIVE_SERVICE_EXIT_REJECTED);
    }
    let mut descriptor = NativeControlDescriptorV4::zeroed();
    if call(
        NATIVE_CONTROL_DESCRIPTOR_OPERATION,
        0,
        (&mut descriptor as *mut NativeControlDescriptorV4) as u64,
        u64::from(NATIVE_CONTROL_DESCRIPTOR_SIZE),
    ) != u64::from(NATIVE_CONTROL_DESCRIPTOR_SIZE)
        || !descriptor.is_compatible()
    {
        exit(NATIVE_SERVICE_EXIT_REJECTED);
    }
    let mut line = [0_u8; MAX_LINE];
    if call(
        NATIVE_CONSOLE_READ_LINE_OPERATION,
        0,
        line.as_mut_ptr() as u64,
        line.len() as u64,
    ) != NATIVE_CALL_FAILED
    {
        exit(NATIVE_SERVICE_EXIT_REJECTED);
    }

    write(console, b"\r\nGinux controller v0 (ring 3)\r\n");
    write(console, b"Type 'help' for commands.\r\n");
    loop {
        write(console, b"ginux> ");
        let length = call(
            NATIVE_CONSOLE_READ_LINE_OPERATION,
            console,
            line.as_mut_ptr() as u64,
            line.len() as u64,
        );
        let Ok(length) = usize::try_from(length) else {
            exit(NATIVE_SERVICE_EXIT_REJECTED);
        };
        let Some(command) = line.get(..length) else {
            exit(NATIVE_SERVICE_EXIT_REJECTED);
        };
        match command {
            b"help" => {
                write(console, b"help        Show this command list\r\n");
                write(console, b"status      Show controller and host state\r\n");
                write(console, b"components  Show embedded image components\r\n");
                write(console, b"shutdown    Shut down Ginux cleanly\r\n");
            }
            b"status" => {
                write(console, b"controller: running (ring 3)\r\n");
                write(console, b"containers: 0 (C1)\r\n");
                write(console, b"integrity: sealed\r\n");
            }
            b"components" => {
                write(console, b"GINUX.EFI component catalog v0:\r\n");
                write(console, b"- kernel: active, ring 0\r\n");
                write(console, b"- controller: active, ring 3\r\n");
                write(console, b"- boot modules: embedded, sealed\r\n");
            }
            b"shutdown" => {
                write(console, b"Shutting down Ginux.\r\n");
                exit(NATIVE_SERVICE_EXIT_COMPLETED);
            }
            b"" => {}
            _ => write(console, b"Unknown command. Type 'help'.\r\n"),
        }
    }
}

fn write(console: u64, bytes: &[u8]) {
    if bytes.is_empty()
        || call(
            NATIVE_CONSOLE_WRITE_OPERATION,
            console,
            bytes.as_ptr() as u64,
            bytes.len() as u64,
        ) != bytes.len() as u64
    {
        exit(NATIVE_SERVICE_EXIT_REJECTED);
    }
}

fn call(number: u64, capability: u64, first: u64, second: u64) -> u64 {
    let result: u64;
    unsafe {
        asm!(
            "int 0x80",
            inlateout("rax") number => result,
            in("rdi") capability,
            in("rsi") first,
            in("rdx") second,
            options(nostack)
        );
    }
    result
}

fn exit(status: u8) -> ! {
    unsafe {
        asm!(
            "int 0x80",
            in("rax") NATIVE_SERVICE_EXIT_OPERATION,
            in("rsi") u64::from(status),
            options(noreturn)
        );
    }
}

const _: () = assert!(NATIVE_CALL_FAILED == u64::MAX - 2);
