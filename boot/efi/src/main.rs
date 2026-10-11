#![no_main]
#![no_std]

use core::arch::asm;
use core::ffi::c_void;
use core::panic::PanicInfo;
use ginux_kernel::{kernel_entry, BootInfo, ResetSystem};

type EfiHandle = *mut c_void;
type EfiStatus = usize;

const EFI_SUCCESS: EfiStatus = 0;
const COM1: u16 = 0x3f8;
const MEMORY_MAP_CAPACITY: usize = 128 * 1024;

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
    reset_system: ResetSystem,
}

type GetMemoryMap =
    unsafe extern "efiapi" fn(*mut usize, *mut u8, *mut usize, *mut usize, *mut u32) -> EfiStatus;

type ExitBootServices = unsafe extern "efiapi" fn(EfiHandle, usize) -> EfiStatus;

#[repr(C)]
struct EfiBootServices {
    header: EfiTableHeader,
    raise_tpl: usize,
    restore_tpl: usize,
    allocate_pages: usize,
    free_pages: usize,
    get_memory_map: GetMemoryMap,
    allocate_pool: usize,
    free_pool: usize,
    create_event: usize,
    set_timer: usize,
    wait_for_event: usize,
    signal_event: usize,
    close_event: usize,
    check_event: usize,
    install_protocol_interface: usize,
    reinstall_protocol_interface: usize,
    uninstall_protocol_interface: usize,
    handle_protocol: usize,
    reserved: usize,
    register_protocol_notify: usize,
    locate_handle: usize,
    locate_device_path: usize,
    install_configuration_table: usize,
    load_image: usize,
    start_image: usize,
    exit: usize,
    unload_image: usize,
    exit_boot_services: ExitBootServices,
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
    boot_services: *mut EfiBootServices,
    number_of_table_entries: usize,
    configuration_table: *mut c_void,
}

#[repr(C, align(16))]
struct MemoryMapStorage([u8; MEMORY_MAP_CAPACITY]);

static mut MEMORY_MAP_STORAGE: MemoryMapStorage = MemoryMapStorage([0; MEMORY_MAP_CAPACITY]);

#[no_mangle]
extern "efiapi" fn efi_main(image: EfiHandle, system_table: *mut EfiSystemTable) -> EfiStatus {
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

    if system_table.is_null() {
        panic!("UEFI did not provide a system table");
    }

    // SAFETY: UEFI owns the system and service tables until ExitBootServices.
    // The handoff function validates the required pointers before using them.
    let boot_info = unsafe { exit_boot_services(image, system_table) };

    // SAFETY: Boot services have ended, the memory-map storage is static, and
    // the kernel entry never returns to firmware boot code.
    unsafe { kernel_entry(core::ptr::addr_of!(boot_info)) }
}

unsafe fn exit_boot_services(image: EfiHandle, system_table: *mut EfiSystemTable) -> BootInfo {
    let boot_services = (*system_table).boot_services;
    let runtime_services = (*system_table).runtime_services;
    if boot_services.is_null() || runtime_services.is_null() {
        panic!("UEFI service table is missing");
    }

    let memory_map = core::ptr::addr_of_mut!(MEMORY_MAP_STORAGE.0).cast::<u8>();

    // ExitBootServices may reject a stale map key if firmware changes its map
    // between the two calls. Reacquire once without invoking any other firmware
    // service, as required by the UEFI handoff protocol.
    for _ in 0..2 {
        let mut memory_map_size = MEMORY_MAP_CAPACITY;
        let mut map_key = 0;
        let mut descriptor_size = 0;
        let mut descriptor_version = 0;

        let status = ((*boot_services).get_memory_map)(
            &mut memory_map_size,
            memory_map,
            &mut map_key,
            &mut descriptor_size,
            &mut descriptor_version,
        );
        if status != EFI_SUCCESS {
            panic!("failed to obtain UEFI memory map");
        }

        let status = ((*boot_services).exit_boot_services)(image, map_key);
        if status == EFI_SUCCESS {
            return BootInfo {
                memory_map,
                memory_map_size,
                memory_descriptor_size: descriptor_size,
                memory_descriptor_version: descriptor_version,
                reset_system: (*runtime_services).reset_system,
            };
        }
    }

    panic!("failed to exit UEFI boot services");
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
