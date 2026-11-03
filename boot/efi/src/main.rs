#![no_main]
#![no_std]

mod generated_boot;

use core::arch::asm;
use core::ffi::c_void;
use core::panic::PanicInfo;
use generated_boot::{
    BOOT_MANIFEST, BOOT_MANIFEST_SHA256, BOOT_MANIFEST_SIGNATURE, CONTROLLER_RUNTIME,
    CONTROL_PLANE_BUNDLE, ROOT_IMAGE,
};
use ginux_kernel::{
    kernel_entry, rebase_loaded_image, BootInfo, ResetSystem, BOOT_INFO_MAGIC, BOOT_INFO_VERSION,
};

type EfiHandle = *mut c_void;
type EfiStatus = usize;

const EFI_SUCCESS: EfiStatus = 0;
const COM1: u16 = 0x3f8;
const MEMORY_MAP_CAPACITY: usize = 128 * 1024;
const PAGE_SIZE: usize = 4096;
// The kernel runs from this image and its bootstrap page tables identity-map
// only [0, 4 GiB) (`INITIAL_IDENTITY_LIMIT`).
const IDENTITY_LIMIT: u64 = 1 << 32;
const ALLOCATE_MAX_ADDRESS: u32 = 1;
const EFI_LOADER_CODE: u32 = 1;
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
static GRAPHICS_OUTPUT_PROTOCOL_GUID: EfiGuid = EfiGuid {
    data1: 0x9042_a9de,
    data2: 0x23dc,
    data3: 0x4a38,
    data4: [0x96, 0xfb, 0x7a, 0xde, 0xd0, 0x80, 0x51, 0x6a],
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
    test_string: usize,
    query_mode: usize,
    set_mode: usize,
    set_attribute: usize,
    clear_screen: unsafe extern "efiapi" fn(*mut EfiSimpleTextOutputProtocol) -> EfiStatus,
}

#[repr(C)]
struct EfiRuntimeServices {
    header: EfiTableHeader,
    get_time: unsafe extern "efiapi" fn(*mut EfiTime, *mut c_void) -> EfiStatus,
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

#[repr(C)]
struct EfiTime {
    year: u16,
    month: u8,
    day: u8,
    hour: u8,
    minute: u8,
    second: u8,
    pad1: u8,
    nanosecond: u32,
    time_zone: i16,
    daylight: u8,
    pad2: u8,
}

type GetMemoryMap =
    unsafe extern "efiapi" fn(*mut usize, *mut u8, *mut usize, *mut usize, *mut u32) -> EfiStatus;

type AllocatePages = unsafe extern "efiapi" fn(u32, u32, usize, *mut u64) -> EfiStatus;
type EfiMain = extern "efiapi" fn(EfiHandle, *mut EfiSystemTable) -> EfiStatus;

type ExitBootServices = unsafe extern "efiapi" fn(EfiHandle, usize) -> EfiStatus;
type HandleProtocol =
    unsafe extern "efiapi" fn(EfiHandle, *const EfiGuid, *mut *mut c_void) -> EfiStatus;
type LocateProtocol =
    unsafe extern "efiapi" fn(*const EfiGuid, *mut c_void, *mut *mut c_void) -> EfiStatus;
type QueryGraphicsMode = unsafe extern "efiapi" fn(
    *mut EfiGraphicsOutputProtocol,
    u32,
    *mut usize,
    *mut *mut EfiGraphicsOutputModeInformation,
) -> EfiStatus;
type SetGraphicsMode = unsafe extern "efiapi" fn(*mut EfiGraphicsOutputProtocol, u32) -> EfiStatus;
type FreePool = unsafe extern "efiapi" fn(*mut c_void) -> EfiStatus;

#[repr(C)]
struct EfiGraphicsOutputModeInformation {
    version: u32,
    horizontal_resolution: u32,
    vertical_resolution: u32,
    pixel_format: u32,
    pixel_information: [u32; 4],
    pixels_per_scan_line: u32,
}

#[repr(C)]
struct EfiGraphicsOutputProtocolMode {
    max_mode: u32,
    mode: u32,
    info: *const EfiGraphicsOutputModeInformation,
    size_of_info: usize,
    framebuffer_base: u64,
    framebuffer_size: usize,
}

#[repr(C)]
struct EfiGraphicsOutputProtocol {
    query_mode: usize,
    set_mode: usize,
    blt: usize,
    mode: *const EfiGraphicsOutputProtocolMode,
}

#[derive(Clone, Copy)]
struct FramebufferHandoff {
    base: u64,
    size: usize,
    width: u32,
    height: u32,
    stride: u32,
    format: u32,
}

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
    allocate_pages: AllocatePages,
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
    get_next_monotonic_count: usize,
    stall: usize,
    set_watchdog_timer: usize,
    connect_controller: usize,
    disconnect_controller: usize,
    open_protocol: usize,
    close_protocol: usize,
    open_protocol_information: usize,
    protocols_per_handle: usize,
    locate_handle_buffer: usize,
    locate_protocol: LocateProtocol,
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

extern "C" {
    // Linker-defined start of the image this code is currently running from.
    static __ImageBase: u8;
}

static mut MEMORY_MAP_STORAGE: MemoryMapStorage = MemoryMapStorage([0; MEMORY_MAP_CAPACITY]);

#[no_mangle]
extern "efiapi" fn efi_main(image: EfiHandle, system_table: *mut EfiSystemTable) -> EfiStatus {
    serial_init();

    // SAFETY: UEFI supplies a valid system table and boot services are still
    // active. Nothing in this image has been modified yet, so a relocated
    // copy starts from a pristine state and repeats only the steps above.
    if !system_table.is_null() {
        if let Some(status) = unsafe { run_below_identity_limit(image, system_table) } {
            return status;
        }
    }

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
                let _ = ((*console).clear_screen)(console);
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

/// Firmware may load the image at its 5 GiB link base (or anywhere else) when
/// RAM exists there, but the kernel requires its image inside the initial
/// identity map. Copy the image below 4 GiB, reapply its base relocations,
/// and continue in the copy. Returns `None` when no move is needed.
unsafe fn run_below_identity_limit(
    image: EfiHandle,
    system_table: *mut EfiSystemTable,
) -> Option<EfiStatus> {
    let boot_services = (*system_table).boot_services;
    if boot_services.is_null() {
        return None;
    }
    let (image_base, image_size) = loaded_image_bounds(image, boot_services);
    // The relocated copy still sees the firmware's original LoadedImage data.
    if running_image_base() != image_base || image_base as u64 + image_size as u64 <= IDENTITY_LIMIT
    {
        return None;
    }
    let mut target = IDENTITY_LIMIT - 1;
    let status = ((*boot_services).allocate_pages)(
        ALLOCATE_MAX_ADDRESS,
        EFI_LOADER_CODE,
        image_size.div_ceil(PAGE_SIZE),
        &mut target,
    );
    if status != EFI_SUCCESS {
        panic!("no memory below 4 GiB for the EFI image");
    }
    let copy = target as *mut u8;
    core::ptr::copy_nonoverlapping(image_base, copy, image_size);
    let delta = target.wrapping_sub(image_base as u64);
    if rebase_loaded_image(core::slice::from_raw_parts_mut(copy, image_size), delta).is_none() {
        panic!("EFI image base relocations are invalid");
    }
    let entry = efi_main as EfiMain as usize - image_base as usize + copy as usize;
    let entry = core::mem::transmute::<usize, EfiMain>(entry);
    Some(entry(image, system_table))
}

fn running_image_base() -> *const u8 {
    // Taking the address of the linker-defined symbol never reads it.
    core::ptr::addr_of!(__ImageBase)
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
    let (_, image_size) = loaded_image_bounds(image, boot_services);
    let image_base = running_image_base();
    let unix_time = read_unix_time(runtime_services);
    let framebuffer = locate_framebuffer(boot_services);

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
                controller_runtime: CONTROLLER_RUNTIME.as_ptr(),
                controller_runtime_size: CONTROLLER_RUNTIME.len(),
                root_image: ROOT_IMAGE.as_ptr(),
                root_image_size: ROOT_IMAGE.len(),
                framebuffer_base: framebuffer.base,
                framebuffer_size: framebuffer.size,
                framebuffer_width: framebuffer.width,
                framebuffer_height: framebuffer.height,
                framebuffer_stride: framebuffer.stride,
                framebuffer_format: framebuffer.format,
                oci_rootfs: core::ptr::null(),
                oci_rootfs_size: 0,
                unix_time,
                reset_system: (*runtime_services).reset_system,
                image_base,
                image_size,
            };
        }
    }

    panic!("failed to exit UEFI boot services");
}

unsafe fn locate_framebuffer(boot_services: *mut EfiBootServices) -> FramebufferHandoff {
    let mut protocol = core::ptr::null_mut();
    if ((*boot_services).locate_protocol)(
        core::ptr::addr_of!(GRAPHICS_OUTPUT_PROTOCOL_GUID),
        core::ptr::null_mut(),
        &mut protocol,
    ) != EFI_SUCCESS
        || protocol.is_null()
    {
        panic!("UEFI did not provide graphics output");
    }
    let graphics = protocol.cast::<EfiGraphicsOutputProtocol>();
    let initial_mode = (*graphics).mode;
    if !initial_mode.is_null() {
        let query: QueryGraphicsMode = core::mem::transmute((*graphics).query_mode);
        let set: SetGraphicsMode = core::mem::transmute((*graphics).set_mode);
        let free_pool: FreePool = core::mem::transmute((*boot_services).free_pool);
        let mut selected_mode = None;
        let mut selected_score = 0_u64;
        for candidate in 0..(*initial_mode).max_mode {
            let mut size = 0_usize;
            let mut info = core::ptr::null_mut();
            if query(graphics, candidate, &mut size, &mut info) == EFI_SUCCESS && !info.is_null() {
                let width = (*info).horizontal_resolution;
                let height = (*info).vertical_resolution;
                let format = (*info).pixel_format;
                let supported = format <= 1 && width <= 1024 && height <= 768;
                // Prefer the native 1024 by 768 target, BGR byte order (which is the
                // controller renderer's in-memory 0x00RRGGBB layout), then the
                // largest supported fallback.  The score keeps mode selection
                // deterministic regardless of firmware enumeration order.
                let exact = width == 1024 && height == 768;
                let score = u64::from(exact) << 63
                    | u64::from(format == 1) << 62
                    | (u64::from(width) * u64::from(height));
                if supported && score > selected_score {
                    selected_mode = Some(candidate);
                    selected_score = score;
                }
                let _ = free_pool(info.cast());
            }
        }
        if let Some(selected_mode) = selected_mode {
            let _ = set(graphics, selected_mode);
        }
    }
    let mode = (*graphics).mode;
    if mode.is_null() || (*mode).info.is_null() {
        panic!("UEFI graphics output mode is unavailable");
    }
    let info = &*(*mode).info;
    let handoff = FramebufferHandoff {
        base: (*mode).framebuffer_base,
        size: (*mode).framebuffer_size,
        width: info.horizontal_resolution,
        height: info.vertical_resolution,
        stride: info.pixels_per_scan_line,
        format: info.pixel_format,
    };
    if handoff.base == 0
        || handoff.size == 0
        || handoff.width == 0
        || handoff.height == 0
        || handoff.stride < handoff.width
        || handoff.format > 1
    {
        panic!("UEFI graphics output mode is unsupported");
    }
    handoff
}

unsafe fn read_unix_time(runtime_services: *mut EfiRuntimeServices) -> u64 {
    let mut time = EfiTime {
        year: 0,
        month: 0,
        day: 0,
        hour: 0,
        minute: 0,
        second: 0,
        pad1: 0,
        nanosecond: 0,
        time_zone: 0,
        daylight: 0,
        pad2: 0,
    };
    if ((*runtime_services).get_time)(&mut time, core::ptr::null_mut()) != EFI_SUCCESS
        || !(1970..=9999).contains(&time.year)
        || !(1..=12).contains(&time.month)
        || !(1..=31).contains(&time.day)
        || time.hour > 23
        || time.minute > 59
        || time.second > 60
    {
        panic!("UEFI wall clock is unavailable");
    }
    let mut days = 0_u64;
    for year in 1970..time.year {
        days += if is_leap_year(year) { 366 } else { 365 };
    }
    const MONTH_DAYS: [u8; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    for month in 1..time.month {
        days += u64::from(MONTH_DAYS[usize::from(month - 1)]);
        if month == 2 && is_leap_year(time.year) {
            days += 1;
        }
    }
    days += u64::from(time.day - 1);
    days * 86_400
        + u64::from(time.hour) * 3_600
        + u64::from(time.minute) * 60
        + u64::from(time.second.min(59))
}

const fn is_leap_year(year: u16) -> bool {
    year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400))
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
fn panic(info: &PanicInfo<'_>) -> ! {
    serial_write("GINUX_BOOT_PANIC\r\n");
    if let Some(location) = info.location() {
        serial_write(location.file());
        serial_write(":");
        serial_write_u64(u64::from(location.line()));
        serial_write(":");
        serial_write_u64(u64::from(location.column()));
        serial_write("\r\n");
    }
    loop {
        core::hint::spin_loop();
    }
}

fn serial_write_u64(mut value: u64) {
    let mut digits = [0_u8; 20];
    let mut index = digits.len();
    loop {
        index -= 1;
        digits[index] = b'0' + (value % 10) as u8;
        value /= 10;
        if value == 0 {
            break;
        }
    }
    for byte in &digits[index..] {
        while unsafe { inb(COM1 + 5) } & 0x20 == 0 {
            core::hint::spin_loop();
        }
        unsafe { outb(COM1, *byte) };
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
