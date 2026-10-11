use core::arch::{
    asm, global_asm,
    x86_64::{__cpuid, __cpuid_count},
};
use core::mem::size_of;
use core::sync::atomic::{AtomicBool, AtomicU8, Ordering};

use super::{paging, timer};

use crate::{
    address_space,
    capability::Rights,
    control_abi, domain, ipc, memory, scheduler,
    scheduler::{RuntimeAction, UserFaultFrame, UserTrapFrame},
    serial_write, serial_write_u64,
};

const KERNEL_CODE_SELECTOR: u16 = 0x08;
const KERNEL_DATA_SELECTOR: u16 = 0x10;
const TSS_SELECTOR: u16 = 0x18;
pub(crate) const USER_DATA_SELECTOR: u16 = 0x2b;
pub(crate) const USER_CODE_SELECTOR: u16 = 0x33;
const SYSCALL_VECTOR: u8 = 0x80;
const CALL_BUDGET_EXHAUSTED: u64 = u64::MAX - 3;
const DOUBLE_FAULT_IST: u8 = 1;
const PAGE_SIZE: usize = 4096;
const EXCEPTION_STACK_SIZE: usize = 16 * 1024;
const EXCEPTION_STACK_STORAGE_SIZE: usize = PAGE_SIZE + EXCEPTION_STACK_SIZE + PAGE_SIZE;
const EXCEPTION_STACK_TOP_OFFSET: usize = PAGE_SIZE + EXCEPTION_STACK_SIZE;
const CPUID_STRUCTURED_FEATURES: u32 = 7;
const CPUID_EXTENDED_MAX: u32 = 0x8000_0000;
const CPUID_EXTENDED_FEATURES: u32 = 0x8000_0001;
const CPUID_NX: u32 = 1 << 20;
const CPUID_SMEP: u32 = 1 << 7;
const CPUID_SMAP: u32 = 1 << 20;
const CPUID_UMIP: u32 = 1 << 2;
const CR4_UMIP: u64 = 1 << 11;
const CR4_SMEP: u64 = 1 << 20;
const CR4_SMAP: u64 = 1 << 21;
const EFER_MSR: u32 = 0xc000_0080;
const EFER_NXE: u64 = 1 << 11;

#[repr(C, packed)]
struct TaskStateSegment {
    reserved0: u32,
    privilege_stacks: [u64; 3],
    reserved1: u64,
    interrupt_stacks: [u64; 7],
    reserved2: u64,
    reserved3: u16,
    io_map_base: u16,
}

#[derive(Clone, Copy)]
#[repr(C, packed)]
struct IdtEntry {
    offset_low: u16,
    selector: u16,
    ist: u8,
    attributes: u8,
    offset_middle: u16,
    offset_high: u32,
    reserved: u32,
}

impl IdtEntry {
    const MISSING: Self = Self {
        offset_low: 0,
        selector: 0,
        ist: 0,
        attributes: 0,
        offset_middle: 0,
        offset_high: 0,
        reserved: 0,
    };

    fn interrupt(handler: usize, ist: u8) -> Self {
        Self {
            offset_low: handler as u16,
            selector: KERNEL_CODE_SELECTOR,
            ist,
            attributes: 0x8e,
            offset_middle: (handler >> 16) as u16,
            offset_high: (handler >> 32) as u32,
            reserved: 0,
        }
    }

    fn is_missing(self) -> bool {
        self.offset_low == 0
            && self.selector == 0
            && self.ist == 0
            && self.attributes == 0
            && self.offset_middle == 0
            && self.offset_high == 0
            && self.reserved == 0
    }

    fn has_handler(self) -> bool {
        self.offset_low != 0 || self.offset_middle != 0 || self.offset_high != 0
    }
}

#[repr(C, packed)]
struct DescriptorTablePointer {
    limit: u16,
    base: u64,
}

#[repr(C, align(4096))]
struct ExceptionStack([u8; EXCEPTION_STACK_STORAGE_SIZE]);

static BREAKPOINT_HANDLED: AtomicBool = AtomicBool::new(false);
static SYSCALL_STATE: AtomicU8 = AtomicU8::new(0);
static SERVICE_STACK_SCRUBBED: AtomicBool = AtomicBool::new(false);
static SERVICE_FRAMES_TRANSFERRED: AtomicBool = AtomicBool::new(false);
static mut USER_RETURN_STACK: u64 = 0;
static mut GDT: [u64; 7] = [0; 7];
static mut TSS: TaskStateSegment = TaskStateSegment {
    reserved0: 0,
    privilege_stacks: [0; 3],
    reserved1: 0,
    interrupt_stacks: [0; 7],
    reserved2: 0,
    reserved3: 0,
    io_map_base: size_of::<TaskStateSegment>() as u16,
};
static mut IDT: [IdtEntry; 256] = [IdtEntry::MISSING; 256];
static mut PRIVILEGE_STACK: ExceptionStack = ExceptionStack([0; EXCEPTION_STACK_STORAGE_SIZE]);
static mut DOUBLE_FAULT_STACK: ExceptionStack = ExceptionStack([0; EXCEPTION_STACK_STORAGE_SIZE]);

global_asm!(
    r#"
    .text
    .global ginux_breakpoint_entry
ginux_breakpoint_entry:
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
    call {breakpoint_handler}
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

    .macro GINUX_EXCEPTION_NO_ERROR vector
    .global ginux_exception_\vector
ginux_exception_\vector:
    push 0
    push \vector
    jmp ginux_fatal_exception_common
    .endm

    .macro GINUX_EXCEPTION_ERROR vector
    .global ginux_exception_\vector
ginux_exception_\vector:
    push \vector
    jmp ginux_fatal_exception_common
    .endm

    GINUX_EXCEPTION_NO_ERROR 0
    GINUX_EXCEPTION_NO_ERROR 1
    GINUX_EXCEPTION_NO_ERROR 2
    GINUX_EXCEPTION_NO_ERROR 4
    GINUX_EXCEPTION_NO_ERROR 5
    GINUX_EXCEPTION_NO_ERROR 6
    GINUX_EXCEPTION_NO_ERROR 7
    .global ginux_double_fault_entry
ginux_double_fault_entry:
    push 8
    jmp ginux_fatal_exception_common
    GINUX_EXCEPTION_NO_ERROR 9
    GINUX_EXCEPTION_ERROR 10
    GINUX_EXCEPTION_ERROR 11
    GINUX_EXCEPTION_ERROR 12
    GINUX_EXCEPTION_ERROR 13
    GINUX_EXCEPTION_ERROR 14
    GINUX_EXCEPTION_NO_ERROR 15
    GINUX_EXCEPTION_NO_ERROR 16
    GINUX_EXCEPTION_ERROR 17
    GINUX_EXCEPTION_NO_ERROR 18
    GINUX_EXCEPTION_NO_ERROR 19
    GINUX_EXCEPTION_NO_ERROR 20
    GINUX_EXCEPTION_ERROR 21
    GINUX_EXCEPTION_NO_ERROR 22
    GINUX_EXCEPTION_NO_ERROR 23
    GINUX_EXCEPTION_NO_ERROR 24
    GINUX_EXCEPTION_NO_ERROR 25
    GINUX_EXCEPTION_NO_ERROR 26
    GINUX_EXCEPTION_NO_ERROR 27
    GINUX_EXCEPTION_NO_ERROR 28
    GINUX_EXCEPTION_ERROR 29
    GINUX_EXCEPTION_ERROR 30
    GINUX_EXCEPTION_NO_ERROR 31

ginux_fatal_exception_common:
    cli
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
    test qword ptr [rsp + 144], 3
    jnz ginux_user_fault_common
    mov rdi, [rsp + 120]
    mov rsi, [rsp + 128]
    mov rdx, cr2
    mov rcx, [rsp + 136]
    and rsp, -16
    call {fatal_exception_handler}
    ud2

ginux_user_fault_common:
    mov r12, rsp
    mov rdi, rsp
    and rsp, -16
    call {user_fault_handler}
    cmp rax, -1
    je ginux_user_return_kernel
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
    add rsp, 16
    iretq

    .section .rodata
    .align 8
    .global ginux_exception_entries
ginux_exception_entries:
    .quad ginux_exception_0
    .quad ginux_exception_1
    .quad ginux_exception_2
    .quad ginux_breakpoint_entry
    .quad ginux_exception_4
    .quad ginux_exception_5
    .quad ginux_exception_6
    .quad ginux_exception_7
    .quad ginux_double_fault_entry
    .quad ginux_exception_9
    .quad ginux_exception_10
    .quad ginux_exception_11
    .quad ginux_exception_12
    .quad ginux_exception_13
    .quad ginux_exception_14
    .quad ginux_exception_15
    .quad ginux_exception_16
    .quad ginux_exception_17
    .quad ginux_exception_18
    .quad ginux_exception_19
    .quad ginux_exception_20
    .quad ginux_exception_21
    .quad ginux_exception_22
    .quad ginux_exception_23
    .quad ginux_exception_24
    .quad ginux_exception_25
    .quad ginux_exception_26
    .quad ginux_exception_27
    .quad ginux_exception_28
    .quad ginux_exception_29
    .quad ginux_exception_30
    .quad ginux_exception_31

    .text
    .global ginux_user_enter
ginux_user_enter:
    mov r10, [rsp + 8]
    mov r11, [rsp + 16]
    push rbx
    push rbp
    push r12
    push r13
    push r14
    push r15
    mov [rip + {user_return_stack}], rsp
    mov rbx, rdx
    push {user_data}
    push rsi
    push 0x202
    push {user_code}
    push rdi
    iretq

    .global ginux_syscall_entry
ginux_syscall_entry:
    cli
    clac
    cld
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
    mov rdi, rax
    mov rsi, [r12 + 72]
    mov rdx, r12
    and rsp, -16
    call {syscall_handler}
    cmp rax, -1
    je ginux_user_return_kernel
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
    iretq
ginux_user_return_kernel:
    mov rsp, [rip + {user_return_stack}]
    pop r15
    pop r14
    pop r13
    pop r12
    pop rbp
    pop rbx
    ret
"#,
    breakpoint_handler = sym breakpoint_handler,
    fatal_exception_handler = sym fatal_exception_handler,
    user_fault_handler = sym user_fault_handler,
    syscall_handler = sym syscall_handler,
    user_return_stack = sym USER_RETURN_STACK,
    user_data = const USER_DATA_SELECTOR as u64,
    user_code = const USER_CODE_SELECTOR as u64,
);

extern "C" {
    static ginux_exception_entries: [usize; 32];
    fn ginux_syscall_entry();
}

extern "sysv64" {
    fn ginux_user_enter(
        entry: usize,
        stack: usize,
        host_handle: u64,
        console_handle: u64,
        ipc_handle: u64,
        identity: u64,
        service_domain: u64,
        service_identity: u64,
    );
}

pub(crate) fn install() -> bool {
    // SAFETY: Initialization runs once with interrupts disabled and before any
    // other CPU can observe these tables.
    unsafe {
        install_gdt_and_tss();
        install_idt();
    }
    exception_gates_are_valid()
}

pub(crate) fn task_register_denies_user_io() -> bool {
    let selector: u16;
    unsafe {
        asm!("str {selector:x}", selector = out(reg) selector, options(nomem, nostack, preserves_flags));
    }
    let low = unsafe { core::ptr::addr_of!(GDT).cast::<u64>().add(3).read() };
    let high = unsafe { core::ptr::addr_of!(GDT).cast::<u64>().add(4).read() };
    let limit = (low & 0xffff) | ((low >> 32) & 0x000f_0000);
    let base =
        ((low >> 16) & 0x00ff_ffff) | ((low >> 32) & 0xff00_0000) | ((high & 0xffff_ffff) << 32);
    let io_map_base = unsafe { core::ptr::addr_of!(TSS.io_map_base).read_unaligned() };
    selector == TSS_SELECTOR
        && low & (1 << 47) != 0
        && (low >> 40) & 0x0f == 0x0b
        && limit == (size_of::<TaskStateSegment>() - 1) as u64
        && base == core::ptr::addr_of!(TSS) as u64
        && u64::from(io_map_base) > limit
}

pub(crate) fn enable_supervisor_execution_prevention() -> bool {
    if __cpuid(0).eax < CPUID_STRUCTURED_FEATURES
        || __cpuid_count(CPUID_STRUCTURED_FEATURES, 0).ebx & CPUID_SMEP == 0
    {
        return false;
    }
    let mut cr4: u64;
    unsafe {
        asm!("mov {}, cr4", out(reg) cr4, options(nomem, nostack, preserves_flags));
        cr4 |= CR4_SMEP;
        asm!("mov cr4, {}", in(reg) cr4, options(nomem, nostack, preserves_flags));
    }
    supervisor_execution_prevention_enabled()
}

pub(crate) fn enable_execute_disable() -> bool {
    if __cpuid(CPUID_EXTENDED_MAX).eax < CPUID_EXTENDED_FEATURES
        || __cpuid(CPUID_EXTENDED_FEATURES).edx & CPUID_NX == 0
    {
        return false;
    }
    unsafe {
        let efer = read_msr(EFER_MSR);
        write_msr(EFER_MSR, efer | EFER_NXE);
        read_msr(EFER_MSR) & EFER_NXE != 0
    }
}

pub(crate) fn execute_disable_enabled() -> bool {
    if __cpuid(CPUID_EXTENDED_MAX).eax < CPUID_EXTENDED_FEATURES {
        return false;
    }
    unsafe { read_msr(EFER_MSR) & EFER_NXE != 0 }
}

pub(crate) fn supervisor_execution_prevention_enabled() -> bool {
    let cr4: u64;
    unsafe {
        asm!("mov {}, cr4", out(reg) cr4, options(nomem, nostack, preserves_flags));
    }
    cr4 & CR4_SMEP != 0
}

pub(crate) fn enable_supervisor_access_prevention() -> bool {
    if __cpuid(0).eax < CPUID_STRUCTURED_FEATURES
        || __cpuid_count(CPUID_STRUCTURED_FEATURES, 0).ebx & CPUID_SMAP == 0
    {
        return false;
    }
    let mut cr4: u64;
    unsafe {
        asm!("mov {}, cr4", out(reg) cr4, options(nomem, nostack, preserves_flags));
        cr4 |= CR4_SMAP;
        asm!(
            "mov cr4, {cr4}",
            "clac",
            cr4 = in(reg) cr4,
            options(nomem, nostack)
        );
    }
    supervisor_access_prevention_enabled()
}

pub(crate) fn supervisor_access_prevention_enabled() -> bool {
    let cr4: u64;
    unsafe {
        asm!("mov {}, cr4", out(reg) cr4, options(nomem, nostack, preserves_flags));
    }
    cr4 & CR4_SMAP != 0
}

pub(crate) fn enable_user_mode_instruction_prevention() -> bool {
    if __cpuid(0).eax < CPUID_STRUCTURED_FEATURES
        || __cpuid_count(CPUID_STRUCTURED_FEATURES, 0).ecx & CPUID_UMIP == 0
    {
        return false;
    }
    let mut cr4: u64;
    unsafe {
        asm!("mov {}, cr4", out(reg) cr4, options(nomem, nostack, preserves_flags));
        cr4 |= CR4_UMIP;
        asm!("mov cr4, {}", in(reg) cr4, options(nomem, nostack, preserves_flags));
    }
    user_mode_instruction_prevention_enabled()
}

pub(crate) fn user_mode_instruction_prevention_enabled() -> bool {
    let cr4: u64;
    unsafe {
        asm!("mov {}, cr4", out(reg) cr4, options(nomem, nostack, preserves_flags));
    }
    cr4 & CR4_UMIP != 0
}

pub(crate) unsafe fn with_supervisor_user_access<T>(operation: impl FnOnce() -> T) -> T {
    asm!("stac", options(nostack));
    let result = operation();
    asm!("clac", options(nostack));
    result
}

pub(crate) fn verify_user_syscall(
    entry: usize,
    stack: usize,
    host_handle: u64,
    console_handle: u64,
    ipc_handle: u64,
    service_domain: u64,
) -> bool {
    if !install_user_interrupt_gate(SYSCALL_VECTOR, ginux_syscall_entry as *const () as usize)
        || !user_interrupt_surface_is_minimal()
    {
        return false;
    }
    SYSCALL_STATE.store(0, Ordering::SeqCst);
    let Some(service_identity) = domain::domain_service_identity(service_domain) else {
        return false;
    };
    // SAFETY: The caller supplies mapped ring-3 code and stack addresses. The
    // syscall gate abandons the user frame and returns on the saved kernel
    // stack, with interrupts disabled throughout.
    unsafe {
        ginux_user_enter(
            entry,
            stack,
            host_handle,
            console_handle,
            ipc_handle,
            1,
            service_domain,
            service_identity,
        )
    };
    SYSCALL_STATE.load(Ordering::SeqCst) == 31
}

pub(crate) fn user_interrupt_surface_is_minimal() -> bool {
    let idt = core::ptr::addr_of!(IDT).cast::<IdtEntry>();
    for vector in 0..256 {
        let entry = unsafe { idt.add(vector).read() };
        let expected = if vector <= 32 || vector == 255 {
            Some(0x8e)
        } else if vector == usize::from(SYSCALL_VECTOR) {
            Some(0xee)
        } else {
            None
        };
        match expected {
            Some(attributes)
                if entry.selector == KERNEL_CODE_SELECTOR
                    && entry.attributes == attributes
                    && entry.ist == if vector == 8 { DOUBLE_FAULT_IST } else { 0 }
                    && entry.reserved == 0
                    && entry.has_handler() => {}
            None if entry.is_missing() => {}
            _ => return false,
        }
    }
    true
}

pub(crate) fn verify_breakpoint() -> bool {
    BREAKPOINT_HANDLED.store(false, Ordering::SeqCst);
    // SAFETY: Vector 3 is installed as a present interrupt gate and its handler
    // preserves the interrupted register state before returning with IRETQ.
    unsafe { asm!("int3", options(nomem, nostack)) };
    BREAKPOINT_HANDLED.load(Ordering::SeqCst)
}

pub(crate) fn double_fault_guard_page() -> usize {
    core::ptr::addr_of!(DOUBLE_FAULT_STACK) as usize
}

pub(crate) fn double_fault_upper_guard_page() -> usize {
    core::ptr::addr_of!(DOUBLE_FAULT_STACK) as usize + EXCEPTION_STACK_TOP_OFFSET
}

pub(crate) fn double_fault_stack_top() -> usize {
    core::ptr::addr_of!(DOUBLE_FAULT_STACK) as usize + EXCEPTION_STACK_TOP_OFFSET
}

pub(crate) fn privilege_stack_guard_page() -> usize {
    core::ptr::addr_of!(PRIVILEGE_STACK) as usize
}

pub(crate) fn privilege_stack_upper_guard_page() -> usize {
    core::ptr::addr_of!(PRIVILEGE_STACK) as usize + EXCEPTION_STACK_TOP_OFFSET
}

pub(crate) fn privilege_stack_top() -> usize {
    core::ptr::addr_of!(PRIVILEGE_STACK) as usize + EXCEPTION_STACK_TOP_OFFSET
}

/// Installs a non-exception interrupt gate while interrupts are disabled.
pub(crate) fn install_interrupt_gate(vector: u8, handler: usize) -> bool {
    if vector < 32 || handler == 0 {
        return false;
    }
    let idt = core::ptr::addr_of_mut!(IDT).cast::<IdtEntry>();
    // SAFETY: Bootstrap remains single-threaded with interrupts disabled. The
    // loaded IDT is writable and the vector is outside the exception range.
    unsafe {
        idt.add(usize::from(vector))
            .write(IdtEntry::interrupt(handler, 0));
        let entry = idt.add(usize::from(vector)).read();
        entry.selector == KERNEL_CODE_SELECTOR && entry.attributes == 0x8e && entry.ist == 0
    }
}

fn install_user_interrupt_gate(vector: u8, handler: usize) -> bool {
    if vector < 32 || handler == 0 {
        return false;
    }
    let idt = core::ptr::addr_of_mut!(IDT).cast::<IdtEntry>();
    unsafe {
        let mut entry = IdtEntry::interrupt(handler, 0);
        entry.attributes = 0xee;
        idt.add(usize::from(vector)).write(entry);
        idt.add(usize::from(vector)).read().attributes == 0xee
    }
}

unsafe fn install_gdt_and_tss() {
    let tss_address = core::ptr::addr_of!(TSS) as u64;
    let privilege_stack_end = privilege_stack_top() as u64;
    let exception_stack_end = double_fault_stack_top() as u64;
    TSS.interrupt_stacks[0] = exception_stack_end;
    TSS.privilege_stacks[0] = privilege_stack_end;

    GDT[0] = 0;
    GDT[1] = 0x00af_9a00_0000_ffff;
    GDT[2] = 0x00af_9200_0000_ffff;
    let limit = (size_of::<TaskStateSegment>() - 1) as u64;
    GDT[3] = limit
        | ((tss_address & 0x00ff_ffff) << 16)
        | (0x89_u64 << 40)
        | (((limit >> 16) & 0x0f) << 48)
        | (((tss_address >> 24) & 0xff) << 56);
    GDT[4] = tss_address >> 32;
    GDT[5] = 0x00cf_f200_0000_ffff;
    GDT[6] = 0x00af_fa00_0000_ffff;

    let pointer = DescriptorTablePointer {
        limit: (size_of::<[u64; 7]>() - 1) as u16,
        base: core::ptr::addr_of!(GDT) as u64,
    };

    asm!(
        "lgdt [{}]",
        "mov ax, {data}",
        "mov ds, ax",
        "mov es, ax",
        "mov ss, ax",
        "push {code}",
        "lea rax, [rip + 2f]",
        "push rax",
        "retfq",
        "2:",
        "mov ax, {tss}",
        "ltr ax",
        in(reg) &pointer,
        data = const KERNEL_DATA_SELECTOR,
        code = const KERNEL_CODE_SELECTOR as u64,
        tss = const TSS_SELECTOR,
        out("rax") _,
    );
}

unsafe fn install_idt() {
    let entries = core::ptr::addr_of!(ginux_exception_entries).cast::<usize>();
    let idt = core::ptr::addr_of_mut!(IDT).cast::<IdtEntry>();
    for vector in 0..32 {
        let ist = if vector == 8 { DOUBLE_FAULT_IST } else { 0 };
        idt.add(vector)
            .write(IdtEntry::interrupt(entries.add(vector).read(), ist));
    }

    let pointer = DescriptorTablePointer {
        limit: (size_of::<[IdtEntry; 256]>() - 1) as u16,
        base: core::ptr::addr_of!(IDT) as u64,
    };
    asm!("lidt [{}]", in(reg) &pointer, options(readonly, nostack, preserves_flags));
}

fn exception_gates_are_valid() -> bool {
    let idt = core::ptr::addr_of!(IDT).cast::<IdtEntry>();
    for vector in 0..32 {
        // SAFETY: IDT is initialized before this verifier runs and is never
        // subsequently mutated on the bootstrap processor.
        let entry = unsafe { idt.add(vector).read() };
        let expected_ist = if vector == 8 { DOUBLE_FAULT_IST } else { 0 };
        if entry.selector != KERNEL_CODE_SELECTOR
            || entry.attributes != 0x8e
            || entry.ist != expected_ist
            || (entry.offset_low == 0 && entry.offset_middle == 0 && entry.offset_high == 0)
        {
            return false;
        }
    }
    true
}

extern "sysv64" fn breakpoint_handler() {
    BREAKPOINT_HANDLED.store(true, Ordering::SeqCst);
    serial_write("ginux: breakpoint exception handled\r\n");
}

extern "sysv64" fn syscall_handler(number: u64, capability: u64, frame: *mut UserTrapFrame) -> u64 {
    let Some(caller_domain) = scheduler::current_domain() else {
        return u64::MAX;
    };
    // Termination must remain available after a domain exhausts its budget.
    if number == 2 {
        SYSCALL_STATE.fetch_or(4, Ordering::SeqCst);
        return exit_result(caller_domain, frame);
    }
    if !domain::begin_operation(caller_domain) {
        return u64::MAX;
    }
    let result = if !domain::charge_call(caller_domain) {
        CALL_BUDGET_EXHAUSTED
    } else {
        dispatch_charged_call(number, capability, caller_domain, frame)
    };
    if !domain::end_operation(caller_domain) {
        return u64::MAX;
    }
    result
}

fn dispatch_charged_call(
    number: u64,
    capability: u64,
    caller_domain: u64,
    frame: *mut UserTrapFrame,
) -> u64 {
    match number {
        0 => control_abi::DISCOVERY_WORD,
        1 => {
            if domain::handle_permits(caller_domain, capability, 1, Rights::READ) {
                SYSCALL_STATE.fetch_or(2, Ordering::SeqCst);
                0x4758
            } else {
                SYSCALL_STATE.fetch_or(1, Ordering::SeqCst);
                u64::MAX - 1
            }
        }
        3 => runtime_result(frame),
        4 => {
            let Some(frame) = (unsafe { frame.as_ref() }) else {
                return u64::MAX;
            };
            let message = frame.first_argument();
            if domain::handle_permits(caller_domain, capability, 3, Rights::WRITE)
                && ipc::send_or_wake(caller_domain, message, |service_side, sequence| {
                    scheduler::wake_ipc_peer(caller_domain, 3, service_side, message, sequence)
                })
            {
                0
            } else {
                u64::MAX - 2
            }
        }
        5 => {
            if domain::handle_permits(caller_domain, capability, 3, Rights::READ) {
                match ipc::receive(caller_domain) {
                    ipc::Receive::Message(message) => message,
                    ipc::Receive::PeerClosed(_) => scheduler::IPC_PEER_CLOSED,
                    ipc::Receive::Empty => u64::MAX - 2,
                }
            } else {
                u64::MAX - 2
            }
        }
        6 => console_write(caller_domain, capability, frame),
        7 => {
            if domain::handle_permits(caller_domain, capability, 1, Rights::MANAGE) {
                domain::take_lifecycle_event().unwrap_or(u64::MAX - 2)
            } else {
                u64::MAX - 2
            }
        }
        8 => blocking_receive(caller_domain, capability, frame),
        9 => console_write_buffer(caller_domain, capability, frame),
        10 => write_abi_descriptor(frame),
        11 => write_fault_descriptor(caller_domain, capability, frame),
        12 => write_lifecycle_descriptor(caller_domain, capability, frame),
        13 => write_ipc_delivery_descriptor(caller_domain, capability, frame),
        14 => write_lifecycle_identity_descriptor(caller_domain, capability, frame),
        _ => u64::MAX,
    }
}

fn blocking_receive(domain_id: u64, capability: u64, frame: *mut UserTrapFrame) -> u64 {
    if !domain::handle_permits(domain_id, capability, 3, Rights::READ) {
        return u64::MAX - 2;
    }
    match ipc::receive(domain_id) {
        ipc::Receive::Message(message) => return message,
        ipc::Receive::PeerClosed(_) => return scheduler::IPC_PEER_CLOSED,
        ipc::Receive::Empty => {}
    }
    let Some(service_side) = ipc::service_side(domain_id) else {
        return u64::MAX - 2;
    };
    let Some(peer_domain) = ipc::peer_domain(domain_id) else {
        return u64::MAX - 2;
    };
    let Some(frame) = (unsafe { frame.as_mut() }) else {
        return u64::MAX;
    };
    let action = scheduler::block_runtime(
        frame,
        USER_CODE_SELECTOR,
        USER_DATA_SELECTOR,
        scheduler::WaitReason::IpcReceive {
            object: 3,
            service_side,
            peer_domain,
        },
    );
    if matches!(action, RuntimeAction::Invalid) {
        u64::MAX - 2
    } else {
        resume_action(action)
    }
}

fn console_write(domain_id: u64, capability: u64, frame: *const UserTrapFrame) -> u64 {
    let Some(frame) = (unsafe { frame.as_ref() }) else {
        return u64::MAX - 2;
    };
    if !domain::handle_permits(domain_id, capability, 2, Rights::WRITE) {
        return u64::MAX - 2;
    }
    let Ok(length) = usize::try_from(frame.second_argument()) else {
        return u64::MAX - 2;
    };
    if !(1..=8).contains(&length) {
        return u64::MAX - 2;
    }
    let bytes = frame.first_argument().to_le_bytes();
    let message = &bytes[..length];
    if !message
        .iter()
        .all(|byte| matches!(*byte, b'\n' | b'\r' | 0x20..=0x7e))
    {
        return u64::MAX - 2;
    }
    let Ok(message) = core::str::from_utf8(message) else {
        return u64::MAX - 2;
    };
    serial_write(message);
    length as u64
}

fn console_write_buffer(domain_id: u64, capability: u64, frame: *const UserTrapFrame) -> u64 {
    let Some(frame) = (unsafe { frame.as_ref() }) else {
        return u64::MAX - 2;
    };
    if !domain::handle_permits(domain_id, capability, 2, Rights::WRITE) {
        return u64::MAX - 2;
    }
    let Ok(length) = usize::try_from(frame.second_argument()) else {
        return u64::MAX - 2;
    };
    if !(1..=control_abi::MAX_BUFFERED_CONSOLE_BYTES as usize).contains(&length) {
        return u64::MAX - 2;
    }
    let mut bytes = [0_u8; control_abi::MAX_BUFFERED_CONSOLE_BYTES as usize];
    if !address_space::copy_from_current_user(frame.first_argument(), &mut bytes[..length])
        || !bytes[..length]
            .iter()
            .all(|byte| matches!(*byte, b'\n' | b'\r' | 0x20..=0x7e))
    {
        return u64::MAX - 2;
    }
    let Ok(message) = core::str::from_utf8(&bytes[..length]) else {
        return u64::MAX - 2;
    };
    serial_write(message);
    SYSCALL_STATE.fetch_or(8, Ordering::SeqCst);
    length as u64
}

fn write_abi_descriptor(frame: *const UserTrapFrame) -> u64 {
    let descriptor = control_abi::descriptor();
    let Some(frame) = (unsafe { frame.as_ref() }) else {
        return u64::MAX - 2;
    };
    if frame.second_argument() != descriptor.len() as u64
        || !address_space::copy_to_current_user(frame.first_argument(), &descriptor)
    {
        return u64::MAX - 2;
    }
    SYSCALL_STATE.fetch_or(16, Ordering::SeqCst);
    descriptor.len() as u64
}

fn write_fault_descriptor(domain_id: u64, capability: u64, frame: *const UserTrapFrame) -> u64 {
    let Some(frame) = (unsafe { frame.as_ref() }) else {
        return u64::MAX - 2;
    };
    if !domain::handle_permits(domain_id, capability, 1, Rights::MANAGE)
        || frame.second_argument() != control_abi::FAULT_DESCRIPTOR_SIZE as u64
    {
        return u64::MAX - 2;
    }
    let Some(descriptor) = domain::delivered_service_fault_descriptor() else {
        return u64::MAX - 2;
    };
    if !address_space::copy_to_current_user(frame.first_argument(), &descriptor) {
        return u64::MAX - 2;
    }
    descriptor.len() as u64
}

fn write_lifecycle_descriptor(domain_id: u64, capability: u64, frame: *const UserTrapFrame) -> u64 {
    let Some(frame) = (unsafe { frame.as_ref() }) else {
        return u64::MAX - 2;
    };
    if !domain::handle_permits(domain_id, capability, 1, Rights::MANAGE)
        || frame.second_argument() != control_abi::LIFECYCLE_DESCRIPTOR_SIZE as u64
    {
        return u64::MAX - 2;
    }
    let Some(descriptor) = domain::delivered_lifecycle_event_descriptor() else {
        return u64::MAX - 2;
    };
    if !address_space::copy_to_current_user(frame.first_argument(), &descriptor) {
        return u64::MAX - 2;
    }
    descriptor.len() as u64
}

fn write_ipc_delivery_descriptor(
    domain_id: u64,
    capability: u64,
    frame: *const UserTrapFrame,
) -> u64 {
    let Some(frame) = (unsafe { frame.as_ref() }) else {
        return u64::MAX - 2;
    };
    if !domain::handle_permits(domain_id, capability, 3, Rights::READ)
        || frame.second_argument() != control_abi::IPC_DELIVERY_DESCRIPTOR_SIZE as u64
    {
        return u64::MAX - 2;
    }
    let Some(descriptor) = ipc::delivery_descriptor(domain_id) else {
        return u64::MAX - 2;
    };
    if !address_space::copy_to_current_user(frame.first_argument(), &descriptor) {
        return u64::MAX - 2;
    }
    descriptor.len() as u64
}

fn write_lifecycle_identity_descriptor(
    domain_id: u64,
    capability: u64,
    frame: *const UserTrapFrame,
) -> u64 {
    let Some(frame) = (unsafe { frame.as_ref() }) else {
        return u64::MAX - 2;
    };
    if !domain::handle_permits(domain_id, capability, 1, Rights::MANAGE)
        || frame.second_argument() != control_abi::LIFECYCLE_IDENTITY_DESCRIPTOR_SIZE as u64
    {
        return u64::MAX - 2;
    }
    let Some(descriptor) = domain::delivered_lifecycle_identity_descriptor() else {
        return u64::MAX - 2;
    };
    if !address_space::copy_to_current_user(frame.first_argument(), &descriptor) {
        return u64::MAX - 2;
    }
    descriptor.len() as u64
}

fn runtime_result(frame: *mut UserTrapFrame) -> u64 {
    let Some(frame) = (unsafe { frame.as_mut() }) else {
        return u64::MAX;
    };
    let action = scheduler::yield_runtime(frame, USER_CODE_SELECTOR, USER_DATA_SELECTOR);
    resume_action(action)
}

fn exit_result(caller_domain: u64, frame: *mut UserTrapFrame) -> u64 {
    let Some(frame) = (unsafe { frame.as_mut() }) else {
        return u64::MAX;
    };
    let service_exit = domain::is_supervised_service_domain(caller_domain);
    let action = scheduler::exit_runtime(
        frame,
        USER_CODE_SELECTOR,
        USER_DATA_SELECTOR,
        Some(scheduler::WaitReason::IpcReceive {
            object: 3,
            service_side: !service_exit,
            peer_domain: caller_domain,
        }),
    );
    if matches!(action, RuntimeAction::Invalid) || !domain::detach_thread(caller_domain) {
        return u64::MAX;
    }
    if service_exit
        && (!ipc::retire_service(caller_domain) || !domain::retire_service(caller_domain))
    {
        return u64::MAX;
    }
    resume_action(action)
}

extern "sysv64" fn user_fault_handler(frame: *mut UserFaultFrame) -> u64 {
    let Some(frame) = (unsafe { frame.as_mut() }) else {
        return u64::MAX;
    };
    let fault_domain = scheduler::current_domain();
    let service_fault = fault_domain.is_some_and(domain::is_supervised_service_domain);
    let fault_address: u64;
    unsafe {
        asm!("mov {}, cr2", out(reg) fault_address, options(nomem, nostack, preserves_flags));
    }
    let fault_metadata = frame.metadata(fault_address);
    let action = scheduler::fault_runtime(
        frame,
        fault_address,
        USER_CODE_SELECTOR,
        USER_DATA_SELECTOR,
        fault_domain.map(|dead_domain| scheduler::WaitReason::IpcReceive {
            object: 3,
            service_side: !service_fault,
            peer_domain: dead_domain,
        }),
    );
    let Some(fault_domain) = fault_domain else {
        return u64::MAX;
    };
    if matches!(action, RuntimeAction::Invalid) || !domain::detach_thread(fault_domain) {
        return u64::MAX;
    }
    if service_fault && !scrub_service_restart_stack() {
        return u64::MAX;
    }
    if service_fault
        && (!domain::record_service_fault(fault_domain, fault_metadata)
            || !ipc::retire_service(fault_domain)
            || !domain::retire_service(fault_domain))
    {
        return u64::MAX;
    }
    if service_fault {
        match setup_service_restart(fault_domain) {
            ServiceRestartSetup::Started => {}
            ServiceRestartSetup::Unavailable => return resume_action(action),
            ServiceRestartSetup::RollbackFailed => return u64::MAX,
        }
    }
    resume_action(action)
}

enum ServiceRestartSetup {
    Started,
    Unavailable,
    RollbackFailed,
}

fn setup_service_restart(fault_domain: u64) -> ServiceRestartSetup {
    let Some(replacement_domain) = domain::initialize_boot_service_restart() else {
        return ServiceRestartSetup::Unavailable;
    };
    let Some(replacement_handle) = domain::service_boot_handle() else {
        return if domain::discard_unstarted_service(replacement_domain) {
            ServiceRestartSetup::Unavailable
        } else {
            ServiceRestartSetup::RollbackFailed
        };
    };
    if !ipc::bind_service(replacement_domain) {
        return if domain::discard_unstarted_service(replacement_domain) {
            ServiceRestartSetup::Unavailable
        } else {
            ServiceRestartSetup::RollbackFailed
        };
    }
    let (Some(old_owner), Some(replacement_owner)) = (
        memory::FrameOwner::domain(fault_domain),
        memory::FrameOwner::domain(replacement_domain),
    ) else {
        return rollback_unstarted_replacement(replacement_domain, true);
    };
    if memory::transfer_frame_owner(old_owner, replacement_owner)
        != Some(address_space::USER_ADDRESS_SPACE_FRAMES)
    {
        return rollback_unstarted_replacement(replacement_domain, true);
    }
    if !domain::adopt_quarantined_service_frames(fault_domain, replacement_domain) {
        let _ = memory::transfer_frame_owner(replacement_owner, old_owner);
        return rollback_unstarted_replacement(replacement_domain, true);
    }
    if !scheduler::queue_service_restart(replacement_domain, replacement_handle) {
        return rollback_transferred_replacement(
            replacement_domain,
            fault_domain,
            old_owner,
            replacement_owner,
            false,
        );
    }
    if !timer::rearm_scheduler_preemption() {
        return rollback_transferred_replacement(
            replacement_domain,
            fault_domain,
            old_owner,
            replacement_owner,
            true,
        );
    }
    SERVICE_FRAMES_TRANSFERRED.store(true, Ordering::Release);
    ServiceRestartSetup::Started
}

fn rollback_unstarted_replacement(replacement_domain: u64, bound: bool) -> ServiceRestartSetup {
    if (!bound || ipc::rollback_service_binding(replacement_domain))
        && domain::discard_unstarted_service(replacement_domain)
    {
        ServiceRestartSetup::Unavailable
    } else {
        ServiceRestartSetup::RollbackFailed
    }
}

fn rollback_transferred_replacement(
    replacement_domain: u64,
    fault_domain: u64,
    old_owner: memory::FrameOwner,
    replacement_owner: memory::FrameOwner,
    queued: bool,
) -> ServiceRestartSetup {
    if (!queued || scheduler::rollback_service_restart(replacement_domain, fault_domain))
        && memory::transfer_frame_owner(replacement_owner, old_owner)
            == Some(address_space::USER_ADDRESS_SPACE_FRAMES)
        && domain::restore_quarantined_service_frames(fault_domain, replacement_domain)
        && ipc::rollback_service_binding(replacement_domain)
        && domain::discard_unstarted_service(replacement_domain)
    {
        ServiceRestartSetup::Unavailable
    } else {
        ServiceRestartSetup::RollbackFailed
    }
}

fn scrub_service_restart_stack() -> bool {
    const ADDRESS_MASK: u64 = 0x000f_ffff_ffff_f000;
    let Some((root, stack_top)) = scheduler::service_restart_stack_mapping() else {
        return false;
    };
    let active_root: u64;
    unsafe {
        asm!("mov {}, cr3", out(reg) active_root, options(nomem, nostack, preserves_flags));
    }
    let Some(stack_base) = stack_top.checked_sub(PAGE_SIZE as u64) else {
        return false;
    };
    if active_root & ADDRESS_MASK != root || stack_top % PAGE_SIZE as u64 != 0 || stack_base == 0 {
        return false;
    }
    let stack = stack_base as *mut u8;
    let scrubbed = unsafe {
        with_supervisor_user_access(|| {
            for offset in 0..PAGE_SIZE {
                stack.add(offset).write_volatile(0);
            }
            for offset in 0..PAGE_SIZE / size_of::<u64>() {
                if stack.cast::<u64>().add(offset).read_volatile() != 0 {
                    return false;
                }
            }
            true
        })
    };
    if !scrubbed {
        return false;
    }
    SERVICE_STACK_SCRUBBED.store(true, Ordering::Release);
    true
}

pub(crate) fn service_restart_stack_was_scrubbed() -> bool {
    SERVICE_STACK_SCRUBBED.load(Ordering::Acquire)
}

pub(crate) fn service_address_space_frames_were_transferred() -> bool {
    SERVICE_FRAMES_TRANSFERRED.load(Ordering::Acquire)
}

fn resume_action(action: RuntimeAction) -> u64 {
    match action {
        RuntimeAction::Resume {
            result,
            address_space_root,
        } if activate_user_root(address_space_root) => result,
        RuntimeAction::Resume { .. } => u64::MAX,
        RuntimeAction::Complete | RuntimeAction::Invalid => u64::MAX,
    }
}

pub(crate) fn activate_user_root(root: u64) -> bool {
    const ADDRESS_MASK: u64 = 0x000f_ffff_ffff_f000;
    if root == 0
        || root & !ADDRESS_MASK != 0
        || scheduler::current_address_space_root() != Some(root)
    {
        return false;
    }
    let Some(owner) = scheduler::current_domain().and_then(memory::FrameOwner::domain) else {
        return false;
    };
    let Some((instruction, stack)) = scheduler::current_instruction_and_stack() else {
        return false;
    };
    if !paging::user_context_matches_layout(instruction, stack) {
        return false;
    }
    let Some(frames) = paging::validated_user_root_frames(root) else {
        return false;
    };
    if !memory::frame_addresses_owned_by(owner, &frames) {
        return false;
    }
    let active: u64;
    unsafe {
        asm!("mov cr3, {}", in(reg) root, options(nostack, preserves_flags));
        asm!("mov {}, cr3", out(reg) active, options(nomem, nostack, preserves_flags));
    }
    active & ADDRESS_MASK == root
}

unsafe fn read_msr(msr: u32) -> u64 {
    let low: u32;
    let high: u32;
    asm!("rdmsr", in("ecx") msr, out("eax") low, out("edx") high, options(nomem, nostack, preserves_flags));
    (u64::from(high) << 32) | u64::from(low)
}

unsafe fn write_msr(msr: u32, value: u64) {
    asm!("wrmsr", in("ecx") msr, in("eax") value as u32, in("edx") (value >> 32) as u32, options(nomem, nostack, preserves_flags));
}

extern "sysv64" fn fatal_exception_handler(
    vector: u64,
    error: u64,
    fault_address: u64,
    instruction: u64,
) -> ! {
    serial_write("ginux: fatal exception vector: ");
    serial_write_u64(vector);
    serial_write("\r\nginux: exception error code: ");
    serial_write_u64(error);
    serial_write("\r\nginux: exception instruction: ");
    serial_write_u64(instruction);
    if vector == 14 {
        serial_write("\r\nginux: page fault address: ");
        serial_write_u64(fault_address);
    }
    serial_write("\r\nGINUX_KERNEL_PANIC\r\n");
    loop {
        core::hint::spin_loop();
    }
}
