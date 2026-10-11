#![no_main]
#![no_std]

use core::arch::asm;
use core::ffi::c_void;
use core::panic::PanicInfo;
use ginux_kernel::{kernel_entry, BootInfo, ResetSystem, BOOT_INFO_MAGIC, BOOT_INFO_VERSION};

type EfiHandle = *mut c_void;
type EfiStatus = usize;

const EFI_SUCCESS: EfiStatus = 0;
const COM1: u16 = 0x3f8;
const MEMORY_MAP_CAPACITY: usize = 128 * 1024;
const BOOT_MANIFEST_SHA256: [u8; 32] = [
    0x95, 0x7b, 0x2f, 0x01, 0xc7, 0x10, 0x4b, 0xb2, 0xbb, 0x43, 0x95, 0xb0, 0x1b, 0x9d, 0xa3, 0x59,
    0x4f, 0x84, 0xcc, 0x75, 0xdb, 0xe6, 0x41, 0x17, 0x75, 0x84, 0x0a, 0x7d, 0xd4, 0xfa, 0x0f, 0x86,
];
static CONTROL_PLANE_BUNDLE: [u8; 23] = [
    0xb8, 0x01, 0x00, 0x00, 0x00, 0xcd, 0x80, 0x3d, 0x58, 0x47, 0x00, 0x00, 0x75, 0x07, 0xb8, 0x02,
    0x00, 0x00, 0x00, 0xcd, 0x80, 0x0f, 0x0b,
];
static BOOT_MANIFEST: [u8; 104] = [
    0x47, 0x49, 0x4e, 0x58, 0x4d, 0x4e, 0x46, 0x00, 0x02, 0x00, 0x48, 0x00, 0x68, 0x00, 0x00, 0x00,
    0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x17, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x92, 0x77, 0x51, 0x14, 0x03, 0xff, 0xff, 0x74,
    0x5c, 0xf8, 0x32, 0x46, 0xba, 0x59, 0xa9, 0xd5, 0xfa, 0x0f, 0x42, 0x5e, 0x81, 0x2b, 0xe1, 0x0a,
    0x61, 0xc5, 0xba, 0x28, 0x85, 0x2b, 0x3c, 0x29, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x07, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];
const ACPI_20_TABLE_GUID: EfiGuid = EfiGuid {
    data1: 0x8868_e871,
    data2: 0xe4f1,
    data3: 0x11d3,
    data4: [0xbc, 0x22, 0x00, 0x80, 0xc7, 0x3c, 0x88, 0x81],
};
const ACPI_10_TABLE_GUID: EfiGuid = EfiGuid {
    data1: 0xeb9d_2d30,
    data2: 0x2d88,
    data3: 0x11d3,
    data4: [0x9a, 0x16, 0x00, 0x90, 0x27, 0x3f, 0xc1, 0x4d],
};

#[derive(Clone, Copy, Eq, PartialEq)]
#[repr(C)]
struct EfiGuid {
    data1: u32,
    data2: u16,
    data3: u16,
    data4: [u8; 8],
}

#[repr(C)]
struct EfiConfigurationTable {
    vendor_guid: EfiGuid,
    vendor_table: *const u8,
}

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
    configuration_table: *const EfiConfigurationTable,
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
    let acpi_rsdp = unsafe { find_acpi_rsdp(system_table) };
    if acpi_rsdp.is_null() {
        panic!("UEFI did not provide an ACPI root pointer");
    }
    let boot_info = unsafe { exit_boot_services(image, system_table, acpi_rsdp) };

    // SAFETY: Boot services have ended, the memory-map storage is static, and
    // the kernel entry never returns to firmware boot code.
    unsafe { kernel_entry(core::ptr::addr_of!(boot_info)) }
}

unsafe fn find_acpi_rsdp(system_table: *const EfiSystemTable) -> *const u8 {
    let table_count = (*system_table).number_of_table_entries;
    let tables = (*system_table).configuration_table;
    if tables.is_null() {
        return core::ptr::null();
    }

    let mut acpi_v1 = core::ptr::null();
    for index in 0..table_count {
        let table = &*tables.add(index);
        if table.vendor_guid == ACPI_20_TABLE_GUID {
            return table.vendor_table;
        }
        if table.vendor_guid == ACPI_10_TABLE_GUID {
            acpi_v1 = table.vendor_table;
        }
    }
    acpi_v1
}

unsafe fn exit_boot_services(
    image: EfiHandle,
    system_table: *mut EfiSystemTable,
    acpi_rsdp: *const u8,
) -> BootInfo {
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
                magic: BOOT_INFO_MAGIC,
                version: BOOT_INFO_VERSION,
                size: core::mem::size_of::<BootInfo>() as u32,
                memory_map,
                memory_map_size,
                memory_descriptor_size: descriptor_size,
                memory_descriptor_version: descriptor_version,
                acpi_rsdp,
                manifest: BOOT_MANIFEST.as_ptr(),
                manifest_size: BOOT_MANIFEST.len(),
                manifest_sha256: BOOT_MANIFEST_SHA256,
                control_plane_bundle: CONTROL_PLANE_BUNDLE.as_ptr(),
                control_plane_bundle_size: CONTROL_PLANE_BUNDLE.len(),
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
