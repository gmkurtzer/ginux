#![no_std]

mod address_space;
mod arch;
mod capability;
mod control_abi;
mod crypto;
mod domain;
mod executable;
mod ipc;
mod linux_elf;
mod manifest;
mod memory;
mod network;
mod pe;
mod scheduler;

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
const KERNEL_STACK_SIZE: usize = 64 * 1024;
const KERNEL_STACK_STORAGE_SIZE: usize = PAGE_SIZE + KERNEL_STACK_SIZE + PAGE_SIZE;
const KERNEL_STACK_TOP_OFFSET: usize = PAGE_SIZE + KERNEL_STACK_SIZE;
pub const BOOT_INFO_MAGIC: u64 = 0x0049_4258_554e_4947;
pub const BOOT_INFO_VERSION: u32 = 3;

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
/// This structure is a version-zero internal contract. It uses an explicit C
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
    if unsafe {
        arch::x86_64::paging::install_initial_identity_map(
            boot_info,
            &image,
            &required_addresses,
            &guard_pages,
        )
    }
    .is_err()
    {
        kernel_fail("ginux: page-table transition failed\r\n");
    }
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
    serial_write("\r\n");

    // SAFETY: The timer vector and APIC are exclusively owned during this
    // single-processor boot test. The routine restores CLI state before return.
    if !unsafe { arch::x86_64::timer::verify_local_apic_interrupt(&local_apic, &apic_timer) } {
        kernel_fail("ginux: local APIC timer interrupt failed\r\n");
    }
    serial_write("ginux: local APIC timer interrupt handled\r\n");

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
    serial_write("\r\nginux: physical frames zeroed before allocation");
    serial_write("\r\nginux: tracked frame release ownership verified");
    serial_write("\r\nginux: stale frame leases rejected");
    serial_write("\r\nginux: per-domain physical frame budgets enforced");
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
    if !linux_elf::verify_parser() {
        kernel_fail("ginux: Linux ELF64 parser invariant failed\r\n");
    }
    serial_write("ginux: non-executable Linux GNU stack enforced\r\n");
    if !linux_elf::verify_load_plan() {
        kernel_fail("ginux: Linux ELF64 load plan invariant failed\r\n");
    }
    serial_write("ginux: Linux ELF headers mapped in executable segment\r\n");
    if !linux_elf::verify_mapped_load_plan(boot_info.image_base as u64, boot_info.image_size) {
        kernel_fail("ginux: Linux ELF64 address-space mapping failed\r\n");
    }
    serial_write("ginux: Linux copy errors and overflow bounds verified\r\n");
    serial_write("ginux: Linux proof workload assembled from source\r\n");
    serial_write("ginux: Linux container-local root credentials verified\r\n");
    serial_write("ginux: Linux errno recovery verified in ring 3\r\n");
    serial_write("ginux: Linux single-thread process identity verified\r\n");
    serial_write("ginux: Linux initial environment vector validated\r\n");
    serial_write("ginux: Linux credential auxiliary vector validated\r\n");
    serial_write("ginux: Linux FS base query validated\r\n");
    serial_write("ginux: Linux clear-child-TID exit action validated\r\n");
    if !ipc::verify_channel_invariants() {
        kernel_fail("\r\nginux: bounded IPC invariant failed\r\n");
    }
    serial_write("ginux: generation-scoped IPC diagnostics verified\r\n");
    serial_write("ginux: monotonic sequence exhaustion contained\r\n");
    let (Some(host_handle), Some(console_handle), Some(ipc_handle), Some(service_ipc_handle)) = (
        domain::control_plane_boot_handle(0),
        domain::control_plane_boot_handle(1),
        domain::control_plane_boot_handle(2),
        domain::service_boot_handle(),
    ) else {
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
    {
        kernel_fail("\r\nginux: cross-domain capability isolation failed\r\n");
    }
    if !address_space::verify_ring3_control_plane(
        control_plane_image.code,
        control_plane_image.entries,
        [control_plane, service_domain],
        [
            [host_handle, console_handle, ipc_handle],
            [0, 0, service_ipc_handle],
        ],
        &local_apic,
        &apic_timer,
    ) {
        kernel_fail("\r\nginux: ring-3 control-plane entry failed\r\n");
    }
    let Some(expected_fault_instruction) =
        address_space::expected_service_fault_instruction(control_plane_image.entries[0])
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
        || !arch::x86_64::paging::identity_map_obeys_wx(boot_info, &image)
        || !arch::x86_64::paging::identity_range_is_read_only_data(
            boot_info.manifest as usize,
            boot_info.manifest_size,
        )
        || !arch::x86_64::paging::identity_range_is_read_only_data(
            boot_info.control_plane_bundle as usize,
            boot_info.control_plane_bundle_size,
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
    if !address_space::verify_ring3_auxiliary_fault() {
        kernel_fail("\r\nginux: ring-3 auxiliary fault invariant failed\r\n");
    }
    serial_write("\r\nginux: stop-only auxiliary fault executed and contained");
    serial_write("\r\nginux: opaque capability handles enforced at call gate");
    serial_write("\r\nginux: control call ABI v2 negotiated");
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
    serial_write("\r\nginux: scheduler context lifecycle verified");
    serial_write("\r\nginux: preemptible user context flags enforced");
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
    for byte in message.bytes() {
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
