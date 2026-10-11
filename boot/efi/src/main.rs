#![no_main]
#![no_std]

mod generated_boot;

use core::arch::asm;
use core::ffi::c_void;
use core::panic::PanicInfo;
use generated_boot::{
    BOOT_MANIFEST, BOOT_MANIFEST_SHA256, BOOT_MANIFEST_SIGNATURE, CONTROL_PLANE_BUNDLE, ROOT_IMAGE,
};
use ginux_kernel::{kernel_entry, BootInfo, ResetSystem, BOOT_INFO_MAGIC, BOOT_INFO_VERSION};

type EfiHandle = *mut c_void;
type EfiStatus = usize;

const EFI_SUCCESS: EfiStatus = 0;
const COM1: u16 = 0x3f8;
const MEMORY_MAP_CAPACITY: usize = 128 * 1024;
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
static LOADED_IMAGE_PROTOCOL_GUID: EfiGuid = EfiGuid {
    data1: 0x5b1b_31a1,
    data2: 0x9562,
    data3: 0x11d2,
    data4: [0x8e, 0x3f, 0x00, 0xa0, 0xc9, 0x69, 0x72, 0x3b],
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
type HandleProtocol =
    unsafe extern "efiapi" fn(EfiHandle, *const EfiGuid, *mut *mut c_void) -> EfiStatus;

#[repr(C)]
struct EfiLoadedImageProtocol {
    revision: u32,
    parent_handle: EfiHandle,
    system_table: *mut EfiSystemTable,
    device_handle: EfiHandle,
    file_path: *mut c_void,
    reserved: *mut c_void,
    load_options_size: u32,
    load_options: *mut c_void,
    image_base: *const u8,
    image_size: u64,
    image_code_type: u32,
    image_data_type: u32,
    unload: usize,
}

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
    handle_protocol: HandleProtocol,
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
    let (image_base, image_size) = loaded_image_bounds(image, boot_services);

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
                manifest_signature: BOOT_MANIFEST_SIGNATURE,
                control_plane_bundle: CONTROL_PLANE_BUNDLE.as_ptr(),
                control_plane_bundle_size: CONTROL_PLANE_BUNDLE.len(),
                root_image: ROOT_IMAGE.as_ptr(),
                root_image_size: ROOT_IMAGE.len(),
                reset_system: (*runtime_services).reset_system,
                image_base,
                image_size,
            };
        }
    }

    panic!("failed to exit UEFI boot services");
}

unsafe fn loaded_image_bounds(
    image: EfiHandle,
    boot_services: *mut EfiBootServices,
) -> (*const u8, usize) {
    let mut interface = core::ptr::null_mut();
    let status = ((*boot_services).handle_protocol)(
        image,
        core::ptr::addr_of!(LOADED_IMAGE_PROTOCOL_GUID),
        &mut interface,
    );
    if status != EFI_SUCCESS || interface.is_null() {
        panic!("UEFI loaded-image protocol is unavailable");
    }
    let loaded = &*interface.cast::<EfiLoadedImageProtocol>();
    let Ok(image_size) = usize::try_from(loaded.image_size) else {
        panic!("loaded EFI image is too large");
    };
    if loaded.image_base.is_null()
        || image_size == 0
        || (loaded.image_base as usize)
            .checked_add(image_size)
            .is_none()
    {
        panic!("loaded EFI image bounds are invalid");
    }
    (loaded.image_base, image_size)
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
