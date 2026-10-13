#![no_std]

mod address_space;
mod arch;
mod capability;
mod control_abi;
mod crypto;
mod domain;
mod executable;
mod initramfs;
mod ipc;
mod linux_elf;
mod linux_mount;
mod linux_process;
mod linux_tmpfs;
mod linux_user;
mod manifest;
mod memory;
mod native_elf;
mod network;
mod packet;
mod pe;
mod scheduler;
mod volume;

use core::arch::{asm, global_asm};
use core::ffi::c_void;
use core::mem::size_of;

const COM1: u16 = 0x3f8;
const EFI_RESET_SHUTDOWN: u32 = 2;
const EFI_SUCCESS: usize = 0;
pub(crate) const EFI_LOADER_CODE: u32 = 1;
const EFI_LOADER_DATA: u32 = 2;
const EFI_BOOT_SERVICES_CODE: u32 = 3;
const EFI_BOOT_SERVICES_DATA: u32 = 4;
pub(crate) const EFI_RUNTIME_SERVICES_CODE: u32 = 5;
const EFI_CONVENTIONAL_MEMORY: u32 = 7;
const UEFI_PAGES_PER_MIB: u64 = 256;
const PAGE_SIZE: usize = 4096;
const Q35_IOMMU_REGISTER_BASE: u64 = 0xfed9_0000;
const KERNEL_STACK_SIZE: usize = 128 * 1024;
const KERNEL_STACK_STORAGE_SIZE: usize = PAGE_SIZE + KERNEL_STACK_SIZE + PAGE_SIZE;
const KERNEL_STACK_TOP_OFFSET: usize = PAGE_SIZE + KERNEL_STACK_SIZE;
pub const BOOT_INFO_MAGIC: u64 = 0x0049_4258_554e_4947;
pub const BOOT_INFO_VERSION: u32 = 4;

#[repr(C, align(4096))]
struct KernelStack([u8; KERNEL_STACK_STORAGE_SIZE]);

static mut KERNEL_STACK: KernelStack = KernelStack([0; KERNEL_STACK_STORAGE_SIZE]);

global_asm!(
    r#"
    .text
    .global ginux_kernel_stack_entry
ginux_kernel_stack_entry:
    cli
    lea rsp, [rip + {stack}]
    add rsp, {stack_size}
    and rsp, -16
    call {kernel_main}
    ud2
"#,
    stack = sym KERNEL_STACK,
    stack_size = const KERNEL_STACK_TOP_OFFSET,
    kernel_main = sym kernel_main,
);

extern "sysv64" {
    fn ginux_kernel_stack_entry(boot_info: *const BootInfo) -> !;
}

pub type ResetSystem = unsafe extern "efiapi" fn(u32, usize, usize, *const c_void);

/// Information fixed by the boot environment before firmware boot services end.
///
/// This structure is a versioned internal contract. It uses an explicit C
/// layout so the boot/kernel boundary can evolve into a language-neutral ABI.
#[repr(C)]
pub struct BootInfo {
    pub magic: u64,
    pub version: u32,
    pub size: u32,
    pub memory_map: *const u8,
    pub memory_map_size: usize,
    pub memory_descriptor_size: usize,
    pub memory_descriptor_version: u32,
    pub acpi_rsdp: *const u8,
    pub manifest: *const u8,
    pub manifest_size: usize,
    pub manifest_sha256: [u8; 32],
    pub manifest_signature: [u8; 64],
    pub control_plane_bundle: *const u8,
    pub control_plane_bundle_size: usize,
    pub root_image: *const u8,
    pub root_image_size: usize,
    pub reset_system: ResetSystem,
    pub image_base: *const u8,
    pub image_size: usize,
}

#[repr(C)]
pub(crate) struct UefiMemoryDescriptor {
    pub(crate) memory_type: u32,
    padding: u32,
    pub(crate) physical_start: u64,
    virtual_start: u64,
    pub(crate) page_count: u64,
    attributes: u64,
}

struct MemorySummary {
    descriptors: usize,
    conventional_pages: u64,
    boot_service_pages: u64,
    loader_pages: u64,
}

/// Takes control after UEFI boot services have ended.
///
/// # Safety
///
/// `boot_info` must identify a valid, immutable `BootInfo`. Its memory-map
/// buffer must remain allocated for the lifetime of the kernel.
pub unsafe extern "sysv64" fn kernel_entry(boot_info: *const BootInfo) -> ! {
    ginux_kernel_stack_entry(boot_info)
}

unsafe extern "sysv64" fn kernel_main(boot_info: *const BootInfo) -> ! {
    serial_init();
    serial_write("ginux: kernel entered after ExitBootServices\r\n");

    if !kernel_stack_is_active() {
        kernel_fail("ginux: kernel stack transition failed\r\n");
    }
    serial_write("ginux: kernel stack active\r\n");

    if boot_info.is_null() {
        kernel_fail("ginux: missing boot information\r\n");
    }

    let boot_info = &*boot_info;
    if boot_info.magic != BOOT_INFO_MAGIC
        || boot_info.version != BOOT_INFO_VERSION
        || boot_info.size < size_of::<BootInfo>() as u32
    {
        kernel_fail("ginux: incompatible boot information\r\n");
    }
    if boot_info.memory_map.is_null()
        || boot_info.memory_map_size == 0
        || boot_info.memory_descriptor_size < size_of::<UefiMemoryDescriptor>()
        || !boot_info
            .memory_map_size
            .is_multiple_of(boot_info.memory_descriptor_size)
    {
        kernel_fail("ginux: invalid UEFI memory map\r\n");
    }

    let Some(acpi_rsdp_end) = validate_acpi_rsdp(boot_info.acpi_rsdp) else {
        kernel_fail("ginux: invalid ACPI root pointer\r\n");
    };
    let Some(image_end) = boot_info
        .image_size
        .checked_sub(1)
        .and_then(|size| (boot_info.image_base as usize).checked_add(size))
    else {
        kernel_fail("ginux: invalid loaded EFI image bounds\r\n");
    };
    if boot_info.image_base.is_null() || image_end >= arch::x86_64::paging::INITIAL_IDENTITY_LIMIT {
        kernel_fail("ginux: loaded EFI image is outside the identity map\r\n");
    }
    let Some(image) = pe::inspect(boot_info.image_base, boot_info.image_size) else {
        kernel_fail("ginux: invalid loaded EFI PE image\r\n");
    };
    serial_write("ginux: versioned boot handoff accepted\r\n");
    serial_write("ginux: loaded EFI PE image accepted\r\nginux: PE sections: ");
    serial_write_usize(image.sections);
    serial_write("\r\nginux: executable PE sections: ");
    serial_write_usize(image.executable_sections);
    serial_write("\r\nginux: writable PE sections: ");
    serial_write_usize(image.writable_sections);
    serial_write("\r\n");
    serial_write("ginux: ACPI root pointer validated\r\n");
    let Some(boot_manifest) = manifest::Manifest::parse_and_verify(
        boot_info.manifest,
        boot_info.manifest_size,
        &boot_info.manifest_sha256,
        &boot_info.manifest_signature,
        arch::x86_64::paging::INITIAL_IDENTITY_LIMIT,
    ) else {
        kernel_fail("ginux: invalid measured boot manifest\r\n");
    };
    serial_write("ginux: signed boot manifest accepted\r\nginux: manifest sequence: ");
    serial_write_u64(boot_manifest.sequence());
    serial_write("\r\nginux: manifest authority grants: ");
    serial_write_usize(boot_manifest.grant_count());
    serial_write("\r\n");
    if !manifest::verify_signature_rejection(
        boot_info.manifest,
        boot_info.manifest_size,
        &boot_info.manifest_signature,
    ) {
        kernel_fail("ginux: invalid manifest signature was accepted\r\n");
    }
    serial_write("ginux: manifest signature rejection verified\r\n");
    if !manifest::verify_parser_rejections(boot_info.manifest, boot_info.manifest_size) {
        kernel_fail("ginux: malformed manifest parser test failed\r\n");
    }
    serial_write("ginux: malformed boot manifests rejected\r\n");
    if !boot_manifest.bundle_matches(
        boot_info.control_plane_bundle,
        boot_info.control_plane_bundle_size,
        arch::x86_64::paging::INITIAL_IDENTITY_LIMIT,
    ) {
        kernel_fail("ginux: control-plane bundle measurement failed\r\n");
    }
    serial_write("ginux: control-plane bundle measurement accepted\r\n");
    if !boot_manifest.root_matches(
        boot_info.root_image,
        boot_info.root_image_size,
        arch::x86_64::paging::INITIAL_IDENTITY_LIMIT,
    ) {
        kernel_fail("ginux: root-image measurement failed\r\n");
    }
    serial_write("ginux: root-image measurement accepted\r\n");
    if !boot_manifest.verify_root_rejection(
        boot_info.root_image,
        boot_info.root_image_size,
        arch::x86_64::paging::INITIAL_IDENTITY_LIMIT,
    ) {
        kernel_fail("ginux: modified root-image measurement accepted\r\n");
    }
    serial_write("ginux: root-image measurement rejection verified\r\n");

    let Some(virtio_block) = arch::x86_64::pci::discover_virtio_block() else {
        kernel_fail("ginux: modern virtio block device discovery failed\r\n");
    };
    if !arch::x86_64::pci::verify_parser_rejections(virtio_block) {
        kernel_fail("ginux: malformed virtio PCI configuration accepted\r\n");
    }
    let Some((virtio_common_physical, virtio_common_length)) = virtio_block.common_region() else {
        kernel_fail("ginux: invalid virtio common configuration range\r\n");
    };
    let Some((virtio_mmio_physical, virtio_mmio_length)) = virtio_block.mmio_span() else {
        kernel_fail("ginux: invalid virtio MMIO span\r\n");
    };
    let Some((virtio_msix_physical, virtio_msix_length)) = virtio_block.msix_mmio_span() else {
        kernel_fail("ginux: invalid virtio MSI-X MMIO span\r\n");
    };
    let Some(virtio_network) = arch::x86_64::pci::discover_virtio_network() else {
        kernel_fail("ginux: modern virtio network device discovery failed\r\n");
    };
    let virtio_network_parser_valid =
        arch::x86_64::pci::verify_network_parser_rejections(virtio_network);
    let virtio_network_firmware_command =
        arch::x86_64::pci::network_function_command(virtio_network);
    let virtio_network_revoked =
        virtio_network_parser_valid && arch::x86_64::pci::revoke_network_function(virtio_network);
    if !virtio_network_parser_valid || !virtio_network_revoked {
        serial_write("ginux: virtio network parser/revoked/firmware-command: ");
        serial_write_u64(u64::from(virtio_network_parser_valid));
        serial_write("/");
        serial_write_u64(u64::from(virtio_network_revoked));
        serial_write("/");
        serial_write_u64(u64::from(virtio_network_firmware_command));
        serial_write("\r\n");
        kernel_fail("ginux: virtio network discovery boundary invalid\r\n");
    }
    let Some((virtio_network_common_physical, virtio_network_common_length)) =
        virtio_network.common_region()
    else {
        kernel_fail("ginux: invalid virtio network common configuration range\r\n");
    };
    let Some((virtio_network_device_physical, virtio_network_device_length)) =
        virtio_network.device_region()
    else {
        kernel_fail("ginux: invalid virtio network device configuration range\r\n");
    };
    let Some((virtio_network_notify_physical, virtio_network_notify_length)) =
        virtio_network.notify_region()
    else {
        kernel_fail("ginux: invalid virtio network notification range\r\n");
    };
    let Some((virtio_network_msix_physical, virtio_network_msix_length)) =
        virtio_network.msix_mmio_span()
    else {
        kernel_fail("ginux: invalid virtio network MSI-X range\r\n");
    };
    let Some((virtio_network_msix_table_physical, virtio_network_msix_table_length)) =
        virtio_network.msix_table_region()
    else {
        kernel_fail("ginux: invalid virtio network MSI-X table range\r\n");
    };
    let Some((virtio_network_msix_pba_physical, virtio_network_msix_pba_length)) =
        virtio_network.msix_pba_region()
    else {
        kernel_fail("ginux: invalid virtio network MSI-X PBA range\r\n");
    };

    let Some(memory_map_end) = boot_info
        .memory_map_size
        .checked_sub(1)
        .and_then(|size| (boot_info.memory_map as usize).checked_add(size))
    else {
        kernel_fail("ginux: UEFI memory map address overflow\r\n");
    };

    let stack_start = core::ptr::addr_of!(KERNEL_STACK) as usize;
    let kernel_stack_guard = stack_start;
    let kernel_stack_start = stack_start + PAGE_SIZE;
    let kernel_stack_upper_guard = stack_start + KERNEL_STACK_TOP_OFFSET;
    let double_fault_guard = arch::x86_64::cpu::double_fault_guard_page();
    let double_fault_upper_guard = arch::x86_64::cpu::double_fault_upper_guard_page();
    let privilege_stack_guard = arch::x86_64::cpu::privilege_stack_guard_page();
    let privilege_stack_upper_guard = arch::x86_64::cpu::privilege_stack_upper_guard_page();
    let required_addresses = [
        kernel_main as *const () as usize,
        boot_info as *const BootInfo as usize,
        boot_info.image_base as usize,
        image_end,
        boot_info.memory_map as usize,
        memory_map_end,
        kernel_stack_start,
        kernel_stack_upper_guard - 1,
        boot_info.reset_system as usize,
        boot_info.acpi_rsdp as usize,
        acpi_rsdp_end,
        boot_info.manifest as usize,
        boot_info.manifest as usize + boot_info.manifest_size - 1,
        boot_info.control_plane_bundle as usize,
        boot_info.control_plane_bundle as usize + boot_info.control_plane_bundle_size - 1,
        boot_info.root_image as usize,
        boot_info.root_image as usize + boot_info.root_image_size - 1,
        privilege_stack_guard + PAGE_SIZE,
        arch::x86_64::cpu::privilege_stack_top() - 1,
        double_fault_guard + PAGE_SIZE,
        arch::x86_64::cpu::double_fault_stack_top() - 1,
    ];
    let guard_pages = [
        0,
        kernel_stack_guard,
        kernel_stack_upper_guard,
        privilege_stack_guard,
        privilege_stack_upper_guard,
        double_fault_guard,
        double_fault_upper_guard,
    ];

    if !arch::x86_64::cpu::enable_execute_disable() {
        kernel_fail("ginux: execute-disable unavailable or inactive\r\n");
    }

    // SAFETY: The entry trampoline disabled interrupts. The required live
    // addresses and the page-table objects are checked against the complete
    // transitional identity-map range before CR3 is changed.
    let Ok(arch::x86_64::paging::InitialMmioMappings {
        block_transport: virtio_mmio,
        block_msix: virtio_msix_mmio,
        iommu: iommu_mmio,
        network_common: virtio_network_common_mmio,
        network_device: virtio_network_device_mmio,
        network_notification: virtio_network_notify_mmio,
        network_msix: virtio_network_msix_mmio,
    }) = (unsafe {
        arch::x86_64::paging::install_initial_identity_map(
            boot_info,
            &image,
            &required_addresses,
            &guard_pages,
            [
                (virtio_mmio_physical, virtio_mmio_length),
                (virtio_msix_physical, virtio_msix_length),
                (Q35_IOMMU_REGISTER_BASE, PAGE_SIZE),
                (virtio_network_common_physical, virtio_network_common_length),
                (virtio_network_device_physical, virtio_network_device_length),
                (virtio_network_notify_physical, virtio_network_notify_length),
                (virtio_network_msix_physical, virtio_network_msix_length),
            ],
        )
    })
    else {
        kernel_fail("ginux: page-table transition failed\r\n");
    };
    serial_write("ginux: kernel page tables active\r\n");
    if !arch::x86_64::cpu::execute_disable_enabled()
        || !arch::x86_64::paging::identity_map_obeys_wx(boot_info, &image)
    {
        kernel_fail("ginux: kernel identity-map W^X inactive\r\n");
    }
    serial_write("ginux: NX-by-default identity map active\r\n");
    if !arch::x86_64::paging::direct_map_alias_is_valid() {
        kernel_fail("ginux: higher-half direct-map alias invalid\r\n");
    }
    serial_write("ginux: supervisor-only higher-half direct map active\r\n");
    if !arch::x86_64::paging::direct_map_translation_is_bounded() {
        kernel_fail("ginux: direct-map translation bounds invalid\r\n");
    }
    serial_write("ginux: bounded direct-map translation verified\r\n");
    serial_write("ginux: PE executable identity pages read-only\r\n");
    serial_write("ginux: PE image section permissions enforced\r\n");
    if !arch::x86_64::paging::identity_range_is_read_only_data(
        boot_info.manifest as usize,
        boot_info.manifest_size,
    ) || !arch::x86_64::paging::identity_range_is_read_only_data(
        boot_info.control_plane_bundle as usize,
        boot_info.control_plane_bundle_size,
    ) || !arch::x86_64::paging::identity_range_is_read_only_data(
        boot_info.root_image as usize,
        boot_info.root_image_size,
    ) {
        kernel_fail("ginux: boot policy inputs remain mutable or executable\r\n");
    }
    serial_write("ginux: boot policy inputs immutable and non-executable\r\n");
    if !arch::x86_64::paging::supervisor_write_protect_enabled() {
        kernel_fail("ginux: supervisor write protection inactive\r\n");
    }
    serial_write("ginux: supervisor write protection active\r\n");
    if !arch::x86_64::paging::static_page_tables_are_read_only() {
        kernel_fail("ginux: static kernel page tables remain writable\r\n");
    }
    serial_write("ginux: static kernel page tables sealed read-only\r\n");
    if !arch::x86_64::cpu::enable_supervisor_execution_prevention() {
        kernel_fail("ginux: SMEP unavailable or inactive\r\n");
    }
    serial_write("ginux: supervisor execution prevention active\r\n");
    if !arch::x86_64::cpu::enable_supervisor_access_prevention() {
        kernel_fail("ginux: SMAP unavailable or inactive\r\n");
    }
    serial_write("ginux: supervisor access prevention active\r\n");
    if !arch::x86_64::cpu::enable_user_mode_instruction_prevention() {
        kernel_fail("ginux: UMIP unavailable or inactive\r\n");
    }
    serial_write("ginux: user-mode instruction prevention active\r\n");
    serial_write("ginux: null and bidirectional stack guard pages active\r\n");

    let Some(platform) = arch::x86_64::acpi::discover(
        boot_info.acpi_rsdp,
        arch::x86_64::paging::INITIAL_IDENTITY_LIMIT,
    ) else {
        kernel_fail("ginux: ACPI platform discovery failed\r\n");
    };
    serial_write("ginux: ACPI MADT validated\r\nginux: enabled processors: ");
    serial_write_usize(platform.enabled_processors);
    serial_write("\r\nginux: I/O APICs: ");
    serial_write_usize(platform.io_apics);
    serial_write("\r\nginux: MADT local APIC address: ");
    serial_write_u64(platform.local_apic_address);
    serial_write("\r\nginux: first I/O APIC address: ");
    serial_write_u64(u64::from(platform.first_io_apic_address));
    serial_write("\r\nginux: first I/O APIC GSI base: ");
    serial_write_u64(u64::from(platform.first_io_apic_gsi));
    serial_write("\r\nginux: ACPI DMAR validated");
    serial_write("\r\nginux: remapping hardware units: ");
    serial_write_usize(platform.iommu_units);
    serial_write("\r\nginux: DMA address width: ");
    serial_write_u64(u64::from(platform.iommu_host_address_width));
    serial_write("\r\nginux: first remapping register base: ");
    serial_write_u64(platform.first_iommu_register_base);
    serial_write("\r\nginux: first remapping unit includes all PCI devices: ");
    serial_write_u64(u64::from(platform.first_iommu_include_all));
    if platform.first_iommu_register_base != Q35_IOMMU_REGISTER_BASE
        || !arch::x86_64::acpi::verify_malformed_dmar_rejected(
            arch::x86_64::paging::INITIAL_IDENTITY_LIMIT,
        )
    {
        kernel_fail("\r\nginux: ACPI DMAR validation invariant failed\r\n");
    }
    serial_write("\r\nginux: malformed DMAR structures rejected");
    if !arch::x86_64::paging::mmio_mapping_is_writable(iommu_mmio)
        || !arch::x86_64::paging::mmio_identity_alias_is_absent(iommu_mmio)
        || !arch::x86_64::paging::mmio_translation_is_bounded(
            iommu_mmio,
            platform.first_iommu_register_base,
            PAGE_SIZE,
        )
    {
        kernel_fail("\r\nginux: IOMMU MMIO mapping invariant failed\r\n");
    }
    let Some(iommu_capabilities) =
        arch::x86_64::iommu::probe(platform.first_iommu_register_base, iommu_mmio)
    else {
        kernel_fail("\r\nginux: IOMMU capability probe failed\r\n");
    };
    let Some(virtio_iommu_profile) = arch::x86_64::iommu::DeviceTranslationProfile::new(
        platform.first_iommu_register_base,
        iommu_mmio,
        iommu_capabilities,
        virtio_block.bus,
        virtio_block.device,
        virtio_block.function,
    ) else {
        kernel_fail("\r\nginux: virtio IOMMU translation profile invalid\r\n");
    };
    let Some(virtio_network_iommu_profile) = arch::x86_64::iommu::DeviceTranslationProfile::new(
        platform.first_iommu_register_base,
        iommu_mmio,
        iommu_capabilities,
        virtio_network.bus,
        virtio_network.device,
        virtio_network.function,
    ) else {
        kernel_fail("\r\nginux: virtio-net IOMMU translation profile invalid\r\n");
    };
    if !virtio_network_iommu_profile.matches_source(
        virtio_network.bus,
        virtio_network.device,
        virtio_network.function,
    ) {
        kernel_fail("\r\nginux: virtio-net IOMMU source identity mismatch\r\n");
    }
    serial_write("\r\nginux: bounded writable uncached IOMMU mapping active");
    serial_write("\r\nginux: IOMMU version: ");
    serial_write_u64(u64::from(iommu_capabilities.major_version));
    serial_write(".");
    serial_write_u64(u64::from(iommu_capabilities.minor_version));
    serial_write("\r\nginux: IOMMU capability: ");
    serial_write_u64(iommu_capabilities.capability);
    serial_write("\r\nginux: IOMMU extended capability: ");
    serial_write_u64(iommu_capabilities.extended_capability);
    serial_write("\r\nginux: IOMMU translation remained disabled");
    serial_write("\r\n");
    serial_write("ginux: ACPI tables use higher-half direct map\r\n");

    if !arch::x86_64::cpu::install() {
        kernel_fail("ginux: architectural exception gates invalid\r\n");
    }
    serial_write("ginux: GDT/TSS/IDT active\r\n");
    if !arch::x86_64::cpu::task_register_denies_user_io() {
        kernel_fail("ginux: user I/O-port isolation inactive\r\n");
    }
    serial_write("ginux: user I/O-port access denied\r\n");
    serial_write("ginux: architectural exception gates active\r\n");
    if !arch::x86_64::cpu::verify_breakpoint() {
        kernel_fail("ginux: breakpoint exception test failed\r\n");
    }

    let Some(local_apic) = arch::x86_64::apic::discover() else {
        kernel_fail("ginux: enabled local APIC not found\r\n");
    };
    if local_apic.mode == arch::x86_64::apic::Mode::XApic
        && local_apic.physical_base != platform.local_apic_address
    {
        kernel_fail("ginux: CPUID/MSR and ACPI disagree on local APIC\r\n");
    }
    if !local_apic.mmio_uses_direct_map() {
        kernel_fail("ginux: local APIC direct-map access invalid\r\n");
    }
    serial_write("ginux: local APIC discovered\r\nginux: local APIC ID: ");
    serial_write_u64(u64::from(local_apic.id));
    serial_write("\r\nginux: local APIC mode: ");
    match local_apic.mode {
        arch::x86_64::apic::Mode::XApic => serial_write("xAPIC"),
        arch::x86_64::apic::Mode::X2Apic => serial_write("x2APIC"),
    }
    serial_write("\r\nginux: local APIC physical base: ");
    serial_write_u64(local_apic.physical_base);
    serial_write("\r\n");
    serial_write("ginux: local APIC uses higher-half direct map\r\n");

    let timers = arch::x86_64::timer::discover();
    serial_write("ginux: timer capabilities discovered\r\nginux: invariant TSC: ");
    serial_write(if timers.invariant_tsc {
        "advertised"
    } else {
        "not-advertised"
    });
    serial_write("\r\nginux: TSC deadline timer: ");
    serial_write(if timers.tsc_deadline {
        "supported"
    } else {
        "unsupported"
    });
    serial_write("\r\nginux: TSC frequency source: ");
    match timers.frequency_source {
        arch::x86_64::timer::FrequencySource::ArchitecturalRatio => {
            serial_write("architectural-ratio")
        }
        arch::x86_64::timer::FrequencySource::NominalProcessorFrequency => {
            serial_write("nominal-processor-frequency")
        }
        arch::x86_64::timer::FrequencySource::Unavailable => serial_write("unavailable"),
    }
    if let Some(frequency) = timers.frequency_hz {
        serial_write("\r\nginux: reported TSC Hz: ");
        serial_write_u64(frequency);
    }
    serial_write("\r\n");

    // SAFETY: The bootstrap processor owns the still-disabled interrupt
    // environment. The checked direct map covers the asserted xAPIC MMIO
    // limit, and no other kernel component uses the PIT or APIC timer.
    let Some(apic_timer) = (unsafe {
        arch::x86_64::timer::calibrate_local_apic(
            &local_apic,
            arch::x86_64::paging::INITIAL_IDENTITY_LIMIT,
        )
    }) else {
        kernel_fail("ginux: local APIC timer calibration failed\r\n");
    };
    serial_write("ginux: local APIC timer calibrated\r\nginux: APIC timer divider: ");
    serial_write_u64(u64::from(apic_timer.divider));
    serial_write("\r\nginux: APIC timer counter Hz: ");
    serial_write_u64(apic_timer.counter_hz);
    serial_write("\r\nginux: calibrated TSC Hz: ");
    serial_write_u64(apic_timer.tsc_hz);
    serial_write("\r\n");

    // SAFETY: The timer vector and APIC are exclusively owned during this
    // single-processor boot test. The routine restores CLI state before return.
    if !unsafe { arch::x86_64::timer::verify_local_apic_interrupt(&local_apic, &apic_timer) } {
        kernel_fail("ginux: local APIC timer interrupt failed\r\n");
    }
    serial_write("ginux: local APIC timer interrupt handled\r\n");

    serial_write("ginux: bounded PCI bus 0 enumerated\r\n");
    serial_write("ginux: modern virtio block function: ");
    serial_write_u64(u64::from(virtio_block.bus));
    serial_write(":");
    serial_write_u64(u64::from(virtio_block.device));
    serial_write(".");
    serial_write_u64(u64::from(virtio_block.function));
    if !platform.first_iommu_covers_pci(
        virtio_block.bus,
        virtio_block.device,
        virtio_block.function,
    ) || !arch::x86_64::acpi::verify_dmar_scope_resolution(
        arch::x86_64::paging::INITIAL_IDENTITY_LIMIT,
    ) {
        kernel_fail("\r\nginux: virtio IOMMU source-scope resolution failed\r\n");
    }
    serial_write("\r\nginux: first IOMMU device scopes: ");
    serial_write_u64(platform.first_iommu_scope_count() as u64);
    serial_write("\r\nginux: virtio IOMMU endpoint scope resolved exactly");
    serial_write("\r\nginux: modern virtio network function: ");
    serial_write_u64(u64::from(virtio_network.bus));
    serial_write(":");
    serial_write_u64(u64::from(virtio_network.device));
    serial_write(".");
    serial_write_u64(u64::from(virtio_network.function));
    if virtio_network.device == virtio_block.device
        || !platform.first_iommu_covers_pci(
            virtio_network.bus,
            virtio_network.device,
            virtio_network.function,
        )
        || !arch::x86_64::pci::network_function_is_disabled(virtio_network)
    {
        kernel_fail("\r\nginux: virtio network discovery invariant failed\r\n");
    }
    serial_write("\r\nginux: virtio-net common BAR address: ");
    serial_write_u64(virtio_network.common_bar_address);
    serial_write("\r\nginux: virtio-net common configuration offset: ");
    serial_write_u64(u64::from(virtio_network.common_offset));
    serial_write("\r\nginux: virtio-net common configuration length: ");
    serial_write_u64(u64::from(virtio_network.common_length));
    serial_write("\r\nginux: virtio-net notification multiplier: ");
    serial_write_u64(u64::from(virtio_network.notify_multiplier));
    serial_write("\r\nginux: virtio-net PCI capabilities: ");
    serial_write_u64(u64::from(virtio_network.capability_count));
    serial_write("\r\nginux: virtio-net MSI-X vectors: ");
    serial_write_u64(u64::from(virtio_network.msix.vector_count));
    serial_write("\r\nginux: modern virtio-net PCI capabilities validated");
    serial_write("\r\nginux: malformed virtio-net PCI configurations rejected");
    serial_write("\r\nginux: firmware virtio-net DMA authority revoked");
    serial_write("\r\nginux: virtio-net PCI decoding and bus mastering disabled");
    serial_write("\r\nginux: virtio-net IOMMU endpoint scope resolved exactly");
    serial_write("\r\nginux: virtio common BAR address: ");
    serial_write_u64(virtio_block.common_bar_address);
    serial_write("\r\nginux: virtio common configuration offset: ");
    serial_write_u64(u64::from(virtio_block.common_offset));
    serial_write("\r\nginux: virtio common configuration length: ");
    serial_write_u64(u64::from(virtio_block.common_length));
    serial_write("\r\nginux: virtio notification multiplier: ");
    serial_write_u64(u64::from(virtio_block.notify_multiplier));
    serial_write("\r\nginux: virtio PCI capabilities: ");
    serial_write_u64(u64::from(virtio_block.capability_count));
    serial_write("\r\nginux: modern virtio-blk PCI capabilities validated\r\n");
    serial_write("ginux: malformed virtio PCI configurations rejected\r\n");
    let Some((msix_table_physical, msix_table_length)) = virtio_block.msix_table_region() else {
        kernel_fail("ginux: invalid virtio MSI-X table range\r\n");
    };
    let Some((msix_pba_physical, msix_pba_length)) = virtio_block.msix_pba_region() else {
        kernel_fail("ginux: invalid virtio MSI-X pending-bit range\r\n");
    };
    serial_write("ginux: virtio MSI-X capability offset: ");
    serial_write_u64(u64::from(virtio_block.msix.capability_offset));
    serial_write("\r\nginux: virtio MSI-X vectors: ");
    serial_write_u64(u64::from(virtio_block.msix.vector_count));
    serial_write("\r\nginux: virtio MSI-X table address/length: ");
    serial_write_u64(msix_table_physical);
    serial_write("/");
    serial_write_usize(msix_table_length);
    serial_write("\r\nginux: virtio MSI-X PBA address/length: ");
    serial_write_u64(msix_pba_physical);
    serial_write("/");
    serial_write_usize(msix_pba_length);
    serial_write("\r\nginux: virtio MSI-X function mask: ");
    serial_write(if virtio_block.msix.function_masked {
        "masked"
    } else {
        "unmasked"
    });
    serial_write("\r\nginux: disabled virtio MSI-X capability validated\r\n");
    if !arch::x86_64::paging::mmio_mapping_is_valid(virtio_mmio)
        || !arch::x86_64::paging::mmio_mapping_is_valid(virtio_msix_mmio)
        || !arch::x86_64::paging::mmio_mappings_are_exact([
            virtio_mmio,
            virtio_msix_mmio,
            iommu_mmio,
            virtio_network_common_mmio,
            virtio_network_device_mmio,
            virtio_network_notify_mmio,
            virtio_network_msix_mmio,
        ])
        || !arch::x86_64::paging::mmio_identity_alias_is_absent(virtio_msix_mmio)
        || !arch::x86_64::paging::mmio_translation_is_bounded(
            virtio_mmio,
            virtio_mmio_physical,
            virtio_mmio_length,
        )
        || !arch::x86_64::paging::mmio_translation_is_bounded(
            virtio_msix_mmio,
            virtio_msix_physical,
            virtio_msix_length,
        )
        || virtio_msix_mmio
            .address(msix_table_physical, msix_table_length)
            .is_none()
        || virtio_msix_mmio
            .address(msix_pba_physical, msix_pba_length)
            .is_none()
        || virtio_mmio
            .address(virtio_common_physical, virtio_common_length)
            .is_none()
        || !arch::x86_64::paging::mmio_mapping_is_valid(virtio_network_common_mmio)
        || !arch::x86_64::paging::mmio_mapping_is_writable(virtio_network_common_mmio)
        || !arch::x86_64::paging::mmio_identity_alias_is_absent(virtio_network_common_mmio)
        || !arch::x86_64::paging::mmio_translation_is_bounded(
            virtio_network_common_mmio,
            virtio_network_common_physical,
            virtio_network_common_length,
        )
        || virtio_network_common_mmio
            .address(virtio_network_common_physical, virtio_network_common_length)
            .is_none()
        || !arch::x86_64::paging::mmio_mapping_is_valid(virtio_network_device_mmio)
        || arch::x86_64::paging::mmio_mapping_is_writable(virtio_network_device_mmio)
        || !arch::x86_64::paging::mmio_identity_alias_is_absent(virtio_network_device_mmio)
        || !arch::x86_64::paging::mmio_translation_is_bounded(
            virtio_network_device_mmio,
            virtio_network_device_physical,
            virtio_network_device_length,
        )
        || virtio_network_device_mmio
            .address(virtio_network_device_physical, virtio_network_device_length)
            .is_none()
        || !arch::x86_64::paging::mmio_mapping_is_writable(virtio_network_notify_mmio)
        || !arch::x86_64::paging::mmio_identity_alias_is_absent(virtio_network_notify_mmio)
        || !arch::x86_64::paging::mmio_translation_is_bounded(
            virtio_network_notify_mmio,
            virtio_network_notify_physical,
            virtio_network_notify_length,
        )
        || !arch::x86_64::paging::mmio_mapping_is_writable(virtio_network_msix_mmio)
        || !arch::x86_64::paging::mmio_identity_alias_is_absent(virtio_network_msix_mmio)
        || !arch::x86_64::paging::mmio_translation_is_bounded(
            virtio_network_msix_mmio,
            virtio_network_msix_physical,
            virtio_network_msix_length,
        )
        || virtio_network_msix_mmio
            .address(
                virtio_network_msix_table_physical,
                virtio_network_msix_table_length,
            )
            .is_none()
        || virtio_network_msix_mmio
            .address(
                virtio_network_msix_pba_physical,
                virtio_network_msix_pba_length,
            )
            .is_none()
    {
        kernel_fail("ginux: virtio MMIO mapping invalid\r\n");
    }
    serial_write("ginux: bounded uncached virtio MMIO mapping active\r\n");
    serial_write("ginux: virtio MMIO translation bounds verified\r\n");
    serial_write("ginux: bounded uncached MSI-X mapping active\r\n");
    serial_write("ginux: MSI-X cacheable aliases removed\r\n");
    serial_write("ginux: bounded uncached virtio-net common mapping active\r\n");
    serial_write("ginux: bounded read-only uncached virtio-net device mapping active\r\n");
    serial_write("ginux: bounded uncached virtio-net notification mapping active\r\n");
    serial_write("ginux: bounded uncached virtio-net MSI-X mapping active\r\n");
    serial_write("ginux: virtio-net cacheable alias absent\r\n");
    let Some(virtio_network_initialization) = arch::x86_64::pci::initialize_network_without_dma(
        virtio_network,
        virtio_network_common_mmio,
        virtio_network_device_mmio,
    ) else {
        kernel_fail("ginux: virtio network initialization failed\r\n");
    };
    if !arch::x86_64::pci::verify_network_feature_rejections(
        virtio_network_initialization.offered_features,
        virtio_network_initialization.accepted_features,
    ) || !arch::x86_64::pci::network_function_is_disabled(virtio_network)
        || !arch::x86_64::pci::verify_network_queue_rejections(virtio_network_initialization)
        || !arch::x86_64::pci::network_device_is_reset(virtio_network, virtio_network_common_mmio)
        || !arch::x86_64::pci::network_msix_is_disabled(virtio_network)
        || virtio_network_notify_mmio
            .address(virtio_network_initialization.receive_notification, 2)
            .is_none()
        || virtio_network_notify_mmio
            .address(virtio_network_initialization.transmit_notification, 2)
            .is_none()
    {
        kernel_fail("ginux: virtio network initialization invariant failed\r\n");
    }
    serial_write("ginux: virtio-net offered features: ");
    serial_write_u64(virtio_network_initialization.offered_features);
    serial_write("\r\nginux: virtio-net accepted features: ");
    serial_write_u64(virtio_network_initialization.accepted_features);
    serial_write("\r\nginux: virtio-net queues available: ");
    serial_write_u64(u64::from(virtio_network_initialization.queue_count));
    serial_write("\r\nginux: virtio-net receive queue size: ");
    serial_write_u64(u64::from(virtio_network_initialization.receive_queue_size));
    serial_write("\r\nginux: virtio-net transmit queue size: ");
    serial_write_u64(u64::from(virtio_network_initialization.transmit_queue_size));
    serial_write("\r\nginux: virtio-net receive notification address: ");
    serial_write_u64(virtio_network_initialization.receive_notification);
    serial_write("\r\nginux: virtio-net transmit notification address: ");
    serial_write_u64(virtio_network_initialization.transmit_notification);
    serial_write("\r\nginux: virtio-net queues remain disabled and DMA-unbound");
    serial_write("\r\nginux: invalid virtio-net queue geometry rejected");
    serial_write("\r\nginux: virtio-net MSI-X remained disabled");
    serial_write("\r\nginux: virtio-net stable MAC validated: ");
    serial_write_u64(
        virtio_network_initialization
            .mac
            .into_iter()
            .fold(0_u64, |value, byte| (value << 8) | u64::from(byte)),
    );
    serial_write("\r\nginux: virtio-net platform DMA negotiation validated");
    serial_write("\r\nginux: invalid virtio-net feature sets rejected");
    serial_write("\r\nginux: virtio-net returned to reset with PCI decoding disabled\r\n");
    if !arch::x86_64::pci::verify_network_msix_programming_rejections(
        virtio_network,
        virtio_network_msix_mmio,
    ) || !arch::x86_64::pci::program_masked_network_msix_vectors(
        virtio_network,
        virtio_network_msix_mmio,
        local_apic.id,
    ) || !arch::x86_64::pci::network_msix_vectors_are_masked(
        virtio_network,
        virtio_network_msix_mmio,
        local_apic.id,
    ) || !arch::x86_64::pci::network_function_is_disabled(virtio_network)
    {
        kernel_fail("ginux: masked virtio-net MSI-X programming failed\r\n");
    }
    serial_write("ginux: invalid virtio-net MSI-X programming rejected atomically\r\n");
    serial_write("ginux: masked virtio-net RX/TX MSI-X vectors programmed\r\n");
    serial_write("ginux: virtio-net MSI-X remained globally disabled\r\n");
    serial_write("ginux: virtio-net remained reset and DMA-disabled after MSI-X setup\r\n");
    if !arch::x86_64::pci::verify_msix_programming_rejections(virtio_block, virtio_msix_mmio) {
        kernel_fail("ginux: invalid MSI-X programming mutated table\r\n");
    }
    if !arch::x86_64::pci::program_masked_msix_vector(
        virtio_block,
        virtio_msix_mmio,
        0,
        local_apic.id,
        33,
    ) {
        kernel_fail("ginux: masked MSI-X vector programming failed\r\n");
    }
    serial_write("ginux: invalid MSI-X programming rejected atomically\r\n");
    serial_write("ginux: kernel-owned MSI-X vector programmed masked\r\n");
    serial_write("ginux: MSI-X function remained disabled\r\n");
    let Some(virtio_initialization) =
        arch::x86_64::pci::initialize_without_dma(virtio_block, virtio_mmio)
    else {
        kernel_fail("ginux: virtio block initialization failed\r\n");
    };
    if !arch::x86_64::pci::verify_feature_rejections(
        virtio_initialization.offered_features,
        virtio_initialization.accepted_features,
    ) {
        kernel_fail("ginux: invalid virtio block features accepted\r\n");
    }
    serial_write("ginux: virtio block offered features: ");
    serial_write_u64(virtio_initialization.offered_features);
    serial_write("\r\nginux: virtio block accepted features: ");
    serial_write_u64(virtio_initialization.accepted_features);
    serial_write("\r\nginux: virtio block queues available: ");
    serial_write_u64(u64::from(virtio_initialization.queue_count));
    serial_write("\r\nginux: virtio block reset and feature negotiation validated\r\n");
    serial_write("ginux: virtio bus mastering remained disabled\r\n");
    serial_write("ginux: virtio block returned to reset state\r\n");

    if !volume::verify_contract() || !volume::install_live() {
        kernel_fail("ginux: persistent volume contract invalid\r\n");
    }
    serial_write("ginux: bounded persistent volume format validated\r\n");
    serial_write("ginux: interrupted volume commits recovered\r\n");
    serial_write("ginux: corrupt persistent volume records rejected\r\n");

    if !capability::verify_authority_invariants() {
        kernel_fail("ginux: capability authority invariant failed\r\n");
    }
    serial_write("ginux: capability derivation and revocation verified\r\n");
    serial_write("ginux: exhaustive capability attenuation verified\r\n");
    if !domain::verify_cross_domain_revocation_invariants() {
        kernel_fail("ginux: cross-domain revocation invariant failed\r\n");
    }
    serial_write("ginux: cross-domain capability revocation verified\r\n");
    serial_write("ginux: per-root delegation provenance verified\r\n");
    serial_write("ginux: exhaustive delegated attenuation verified\r\n");
    serial_write("ginux: independent delegation roots verified\r\n");
    if !domain::verify_domain_ancestry_invariants() {
        kernel_fail("ginux: domain ancestry invariant failed\r\n");
    }
    serial_write("ginux: domain lifecycle ancestry verified\r\n");
    if !domain::verify_generation_exhaustion_invariants() {
        kernel_fail("ginux: generation exhaustion invariant failed\r\n");
    }
    serial_write("ginux: exhausted generational slots skipped\r\n");
    if !domain::verify_frame_quarantine_registry_invariants() {
        kernel_fail("ginux: frame quarantine registry invariant failed\r\n");
    }
    serial_write("ginux: per-service frame quarantines isolated\r\n");
    if !domain::verify_auxiliary_service_registry_invariants() {
        kernel_fail("ginux: auxiliary service registry invariant failed\r\n");
    }
    serial_write("ginux: bounded auxiliary service registry verified\r\n");
    serial_write("ginux: service lifecycle roles separated\r\n");
    serial_write("ginux: service failure policies separated\r\n");
    serial_write("ginux: domain resource admission policies verified\r\n");
    serial_write("ginux: auxiliary service quiescence enforced\r\n");
    serial_write("ginux: auxiliary supervision authority enforced\r\n");
    serial_write("ginux: auxiliary retirement events sequenced\r\n");
    serial_write("ginux: auxiliary retirement publication atomic\r\n");
    if !domain::verify_auxiliary_stop_revocation_invariants() {
        kernel_fail("ginux: auxiliary stop revocation invariant failed\r\n");
    }
    serial_write("ginux: auxiliary stop authority revoked\r\n");
    if !domain::verify_auxiliary_retirement_overflow_invariants() {
        kernel_fail("ginux: auxiliary retirement overflow invariant failed\r\n");
    }
    serial_write("ginux: auxiliary retirement overflow summarized\r\n");
    if !domain::verify_auxiliary_fault_policy_invariants() {
        kernel_fail("ginux: auxiliary fault policy invariant failed\r\n");
    }
    serial_write("ginux: stop-only auxiliary faults contained\r\n");
    serial_write("ginux: auxiliary fault authority revoked before publication\r\n");
    if !domain::verify_service_restart_budget_invariants() {
        kernel_fail("ginux: service restart budget invariant failed\r\n");
    }
    serial_write("ginux: bounded service restart policy verified\r\n");
    if !domain::verify_clean_service_exit_invariants() {
        kernel_fail("ginux: clean service exit invariant failed\r\n");
    }
    serial_write("ginux: clean service exit publication atomic\r\n");
    if !domain::verify_lifecycle_event_invariants() {
        kernel_fail("ginux: lifecycle event queue invariant failed\r\n");
    }
    serial_write("ginux: bounded lifecycle event queue verified\r\n");
    serial_write("ginux: lifecycle event overflow notification verified\r\n");

    let Some(control_plane) = domain::initialize_control_plane(&boot_manifest) else {
        kernel_fail("ginux: control-plane domain construction failed\r\n");
    };
    serial_write(
        "ginux: persistent control-plane domain sealed\r\nginux: control-plane domain ID: ",
    );
    serial_write_u64(control_plane);
    serial_write("\r\n");
    let Some(service_domain) = domain::initialize_boot_service() else {
        kernel_fail("ginux: isolated service domain construction failed\r\n");
    };
    let Some(service_identity) = domain::domain_service_identity(service_domain) else {
        kernel_fail("ginux: stable service identity unavailable\r\n");
    };
    if domain::primary_service_identity() != Some(service_identity) {
        kernel_fail("ginux: primary service identity mismatch\r\n");
    }
    serial_write("ginux: IPC-only service domain ID: ");
    serial_write_u64(service_domain);
    serial_write("\r\n");

    let summary = summarize_memory(boot_info);
    serial_write("ginux: memory descriptors: ");
    serial_write_usize(summary.descriptors);
    serial_write("\r\nginux: usable memory MiB: ");
    serial_write_u64(summary.conventional_pages / UEFI_PAGES_PER_MIB);
    serial_write("\r\nginux: reclaimable boot-service pages: ");
    serial_write_u64(summary.boot_service_pages);
    serial_write("\r\nginux: reserved loader pages: ");
    serial_write_u64(summary.loader_pages);
    if !verify_initial_frame_allocation(boot_info) {
        kernel_fail("ginux: physical frame allocation test failed\r\n");
    }
    serial_write("\r\nginux: physical frame allocator active");
    serial_write("\r\nginux: reclaimed boot-service frames: ");
    serial_write_u64(memory::reclaimed_boot_service_frame_count().unwrap_or(0));
    serial_write("\r\nginux: atomic frame reservations verified");
    serial_write("\r\nginux: contiguous frame reservations verified");
    serial_write("\r\nginux: physical frames zeroed before allocation");
    serial_write("\r\nginux: tracked frame release ownership verified");
    serial_write("\r\nginux: stale frame leases rejected");
    serial_write("\r\nginux: per-domain physical frame budgets enforced");
    if !arch::x86_64::pci::verify_queue_rejections() {
        kernel_fail("\r\nginux: virtio queue contract invalid\r\n");
    }
    let Some([virtio_dma_frame, iommu_guard_frame]) =
        memory::allocate_frames::<2>(memory::FrameOwner::KERNEL)
    else {
        kernel_fail("\r\nginux: virtio DMA and guard frame allocation failed\r\n");
    };
    if !memory::frames_owned_by(
        memory::FrameOwner::KERNEL,
        &[virtio_dma_frame, iommu_guard_frame],
    ) {
        kernel_fail("\r\nginux: virtio DMA and guard frame ownership invalid\r\n");
    }
    let Some(iommu_translation) = arch::x86_64::iommu::build_inactive_translation(
        iommu_capabilities,
        virtio_block.bus,
        virtio_block.device,
        virtio_block.function,
        virtio_dma_frame,
    ) else {
        kernel_fail("\r\nginux: inactive IOMMU translation construction failed\r\n");
    };
    if !iommu_translation.is_exact(virtio_dma_frame)
        || !arch::x86_64::iommu::verify_inactive_translation_rejections(
            iommu_capabilities,
            virtio_dma_frame,
        )
        || arch::x86_64::iommu::probe(platform.first_iommu_register_base, iommu_mmio).is_none()
    {
        kernel_fail("\r\nginux: inactive IOMMU translation invariant failed\r\n");
    }
    let Some(virtio_iommu_evidence) = iommu_translation.evidence(virtio_dma_frame) else {
        kernel_fail("\r\nginux: inactive IOMMU translation evidence unavailable\r\n");
    };
    serial_write("\r\nginux: IOMMU root/context hierarchy constructed");
    serial_write("\r\nginux: IOMMU root table address: ");
    serial_write_u64(iommu_translation.root_table_address());
    serial_write("\r\nginux: one-page identity DMA translation encoded exactly");
    serial_write("\r\nginux: invalid IOMMU table inputs rejected");
    serial_write("\r\nginux: IOMMU hardware state unchanged after table construction");
    let Some(iommu_active) = arch::x86_64::iommu::activate_translation(
        platform.first_iommu_register_base,
        iommu_mmio,
        iommu_capabilities,
        &iommu_translation,
        virtio_dma_frame,
    ) else {
        kernel_fail("\r\nginux: IOMMU translation activation failed\r\n");
    };
    if !iommu_active.is_active() {
        kernel_fail("\r\nginux: IOMMU translation activation invariant failed\r\n");
    }
    serial_write("\r\nginux: IOMMU root table installed");
    serial_write("\r\nginux: IOMMU translation enabled for virtio DMA");
    let Some(virtio_dma_virtual) = virtio_dma_frame.direct_map_address() else {
        kernel_fail("\r\nginux: virtio DMA frame is outside direct map\r\n");
    };
    let Some(iommu_guard_virtual) = iommu_guard_frame.direct_map_address() else {
        kernel_fail("\r\nginux: IOMMU guard frame is outside direct map\r\n");
    };
    // The guard remains owned but deliberately absent from the translation
    // hierarchy. A blocked device write must leave every sentinel byte intact.
    unsafe {
        core::ptr::write_bytes(iommu_guard_virtual as *mut u8, 0xa5, 4096);
    }
    // SAFETY: The queue page is exclusively kernel-owned and mapped by the
    // active IOMMU hierarchy. The guard target is separately owned, aligned,
    // and intentionally unmapped. `finish` resets the device and revokes bus
    // mastering before either lease is inspected or released.
    let Some(mut fault_probe) = (unsafe {
        arch::x86_64::pci::VirtioBlockBackend::start(
            virtio_block,
            virtio_mmio,
            virtio_dma_frame.address(),
            virtio_dma_virtual,
        )
    }) else {
        kernel_fail("\r\nginux: adversarial virtio backend activation failed\r\n");
    };
    let Some(unmapped_attempt) = fault_probe.attempt_unmapped_read(2, iommu_guard_frame.address())
    else {
        kernel_fail("\r\nginux: unauthorized virtio DMA request was not submitted\r\n");
    };
    let faulting_transport_bounded = unmapped_attempt.transport_outcome_is_bounded(0);
    let faulting_device_quiesced = fault_probe.finish();
    let guard_unchanged = (0..4096)
        .all(|offset| unsafe { (iommu_guard_virtual as *const u8).add(offset).read() == 0xa5 });
    if !faulting_transport_bounded {
        kernel_fail("\r\nginux: unauthorized virtio DMA request state invalid\r\n");
    }
    if !faulting_device_quiesced {
        kernel_fail("\r\nginux: faulting virtio device teardown failed\r\n");
    }
    if !guard_unchanged {
        kernel_fail("\r\nginux: unmapped DMA guard frame was modified\r\n");
    }
    serial_write("\r\nginux: unauthorized virtio DMA payload blocked");
    serial_write("\r\nginux: faulting virtio transport outcome bounded");
    serial_write("\r\nginux: unmapped DMA guard frame remained unchanged");
    serial_write("\r\nginux: faulting virtio device reset and bus mastering revoked");
    if !iommu_active.recover_expected_write_fault(
        virtio_block.bus,
        virtio_block.device,
        virtio_block.function,
        iommu_guard_frame.address(),
    ) {
        kernel_fail("\r\nginux: expected IOMMU fault validation or recovery failed\r\n");
    }
    serial_write("\r\nginux: IOMMU fault source and address validated");
    serial_write("\r\nginux: IOMMU write-permission fault reason validated");
    serial_write("\r\nginux: IOMMU fault record cleared and translation recovered");
    // SAFETY: The allocator returned one exclusively kernel-owned page, its
    // direct-map alias covers the complete physical frame, and the lease stays
    // live until the device has reset and bus mastering is disabled.
    let Some(virtio_read) = (unsafe {
        arch::x86_64::pci::read_blank_sector_interrupt(
            virtio_block,
            virtio_mmio,
            virtio_msix_mmio,
            &local_apic,
            virtio_dma_frame.address(),
            virtio_dma_virtual,
            2,
        )
    }) else {
        kernel_fail("\r\nginux: bounded virtio block read failed\r\n");
    };
    serial_write("\r\nginux: kernel-owned virtio MSI-X interrupt delivered");
    serial_write("\r\nginux: exactly one virtio completion interrupt handled");
    serial_write("\r\nginux: virtio MSI-X masked and disabled after completion");
    serial_write("\r\nginux: temporary virtio interrupt gate retired");
    if virtio_read.capacity_sectors != 2048 || virtio_read.used_length != 513 {
        kernel_fail("\r\nginux: deterministic virtio block geometry invalid\r\n");
    }
    // SAFETY: The one-shot probe returned only after reset and bus-master
    // revocation. The same exclusive lease remains live and is reused by the
    // persistent backend until `finish` proves teardown.
    let Some(mut persistent_block) = (unsafe {
        arch::x86_64::pci::VirtioBlockBackend::start(
            virtio_block,
            virtio_mmio,
            virtio_dma_frame.address(),
            virtio_dma_virtual,
        )
    }) else {
        kernel_fail("\r\nginux: persistent virtio backend activation failed\r\n");
    };
    const DISK_VOLUME_PAYLOAD: &[u8] = b"ginux-reboot-persistent\n";
    let disk_volume = (|| {
        if persistent_block.capacity_sectors() != 2048 {
            return None;
        }
        let initial = volume::recover(&mut persistent_block);
        let recovered_after_reboot = initial.is_some();
        let record = match initial {
            Some(record) => record,
            None => volume::commit(&mut persistent_block, DISK_VOLUME_PAYLOAD).ok()?,
        };
        if record.generation != 1 || record.payload() != DISK_VOLUME_PAYLOAD {
            return None;
        }
        let verified = volume::recover(&mut persistent_block)?;
        (verified == record).then_some((verified, recovered_after_reboot))
    })();
    if !persistent_block.finish() {
        kernel_fail("\r\nginux: persistent virtio backend teardown failed\r\n");
    }
    serial_write("\r\nginux: persistent virtio device reset and bus mastering revoked");
    if !iommu_active.finish()
        || arch::x86_64::iommu::probe(platform.first_iommu_register_base, iommu_mmio).is_none()
    {
        kernel_fail("\r\nginux: IOMMU translation teardown failed\r\n");
    }
    serial_write("\r\nginux: IOMMU translation disabled after device quiescence");
    serial_write("\r\nginux: IOMMU root table cleared");
    serial_write("\r\nginux: IOMMU fault status clear after recovery");
    if !iommu_translation.retire() {
        kernel_fail("\r\nginux: inactive IOMMU translation retirement failed\r\n");
    }
    serial_write("\r\nginux: IOMMU translation frames scrubbed and released");
    if !memory::scrub_and_release_frames(memory::FrameOwner::KERNEL, &[iommu_guard_frame])
        || memory::any_frame_lease_live(&[iommu_guard_frame])
    {
        kernel_fail("\r\nginux: IOMMU guard frame retirement failed\r\n");
    }
    serial_write("\r\nginux: IOMMU guard frame scrubbed and released");
    if !memory::scrub_and_release_frames(memory::FrameOwner::KERNEL, &[virtio_dma_frame])
        || memory::any_frame_lease_live(&[virtio_dma_frame])
    {
        kernel_fail("\r\nginux: virtio DMA frame retirement failed\r\n");
    }
    serial_write("\r\nginux: virtio DMA frame scrubbed and released");
    if !virtio_iommu_evidence.is_retired()
        || !virtio_network_iommu_profile.is_inactive()
        || !arch::x86_64::pci::network_function_is_disabled(virtio_network)
        || !arch::x86_64::pci::verify_network_dma_layout()
        || !arch::x86_64::pci::verify_network_dma_layout_rejections()
    {
        kernel_fail("\r\nginux: virtio-net DMA construction precondition failed\r\n");
    }
    let Some([virtio_network_dma_frame, virtio_network_guard_frame]) =
        memory::allocate_frames::<2>(memory::FrameOwner::KERNEL)
    else {
        kernel_fail("\r\nginux: virtio-net DMA and guard frame allocation failed\r\n");
    };
    // SAFETY: The allocator owns and directly maps this live kernel lease.
    if !memory::frames_owned_by(
        memory::FrameOwner::KERNEL,
        &[virtio_network_dma_frame, virtio_network_guard_frame],
    ) || unsafe { !frame_is_zero(virtio_network_dma_frame) }
        || unsafe { !frame_is_zero(virtio_network_guard_frame) }
    {
        kernel_fail("\r\nginux: virtio-net DMA or guard frame ownership invalid\r\n");
    }
    let Some(virtio_network_translation) = arch::x86_64::iommu::build_inactive_translation(
        iommu_capabilities,
        virtio_network.bus,
        virtio_network.device,
        virtio_network.function,
        virtio_network_dma_frame,
    ) else {
        kernel_fail("\r\nginux: virtio-net IOMMU hierarchy construction failed\r\n");
    };
    let Some(virtio_network_evidence) =
        virtio_network_translation.evidence(virtio_network_dma_frame)
    else {
        kernel_fail("\r\nginux: virtio-net IOMMU evidence unavailable\r\n");
    };
    if !virtio_network_translation.is_exact(virtio_network_dma_frame)
        || !virtio_network_evidence.is_fresh_after(virtio_iommu_evidence)
        || !virtio_network_iommu_profile.is_inactive()
        || !arch::x86_64::pci::network_function_is_disabled(virtio_network)
        || !arch::x86_64::iommu::verify_inactive_translation_rejections(
            iommu_capabilities,
            virtio_network_dma_frame,
        )
    {
        kernel_fail("\r\nginux: virtio-net DMA confinement invariant failed\r\n");
    }
    serial_write("\r\nginux: generation-owned virtio-net DMA page allocated");
    serial_write("\r\nginux: bounded virtio-net RX/TX DMA layout validated");
    serial_write("\r\nginux: malformed virtio-net DMA layouts rejected");
    serial_write("\r\nginux: dedicated virtio-net IOMMU hierarchy constructed");
    serial_write("\r\nginux: virtio-net IOMMU source and DMA page encoded exactly");
    serial_write("\r\nginux: virtio-net DMA and IOMMU leases are generation-fresh");
    serial_write("\r\nginux: virtio-net IOMMU hardware remained inactive");
    serial_write("\r\nginux: virtio-net bus mastering remained disabled");
    let Some(virtio_network_dma_virtual) = virtio_network_dma_frame.direct_map_address() else {
        kernel_fail("\r\nginux: virtio-net DMA page is outside direct map\r\n");
    };
    let Some(virtio_network_guard_virtual) = virtio_network_guard_frame.direct_map_address() else {
        kernel_fail("\r\nginux: virtio-net guard page is outside direct map\r\n");
    };
    let Some(virtio_network_active) = arch::x86_64::iommu::activate_translation(
        platform.first_iommu_register_base,
        iommu_mmio,
        iommu_capabilities,
        &virtio_network_translation,
        virtio_network_dma_frame,
    ) else {
        kernel_fail("\r\nginux: virtio-net IOMMU activation failed\r\n");
    };
    let network_queues_activated = virtio_network_active.is_active()
        && unsafe {
            arch::x86_64::pci::activate_network_queues_without_io(
                virtio_network,
                virtio_network_common_mmio,
                virtio_network_dma_frame.address(),
                virtio_network_dma_virtual,
            )
        };
    // SAFETY: The same exclusive page and exact source translation remain
    // live. The empty-queue proof returned only after resetting the device and
    // revoking bus mastering, so the one-shot transmit can safely reconfigure
    // the function before the IOMMU translation is retired below.
    let network_transmit = network_queues_activated
        .then(|| unsafe {
            arch::x86_64::pci::transmit_network_probe(
                virtio_network,
                virtio_network_common_mmio,
                virtio_network_notify_mmio,
                virtio_network_msix_mmio,
                &local_apic,
                virtio_network_dma_frame.address(),
                virtio_network_dma_virtual,
            )
        })
        .flatten();
    // The guard is owned but absent from the NIC's exact translation. A
    // blocked receive write must leave every byte unchanged.
    unsafe {
        core::ptr::write_bytes(virtio_network_guard_virtual as *mut u8, 0xa5, PAGE_SIZE);
    }
    // SAFETY: Both pages are exclusively kernel-owned. Only the queue page is
    // in the active VT-d hierarchy; the aligned guard target is deliberately
    // absent and remains live until after the device is reset below.
    let network_dma_fault = network_transmit
        .is_some()
        .then(|| unsafe {
            arch::x86_64::pci::attempt_unmapped_network_receive(
                virtio_network,
                virtio_network_common_mmio,
                virtio_network_notify_mmio,
                virtio_network_msix_mmio,
                &local_apic,
                virtio_network_dma_frame.address(),
                virtio_network_dma_virtual,
                virtio_network_guard_frame.address(),
            )
        })
        .flatten();
    let network_fault_completion_published =
        network_dma_fault.map(|fault| fault.completion_published);
    let network_guard_unchanged = (0..PAGE_SIZE).all(|offset| unsafe {
        (virtio_network_guard_virtual as *const u8)
            .add(offset)
            .read()
            == 0xa5
    });
    if network_fault_completion_published.is_none() || !network_guard_unchanged {
        kernel_fail("\r\nginux: unauthorized virtio-net RX DMA was not contained\r\n");
    }
    if !virtio_network_active.recover_expected_write_fault(
        virtio_network.bus,
        virtio_network.device,
        virtio_network.function,
        virtio_network_guard_frame.address(),
    ) {
        kernel_fail("\r\nginux: virtio-net IOMMU fault validation or recovery failed\r\n");
    }
    // SAFETY: The faulting exchange returned with the function reset and bus
    // mastering revoked. Fault state has been cleared and the exact translation
    // recovered before reusing the mapped page for a valid reply.
    let network_receive = network_dma_fault
        .is_some()
        .then(|| unsafe {
            arch::x86_64::pci::receive_network_arp_reply(
                virtio_network,
                virtio_network_common_mmio,
                virtio_network_notify_mmio,
                virtio_network_msix_mmio,
                &local_apic,
                virtio_network_dma_frame.address(),
                virtio_network_dma_virtual,
            )
        })
        .flatten();
    let network_translation_retired = virtio_network_active.finish();
    if !network_queues_activated {
        kernel_fail("\r\nginux: virtio-net empty queue activation failed\r\n");
    }
    if network_transmit.is_none_or(|transmit| transmit.frame_length != 19) {
        kernel_fail("\r\nginux: confined virtio-net transmit failed\r\n");
    }
    if network_receive.is_none_or(|receive| !(42..=1523).contains(&receive.frame_length)) {
        kernel_fail("\r\nginux: confined virtio-net receive failed\r\n");
    }
    if !network_translation_retired
        || !arch::x86_64::pci::network_function_is_disabled(virtio_network)
    {
        kernel_fail("\r\nginux: virtio-net translation retirement failed\r\n");
    }
    serial_write("\r\nginux: virtio-net IOMMU activated before queue authority");
    serial_write("\r\nginux: empty virtio-net RX/TX queues bound to confined DMA");
    serial_write("\r\nginux: virtio-net DRIVER_OK established with empty rings");
    serial_write("\r\nginux: virtio-net bus mastering enabled last");
    serial_write("\r\nginux: empty virtio-net queues quiesced without notification");
    serial_write("\r\nginux: virtio-net bus mastering revoked before IOMMU teardown");
    serial_write("\r\nginux: confined virtio-net TX descriptor published");
    serial_write("\r\nginux: virtio-net TX queue notified");
    serial_write("\r\nginux: exactly one virtio-net TX interrupt delivered");
    serial_write("\r\nginux: virtio-net TX used-ring completion validated");
    serial_write("\r\nginux: virtio-net TX payload and guards preserved");
    serial_write("\r\nginux: bounded Ethernet probe transmitted");
    serial_write("\r\nginux: virtio-net TX route retired before DMA release");
    serial_write("\r\nginux: unauthorized virtio-net RX DMA payload blocked");
    serial_write("\r\nginux: faulting virtio-net transport outcome bounded");
    serial_write("\r\nginux: virtio-net unmapped guard page remained unchanged");
    serial_write("\r\nginux: faulting virtio-net device quiesced before recovery");
    serial_write("\r\nginux: virtio-net IOMMU fault source and address validated");
    serial_write("\r\nginux: virtio-net IOMMU write fault cleared and translation recovered");
    serial_write("\r\nginux: confined virtio-net RX descriptor published");
    serial_write("\r\nginux: bounded ARP request transmitted");
    serial_write("\r\nginux: exactly one virtio-net RX interrupt delivered");
    serial_write("\r\nginux: virtio-net RX used-ring completion validated");
    serial_write("\r\nginux: QEMU gateway ARP reply validated");
    serial_write("\r\nginux: virtio-net RX payload bounds preserved");
    serial_write("\r\nginux: virtio-net RX/TX routes retired before DMA release");
    if !virtio_network_translation.retire() {
        kernel_fail("\r\nginux: virtio-net DMA confinement retirement failed\r\n");
    }
    if !memory::scrub_and_release_frames(memory::FrameOwner::KERNEL, &[virtio_network_guard_frame])
        || memory::any_frame_lease_live(&[virtio_network_guard_frame])
    {
        kernel_fail("\r\nginux: virtio-net guard frame retirement failed\r\n");
    }
    serial_write("\r\nginux: virtio-net unmapped guard page scrubbed and released");
    if !memory::scrub_and_release_frames(memory::FrameOwner::KERNEL, &[virtio_network_dma_frame])
        || memory::any_frame_lease_live(&[virtio_network_dma_frame])
        || !virtio_network_evidence.is_retired()
    {
        kernel_fail("\r\nginux: virtio-net DMA frame retirement failed\r\n");
    }
    serial_write("\r\nginux: virtio-net DMA confinement retired cleanly");
    let Some((disk_record, recovered_after_reboot)) = disk_volume else {
        kernel_fail("\r\nginux: persistent virtio volume transaction failed\r\n");
    };
    serial_write("\r\nginux: generation-owned virtio DMA frame allocated");
    serial_write("\r\nginux: malformed virtio queue geometry rejected");
    serial_write("\r\nginux: bounded blank virtio sector read completed");
    serial_write("\r\nginux: virtio read guard bytes preserved");
    serial_write("\r\nginux: virtio used-ring completion validated");
    serial_write("\r\nginux: deterministic virtio capacity sectors: ");
    serial_write_u64(virtio_read.capacity_sectors);
    serial_write("\r\nginux: virtio bus mastering revoked after read");
    serial_write("\r\nginux: bounded virtio volume transaction completed");
    serial_write("\r\nginux: disk-backed volume generation: ");
    serial_write_u64(disk_record.generation);
    serial_write(if recovered_after_reboot {
        "\r\nginux: disk-backed volume recovered after reboot"
    } else {
        "\r\nginux: disk-backed volume initialized and flushed"
    });
    if !address_space::verify_isolated_user_mapping() {
        kernel_fail("\r\nginux: isolated address-space test failed\r\n");
    }
    serial_write("\r\nginux: isolated address space active");
    if !address_space::verify_control_plane_wx_layout() {
        kernel_fail("\r\nginux: control-plane W^X layout failed\r\n");
    }
    serial_write("\r\nginux: control-plane W^X mappings active");
    serial_write("\r\nginux: address-space frames use higher-half direct map");
    if !address_space::verify_auxiliary_address_space_lifecycle() {
        kernel_fail("\r\nginux: auxiliary address-space lifecycle failed\r\n");
    }
    serial_write("\r\nginux: auxiliary address-space transaction verified");
    serial_write("\r\nginux: concurrent auxiliary address spaces isolated");
    serial_write("\r\nginux: auxiliary scheduler admission resource-gated");
    serial_write("\r\nginux: auxiliary address-space ownership retained");
    serial_write("\r\nginux: auxiliary runtime teardown path connected");
    serial_write("\r\nginux: auxiliary teardown restores kernel page tables");
    let control_plane_bundle = core::slice::from_raw_parts(
        boot_info.control_plane_bundle,
        boot_info.control_plane_bundle_size,
    );
    let Some(control_plane_image) = executable::parse(control_plane_bundle) else {
        kernel_fail("\r\nginux: control-plane executable container invalid\r\n");
    };
    if !executable::verify_parser_rejections(control_plane_bundle) {
        kernel_fail("\r\nginux: malformed executable containers accepted\r\n");
    }
    serial_write("\r\nginux: bounded executable container accepted");
    serial_write("\r\nginux: authenticated executable entry points accepted");
    serial_write("\r\nginux: malformed executable containers rejected\r\n");
    let root_image = core::slice::from_raw_parts(boot_info.root_image, boot_info.root_image_size);
    if !initramfs::verify_pinned_image(root_image) {
        kernel_fail("ginux: bounded initramfs image invalid\r\n");
    }
    serial_write("ginux: deterministic initramfs image accepted\r\n");
    serial_write("ginux: bounded newc archive paths validated\r\n");
    serial_write("ginux: executable init entry discovered\r\n");
    if !initramfs::verify_rejections(root_image) {
        kernel_fail("ginux: malformed initramfs parser test failed\r\n");
    }
    serial_write("ginux: malformed initramfs archives rejected\r\n");
    if !initramfs::verify_owned_materialization(root_image) {
        kernel_fail("ginux: owned root-image materialization failed\r\n");
    }
    serial_write("ginux: measured root image copied into owned memory\r\n");
    serial_write("ginux: owned root image remained source-independent\r\n");
    serial_write("ginux: owned root frames scrubbed and released\r\n");
    if !initramfs::verify_read_only_index(root_image) {
        kernel_fail("ginux: read-only root index failed\r\n");
    }
    serial_write("ginux: bounded read-only root index constructed\r\n");
    serial_write("ginux: root lookups resolved only owned image bytes\r\n");
    serial_write("ginux: invalid root lookup paths denied\r\n");
    serial_write("ginux: indexed root frames scrubbed and released\r\n");
    if !initramfs::publish_read_only_root(root_image) {
        kernel_fail("ginux: persistent read-only root publication failed\r\n");
    }
    serial_write("ginux: measured root published in kernel-owned memory\r\n");
    let Some(volume_service_node) = initramfs::published_regular_file_node(b"native/volume") else {
        kernel_fail("ginux: isolated volume service missing\r\n");
    };
    let Some(volume_service) = initramfs::published_file_by_node(volume_service_node) else {
        kernel_fail("ginux: isolated volume service unreadable\r\n");
    };
    let Some(volume_image) = native_elf::inspect(volume_service) else {
        kernel_fail("ginux: isolated volume service ELF invalid\r\n");
    };
    if volume_image.bytes.len() != volume_service.len()
        || volume_image.entry == 0
        || volume_image.segments.iter().flatten().count() != 3
        || volume_image.relocation_count == 0
        || volume_image.relocations[..volume_image.relocation_count]
            .iter()
            .any(|relocation| {
                relocation.is_none_or(|relocation| relocation.target == 0 || relocation.addend == 0)
            })
    {
        kernel_fail("ginux: isolated volume service load plan incomplete\r\n");
    }
    serial_write("ginux: measured isolated volume service discovered\r\n");
    serial_write("ginux: bounded native PIE load plan accepted\r\n");
    serial_write("ginux: relative-only service relocations accepted\r\n");
    if !address_space::verify_native_service_materialization(&volume_image) {
        kernel_fail("ginux: native volume service materialization failed\r\n");
    }
    serial_write("ginux: native service segments materialized in owned frames\r\n");
    serial_write("ginux: native service relative relocations applied\r\n");
    serial_write("ginux: native service W^X address space verified\r\n");
    serial_write("ginux: native service guard pages verified\r\n");
    let Some(network_service_node) = initramfs::published_regular_file_node(b"native/network")
    else {
        kernel_fail("ginux: isolated network service missing\r\n");
    };
    let Some(network_service) = initramfs::published_file_by_node(network_service_node) else {
        kernel_fail("ginux: isolated network service unreadable\r\n");
    };
    let Some(network_image) = native_elf::inspect(network_service) else {
        kernel_fail("ginux: isolated network service ELF invalid\r\n");
    };
    if network_image.bytes.len() != network_service.len()
        || network_image.entry == 0
        || network_image.segments.iter().flatten().count() != 3
        || network_image.relocation_count == 0
        || network_image.relocations[..network_image.relocation_count]
            .iter()
            .any(|relocation| {
                relocation.is_none_or(|relocation| relocation.target == 0 || relocation.addend == 0)
            })
    {
        kernel_fail("ginux: isolated network service load plan incomplete\r\n");
    }
    serial_write("ginux: measured isolated network service discovered\r\n");
    serial_write("ginux: bounded native network PIE load plan accepted\r\n");
    serial_write("ginux: native network service relative relocations accepted\r\n");
    if !address_space::verify_native_service_materialization(&network_image) {
        kernel_fail("ginux: native network service materialization failed\r\n");
    }
    serial_write("ginux: native network service segments materialized in owned frames\r\n");
    serial_write("ginux: native network service relative relocations applied\r\n");
    serial_write("ginux: native network service W^X address space verified\r\n");
    serial_write("ginux: native network service guard pages verified\r\n");
    let Some(protocol_service_node) = initramfs::published_regular_file_node(b"native/protocol")
    else {
        kernel_fail("ginux: isolated protocol service missing\r\n");
    };
    let Some(protocol_service) = initramfs::published_file_by_node(protocol_service_node) else {
        kernel_fail("ginux: isolated protocol service unreadable\r\n");
    };
    let Some(protocol_image) = native_elf::inspect(protocol_service) else {
        kernel_fail("ginux: isolated protocol service ELF invalid\r\n");
    };
    if protocol_image.bytes.len() != protocol_service.len()
        || protocol_image.entry == 0
        || protocol_image.segments.iter().flatten().count() != 3
        || protocol_image.relocation_count == 0
        || protocol_image.relocations[..protocol_image.relocation_count]
            .iter()
            .any(|relocation| {
                relocation.is_none_or(|relocation| relocation.target == 0 || relocation.addend == 0)
            })
    {
        kernel_fail("ginux: isolated protocol service load plan incomplete\r\n");
    }
    serial_write("ginux: measured isolated protocol service discovered\r\n");
    serial_write("ginux: bounded native protocol PIE load plan accepted\r\n");
    serial_write("ginux: native protocol service relative relocations accepted\r\n");
    if !address_space::verify_native_service_materialization(&protocol_image) {
        kernel_fail("ginux: native protocol service materialization failed\r\n");
    }
    serial_write("ginux: native protocol service segments materialized in owned frames\r\n");
    serial_write("ginux: native protocol service relative relocations applied\r\n");
    serial_write("ginux: native protocol service W^X address space verified\r\n");
    serial_write("ginux: native protocol service guard pages verified\r\n");
    if !linux_mount::verify_contract() || !linux_mount::install_initial() {
        kernel_fail("ginux: initial Linux mount namespace invalid\r\n");
    }
    serial_write("ginux: generation-safe Linux mount namespace installed\r\n");
    serial_write("ginux: immutable measured root mounted read-only\r\n");
    if !linux_user::verify_contract() || !linux_user::install_initial() {
        kernel_fail("ginux: initial Linux user namespace invalid\r\n");
    }
    serial_write("ginux: generation-safe Linux user namespace installed\r\n");
    serial_write("ginux: container root mapped outside native authority\r\n");
    serial_write("ginux: Linux privilege policy sealed fail-closed\r\n");
    if !linux_tmpfs::verify_contract() || !linux_tmpfs::install() {
        kernel_fail("ginux: bounded Linux tmpfs invalid\r\n");
    }
    serial_write("ginux: generation-safe Linux tmpfs mounted at /tmp\r\n");
    let Some(system_profile_node) =
        initramfs::published_regular_file_node(b"etc/ginux-system-profile")
    else {
        kernel_fail("ginux: minimal system profile missing\r\n");
    };
    if initramfs::published_file_by_node(system_profile_node).is_none() {
        kernel_fail("ginux: minimal system profile unreadable\r\n");
    }
    serial_write("ginux: pinned minimal system profile accepted\r\n");
    if !linux_process::verify_lifecycle_contract() {
        kernel_fail("ginux: bounded Linux process lifecycle contract invalid\r\n");
    }
    serial_write("ginux: bounded Linux process lifecycle contract validated\r\n");
    serial_write("ginux: generation-bound Linux process roots validated\r\n");
    serial_write("ginux: bounded Linux process resource bundles validated\r\n");
    serial_write("ginux: Linux process mapping inventory validated\r\n");
    serial_write("ginux: transactional Linux child publication validated\r\n");
    serial_write("ginux: cloned Linux child resources published atomically\r\n");
    serial_write("ginux: address-space-bound Linux process contexts validated\r\n");
    serial_write("ginux: Linux fork child return context derived\r\n");
    serial_write("ginux: Linux process contexts staged on native scheduler\r\n");
    serial_write("ginux: scheduled Linux process roots matched ownership\r\n");
    serial_write("ginux: stale Linux process scheduler bindings rejected\r\n");
    serial_write("ginux: Linux process roots inherited kernel mappings\r\n");
    serial_write("ginux: scheduled Linux child CR3 activated and restored\r\n");
    serial_write("ginux: scheduled Linux child returned from ring 3\r\n");
    serial_write("ginux: Linux fork child RAX zero executed\r\n");
    serial_write("ginux: Linux process child FS isolation executed\r\n");
    serial_write("ginux: complete Linux process register frame executed\r\n");
    serial_write("ginux: Linux process child exit captured from ring 3\r\n");
    serial_write("ginux: Linux process child exit drove zombie wait status\r\n");
    if !linux_process::verify_private_page_copy_contract() {
        kernel_fail("ginux: bounded Linux private page copy invalid\r\n");
    }
    serial_write("ginux: bounded Linux private page copy validated\r\n");
    if !address_space::verify_linux_process_hierarchy_clone() {
        kernel_fail("ginux: bounded Linux child hierarchy clone invalid\r\n");
    }
    serial_write("ginux: bounded Linux child hierarchy clone validated\r\n");
    let Some(system_init_node) = initramfs::published_regular_file_node(b"sbin/init") else {
        kernel_fail("ginux: minimal system init missing\r\n");
    };
    let Some(system_init) = initramfs::published_file_by_node(system_init_node) else {
        kernel_fail("ginux: minimal system init unreadable\r\n");
    };
    let Some(system_init_plan) = linux_elf::measured_static_exec_plan(system_init) else {
        kernel_fail("ginux: minimal system init invalid\r\n");
    };
    if system_init_plan.entry != 0x41_5100
        || system_init_plan.executable.page_count != 2
        || system_init_plan.writable.page_count != 1
        || system_init_plan.executable.writable
        || !system_init_plan.executable.executable
        || !system_init_plan.writable.writable
        || system_init_plan.writable.executable
    {
        kernel_fail("ginux: minimal system init plan outside profile\r\n");
    }
    serial_write("ginux: measured minimal system init planned\r\n");
    let Some(exec_second_node) = initramfs::published_regular_file_node(b"bin/exec-second") else {
        kernel_fail("ginux: second measured Linux exec target missing\r\n");
    };
    let Some(exec_second) = initramfs::published_file_by_node(exec_second_node) else {
        kernel_fail("ginux: second measured Linux exec target unreadable\r\n");
    };
    let Some(signal_probe_node) = initramfs::published_regular_file_node(b"bin/signal-probe")
    else {
        kernel_fail("ginux: measured Linux signal probe missing\r\n");
    };
    let Some(signal_probe) = initramfs::published_file_by_node(signal_probe_node) else {
        kernel_fail("ginux: measured Linux signal probe unreadable\r\n");
    };
    let Some(exec_second_plan) = linux_elf::measured_static_exec_plan(exec_second) else {
        kernel_fail("ginux: second measured Linux exec target invalid\r\n");
    };
    let Some(signal_probe_plan) = linux_elf::measured_static_exec_plan(signal_probe) else {
        kernel_fail("ginux: measured Linux signal probe invalid\r\n");
    };
    let measured_file = |name: &[u8]| {
        initramfs::published_regular_file_node(name).and_then(initramfs::published_file_by_node)
    };
    let Some(dynamic_main) = measured_file(b"linux/init") else {
        kernel_fail("ginux: measured Linux dynamic main missing\r\n");
    };
    let Some(dynamic_interpreter) = measured_file(b"linux/ld-ginux.so") else {
        kernel_fail("ginux: measured Linux interpreter missing\r\n");
    };
    let Some(dynamic_dependency) = measured_file(b"linux/libginux.so") else {
        kernel_fail("ginux: measured Linux primary dependency missing\r\n");
    };
    let Some(dynamic_auxiliary) = measured_file(b"linux/libginux-aux.so") else {
        kernel_fail("ginux: measured Linux auxiliary dependency missing\r\n");
    };
    let Some(dynamic_plan) = linux_elf::measured_interpreted_exec_plan(
        dynamic_main,
        dynamic_interpreter,
        [dynamic_dependency, dynamic_auxiliary],
    ) else {
        kernel_fail("ginux: measured Linux interpreted graph invalid\r\n");
    };
    if dynamic_plan.process.initial_entry != 0x60_0200
        || dynamic_plan.process.main_entry != 0x40_0100
        || dynamic_plan.dependency_bases != [0x61_0000, 0x63_0000]
        || linux_elf::measured_interpreted_exec_plan(
            signal_probe,
            dynamic_interpreter,
            [dynamic_dependency, dynamic_auxiliary],
        )
        .is_some()
        || linux_elf::measured_interpreted_exec_plan(
            dynamic_main,
            dynamic_interpreter,
            [dynamic_auxiliary, dynamic_dependency],
        )
        .is_some()
        || linux_elf::measured_interpreted_exec_plan(
            dynamic_main,
            &dynamic_interpreter[..64],
            [dynamic_dependency, dynamic_auxiliary],
        )
        .is_some()
    {
        kernel_fail("ginux: measured Linux interpreted graph substitution accepted\r\n");
    }
    serial_write("ginux: measured-root Linux interpreted exec graph planned\r\n");
    serial_write("ginux: measured-root Linux graph substitution rejected\r\n");
    if signal_probe == exec_second
        || signal_probe_plan.executable.virtual_page == exec_second_plan.executable.virtual_page
        || signal_probe_plan.executable.page_count != 2
        || exec_second_plan.executable.page_count != 2
    {
        kernel_fail("ginux: measured Linux exec target invalid\r\n");
    }
    serial_write("ginux: dedicated measured Linux signal runtime planned\r\n");
    serial_write("ginux: multiple measured Linux exec targets planned\r\n");
    if !linux_elf::verify_parser() {
        kernel_fail("ginux: Linux ELF64 parser invariant failed\r\n");
    }
    serial_write("ginux: non-executable Linux GNU stack enforced\r\n");
    serial_write("ginux: bounded Linux ELF metadata headers validated\r\n");
    serial_write("ginux: bounded Linux interpreter metadata validated\r\n");
    serial_write("ginux: bounded Linux ELF verifier stack usage validated\r\n");
    serial_write("ginux: bounded Linux GNU RELRO metadata validated\r\n");
    serial_write("ginux: three-load Linux ELF runtime mapping verified\r\n");
    if !linux_elf::verify_linked_probe() {
        kernel_fail("ginux: linked Linux ELF64 probe invariant failed\r\n");
    }
    serial_write("ginux: live Linux process address-space snapshot validated\r\n");
    serial_write("ginux: incomplete Linux process snapshots rejected\r\n");
    serial_write("ginux: live Linux PID 1 context admitted to process model\r\n");
    serial_write("ginux: live Linux process registry persisted across syscalls\r\n");
    serial_write("ginux: live Linux child address space cloned transactionally\r\n");
    serial_write("ginux: live Linux cloned child executed in ring 3\r\n");
    serial_write("ginux: toolchain-linked static Linux ELF executed\r\n");
    serial_write("ginux: five-page Linux executable mapping validated\r\n");
    serial_write("ginux: bounded Linux exec rejection preserved live image\r\n");
    serial_write("ginux: Linux exec mapping rollback restored active image\r\n");
    serial_write("ginux: Linux exec argv and environment delivered\r\n");
    serial_write("ginux: bounded Linux exec vectors validated\r\n");
    serial_write("ginux: old Linux image pages removed after exec\r\n");
    serial_write("ginux: replaced Linux image retired cleanly\r\n");
    serial_write("ginux: Linux close-on-exec descriptors transitioned\r\n");
    serial_write("ginux: Linux exec signal state transitioned\r\n");
    serial_write("ginux: Linux exec process identity installed\r\n");
    serial_write("ginux: Linux executable link followed exec identity\r\n");
    serial_write("ginux: measured-root Linux exec selection validated\r\n");
    serial_write("ginux: measured-dirfd Linux execveat validated\r\n");
    serial_write("ginux: measured-fd Linux empty-path exec validated\r\n");
    serial_write("ginux: measured Linux exec selection is profile-driven\r\n");
    serial_write("ginux: independent measured Linux exec layouts validated\r\n");
    serial_write("ginux: runtime-selected Linux exec plans committed\r\n");
    serial_write("ginux: variable-page Linux exec images committed\r\n");
    serial_write("ginux: Linux active image ownership transitioned\r\n");
    serial_write("ginux: generation-bound Linux image ownership verified\r\n");
    serial_write("ginux: generation-bound Linux page-table hierarchy verified\r\n");
    serial_write("ginux: Linux RELRO ownership permissions synchronized\r\n");
    serial_write("ginux: stale Linux image retirement rejected\r\n");
    serial_write("ginux: old Linux image leases retired at exec commit\r\n");
    serial_write("ginux: Linux image lease double release rejected\r\n");
    serial_write("ginux: Linux replacement frames allocated on demand\r\n");
    serial_write("ginux: Linux replacement frame generations reclaimed\r\n");
    if !linux_elf::verify_load_plan() {
        kernel_fail("ginux: Linux ELF64 load plan invariant failed\r\n");
    }
    serial_write("ginux: bounded multi-page Linux ELF plan verified\r\n");
    serial_write("ginux: bounded Linux ET_DYN load bias validated\r\n");
    if !linux_elf::verify_interpreted_plan() {
        kernel_fail("ginux: authenticated Linux dependency plan invalid\r\n");
    }
    serial_write("ginux: bounded Linux interpreter resolution plan validated\r\n");
    serial_write("ginux: measured Linux interpreter bytes enforced\r\n");
    serial_write("ginux: authenticated Linux dependency bytes enforced\r\n");
    serial_write("ginux: Linux dependency SONAME identity bound\r\n");
    if !linux_elf::verify_dynamic_relocation_rejections() {
        kernel_fail("ginux: malformed Linux dynamic relocation metadata accepted\r\n");
    }
    serial_write("ginux: bounded Linux relative relocation metadata validated\r\n");
    serial_write("ginux: bounded Linux symbol relocation metadata validated\r\n");
    serial_write("ginux: bounded Linux PLT relocation metadata validated\r\n");
    serial_write("ginux: split Linux PLT relocation table validated\r\n");
    serial_write("ginux: bounded Linux SysV hash metadata validated\r\n");
    serial_write("ginux: bounded Linux GNU hash metadata validated\r\n");
    if !linux_elf::verify_unresolved_dynamic_image_rejected() {
        kernel_fail("ginux: unresolved dynamic Linux image admitted as static\r\n");
    }
    serial_write("ginux: unresolved dynamic Linux static entry denied\r\n");
    serial_write("ginux: separate-header Linux interpreter plan validated\r\n");
    serial_write("ginux: Linux dynamic startup metadata validated\r\n");
    serial_write("ginux: Linux auxiliary vector encoding validated\r\n");
    serial_write("ginux: Linux page-aliasing ELF segments rejected\r\n");
    serial_write("ginux: Linux ELF entry metadata overlap rejected\r\n");
    if !linux_elf::verify_dependency_list_metadata() {
        kernel_fail("ginux: bounded Linux dependency-list metadata invalid\r\n");
    }
    serial_write("ginux: bounded Linux dependency list validated\r\n");
    serial_write("ginux: bounded Linux dependency authority graph validated\r\n");
    serial_write("ginux: bounded Linux dependency TLS metadata validated\r\n");
    if !linux_elf::verify_tls_relocation_metadata() {
        kernel_fail("ginux: malformed Linux TLS relocation metadata accepted\r\n");
    }
    serial_write("ginux: bounded Linux TPOFF64 relocation metadata validated\r\n");
    if !linux_elf::verify_static_tls_layout() {
        kernel_fail("ginux: bounded Linux static TLS layout invalid\r\n");
    }
    serial_write("ginux: bounded Linux static TLS layout plan validated\r\n");
    serial_write("ginux: reusable Linux TLS page materialization validated\r\n");
    if !arch::x86_64::cpu::verify_linux_tls_context_contract() {
        kernel_fail("ginux: bounded Linux TLS execution context invalid\r\n");
    }
    serial_write("ginux: bounded Linux TLS execution context validated\r\n");
    if !linux_elf::verify_tls_template_metadata() {
        kernel_fail("ginux: bounded Linux TLS template metadata invalid\r\n");
    }
    serial_write("ginux: bounded Linux TLS template metadata validated\r\n");
    if !linux_elf::verify_mapped_load_plan(boot_info.image_base as u64, boot_info.image_size) {
        kernel_fail("ginux: Linux ELF64 address-space mapping failed\r\n");
    }
    serial_write("ginux: Linux ELF headers mapped in executable segment\r\n");
    serial_write("ginux: bounded multi-page Linux ELF mapping verified\r\n");
    if !linux_elf::verify_mapped_system_init_plan(boot_info.image_base as u64, boot_info.image_size)
    {
        kernel_fail("ginux: minimal system init execution failed\r\n");
    }
    serial_write("ginux: measured minimal system init executed as PID 1\r\n");
    serial_write("ginux: persistent Linux PID 1 supervisor lifecycle validated\r\n");
    serial_write("ginux: bounded Linux standard signal lifecycle validated\r\n");
    serial_write("ginux: bounded Linux private futex lifecycle validated\r\n");
    serial_write("ginux: bounded Linux devfs profile validated\r\n");
    serial_write("ginux: lifecycle-aware bounded procfs validated\r\n");
    serial_write("ginux: generation-safe bounded tmpfs lifecycle validated\r\n");
    serial_write("ginux: minimal system init exited cleanly\r\n");
    if !linux_elf::verify_mapped_interpreted_plan(boot_info.image_base as u64, boot_info.image_size)
    {
        kernel_fail("ginux: interpreted Linux ELF64 address-space mapping failed\r\n");
    }
    serial_write("ginux: measured-root interpreted Linux exec committed\r\n");
    serial_write("ginux: interpreted Linux exec graph remapped\r\n");
    serial_write("ginux: bounded Linux cross-area copy-in validated\r\n");
    serial_write("ginux: complete interpreted Linux graph retired at exec\r\n");
    serial_write("ginux: interpreted Linux exec stack replaced atomically\r\n");
    serial_write("ginux: old interpreted Linux stack lease retired at exec\r\n");
    serial_write("ginux: replacement Linux stack lease reclaimed\r\n");
    serial_write("ginux: static Linux exec stack replaced atomically\r\n");
    serial_write("ginux: old static Linux stack lease retired at exec\r\n");
    serial_write("ginux: replacement static Linux stack lease reclaimed\r\n");
    serial_write("ginux: interpreted Linux exec heap replaced atomically\r\n");
    serial_write("ginux: old interpreted Linux heap lease retired at exec\r\n");
    serial_write("ginux: replacement Linux heap lease reclaimed\r\n");
    serial_write("ginux: static Linux exec heap replaced atomically\r\n");
    serial_write("ginux: old static Linux heap lease retired at exec\r\n");
    serial_write("ginux: replacement static Linux heap lease reclaimed\r\n");
    serial_write("ginux: Linux exec process break reset\r\n");
    serial_write("ginux: interpreted Linux TLS state rematerialized\r\n");
    serial_write("ginux: interpreted Linux TLS leases replaced atomically\r\n");
    serial_write("ginux: old interpreted Linux TLS leases retired at exec\r\n");
    serial_write("ginux: replacement Linux TLS leases reclaimed\r\n");
    serial_write("ginux: interpreted Linux sibling stack replaced atomically\r\n");
    serial_write("ginux: old interpreted Linux sibling stack lease retired at exec\r\n");
    serial_write("ginux: replacement Linux sibling stack lease reclaimed\r\n");
    serial_write("ginux: interpreted Linux mmap registry empty across exec\r\n");
    serial_write("ginux: static Linux mmap registry empty across exec\r\n");
    serial_write("ginux: interpreted Linux page-table hierarchy replaced atomically\r\n");
    serial_write("ginux: old interpreted Linux page-table leases retired at exec\r\n");
    serial_write("ginux: replacement Linux page-table leases reclaimed\r\n");
    serial_write("ginux: static Linux page-table hierarchy replaced atomically\r\n");
    serial_write("ginux: old static Linux page-table leases retired at exec\r\n");
    serial_write("ginux: replacement static Linux page-table leases reclaimed\r\n");
    serial_write("ginux: static Linux exec auxiliary vector rebuilt\r\n");
    serial_write("ginux: interpreted Linux exec auxiliary vector rebuilt\r\n");
    serial_write("ginux: canonical interpreted Linux executable identity validated\r\n");
    serial_write("ginux: interpreted Linux exec vectors generalized\r\n");
    serial_write("ginux: combined Linux static TLS materialized\r\n");
    serial_write("ginux: ordered Linux dependency TLS layout materialized\r\n");
    serial_write("ginux: Linux TLS active before initializers\r\n");
    serial_write("ginux: Linux dependency TLS active before constructors\r\n");
    serial_write("ginux: toolchain-driven Linux dependency TLS relocation applied\r\n");
    serial_write("ginux: toolchain-driven Linux auxiliary TLS relocation applied\r\n");
    serial_write("ginux: linker-resolved Linux TLS symbols executed\r\n");
    serial_write("ginux: bounded Linux TLS syscall copies validated\r\n");
    serial_write("ginux: independent Linux sibling TLS materialized\r\n");
    serial_write("ginux: non-current Linux TLS syscall access denied\r\n");
    serial_write("ginux: Linux TLS execution context switched and restored\r\n");
    serial_write("ginux: guarded Linux sibling stack materialized\r\n");
    serial_write("ginux: non-current Linux stack syscall access denied\r\n");
    serial_write("ginux: bounded Linux clone contract validated\r\n");
    serial_write("ginux: Linux child return context constructed\r\n");
    serial_write("ginux: Linux child context cleared on slot reuse\r\n");
    serial_write("ginux: failed Linux clone publication rolled back\r\n");
    serial_write("ginux: Linux parent and child TID publication validated\r\n");
    serial_write("ginux: unpublished Linux child retired cleanly\r\n");
    serial_write("ginux: full Linux syscall register frame preserved\r\n");
    serial_write("ginux: typed Linux syscall frame validated\r\n");
    serial_write("ginux: complete Linux child context persisted\r\n");
    serial_write("ginux: stale Linux return contexts rejected\r\n");
    serial_write("ginux: Linux child return frame installed atomically\r\n");
    serial_write("ginux: Linux return ownership restored to parent\r\n");
    serial_write("ginux: stale Linux return installation left state unchanged\r\n");
    serial_write("ginux: live Linux syscall return frame round-trip validated\r\n");
    serial_write("ginux: guarded Linux child executed in ring 3\r\n");
    serial_write("ginux: Linux child exit restored suspended parent\r\n");
    serial_write("ginux: bounded Linux clone syscall dispatched\r\n");
    serial_write("ginux: sequential Linux clone slot reuse validated\r\n");
    serial_write("ginux: reused Linux child stack began scrubbed\r\n");
    serial_write("ginux: monotonic Linux clone TIDs validated\r\n");
    serial_write("ginux: live Linux child TIDs matched published words\r\n");
    serial_write("ginux: live Linux child FS self pointers validated\r\n");
    serial_write("ginux: per-thread Linux signal masks isolated\r\n");
    serial_write("ginux: bidirectional Linux sched_yield switching validated\r\n");
    serial_write("ginux: occupied Linux clone capacity rejected live\r\n");
    serial_write("ginux: live Linux clone failures rolled back transactionally\r\n");
    serial_write("ginux: Linux clone return values validated\r\n");
    serial_write("ginux: Linux clone child TID cleared on exit\r\n");
    serial_write("ginux: Linux clone parent TID preserved on child exit\r\n");
    serial_write("ginux: Linux parent and child futex waits blocked\r\n");
    serial_write("ginux: explicit Linux futex wake restored child\r\n");
    serial_write("ginux: Linux clear-child-TID wake restored parent\r\n");
    serial_write("ginux: Linux futex waiter state reclaimed\r\n");
    serial_write("ginux: per-thread Linux syscall stacks validated\r\n");
    serial_write("ginux: bounded Linux TID lifecycle validated\r\n");
    serial_write("ginux: stale Linux thread handles rejected\r\n");
    serial_write("ginux: Linux per-thread exit state isolated\r\n");
    serial_write("ginux: Linux thread exit registrations cleared\r\n");
    serial_write("ginux: Linux thread table reclaimed on exit\r\n");
    serial_write("ginux: sibling Linux TLS frame scrubbed and released\r\n");
    serial_write("ginux: Linux sibling stack scrubbed and released\r\n");
    serial_write("ginux: retired Linux thread stacks scrubbed\r\n");
    serial_write("ginux: Linux TLS frame scrubbed and released\r\n");
    serial_write("ginux: bounded interpreted Linux ELF mapping verified\r\n");
    serial_write("ginux: toolchain-linked Linux interpreter executed\r\n");
    serial_write("ginux: two-page Linux interpreter RX mapping verified\r\n");
    serial_write("ginux: userspace Linux interpreter metadata discovered\r\n");
    serial_write("ginux: userspace Linux runtime files discovered\r\n");
    serial_write("ginux: userspace Linux PT_LOAD mappings derived\r\n");
    serial_write("ginux: userspace Linux startup metadata consumed\r\n");
    serial_write("ginux: userspace Linux main segments mapped\r\n");
    serial_write("ginux: userspace Linux DT_NEEDED path resolved\r\n");
    serial_write("ginux: userspace Linux dependency segments mapped\r\n");
    serial_write("ginux: complete interpreted Linux image ownership tracked\r\n");
    serial_write("ginux: userspace Linux dependency bias selected\r\n");
    serial_write("ginux: Linux dependency frames allocated on demand\r\n");
    serial_write("ginux: Linux executable frames allocated on demand\r\n");
    serial_write("ginux: bounded Linux dependency page contracts applied\r\n");
    serial_write("ginux: bounded Linux dependency mapping slots isolated\r\n");
    serial_write("ginux: userspace Linux auxiliary dependency mapped\r\n");
    serial_write("ginux: Linux read-only dependency segment mapped\r\n");
    serial_write("ginux: toolchain-linked Linux PIE and shared object executed\r\n");
    serial_write("ginux: linker-emitted Linux dynamic tags validated\r\n");
    serial_write("ginux: separate-header interpreted Linux mapping verified\r\n");
    serial_write("ginux: Linux dependency W^X mapping verified\r\n");
    serial_write("ginux: interpreter-first Linux ring-3 entry verified\r\n");
    serial_write("ginux: userspace Linux interpreter self-relocation applied\r\n");
    serial_write("ginux: userspace Linux relocation table shapes validated\r\n");
    serial_write("ginux: userspace Linux relative relocation applied\r\n");
    serial_write("ginux: userspace Linux GLOB_DAT relocation applied\r\n");
    serial_write("ginux: userspace Linux JUMP_SLOT relocation applied\r\n");
    serial_write("ginux: userspace Linux hash bucket resolution applied\r\n");
    serial_write("ginux: userspace Linux symbol names matched\r\n");
    serial_write("ginux: userspace Linux cross-object symbol resolved\r\n");
    serial_write("ginux: userspace Linux two-provider symbols resolved\r\n");
    serial_write("ginux: Linux JUMP_SLOT dependency call executed\r\n");
    serial_write("ginux: Linux auxiliary JUMP_SLOT dependency call executed\r\n");
    serial_write("ginux: userspace Linux dependency self-relocations applied\r\n");
    serial_write("ginux: userspace Linux auxiliary dependency self-relocations applied\r\n");
    serial_write("ginux: bounded Linux main preinit array validated\r\n");
    serial_write("ginux: userspace Linux main preinit metadata consumed\r\n");
    serial_write("ginux: userspace Linux main preinit executed\r\n");
    serial_write("ginux: Linux preinit-before-dependency initialization ordered\r\n");
    serial_write("ginux: bounded Linux dependency init arrays validated\r\n");
    serial_write("ginux: userspace Linux init arrays bound to relocations\r\n");
    serial_write("ginux: userspace Linux dependency constructors executed\r\n");
    serial_write("ginux: bounded Linux main init array validated\r\n");
    serial_write("ginux: userspace Linux main constructor executed\r\n");
    serial_write("ginux: Linux dependency-before-main initialization ordered\r\n");
    serial_write("ginux: userspace Linux constructor startup context delivered\r\n");
    serial_write("ginux: bounded Linux DT_INIT functions validated\r\n");
    serial_write("ginux: userspace Linux DT_INIT functions executed\r\n");
    serial_write("ginux: Linux DT_INIT-before-init-array ordering enforced\r\n");
    serial_write("ginux: Linux auxiliary dependency entry executed\r\n");
    serial_write("ginux: metadata-driven Linux relative relocations applied\r\n");
    serial_write("ginux: Linux copy errors and overflow bounds verified\r\n");
    serial_write("ginux: Linux proof workload assembled from source\r\n");
    serial_write("ginux: Linux bounded console writes validated\r\n");
    serial_write("ginux: Linux atomic vectored console writes validated\r\n");
    serial_write("ginux: Linux bounded descriptor polling validated\r\n");
    serial_write("ginux: Linux bounded ppoll inputs validated\r\n");
    serial_write("ginux: Linux bounded epoll lifecycle validated\r\n");
    serial_write("ginux: Linux bounded epoll_pwait inputs validated\r\n");
    serial_write("ginux: Linux bounded epoll_pwait2 timeout validated\r\n");
    serial_write("ginux: Linux four-slot auxiliary descriptor table validated\r\n");
    serial_write("ginux: Linux bounded eventfd notification validated\r\n");
    serial_write("ginux: Linux bounded timerfd expiration validated\r\n");
    serial_write("ginux: Linux bounded pipe notification validated\r\n");
    serial_write("ginux: Linux atomic vectored pipe I/O validated\r\n");
    serial_write("ginux: Linux mutable descriptor status flags validated\r\n");
    serial_write("ginux: Linux bounded pipe introspection validated\r\n");
    serial_write("ginux: Linux anonymous descriptor metadata validated\r\n");
    serial_write("ginux: Linux bounded syscall stack movement validated\r\n");
    serial_write("ginux: Linux callable syscall assertion helpers validated\r\n");
    serial_write("ginux: Linux container-local root credentials verified\r\n");
    serial_write("ginux: Linux errno recovery verified in ring 3\r\n");
    serial_write("ginux: Linux single-thread signal mask validated\r\n");
    serial_write("ginux: Linux bounded signal actions validated\r\n");
    serial_write("ginux: Linux synchronous SIGSEGV lifecycle validated\r\n");
    serial_write("ginux: Linux self-directed signal delivery validated\r\n");
    serial_write("ginux: Linux thread-directed signal delivery validated\r\n");
    serial_write("ginux: Linux legacy thread signal delivery validated\r\n");
    serial_write("ginux: Linux pending signal delivery validated\r\n");
    serial_write("ginux: Linux standard signal queue ordering validated\r\n");
    serial_write("ginux: Linux chained pending signal delivery validated\r\n");
    serial_write("ginux: Linux scheduler-backed timed signal wait validated\r\n");
    serial_write("ginux: Linux signal-interrupted timed wait validated\r\n");
    serial_write("ginux: Linux one-shot signal action validated\r\n");
    serial_write("ginux: Linux rt signal frame ABI validated\r\n");
    serial_write("ginux: Linux mutable signal context validated\r\n");
    serial_write("ginux: Linux autodisarming alternate stack validated\r\n");
    serial_write("ginux: Linux alternate signal stack delivery validated\r\n");
    serial_write("ginux: Linux bounded alternate signal stack validated\r\n");
    serial_write("ginux: Linux single-thread process identity verified\r\n");
    serial_write("ginux: Linux initial environment vector validated\r\n");
    serial_write("ginux: Linux credential auxiliary vector validated\r\n");
    serial_write("ginux: Linux clock-tick auxiliary vector validated\r\n");
    serial_write("ginux: Linux platform auxiliary vector validated\r\n");
    serial_write("ginux: Linux conservative hardware capabilities validated\r\n");
    serial_write("ginux: Linux FS base query validated\r\n");
    serial_write("ginux: Linux clear-child-TID exit action validated\r\n");
    serial_write("ginux: Linux robust-list registration validated\r\n");
    serial_write("ginux: Linux scheduler-backed timed futex wait validated\r\n");
    serial_write("ginux: Linux read-only memory transition validated\r\n");
    serial_write("ginux: Linux initial process namespace identity validated\r\n");
    serial_write("ginux: Linux bounded process name state validated\r\n");
    serial_write("ginux: Linux no-new-privileges boundary validated\r\n");
    serial_write("ginux: Linux static executable metadata validated\r\n");
    serial_write("ginux: Linux bounded brk heap validated\r\n");
    serial_write("ginux: Linux bounded anonymous mapping lifecycle validated\r\n");
    serial_write("ginux: Linux two-page anonymous extent validated\r\n");
    serial_write("ginux: multiple bounded Linux mmap areas validated\r\n");
    serial_write("ginux: bounded Linux mmap area splitting validated\r\n");
    serial_write("ginux: Linux anonymous mapping write seal validated\r\n");
    serial_write("ginux: Linux atomic multi-page write seal validated\r\n");
    serial_write("ginux: bounded Linux mmap protection splitting validated\r\n");
    serial_write("ginux: idempotent Linux mmap protection validated\r\n");
    serial_write("ginux: adjacent Linux mmap range operations validated\r\n");
    serial_write("ginux: four-page fragmented Linux mmap window validated\r\n");
    serial_write("ginux: bounded anonymous fixed mmap placement validated\r\n");
    serial_write("ginux: bounded anonymous fixed mmap replacement validated\r\n");
    serial_write("ginux: bounded PROT_NONE fixed replacement validated\r\n");
    serial_write("ginux: bounded read-only anonymous mapping validated\r\n");
    serial_write("ginux: page-rounded Linux mmap lengths validated\r\n");
    serial_write("ginux: Linux PROT_NONE reservations validated\r\n");
    serial_write("ginux: Linux PROT_NONE activation validated\r\n");
    serial_write("ginux: Linux mmap frames allocated on demand\r\n");
    serial_write("ginux: Linux mmap frame leases reclaimed\r\n");
    serial_write("ginux: Linux anonymous mapping discard validated\r\n");
    serial_write("ginux: Linux anonymous mapping residency validated\r\n");
    serial_write("ginux: Linux heap copy bounds validated\r\n");
    serial_write("ginux: Linux stack copy bounds validated\r\n");
    serial_write("ginux: Linux directional VMA copies validated\r\n");
    serial_write("ginux: indexed Linux working directory validated\r\n");
    serial_write("ginux: relative Linux directory changes validated\r\n");
    serial_write("ginux: Linux fchdir descriptor boundary validated\r\n");
    serial_write("ginux: Linux working-directory traversal rejected\r\n");
    serial_write("ginux: Linux proc self executable link validated\r\n");
    serial_write("ginux: Linux proc self executable link-at validated\r\n");
    serial_write("ginux: Linux process umask state validated\r\n");
    serial_write("ginux: Linux disconnected console input validated\r\n");
    serial_write("ginux: Linux console descriptor metadata validated\r\n");
    serial_write("ginux: Linux fixed console geometry validated\r\n");
    serial_write("ginux: Linux fixed console termios validated\r\n");
    serial_write("ginux: Linux non-seekable console validated\r\n");
    serial_write("ginux: Linux console descriptor close validated\r\n");
    serial_write("ginux: Linux console descriptor duplication validated\r\n");
    serial_write("ginux: Linux targeted descriptor duplication validated\r\n");
    serial_write("ginux: Linux console descriptor flags validated\r\n");
    serial_write("ginux: Linux minimum descriptor duplication validated\r\n");
    serial_write("ginux: Linux close-on-exec descriptor state validated\r\n");
    serial_write("ginux: Linux bounded null device validated\r\n");
    serial_write("ginux: Linux bounded null device metadata validated\r\n");
    serial_write("ginux: Linux bounded null device access validated\r\n");
    serial_write("ginux: Linux bounded zero device validated\r\n");
    serial_write("ginux: Linux bounded console device validated\r\n");
    serial_write("ginux: Linux bounded tty alias validated\r\n");
    serial_write("ginux: Linux bounded device statx validated\r\n");
    serial_write("ginux: Linux atomic zero-device readv validated\r\n");
    serial_write("ginux: Linux bounded device writev validated\r\n");
    serial_write("ginux: Linux immutable device directory validated\r\n");
    serial_write("ginux: Linux immutable runtime ELF files validated\r\n");
    serial_write("ginux: Linux auxiliary dependency runtime file validated\r\n");
    serial_write("ginux: Linux read-only runtime file mapping validated\r\n");
    serial_write("ginux: measured-root Linux executable graph validated\r\n");
    serial_write("ginux: measured-root Linux executable graph executed\r\n");
    serial_write("ginux: measured-root Linux file content validated\r\n");
    serial_write("ginux: measured-root Linux file metadata validated\r\n");
    serial_write("ginux: measured-root Linux file mutation denied\r\n");
    serial_write("ginux: measured-root Linux directory enumerated\r\n");
    serial_write("ginux: measured-root Linux directory cursor validated\r\n");
    serial_write("ginux: measured-root Linux root enumerated\r\n");
    serial_write("ginux: measured-root Linux root pagination validated\r\n");
    serial_write("ginux: measured-root Linux empty directory opened\r\n");
    serial_write("ginux: measured-root Linux dirfd lookup validated\r\n");
    serial_write("ginux: measured-root Linux relative metadata validated\r\n");
    serial_write("ginux: indexed measured-root file descriptors validated\r\n");
    serial_write("ginux: generic measured-root Linux file validated\r\n");
    serial_write("ginux: index-driven measured child lookup validated\r\n");
    serial_write("ginux: unlisted measured-root Linux file validated\r\n");
    serial_write("ginux: indexed measured-root directories validated\r\n");
    serial_write("ginux: unlisted measured-root directory opened\r\n");
    serial_write("ginux: nested measured-root dirfd traversal validated\r\n");
    serial_write("ginux: measured-root traversal components rejected\r\n");
    serial_write("ginux: expanded measured-root index capacity validated\r\n");
    serial_write("ginux: Linux saved credential queries validated\r\n");
    serial_write("ginux: Linux supplementary root group validated\r\n");
    serial_write("ginux: Linux fixed execution personality validated\r\n");
    serial_write("ginux: Linux empty capability boundary validated\r\n");
    serial_write("ginux: Linux immutable resource limits validated\r\n");
    serial_write("ginux: Linux single-CPU topology queries validated\r\n");
    serial_write("ginux: Linux single-thread scheduling yield validated\r\n");
    serial_write("ginux: Linux fixed scheduler policy validated\r\n");
    serial_write("ginux: Linux fixed process priority validated\r\n");
    serial_write("ginux: Linux bounded monotonic clocks validated\r\n");
    serial_write("ginux: Linux bounded clock resolution validated\r\n");
    serial_write("ginux: Linux scheduler-backed nanosleep validated\r\n");
    serial_write("ginux: Linux signal-interrupted nanosleep validated\r\n");
    serial_write("ginux: Linux monotonic clock_nanosleep validated\r\n");
    serial_write("ginux: Linux scheduler-backed poll waits validated\r\n");
    serial_write("ginux: Linux scheduler-backed ppoll waits validated\r\n");
    serial_write("ginux: Linux scheduler-backed epoll waits validated\r\n");
    serial_write("ginux: Linux scheduler-backed epoll masked waits validated\r\n");
    serial_write("ginux: Linux signal-interrupted masked waits validated\r\n");
    serial_write("ginux: Linux signal-interrupted epoll waits validated\r\n");
    serial_write("ginux: Linux signal-interrupted poll waits validated\r\n");
    serial_write("ginux: Linux signal-interrupted epoll_wait validated\r\n");
    serial_write("ginux: Linux zero-descriptor poll sleeps validated\r\n");
    serial_write("ginux: Linux scheduler-backed eventfd reads validated\r\n");
    serial_write("ginux: Linux signal-interrupted eventfd reads validated\r\n");
    serial_write("ginux: Linux scheduler-backed eventfd writes validated\r\n");
    serial_write("ginux: Linux signal-interrupted eventfd writes validated\r\n");
    serial_write("ginux: Linux eventfd write snapshot isolation validated\r\n");
    serial_write("ginux: Linux scheduler-backed pipe reads validated\r\n");
    serial_write("ginux: Linux scheduler-backed pipe readv validated\r\n");
    serial_write("ginux: Linux signal-interrupted pipe reads validated\r\n");
    serial_write("ginux: Linux scheduler-backed pipe writes validated\r\n");
    serial_write("ginux: Linux scheduler-backed pipe writev validated\r\n");
    serial_write("ginux: Linux pipe writev snapshot isolation validated\r\n");
    serial_write("ginux: Linux signal-interrupted pipe writes validated\r\n");
    serial_write("ginux: Linux scheduler-backed timerfd reads validated\r\n");
    serial_write("ginux: Linux signal-interrupted timerfd reads validated\r\n");
    serial_write("ginux: APIC-driven Linux wait expiry validated\r\n");
    serial_write("ginux: APIC-driven Linux wait preemption validated\r\n");
    serial_write("ginux: Linux single-task CPU time validated\r\n");
    serial_write("ginux: Linux bounded resource usage validated\r\n");
    serial_write("ginux: Linux disabled interval timers validated\r\n");
    if !ipc::verify_channel_invariants() {
        kernel_fail("\r\nginux: bounded IPC invariant failed\r\n");
    }
    if !packet::verify_invariants() || !packet::install() {
        kernel_fail("\r\nginux: bounded packet handoff invariant failed\r\n");
    }
    serial_write("ginux: bounded packet handoff invariants verified\r\n");
    serial_write("ginux: generation-scoped IPC diagnostics verified\r\n");
    serial_write("ginux: monotonic sequence exhaustion contained\r\n");
    let (
        Some(host_handle),
        Some(console_handle),
        Some(ipc_handle),
        Some(root_handle),
        Some(service_ipc_handle),
    ) = (
        domain::control_plane_boot_handle(0),
        domain::control_plane_boot_handle(1),
        domain::control_plane_boot_handle(2),
        domain::control_plane_boot_handle(4),
        domain::service_boot_handle(),
    )
    else {
        kernel_fail("\r\nginux: control-plane bootstrap handles unavailable\r\n");
    };
    let ipc_rights = capability::Rights::READ.union(capability::Rights::WRITE);
    if domain::control_plane_id() != Some(control_plane)
        || !domain::control_owns_service(control_plane, service_domain)
        || !domain::handle_permits(service_domain, service_ipc_handle, 3, ipc_rights)
        || domain::handle_permits(
            control_plane,
            service_ipc_handle,
            3,
            capability::Rights::READ,
        )
        || domain::handle_permits(service_domain, ipc_handle, 3, capability::Rights::READ)
        || !domain::handle_permits(
            control_plane,
            root_handle,
            control_abi::ROOT_OBJECT,
            capability::Rights::READ,
        )
        || domain::handle_permits(
            control_plane,
            root_handle,
            control_abi::ROOT_OBJECT,
            capability::Rights::WRITE,
        )
    {
        kernel_fail("\r\nginux: cross-domain capability isolation failed\r\n");
    }
    if !address_space::verify_ring3_control_plane(
        control_plane_image.code,
        control_plane_image.entries,
        [control_plane, service_domain],
        [
            [host_handle, console_handle, ipc_handle, root_handle],
            [0, 0, service_ipc_handle, 0],
        ],
        &local_apic,
        &apic_timer,
    ) {
        kernel_fail("\r\nginux: ring-3 control-plane entry failed\r\n");
    }
    if !initramfs::published_root_is_live() {
        kernel_fail("\r\nginux: published root ownership was lost\r\n");
    }
    serial_write("\r\nginux: read-only root capability enforced");
    serial_write("\r\nginux: ring-3 root metadata lookup completed");
    serial_write("\r\nginux: ring-3 root file read completed");
    serial_write("\r\nginux: ring-3 root directory enumeration completed");
    serial_write("\r\nginux: non-directory enumeration denied");
    serial_write("\r\nginux: invalid root userspace requests denied");
    serial_write("\r\nginux: published root remains kernel-owned");
    let Some(expected_fault_instruction) =
        address_space::expected_service_fault_instruction(control_plane_image.entries[1])
    else {
        kernel_fail("\r\nginux: service fault instruction overflowed\r\n");
    };
    if !domain::domain_is_retired(service_domain)
        || !domain::service_fault_was_delivered(service_domain)
        || domain::last_fault_event_audit() != Some((1, service_domain, 1))
        || domain::last_fault_service_identity() != Some(service_identity)
        || domain::primary_service_identity() != Some(service_identity)
        || !domain::delivered_fault_matches(
            service_domain,
            domain::FaultMetadata {
                vector: 14,
                error: 6,
                address: 0,
                instruction: expected_fault_instruction,
            },
        )
        || !arch::x86_64::paging::static_page_tables_are_read_only()
        || !arch::x86_64::paging::mmio_mapping_is_valid(virtio_mmio)
        || !arch::x86_64::pci::bus_master_is_disabled(virtio_block)
        || !arch::x86_64::pci::device_is_reset(virtio_block, virtio_mmio)
        || !arch::x86_64::paging::identity_map_obeys_wx(boot_info, &image)
        || !arch::x86_64::paging::identity_range_is_read_only_data(
            boot_info.manifest as usize,
            boot_info.manifest_size,
        )
        || !arch::x86_64::paging::identity_range_is_read_only_data(
            boot_info.control_plane_bundle as usize,
            boot_info.control_plane_bundle_size,
        )
        || !arch::x86_64::paging::identity_range_is_read_only_data(
            boot_info.root_image as usize,
            boot_info.root_image_size,
        )
        || !arch::x86_64::cpu::execute_disable_enabled()
        || !arch::x86_64::cpu::supervisor_execution_prevention_enabled()
        || !arch::x86_64::cpu::supervisor_access_prevention_enabled()
        || !arch::x86_64::cpu::user_interrupt_surface_is_minimal()
        || !arch::x86_64::cpu::user_mode_instruction_prevention_enabled()
        || !arch::x86_64::cpu::service_restart_stack_was_scrubbed()
        || !arch::x86_64::cpu::service_address_space_frames_were_transferred()
        || !arch::x86_64::paging::verify_user_context_layout_invariants()
        || !domain::call_budget_proof_complete()
        || !domain::thread_budget_proof_complete()
        || !domain::operation_accounting_proof_complete()
        || !domain::service_frame_quarantine_is_clear()
        || domain::handle_permits(
            service_domain,
            service_ipc_handle,
            3,
            capability::Rights::READ,
        )
    {
        kernel_fail("\r\nginux: faulted service authority remained live\r\n");
    }
    let Some(replacement_domain) = domain::last_retired_service_domain() else {
        kernel_fail("\r\nginux: replacement service retirement unavailable\r\n");
    };
    if replacement_domain == service_domain
        || domain::service_boot_handle().is_some()
        || !domain::domain_is_retired(replacement_domain)
        || !domain::service_ready_was_delivered(replacement_domain)
        || !domain::service_exit_was_delivered(replacement_domain)
        || !domain::service_restart_budget_proof_complete()
    {
        kernel_fail("\r\nginux: replacement service generation invariant failed\r\n");
    }
    if ipc::last_message_audit() != Some(((replacement_domain, 4), (control_plane, 4))) {
        kernel_fail("\r\nginux: IPC message provenance invariant failed\r\n");
    }
    if ipc::receive(control_plane) != ipc::Receive::PeerClosed(service_domain)
        || ipc::receive(control_plane) != ipc::Receive::PeerClosed(replacement_domain)
        || ipc::receive(control_plane) != ipc::Receive::Empty
        || ipc::last_closure_audit() != Some((replacement_domain, 2))
    {
        kernel_fail("\r\nginux: sticky IPC peer closure invariant failed\r\n");
    }
    if !address_space::verify_ring3_auxiliary_exit() {
        kernel_fail("\r\nginux: ring-3 auxiliary exit invariant failed\r\n");
    }
    serial_write("\r\nginux: auxiliary exit status sequenced and retired");
    serial_write("\r\nginux: userspace auxiliary creation authority enforced");
    serial_write("\r\nginux: userspace auxiliary stop sequenced and retired");
    serial_write("\r\nginux: failed auxiliary load rollback enforced");
    serial_write("\r\nginux: queued auxiliary cancellation and teardown enforced");
    serial_write("\r\nginux: ready auxiliary cancellation and teardown enforced");
    serial_write("\r\nginux: yielded auxiliary cancellation and teardown enforced");
    serial_write("\r\nginux: userspace auxiliary load and admission enforced");
    serial_write("\r\nginux: private auxiliary payload initialization enforced");
    serial_write("\r\nginux: auxiliary initialization state observable");
    serial_write("\r\nginux: uninitialized auxiliary admission denied");
    serial_write("\r\nginux: explicit auxiliary start authority enforced");
    serial_write("\r\nginux: auxiliary runtime state query enforced");
    serial_write("\r\nginux: userspace auxiliary lifecycle observation enforced");
    serial_write("\r\nginux: userspace auxiliary lifecycle descriptor enforced");
    serial_write("\r\nginux: userspace auxiliary service identity enforced");
    serial_write("\r\nginux: auxiliary management authority denied");
    if !address_space::verify_ring3_network_service() {
        kernel_fail("\r\nginux: ring-3 network service invariant failed\r\n");
    }
    serial_write("\r\nginux: isolated network device probe completed");
    serial_write("\r\nginux: network device command validation enforced");
    serial_write("\r\nginux: network link state configured");
    serial_write("\r\nginux: network IPv4 configuration retained");
    serial_write("\r\nginux: network default gateway validated");
    serial_write("\r\nginux: network configuration generation tracked");
    serial_write("\r\nginux: network gateway invalidated on address change");
    serial_write("\r\nginux: bounded static network route retained");
    serial_write("\r\nginux: coherent network state snapshot copied");
    serial_write("\r\nginux: network device capability revoked on exit");
    if !address_space::verify_ring3_network_observer() {
        kernel_fail("\r\nginux: ring-3 network observer invariant failed\r\n");
    }
    serial_write("\r\nginux: read-only network observer isolated");
    serial_write("\r\nginux: network mutation denied to observer");
    serial_write("\r\nginux: observer capability revoked on exit");
    if !address_space::verify_ring3_network_configurator() {
        kernel_fail("\r\nginux: ring-3 network configurator invariant failed\r\n");
    }
    serial_write("\r\nginux: write-only network configurator isolated");
    serial_write("\r\nginux: network observation denied to configurator");
    serial_write("\r\nginux: denied network copy-out left memory unchanged");
    serial_write("\r\nginux: stale network configuration apply rejected");
    serial_write("\r\nginux: atomic network configuration apply completed");
    serial_write("\r\nginux: configurator capability revoked on exit");
    if !address_space::verify_ring3_network_fault_containment() {
        kernel_fail("\r\nginux: network service fault containment invariant failed\r\n");
    }
    serial_write("\r\nginux: faulted network service contained");
    serial_write("\r\nginux: faulted network authority revoked");
    serial_write("\r\nginux: network state survived service fault");
    if !address_space::verify_ring3_network_recovery() {
        kernel_fail("\r\nginux: replacement network service invariant failed\r\n");
    }
    serial_write("\r\nginux: replacement network service observed retained state");
    serial_write("\r\nginux: replacement network capability lifecycle isolated");
    if !address_space::verify_ring3_volume_service() {
        kernel_fail("\r\nginux: ring-3 persistent volume service invariant failed\r\n");
    }
    serial_write("\r\nginux: write-only volume service committed generation");
    serial_write("\r\nginux: faulted volume service authority revoked");
    serial_write("\r\nginux: persistent volume survived service fault");
    serial_write("\r\nginux: read-only replacement recovered volume");
    serial_write("\r\nginux: volume mutation denied to replacement");
    if !address_space::verify_ring3_auxiliary_fault() {
        kernel_fail("\r\nginux: ring-3 auxiliary fault invariant failed\r\n");
    }
    serial_write("\r\nginux: stop-only auxiliary fault executed and contained");
    if !unsafe { arch::x86_64::timer::verify_device_wait_timeout_interrupt() } {
        kernel_fail("\r\nginux: device-wait timeout interrupt invariant failed\r\n");
    }
    serial_write("\r\nginux: APIC device-wait timeout delivered");
    let Some(virtio_status_physical) = virtio_block.status_register_physical() else {
        kernel_fail("\r\nginux: virtio status register unavailable\r\n");
    };
    let Some(native_volume_device) = address_space::NativeVolumeDevice::new(
        virtio_status_physical,
        virtio_block,
        virtio_mmio,
        virtio_msix_mmio,
        virtio_iommu_profile,
    ) else {
        kernel_fail("\r\nginux: native volume device profile invalid\r\n");
    };
    if !address_space::verify_ring3_read_only_device_grant(virtio_status_physical, 0) {
        kernel_fail("\r\nginux: ring-3 block-device grant invariant failed\r\n");
    }
    if !arch::x86_64::pci::bus_master_is_disabled(virtio_block)
        || !arch::x86_64::pci::device_is_reset(virtio_block, virtio_mmio)
    {
        kernel_fail("\r\nginux: block-device observer changed device state\r\n");
    }
    serial_write("\r\nginux: read-only virtio MMIO grant exercised in ring 3");
    serial_write("\r\nginux: denied virtio MMIO write contained and revoked");
    serial_write("\r\nginux: observed virtio device remained reset and DMA-disabled");
    let native_volume_run = address_space::verify_native_volume_service(
        &volume_image,
        native_volume_device,
        &local_apic,
        None,
    );
    let native_volume_valid = native_volume_run.is_some();
    let native_volume_quiesced = arch::x86_64::pci::quiesce(virtio_block, virtio_mmio);
    if !native_volume_valid || !native_volume_quiesced {
        serial_write("\r\nginux: native volume valid/quiesced: ");
        serial_write_u64(u64::from(native_volume_valid));
        serial_write(" ");
        serial_write_u64(u64::from(native_volume_quiesced));
        kernel_fail("\r\nginux: compiled native volume service invariant failed\r\n");
    }
    let Some(native_volume_run) = native_volume_run else {
        kernel_fail("\r\nginux: native volume outcome unavailable\r\n");
    };
    let native_volume_outcome = native_volume_run.outcome;
    serial_write("\r\nginux: compiled native volume service executed in ring 3");
    serial_write("\r\nginux: native service bootstrap ABI v0 validated in ring 3");
    serial_write("\r\nginux: native service lifecycle ABI v0 clean exit validated in ring 3");
    serial_write("\r\nginux: compiled native service negotiated control ABI v4");
    serial_write("\r\nginux: compiled native service validated control descriptor v4");
    serial_write("\r\nginux: native volume service owned virtio request sequence");
    match native_volume_outcome {
        address_space::NativeVolumeOutcome::Initialized => {
            serial_write("\r\nginux: native volume record initialized and flushed");
            serial_write("\r\nginux: isolated volume prepare flush commit sequence completed");
        }
        address_space::NativeVolumeOutcome::Recovered => {
            serial_write("\r\nginux: native volume record recovered after reboot");
        }
        address_space::NativeVolumeOutcome::Rejected => {
            serial_write("\r\nginux: corrupt native volume media rejected without modification");
        }
        address_space::NativeVolumeOutcome::RestartRequired => {
            kernel_fail("\r\nginux: normal native volume run requested restart\r\n");
        }
        address_space::NativeVolumeOutcome::IncompatibleAbi => {
            kernel_fail("\r\nginux: normal native volume run rejected its ABI\r\n");
        }
        address_space::NativeVolumeOutcome::Faulted => {
            kernel_fail("\r\nginux: normal native volume run reported a fault\r\n");
        }
    }
    serial_write("\r\nginux: native volume service exited cleanly and device quiesced");
    let replacement_volume_run = address_space::verify_native_volume_service(
        &volume_image,
        native_volume_device,
        &local_apic,
        Some(native_volume_run.service_identity),
    );
    let replacement_volume_quiesced = arch::x86_64::pci::quiesce(virtio_block, virtio_mmio);
    let replacement_volume_valid = replacement_volume_run.as_ref().is_some_and(|replacement| {
        replacement.domain != native_volume_run.domain
            && replacement.iommu_lease_is_fresh_after(&native_volume_run)
            && replacement.service_identity == native_volume_run.service_identity
            && replacement.outcome
                == match native_volume_outcome {
                    address_space::NativeVolumeOutcome::Initialized
                    | address_space::NativeVolumeOutcome::Recovered => {
                        address_space::NativeVolumeOutcome::Recovered
                    }
                    address_space::NativeVolumeOutcome::Rejected => {
                        address_space::NativeVolumeOutcome::Rejected
                    }
                    address_space::NativeVolumeOutcome::RestartRequired => {
                        address_space::NativeVolumeOutcome::RestartRequired
                    }
                    address_space::NativeVolumeOutcome::IncompatibleAbi => {
                        address_space::NativeVolumeOutcome::IncompatibleAbi
                    }
                    address_space::NativeVolumeOutcome::Faulted => {
                        address_space::NativeVolumeOutcome::Faulted
                    }
                }
    });
    if !replacement_volume_valid || !replacement_volume_quiesced {
        serial_write("\r\nginux: replacement native volume valid/quiesced: ");
        serial_write_u64(u64::from(replacement_volume_valid));
        serial_write(" ");
        serial_write_u64(u64::from(replacement_volume_quiesced));
        kernel_fail("\r\nginux: replacement native volume service invariant failed\r\n");
    }
    let Some(replacement_volume_run) = replacement_volume_run else {
        kernel_fail("\r\nginux: replacement native volume outcome unavailable\r\n");
    };
    serial_write("\r\nginux: replacement native volume domain generation started");
    serial_write("\r\nginux: native volume service identity survived replacement");
    serial_write(match native_volume_outcome {
        address_space::NativeVolumeOutcome::Initialized
        | address_space::NativeVolumeOutcome::Recovered => {
            "\r\nginux: replacement native volume service recovered exact record"
        }
        address_space::NativeVolumeOutcome::Rejected => {
            "\r\nginux: replacement native volume service repeated fail-closed rejection"
        }
        address_space::NativeVolumeOutcome::RestartRequired => {
            "\r\nginux: replacement native volume service requested restart unexpectedly"
        }
        address_space::NativeVolumeOutcome::IncompatibleAbi => {
            "\r\nginux: replacement native volume service rejected its ABI unexpectedly"
        }
        address_space::NativeVolumeOutcome::Faulted => {
            "\r\nginux: replacement native volume service faulted unexpectedly"
        }
    });
    serial_write("\r\nginux: replacement native volume resources retired cleanly");
    let incompatible_volume_run = address_space::verify_native_volume_service_incompatible_abi(
        &volume_image,
        native_volume_device,
        &local_apic,
        native_volume_run.service_identity,
    );
    let incompatible_volume_quiesced = arch::x86_64::pci::quiesce(virtio_block, virtio_mmio);
    let incompatible_volume_valid = incompatible_volume_run
        .as_ref()
        .is_some_and(|incompatible| {
            incompatible.domain != native_volume_run.domain
                && incompatible.domain != replacement_volume_run.domain
                && incompatible.iommu_lease_is_fresh_after(&replacement_volume_run)
                && incompatible.service_identity == native_volume_run.service_identity
                && incompatible.outcome == address_space::NativeVolumeOutcome::IncompatibleAbi
        });
    if !incompatible_volume_valid || !incompatible_volume_quiesced {
        serial_write("\r\nginux: incompatible native volume valid/quiesced: ");
        serial_write_u64(u64::from(incompatible_volume_valid));
        serial_write(" ");
        serial_write_u64(u64::from(incompatible_volume_quiesced));
        kernel_fail("\r\nginux: incompatible native volume ABI invariant failed\r\n");
    }
    let Some(incompatible_volume_run) = incompatible_volume_run else {
        kernel_fail("\r\nginux: incompatible native volume outcome unavailable\r\n");
    };
    serial_write("\r\nginux: incompatible native service ABI rejected before device activation");
    serial_write("\r\nginux: incompatible native service generation retired cleanly");
    let recovered_after_incompatible = address_space::verify_native_volume_service(
        &volume_image,
        native_volume_device,
        &local_apic,
        Some(native_volume_run.service_identity),
    );
    let recovered_after_incompatible_quiesced =
        arch::x86_64::pci::quiesce(virtio_block, virtio_mmio);
    let recovered_after_incompatible_valid =
        recovered_after_incompatible
            .as_ref()
            .is_some_and(|recovered| {
                recovered.domain != native_volume_run.domain
                    && recovered.domain != replacement_volume_run.domain
                    && recovered.domain != incompatible_volume_run.domain
                    && recovered.iommu_lease_is_fresh_after(&incompatible_volume_run)
                    && recovered.service_identity == native_volume_run.service_identity
                    && recovered.outcome
                        == match native_volume_outcome {
                            address_space::NativeVolumeOutcome::Initialized
                            | address_space::NativeVolumeOutcome::Recovered => {
                                address_space::NativeVolumeOutcome::Recovered
                            }
                            address_space::NativeVolumeOutcome::Rejected => {
                                address_space::NativeVolumeOutcome::Rejected
                            }
                            address_space::NativeVolumeOutcome::RestartRequired => {
                                address_space::NativeVolumeOutcome::RestartRequired
                            }
                            address_space::NativeVolumeOutcome::IncompatibleAbi => {
                                address_space::NativeVolumeOutcome::IncompatibleAbi
                            }
                            address_space::NativeVolumeOutcome::Faulted => {
                                address_space::NativeVolumeOutcome::Faulted
                            }
                        }
            });
    if !recovered_after_incompatible_valid || !recovered_after_incompatible_quiesced {
        serial_write("\r\nginux: post-ABI-rejection native volume valid/quiesced: ");
        serial_write_u64(u64::from(recovered_after_incompatible_valid));
        serial_write(" ");
        serial_write_u64(u64::from(recovered_after_incompatible_quiesced));
        kernel_fail("\r\nginux: post-ABI-rejection recovery invariant failed\r\n");
    }
    let Some(recovered_after_incompatible) = recovered_after_incompatible else {
        kernel_fail("\r\nginux: post-ABI-rejection outcome unavailable\r\n");
    };
    serial_write("\r\nginux: compatible native service recovered after ABI rejection");
    let incompatible_descriptor_run =
        address_space::verify_native_volume_service_incompatible_descriptor(
            &volume_image,
            native_volume_device,
            &local_apic,
            native_volume_run.service_identity,
        );
    let incompatible_descriptor_quiesced = arch::x86_64::pci::quiesce(virtio_block, virtio_mmio);
    let incompatible_descriptor_valid =
        incompatible_descriptor_run
            .as_ref()
            .is_some_and(|incompatible| {
                incompatible.domain != native_volume_run.domain
                    && incompatible.domain != replacement_volume_run.domain
                    && incompatible.domain != incompatible_volume_run.domain
                    && incompatible.domain != recovered_after_incompatible.domain
                    && incompatible.iommu_lease_is_fresh_after(&recovered_after_incompatible)
                    && incompatible.service_identity == native_volume_run.service_identity
                    && incompatible.outcome == address_space::NativeVolumeOutcome::IncompatibleAbi
            });
    if !incompatible_descriptor_valid || !incompatible_descriptor_quiesced {
        serial_write("\r\nginux: incompatible native descriptor valid/quiesced: ");
        serial_write_u64(u64::from(incompatible_descriptor_valid));
        serial_write(" ");
        serial_write_u64(u64::from(incompatible_descriptor_quiesced));
        kernel_fail("\r\nginux: incompatible native descriptor invariant failed\r\n");
    }
    let Some(incompatible_descriptor_run) = incompatible_descriptor_run else {
        kernel_fail("\r\nginux: incompatible native descriptor outcome unavailable\r\n");
    };
    serial_write(
        "\r\nginux: malformed native control descriptor rejected before device activation",
    );
    serial_write("\r\nginux: malformed descriptor service generation retired cleanly");
    let recovered_after_descriptor = address_space::verify_native_volume_service(
        &volume_image,
        native_volume_device,
        &local_apic,
        Some(native_volume_run.service_identity),
    );
    let recovered_after_descriptor_quiesced = arch::x86_64::pci::quiesce(virtio_block, virtio_mmio);
    let recovered_after_descriptor_valid =
        recovered_after_descriptor
            .as_ref()
            .is_some_and(|recovered| {
                recovered.domain != native_volume_run.domain
                    && recovered.domain != replacement_volume_run.domain
                    && recovered.domain != incompatible_volume_run.domain
                    && recovered.domain != recovered_after_incompatible.domain
                    && recovered.domain != incompatible_descriptor_run.domain
                    && recovered.iommu_lease_is_fresh_after(&incompatible_descriptor_run)
                    && recovered.service_identity == native_volume_run.service_identity
                    && recovered.outcome
                        == match native_volume_outcome {
                            address_space::NativeVolumeOutcome::Initialized
                            | address_space::NativeVolumeOutcome::Recovered => {
                                address_space::NativeVolumeOutcome::Recovered
                            }
                            address_space::NativeVolumeOutcome::Rejected => {
                                address_space::NativeVolumeOutcome::Rejected
                            }
                            address_space::NativeVolumeOutcome::RestartRequired => {
                                address_space::NativeVolumeOutcome::RestartRequired
                            }
                            address_space::NativeVolumeOutcome::IncompatibleAbi => {
                                address_space::NativeVolumeOutcome::IncompatibleAbi
                            }
                            address_space::NativeVolumeOutcome::Faulted => {
                                address_space::NativeVolumeOutcome::Faulted
                            }
                        }
            });
    if !recovered_after_descriptor_valid || !recovered_after_descriptor_quiesced {
        serial_write("\r\nginux: post-descriptor-rejection native volume valid/quiesced: ");
        serial_write_u64(u64::from(recovered_after_descriptor_valid));
        serial_write(" ");
        serial_write_u64(u64::from(recovered_after_descriptor_quiesced));
        kernel_fail("\r\nginux: post-descriptor-rejection recovery invariant failed\r\n");
    }
    let Some(recovered_after_descriptor) = recovered_after_descriptor else {
        kernel_fail("\r\nginux: post-descriptor-rejection outcome unavailable\r\n");
    };
    serial_write("\r\nginux: compatible native service recovered after descriptor rejection");
    let faulted_volume_run = address_space::verify_native_volume_service_fault(
        &volume_image,
        native_volume_device,
        &local_apic,
        native_volume_run.service_identity,
    );
    let faulted_volume_quiesced = arch::x86_64::pci::quiesce(virtio_block, virtio_mmio);
    let faulted_volume_valid = faulted_volume_run.as_ref().is_some_and(|faulted| {
        faulted.domain != native_volume_run.domain
            && faulted.domain != replacement_volume_run.domain
            && faulted.domain != incompatible_volume_run.domain
            && faulted.domain != recovered_after_incompatible.domain
            && faulted.domain != incompatible_descriptor_run.domain
            && faulted.domain != recovered_after_descriptor.domain
            && faulted.iommu_lease_is_fresh_after(&recovered_after_descriptor)
            && faulted.service_identity == native_volume_run.service_identity
            && faulted.outcome == address_space::NativeVolumeOutcome::Faulted
    });
    if !faulted_volume_valid || !faulted_volume_quiesced {
        serial_write("\r\nginux: faulted native volume valid/quiesced: ");
        serial_write_u64(u64::from(faulted_volume_valid));
        serial_write(" ");
        serial_write_u64(u64::from(faulted_volume_quiesced));
        kernel_fail("\r\nginux: faulted native volume service invariant failed\r\n");
    }
    let Some(faulted_volume_run) = faulted_volume_run else {
        kernel_fail("\r\nginux: faulted native volume outcome unavailable\r\n");
    };
    serial_write("\r\nginux: compiled native volume service fault contained");
    serial_write("\r\nginux: faulted native device service authority revoked");
    serial_write("\r\nginux: incomplete native DMA scrubbed after fault");
    let recovered_after_fault = address_space::verify_native_volume_service(
        &volume_image,
        native_volume_device,
        &local_apic,
        Some(native_volume_run.service_identity),
    );
    let recovered_after_fault_quiesced = arch::x86_64::pci::quiesce(virtio_block, virtio_mmio);
    let recovered_after_fault_valid = recovered_after_fault.as_ref().is_some_and(|recovered| {
        recovered.domain != native_volume_run.domain
            && recovered.domain != replacement_volume_run.domain
            && recovered.domain != faulted_volume_run.domain
            && recovered.iommu_lease_is_fresh_after(&faulted_volume_run)
            && recovered.service_identity == native_volume_run.service_identity
            && recovered.outcome
                == match native_volume_outcome {
                    address_space::NativeVolumeOutcome::Initialized
                    | address_space::NativeVolumeOutcome::Recovered => {
                        address_space::NativeVolumeOutcome::Recovered
                    }
                    address_space::NativeVolumeOutcome::Rejected => {
                        address_space::NativeVolumeOutcome::Rejected
                    }
                    address_space::NativeVolumeOutcome::RestartRequired => {
                        address_space::NativeVolumeOutcome::RestartRequired
                    }
                    address_space::NativeVolumeOutcome::IncompatibleAbi => {
                        address_space::NativeVolumeOutcome::IncompatibleAbi
                    }
                    address_space::NativeVolumeOutcome::Faulted => {
                        address_space::NativeVolumeOutcome::Faulted
                    }
                }
    });
    if !recovered_after_fault_valid || !recovered_after_fault_quiesced {
        serial_write("\r\nginux: post-fault native volume valid/quiesced: ");
        serial_write_u64(u64::from(recovered_after_fault_valid));
        serial_write(" ");
        serial_write_u64(u64::from(recovered_after_fault_quiesced));
        kernel_fail("\r\nginux: post-fault native volume recovery invariant failed\r\n");
    }
    let Some(recovered_after_fault) = recovered_after_fault else {
        kernel_fail("\r\nginux: post-fault native volume outcome unavailable\r\n");
    };
    serial_write("\r\nginux: post-fault native volume generation started");
    serial_write("\r\nginux: native volume identity survived actual driver fault");
    serial_write(match native_volume_outcome {
        address_space::NativeVolumeOutcome::Initialized
        | address_space::NativeVolumeOutcome::Recovered => {
            "\r\nginux: post-fault native volume service recovered exact record"
        }
        address_space::NativeVolumeOutcome::Rejected => {
            "\r\nginux: post-fault native volume service preserved rejection"
        }
        address_space::NativeVolumeOutcome::RestartRequired => {
            "\r\nginux: post-fault native volume service requested restart unexpectedly"
        }
        address_space::NativeVolumeOutcome::IncompatibleAbi => {
            "\r\nginux: post-fault native volume service rejected its ABI unexpectedly"
        }
        address_space::NativeVolumeOutcome::Faulted => {
            "\r\nginux: post-fault native volume service faulted unexpectedly"
        }
    });
    serial_write("\r\nginux: post-fault native volume resources retired cleanly");
    let cancelled_volume_run = address_space::verify_native_volume_service_cancellation(
        &volume_image,
        native_volume_device,
        &local_apic,
        native_volume_run.service_identity,
    );
    let cancelled_volume_quiesced = arch::x86_64::pci::quiesce(virtio_block, virtio_mmio);
    let cancelled_volume_valid = cancelled_volume_run.as_ref().is_some_and(|cancelled| {
        cancelled.domain != native_volume_run.domain
            && cancelled.domain != replacement_volume_run.domain
            && cancelled.domain != faulted_volume_run.domain
            && cancelled.domain != recovered_after_fault.domain
            && cancelled.iommu_lease_is_fresh_after(&recovered_after_fault)
            && cancelled.service_identity == native_volume_run.service_identity
            && cancelled.outcome == address_space::NativeVolumeOutcome::RestartRequired
    });
    if !cancelled_volume_valid || !cancelled_volume_quiesced {
        serial_write("\r\nginux: cancelled native volume valid/quiesced: ");
        serial_write_u64(u64::from(cancelled_volume_valid));
        serial_write(" ");
        serial_write_u64(u64::from(cancelled_volume_quiesced));
        kernel_fail("\r\nginux: native volume cancellation invariant failed\r\n");
    }
    let Some(cancelled_volume_run) = cancelled_volume_run else {
        kernel_fail("\r\nginux: cancelled native volume outcome unavailable\r\n");
    };
    serial_write("\r\nginux: notified native volume timeout cancelled safely");
    serial_write("\r\nginux: cancelled native volume request generation retired");
    let recovered_after_cancellation = address_space::verify_native_volume_service(
        &volume_image,
        native_volume_device,
        &local_apic,
        Some(native_volume_run.service_identity),
    );
    let recovered_after_cancellation_quiesced =
        arch::x86_64::pci::quiesce(virtio_block, virtio_mmio);
    let recovered_after_cancellation_valid =
        recovered_after_cancellation
            .as_ref()
            .is_some_and(|recovered| {
                recovered.domain != native_volume_run.domain
                    && recovered.domain != replacement_volume_run.domain
                    && recovered.domain != faulted_volume_run.domain
                    && recovered.domain != recovered_after_fault.domain
                    && recovered.domain != cancelled_volume_run.domain
                    && recovered.iommu_lease_is_fresh_after(&cancelled_volume_run)
                    && recovered.service_identity == native_volume_run.service_identity
                    && recovered.outcome
                        == match native_volume_outcome {
                            address_space::NativeVolumeOutcome::Initialized
                            | address_space::NativeVolumeOutcome::Recovered => {
                                address_space::NativeVolumeOutcome::Recovered
                            }
                            address_space::NativeVolumeOutcome::Rejected => {
                                address_space::NativeVolumeOutcome::Rejected
                            }
                            address_space::NativeVolumeOutcome::RestartRequired => {
                                address_space::NativeVolumeOutcome::RestartRequired
                            }
                            address_space::NativeVolumeOutcome::IncompatibleAbi => {
                                address_space::NativeVolumeOutcome::IncompatibleAbi
                            }
                            address_space::NativeVolumeOutcome::Faulted => {
                                address_space::NativeVolumeOutcome::Faulted
                            }
                        }
            });
    if !recovered_after_cancellation_valid || !recovered_after_cancellation_quiesced {
        serial_write("\r\nginux: post-cancellation native volume valid/quiesced: ");
        serial_write_u64(u64::from(recovered_after_cancellation_valid));
        serial_write(" ");
        serial_write_u64(u64::from(recovered_after_cancellation_quiesced));
        kernel_fail("\r\nginux: post-cancellation native volume invariant failed\r\n");
    }
    if !address_space::verify_native_network_service(&network_image) {
        kernel_fail("\r\nginux: compiled native network service invariant failed\r\n");
    }
    serial_write("\r\nginux: compiled native network service executed in ring 3");
    serial_write("\r\nginux: native network service negotiated control ABI v4");
    serial_write("\r\nginux: native network service validated control descriptor v4");
    serial_write("\r\nginux: native network service started without boot capabilities");
    serial_write("\r\nginux: native network service exited through lifecycle ABI v0");
    serial_write("\r\nginux: native network service address space retired cleanly");
    if !address_space::verify_native_network_bootstrap(
        &network_image,
        virtio_network,
        virtio_network_initialization,
        virtio_network_iommu_profile,
    ) {
        kernel_fail("\r\nginux: native network bootstrap invariant failed\r\n");
    }
    serial_write("\r\nginux: native network bootstrap ABI v0 validated in ring 3");
    serial_write("\r\nginux: native network read capability exercised in ring 3");
    serial_write("\r\nginux: native network DMA lease remained device-inaccessible");
    serial_write("\r\nginux: native network MMIO remained PCI-disabled");
    serial_write("\r\nginux: native network bootstrap resources retired cleanly");
    if !address_space::verify_native_network_bootstrap_rejection(
        &network_image,
        virtio_network,
        virtio_network_initialization,
        virtio_network_iommu_profile,
    ) {
        kernel_fail("\r\nginux: malformed native network bootstrap invariant failed\r\n");
    }
    serial_write("\r\nginux: malformed native network bootstrap rejected in ring 3");
    serial_write("\r\nginux: malformed network generation fault contained");
    serial_write("\r\nginux: malformed network capability revoked");
    serial_write("\r\nginux: malformed network resources retired before device activation");
    if !address_space::verify_native_network_dma_confinement(
        &network_image,
        virtio_network,
        virtio_network_initialization,
        virtio_network_iommu_profile,
    ) {
        kernel_fail("\r\nginux: native network DMA confinement invariant failed\r\n");
    }
    serial_write("\r\nginux: native network generation received exact VT-d domain");
    serial_write("\r\nginux: native network VT-d activated before PCI authority");
    serial_write("\r\nginux: native network bus mastering remained disabled");
    serial_write("\r\nginux: native network VT-d retired before DMA release");
    serial_write("\r\nginux: native network translation tables scrubbed cleanly");
    if !address_space::verify_native_network_dma_fault_teardown(
        &network_image,
        virtio_network,
        virtio_network_initialization,
        virtio_network_iommu_profile,
    ) {
        kernel_fail("\r\nginux: faulted native network VT-d teardown failed\r\n");
    }
    serial_write("\r\nginux: faulted native network generation retired active VT-d");
    serial_write("\r\nginux: faulted network translation disabled before DMA scrub");
    serial_write("\r\nginux: faulted network VT-d tables released cleanly");
    if !address_space::verify_native_network_empty_queue_activation(
        &network_image,
        virtio_network,
        virtio_network_initialization,
        virtio_network_iommu_profile,
        virtio_network_common_mmio,
    ) {
        kernel_fail("\r\nginux: native network empty-queue delegation failed\r\n");
    }
    serial_write("\r\nginux: native network service configured empty RX/TX queues");
    serial_write("\r\nginux: native network queue layout validated by kernel");
    serial_write("\r\nginux: native network bus mastering authorized by capability");
    serial_write("\r\nginux: native network bus mastering enabled last");
    serial_write("\r\nginux: native network device quiesced before VT-d teardown");
    serial_write("\r\nginux: native network empty-queue generation retired cleanly");
    if !address_space::verify_native_network_active_fault_teardown(
        &network_image,
        virtio_network,
        virtio_network_initialization,
        virtio_network_iommu_profile,
        virtio_network_common_mmio,
    ) {
        kernel_fail("\r\nginux: active native network fault teardown failed\r\n");
    }
    serial_write("\r\nginux: active native network driver fault injected at call boundary");
    serial_write("\r\nginux: faulted native network queues were live before quiescence");
    serial_write("\r\nginux: faulted native network bus mastering revoked");
    serial_write("\r\nginux: faulted native network device reset before VT-d teardown");
    serial_write("\r\nginux: active network fault retired translation before DMA");
    serial_write("\r\nginux: active network fault contained without kernel panic");
    if !address_space::verify_native_network_transmit_probe(
        &network_image,
        virtio_network,
        virtio_network_initialization,
        virtio_network_iommu_profile,
        virtio_network_common_mmio,
        virtio_network_msix_mmio,
        &local_apic,
    ) {
        kernel_fail("\r\nginux: native network service transmit failed\r\n");
    }
    serial_write("\r\nginux: native network service published bounded TX descriptor");
    serial_write("\r\nginux: native network service notified transmit queue");
    serial_write("\r\nginux: native network service blocked for TX interrupt");
    serial_write("\r\nginux: native network TX replay rejected");
    serial_write("\r\nginux: kernel validated native network TX descriptor and payload");
    serial_write("\r\nginux: native network TX payload guards preserved");
    serial_write("\r\nginux: native network service transmitted Ethernet probe");
    serial_write("\r\nginux: native network TX interrupt route retired cleanly");
    if !address_space::verify_native_network_transmit_timeout(
        &network_image,
        virtio_network,
        virtio_network_initialization,
        virtio_network_iommu_profile,
        virtio_network_common_mmio,
        virtio_network_msix_mmio,
        &local_apic,
    ) {
        kernel_fail("\r\nginux: native network notified timeout was not contained\r\n");
    }
    serial_write("\r\nginux: notified native network timeout cancelled safely");
    serial_write("\r\nginux: ambiguous native network generation retired");
    if !address_space::verify_native_network_transmit_probe(
        &network_image,
        virtio_network,
        virtio_network_initialization,
        virtio_network_iommu_profile,
        virtio_network_common_mmio,
        virtio_network_msix_mmio,
        &local_apic,
    ) {
        kernel_fail("\r\nginux: native network timeout recovery failed\r\n");
    }
    serial_write("\r\nginux: fresh native network generation recovered after timeout");
    serial_write("\r\nginux: recovered network generation completed TX interrupt");
    if !address_space::verify_native_network_receive_arp(
        &network_image,
        virtio_network,
        virtio_network_initialization,
        virtio_network_iommu_profile,
        virtio_network_common_mmio,
        virtio_network_msix_mmio,
        &local_apic,
    ) {
        kernel_fail("\r\nginux: native network service ARP receive failed\r\n");
    }
    serial_write("\r\nginux: native network service published bounded RX descriptor");
    serial_write("\r\nginux: native network service transmitted ARP request");
    serial_write("\r\nginux: native network service blocked for RX interrupt");
    serial_write("\r\nginux: native network RX replay rejected");
    serial_write("\r\nginux: native network service validated gateway ARP reply");
    serial_write("\r\nginux: kernel validated native network RX descriptor and ARP reply");
    serial_write("\r\nginux: native network RX payload guards preserved");
    serial_write("\r\nginux: native network TX and RX interrupt routes retired cleanly");
    if !address_space::verify_native_network_receive_timeout(
        &network_image,
        virtio_network,
        virtio_network_initialization,
        virtio_network_iommu_profile,
        virtio_network_common_mmio,
        virtio_network_msix_mmio,
        &local_apic,
    ) {
        kernel_fail("\r\nginux: native network RX timeout was not contained\r\n");
    }
    serial_write("\r\nginux: native network RX timeout followed completed TX");
    serial_write("\r\nginux: ambiguous native network RX generation retired");
    if !address_space::verify_native_network_receive_arp(
        &network_image,
        virtio_network,
        virtio_network_initialization,
        virtio_network_iommu_profile,
        virtio_network_common_mmio,
        virtio_network_msix_mmio,
        &local_apic,
    ) {
        kernel_fail("\r\nginux: native network RX timeout recovery failed\r\n");
    }
    serial_write("\r\nginux: fresh native network generation recovered after RX timeout");
    serial_write("\r\nginux: recovered network generation validated gateway ARP reply");
    if !address_space::verify_native_network_malformed_receive(
        &network_image,
        virtio_network,
        virtio_network_initialization,
        virtio_network_iommu_profile,
        virtio_network_common_mmio,
        virtio_network_msix_mmio,
        &local_apic,
    ) {
        kernel_fail("\r\nginux: malformed native network receive was not contained\r\n");
    }
    serial_write("\r\nginux: malformed network ARP reply injected after RX completion");
    serial_write("\r\nginux: native network service rejected malformed ARP reply");
    serial_write("\r\nginux: malformed network receive generation retired safely");
    if !address_space::verify_native_network_receive_arp(
        &network_image,
        virtio_network,
        virtio_network_initialization,
        virtio_network_iommu_profile,
        virtio_network_common_mmio,
        virtio_network_msix_mmio,
        &local_apic,
    ) {
        kernel_fail("\r\nginux: malformed native network receive recovery failed\r\n");
    }
    serial_write("\r\nginux: fresh native network generation recovered after malformed reply");
    serial_write("\r\nginux: post-malformation network generation validated ARP reply");
    if !address_space::verify_native_network_receive_reuse(
        &network_image,
        virtio_network,
        virtio_network_initialization,
        virtio_network_iommu_profile,
        virtio_network_common_mmio,
        virtio_network_msix_mmio,
        &local_apic,
    ) {
        kernel_fail("\r\nginux: native network receive replenishment failed\r\n");
    }
    serial_write("\r\nginux: native network service replenished RX descriptor");
    serial_write("\r\nginux: native network service completed second ARP exchange");
    serial_write("\r\nginux: native network request generations one through four completed");
    serial_write("\r\nginux: kernel validated both native network ARP exchanges");
    if !address_space::verify_native_network_packet_handoff(
        &network_image,
        &protocol_image,
        virtio_network,
        virtio_network_initialization,
        virtio_network_iommu_profile,
        (
            virtio_network_common_mmio,
            virtio_network_msix_mmio,
            &local_apic,
        ),
    ) {
        kernel_fail("\r\nginux: bounded native packet handoff failed\r\n");
    }
    serial_write("\r\nginux: native network service copied validated frame out of DMA");
    serial_write("\r\nginux: packet producer retained no read authority");
    serial_write("\r\nginux: compiled Rust protocol service consumed Ethernet frame");
    serial_write("\r\nginux: protocol service validated gateway ARP reply");
    serial_write("\r\nginux: protocol service rejected malformed ARP metadata");
    serial_write("\r\nginux: protocol consumer retained no device or write authority");
    serial_write("\r\nginux: packet survived producer and device retirement");
    serial_write("\r\nginux: protocol service address space retired cleanly");
    if !address_space::verify_native_network_packet_fault_handoff(
        &network_image,
        virtio_network,
        virtio_network_initialization,
        virtio_network_iommu_profile,
        virtio_network_common_mmio,
        virtio_network_msix_mmio,
        &local_apic,
    ) {
        kernel_fail("\r\nginux: faulted native packet handoff failed\r\n");
    }
    serial_write("\r\nginux: network driver faulted after packet publication");
    serial_write("\r\nginux: faulted packet producer authority revoked");
    serial_write("\r\nginux: fault teardown preserved copied packet sequence two");
    serial_write("\r\nginux: fresh ring-3 consumer received fault-surviving packet");
    if !address_space::verify_native_network_packet_batch(
        &network_image,
        virtio_network,
        virtio_network_initialization,
        virtio_network_iommu_profile,
        virtio_network_common_mmio,
        virtio_network_msix_mmio,
        &local_apic,
    ) {
        kernel_fail("\r\nginux: bounded native packet FIFO failed\r\n");
    }
    serial_write("\r\nginux: network producer queued two packets before retirement");
    serial_write("\r\nginux: packet FIFO preserved sequence three before four");
    serial_write("\r\nginux: separate ring-3 consumers drained packet FIFO in order");
    serial_write("\r\nginux: bounded packet FIFO backpressure invariant verified");
    if !address_space::verify_packet_wait_wakeup(false) {
        kernel_fail("\r\nginux: blocking packet wait failed\r\n");
    }
    serial_write("\r\nginux: empty packet consumer blocked on exact sequence");
    serial_write("\r\nginux: write-only packet producer wait denied");
    serial_write("\r\nginux: packet publication woke exactly one read-only consumer");
    serial_write("\r\nginux: stale packet waits rejected without blocking");
    serial_write("\r\nginux: woken packet consumer received exact frame");
    if !address_space::verify_packet_wait_cancellation() {
        kernel_fail("\r\nginux: blocked packet wait cancellation failed\r\n");
    }
    serial_write("\r\nginux: blocked packet consumer stopped by control authority");
    serial_write("\r\nginux: cancelled packet wait key released");
    serial_write("\r\nginux: cancelled packet consumer frames and capability retired");
    if !address_space::verify_packet_wait_wakeup(false) {
        kernel_fail("\r\nginux: replacement packet wait failed\r\n");
    }
    serial_write("\r\nginux: replacement packet consumer claimed cancelled sequence");
    serial_write("\r\nginux: replacement packet consumer received exact frame");
    if !address_space::verify_packet_wait_timeout() {
        kernel_fail("\r\nginux: bounded packet wait timeout failed\r\n");
    }
    serial_write("\r\nginux: bounded packet wait expired through APIC deadline");
    serial_write("\r\nginux: packet timeout preempted non-cooperative peer");
    serial_write("\r\nginux: timed-out packet wait key released");
    if !address_space::verify_packet_wait_wakeup(true) {
        kernel_fail("\r\nginux: post-timeout packet wait replacement failed\r\n");
    }
    serial_write("\r\nginux: replacement packet consumer claimed timed-out sequence");
    serial_write("\r\nginux: replacement packet consumer received post-timeout frame");
    serial_write("\r\nginux: packet publication cancelled pending wait deadline");
    if !address_space::verify_packet_backpressure_wakeup() {
        kernel_fail("\r\nginux: packet backpressure wakeup failed\r\n");
    }
    serial_write("\r\nginux: full packet FIFO blocked write-only producer");
    serial_write("\r\nginux: packet consumption woke exactly one blocked producer");
    serial_write("\r\nginux: woken packet producer retried exact next sequence");
    serial_write("\r\nginux: packet FIFO ordering survived backpressure wakeup");
    if !address_space::verify_native_protocol_wait_loop(&protocol_image) {
        kernel_fail("\r\nginux: resident native protocol wait loop failed\r\n");
    }
    serial_write("\r\nginux: compiled protocol service waited on an empty packet FIFO");
    serial_write("\r\nginux: resident protocol domain consumed two ordered packets");
    serial_write("\r\nginux: protocol domain remained live between packet arrivals");
    serial_write("\r\nginux: protocol service learned bounded ARP neighbor");
    serial_write("\r\nginux: protocol service refreshed stable ARP neighbor");
    serial_write("\r\nginux: conflicting ARP neighbor mapping rejected without mutation");
    serial_write("\r\nginux: resident protocol wait keys and frames retired cleanly");
    if !address_space::verify_supervised_native_protocol_loop(&protocol_image) {
        kernel_fail("\r\nginux: supervised native protocol loop failed\r\n");
    }
    serial_write("\r\nginux: supervised protocol loop consumed multiple packets");
    serial_write("\r\nginux: control plane stopped blocked protocol service");
    serial_write("\r\nginux: supervised protocol wait and frames retired cleanly");
    if !address_space::verify_native_protocol_ipv4_dispatch(&protocol_image) {
        kernel_fail("\r\nginux: native protocol IPv4 dispatch failed\r\n");
    }
    serial_write("\r\nginux: protocol service dispatched ARP then IPv4");
    serial_write("\r\nginux: protocol service validated IPv4 header checksum");
    serial_write("\r\nginux: malformed IPv4 header rejected in ring 3");
    serial_write("\r\nginux: IPv4 protocol generation retired cleanly");
    if !address_space::verify_native_protocol_icmp_echo(&protocol_image) {
        kernel_fail("\r\nginux: native protocol ICMP echo validation failed\r\n");
    }
    serial_write("\r\nginux: protocol service validated ICMP echo request");
    serial_write("\r\nginux: ICMP type code identifier and sequence accepted");
    serial_write("\r\nginux: malformed ICMP checksum rejected in ring 3");
    serial_write("\r\nginux: ICMP protocol generation retired cleanly");
    if !address_space::verify_native_protocol_icmp_reply(&protocol_image) {
        kernel_fail("\r\nginux: native protocol ICMP echo reply failed\r\n");
    }
    serial_write("\r\nginux: protocol service constructed exact ICMP echo reply");
    serial_write("\r\nginux: protocol egress authority remained write only");
    serial_write("\r\nginux: isolated egress consumer retained read-only authority");
    serial_write("\r\nginux: egress packet survived protocol service retirement");
    serial_write("\r\nginux: isolated egress consumer received exact echo reply");
    serial_write("\r\nginux: protocol egress sequence consumed exactly once");
    serial_write("\r\nginux: replenished network buffer guards preserved");
    serial_write("\r\nginux: replenished network interrupt routes retired cleanly");
    serial_write("\r\nginux: post-cancellation native volume generation started");
    serial_write(match native_volume_outcome {
        address_space::NativeVolumeOutcome::Initialized
        | address_space::NativeVolumeOutcome::Recovered => {
            "\r\nginux: post-cancellation native volume recovered exact record"
        }
        address_space::NativeVolumeOutcome::Rejected => {
            "\r\nginux: post-cancellation native volume preserved rejection"
        }
        address_space::NativeVolumeOutcome::RestartRequired => {
            "\r\nginux: post-cancellation native volume requested restart unexpectedly"
        }
        address_space::NativeVolumeOutcome::IncompatibleAbi => {
            "\r\nginux: post-cancellation native volume rejected its ABI unexpectedly"
        }
        address_space::NativeVolumeOutcome::Faulted => {
            "\r\nginux: post-cancellation native volume faulted unexpectedly"
        }
    });
    serial_write("\r\nginux: post-cancellation native resources retired cleanly");
    serial_write("\r\nginux: every native volume generation used an active IOMMU domain");
    serial_write("\r\nginux: native volume generations received fresh IOMMU leases");
    serial_write("\r\nginux: native volume IOMMU retired after device quiescence");
    serial_write("\r\nginux: fault and timeout paths retired native volume IOMMU state");
    serial_write("\r\nginux: native volume write-only grant exercised in ring 3");
    serial_write("\r\nginux: isolated virtio feature negotiation completed");
    serial_write("\r\nginux: isolated virtio queue geometry configured with owned DMA");
    serial_write("\r\nginux: isolated DMA lease initialized in ring 3");
    serial_write("\r\nginux: isolated virtio queue bound to inactive DMA lease");
    serial_write("\r\nginux: isolated virtio queue activated before publication");
    serial_write("\r\nginux: isolated virtio read descriptors prepared in ring 3");
    serial_write("\r\nginux: isolated virtio available entry published in ring 3");
    serial_write("\r\nginux: stale isolated virtio wait generation rejected");
    serial_write("\r\nginux: isolated virtio queue notified from ring 3");
    serial_write("\r\nginux: isolated virtio service blocked on MSI-X");
    serial_write("\r\nginux: kernel-owned MSI-X woke isolated virtio service");
    serial_write("\r\nginux: isolated virtio timeout cancelled after MSI-X");
    serial_write("\r\nginux: isolated virtio live timeout retried successfully");
    serial_write("\r\nginux: duplicate isolated virtio wait rejected");
    serial_write("\r\nginux: isolated volume slots scanned in ring 3");
    serial_write("\r\nginux: isolated virtio request generation advanced and reused");
    serial_write("\r\nginux: completed native volume service authority revoked");
    serial_write("\r\nginux: virtio DMA validation followed hardware quiescence");
    serial_write("\r\nginux: clean exit quiesced device before DMA release");
    serial_write("\r\nginux: retired DMA lease scrubbed and released");
    serial_write("\r\nginux: native volume device quiesced before release");
    serial_write("\r\nginux: isolated virtio MSI-X route retired before DMA release");
    serial_write("\r\nginux: opaque capability handles enforced at call gate");
    serial_write("\r\nginux: control call ABI v4 negotiated");
    serial_write("\r\nginux: userspace console capability enforced");
    serial_write("\r\nginux: minimal user interrupt surface enforced");
    serial_write("\r\nginux: bounded userspace read enforced");
    serial_write("\r\nginux: user copies bound to owned mappings");
    serial_write("\r\nginux: page-granular user range validation enforced");
    serial_write("\r\nginux: structured userspace ABI result verified");
    serial_write("\r\nginux: ring-3 fault survivor resumed and exited");
    serial_write("\r\nginux: user page fault contained");
    serial_write("\r\nginux: isolated scheduler address spaces switched");
    serial_write("\r\nginux: scheduler CR3 roots validated");
    serial_write("\r\nginux: bounded user page-table hierarchy validated");
    serial_write("\r\nginux: address-space resource contract shared");
    serial_write("\r\nginux: scheduler address-space ownership validated");
    serial_write("\r\nginux: scheduler user mappings validated");
    serial_write("\r\nginux: one-shot user preemption completed");
    serial_write("\r\nginux: restarted service preemption completed");
    serial_write("\r\nginux: completed address-space frames reclaimed");
    serial_write("\r\nginux: retired domain frame ownership cleared");
    serial_write("\r\nginux: retired service frame quarantine cleared");
    serial_write("\r\nginux: capability-gated bounded IPC verified");
    serial_write("\r\nginux: directional IPC endpoints isolated");
    serial_write("\r\nginux: generation-bound IPC endpoints verified");
    serial_write("\r\nginux: generation-tagged IPC messages verified");
    serial_write("\r\nginux: monotonic queued IPC sequences verified");
    serial_write("\r\nginux: structured IPC delivery provenance verified");
    serial_write("\r\nginux: IPC diagnostic authority enforced");
    serial_write("\r\nginux: blocking IPC direct wake completed");
    serial_write("\r\nginux: direct IPC wake provenance verified");
    serial_write("\r\nginux: sequenced direct IPC delivery verified");
    serial_write("\r\nginux: IPC wait-key matching enforced");
    serial_write("\r\nginux: blocked IPC peer-close cleanup verified");
    serial_write("\r\nginux: sticky IPC peer closure delivered");
    serial_write("\r\nginux: monotonic IPC peer closures verified");
    serial_write("\r\nginux: isolated service request-reply completed");
    serial_write("\r\nginux: service domain authority restricted to IPC");
    serial_write("\r\nginux: faulted service domain authority retired");
    serial_write("\r\nginux: service fault event delivered to control plane");
    serial_write("\r\nginux: service fault event generation matched");
    serial_write("\r\nginux: monotonic lifecycle events verified");
    serial_write("\r\nginux: structured lifecycle event delivered");
    serial_write("\r\nginux: structured lifecycle service identity delivered");
    serial_write("\r\nginux: lifecycle diagnostic authority enforced");
    serial_write("\r\nginux: service fault metadata matched");
    serial_write("\r\nginux: structured service fault delivered");
    serial_write("\r\nginux: service fault diagnostic authority enforced");
    serial_write("\r\nginux: service domain generation reuse verified");
    serial_write("\r\nginux: stable service identity survived restart");
    serial_write("\r\nginux: service restart budget exhausted");
    serial_write("\r\nginux: service restart rollback invariants verified");
    serial_write("\r\nginux: failed service stack scrubbed before restart");
    serial_write("\r\nginux: service frame ownership transferred to replacement");
    serial_write("\r\nginux: replacement service context executed");
    serial_write("\r\nginux: replacement service request-reply completed");
    serial_write("\r\nginux: volatile storage service round-trip completed");
    serial_write("\r\nginux: volatile storage invalid command rejected");
    serial_write("\r\nginux: replacement service readiness delivered");
    serial_write("\r\nginux: replacement service clean exit delivered");
    serial_write("\r\nginux: per-domain syscall budgets enforced");
    serial_write("\r\nginux: per-domain thread budgets enforced");
    serial_write("\r\nginux: in-flight domain operations drained");
    if !scheduler::verify_scheduler_invariants() {
        kernel_fail("\r\nginux: scheduler lifecycle invariant failed\r\n");
    }
    if !arch::x86_64::cpu::verify_user_fs_base_installation() {
        kernel_fail("\r\nginux: per-thread FS base installation failed\r\n");
    }
    serial_write("\r\nginux: scheduler context lifecycle verified");
    serial_write("\r\nginux: preemptible user context flags enforced");
    serial_write("\r\nginux: per-thread FS base context preserved");
    if !crypto::verify_sha256_known_answers() {
        kernel_fail("\r\nginux: bootstrap SHA-256 self-test failed\r\n");
    }
    serial_write("\r\nginux: bootstrap SHA-256 known answers verified");
    serial_write("\r\nGINUX_KERNEL_OK\r\n");

    (boot_info.reset_system)(EFI_RESET_SHUTDOWN, EFI_SUCCESS, 0, core::ptr::null());
    kernel_fail("ginux: firmware shutdown returned unexpectedly\r\n");
}

fn validate_acpi_rsdp(rsdp: *const u8) -> Option<usize> {
    const RSDP_V1_SIZE: usize = 20;
    const RSDP_V2_SIZE: usize = 36;
    const RSDP_SIGNATURE: &[u8; 8] = b"RSD PTR ";

    if rsdp.is_null() || rsdp as usize >= arch::x86_64::paging::INITIAL_IDENTITY_LIMIT {
        return None;
    }
    // SAFETY: The UEFI configuration table supplied this physical pointer. We
    // first inspect only the fixed ACPI 1.0 prefix, then bound any extended
    // length before reading it.
    unsafe {
        for (offset, expected) in RSDP_SIGNATURE.iter().enumerate() {
            if rsdp.add(offset).read() != *expected {
                return None;
            }
        }
        if checksum(rsdp, RSDP_V1_SIZE) != 0 {
            return None;
        }
        let revision = rsdp.add(15).read();
        let length = if revision >= 2 {
            let length = core::ptr::read_unaligned(rsdp.add(20).cast::<u32>()) as usize;
            if length < RSDP_V2_SIZE {
                return None;
            }
            length
        } else {
            RSDP_V1_SIZE
        };
        let end = (rsdp as usize).checked_add(length.checked_sub(1)?)?;
        if end >= arch::x86_64::paging::INITIAL_IDENTITY_LIMIT || checksum(rsdp, length) != 0 {
            return None;
        }
        Some(end)
    }
}

unsafe fn checksum(bytes: *const u8, length: usize) -> u8 {
    let mut sum = 0_u8;
    for offset in 0..length {
        sum = sum.wrapping_add(bytes.add(offset).read());
    }
    sum
}

fn verify_initial_frame_allocation(boot_info: &BootInfo) -> bool {
    if !memory::initialize(
        boot_info,
        arch::x86_64::paging::INITIAL_IDENTITY_LIMIT as u64,
    ) {
        return false;
    }
    if !memory::verify_reservation_invariants() {
        return false;
    }
    if memory::reclaimed_boot_service_frame_count().is_none_or(|count| count == 0) {
        return false;
    }
    let Some([first, second]) = memory::allocate_frames::<2>(memory::FrameOwner::KERNEL) else {
        return false;
    };
    let first_address = first.address();
    let second_address = second.address();
    if first_address == second_address || first_address % 4096 != 0 || second_address % 4096 != 0 {
        return false;
    }

    let (Some(first_mapped), Some(second_mapped)) =
        (first.direct_map_address(), second.direct_map_address())
    else {
        return false;
    };

    // SAFETY: Both frames are conventional UEFI memory, distinct, aligned,
    // and covered by the checked direct map. Volatile accesses prove both
    // fresh and dirty recycled frames are scrubbed by the allocation boundary.
    unsafe {
        let first_word = first_mapped as *mut u64;
        let second_word = second_mapped as *mut u64;
        if !frame_is_zero(first) || !frame_is_zero(second) {
            return false;
        }
        first_word.write_volatile(0x4749_4e55_5800_0001);
        second_word.write_volatile(0x4749_4e55_5800_0002);
        first_word
            .add(PAGE_SIZE / size_of::<u64>() - 1)
            .write_volatile(0x4749_4e55_5800_0011);
        second_word
            .add(PAGE_SIZE / size_of::<u64>() - 1)
            .write_volatile(0x4749_4e55_5800_0012);
        if !memory::release_frames(memory::FrameOwner::KERNEL, [first, second]) {
            return false;
        }
        let Some(reused) = memory::allocate_frames::<2>(memory::FrameOwner::KERNEL) else {
            return false;
        };
        let scrubbed = reused.iter().all(|frame| frame_is_zero(*frame));
        scrubbed
            && memory::release_frames(memory::FrameOwner::KERNEL, reused)
            && !memory::initialize(
                boot_info,
                arch::x86_64::paging::INITIAL_IDENTITY_LIMIT as u64,
            )
    }
}

unsafe fn frame_is_zero(frame: memory::FrameLease) -> bool {
    let Some(address) = frame.direct_map_address() else {
        return false;
    };
    let words = address as *const u64;
    (0..PAGE_SIZE / size_of::<u64>()).all(|offset| words.add(offset).read_volatile() == 0)
}

fn kernel_stack_is_active() -> bool {
    let stack_pointer: usize;
    // SAFETY: Reading RSP does not alter machine or memory state.
    unsafe {
        asm!(
            "mov {}, rsp",
            out(reg) stack_pointer,
            options(nomem, nostack, preserves_flags)
        );
    }

    let stack_start = core::ptr::addr_of!(KERNEL_STACK) as usize;
    let stack_start = stack_start + PAGE_SIZE;
    let stack_end = stack_start + KERNEL_STACK_SIZE;
    (stack_start..stack_end).contains(&stack_pointer)
}

fn summarize_memory(boot_info: &BootInfo) -> MemorySummary {
    let descriptors = boot_info.memory_map_size / boot_info.memory_descriptor_size;
    let mut summary = MemorySummary {
        descriptors,
        conventional_pages: 0,
        boot_service_pages: 0,
        loader_pages: 0,
    };

    for index in 0..descriptors {
        // SAFETY: BootInfo validation established a non-null map, a descriptor
        // stride large enough for the required prefix, and an integral count.
        let descriptor = unsafe {
            let address = boot_info
                .memory_map
                .add(index * boot_info.memory_descriptor_size);
            core::ptr::read_unaligned(address.cast::<UefiMemoryDescriptor>())
        };

        match descriptor.memory_type {
            EFI_CONVENTIONAL_MEMORY => {
                summary.conventional_pages = summary
                    .conventional_pages
                    .saturating_add(descriptor.page_count);
            }
            EFI_BOOT_SERVICES_CODE | EFI_BOOT_SERVICES_DATA => {
                summary.boot_service_pages = summary
                    .boot_service_pages
                    .saturating_add(descriptor.page_count);
            }
            EFI_LOADER_CODE | EFI_LOADER_DATA => {
                summary.loader_pages = summary.loader_pages.saturating_add(descriptor.page_count);
            }
            _ => {}
        }
    }

    summary
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
    serial_write_bytes(message.as_bytes());
}

pub(crate) fn serial_write_bytes(bytes: &[u8]) {
    for byte in bytes.iter().copied() {
        while unsafe { inb(COM1 + 5) } & 0x20 == 0 {
            core::hint::spin_loop();
        }

        // SAFETY: COM1 was initialized above and is the pinned serial device.
        unsafe { outb(COM1, byte) };
    }
}

fn serial_write_usize(value: usize) {
    serial_write_u64(value as u64);
}

fn serial_write_u64(mut value: u64) {
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
