use core::arch::{asm, global_asm};
use core::mem::size_of;
use core::sync::atomic::{AtomicBool, AtomicU8, Ordering};

use crate::{serial_write, serial_write_u64};

const KERNEL_CODE_SELECTOR: u16 = 0x08;
const KERNEL_DATA_SELECTOR: u16 = 0x10;
const TSS_SELECTOR: u16 = 0x18;
const USER_DATA_SELECTOR: u16 = 0x2b;
const USER_CODE_SELECTOR: u16 = 0x33;
const SYSCALL_VECTOR: u8 = 0x80;
const DOUBLE_FAULT_IST: u8 = 1;
const PAGE_SIZE: usize = 4096;
const EXCEPTION_STACK_SIZE: usize = 16 * 1024;
const EXCEPTION_STACK_STORAGE_SIZE: usize = PAGE_SIZE + EXCEPTION_STACK_SIZE;

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
static mut DOUBLE_FAULT_STACK: ExceptionStack = ExceptionStack([0; EXCEPTION_STACK_STORAGE_SIZE]);

global_asm!(
    r#"
    .text
    .global ginux_breakpoint_entry
ginux_breakpoint_entry:
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
    mov rdi, [rsp + 120]
    mov rsi, [rsp + 128]
    mov rdx, cr2
    and rsp, -16
    call {fatal_exception_handler}
    ud2

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
    mov [rip + {user_return_stack}], rsp
    push {user_data}
    push rsi
    push 2
    push {user_code}
    push rdi
    iretq

    .global ginux_syscall_entry
ginux_syscall_entry:
    cli
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
    and rsp, -16
    call {syscall_handler}
    cmp rax, -1
    je 3f
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
3:
    mov rsp, [rip + {user_return_stack}]
    ret
"#,
    breakpoint_handler = sym breakpoint_handler,
    fatal_exception_handler = sym fatal_exception_handler,
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
    fn ginux_user_enter(entry: usize, stack: usize);
}

pub(crate) fn install(kernel_stack_top: usize) -> bool {
    // SAFETY: Initialization runs once with interrupts disabled and before any
    // other CPU can observe these tables.
    unsafe {
        install_gdt_and_tss(kernel_stack_top);
        install_idt();
    }
    exception_gates_are_valid()
}

pub(crate) fn verify_user_syscall(entry: usize, stack: usize) -> bool {
    if !install_user_interrupt_gate(SYSCALL_VECTOR, ginux_syscall_entry as *const () as usize) {
        return false;
    }
    SYSCALL_STATE.store(0, Ordering::SeqCst);
    // SAFETY: The caller supplies mapped ring-3 code and stack addresses. The
    // syscall gate abandons the user frame and returns on the saved kernel
    // stack, with interrupts disabled throughout.
    unsafe { ginux_user_enter(entry, stack) };
    SYSCALL_STATE.load(Ordering::SeqCst) == 3
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

unsafe fn install_gdt_and_tss(kernel_stack_top: usize) {
    let tss_address = core::ptr::addr_of!(TSS) as u64;
    let exception_stack_end =
        core::ptr::addr_of!(DOUBLE_FAULT_STACK) as u64 + EXCEPTION_STACK_STORAGE_SIZE as u64;
    TSS.interrupt_stacks[0] = exception_stack_end;
    TSS.privilege_stacks[0] = kernel_stack_top as u64;

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

extern "sysv64" fn syscall_handler(number: u64) -> u64 {
    match number {
        1 => {
            SYSCALL_STATE.fetch_or(1, Ordering::SeqCst);
            0x4758
        }
        2 => {
            SYSCALL_STATE.fetch_or(2, Ordering::SeqCst);
            u64::MAX
        }
        _ => u64::MAX,
    }
}

extern "sysv64" fn fatal_exception_handler(vector: u64, error: u64, fault_address: u64) -> ! {
    serial_write("ginux: fatal exception vector: ");
    serial_write_u64(vector);
    serial_write("\r\nginux: exception error code: ");
    serial_write_u64(error);
    if vector == 14 {
        serial_write("\r\nginux: page fault address: ");
        serial_write_u64(fault_address);
    }
    serial_write("\r\nGINUX_KERNEL_PANIC\r\n");
    loop {
        core::hint::spin_loop();
    }
}
