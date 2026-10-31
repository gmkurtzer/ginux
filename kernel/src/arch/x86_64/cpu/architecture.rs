use core::{arch::asm, cell::UnsafeCell, mem::size_of, sync::atomic::AtomicBool};

use super::{
    ginux_exception_entries, read_msr, write_msr, BOOT_CPU_INDEX, CPU_ENTRY_STATE_LIMIT,
    CPU_KERNEL_STACK_STORAGE_SIZE, CPU_KERNEL_STACK_TOP_OFFSET, DOUBLE_FAULT_IST,
    EXCEPTION_STACK_STORAGE_SIZE, EXCEPTION_STACK_TOP_OFFSET, GS_BASE_MSR, IST_STACK_STORAGE_SIZE,
    IST_STACK_TOP_OFFSET, KERNEL_CODE_SELECTOR, KERNEL_DATA_SELECTOR, KERNEL_GS_BASE_MSR,
    MACHINE_CHECK_IST, NMI_IST, SYSCALL_VECTOR, TSS_SELECTOR,
};

#[derive(Clone, Copy)]
#[repr(C, packed)]
pub(super) struct TaskStateSegment {
    pub(super) reserved0: u32,
    pub(super) privilege_stacks: [u64; 3],
    pub(super) reserved1: u64,
    pub(super) interrupt_stacks: [u64; 7],
    pub(super) reserved2: u64,
    pub(super) reserved3: u16,
    pub(super) io_map_base: u16,
}

#[derive(Clone, Copy)]
#[repr(C, packed)]
pub(super) struct IdtEntry {
    pub(super) offset_low: u16,
    pub(super) selector: u16,
    pub(super) ist: u8,
    pub(super) attributes: u8,
    pub(super) offset_middle: u16,
    pub(super) offset_high: u32,
    pub(super) reserved: u32,
}

impl IdtEntry {
    pub(super) const MISSING: Self = Self {
        offset_low: 0,
        selector: 0,
        ist: 0,
        attributes: 0,
        offset_middle: 0,
        offset_high: 0,
        reserved: 0,
    };

    pub(super) fn interrupt(handler: usize, ist: u8) -> Self {
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

    pub(super) fn is_missing(self) -> bool {
        self.offset_low == 0
            && self.selector == 0
            && self.ist == 0
            && self.attributes == 0
            && self.offset_middle == 0
            && self.offset_high == 0
            && self.reserved == 0
    }

    pub(super) fn has_handler(self) -> bool {
        self.offset_low != 0 || self.offset_middle != 0 || self.offset_high != 0
    }
}

#[repr(C, packed)]
pub(super) struct DescriptorTablePointer {
    pub(super) limit: u16,
    pub(super) base: u64,
}

#[repr(C, align(4096))]
pub(super) struct ExceptionStack([u8; EXCEPTION_STACK_STORAGE_SIZE]);

#[repr(C, align(4096))]
pub(super) struct IstStack([u8; IST_STACK_STORAGE_SIZE]);

#[repr(C, align(4096))]
pub(super) struct CpuKernelStack([u8; CPU_KERNEL_STACK_STORAGE_SIZE]);

#[derive(Clone, Copy)]
#[repr(C, align(64))]
pub(super) struct CpuEntryState {
    pub(super) return_stack: u64,
    pub(super) user_rsp: u64,
    pub(super) user_rip: u64,
    pub(super) user_rflags: u64,
    pub(super) kernel_frame: u64,
    pub(super) syscall_result: u64,
    pub(super) signal_rax: u64,
    pub(super) signal_rcx: u64,
    pub(super) signal_r11: u64,
}

impl CpuEntryState {
    pub(super) const EMPTY: Self = Self {
        return_stack: 0,
        user_rsp: 0,
        user_rip: 0,
        user_rflags: 0,
        kernel_frame: 0,
        syscall_result: 0,
        signal_rax: 0,
        signal_rcx: 0,
        signal_r11: 0,
    };
}

pub(super) struct CpuArchitectureOwner {
    pub(super) extended_user_state_enabled: AtomicBool,
    pub(super) entry_states: UnsafeCell<[CpuEntryState; CPU_ENTRY_STATE_LIMIT]>,
    pub(super) gdt: UnsafeCell<[[u64; 7]; CPU_ENTRY_STATE_LIMIT]>,
    pub(super) tss: UnsafeCell<[TaskStateSegment; CPU_ENTRY_STATE_LIMIT]>,
    pub(super) idt: UnsafeCell<[[IdtEntry; 256]; CPU_ENTRY_STATE_LIMIT]>,
    pub(super) privilege_stack: UnsafeCell<[ExceptionStack; CPU_ENTRY_STATE_LIMIT]>,
    pub(super) kernel_stack: UnsafeCell<[CpuKernelStack; CPU_ENTRY_STATE_LIMIT]>,
    pub(super) double_fault_stack: UnsafeCell<[IstStack; CPU_ENTRY_STATE_LIMIT]>,
    pub(super) nmi_stack: UnsafeCell<[IstStack; CPU_ENTRY_STATE_LIMIT]>,
    pub(super) machine_check_stack: UnsafeCell<[IstStack; CPU_ENTRY_STATE_LIMIT]>,
}

unsafe impl Sync for CpuArchitectureOwner {}

pub(super) static CPU_ARCHITECTURE: CpuArchitectureOwner = CpuArchitectureOwner {
    extended_user_state_enabled: AtomicBool::new(false),
    entry_states: UnsafeCell::new([CpuEntryState::EMPTY; CPU_ENTRY_STATE_LIMIT]),
    gdt: UnsafeCell::new([[0; 7]; CPU_ENTRY_STATE_LIMIT]),
    tss: UnsafeCell::new(
        [const {
            TaskStateSegment {
                reserved0: 0,
                privilege_stacks: [0; 3],
                reserved1: 0,
                interrupt_stacks: [0; 7],
                reserved2: 0,
                reserved3: 0,
                io_map_base: size_of::<TaskStateSegment>() as u16,
            }
        }; CPU_ENTRY_STATE_LIMIT],
    ),
    idt: UnsafeCell::new([[IdtEntry::MISSING; 256]; CPU_ENTRY_STATE_LIMIT]),
    privilege_stack: UnsafeCell::new(
        [const { ExceptionStack([0; EXCEPTION_STACK_STORAGE_SIZE]) }; CPU_ENTRY_STATE_LIMIT],
    ),
    kernel_stack: UnsafeCell::new(
        [const { CpuKernelStack([0; CPU_KERNEL_STACK_STORAGE_SIZE]) }; CPU_ENTRY_STATE_LIMIT],
    ),
    double_fault_stack: UnsafeCell::new(
        [const { IstStack([0; IST_STACK_STORAGE_SIZE]) }; CPU_ENTRY_STATE_LIMIT],
    ),
    nmi_stack: UnsafeCell::new(
        [const { IstStack([0; IST_STACK_STORAGE_SIZE]) }; CPU_ENTRY_STATE_LIMIT],
    ),
    machine_check_stack: UnsafeCell::new(
        [const { IstStack([0; IST_STACK_STORAGE_SIZE]) }; CPU_ENTRY_STATE_LIMIT],
    ),
};

pub(crate) fn install() -> bool {
    install_for_cpu(BOOT_CPU_INDEX)
}

pub(crate) fn install_for_cpu(cpu: usize) -> bool {
    if cpu >= CPU_ENTRY_STATE_LIMIT {
        return false;
    }
    // SAFETY: Initialization runs once with interrupts disabled and before any
    // other CPU can observe this CPU's private tables.
    unsafe {
        let entry_state = CPU_ARCHITECTURE
            .entry_states
            .get()
            .cast::<CpuEntryState>()
            .add(cpu);
        entry_state.write(CpuEntryState::EMPTY);
        write_msr(GS_BASE_MSR, entry_state as u64);
        // Ring 3 begins with the architectural user GS base.  Linux syscall
        // and user-fault entry use SWAPGS to recover this CPU's private entry
        // state without exposing it through ordinary userspace GS accesses.
        write_msr(KERNEL_GS_BASE_MSR, 0);
        install_gdt_and_tss(cpu);
        install_idt(cpu);
    }
    entry_state_is_installed() && exception_gates_are_valid(cpu)
}

pub(super) fn current_entry_state() -> *mut CpuEntryState {
    // SAFETY: `install` assigns GS_BASE to this CPU's entry-state slot before
    // userspace or interrupts can use the privilege-entry paths.
    unsafe { read_msr(GS_BASE_MSR) as *mut CpuEntryState }
}

fn entry_state_is_installed() -> bool {
    let base = current_entry_state() as usize;
    let states = CPU_ARCHITECTURE
        .entry_states
        .get()
        .cast_const()
        .cast::<CpuEntryState>() as usize;
    base >= states
        && base < states + size_of::<[CpuEntryState; CPU_ENTRY_STATE_LIMIT]>()
        && (base - states).is_multiple_of(size_of::<CpuEntryState>())
}

pub(crate) fn current_cpu_index() -> usize {
    current_cpu_index_checked().expect("CPU entry state is installed")
}

pub(crate) fn current_cpu_index_checked() -> Option<usize> {
    let base = current_entry_state() as usize;
    let states = CPU_ARCHITECTURE
        .entry_states
        .get()
        .cast_const()
        .cast::<CpuEntryState>() as usize;
    (base >= states
        && base < states + size_of::<[CpuEntryState; CPU_ENTRY_STATE_LIMIT]>()
        && (base - states).is_multiple_of(size_of::<CpuEntryState>()))
    .then_some((base - states) / size_of::<CpuEntryState>())
}

pub(crate) fn task_register_denies_user_io() -> bool {
    let selector: u16;
    unsafe {
        asm!("str {selector:x}", selector = out(reg) selector, options(nomem, nostack, preserves_flags));
    }
    let cpu = current_cpu_index();
    let gdt = CPU_ARCHITECTURE.gdt.get().cast_const().cast::<[u64; 7]>();
    let low = unsafe { core::ptr::addr_of!((*gdt.add(cpu))[3]).read() };
    let high = unsafe {
        CPU_ARCHITECTURE
            .gdt
            .get()
            .cast_const()
            .cast::<[u64; 7]>()
            .add(cpu)
            .cast::<u64>()
            .add(4)
            .read()
    };
    let limit = (low & 0xffff) | ((low >> 32) & 0x000f_0000);
    let base =
        ((low >> 16) & 0x00ff_ffff) | ((low >> 32) & 0xff00_0000) | ((high & 0xffff_ffff) << 32);
    let io_map_base = unsafe {
        let tss = CPU_ARCHITECTURE
            .tss
            .get()
            .cast::<TaskStateSegment>()
            .add(cpu);
        core::ptr::addr_of!((*tss).io_map_base).read_unaligned()
    };
    selector == TSS_SELECTOR
        && low & (1 << 47) != 0
        && (low >> 40) & 0x0f == 0x0b
        && limit == (size_of::<TaskStateSegment>() - 1) as u64
        && base
            == unsafe {
                CPU_ARCHITECTURE
                    .tss
                    .get()
                    .cast::<TaskStateSegment>()
                    .add(cpu)
            } as u64
        && u64::from(io_map_base) > limit
}

pub(crate) fn user_interrupt_surface_is_minimal() -> bool {
    let idt = current_idt();
    for vector in 0..256 {
        let entry = unsafe { idt.add(vector).read() };
        if vector == usize::from(SYSCALL_VECTOR) {
            if entry.selector != KERNEL_CODE_SELECTOR
                || entry.attributes != 0xee
                || entry.ist != 0
                || entry.reserved != 0
                || !entry.has_handler()
            {
                return false;
            }
        } else if vector <= 32 || vector == 255 || !entry.is_missing() {
            // Kernel interrupt sources may populate additional vectors, but
            // none may enlarge the user-callable surface beyond INT 0x80.
            if entry.selector != KERNEL_CODE_SELECTOR
                || entry.attributes != 0x8e
                || entry.ist != exception_ist(vector)
                || entry.reserved != 0
                || !entry.has_handler()
            {
                return false;
            }
        }
    }
    true
}

fn current_idt() -> *const IdtEntry {
    unsafe {
        CPU_ARCHITECTURE
            .idt
            .get()
            .cast_const()
            .cast::<[IdtEntry; 256]>()
            .add(current_cpu_index())
            .cast::<IdtEntry>()
    }
}

pub(crate) fn double_fault_guard_page() -> usize {
    stack_address(CPU_ARCHITECTURE.double_fault_stack.get(), BOOT_CPU_INDEX)
}

pub(crate) fn double_fault_stack_top() -> usize {
    CPU_ARCHITECTURE.double_fault_stack.get().cast_const() as usize + IST_STACK_TOP_OFFSET
}

pub(crate) fn nmi_guard_page() -> usize {
    stack_address(CPU_ARCHITECTURE.nmi_stack.get(), BOOT_CPU_INDEX)
}

#[allow(dead_code)]
pub(crate) fn nmi_upper_guard_page() -> usize {
    nmi_guard_page() + IST_STACK_TOP_OFFSET
}

pub(crate) fn nmi_stack_top() -> usize {
    CPU_ARCHITECTURE.nmi_stack.get().cast_const() as usize + IST_STACK_TOP_OFFSET
}

pub(crate) fn machine_check_guard_page() -> usize {
    stack_address(CPU_ARCHITECTURE.machine_check_stack.get(), BOOT_CPU_INDEX)
}

#[allow(dead_code)]
pub(crate) fn machine_check_upper_guard_page() -> usize {
    machine_check_guard_page() + IST_STACK_TOP_OFFSET
}

pub(crate) fn machine_check_stack_top() -> usize {
    CPU_ARCHITECTURE.machine_check_stack.get().cast_const() as usize + IST_STACK_TOP_OFFSET
}

pub(crate) fn privilege_stack_guard_page() -> usize {
    exception_stack_address(BOOT_CPU_INDEX)
}

pub(crate) fn privilege_stack_top() -> usize {
    CPU_ARCHITECTURE.privilege_stack.get().cast_const() as usize + EXCEPTION_STACK_TOP_OFFSET
}

pub(crate) fn kernel_stack_top_for_cpu(cpu: usize) -> Option<usize> {
    (cpu < CPU_ENTRY_STATE_LIMIT).then(|| {
        stack_address(CPU_ARCHITECTURE.kernel_stack.get(), cpu) + CPU_KERNEL_STACK_TOP_OFFSET
    })
}

pub(crate) fn stack_guard_pages() -> [usize; CPU_ENTRY_STATE_LIMIT * 10] {
    let mut pages = [0; CPU_ENTRY_STATE_LIMIT * 10];
    for cpu in 0..CPU_ENTRY_STATE_LIMIT {
        let bases = [
            exception_stack_address(cpu),
            stack_address(CPU_ARCHITECTURE.kernel_stack.get(), cpu),
            stack_address(CPU_ARCHITECTURE.double_fault_stack.get(), cpu),
            stack_address(CPU_ARCHITECTURE.nmi_stack.get(), cpu),
            stack_address(CPU_ARCHITECTURE.machine_check_stack.get(), cpu),
        ];
        let sizes = [
            EXCEPTION_STACK_TOP_OFFSET,
            CPU_KERNEL_STACK_TOP_OFFSET,
            IST_STACK_TOP_OFFSET,
            IST_STACK_TOP_OFFSET,
            IST_STACK_TOP_OFFSET,
        ];
        for stack in 0..5 {
            pages[cpu * 10 + stack * 2] = bases[stack];
            pages[cpu * 10 + stack * 2 + 1] = bases[stack] + sizes[stack];
        }
    }
    pages
}

fn exception_stack_address(cpu: usize) -> usize {
    unsafe {
        CPU_ARCHITECTURE
            .privilege_stack
            .get()
            .cast::<ExceptionStack>()
            .add(cpu) as usize
    }
}

fn stack_address<T>(stacks: *mut [T; CPU_ENTRY_STATE_LIMIT], cpu: usize) -> usize {
    unsafe { stacks.cast::<T>().add(cpu) as usize }
}

/// Installs a non-exception interrupt gate while interrupts are disabled.
pub(crate) fn install_interrupt_gate(vector: u8, handler: usize) -> bool {
    if vector < 32 || handler == 0 {
        return false;
    }
    // SAFETY: Bootstrap remains single-threaded with interrupts disabled. The
    // loaded IDT is writable and the vector is outside the exception range.
    unsafe {
        for cpu in 0..CPU_ENTRY_STATE_LIMIT {
            let idt = CPU_ARCHITECTURE
                .idt
                .get()
                .cast::<[IdtEntry; 256]>()
                .add(cpu)
                .cast::<IdtEntry>();
            idt.add(usize::from(vector))
                .write(IdtEntry::interrupt(handler, 0));
        }
        let entry = current_idt().add(usize::from(vector)).read();
        entry.selector == KERNEL_CODE_SELECTOR && entry.attributes == 0x8e && entry.ist == 0
    }
}

/// Removes a temporary kernel interrupt gate while interrupts are disabled.
pub(crate) fn clear_interrupt_gate(vector: u8) -> bool {
    if vector <= 32 || vector == SYSCALL_VECTOR || vector == u8::MAX {
        return false;
    }
    // SAFETY: The caller has quiesced the interrupt source and keeps
    // interrupts disabled while replacing this non-reserved gate.
    unsafe {
        for cpu in 0..CPU_ENTRY_STATE_LIMIT {
            let idt = CPU_ARCHITECTURE
                .idt
                .get()
                .cast::<[IdtEntry; 256]>()
                .add(cpu)
                .cast::<IdtEntry>();
            idt.add(usize::from(vector)).write(IdtEntry::MISSING);
        }
        current_idt().add(usize::from(vector)).read().is_missing()
    }
}

pub(super) fn install_user_interrupt_gate(vector: u8, handler: usize) -> bool {
    if vector < 32 || handler == 0 {
        return false;
    }
    unsafe {
        let mut entry = IdtEntry::interrupt(handler, 0);
        entry.attributes = 0xee;
        for cpu in 0..CPU_ENTRY_STATE_LIMIT {
            let idt = CPU_ARCHITECTURE
                .idt
                .get()
                .cast::<[IdtEntry; 256]>()
                .add(cpu)
                .cast::<IdtEntry>();
            idt.add(usize::from(vector)).write(entry);
        }
        current_idt().add(usize::from(vector)).read().attributes == 0xee
    }
}

unsafe fn install_gdt_and_tss(cpu: usize) {
    let tss = &mut *CPU_ARCHITECTURE
        .tss
        .get()
        .cast::<TaskStateSegment>()
        .add(cpu);
    let gdt = &mut *CPU_ARCHITECTURE.gdt.get().cast::<[u64; 7]>().add(cpu);
    let tss_address = core::ptr::from_ref(tss) as u64;
    let privilege_stack_end = (exception_stack_address(cpu) + EXCEPTION_STACK_TOP_OFFSET) as u64;
    tss.interrupt_stacks[usize::from(DOUBLE_FAULT_IST - 1)] =
        (stack_address(CPU_ARCHITECTURE.double_fault_stack.get(), cpu) + IST_STACK_TOP_OFFSET)
            as u64;
    tss.interrupt_stacks[usize::from(NMI_IST - 1)] =
        (stack_address(CPU_ARCHITECTURE.nmi_stack.get(), cpu) + IST_STACK_TOP_OFFSET) as u64;
    tss.interrupt_stacks[usize::from(MACHINE_CHECK_IST - 1)] =
        (stack_address(CPU_ARCHITECTURE.machine_check_stack.get(), cpu) + IST_STACK_TOP_OFFSET)
            as u64;
    tss.privilege_stacks[0] = privilege_stack_end;

    gdt[0] = 0;
    gdt[1] = 0x00af_9a00_0000_ffff;
    gdt[2] = 0x00af_9200_0000_ffff;
    let limit = (size_of::<TaskStateSegment>() - 1) as u64;
    gdt[3] = limit
        | ((tss_address & 0x00ff_ffff) << 16)
        | (0x89_u64 << 40)
        | (((limit >> 16) & 0x0f) << 48)
        | (((tss_address >> 24) & 0xff) << 56);
    gdt[4] = tss_address >> 32;
    gdt[5] = 0x00cf_f200_0000_ffff;
    gdt[6] = 0x00af_fa00_0000_ffff;

    let pointer = DescriptorTablePointer {
        limit: (size_of::<[u64; 7]>() - 1) as u16,
        base: core::ptr::from_ref(gdt) as u64,
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

unsafe fn install_idt(cpu: usize) {
    let entries = core::ptr::addr_of!(ginux_exception_entries).cast::<usize>();
    let idt = CPU_ARCHITECTURE
        .idt
        .get()
        .cast::<[IdtEntry; 256]>()
        .add(cpu)
        .cast::<IdtEntry>();
    for vector in 0..32 {
        let ist = exception_ist(vector);
        idt.add(vector)
            .write(IdtEntry::interrupt(entries.add(vector).read(), ist));
    }

    let pointer = DescriptorTablePointer {
        limit: (size_of::<[IdtEntry; 256]>() - 1) as u16,
        base: CPU_ARCHITECTURE.idt.get().cast_const() as u64,
    };
    asm!("lidt [{}]", in(reg) &pointer, options(readonly, nostack, preserves_flags));
}

fn exception_gates_are_valid(cpu: usize) -> bool {
    let idt = unsafe {
        CPU_ARCHITECTURE
            .idt
            .get()
            .cast_const()
            .cast::<[IdtEntry; 256]>()
            .add(cpu)
            .cast::<IdtEntry>()
    };
    for vector in 0..32 {
        // SAFETY: IDT is initialized before this verifier runs and is never
        // subsequently mutated on the bootstrap processor.
        let entry = unsafe { idt.add(vector).read() };
        let expected_ist = exception_ist(vector);
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

const fn exception_ist(vector: usize) -> u8 {
    match vector {
        2 => NMI_IST,
        8 => DOUBLE_FAULT_IST,
        18 => MACHINE_CHECK_IST,
        _ => 0,
    }
}
