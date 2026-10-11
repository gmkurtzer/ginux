#![no_std]

mod address_space;
mod arch;
mod capability;
mod crypto;
mod domain;
mod executable;
mod ipc;
mod manifest;
mod memory;
mod scheduler;

use core::arch::{asm, global_asm};
use core::ffi::c_void;
use core::mem::size_of;

const COM1: u16 = 0x3f8;
const EFI_RESET_SHUTDOWN: u32 = 2;
const EFI_SUCCESS: usize = 0;
const EFI_LOADER_CODE: u32 = 1;
const EFI_LOADER_DATA: u32 = 2;
const EFI_BOOT_SERVICES_CODE: u32 = 3;
const EFI_BOOT_SERVICES_DATA: u32 = 4;
const EFI_CONVENTIONAL_MEMORY: u32 = 7;
const UEFI_PAGES_PER_MIB: u64 = 256;
const PAGE_SIZE: usize = 4096;
const KERNEL_STACK_SIZE: usize = 64 * 1024;
const KERNEL_STACK_STORAGE_SIZE: usize = PAGE_SIZE + KERNEL_STACK_SIZE;
pub const BOOT_INFO_MAGIC: u64 = 0x0049_4258_554e_4947;
pub const BOOT_INFO_VERSION: u32 = 2;

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
    stack_size = const KERNEL_STACK_STORAGE_SIZE,
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
}

#[repr(C)]
struct UefiMemoryDescriptor {
    memory_type: u32,
    padding: u32,
    physical_start: u64,
    virtual_start: u64,
    page_count: u64,
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
    serial_write("ginux: versioned boot handoff accepted\r\n");
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
    let double_fault_guard = arch::x86_64::cpu::double_fault_guard_page();
    let privilege_stack_guard = arch::x86_64::cpu::privilege_stack_guard_page();
    let required_addresses = [
        kernel_main as *const () as usize,
        boot_info as *const BootInfo as usize,
        boot_info.memory_map as usize,
        memory_map_end,
        kernel_stack_start,
        stack_start + KERNEL_STACK_STORAGE_SIZE - 1,
        boot_info.reset_system as usize,
        boot_info.acpi_rsdp as usize,
        acpi_rsdp_end,
        boot_info.manifest as usize,
        boot_info.manifest as usize + boot_info.manifest_size - 1,
        boot_info.control_plane_bundle as usize,
        boot_info.control_plane_bundle as usize + boot_info.control_plane_bundle_size - 1,
        privilege_stack_guard + PAGE_SIZE,
        arch::x86_64::cpu::privilege_stack_top() - 1,
    ];
    let guard_pages = [
        0,
        kernel_stack_guard,
        privilege_stack_guard,
        double_fault_guard,
    ];

    // SAFETY: The entry trampoline disabled interrupts. The required live
    // addresses and the page-table objects are checked against the complete
    // transitional identity-map range before CR3 is changed.
    if unsafe {
        arch::x86_64::paging::install_initial_identity_map(&required_addresses, &guard_pages)
    }
    .is_err()
    {
        kernel_fail("ginux: page-table transition failed\r\n");
    }
    serial_write("ginux: kernel page tables active\r\n");
    serial_write("ginux: null and stack guard pages active\r\n");

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

    if !arch::x86_64::cpu::install() {
        kernel_fail("ginux: architectural exception gates invalid\r\n");
    }
    serial_write("ginux: GDT/TSS/IDT active\r\n");
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
    // environment. The transitional identity map covers the asserted xAPIC
    // MMIO limit, and no other kernel component uses the PIT or APIC timer.
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
    if !domain::verify_cross_domain_revocation_invariants() {
        kernel_fail("ginux: cross-domain revocation invariant failed\r\n");
    }
    serial_write("ginux: cross-domain capability revocation verified\r\n");
    serial_write("ginux: per-root delegation provenance verified\r\n");
    if !domain::verify_domain_ancestry_invariants() {
        kernel_fail("ginux: domain ancestry invariant failed\r\n");
    }
    serial_write("ginux: domain lifecycle ancestry verified\r\n");
    if !domain::verify_lifecycle_event_invariants() {
        kernel_fail("ginux: lifecycle event queue invariant failed\r\n");
    }
    serial_write("ginux: bounded lifecycle event queue verified\r\n");

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
    serial_write("\r\nginux: atomic frame reservations verified");
    serial_write("\r\nginux: tracked frame release ownership verified");
    if !address_space::verify_isolated_user_mapping() {
        kernel_fail("\r\nginux: isolated address-space test failed\r\n");
    }
    serial_write("\r\nginux: isolated address space active");
    if !address_space::verify_control_plane_wx_layout() {
        kernel_fail("\r\nginux: control-plane W^X layout failed\r\n");
    }
    serial_write("\r\nginux: control-plane W^X mappings active");
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
    if !ipc::verify_channel_invariants() {
        kernel_fail("\r\nginux: bounded IPC invariant failed\r\n");
    }
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
    if !domain::domain_is_retired(service_domain)
        || !domain::service_fault_was_delivered(service_domain)
        || !domain::call_budget_proof_complete()
        || !domain::thread_budget_proof_complete()
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
    {
        kernel_fail("\r\nginux: replacement service generation invariant failed\r\n");
    }
    serial_write("\r\nginux: opaque capability handles enforced at call gate");
    serial_write("\r\nginux: control call ABI v1 negotiated");
    serial_write("\r\nginux: userspace console capability enforced");
    serial_write("\r\nginux: ring-3 fault survivor resumed and exited");
    serial_write("\r\nginux: user invalid-opcode fault contained");
    serial_write("\r\nginux: isolated scheduler address spaces switched");
    serial_write("\r\nginux: one-shot user preemption completed");
    serial_write("\r\nginux: restarted service preemption completed");
    serial_write("\r\nginux: completed address-space frames reclaimed");
    serial_write("\r\nginux: capability-gated bounded IPC verified");
    serial_write("\r\nginux: directional IPC endpoints isolated");
    serial_write("\r\nginux: blocking IPC direct wake completed");
    serial_write("\r\nginux: IPC wait-key matching enforced");
    serial_write("\r\nginux: blocked IPC peer-close cleanup verified");
    serial_write("\r\nginux: isolated service request-reply completed");
    serial_write("\r\nginux: service domain authority restricted to IPC");
    serial_write("\r\nginux: faulted service domain authority retired");
    serial_write("\r\nginux: service fault event delivered to control plane");
    serial_write("\r\nginux: service fault event generation matched");
    serial_write("\r\nginux: service domain generation reuse verified");
    serial_write("\r\nginux: replacement service context executed");
    serial_write("\r\nginux: per-domain syscall budgets enforced");
    serial_write("\r\nginux: per-domain thread budgets enforced");
    if !scheduler::verify_scheduler_invariants() {
        kernel_fail("\r\nginux: scheduler lifecycle invariant failed\r\n");
    }
    serial_write("\r\nginux: scheduler context lifecycle verified");
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
    let Some([first, second]) = memory::allocate_frames::<2>() else {
        return false;
    };
    if first == second || first % 4096 != 0 || second % 4096 != 0 {
        return false;
    }

    // SAFETY: Both frames are conventional UEFI memory, distinct, aligned,
    // and covered by the active identity map. Volatile accesses prove they
    // are writable without letting the compiler remove the test.
    unsafe {
        let first_word = first as *mut u64;
        let second_word = second as *mut u64;
        first_word.write_volatile(0x4749_4e55_5800_0001);
        second_word.write_volatile(0x4749_4e55_5800_0002);
        let valid = first_word.read_volatile() == 0x4749_4e55_5800_0001
            && second_word.read_volatile() == 0x4749_4e55_5800_0002;
        first_word.write_volatile(0);
        second_word.write_volatile(0);
        valid
            && memory::release_frames([first, second])
            && !memory::initialize(
                boot_info,
                arch::x86_64::paging::INITIAL_IDENTITY_LIMIT as u64,
            )
    }
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
