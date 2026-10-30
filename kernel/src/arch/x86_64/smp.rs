use core::arch::{asm, global_asm};
use core::ptr::copy_nonoverlapping;
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

use super::{acpi::PlatformInfo, apic::LocalApic, cpu, paging};

const CPU_LIMIT: usize = 4;
pub(crate) const TRAMPOLINE_PHYSICAL: usize = 0x8000;
const TRAMPOLINE_VECTOR: u8 = (TRAMPOLINE_PHYSICAL >> 12) as u8;
const STARTUP_WAIT_LIMIT: usize = 20_000_000;

static ONLINE: [AtomicBool; CPU_LIMIT] = [const { AtomicBool::new(false) }; CPU_LIMIT];
static ONLINE_COUNT: AtomicU32 = AtomicU32::new(1);
static MAILBOX_CR3: AtomicU64 = AtomicU64::new(0);
static MAILBOX_STACK: AtomicU64 = AtomicU64::new(0);
static MAILBOX_ENTRY: AtomicU64 = AtomicU64::new(0);
static MAILBOX_CPU: AtomicU32 = AtomicU32::new(0);

global_asm!(
    r#"
    .pushsection .text.ginux_ap_trampoline,"ax"
    .set ginux_ap_gdt_pointer_physical, ginux_ap_trampoline_gdt_pointer - ginux_ap_trampoline_start + 0x8000
    .set ginux_ap_cr3_pointer_physical, ginux_ap_trampoline_cr3_pointer - ginux_ap_trampoline_start + 0x8000
    .balign 16
    .global ginux_ap_trampoline_start
ginux_ap_trampoline_start:
    .code16
    cli
    cld
    xor ax, ax
    mov ds, ax
    mov es, ax
    mov ss, ax
    mov si, offset ginux_ap_gdt_pointer_physical
    lgdt [si]
    mov eax, cr0
    or eax, 1
    mov cr0, eax
    .byte 0x66, 0xea
    .long ginux_ap_trampoline_protected - ginux_ap_trampoline_start + 0x8000
    .word 0x08

ginux_ap_trampoline_protected:
    .code32
    mov ax, 0x10
    mov ds, ax
    mov es, ax
    mov ss, ax
    mov eax, cr4
    or eax, 1 << 5
    mov cr4, eax
    mov esi, ginux_ap_cr3_pointer_physical
    mov eax, dword ptr [esi]
    mov cr3, eax
    mov ecx, 0xc0000080
    rdmsr
    or eax, (1 << 8) | (1 << 11)
    wrmsr
    mov eax, cr0
    or eax, 0x80000001
    mov cr0, eax
    .byte 0xea
    .long ginux_ap_trampoline_long - ginux_ap_trampoline_start + 0x8000
    .word 0x18

ginux_ap_trampoline_long:
    .code64
    mov ax, 0x10
    mov ds, ax
    mov es, ax
    mov ss, ax
    mov rbx, qword ptr [rip + ginux_ap_trampoline_stack_pointer]
    mov rsp, qword ptr [rbx]
    and rsp, -16
    mov rbx, qword ptr [rip + ginux_ap_trampoline_cpu_pointer]
    mov edi, dword ptr [rbx]
    mov rbx, qword ptr [rip + ginux_ap_trampoline_entry_pointer]
    mov rax, qword ptr [rbx]
    call rax
    ud2

    .balign 8
ginux_ap_trampoline_cr3_pointer:
    .quad {mailbox_cr3}
ginux_ap_trampoline_stack_pointer:
    .quad {mailbox_stack}
ginux_ap_trampoline_entry_pointer:
    .quad {mailbox_entry}
ginux_ap_trampoline_cpu_pointer:
    .quad {mailbox_cpu}

    .balign 8
ginux_ap_trampoline_gdt:
    .quad 0
    .quad 0x00cf9a000000ffff
    .quad 0x00cf92000000ffff
    .quad 0x00af9a000000ffff
ginux_ap_trampoline_gdt_end:
ginux_ap_trampoline_gdt_pointer:
    .word ginux_ap_trampoline_gdt_end - ginux_ap_trampoline_gdt - 1
    .long ginux_ap_trampoline_gdt - ginux_ap_trampoline_start + 0x8000
    .global ginux_ap_trampoline_end
ginux_ap_trampoline_end:
    .code64
    .popsection
"#,
    mailbox_cr3 = sym MAILBOX_CR3,
    mailbox_stack = sym MAILBOX_STACK,
    mailbox_entry = sym MAILBOX_ENTRY,
    mailbox_cpu = sym MAILBOX_CPU,
);

unsafe extern "C" {
    static ginux_ap_trampoline_start: u8;
    static ginux_ap_trampoline_end: u8;
}

pub(crate) fn prepare_trampoline() -> bool {
    let start = core::ptr::addr_of!(ginux_ap_trampoline_start) as usize;
    let end = core::ptr::addr_of!(ginux_ap_trampoline_end) as usize;
    let size = end.saturating_sub(start);
    if size == 0 || size > 4096 {
        return false;
    }
    unsafe {
        core::ptr::write_bytes(TRAMPOLINE_PHYSICAL as *mut u8, 0, 4096);
        copy_nonoverlapping(start as *const u8, TRAMPOLINE_PHYSICAL as *mut u8, size);
    }
    true
}

pub(crate) fn start_application_processors(platform: PlatformInfo, apic: &LocalApic) -> bool {
    if platform.enabled_processors == 0 || platform.enabled_processors > CPU_LIMIT {
        return false;
    }
    ONLINE[0].store(true, Ordering::Release);
    for (cpu, target) in platform.processor_apic_ids[..platform.enabled_processors]
        .iter()
        .copied()
        .filter(|target| *target != u32::from(apic.id))
        .enumerate()
    {
        let cpu = cpu + 1;
        let Some(stack) = cpu::privilege_stack_top_for_cpu(cpu) else {
            return false;
        };
        MAILBOX_CR3.store(paging::kernel_root_address(), Ordering::Release);
        MAILBOX_STACK.store(stack as u64, Ordering::Release);
        MAILBOX_ENTRY.store(
            application_processor_main as *const () as u64,
            Ordering::Release,
        );
        MAILBOX_CPU.store(cpu as u32, Ordering::Release);
        if unsafe { !apic.start_processor(target, TRAMPOLINE_VECTOR) } {
            return false;
        }
        let mut online = false;
        for _ in 0..STARTUP_WAIT_LIMIT {
            if ONLINE[cpu].load(Ordering::Acquire) {
                online = true;
                break;
            }
            core::hint::spin_loop();
        }
        if !online {
            return false;
        }
    }
    ONLINE_COUNT.load(Ordering::Acquire) as usize == platform.enabled_processors
}

pub(crate) fn online_count() -> usize {
    ONLINE_COUNT.load(Ordering::Acquire) as usize
}

extern "sysv64" fn application_processor_main(cpu_index: u32) -> ! {
    unsafe { asm!("cli", options(nomem, nostack)) };
    let cpu = cpu_index as usize;
    if cpu == 0
        || cpu >= CPU_LIMIT
        || !cpu::install_for_cpu(cpu)
        || !cpu::enable_execute_disable()
        || !cpu::enable_supervisor_execution_prevention()
        || !cpu::enable_supervisor_access_prevention()
        || !cpu::enable_user_mode_instruction_prevention()
    {
        loop {
            unsafe { asm!("hlt", options(nomem, nostack)) };
        }
    }
    let _ = cpu::enable_extended_user_state();
    ONLINE[cpu].store(true, Ordering::Release);
    ONLINE_COUNT.fetch_add(1, Ordering::AcqRel);
    loop {
        unsafe { asm!("hlt", options(nomem, nostack)) };
    }
}
