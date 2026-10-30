use core::{cell::UnsafeCell, mem::size_of, sync::atomic::AtomicBool};

use super::{
    CPU_ENTRY_STATE_LIMIT, EXCEPTION_STACK_STORAGE_SIZE, IST_STACK_STORAGE_SIZE,
    KERNEL_CODE_SELECTOR,
};

#[repr(C, packed)]
#[derive(Clone, Copy)]
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
    pub(super) gdt: UnsafeCell<[u64; 7]>,
    pub(super) tss: UnsafeCell<TaskStateSegment>,
    pub(super) idt: UnsafeCell<[IdtEntry; 256]>,
    pub(super) privilege_stack: UnsafeCell<ExceptionStack>,
    pub(super) double_fault_stack: UnsafeCell<IstStack>,
    pub(super) nmi_stack: UnsafeCell<IstStack>,
    pub(super) machine_check_stack: UnsafeCell<IstStack>,
}

unsafe impl Sync for CpuArchitectureOwner {}

pub(super) static CPU_ARCHITECTURE: CpuArchitectureOwner = CpuArchitectureOwner {
    extended_user_state_enabled: AtomicBool::new(false),
    entry_states: UnsafeCell::new([CpuEntryState::EMPTY; CPU_ENTRY_STATE_LIMIT]),
    gdt: UnsafeCell::new([0; 7]),
    tss: UnsafeCell::new(TaskStateSegment {
        reserved0: 0,
        privilege_stacks: [0; 3],
        reserved1: 0,
        interrupt_stacks: [0; 7],
        reserved2: 0,
        reserved3: 0,
        io_map_base: size_of::<TaskStateSegment>() as u16,
    }),
    idt: UnsafeCell::new([IdtEntry::MISSING; 256]),
    privilege_stack: UnsafeCell::new(ExceptionStack([0; EXCEPTION_STACK_STORAGE_SIZE])),
    double_fault_stack: UnsafeCell::new(IstStack([0; IST_STACK_STORAGE_SIZE])),
    nmi_stack: UnsafeCell::new(IstStack([0; IST_STACK_STORAGE_SIZE])),
    machine_check_stack: UnsafeCell::new(IstStack([0; IST_STACK_STORAGE_SIZE])),
};
