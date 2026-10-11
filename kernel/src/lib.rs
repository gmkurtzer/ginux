#![no_std]

mod address_space;
mod arch;
mod capability;
mod domain;
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
pub const BOOT_INFO_VERSION: u32 = 1;

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
    ];
    let guard_pages = [0, kernel_stack_guard, double_fault_guard];

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

    if !arch::x86_64::cpu::install(stack_start + KERNEL_STACK_STORAGE_SIZE) {
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

    let Some(control_plane) = domain::create_and_verify_control_plane() else {
        kernel_fail("ginux: control-plane domain construction failed\r\n");
    };
    serial_write("ginux: control-plane domain metadata sealed\r\nginux: control-plane domain ID: ");
    serial_write_u64(control_plane);
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
    if !address_space::verify_isolated_user_mapping() {
        kernel_fail("\r\nginux: isolated address-space test failed\r\n");
    }
    serial_write("\r\nginux: isolated address space active");
    if !address_space::verify_control_plane_wx_layout() {
        kernel_fail("\r\nginux: control-plane W^X layout failed\r\n");
    }
    serial_write("\r\nginux: control-plane W^X mappings active");
    if !address_space::verify_ring3_control_plane() {
        kernel_fail("\r\nginux: ring-3 control-plane entry failed\r\n");
    }
    serial_write("\r\nginux: ring-3 syscall round trip and exit handled");
    if !scheduler::verify_scheduler_invariants() {
        kernel_fail("\r\nginux: scheduler lifecycle invariant failed\r\n");
    }
    serial_write("\r\nginux: scheduler context lifecycle verified");
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
    let (Some(first), Some(second)) = (memory::allocate_frame(), memory::allocate_frame()) else {
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
