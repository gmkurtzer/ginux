#![no_std]

use core::arch::asm;
use core::ffi::c_void;

const COM1: u16 = 0x3f8;
const EFI_RESET_SHUTDOWN: u32 = 2;
const EFI_SUCCESS: usize = 0;

pub type ResetSystem = unsafe extern "efiapi" fn(u32, usize, usize, *const c_void);

/// Information fixed by the boot environment before firmware boot services end.
///
/// This structure is a version-zero internal contract. It uses an explicit C
/// layout so the boot/kernel boundary can evolve into a language-neutral ABI.
#[repr(C)]
pub struct BootInfo {
    pub memory_map: *const u8,
    pub memory_map_size: usize,
    pub memory_descriptor_size: usize,
    pub memory_descriptor_version: u32,
    pub reset_system: ResetSystem,
}

/// Takes control after UEFI boot services have ended.
///
/// # Safety
///
/// `boot_info` must identify a valid, immutable `BootInfo`. Its memory-map
/// buffer must remain allocated for the lifetime of the kernel.
pub unsafe extern "sysv64" fn kernel_entry(boot_info: *const BootInfo) -> ! {
    serial_init();
    serial_write("ginux: kernel entered after ExitBootServices\r\n");

    if boot_info.is_null() {
        kernel_fail("ginux: missing boot information\r\n");
    }

    let boot_info = &*boot_info;
    if boot_info.memory_map.is_null()
        || boot_info.memory_map_size == 0
        || boot_info.memory_descriptor_size == 0
        || !boot_info
            .memory_map_size
            .is_multiple_of(boot_info.memory_descriptor_size)
    {
        kernel_fail("ginux: invalid UEFI memory map\r\n");
    }

    let descriptor_count = boot_info.memory_map_size / boot_info.memory_descriptor_size;
    serial_write("ginux: memory descriptors: ");
    serial_write_usize(descriptor_count);
    serial_write("\r\nGINUX_KERNEL_OK\r\n");

    (boot_info.reset_system)(EFI_RESET_SHUTDOWN, EFI_SUCCESS, 0, core::ptr::null());
    kernel_fail("ginux: firmware shutdown returned unexpectedly\r\n");
}

fn kernel_fail(message: &str) -> ! {
    serial_write(message);
    serial_write("GINUX_KERNEL_PANIC\r\n");
    loop {
        core::hint::spin_loop();
    }
}

fn serial_init() {
    // SAFETY: COM1 is part of the pinned QEMU machine profile.
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

fn serial_write_usize(mut value: usize) {
    let mut digits = [0_u8; 20];
    let mut cursor = digits.len();

    if value == 0 {
        serial_write("0");
        return;
    }

    while value != 0 {
        cursor -= 1;
        digits[cursor] = b'0' + (value % 10) as u8;
        value /= 10;
    }

    for digit in &digits[cursor..] {
        // The array contains only ASCII decimal digits.
        let text = [*digit];
        if let Ok(text) = core::str::from_utf8(&text) {
            serial_write(text);
        }
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
