use core::arch::{asm, global_asm};
use core::ptr::copy_nonoverlapping;
use core::sync::atomic::{AtomicBool, AtomicU16, AtomicU32, AtomicU64, AtomicU8, Ordering};

use super::{acpi::PlatformInfo, apic::LocalApic, cpu, paging};
use crate::{
    memory::FrameLease,
    sync::{LockRank, SpinLock},
};

const CPU_LIMIT: usize = 4;
pub(crate) const TRAMPOLINE_PHYSICAL: usize = 0x8000;
const TRAMPOLINE_VECTOR: u8 = (TRAMPOLINE_PHYSICAL >> 12) as u8;
const STARTUP_WAIT_LIMIT: usize = 20_000_000;
const IPI_WAIT_LIMIT: usize = 20_000_000;
const IPI_VECTOR: u8 = 0xf1;
const COMMAND_NONE: u8 = 0;
const COMMAND_PING: u8 = 1;
const COMMAND_INVALIDATE: u8 = 2;
const COMMAND_HOLD: u8 = 3;
const COMMAND_LINUX_PROCESS: u8 = 4;
const COMMAND_RUNTIME_OPERATION: u8 = 5;
const COMMAND_LINUX_RUNTIME_THREAD: u8 = 6;
const LINUX_RESULT_PENDING: u16 = u16::MAX;
const LINUX_RESULT_FAILED: u16 = u16::MAX - 1;

#[derive(Clone, Copy)]
struct LinuxProcessJob {
    root: FrameLease,
    entry: u64,
    stack: u64,
    fs_base: u64,
    general: [u64; 15],
}

#[derive(Clone, Copy)]
struct LinuxRuntimeThreadJob {
    root: FrameLease,
    slot: u8,
    generation: u32,
    tid: u32,
    entry: u64,
    stack: u64,
    fs_base: u64,
    general: [u64; 15],
    rflags: u64,
}

pub(crate) struct LinuxProcessTicket {
    cpu: usize,
    generation: u32,
}

static ONLINE: [AtomicBool; CPU_LIMIT] = [const { AtomicBool::new(false) }; CPU_LIMIT];
static ONLINE_COUNT: AtomicU32 = AtomicU32::new(1);
static MAILBOX_CR3: AtomicU64 = AtomicU64::new(0);
static MAILBOX_STACK: AtomicU64 = AtomicU64::new(0);
static MAILBOX_ENTRY: AtomicU64 = AtomicU64::new(0);
static MAILBOX_CPU: AtomicU32 = AtomicU32::new(0);
static APIC_IDS: [AtomicU32; CPU_LIMIT] = [const { AtomicU32::new(u32::MAX) }; CPU_LIMIT];
static APIC_BASE: AtomicU64 = AtomicU64::new(0);
static APIC_MODE: AtomicU8 = AtomicU8::new(0);
static IPI_COMMAND: [AtomicU8; CPU_LIMIT] = [const { AtomicU8::new(0) }; CPU_LIMIT];
static IPI_ARGUMENT: [AtomicU64; CPU_LIMIT] = [const { AtomicU64::new(0) }; CPU_LIMIT];
static IPI_GENERATION: [AtomicU32; CPU_LIMIT] = [const { AtomicU32::new(0) }; CPU_LIMIT];
static IPI_ACKNOWLEDGED: [AtomicU32; CPU_LIMIT] = [const { AtomicU32::new(0) }; CPU_LIMIT];
static IPI_ENTERED: [AtomicBool; CPU_LIMIT] = [const { AtomicBool::new(false) }; CPU_LIMIT];
static IPI_RELEASE: AtomicBool = AtomicBool::new(false);
static CONCURRENT_CPUS: AtomicU32 = AtomicU32::new(0);
static LINUX_PROCESS_JOBS: SpinLock<[Option<LinuxProcessJob>; CPU_LIMIT]> =
    SpinLock::new(LockRank::ARCHITECTURE, [None; CPU_LIMIT]);
static LINUX_PROCESS_RESULTS: [AtomicU16; CPU_LIMIT] =
    [const { AtomicU16::new(LINUX_RESULT_FAILED) }; CPU_LIMIT];
static LINUX_RUNTIME_THREAD_JOB: SpinLock<Option<LinuxRuntimeThreadJob>> =
    SpinLock::new(LockRank::CROSS_CPU_DISPATCH, None);
static LINUX_RUNTIME_THREAD_ACTIVE: AtomicBool = AtomicBool::new(false);
static LINUX_RUNTIME_THREAD_SLOT: AtomicU8 = AtomicU8::new(u8::MAX);
pub(super) static TLB_SHOOTDOWN_ADDRESS: AtomicU64 = AtomicU64::new(0);
pub(super) static TLB_SHOOTDOWN_ACKNOWLEDGED: AtomicU32 = AtomicU32::new(0);

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

    .text
    .global ginux_ipi_entry
ginux_ipi_entry:
    clac
    cld
    push rax
    push rbx
    push rcx
    push rdx
    push rsi
    push rdi
    push rbp
    push r8
    push r9
    push r10
    push r11
    push r12
    push r13
    push r14
    push r15
    mov r12, rsp
    and rsp, -16
    call {ipi_handler}
    mov rsp, r12
    pop r15
    pop r14
    pop r13
    pop r12
    pop r11
    pop r10
    pop r9
    pop r8
    pop rbp
    pop rdi
    pop rsi
    pop rdx
    pop rcx
    pop rbx
    pop rax
    iretq
"#,
    mailbox_cr3 = sym MAILBOX_CR3,
    mailbox_stack = sym MAILBOX_STACK,
    mailbox_entry = sym MAILBOX_ENTRY,
    mailbox_cpu = sym MAILBOX_CPU,
    ipi_handler = sym interprocessor_interrupt_handler,
);

unsafe extern "C" {
    static ginux_ap_trampoline_start: u8;
    static ginux_ap_trampoline_end: u8;
    fn ginux_ipi_entry();
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
    if !cpu::install_interrupt_gate(IPI_VECTOR, ginux_ipi_entry as *const () as usize) {
        return false;
    }
    APIC_BASE.store(apic.physical_base, Ordering::Release);
    APIC_MODE.store(
        u8::from(apic.mode == super::apic::Mode::X2Apic),
        Ordering::Release,
    );
    APIC_IDS[0].store(u32::from(apic.id), Ordering::Release);
    ONLINE[0].store(true, Ordering::Release);
    for (cpu, target) in platform.processor_apic_ids[..platform.enabled_processors]
        .iter()
        .copied()
        .filter(|target| *target != u32::from(apic.id))
        .enumerate()
    {
        let cpu = cpu + 1;
        let Some(stack) = cpu::kernel_stack_top_for_cpu(cpu) else {
            return false;
        };
        MAILBOX_CR3.store(paging::kernel_root_address(), Ordering::Release);
        MAILBOX_STACK.store(stack as u64, Ordering::Release);
        MAILBOX_ENTRY.store(
            application_processor_main as *const () as u64,
            Ordering::Release,
        );
        MAILBOX_CPU.store(cpu as u32, Ordering::Release);
        APIC_IDS[cpu].store(target, Ordering::Release);
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

pub(crate) fn execute_runtime_operation(cpu: usize, operation: u64) -> bool {
    if cpu == 0 || cpu >= online_count() {
        return false;
    }
    dispatch(cpu, COMMAND_RUNTIME_OPERATION, operation).is_some_and(|generation| {
        wait_for_ack(cpu, generation) && IPI_ARGUMENT[cpu].load(Ordering::Acquire) == 1
    })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn begin_linux_runtime_thread(
    root: FrameLease,
    slot: u8,
    generation: u32,
    tid: u32,
    entry: u64,
    stack: u64,
    fs_base: u64,
    general: [u64; 15],
    rflags: u64,
) -> bool {
    if online_count() < 2
        || LINUX_RUNTIME_THREAD_ACTIVE
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
    {
        return false;
    }
    *LINUX_RUNTIME_THREAD_JOB.lock() = Some(LinuxRuntimeThreadJob {
        root,
        slot,
        generation,
        tid,
        entry,
        stack,
        fs_base,
        general,
        rflags,
    });
    LINUX_RUNTIME_THREAD_SLOT.store(slot, Ordering::Release);
    if dispatch(1, COMMAND_LINUX_RUNTIME_THREAD, 0).is_none() {
        *LINUX_RUNTIME_THREAD_JOB.lock() = None;
        LINUX_RUNTIME_THREAD_SLOT.store(u8::MAX, Ordering::Release);
        LINUX_RUNTIME_THREAD_ACTIVE.store(false, Ordering::Release);
        return false;
    }
    true
}

pub(crate) fn linux_runtime_thread_is_remote(slot: usize) -> bool {
    linux_runtime_thread_slot() == Some(slot)
}

pub(crate) fn linux_runtime_thread_slot() -> Option<usize> {
    if !LINUX_RUNTIME_THREAD_ACTIVE.load(Ordering::Acquire) {
        return None;
    }
    let slot = usize::from(LINUX_RUNTIME_THREAD_SLOT.load(Ordering::Acquire));
    (slot != usize::from(u8::MAX)).then_some(slot)
}

pub(crate) fn verify_cross_cpu_mechanisms() -> bool {
    let count = online_count();
    if count == 0 || count > CPU_LIMIT {
        return false;
    }
    for cpu in 1..count {
        if !dispatch_and_wait(cpu, COMMAND_PING, 0)
            || !dispatch_and_wait(cpu, COMMAND_INVALIDATE, TRAMPOLINE_PHYSICAL as u64)
        {
            return false;
        }
    }
    if count == 1 {
        return true;
    }

    IPI_RELEASE.store(false, Ordering::Release);
    for (cpu, entered) in IPI_ENTERED.iter().enumerate().take(count).skip(1) {
        entered.store(false, Ordering::Release);
        if dispatch(cpu, COMMAND_HOLD, 0).is_none() {
            IPI_RELEASE.store(true, Ordering::Release);
            return false;
        }
    }
    for _ in 0..IPI_WAIT_LIMIT {
        if (1..count).all(|cpu| IPI_ENTERED[cpu].load(Ordering::Acquire)) {
            break;
        }
        core::hint::spin_loop();
    }
    if !(1..count).all(|cpu| IPI_ENTERED[cpu].load(Ordering::Acquire))
        || CONCURRENT_CPUS.fetch_add(1, Ordering::AcqRel) as usize + 1 != count
    {
        IPI_RELEASE.store(true, Ordering::Release);
        return false;
    }
    IPI_RELEASE.store(true, Ordering::Release);
    CONCURRENT_CPUS.fetch_sub(1, Ordering::AcqRel);
    for (cpu, generation) in IPI_GENERATION.iter().enumerate().take(count).skip(1) {
        let generation = generation.load(Ordering::Acquire);
        if !wait_for_ack(cpu, generation) {
            return false;
        }
    }
    CONCURRENT_CPUS.load(Ordering::Acquire) == 0
}

pub(crate) fn execute_linux_process(
    cpu: usize,
    root: FrameLease,
    entry: u64,
    stack: u64,
    fs_base: u64,
    general: [u64; 15],
) -> Option<u8> {
    let ticket = begin_linux_process(cpu, root, entry, stack, fs_base, general)?;
    finish_linux_process(ticket)
}

pub(crate) fn begin_linux_process(
    cpu: usize,
    root: FrameLease,
    entry: u64,
    stack: u64,
    fs_base: u64,
    general: [u64; 15],
) -> Option<LinuxProcessTicket> {
    if cpu == 0 || cpu >= online_count() {
        return None;
    }
    {
        let mut jobs = LINUX_PROCESS_JOBS.lock();
        if jobs[cpu].is_some() {
            return None;
        }
        jobs[cpu] = Some(LinuxProcessJob {
            root,
            entry,
            stack,
            fs_base,
            general,
        });
    }
    LINUX_PROCESS_RESULTS[cpu].store(LINUX_RESULT_PENDING, Ordering::Release);
    let Some(generation) = dispatch(cpu, COMMAND_LINUX_PROCESS, 0) else {
        LINUX_PROCESS_JOBS.lock()[cpu] = None;
        return None;
    };
    Some(LinuxProcessTicket { cpu, generation })
}

pub(crate) fn finish_linux_process(ticket: LinuxProcessTicket) -> Option<u8> {
    if !wait_for_ack(ticket.cpu, ticket.generation) {
        return None;
    }
    let result = LINUX_PROCESS_RESULTS[ticket.cpu].swap(LINUX_RESULT_FAILED, Ordering::AcqRel);
    (result <= u16::from(u8::MAX)).then_some(result as u8)
}

fn dispatch_and_wait(cpu: usize, command: u8, argument: u64) -> bool {
    dispatch(cpu, command, argument).is_some_and(|generation| wait_for_ack(cpu, generation))
}

fn dispatch(cpu: usize, command: u8, argument: u64) -> Option<u32> {
    if cpu == 0 || cpu >= online_count() || command == COMMAND_NONE {
        return None;
    }
    let generation = IPI_GENERATION[cpu]
        .fetch_add(1, Ordering::AcqRel)
        .checked_add(1)?;
    IPI_ARGUMENT[cpu].store(argument, Ordering::Release);
    IPI_COMMAND[cpu].store(command, Ordering::Release);
    let target = APIC_IDS[cpu].load(Ordering::Acquire);
    let apic = saved_local_apic()?;
    // SAFETY: this vector is installed in every online CPU's private IDT and
    // the discovered APIC ID identifies the selected processor.
    unsafe { apic.send_fixed(target, IPI_VECTOR) }.then_some(generation)
}

fn wait_for_ack(cpu: usize, generation: u32) -> bool {
    for _ in 0..IPI_WAIT_LIMIT {
        if IPI_ACKNOWLEDGED[cpu].load(Ordering::Acquire) == generation {
            return true;
        }
        core::hint::spin_loop();
    }
    false
}

fn saved_local_apic() -> Option<LocalApic> {
    let mode = if APIC_MODE.load(Ordering::Acquire) == 1 {
        super::apic::Mode::X2Apic
    } else {
        super::apic::Mode::XApic
    };
    LocalApic::from_saved(0, mode, APIC_BASE.load(Ordering::Acquire))
}

pub(crate) fn invalidate_runtime_translation(address: u64) -> bool {
    if address == 0 || !address.is_multiple_of(4096) {
        return false;
    }
    if TLB_SHOOTDOWN_ADDRESS
        .compare_exchange(0, address, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return false;
    }
    TLB_SHOOTDOWN_ACKNOWLEDGED.store(0, Ordering::Release);
    let current = cpu::current_cpu_index();
    let Some(apic) = saved_local_apic() else {
        TLB_SHOOTDOWN_ADDRESS.store(0, Ordering::Release);
        return false;
    };
    let mut targets = 0_u32;
    for target in 0..online_count() {
        if target == current || !ONLINE[target].load(Ordering::Acquire) {
            continue;
        }
        let apic_id = APIC_IDS[target].load(Ordering::Acquire);
        if !(unsafe { apic.send_nmi(apic_id) }) {
            TLB_SHOOTDOWN_ADDRESS.store(0, Ordering::Release);
            return false;
        }
        targets += 1;
    }
    unsafe { asm!("invlpg [{}]", in(reg) address, options(nostack, preserves_flags)) };
    let acknowledged = (0..IPI_WAIT_LIMIT).any(|_| {
        if TLB_SHOOTDOWN_ACKNOWLEDGED.load(Ordering::Acquire) >= targets {
            true
        } else {
            core::hint::spin_loop();
            false
        }
    });
    TLB_SHOOTDOWN_ADDRESS.store(0, Ordering::Release);
    acknowledged
}

extern "sysv64" fn interprocessor_interrupt_handler() {
    let cpu = cpu::current_cpu_index();
    if cpu >= CPU_LIMIT {
        return;
    }
    let generation = IPI_GENERATION[cpu].load(Ordering::Acquire);
    match IPI_COMMAND[cpu].swap(COMMAND_NONE, Ordering::AcqRel) {
        COMMAND_PING => {}
        COMMAND_INVALIDATE => {
            let address = IPI_ARGUMENT[cpu].load(Ordering::Acquire);
            unsafe { asm!("invlpg [{}]", in(reg) address, options(nostack, preserves_flags)) };
        }
        COMMAND_HOLD => {
            CONCURRENT_CPUS.fetch_add(1, Ordering::AcqRel);
            IPI_ENTERED[cpu].store(true, Ordering::Release);
            while !IPI_RELEASE.load(Ordering::Acquire) {
                core::hint::spin_loop();
            }
            CONCURRENT_CPUS.fetch_sub(1, Ordering::AcqRel);
        }
        COMMAND_LINUX_PROCESS => {
            let job = LINUX_PROCESS_JOBS.lock()[cpu].take();
            let result = job
                .and_then(|job| {
                    crate::address_space::execute_linux_process_child(
                        job.root,
                        job.entry,
                        job.stack,
                        job.fs_base,
                        job.general,
                    )
                })
                .map_or(LINUX_RESULT_FAILED, u16::from);
            LINUX_PROCESS_RESULTS[cpu].store(result, Ordering::Release);
        }
        COMMAND_RUNTIME_OPERATION => {
            let operation = IPI_ARGUMENT[cpu].load(Ordering::Acquire);
            let result = {
                let _kernel_entry = crate::sync::kernel_entry_lock();
                cpu::execute_cross_cpu_runtime_operation(operation)
            };
            IPI_ARGUMENT[cpu].store(u64::from(result), Ordering::Release);
        }
        COMMAND_LINUX_RUNTIME_THREAD => {
            let job = LINUX_RUNTIME_THREAD_JOB.lock().take();
            let result = job.is_some_and(|job| {
                cpu::begin_dispatched_linux_runtime_thread(
                    job.root,
                    job.slot,
                    job.generation,
                    job.tid,
                    job.entry,
                    job.stack,
                    job.fs_base,
                    job.general,
                    job.rflags,
                )
            });
            IPI_ARGUMENT[cpu].store(u64::from(result), Ordering::Release);
            LINUX_RUNTIME_THREAD_SLOT.store(u8::MAX, Ordering::Release);
            LINUX_RUNTIME_THREAD_ACTIVE.store(false, Ordering::Release);
        }
        _ => {}
    }
    IPI_ACKNOWLEDGED[cpu].store(generation, Ordering::Release);
    if let Some(apic) = saved_local_apic() {
        unsafe { apic.end_of_interrupt() };
    }
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
    let Some(apic) = saved_local_apic() else {
        loop {
            unsafe { asm!("hlt", options(nomem, nostack)) };
        }
    };
    unsafe { apic.enable_software() };
    ONLINE[cpu].store(true, Ordering::Release);
    ONLINE_COUNT.fetch_add(1, Ordering::AcqRel);
    loop {
        unsafe { asm!("sti; hlt", options(nomem, nostack)) };
    }
}
