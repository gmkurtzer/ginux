#![no_main]
#![no_std]

use core::arch::asm;
use core::ffi::c_void;
use core::panic::PanicInfo;

type EfiHandle = *mut c_void;
type EfiStatus = usize;

const EFI_SUCCESS: EfiStatus = 0;
const EFI_RESET_SHUTDOWN: u32 = 2;
const COM1: u16 = 0x3f8;

#[repr(C)]
struct EfiTableHeader {
    signature: u64,
    revision: u32,
    header_size: u32,
    crc32: u32,
    reserved: u32,
}

#[repr(C)]
struct EfiSimpleTextOutputProtocol {
    reset: usize,
    output_string:
        unsafe extern "efiapi" fn(*mut EfiSimpleTextOutputProtocol, *const u16) -> EfiStatus,
}

#[repr(C)]
struct EfiRuntimeServices {
    header: EfiTableHeader,
    get_time: usize,
    set_time: usize,
    get_wakeup_time: usize,
    set_wakeup_time: usize,
    set_virtual_address_map: usize,
    convert_pointer: usize,
    get_variable: usize,
    get_next_variable_name: usize,
    set_variable: usize,
    get_next_high_monotonic_count: usize,
    reset_system: unsafe extern "efiapi" fn(u32, EfiStatus, usize, *const c_void),
}

#[repr(C)]
struct EfiSystemTable {
    header: EfiTableHeader,
    firmware_vendor: *mut u16,
    firmware_revision: u32,
    console_in_handle: EfiHandle,
    console_in: *mut c_void,
    console_out_handle: EfiHandle,
    console_out: *mut EfiSimpleTextOutputProtocol,
    standard_error_handle: EfiHandle,
    standard_error: *mut EfiSimpleTextOutputProtocol,
    runtime_services: *mut EfiRuntimeServices,
    boot_services: *mut c_void,
    number_of_table_entries: usize,
    configuration_table: *mut c_void,
}

#[no_mangle]
extern "efiapi" fn efi_main(_image: EfiHandle, system_table: *mut EfiSystemTable) -> EfiStatus {
    serial_init();
    serial_write("ginux: UEFI boot entered\r\n");

    // Keep the firmware-console message as a convenience for UTM's graphical view.
    // Serial is the authoritative interface used by automated tests.
    let message = [
        b'G' as u16,
        b'i' as u16,
        b'n' as u16,
        b'u' as u16,
        b'x' as u16,
        b' ' as u16,
        b'b' as u16,
        b'o' as u16,
        b'o' as u16,
        b't' as u16,
        b'e' as u16,
        b'd' as u16,
        b'\r' as u16,
        b'\n' as u16,
        0,
    ];

    if !system_table.is_null() {
        // SAFETY: UEFI supplies a valid system table for the lifetime of this entry call.
        // The protocol and UTF-16 string layouts follow the UEFI ABI.
        unsafe {
            let console = (*system_table).console_out;
            if !console.is_null() {
                let _ = ((*console).output_string)(console, message.as_ptr());
            }
        }
    }

    serial_write("GINUX_BOOT_OK\r\n");

    if !system_table.is_null() {
        // SAFETY: The runtime-services pointer is supplied by UEFI. A shutdown
        // reset gives both interactive and automated runs a clean completion.
        unsafe {
            let runtime = (*system_table).runtime_services;
            if !runtime.is_null() {
                ((*runtime).reset_system)(EFI_RESET_SHUTDOWN, EFI_SUCCESS, 0, core::ptr::null());
            }
        }
    }

    EFI_SUCCESS
}

#[panic_handler]
fn panic(_info: &PanicInfo<'_>) -> ! {
    serial_write("GINUX_BOOT_PANIC\r\n");
    loop {
        core::hint::spin_loop();
    }
}

fn serial_init() {
    // SAFETY: COM1 is part of the pinned QEMU machine profile. These accesses only
    // configure that emulated UART and do not touch memory.
    unsafe {
        outb(COM1 + 1, 0x00);
        outb(COM1 + 3, 0x80);
        outb(COM1, 0x03);
        outb(COM1 + 1, 0x00);
        outb(COM1 + 3, 0x03);
        outb(COM1 + 2, 0xc7);
        outb(COM1 + 4, 0x0b);
    }
}

fn serial_write(message: &str) {
    for byte in message.bytes() {
        while unsafe { inb(COM1 + 5) } & 0x20 == 0 {
            core::hint::spin_loop();
        }

        // SAFETY: COM1 was initialized above and is the pinned serial device.
        unsafe { outb(COM1, byte) };
    }
}

unsafe fn outb(port: u16, value: u8) {
    asm!(
        "out dx, al",
        in("dx") port,
        in("al") value,
        options(nomem, nostack, preserves_flags)
    );
}

unsafe fn inb(port: u16) -> u8 {
    let value: u8;
    asm!(
        "in al, dx",
        in("dx") port,
        out("al") value,
        options(nomem, nostack, preserves_flags)
    );
    value
}
