#![no_std]
#![no_main]

use core::arch::{asm, global_asm};

use ginux_native_abi::{
    native_control_abi_is_compatible, NativeControlDescriptorV4, NativeDisplayRequestV0,
    NativeGraphicsBootV0, NativeGraphicsBootV1, NativeInputEventV0, NATIVE_CALL_FAILED,
    NATIVE_CONSOLE_WRITE_OPERATION, NATIVE_CONTROL_DESCRIPTOR_OPERATION,
    NATIVE_CONTROL_DESCRIPTOR_SIZE, NATIVE_CONTROL_DISCOVERY_OPERATION, NATIVE_DISPLAY_OPERATION,
    NATIVE_DISPLAY_QUERY, NATIVE_DISPLAY_RESET, NATIVE_DISPLAY_WRITE_PIXEL,
    NATIVE_GRAPHICS_BOOT_V1_VERSION, NATIVE_GRAPHICS_COMMAND_COMPLETE,
    NATIVE_GRAPHICS_COMMAND_FAULT, NATIVE_GRAPHICS_COMMAND_VALIDATE_DEVICES, NATIVE_INPUT_KIND_KEY,
    NATIVE_INPUT_KIND_POINTER_BUTTON, NATIVE_INPUT_KIND_POINTER_MOTION, NATIVE_INPUT_OPERATION,
    NATIVE_INPUT_READ, NATIVE_INPUT_RESET, NATIVE_SERVICE_EXIT_COMPLETED,
    NATIVE_SERVICE_EXIT_OPERATION,
};

const READY: &[u8] = b"ginux: graphical controller service ready\r\n";
const PROTOCOL_READY: &[u8] = b"ginux: native display protocol conformance validated in ring 3\r\n";
const SEAT_READY: &[u8] = b"ginux: controller seat focus and leases validated in ring 3\r\n";
const WINDOWING_READY: &[u8] = b"ginux: native windowing and toolkit validated in ring 3\r\n";
const CONTROLLER_READY: &[u8] = b"ginux: graphical controller workflows validated in ring 3\r\n";
const DOMAIN_BRIDGE_READY: &[u8] = b"ginux: isolated domain display bridge validated in ring 3\r\n";
const WAYLAND_READY: &[u8] = b"ginux: Linux domain Wayland environment validated in ring 3\r\n";
const PRESENTATION_READY: &[u8] = b"ginux: domain desktop presentation validated in ring 3\r\n";
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
    let valid_v1 = info.version == NATIVE_GRAPHICS_BOOT_V1_VERSION
        && unsafe {
            (info as *const NativeGraphicsBootV0 as *const NativeGraphicsBootV1)
                .as_ref()
                .is_some_and(NativeGraphicsBootV1::is_valid)
        };
    if !info.is_valid() && !valid_v1 {
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
        NATIVE_GRAPHICS_COMMAND_VALIDATE_DEVICES
            if info.version == NATIVE_GRAPHICS_BOOT_V1_VERSION =>
        {
            let Some(info) = (unsafe {
                (info as *const NativeGraphicsBootV0 as *const NativeGraphicsBootV1).as_ref()
            }) else {
                fail();
            };
            if !info.is_valid()
                || !validate_devices(info)
                || !ginux_display_protocol::contract_probe()
                || !ginux_display_protocol::seat_contract_probe()
                || !ginux_windowing::contract_probe()
                || !ginux_controller_ui::contract_probe()
                || !ginux_domain_display::contract_probe()
                || !ginux_wayland_domain::contract_probe()
                || !ginux_domain_presentation::contract_probe()
                || call(
                    NATIVE_CONSOLE_WRITE_OPERATION,
                    info.v0.console_capability,
                    PROTOCOL_READY.as_ptr() as u64,
                    PROTOCOL_READY.len() as u64,
                ) != PROTOCOL_READY.len() as u64
            {
                fail();
            }
            if call(
                NATIVE_CONSOLE_WRITE_OPERATION,
                info.v0.console_capability,
                SEAT_READY.as_ptr() as u64,
                SEAT_READY.len() as u64,
            ) != SEAT_READY.len() as u64
            {
                fail();
            }
            if call(
                NATIVE_CONSOLE_WRITE_OPERATION,
                info.v0.console_capability,
                DOMAIN_BRIDGE_READY.as_ptr() as u64,
                DOMAIN_BRIDGE_READY.len() as u64,
            ) != DOMAIN_BRIDGE_READY.len() as u64
            {
                fail();
            }
            if call(
                NATIVE_CONSOLE_WRITE_OPERATION,
                info.v0.console_capability,
                WAYLAND_READY.as_ptr() as u64,
                WAYLAND_READY.len() as u64,
            ) != WAYLAND_READY.len() as u64
            {
                fail();
            }
            if call(
                NATIVE_CONSOLE_WRITE_OPERATION,
                info.v0.console_capability,
                PRESENTATION_READY.as_ptr() as u64,
                PRESENTATION_READY.len() as u64,
            ) != PRESENTATION_READY.len() as u64
            {
                fail();
            }
            if call(
                NATIVE_CONSOLE_WRITE_OPERATION,
                info.v0.console_capability,
                CONTROLLER_READY.as_ptr() as u64,
                CONTROLLER_READY.len() as u64,
            ) != CONTROLLER_READY.len() as u64
            {
                fail();
            }
            if call(
                NATIVE_CONSOLE_WRITE_OPERATION,
                info.v0.console_capability,
                WINDOWING_READY.as_ptr() as u64,
                WINDOWING_READY.len() as u64,
            ) != WINDOWING_READY.len() as u64
            {
                fail();
            }
            exit(NATIVE_SERVICE_EXIT_COMPLETED)
        }
        _ => fail(),
    }
}

fn validate_devices(info: &NativeGraphicsBootV1) -> bool {
    let mut display = NativeDisplayRequestV0 {
        size: NativeDisplayRequestV0::SIZE,
        version: 0,
        command: NATIVE_DISPLAY_QUERY,
        x: 0,
        y: 0,
        color: 0,
        width: 0,
        height: 0,
        stride: 0,
        format: 0,
        reserved: 0,
    };
    if call(
        NATIVE_DISPLAY_OPERATION,
        info.v0.console_capability,
        (&mut display as *mut NativeDisplayRequestV0) as u64,
        0,
    ) != NATIVE_CALL_FAILED
        || call(
            NATIVE_DISPLAY_OPERATION,
            info.display_capability,
            (&mut display as *mut NativeDisplayRequestV0) as u64,
            0,
        ) != u64::from(NativeDisplayRequestV0::SIZE)
        || display.width == 0
        || display.height == 0
        || display.stride < display.width
        || display.format > 1
    {
        return false;
    }
    display.command = NATIVE_DISPLAY_WRITE_PIXEL;
    display.x = 0;
    display.y = 0;
    display.color = 0x0014_3658;
    if call(
        NATIVE_DISPLAY_OPERATION,
        info.display_capability,
        (&mut display as *mut NativeDisplayRequestV0) as u64,
        0,
    ) != u64::from(NativeDisplayRequestV0::SIZE)
    {
        return false;
    }
    display.command = NATIVE_DISPLAY_RESET;
    if call(
        NATIVE_DISPLAY_OPERATION,
        info.display_capability,
        (&mut display as *mut NativeDisplayRequestV0) as u64,
        0,
    ) != u64::from(NativeDisplayRequestV0::SIZE)
    {
        return false;
    }

    let mut event = NativeInputEventV0::EMPTY;
    if call(
        NATIVE_INPUT_OPERATION,
        info.display_capability,
        (&mut event as *mut NativeInputEventV0) as u64,
        u64::from(NATIVE_INPUT_READ),
    ) != NATIVE_CALL_FAILED
    {
        return false;
    }
    for (index, expected_kind) in [
        NATIVE_INPUT_KIND_KEY,
        NATIVE_INPUT_KIND_POINTER_MOTION,
        NATIVE_INPUT_KIND_POINTER_BUTTON,
    ]
    .into_iter()
    .enumerate()
    {
        if call(
            NATIVE_INPUT_OPERATION,
            info.input_capability,
            (&mut event as *mut NativeInputEventV0) as u64,
            u64::from(NATIVE_INPUT_READ),
        ) != u64::from(NativeInputEventV0::SIZE)
            || event.kind != expected_kind
            || event.sequence != index as u64 + 1
        {
            return false;
        }
    }
    call(
        NATIVE_INPUT_OPERATION,
        info.input_capability,
        0,
        u64::from(NATIVE_INPUT_RESET),
    ) == 0
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
