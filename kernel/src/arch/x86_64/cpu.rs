use core::arch::{
    asm, global_asm,
    x86_64::{__cpuid, __cpuid_count},
};
use core::mem::size_of;
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicU8, Ordering};

use super::{paging, timer};

use crate::{
    address_space,
    capability::Rights,
    control_abi, domain, ipc, memory, network, scheduler,
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
const STAR_MSR: u32 = 0xc000_0081;
const LSTAR_MSR: u32 = 0xc000_0082;
const FMASK_MSR: u32 = 0xc000_0084;
const FS_BASE_MSR: u32 = 0xc000_0100;
const EFER_SCE: u64 = 1;
const EFER_NXE: u64 = 1 << 11;
const LINUX_READ_SYSCALL: u64 = 0;
const LINUX_WRITE_SYSCALL: u64 = 1;
const LINUX_CLOSE_SYSCALL: u64 = 3;
const LINUX_FSTAT_SYSCALL: u64 = 5;
const LINUX_POLL_SYSCALL: u64 = 7;
const LINUX_LSEEK_SYSCALL: u64 = 8;
const LINUX_MMAP_SYSCALL: u64 = 9;
const LINUX_MPROTECT_SYSCALL: u64 = 10;
const LINUX_MUNMAP_SYSCALL: u64 = 11;
const LINUX_BRK_SYSCALL: u64 = 12;
const LINUX_RT_SIGACTION_SYSCALL: u64 = 13;
const LINUX_RT_SIGPROCMASK_SYSCALL: u64 = 14;
const LINUX_IOCTL_SYSCALL: u64 = 16;
const LINUX_READV_SYSCALL: u64 = 19;
const LINUX_WRITEV_SYSCALL: u64 = 20;
const LINUX_SCHED_YIELD_SYSCALL: u64 = 24;
const LINUX_MINCORE_SYSCALL: u64 = 27;
const LINUX_MADVISE_SYSCALL: u64 = 28;
const LINUX_DUP_SYSCALL: u64 = 32;
const LINUX_DUP2_SYSCALL: u64 = 33;
const LINUX_GETITIMER_SYSCALL: u64 = 36;
const LINUX_SETITIMER_SYSCALL: u64 = 38;
const LINUX_GETPID_SYSCALL: u64 = 39;
const LINUX_EXIT_SYSCALL: u64 = 60;
const LINUX_UNAME_SYSCALL: u64 = 63;
const LINUX_FCNTL_SYSCALL: u64 = 72;
const LINUX_GETCWD_SYSCALL: u64 = 79;
const LINUX_CHDIR_SYSCALL: u64 = 80;
const LINUX_READLINK_SYSCALL: u64 = 89;
const LINUX_UMASK_SYSCALL: u64 = 95;
const LINUX_GETRLIMIT_SYSCALL: u64 = 97;
const LINUX_GETRUSAGE_SYSCALL: u64 = 98;
const LINUX_TIMES_SYSCALL: u64 = 100;
const LINUX_GETUID_SYSCALL: u64 = 102;
const LINUX_GETGID_SYSCALL: u64 = 104;
const LINUX_GETEUID_SYSCALL: u64 = 107;
const LINUX_GETEGID_SYSCALL: u64 = 108;
const LINUX_GETPPID_SYSCALL: u64 = 110;
const LINUX_GETPGRP_SYSCALL: u64 = 111;
const LINUX_GETGROUPS_SYSCALL: u64 = 115;
const LINUX_SETGROUPS_SYSCALL: u64 = 116;
const LINUX_GETRESUID_SYSCALL: u64 = 118;
const LINUX_GETRESGID_SYSCALL: u64 = 120;
const LINUX_GETPGID_SYSCALL: u64 = 121;
const LINUX_GETSID_SYSCALL: u64 = 124;
const LINUX_CAPGET_SYSCALL: u64 = 125;
const LINUX_CAPSET_SYSCALL: u64 = 126;
const LINUX_SIGALTSTACK_SYSCALL: u64 = 131;
const LINUX_PERSONALITY_SYSCALL: u64 = 135;
const LINUX_GETPRIORITY_SYSCALL: u64 = 140;
const LINUX_SETPRIORITY_SYSCALL: u64 = 141;
const LINUX_SCHED_SETPARAM_SYSCALL: u64 = 142;
const LINUX_SCHED_GETPARAM_SYSCALL: u64 = 143;
const LINUX_SCHED_SETSCHEDULER_SYSCALL: u64 = 144;
const LINUX_SCHED_GETSCHEDULER_SYSCALL: u64 = 145;
const LINUX_SCHED_GET_PRIORITY_MAX_SYSCALL: u64 = 146;
const LINUX_SCHED_GET_PRIORITY_MIN_SYSCALL: u64 = 147;
const LINUX_PRCTL_SYSCALL: u64 = 157;
const LINUX_ARCH_PRCTL_SYSCALL: u64 = 158;
const LINUX_GETTID_SYSCALL: u64 = 186;
const LINUX_FUTEX_SYSCALL: u64 = 202;
const LINUX_SCHED_SETAFFINITY_SYSCALL: u64 = 203;
const LINUX_SCHED_GETAFFINITY_SYSCALL: u64 = 204;
const LINUX_GETDENTS64_SYSCALL: u64 = 217;
const LINUX_SET_TID_ADDRESS_SYSCALL: u64 = 218;
const LINUX_CLOCK_GETTIME_SYSCALL: u64 = 228;
const LINUX_CLOCK_GETRES_SYSCALL: u64 = 229;
const LINUX_EXIT_GROUP_SYSCALL: u64 = 231;
const LINUX_EPOLL_WAIT_SYSCALL: u64 = 232;
const LINUX_EPOLL_CTL_SYSCALL: u64 = 233;
const LINUX_OPENAT_SYSCALL: u64 = 257;
const LINUX_NEWFSTATAT_SYSCALL: u64 = 262;
const LINUX_READLINKAT_SYSCALL: u64 = 267;
const LINUX_FACCESSAT_SYSCALL: u64 = 269;
const LINUX_PPOLL_SYSCALL: u64 = 271;
const LINUX_SET_ROBUST_LIST_SYSCALL: u64 = 273;
const LINUX_GET_ROBUST_LIST_SYSCALL: u64 = 274;
const LINUX_EPOLL_PWAIT_SYSCALL: u64 = 281;
const LINUX_EVENTFD_SYSCALL: u64 = 284;
const LINUX_EVENTFD2_SYSCALL: u64 = 290;
const LINUX_EPOLL_CREATE1_SYSCALL: u64 = 291;
const LINUX_PRLIMIT64_SYSCALL: u64 = 302;
const LINUX_GETCPU_SYSCALL: u64 = 309;
const LINUX_STATX_SYSCALL: u64 = 332;
const LINUX_FACCESSAT2_SYSCALL: u64 = 439;
const LINUX_EPOLL_PWAIT2_SYSCALL: u64 = 441;
const LINUX_ARCH_SET_FS: u64 = 0x1002;
const LINUX_ARCH_GET_FS: u64 = 0x1003;
const LINUX_PROBE_EXIT_STATUS: u64 = 42;
const LINUX_CAPABILITY_VERSION_3: u32 = 0x2008_0522;
const LINUX_CAPABILITY_DATA_SIZE: usize = 24;
const LINUX_RETURN_TO_KERNEL: u64 = (-4096_i64) as u64;
const LINUX_EPERM: u64 = u64::MAX;
const LINUX_EBADF: u64 = u64::MAX - 8;
const LINUX_ENOENT: u64 = u64::MAX - 1;
const LINUX_ESRCH: u64 = u64::MAX - 2;
const LINUX_EFAULT: u64 = u64::MAX - 13;
const LINUX_EAGAIN: u64 = u64::MAX - 10;
const LINUX_EACCES: u64 = u64::MAX - 12;
const LINUX_EEXIST: u64 = u64::MAX - 16;
const LINUX_ENOTDIR: u64 = u64::MAX - 19;
const LINUX_EISDIR: u64 = u64::MAX - 20;
const LINUX_ENOMEM: u64 = u64::MAX - 11;
const LINUX_ESPIPE: u64 = u64::MAX - 28;
const LINUX_EMFILE: u64 = u64::MAX - 23;
const LINUX_EINVAL: u64 = u64::MAX - 21;
const LINUX_ENOTTY: u64 = u64::MAX - 24;
const LINUX_ERANGE: u64 = u64::MAX - 33;
const LINUX_ENAMETOOLONG: u64 = u64::MAX - 35;
const LINUX_ENOSYS: u64 = u64::MAX - 37;
const LINUX_RLIMIT_STACK: u64 = 3;
const LINUX_RLIMIT_NOFILE: u64 = 7;
const LINUX_PROT_READ: u64 = 1;
const LINUX_PROT_WRITE: u64 = 2;
const LINUX_MAP_PRIVATE: u64 = 2;
const LINUX_MAP_ANONYMOUS: u64 = 0x20;
const LINUX_MADV_DONTNEED: u64 = 4;
const LINUX_STAT_SIZE: usize = 144;
const LINUX_STAT_CHARACTER_DEVICE_MODE: u32 = 0o020600;
const LINUX_STAT_DIRECTORY_MODE: u32 = 0o040555;
const LINUX_STATX_BASIC_STATS: u32 = 0x7ff;
const LINUX_STATX_RESERVED: u32 = 0x8000_0000;
const LINUX_WRITE_LIMIT: usize = 256;
const LINUX_IOV_LIMIT: usize = 4;
const LINUX_POLLFD_LIMIT: usize = 4;
const LINUX_AUX_DESCRIPTOR_FIRST: u64 = 2;
const LINUX_AUX_DESCRIPTOR_LIMIT: u64 = 4;
const LINUX_POLLIN: u16 = 1;
const LINUX_POLLOUT: u16 = 4;
const LINUX_POLLHUP: u16 = 0x10;
const LINUX_POLLNVAL: u16 = 0x20;
const LINUX_EPOLL_CTL_ADD: u64 = 1;
const LINUX_EPOLL_CTL_DEL: u64 = 2;
const LINUX_EPOLL_CTL_MOD: u64 = 3;
const LINUX_EPOLL_SUPPORTED_EVENTS: u32 = (LINUX_POLLIN | LINUX_POLLOUT) as u32;
const LINUX_EFD_SEMAPHORE: u64 = 1;
const LINUX_EFD_NONBLOCK: u64 = 0x800;
const LINUX_AT_FDCWD: u32 = (-100_i32) as u32;
const LINUX_TCGETS: u64 = 0x5401;
const LINUX_TIOCGWINSZ: u64 = 0x5413;
const LINUX_DESCRIPTOR_INPUT: u8 = 1;
const LINUX_DESCRIPTOR_OUTPUT: u8 = 2;
const LINUX_DEVICE_ZERO: u8 = 1 << 2;
const LINUX_DEVICE_CONSOLE: u8 = 1 << 3;
const LINUX_DEVICE_DIRECTORY: u8 = 1 << 4;
const LINUX_F_DUPFD: u64 = 0;
const LINUX_F_GETFD: u64 = 1;
const LINUX_F_SETFD: u64 = 2;
const LINUX_F_GETFL: u64 = 3;
const LINUX_F_DUPFD_CLOEXEC: u64 = 1030;
const LINUX_FD_CLOEXEC: u8 = 1;
const LINUX_O_CLOEXEC: u64 = 0x80000;
const LINUX_O_DIRECTORY: u64 = 0x10000;
const LINUX_PR_SET_NAME: u64 = 15;
const LINUX_PR_GET_NAME: u64 = 16;
const LINUX_PR_SET_NO_NEW_PRIVS: u64 = 38;
const LINUX_PR_GET_NO_NEW_PRIVS: u64 = 39;
const LINUX_PROCESS_NAME_SIZE: usize = 16;
const LINUX_INITIAL_PROCESS_NAME: u64 = 0x7469_6e69;
const LINUX_SIG_BLOCK: u64 = 0;
const LINUX_SIG_UNBLOCK: u64 = 1;
const LINUX_SIG_SETMASK: u64 = 2;
const LINUX_SIGNAL_SET_SIZE: u64 = 8;
const LINUX_SIGNAL_ACTION_SIZE: usize = 32;
const LINUX_SIGNAL_COUNT: usize = 64;
const LINUX_ALT_STACK_SIZE: usize = 24;
const LINUX_MIN_ALT_STACK_SIZE: u64 = 2048;
const LINUX_SS_DISABLE: u32 = 2;
const LINUX_UNBLOCKABLE_SIGNALS: u64 = (1 << (9 - 1)) | (1 << (19 - 1));
const LINUX_UTSNAME_SIZE: usize = 6 * 65;
const LINUX_ROBUST_LIST_HEAD_SIZE: usize = 24;
const LINUX_CLEAR_CHILD_TID_OFFSET: u64 = 400;
const LINUX_CLOCK_MONOTONIC: u64 = 1;
const LINUX_CLOCK_PROCESS_CPUTIME_ID: u64 = 2;
const LINUX_CLOCK_THREAD_CPUTIME_ID: u64 = 3;
const LINUX_CLOCK_MONOTONIC_RAW: u64 = 4;
const NANOSECONDS_PER_SECOND: u64 = 1_000_000_000;
const LINUX_CLOCK_TICKS_PER_SECOND: u64 = 100;

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
static ELF_PROBE_ACTIVE: AtomicBool = AtomicBool::new(false);
static ELF_PROBE_COMPLETED: AtomicBool = AtomicBool::new(false);
static ELF_PROBE_STACK: AtomicU64 = AtomicU64::new(0);
static ELF_PROBE_CODE_BASE: AtomicU64 = AtomicU64::new(0);
static ELF_PROBE_CODE_SIZE: AtomicU64 = AtomicU64::new(0);
static ELF_PROBE_DATA: AtomicU64 = AtomicU64::new(0);
static ELF_PROBE_DATA_SIZE: AtomicU64 = AtomicU64::new(0);
static ELF_PROBE_RELRO: AtomicU64 = AtomicU64::new(0);
static ELF_PROBE_RELRO_SIZE: AtomicU64 = AtomicU64::new(0);
static ELF_PROBE_BRK: AtomicU64 = AtomicU64::new(0);
static ELF_PROBE_BRK_LIMIT: AtomicU64 = AtomicU64::new(0);
static ELF_PROBE_MMAP: AtomicU64 = AtomicU64::new(0);
static ELF_PROBE_MMAP_FRAME: AtomicU64 = AtomicU64::new(0);
static ELF_PROBE_MMAP_TAIL_FRAME: AtomicU64 = AtomicU64::new(0);
static ELF_PROBE_MMAP_PAGES: AtomicU8 = AtomicU8::new(0);
static ELF_PROBE_MMAP_WRITABLE: AtomicBool = AtomicBool::new(false);
static ELF_PROBE_CLEAR_CHILD_TID: AtomicU64 = AtomicU64::new(0);
static ELF_PROBE_ROBUST_LIST: AtomicU64 = AtomicU64::new(0);
static ELF_PROBE_READ_ONLY_DATA_PAGE: AtomicU64 = AtomicU64::new(0);
static ELF_PROBE_UMASK: AtomicU64 = AtomicU64::new(0);
static ELF_PROBE_CONSOLE_DESCRIPTORS: AtomicU8 = AtomicU8::new(0);
static ELF_PROBE_CONSOLE_CLOEXEC: AtomicU8 = AtomicU8::new(0);
static ELF_PROBE_DEVICE_DESCRIPTORS: [AtomicU8; 2] = [const { AtomicU8::new(0) }; 2];
static ELF_PROBE_EPOLL_DESCRIPTOR: AtomicU8 = AtomicU8::new(0);
static ELF_PROBE_EPOLL_TARGET: AtomicU64 = AtomicU64::new(u64::MAX);
static ELF_PROBE_EPOLL_EVENTS: AtomicU32 = AtomicU32::new(0);
static ELF_PROBE_EPOLL_DATA: AtomicU64 = AtomicU64::new(0);
static ELF_PROBE_EVENTFD_DESCRIPTOR: AtomicU8 = AtomicU8::new(0);
static ELF_PROBE_EVENTFD_COUNTER: AtomicU64 = AtomicU64::new(0);
static ELF_PROBE_EVENTFD_FLAGS: AtomicU32 = AtomicU32::new(0);
static ELF_PROBE_DIRECTORY_CURSORS: [AtomicU8; 2] = [const { AtomicU8::new(0) }; 2];
static ELF_PROBE_CLOCK_EPOCH: AtomicU64 = AtomicU64::new(0);
static ELF_PROBE_CLOCK_FREQUENCY: AtomicU64 = AtomicU64::new(0);
static ELF_PROBE_PROCESS_NAME_LOW: AtomicU64 = AtomicU64::new(0);
static ELF_PROBE_PROCESS_NAME_HIGH: AtomicU64 = AtomicU64::new(0);
static ELF_PROBE_NO_NEW_PRIVS: AtomicBool = AtomicBool::new(false);
static ELF_PROBE_SIGNAL_MASK: AtomicU64 = AtomicU64::new(0);
static ELF_PROBE_SIGNAL_HANDLERS: [AtomicU64; LINUX_SIGNAL_COUNT] =
    [const { AtomicU64::new(0) }; LINUX_SIGNAL_COUNT];
static ELF_PROBE_SIGNAL_FLAGS: [AtomicU64; LINUX_SIGNAL_COUNT] =
    [const { AtomicU64::new(0) }; LINUX_SIGNAL_COUNT];
static ELF_PROBE_SIGNAL_RESTORERS: [AtomicU64; LINUX_SIGNAL_COUNT] =
    [const { AtomicU64::new(0) }; LINUX_SIGNAL_COUNT];
static ELF_PROBE_SIGNAL_ACTION_MASKS: [AtomicU64; LINUX_SIGNAL_COUNT] =
    [const { AtomicU64::new(0) }; LINUX_SIGNAL_COUNT];
static ELF_PROBE_ALT_STACK_POINTER: AtomicU64 = AtomicU64::new(0);
static ELF_PROBE_ALT_STACK_SIZE: AtomicU64 = AtomicU64::new(0);
static ELF_PROBE_ALT_STACK_FLAGS: AtomicU32 = AtomicU32::new(LINUX_SS_DISABLE);
static SERVICE_STACK_SCRUBBED: AtomicBool = AtomicBool::new(false);
static SERVICE_FRAMES_TRANSFERRED: AtomicBool = AtomicBool::new(false);
static mut USER_RETURN_STACK: u64 = 0;
static mut LINUX_USER_RSP: u64 = 0;
static mut LINUX_USER_RIP: u64 = 0;
static mut LINUX_USER_RFLAGS: u64 = 0;
static mut LINUX_KERNEL_FRAME: u64 = 0;
static mut LINUX_SYSCALL_RESULT: u64 = 0;
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

    .global ginux_linux_syscall_entry
ginux_linux_syscall_entry:
    cli
    clac
    cld
    mov [rip + {linux_user_rsp}], rsp
    mov [rip + {linux_user_rip}], rcx
    mov [rip + {linux_user_rflags}], r11
    mov rsp, [rip + {user_return_stack}]
    push rdi
    push rsi
    push rdx
    push r8
    push r9
    push r10
    mov [rip + {linux_kernel_frame}], rsp
    mov rdi, rax
    mov rsi, rsp
    mov rdx, [rip + {linux_user_rsp}]
    mov rcx, [rip + {linux_user_rip}]
    mov r8, [rip + {linux_user_rflags}]
    and rsp, -16
    call {linux_syscall_handler}
    cmp rax, {linux_return_to_kernel}
    je ginux_user_return_kernel
    mov [rip + {linux_syscall_result}], rax
    mov rsp, [rip + {linux_kernel_frame}]
    pop r10
    pop r9
    pop r8
    pop rdx
    pop rsi
    pop rdi
    mov rax, [rip + {linux_syscall_result}]
    mov rcx, [rip + {linux_user_rip}]
    mov r11, [rip + {linux_user_rflags}]
    mov rsp, [rip + {linux_user_rsp}]
    sysretq
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
    linux_syscall_handler = sym linux_syscall_handler,
    user_return_stack = sym USER_RETURN_STACK,
    linux_user_rsp = sym LINUX_USER_RSP,
    linux_user_rip = sym LINUX_USER_RIP,
    linux_user_rflags = sym LINUX_USER_RFLAGS,
    linux_kernel_frame = sym LINUX_KERNEL_FRAME,
    linux_syscall_result = sym LINUX_SYSCALL_RESULT,
    linux_return_to_kernel = const LINUX_RETURN_TO_KERNEL as i64,
    user_data = const USER_DATA_SELECTOR as u64,
    user_code = const USER_CODE_SELECTOR as u64,
);

extern "C" {
    static ginux_exception_entries: [usize; 32];
    fn ginux_syscall_entry();
    fn ginux_linux_syscall_entry();
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

pub(crate) fn execute_auxiliary(
    entry: usize,
    stack: usize,
    console_handle: u64,
    domain_id: u64,
    service_identity: u64,
) {
    unsafe {
        ginux_user_enter(
            entry,
            stack,
            0,
            console_handle,
            0,
            4,
            domain_id,
            service_identity,
        )
    };
}

pub(crate) struct LinuxElfExecution {
    pub(crate) entry: usize,
    pub(crate) stack: usize,
    pub(crate) code: usize,
    pub(crate) code_size: usize,
    pub(crate) data: usize,
    pub(crate) data_size: usize,
    pub(crate) relro: usize,
    pub(crate) relro_size: usize,
    pub(crate) heap: usize,
    pub(crate) heap_size: usize,
    pub(crate) mmap: usize,
    pub(crate) mmap_frames: [u64; 2],
}

pub(crate) fn verify_linux_elf_entry(execution: LinuxElfExecution) -> bool {
    let Some(heap_limit) = execution.heap.checked_add(execution.heap_size) else {
        return false;
    };
    if ELF_PROBE_ACTIVE.swap(true, Ordering::SeqCst) {
        return false;
    }
    let Some(clock_frequency) = timer::monotonic_frequency()
        .filter(|frequency| *frequency <= u64::MAX / NANOSECONDS_PER_SECOND)
    else {
        ELF_PROBE_ACTIVE.store(false, Ordering::SeqCst);
        return false;
    };
    let Some(saved_msrs) = configure_linux_syscall() else {
        ELF_PROBE_ACTIVE.store(false, Ordering::SeqCst);
        return false;
    };
    ELF_PROBE_COMPLETED.store(false, Ordering::SeqCst);
    ELF_PROBE_STACK.store(execution.stack as u64, Ordering::SeqCst);
    ELF_PROBE_CODE_BASE.store(execution.code as u64, Ordering::SeqCst);
    ELF_PROBE_CODE_SIZE.store(execution.code_size as u64, Ordering::SeqCst);
    ELF_PROBE_DATA.store(execution.data as u64, Ordering::SeqCst);
    ELF_PROBE_DATA_SIZE.store(execution.data_size as u64, Ordering::SeqCst);
    ELF_PROBE_RELRO.store(execution.relro as u64, Ordering::SeqCst);
    ELF_PROBE_RELRO_SIZE.store(execution.relro_size as u64, Ordering::SeqCst);
    ELF_PROBE_BRK.store(execution.heap as u64, Ordering::SeqCst);
    ELF_PROBE_BRK_LIMIT.store(heap_limit as u64, Ordering::SeqCst);
    ELF_PROBE_MMAP.store(execution.mmap as u64, Ordering::SeqCst);
    ELF_PROBE_MMAP_FRAME.store(execution.mmap_frames[0], Ordering::SeqCst);
    ELF_PROBE_MMAP_TAIL_FRAME.store(execution.mmap_frames[1], Ordering::SeqCst);
    ELF_PROBE_MMAP_PAGES.store(0, Ordering::SeqCst);
    ELF_PROBE_MMAP_WRITABLE.store(false, Ordering::SeqCst);
    ELF_PROBE_CLEAR_CHILD_TID.store(0, Ordering::SeqCst);
    ELF_PROBE_ROBUST_LIST.store(0, Ordering::SeqCst);
    ELF_PROBE_READ_ONLY_DATA_PAGE.store(0, Ordering::SeqCst);
    ELF_PROBE_UMASK.store(0o022, Ordering::SeqCst);
    ELF_PROBE_CONSOLE_DESCRIPTORS.store(
        LINUX_DESCRIPTOR_INPUT | (LINUX_DESCRIPTOR_OUTPUT << 2),
        Ordering::SeqCst,
    );
    ELF_PROBE_CONSOLE_CLOEXEC.store(0, Ordering::SeqCst);
    for descriptor in &ELF_PROBE_DEVICE_DESCRIPTORS {
        descriptor.store(0, Ordering::SeqCst);
    }
    ELF_PROBE_EPOLL_DESCRIPTOR.store(0, Ordering::SeqCst);
    ELF_PROBE_EPOLL_TARGET.store(u64::MAX, Ordering::SeqCst);
    ELF_PROBE_EPOLL_EVENTS.store(0, Ordering::SeqCst);
    ELF_PROBE_EPOLL_DATA.store(0, Ordering::SeqCst);
    ELF_PROBE_EVENTFD_DESCRIPTOR.store(0, Ordering::SeqCst);
    ELF_PROBE_EVENTFD_COUNTER.store(0, Ordering::SeqCst);
    ELF_PROBE_EVENTFD_FLAGS.store(0, Ordering::SeqCst);
    for cursor in &ELF_PROBE_DIRECTORY_CURSORS {
        cursor.store(0, Ordering::SeqCst);
    }
    ELF_PROBE_CLOCK_FREQUENCY.store(clock_frequency, Ordering::SeqCst);
    ELF_PROBE_CLOCK_EPOCH.store(timer::timestamp_counter(), Ordering::SeqCst);
    ELF_PROBE_PROCESS_NAME_LOW.store(LINUX_INITIAL_PROCESS_NAME, Ordering::SeqCst);
    ELF_PROBE_PROCESS_NAME_HIGH.store(0, Ordering::SeqCst);
    ELF_PROBE_NO_NEW_PRIVS.store(false, Ordering::SeqCst);
    ELF_PROBE_SIGNAL_MASK.store(0, Ordering::SeqCst);
    for signal in 0..LINUX_SIGNAL_COUNT {
        ELF_PROBE_SIGNAL_HANDLERS[signal].store(0, Ordering::SeqCst);
        ELF_PROBE_SIGNAL_FLAGS[signal].store(0, Ordering::SeqCst);
        ELF_PROBE_SIGNAL_RESTORERS[signal].store(0, Ordering::SeqCst);
        ELF_PROBE_SIGNAL_ACTION_MASKS[signal].store(0, Ordering::SeqCst);
    }
    ELF_PROBE_ALT_STACK_POINTER.store(0, Ordering::SeqCst);
    ELF_PROBE_ALT_STACK_SIZE.store(0, Ordering::SeqCst);
    ELF_PROBE_ALT_STACK_FLAGS.store(LINUX_SS_DISABLE, Ordering::SeqCst);
    unsafe { ginux_user_enter(execution.entry, execution.stack, 0, 0, 0, 0, 0, 0) };
    let msrs_restored = restore_linux_syscall(saved_msrs);
    let inactive = !ELF_PROBE_ACTIVE.swap(false, Ordering::SeqCst);
    let completed = ELF_PROBE_COMPLETED.load(Ordering::SeqCst);
    let umask_restored = ELF_PROBE_UMASK.swap(0, Ordering::SeqCst) == 0o022;
    let descriptor_snapshot = [
        u64::from(ELF_PROBE_CONSOLE_DESCRIPTORS.load(Ordering::SeqCst)),
        u64::from(ELF_PROBE_CONSOLE_CLOEXEC.load(Ordering::SeqCst)),
        u64::from(ELF_PROBE_DEVICE_DESCRIPTORS[0].load(Ordering::SeqCst)),
        u64::from(ELF_PROBE_DEVICE_DESCRIPTORS[1].load(Ordering::SeqCst)),
        u64::from(ELF_PROBE_EPOLL_DESCRIPTOR.load(Ordering::SeqCst)),
        ELF_PROBE_EPOLL_TARGET.load(Ordering::SeqCst),
        u64::from(ELF_PROBE_EPOLL_EVENTS.load(Ordering::SeqCst)),
        ELF_PROBE_EPOLL_DATA.load(Ordering::SeqCst),
        u64::from(ELF_PROBE_DIRECTORY_CURSORS[0].load(Ordering::SeqCst)),
        u64::from(ELF_PROBE_DIRECTORY_CURSORS[1].load(Ordering::SeqCst)),
        u64::from(ELF_PROBE_EVENTFD_DESCRIPTOR.load(Ordering::SeqCst)),
        ELF_PROBE_EVENTFD_COUNTER.load(Ordering::SeqCst),
        u64::from(ELF_PROBE_EVENTFD_FLAGS.load(Ordering::SeqCst)),
    ];
    let mut devices_closed = true;
    for descriptor in &ELF_PROBE_DEVICE_DESCRIPTORS {
        devices_closed &= descriptor.swap(0, Ordering::SeqCst) == 0;
    }
    let mut cursors_reset = true;
    for cursor in &ELF_PROBE_DIRECTORY_CURSORS {
        cursors_reset &= cursor.swap(0, Ordering::SeqCst) == 0;
    }
    let descriptor_state_valid = ELF_PROBE_CONSOLE_DESCRIPTORS.swap(0, Ordering::SeqCst) == 0
        && ELF_PROBE_CONSOLE_CLOEXEC.swap(0, Ordering::SeqCst) == 0
        && devices_closed
        && ELF_PROBE_EPOLL_DESCRIPTOR.swap(0, Ordering::SeqCst) == 0
        && ELF_PROBE_EPOLL_TARGET.swap(u64::MAX, Ordering::SeqCst) == u64::MAX
        && ELF_PROBE_EPOLL_EVENTS.swap(0, Ordering::SeqCst) == 0
        && ELF_PROBE_EPOLL_DATA.swap(0, Ordering::SeqCst) == 0
        && ELF_PROBE_EVENTFD_DESCRIPTOR.swap(0, Ordering::SeqCst) == 0
        && ELF_PROBE_EVENTFD_COUNTER.swap(0, Ordering::SeqCst) == 0
        && ELF_PROBE_EVENTFD_FLAGS.swap(0, Ordering::SeqCst) == 0
        && cursors_reset;
    if !descriptor_state_valid {
        serial_write("ginux: Linux descriptor state leak:");
        for value in descriptor_snapshot {
            serial_write(" ");
            serial_write_u64(value);
        }
        serial_write("\r\n");
    }
    let clock_state_valid = ELF_PROBE_CLOCK_EPOCH.swap(0, Ordering::SeqCst) != 0
        && ELF_PROBE_CLOCK_FREQUENCY.swap(0, Ordering::SeqCst) == clock_frequency;
    let mmap_inactive = ELF_PROBE_MMAP_PAGES.swap(0, Ordering::SeqCst) == 0;
    let mmap_not_writable = !ELF_PROBE_MMAP_WRITABLE.swap(false, Ordering::SeqCst);
    let mmap_address_valid = ELF_PROBE_MMAP.swap(0, Ordering::SeqCst) == execution.mmap as u64;
    let mmap_frames_valid = ELF_PROBE_MMAP_FRAME.swap(0, Ordering::SeqCst)
        == execution.mmap_frames[0]
        && ELF_PROBE_MMAP_TAIL_FRAME.swap(0, Ordering::SeqCst) == execution.mmap_frames[1];
    let mmap_state_valid =
        mmap_inactive && mmap_not_writable && mmap_address_valid && mmap_frames_valid;
    let process_name_restored = ELF_PROBE_PROCESS_NAME_LOW.swap(0, Ordering::SeqCst)
        == LINUX_INITIAL_PROCESS_NAME
        && ELF_PROBE_PROCESS_NAME_HIGH.swap(0, Ordering::SeqCst) == 0;
    let no_new_privileges_enforced = ELF_PROBE_NO_NEW_PRIVS.swap(false, Ordering::SeqCst);
    let signal_mask_restored = ELF_PROBE_SIGNAL_MASK.swap(0, Ordering::SeqCst) == 0;
    let signal_actions_restored = (0..LINUX_SIGNAL_COUNT).all(|signal| {
        ELF_PROBE_SIGNAL_HANDLERS[signal].swap(0, Ordering::SeqCst) == 0
            && ELF_PROBE_SIGNAL_FLAGS[signal].swap(0, Ordering::SeqCst) == 0
            && ELF_PROBE_SIGNAL_RESTORERS[signal].swap(0, Ordering::SeqCst) == 0
            && ELF_PROBE_SIGNAL_ACTION_MASKS[signal].swap(0, Ordering::SeqCst) == 0
    });
    let alt_stack_disabled = ELF_PROBE_ALT_STACK_POINTER.swap(0, Ordering::SeqCst) == 0
        && ELF_PROBE_ALT_STACK_SIZE.swap(0, Ordering::SeqCst) == 0
        && ELF_PROBE_ALT_STACK_FLAGS.swap(LINUX_SS_DISABLE, Ordering::SeqCst) == LINUX_SS_DISABLE;
    let robust_list_cleared = ELF_PROBE_ROBUST_LIST.swap(0, Ordering::SeqCst) == 0;
    let protected_page = ELF_PROBE_READ_ONLY_DATA_PAGE.swap(0, Ordering::SeqCst);
    let clear_child_tid = ELF_PROBE_CLEAR_CHILD_TID.swap(0, Ordering::SeqCst);
    let mut cleared_tid = [0xff_u8; size_of::<u32>()];
    let child_tid_cleared = clear_child_tid == execution.data as u64 + LINUX_CLEAR_CHILD_TID_OFFSET
        && copy_from_linux_user(clear_child_tid, &mut cleared_tid)
        && cleared_tid == [0; size_of::<u32>()];
    // SAFETY: the loader owns and retains the mapped heap page through this
    // verification, and SMAP access is enabled only around the bounded read.
    let heap_written = unsafe {
        with_supervisor_user_access(|| {
            (execution.heap as *const u64).read_volatile() == 0x4845_4150_4252_4b21
        })
    };
    ELF_PROBE_STACK.store(0, Ordering::SeqCst);
    ELF_PROBE_CODE_BASE.store(0, Ordering::SeqCst);
    ELF_PROBE_CODE_SIZE.store(0, Ordering::SeqCst);
    ELF_PROBE_DATA.store(0, Ordering::SeqCst);
    ELF_PROBE_DATA_SIZE.store(0, Ordering::SeqCst);
    ELF_PROBE_RELRO.store(0, Ordering::SeqCst);
    ELF_PROBE_RELRO_SIZE.store(0, Ordering::SeqCst);
    ELF_PROBE_BRK.store(0, Ordering::SeqCst);
    ELF_PROBE_BRK_LIMIT.store(0, Ordering::SeqCst);
    inactive
        && completed
        && child_tid_cleared
        && robust_list_cleared
        && protected_page == execution.relro as u64
        && heap_written
        && umask_restored
        && descriptor_state_valid
        && clock_state_valid
        && mmap_state_valid
        && process_name_restored
        && no_new_privileges_enforced
        && signal_mask_restored
        && signal_actions_restored
        && alt_stack_disabled
        && msrs_restored
}

fn configure_linux_syscall() -> Option<[u64; 5]> {
    let star = (0x20_u64 << 48) | (u64::from(KERNEL_CODE_SELECTOR) << 32);
    let lstar = ginux_linux_syscall_entry as *const () as u64;
    let fmask = (1_u64 << 9) | (1_u64 << 10) | (1_u64 << 18);
    unsafe {
        let saved = [
            read_msr(EFER_MSR),
            read_msr(STAR_MSR),
            read_msr(LSTAR_MSR),
            read_msr(FMASK_MSR),
            read_msr(FS_BASE_MSR),
        ];
        write_msr(STAR_MSR, star);
        write_msr(LSTAR_MSR, lstar);
        write_msr(FMASK_MSR, fmask);
        write_msr(EFER_MSR, read_msr(EFER_MSR) | EFER_SCE);
        let valid = read_msr(STAR_MSR) == star
            && read_msr(LSTAR_MSR) == lstar
            && read_msr(FMASK_MSR) == fmask
            && read_msr(EFER_MSR) & EFER_SCE != 0;
        if valid {
            Some(saved)
        } else {
            let _ = restore_linux_syscall(saved);
            None
        }
    }
}

fn restore_linux_syscall(saved: [u64; 5]) -> bool {
    unsafe {
        write_msr(FS_BASE_MSR, saved[4]);
        write_msr(FMASK_MSR, saved[3]);
        write_msr(LSTAR_MSR, saved[2]);
        write_msr(STAR_MSR, saved[1]);
        write_msr(EFER_MSR, saved[0]);
        read_msr(EFER_MSR) == saved[0]
            && read_msr(STAR_MSR) == saved[1]
            && read_msr(LSTAR_MSR) == saved[2]
            && read_msr(FMASK_MSR) == saved[3]
            && read_msr(FS_BASE_MSR) == saved[4]
    }
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

extern "sysv64" fn linux_syscall_handler(
    number: u64,
    registers: *const u64,
    user_stack: u64,
    user_rip: u64,
    user_rflags: u64,
) -> u64 {
    if registers.is_null() {
        ELF_PROBE_ACTIVE.store(false, Ordering::SeqCst);
        return LINUX_RETURN_TO_KERNEL;
    }
    // SAFETY: the assembly entry stub passes the base of six saved registers.
    let registers = unsafe { core::slice::from_raw_parts(registers, 6) };
    let register = |index: usize| registers[index];
    let count = register(3);
    let buffer = register(4);
    let first_argument = register(5);
    let code_start = ELF_PROBE_CODE_BASE.load(Ordering::SeqCst);
    let code_end = code_start.saturating_add(ELF_PROBE_CODE_SIZE.load(Ordering::SeqCst));
    if !ELF_PROBE_ACTIVE.load(Ordering::SeqCst)
        || user_stack != ELF_PROBE_STACK.load(Ordering::SeqCst)
        || !(code_start..code_end).contains(&user_rip)
        || user_rflags & ((3 << 12) | (1 << 17)) != 0
    {
        ELF_PROBE_ACTIVE.store(false, Ordering::SeqCst);
        return LINUX_RETURN_TO_KERNEL;
    }
    match number {
        LINUX_READ_SYSCALL => linux_descriptor_read(first_argument, buffer, count),
        LINUX_WRITE_SYSCALL => linux_descriptor_write(first_argument, buffer, count),
        LINUX_SCHED_YIELD_SYSCALL => 0,
        LINUX_CLOSE_SYSCALL => linux_descriptor_close(first_argument),
        LINUX_POLL_SYSCALL => linux_poll(first_argument, buffer, count),
        LINUX_FSTAT_SYSCALL => {
            let Some(stat) = linux_descriptor_stat(first_argument) else {
                return LINUX_EBADF;
            };
            if !copy_to_linux_user(buffer, &stat) {
                return LINUX_EFAULT;
            }
            0
        }
        LINUX_LSEEK_SYSCALL => {
            if linux_eventfd_descriptor(first_argument) {
                return LINUX_ESPIPE;
            }
            let device = linux_device_descriptor(first_argument);
            if device & LINUX_DEVICE_CONSOLE != 0 {
                LINUX_ESPIPE
            } else if device & LINUX_DEVICE_DIRECTORY != 0 {
                if buffer == 0 && count == 0 {
                    linux_directory_cursor(first_argument).store(0, Ordering::SeqCst);
                    0
                } else {
                    LINUX_EINVAL
                }
            } else if device != 0 {
                0
            } else if linux_console_descriptor_open(first_argument) {
                LINUX_ESPIPE
            } else {
                LINUX_EBADF
            }
        }
        LINUX_MMAP_SYSCALL => linux_mmap(
            first_argument,
            buffer,
            count,
            register(0),
            register(2),
            register(1),
        ),
        LINUX_MUNMAP_SYSCALL => linux_munmap(first_argument, buffer),
        LINUX_MPROTECT_SYSCALL => {
            if !first_argument.is_multiple_of(PAGE_SIZE as u64) || count != LINUX_PROT_READ {
                return LINUX_EINVAL;
            }
            let relro = ELF_PROBE_RELRO.load(Ordering::SeqCst);
            let relro_size = ELF_PROBE_RELRO_SIZE.load(Ordering::SeqCst);
            if first_argument == relro
                && buffer == PAGE_SIZE as u64
                && relro_size != 0
                && relro_size <= PAGE_SIZE as u64
                && ELF_PROBE_READ_ONLY_DATA_PAGE.load(Ordering::SeqCst) == 0
                && address_space::protect_current_user_page_read_only(first_argument)
            {
                ELF_PROBE_READ_ONLY_DATA_PAGE.store(first_argument, Ordering::SeqCst);
                return 0;
            }
            let mapping = ELF_PROBE_MMAP.load(Ordering::SeqCst);
            let mapping_pages = ELF_PROBE_MMAP_PAGES.load(Ordering::SeqCst);
            if first_argument == mapping
                && mapping_pages != 0
                && buffer == u64::from(mapping_pages) * PAGE_SIZE as u64
                && ELF_PROBE_MMAP_WRITABLE.load(Ordering::SeqCst)
                && address_space::protect_current_linux_pages_read_only(
                    first_argument,
                    &[
                        ELF_PROBE_MMAP_FRAME.load(Ordering::SeqCst),
                        ELF_PROBE_MMAP_TAIL_FRAME.load(Ordering::SeqCst),
                    ][..usize::from(mapping_pages)],
                )
            {
                ELF_PROBE_MMAP_WRITABLE.store(false, Ordering::SeqCst);
                return 0;
            }
            LINUX_EINVAL
        }
        LINUX_IOCTL_SYSCALL => {
            let device = linux_device_descriptor(first_argument);
            if device != 0 && device & LINUX_DEVICE_CONSOLE == 0 {
                return LINUX_ENOTTY;
            }
            if device == 0 && !linux_console_descriptor_open(first_argument) {
                return LINUX_EBADF;
            }
            match buffer {
                LINUX_TCGETS => {
                    let termios = [0_u8; 36];
                    if !copy_to_linux_user(count, &termios) {
                        return LINUX_EFAULT;
                    }
                }
                LINUX_TIOCGWINSZ => {
                    let mut window = [0_u8; 8];
                    window[..2].copy_from_slice(&24_u16.to_ne_bytes());
                    window[2..4].copy_from_slice(&80_u16.to_ne_bytes());
                    if !copy_to_linux_user(count, &window) {
                        return LINUX_EFAULT;
                    }
                }
                _ => return LINUX_ENOTTY,
            }
            0
        }
        LINUX_READV_SYSCALL => linux_descriptor_readv(first_argument, buffer, count),
        LINUX_WRITEV_SYSCALL => linux_descriptor_writev(first_argument, buffer, count),
        LINUX_MINCORE_SYSCALL => linux_mincore(first_argument, buffer, count),
        LINUX_MADVISE_SYSCALL => linux_madvise(first_argument, buffer, count),
        LINUX_DUP_SYSCALL => linux_console_dup(first_argument),
        LINUX_DUP2_SYSCALL => linux_console_dup2(first_argument, buffer),
        LINUX_GETITIMER_SYSCALL => linux_getitimer(first_argument, buffer),
        LINUX_SETITIMER_SYSCALL => linux_setitimer(first_argument, buffer, count),
        LINUX_EPOLL_WAIT_SYSCALL => linux_epoll_wait(first_argument, buffer, count, register(0)),
        LINUX_EPOLL_CTL_SYSCALL => linux_epoll_ctl(first_argument, buffer, count, register(0)),
        LINUX_FCNTL_SYSCALL => linux_descriptor_fcntl(first_argument, buffer, count),
        LINUX_BRK_SYSCALL => {
            let current = ELF_PROBE_BRK.load(Ordering::SeqCst);
            let limit = ELF_PROBE_BRK_LIMIT.load(Ordering::SeqCst);
            if first_argument == 0 {
                current
            } else if (limit - PAGE_SIZE as u64..=limit).contains(&first_argument) {
                ELF_PROBE_BRK.store(first_argument, Ordering::SeqCst);
                first_argument
            } else {
                current
            }
        }
        LINUX_RT_SIGACTION_SYSCALL => {
            linux_rt_sigaction(first_argument, buffer, count, register(0))
        }
        LINUX_RT_SIGPROCMASK_SYSCALL => {
            linux_rt_sigprocmask(first_argument, buffer, count, register(0))
        }
        LINUX_GETPID_SYSCALL | LINUX_GETTID_SYSCALL => 1,
        LINUX_GETPPID_SYSCALL => 0,
        LINUX_GETPGRP_SYSCALL => 1,
        LINUX_GETPGID_SYSCALL | LINUX_GETSID_SYSCALL => {
            if matches!(first_argument, 0 | 1) {
                1
            } else {
                LINUX_ESRCH
            }
        }
        LINUX_SIGALTSTACK_SYSCALL => linux_sigaltstack(first_argument, buffer),
        LINUX_SET_TID_ADDRESS_SYSCALL => {
            if !linux_writable_user_range_contains(first_argument, size_of::<u32>()) {
                return LINUX_EFAULT;
            }
            ELF_PROBE_CLEAR_CHILD_TID.store(first_argument, Ordering::SeqCst);
            1
        }
        LINUX_SET_ROBUST_LIST_SYSCALL => {
            if buffer != LINUX_ROBUST_LIST_HEAD_SIZE as u64 {
                return LINUX_EINVAL;
            }
            if !linux_writable_user_range_contains(first_argument, LINUX_ROBUST_LIST_HEAD_SIZE) {
                return LINUX_EFAULT;
            }
            ELF_PROBE_ROBUST_LIST.store(first_argument, Ordering::SeqCst);
            0
        }
        LINUX_GET_ROBUST_LIST_SYSCALL => {
            if !matches!(first_argument, 0 | 1) {
                return LINUX_ESRCH;
            }
            if !copy_linux_robust_list(buffer, count) {
                return LINUX_EFAULT;
            }
            0
        }
        LINUX_GETUID_SYSCALL
        | LINUX_GETGID_SYSCALL
        | LINUX_GETEUID_SYSCALL
        | LINUX_GETEGID_SYSCALL => 0,
        LINUX_GETGROUPS_SYSCALL => {
            if (first_argument as i32) < 0 {
                return LINUX_EINVAL;
            }
            if first_argument == 0 {
                return 1;
            }
            if !copy_to_linux_user(buffer, &0_u32.to_ne_bytes()) {
                return LINUX_EFAULT;
            }
            1
        }
        LINUX_SETGROUPS_SYSCALL => LINUX_EPERM,
        LINUX_GETRESUID_SYSCALL | LINUX_GETRESGID_SYSCALL => {
            if !copy_linux_root_id_triple(first_argument, buffer, count) {
                return LINUX_EFAULT;
            }
            0
        }
        LINUX_PERSONALITY_SYSCALL => match first_argument {
            0 | 0xffff_ffff => 0,
            _ => LINUX_EINVAL,
        },
        LINUX_CAPGET_SYSCALL => linux_capget(first_argument, buffer),
        LINUX_CAPSET_SYSCALL => linux_capset(first_argument, buffer),
        LINUX_GETPRIORITY_SYSCALL => {
            if first_argument != 0 {
                return LINUX_EINVAL;
            }
            if !matches!(buffer, 0 | 1) {
                return LINUX_ESRCH;
            }
            20
        }
        LINUX_SETPRIORITY_SYSCALL => {
            if first_argument != 0 {
                return LINUX_EINVAL;
            }
            if !matches!(buffer, 0 | 1) {
                return LINUX_ESRCH;
            }
            if count == 0 {
                0
            } else {
                LINUX_EPERM
            }
        }
        LINUX_SCHED_SETPARAM_SYSCALL => linux_set_scheduler(first_argument, 0, buffer),
        LINUX_SCHED_SETSCHEDULER_SYSCALL => linux_set_scheduler(first_argument, buffer, count),
        LINUX_SCHED_GETPARAM_SYSCALL => {
            if !matches!(first_argument, 0 | 1) {
                return LINUX_ESRCH;
            }
            if !copy_to_linux_user(buffer, &0_u32.to_ne_bytes()) {
                return LINUX_EFAULT;
            }
            0
        }
        LINUX_SCHED_GETSCHEDULER_SYSCALL => {
            if matches!(first_argument, 0 | 1) {
                0
            } else {
                LINUX_ESRCH
            }
        }
        LINUX_SCHED_GET_PRIORITY_MAX_SYSCALL | LINUX_SCHED_GET_PRIORITY_MIN_SYSCALL => {
            if first_argument == 0 {
                0
            } else {
                LINUX_EINVAL
            }
        }
        LINUX_PRCTL_SYSCALL => match first_argument {
            LINUX_PR_SET_NAME => linux_set_process_name(buffer),
            LINUX_PR_GET_NAME => linux_get_process_name(buffer),
            LINUX_PR_SET_NO_NEW_PRIVS
                if buffer == 1 && count == 0 && register(0) == 0 && register(2) == 0 =>
            {
                ELF_PROBE_NO_NEW_PRIVS.store(true, Ordering::SeqCst);
                0
            }
            LINUX_PR_SET_NO_NEW_PRIVS => LINUX_EINVAL,
            LINUX_PR_GET_NO_NEW_PRIVS
                if buffer == 0 && count == 0 && register(0) == 0 && register(2) == 0 =>
            {
                u64::from(ELF_PROBE_NO_NEW_PRIVS.load(Ordering::SeqCst))
            }
            LINUX_PR_GET_NO_NEW_PRIVS => LINUX_EINVAL,
            _ => LINUX_EINVAL,
        },
        LINUX_ARCH_PRCTL_SYSCALL
            if first_argument == LINUX_ARCH_SET_FS
                && buffer == ELF_PROBE_DATA.load(Ordering::SeqCst) =>
        {
            // SAFETY: the bounded proof accepts only its declared canonical
            // user-data base and restores the supervisor FS base afterward.
            unsafe { write_msr(FS_BASE_MSR, buffer) };
            0
        }
        LINUX_ARCH_PRCTL_SYSCALL if first_argument == LINUX_ARCH_GET_FS => {
            // SAFETY: FS base is the active thread-local value programmed by
            // this bounded proof and restored before returning to the kernel.
            let fs_base = unsafe { read_msr(FS_BASE_MSR) }.to_ne_bytes();
            if !copy_to_linux_user(buffer, &fs_base) {
                return LINUX_EFAULT;
            }
            0
        }
        LINUX_ARCH_PRCTL_SYSCALL => LINUX_EINVAL,
        LINUX_UNAME_SYSCALL => {
            let mut name = [0_u8; LINUX_UTSNAME_SIZE];
            name[..6].copy_from_slice(b"Ginux\0");
            name[65..71].copy_from_slice(b"ginux\0");
            name[130..136].copy_from_slice(b"0.1.0\0");
            name[195..198].copy_from_slice(b"#1\0");
            name[260..267].copy_from_slice(b"x86_64\0");
            name[325..337].copy_from_slice(b"localdomain\0");
            if !copy_to_linux_user(first_argument, &name) {
                return LINUX_EFAULT;
            }
            0
        }
        LINUX_GETCWD_SYSCALL => {
            if buffer < 2 {
                return LINUX_ERANGE;
            }
            if !copy_to_linux_user(first_argument, b"/\0") {
                return LINUX_EFAULT;
            }
            2
        }
        LINUX_CHDIR_SYSCALL => {
            let mut path = [0_u8; 16];
            match copy_linux_c_string(first_argument, &mut path) {
                Ok(1) if path[0] == b'/' => 0,
                Ok(_) => LINUX_ENOENT,
                Err(error) => error,
            }
        }
        LINUX_READLINK_SYSCALL => linux_readlink_self(first_argument, buffer, count),
        LINUX_OPENAT_SYSCALL => linux_openat(first_argument, buffer, count),
        LINUX_NEWFSTATAT_SYSCALL => linux_newfstatat(first_argument, buffer, count, register(0)),
        LINUX_READLINKAT_SYSCALL => {
            if first_argument as u32 != LINUX_AT_FDCWD {
                return LINUX_EBADF;
            }
            linux_readlink_self(buffer, count, register(0))
        }
        LINUX_EPOLL_CREATE1_SYSCALL => linux_epoll_create1(first_argument),
        LINUX_EVENTFD_SYSCALL => linux_eventfd(first_argument, 0),
        LINUX_EVENTFD2_SYSCALL => linux_eventfd(first_argument, buffer),
        LINUX_EPOLL_PWAIT_SYSCALL => linux_epoll_pwait(
            first_argument,
            buffer,
            count,
            register(0),
            register(2),
            register(1),
        ),
        LINUX_EPOLL_PWAIT2_SYSCALL => linux_epoll_pwait2(
            first_argument,
            buffer,
            count,
            register(0),
            register(2),
            register(1),
        ),
        LINUX_PPOLL_SYSCALL => linux_ppoll(first_argument, buffer, count, register(0), register(2)),
        LINUX_FACCESSAT_SYSCALL => linux_faccessat(buffer, count, 0),
        LINUX_FACCESSAT2_SYSCALL => linux_faccessat(buffer, count, register(0)),
        LINUX_UMASK_SYSCALL => ELF_PROBE_UMASK.swap(first_argument & 0o777, Ordering::SeqCst),
        LINUX_GETRLIMIT_SYSCALL => {
            let Some(limit) = linux_resource_limit(first_argument) else {
                return LINUX_EINVAL;
            };
            if !copy_to_linux_user(buffer, &limit) {
                return LINUX_EFAULT;
            }
            0
        }
        LINUX_GETRUSAGE_SYSCALL => linux_getrusage(first_argument, buffer),
        LINUX_TIMES_SYSCALL => linux_times(first_argument),
        LINUX_FUTEX_SYSCALL => linux_futex(first_argument, buffer, count),
        LINUX_PRLIMIT64_SYSCALL => {
            if !matches!(first_argument, 0 | 1) {
                return LINUX_ESRCH;
            }
            let Some(limit) = linux_resource_limit(buffer) else {
                return LINUX_EINVAL;
            };
            if count != 0 {
                return LINUX_EPERM;
            }
            let old_limit = register(0);
            if old_limit != 0 && !copy_to_linux_user(old_limit, &limit) {
                return LINUX_EFAULT;
            }
            0
        }
        LINUX_SCHED_SETAFFINITY_SYSCALL => {
            if !matches!(first_argument, 0 | 1) {
                return LINUX_ESRCH;
            }
            if buffer < size_of::<u64>() as u64 {
                return LINUX_EINVAL;
            }
            let mut mask = [0_u8; size_of::<u64>()];
            if !copy_from_linux_user(count, &mut mask) {
                return LINUX_EFAULT;
            }
            if u64::from_ne_bytes(mask) == 1 {
                0
            } else {
                LINUX_EINVAL
            }
        }
        LINUX_SCHED_GETAFFINITY_SYSCALL => {
            if !matches!(first_argument, 0 | 1) {
                return LINUX_ESRCH;
            }
            if buffer < size_of::<u64>() as u64 {
                return LINUX_EINVAL;
            }
            if !copy_to_linux_user(count, &1_u64.to_ne_bytes()) {
                return LINUX_EFAULT;
            }
            size_of::<u64>() as u64
        }
        LINUX_GETCPU_SYSCALL => {
            if !copy_linux_cpu_location(first_argument, buffer) {
                return LINUX_EFAULT;
            }
            0
        }
        LINUX_GETDENTS64_SYSCALL => linux_getdents64(first_argument, buffer, count),
        LINUX_CLOCK_GETTIME_SYSCALL => linux_clock_gettime(first_argument, buffer),
        LINUX_CLOCK_GETRES_SYSCALL => linux_clock_getres(first_argument, buffer),
        LINUX_STATX_SYSCALL => linux_statx(buffer, count, register(0), register(2)),
        LINUX_EXIT_SYSCALL | LINUX_EXIT_GROUP_SYSCALL
            if first_argument == LINUX_PROBE_EXIT_STATUS =>
        {
            let clear_child_tid = ELF_PROBE_CLEAR_CHILD_TID.load(Ordering::SeqCst);
            if clear_child_tid == 0 || !copy_to_linux_user(clear_child_tid, &0_u32.to_ne_bytes()) {
                return LINUX_EFAULT;
            }
            ELF_PROBE_ROBUST_LIST.store(0, Ordering::SeqCst);
            ELF_PROBE_COMPLETED.store(true, Ordering::SeqCst);
            ELF_PROBE_ACTIVE.store(false, Ordering::SeqCst);
            LINUX_RETURN_TO_KERNEL
        }
        _ => LINUX_ENOSYS,
    }
}

fn linux_clock_gettime(clock: u64, output_address: u64) -> u64 {
    if !linux_clock_supported(clock) {
        return LINUX_EINVAL;
    }
    let Some((seconds, nanoseconds)) = linux_elapsed_time() else {
        return LINUX_ENOSYS;
    };
    let mut timespec = [0_u8; 16];
    timespec[..8].copy_from_slice(&seconds.to_ne_bytes());
    timespec[8..].copy_from_slice(&nanoseconds.to_ne_bytes());
    if !copy_to_linux_user(output_address, &timespec) {
        return LINUX_EFAULT;
    }
    0
}

fn linux_elapsed_time() -> Option<(u64, u64)> {
    let frequency = ELF_PROBE_CLOCK_FREQUENCY.load(Ordering::SeqCst);
    let epoch = ELF_PROBE_CLOCK_EPOCH.load(Ordering::SeqCst);
    if frequency == 0 || epoch == 0 {
        return None;
    }
    let ticks = timer::timestamp_counter().saturating_sub(epoch);
    Some((
        ticks / frequency,
        ticks % frequency * NANOSECONDS_PER_SECOND / frequency,
    ))
}

fn linux_clock_supported(clock: u64) -> bool {
    matches!(
        clock,
        LINUX_CLOCK_MONOTONIC
            | LINUX_CLOCK_PROCESS_CPUTIME_ID
            | LINUX_CLOCK_THREAD_CPUTIME_ID
            | LINUX_CLOCK_MONOTONIC_RAW
    )
}

fn linux_times(output_address: u64) -> u64 {
    let frequency = ELF_PROBE_CLOCK_FREQUENCY.load(Ordering::SeqCst);
    let epoch = ELF_PROBE_CLOCK_EPOCH.load(Ordering::SeqCst);
    if frequency == 0 || epoch == 0 {
        return LINUX_ENOSYS;
    }
    let elapsed = timer::timestamp_counter().saturating_sub(epoch);
    let ticks = elapsed / frequency * LINUX_CLOCK_TICKS_PER_SECOND
        + elapsed % frequency * LINUX_CLOCK_TICKS_PER_SECOND / frequency;
    if output_address != 0 {
        let mut accounting = [0_u8; 32];
        accounting[..8].copy_from_slice(&ticks.to_ne_bytes());
        if !copy_to_linux_user(output_address, &accounting) {
            return LINUX_EFAULT;
        }
    }
    ticks
}

fn linux_getrusage(who: u64, output_address: u64) -> u64 {
    let children = who == u32::MAX as u64 || who == u64::MAX;
    if !matches!(who, 0 | 1) && !children {
        return LINUX_EINVAL;
    }
    let mut usage = [0_u8; 144];
    if !children {
        let Some((seconds, nanoseconds)) = linux_elapsed_time() else {
            return LINUX_ENOSYS;
        };
        usage[..8].copy_from_slice(&seconds.to_ne_bytes());
        usage[8..16].copy_from_slice(&(nanoseconds / 1_000).to_ne_bytes());
    }
    if !copy_to_linux_user(output_address, &usage) {
        return LINUX_EFAULT;
    }
    0
}

fn linux_getitimer(which: u64, output_address: u64) -> u64 {
    if which > 2 {
        return LINUX_EINVAL;
    }
    if !copy_to_linux_user(output_address, &[0_u8; 32]) {
        return LINUX_EFAULT;
    }
    0
}

fn linux_setitimer(which: u64, input_address: u64, old_output_address: u64) -> u64 {
    if which > 2 {
        return LINUX_EINVAL;
    }
    if !linux_readable_user_range_contains(input_address, 32)
        || (old_output_address != 0 && !linux_writable_user_range_contains(old_output_address, 32))
    {
        return LINUX_EFAULT;
    }
    let mut requested = [0_u8; 32];
    if !copy_from_linux_user(input_address, &mut requested) {
        return LINUX_EFAULT;
    }
    if requested != [0_u8; 32] {
        return LINUX_ENOSYS;
    }
    if old_output_address != 0 && !copy_to_linux_user(old_output_address, &[0_u8; 32]) {
        return LINUX_EFAULT;
    }
    0
}

fn linux_futex(address: u64, operation: u64, value: u64) -> u64 {
    if !address.is_multiple_of(size_of::<u32>() as u64) {
        return LINUX_EINVAL;
    }
    let mut bytes = [0_u8; size_of::<u32>()];
    if !copy_from_linux_user(address, &mut bytes) {
        return LINUX_EFAULT;
    }
    match operation {
        0 | 128 => {
            if u64::from(u32::from_ne_bytes(bytes)) != value {
                LINUX_EAGAIN
            } else {
                LINUX_ENOSYS
            }
        }
        1 | 129 => 0,
        _ => LINUX_EINVAL,
    }
}

fn linux_mmap(address: u64, length: u64, protection: u64, flags: u64, fd: u64, offset: u64) -> u64 {
    let pages = match length {
        length if length == PAGE_SIZE as u64 => 1,
        length if length == 2 * PAGE_SIZE as u64 => 2,
        _ => return LINUX_EINVAL,
    };
    if address != 0
        || protection != (LINUX_PROT_READ | LINUX_PROT_WRITE)
        || flags != (LINUX_MAP_PRIVATE | LINUX_MAP_ANONYMOUS)
        || fd != u64::MAX
        || offset != 0
    {
        return LINUX_EINVAL;
    }
    if ELF_PROBE_MMAP_PAGES.load(Ordering::SeqCst) != 0 {
        return LINUX_ENOMEM;
    }
    let mapping = ELF_PROBE_MMAP.load(Ordering::SeqCst);
    let frame = ELF_PROBE_MMAP_FRAME.load(Ordering::SeqCst);
    if mapping == 0 || frame == 0 || !address_space::map_current_linux_page(mapping, frame) {
        return LINUX_ENOMEM;
    }
    if pages == 2 {
        let tail_frame = ELF_PROBE_MMAP_TAIL_FRAME.load(Ordering::SeqCst);
        if tail_frame == 0
            || !address_space::map_current_linux_page(mapping + PAGE_SIZE as u64, tail_frame)
        {
            let _ = address_space::unmap_current_linux_page(mapping, frame);
            return LINUX_ENOMEM;
        }
    }
    ELF_PROBE_MMAP_PAGES.store(pages, Ordering::SeqCst);
    ELF_PROBE_MMAP_WRITABLE.store(true, Ordering::SeqCst);
    mapping
}

fn linux_munmap(address: u64, length: u64) -> u64 {
    let mapping = ELF_PROBE_MMAP.load(Ordering::SeqCst);
    let pages = ELF_PROBE_MMAP_PAGES.load(Ordering::SeqCst);
    if address != mapping
        || !matches!(length, length if length == PAGE_SIZE as u64 || length == 2 * PAGE_SIZE as u64)
        || (pages != 0 && length != u64::from(pages) * PAGE_SIZE as u64)
    {
        return LINUX_EINVAL;
    }
    if pages == 0 {
        return 0;
    }
    let frame = ELF_PROBE_MMAP_FRAME.load(Ordering::SeqCst);
    if !address_space::unmap_current_linux_page(mapping, frame) {
        return LINUX_EINVAL;
    }
    if pages == 2 {
        let tail_frame = ELF_PROBE_MMAP_TAIL_FRAME.load(Ordering::SeqCst);
        if !address_space::unmap_current_linux_page(mapping + PAGE_SIZE as u64, tail_frame) {
            return LINUX_EINVAL;
        }
    }
    ELF_PROBE_MMAP_PAGES.store(0, Ordering::SeqCst);
    ELF_PROBE_MMAP_WRITABLE.store(false, Ordering::SeqCst);
    0
}

fn linux_madvise(address: u64, length: u64, advice: u64) -> u64 {
    let mapping = ELF_PROBE_MMAP.load(Ordering::SeqCst);
    let pages = ELF_PROBE_MMAP_PAGES.load(Ordering::SeqCst);
    if address != mapping
        || pages == 0
        || length != u64::from(pages) * PAGE_SIZE as u64
        || advice != LINUX_MADV_DONTNEED
    {
        return LINUX_EINVAL;
    }
    let frame = ELF_PROBE_MMAP_FRAME.load(Ordering::SeqCst);
    if !address_space::zero_current_linux_page(mapping, frame) {
        return LINUX_EINVAL;
    }
    if pages == 2
        && !address_space::zero_current_linux_page(
            mapping + PAGE_SIZE as u64,
            ELF_PROBE_MMAP_TAIL_FRAME.load(Ordering::SeqCst),
        )
    {
        return LINUX_EINVAL;
    }
    0
}

fn linux_mincore(address: u64, length: u64, vector: u64) -> u64 {
    let mapping = ELF_PROBE_MMAP.load(Ordering::SeqCst);
    let pages = ELF_PROBE_MMAP_PAGES.load(Ordering::SeqCst);
    if address != mapping
        || !matches!(length, length if length == PAGE_SIZE as u64 || length == 2 * PAGE_SIZE as u64)
    {
        return LINUX_EINVAL;
    }
    if pages == 0 || length != u64::from(pages) * PAGE_SIZE as u64 {
        return LINUX_ENOMEM;
    }
    if !copy_to_linux_user(vector, &[1, 1][..usize::from(pages)]) {
        return LINUX_EFAULT;
    }
    0
}

fn linux_clock_getres(clock: u64, output_address: u64) -> u64 {
    if !linux_clock_supported(clock) {
        return LINUX_EINVAL;
    }
    let frequency = ELF_PROBE_CLOCK_FREQUENCY.load(Ordering::SeqCst);
    if frequency == 0 {
        return LINUX_ENOSYS;
    }
    if output_address == 0 {
        return 0;
    }
    let resolution = NANOSECONDS_PER_SECOND.div_ceil(frequency);
    let mut timespec = [0_u8; 16];
    timespec[..8].copy_from_slice(&(resolution / NANOSECONDS_PER_SECOND).to_ne_bytes());
    timespec[8..].copy_from_slice(&(resolution % NANOSECONDS_PER_SECOND).to_ne_bytes());
    if !copy_to_linux_user(output_address, &timespec) {
        return LINUX_EFAULT;
    }
    0
}

fn copy_linux_robust_list(head_address: u64, size_address: u64) -> bool {
    let word_size = size_of::<u64>();
    if !linux_writable_user_range_contains(head_address, word_size)
        || !linux_writable_user_range_contains(size_address, word_size)
    {
        return false;
    }
    let head = ELF_PROBE_ROBUST_LIST.load(Ordering::SeqCst).to_ne_bytes();
    let size = (LINUX_ROBUST_LIST_HEAD_SIZE as u64).to_ne_bytes();
    copy_to_linux_user(head_address, &head) && copy_to_linux_user(size_address, &size)
}

fn linux_readlink_self(path_address: u64, output_address: u64, output_size: u64) -> u64 {
    if output_size == 0 {
        return LINUX_EINVAL;
    }
    let mut path = [0_u8; 16];
    let path_length = match copy_linux_c_string(path_address, &mut path) {
        Ok(length) => length,
        Err(error) => return error,
    };
    if &path[..path_length] != b"/proc/self/exe" {
        return LINUX_ENOENT;
    }
    let output_length = usize::try_from(output_size).unwrap_or(usize::MAX).min(5);
    if !copy_to_linux_user(output_address, &b"/init"[..output_length]) {
        return LINUX_EFAULT;
    }
    output_length as u64
}

fn linux_openat(_directory: u64, path_address: u64, flags: u64) -> u64 {
    let mut path = [0_u8; 16];
    let path_length = match copy_linux_c_string(path_address, &mut path) {
        Ok(length) => length,
        Err(error) => return error,
    };
    let kind = match &path[..path_length] {
        b"/dev/null" => 0,
        b"/dev/zero" => LINUX_DEVICE_ZERO,
        b"/dev/console" | b"/dev/tty" => LINUX_DEVICE_CONSOLE,
        b"/dev" => LINUX_DEVICE_DIRECTORY,
        _ => return LINUX_ENOENT,
    };
    let access = flags & 3;
    if access == 3 || flags & !(3 | LINUX_O_CLOEXEC | LINUX_O_DIRECTORY) != 0 {
        return LINUX_EINVAL;
    }
    if kind & LINUX_DEVICE_DIRECTORY != 0 && access != 0 {
        return LINUX_EISDIR;
    }
    if kind & LINUX_DEVICE_DIRECTORY == 0 && flags & LINUX_O_DIRECTORY != 0 {
        return LINUX_ENOTDIR;
    }
    let state = kind | (access + 1) as u8;
    let Some(descriptor) = (LINUX_AUX_DESCRIPTOR_FIRST..LINUX_AUX_DESCRIPTOR_LIMIT)
        .find(|descriptor| linux_aux_descriptor_free(*descriptor))
    else {
        return LINUX_EMFILE;
    };
    ELF_PROBE_DEVICE_DESCRIPTORS[(descriptor - LINUX_AUX_DESCRIPTOR_FIRST) as usize]
        .store(state, Ordering::SeqCst);
    if flags & LINUX_O_CLOEXEC != 0 {
        ELF_PROBE_CONSOLE_CLOEXEC.fetch_or(1 << descriptor, Ordering::SeqCst);
    } else {
        ELF_PROBE_CONSOLE_CLOEXEC.fetch_and(!(1 << descriptor), Ordering::SeqCst);
    }
    linux_directory_cursor(descriptor).store(0, Ordering::SeqCst);
    descriptor
}

fn linux_newfstatat(_directory: u64, path_address: u64, stat_address: u64, flags: u64) -> u64 {
    if flags != 0 {
        return LINUX_EINVAL;
    }
    let mut path = [0_u8; 16];
    let path_length = match copy_linux_c_string(path_address, &mut path) {
        Ok(length) => length,
        Err(error) => return error,
    };
    let stat = match &path[..path_length] {
        b"/dev/null" | b"/dev/zero" | b"/dev/console" | b"/dev/tty" => {
            linux_stat(LINUX_STAT_CHARACTER_DEVICE_MODE, 1)
        }
        b"/dev" => linux_stat(LINUX_STAT_DIRECTORY_MODE, 2),
        _ => return LINUX_ENOENT,
    };
    if !copy_to_linux_user(stat_address, &stat) {
        return LINUX_EFAULT;
    }
    0
}

fn linux_faccessat(path_address: u64, mode: u64, flags: u64) -> u64 {
    if flags != 0 || mode & !7 != 0 {
        return LINUX_EINVAL;
    }
    let mut path = [0_u8; 16];
    let path_length = match copy_linux_c_string(path_address, &mut path) {
        Ok(length) => length,
        Err(error) => return error,
    };
    let directory = match &path[..path_length] {
        b"/dev/null" | b"/dev/zero" | b"/dev/console" | b"/dev/tty" => false,
        b"/dev" => true,
        _ => return LINUX_ENOENT,
    };
    if !directory && mode & 1 != 0 || directory && mode & 2 != 0 {
        LINUX_EACCES
    } else {
        0
    }
}

fn linux_statx(path_address: u64, flags: u64, mask: u64, stat_address: u64) -> u64 {
    if flags != 0 || mask & u64::from(LINUX_STATX_RESERVED) != 0 {
        return LINUX_EINVAL;
    }
    let mut path = [0_u8; 16];
    let path_length = match copy_linux_c_string(path_address, &mut path) {
        Ok(length) => length,
        Err(error) => return error,
    };
    let (inode, minor, mode) = match &path[..path_length] {
        b"/dev/null" => (1_u64, 3_u32, LINUX_STAT_CHARACTER_DEVICE_MODE),
        b"/dev/zero" => (2, 5, LINUX_STAT_CHARACTER_DEVICE_MODE),
        b"/dev/console" => (3, 1, LINUX_STAT_CHARACTER_DEVICE_MODE),
        b"/dev/tty" => (3, 0, LINUX_STAT_CHARACTER_DEVICE_MODE),
        b"/dev" => (4, 0, LINUX_STAT_DIRECTORY_MODE),
        _ => return LINUX_ENOENT,
    };
    let mut stat = [0_u8; 256];
    stat[..4].copy_from_slice(&LINUX_STATX_BASIC_STATS.to_ne_bytes());
    stat[4..8].copy_from_slice(&(PAGE_SIZE as u32).to_ne_bytes());
    stat[16..20].copy_from_slice(&1_u32.to_ne_bytes());
    stat[28..30].copy_from_slice(&(mode as u16).to_ne_bytes());
    stat[32..40].copy_from_slice(&inode.to_ne_bytes());
    if mode == LINUX_STAT_CHARACTER_DEVICE_MODE {
        stat[128..132].copy_from_slice(&1_u32.to_ne_bytes());
        stat[132..136].copy_from_slice(&minor.to_ne_bytes());
    }
    if !copy_to_linux_user(stat_address, &stat) {
        return LINUX_EFAULT;
    }
    0
}

fn linux_descriptor_read(descriptor: u64, address: u64, count: u64) -> u64 {
    if let Some(result) = linux_eventfd_read(descriptor, address, count) {
        return result;
    }
    let device = linux_device_descriptor(descriptor);
    if device & LINUX_DEVICE_DIRECTORY != 0 {
        return LINUX_EISDIR;
    }
    let access = device & 3;
    if access == LINUX_DESCRIPTOR_INPUT || access == 3 {
        if device & LINUX_DEVICE_ZERO == 0 || count == 0 {
            return 0;
        }
        let Ok(length) = usize::try_from(count) else {
            return LINUX_EINVAL;
        };
        if length > LINUX_WRITE_LIMIT {
            return LINUX_EINVAL;
        }
        if !copy_to_linux_user(address, &[0_u8; LINUX_WRITE_LIMIT][..length]) {
            return LINUX_EFAULT;
        }
        count
    } else if linux_console_descriptor(descriptor) == LINUX_DESCRIPTOR_INPUT {
        0
    } else {
        LINUX_EBADF
    }
}

fn linux_poll(address: u64, count: u64, timeout: u64) -> u64 {
    let Ok(descriptor_count) = usize::try_from(count) else {
        return LINUX_EINVAL;
    };
    if descriptor_count > LINUX_POLLFD_LIMIT {
        return LINUX_EINVAL;
    }
    if descriptor_count == 0 {
        return if timeout as u32 == 0 { 0 } else { LINUX_ENOSYS };
    }
    let mut pollfds = [0_u8; LINUX_POLLFD_LIMIT * 8];
    let active = &mut pollfds[..descriptor_count * 8];
    if !copy_from_linux_user(address, active) {
        return LINUX_EFAULT;
    }
    let mut ready = 0_u64;
    for pollfd in active.as_chunks_mut::<8>().0 {
        let descriptor = i32::from_ne_bytes(pollfd[..4].try_into().unwrap());
        let events = u16::from_ne_bytes(pollfd[4..6].try_into().unwrap());
        let returned = linux_descriptor_poll_events(descriptor, events);
        pollfd[6..8].copy_from_slice(&returned.to_ne_bytes());
        ready += u64::from(returned != 0);
    }
    if ready == 0 && timeout as u32 != 0 {
        return LINUX_ENOSYS;
    }
    if !copy_to_linux_user(address, active) {
        return LINUX_EFAULT;
    }
    ready
}

fn linux_ppoll(
    address: u64,
    count: u64,
    timeout_address: u64,
    signal_mask_address: u64,
    signal_set_size: u64,
) -> u64 {
    let waits = if timeout_address == 0 {
        true
    } else {
        let mut timeout = [0_u8; 16];
        if !copy_from_linux_user(timeout_address, &mut timeout) {
            return LINUX_EFAULT;
        }
        let seconds = i64::from_ne_bytes(timeout[..8].try_into().unwrap());
        let nanoseconds = i64::from_ne_bytes(timeout[8..].try_into().unwrap());
        if seconds < 0 || !(0..NANOSECONDS_PER_SECOND as i64).contains(&nanoseconds) {
            return LINUX_EINVAL;
        }
        seconds != 0 || nanoseconds != 0
    };
    if signal_mask_address != 0 {
        if signal_set_size != LINUX_SIGNAL_SET_SIZE {
            return LINUX_EINVAL;
        }
        let mut signal_mask = [0_u8; LINUX_SIGNAL_SET_SIZE as usize];
        if !copy_from_linux_user(signal_mask_address, &mut signal_mask) {
            return LINUX_EFAULT;
        }
    }
    linux_poll(address, count, u64::from(waits))
}

fn linux_epoll_create1(flags: u64) -> u64 {
    if flags & !LINUX_O_CLOEXEC != 0 {
        return LINUX_EINVAL;
    }
    if ELF_PROBE_EPOLL_DESCRIPTOR.load(Ordering::SeqCst) != 0 {
        return LINUX_EMFILE;
    }
    let Some(descriptor) = (LINUX_AUX_DESCRIPTOR_FIRST..LINUX_AUX_DESCRIPTOR_LIMIT)
        .find(|descriptor| linux_aux_descriptor_free(*descriptor))
    else {
        return LINUX_EMFILE;
    };
    ELF_PROBE_EPOLL_DESCRIPTOR.store(descriptor as u8, Ordering::SeqCst);
    ELF_PROBE_EPOLL_TARGET.store(u64::MAX, Ordering::SeqCst);
    ELF_PROBE_EPOLL_EVENTS.store(0, Ordering::SeqCst);
    ELF_PROBE_EPOLL_DATA.store(0, Ordering::SeqCst);
    if flags & LINUX_O_CLOEXEC != 0 {
        ELF_PROBE_CONSOLE_CLOEXEC.fetch_or(1 << descriptor, Ordering::SeqCst);
    } else {
        ELF_PROBE_CONSOLE_CLOEXEC.fetch_and(!(1 << descriptor), Ordering::SeqCst);
    }
    descriptor
}

fn linux_eventfd(initial: u64, flags: u64) -> u64 {
    if flags & !(LINUX_EFD_SEMAPHORE | LINUX_EFD_NONBLOCK | LINUX_O_CLOEXEC) != 0 {
        return LINUX_EINVAL;
    }
    if ELF_PROBE_EVENTFD_DESCRIPTOR.load(Ordering::SeqCst) != 0 {
        return LINUX_EMFILE;
    }
    let Some(descriptor) = (LINUX_AUX_DESCRIPTOR_FIRST..LINUX_AUX_DESCRIPTOR_LIMIT)
        .find(|descriptor| linux_aux_descriptor_free(*descriptor))
    else {
        return LINUX_EMFILE;
    };
    ELF_PROBE_EVENTFD_COUNTER.store(u64::from(initial as u32), Ordering::SeqCst);
    ELF_PROBE_EVENTFD_FLAGS.store(flags as u32, Ordering::SeqCst);
    ELF_PROBE_EVENTFD_DESCRIPTOR.store(descriptor as u8, Ordering::SeqCst);
    if flags & LINUX_O_CLOEXEC != 0 {
        ELF_PROBE_CONSOLE_CLOEXEC.fetch_or(1 << descriptor, Ordering::SeqCst);
    } else {
        ELF_PROBE_CONSOLE_CLOEXEC.fetch_and(!(1 << descriptor), Ordering::SeqCst);
    }
    descriptor
}

fn linux_eventfd_read(descriptor: u64, address: u64, count: u64) -> Option<u64> {
    if !linux_eventfd_descriptor(descriptor) {
        return None;
    }
    if count != 8 {
        return Some(LINUX_EINVAL);
    }
    let current = ELF_PROBE_EVENTFD_COUNTER.load(Ordering::SeqCst);
    if current == 0 {
        return Some(
            if ELF_PROBE_EVENTFD_FLAGS.load(Ordering::SeqCst) & LINUX_EFD_NONBLOCK as u32 != 0 {
                LINUX_EAGAIN
            } else {
                LINUX_ENOSYS
            },
        );
    }
    let semaphore =
        ELF_PROBE_EVENTFD_FLAGS.load(Ordering::SeqCst) & LINUX_EFD_SEMAPHORE as u32 != 0;
    let result = if semaphore { 1 } else { current };
    if !copy_to_linux_user(address, &result.to_ne_bytes()) {
        return Some(LINUX_EFAULT);
    }
    ELF_PROBE_EVENTFD_COUNTER.store(if semaphore { current - 1 } else { 0 }, Ordering::SeqCst);
    Some(8)
}

fn linux_eventfd_write(descriptor: u64, address: u64, count: u64) -> Option<u64> {
    if !linux_eventfd_descriptor(descriptor) {
        return None;
    }
    if count != 8 {
        return Some(LINUX_EINVAL);
    }
    let mut bytes = [0_u8; 8];
    if !copy_from_linux_user(address, &mut bytes) {
        return Some(LINUX_EFAULT);
    }
    let increment = u64::from_ne_bytes(bytes);
    if increment == u64::MAX {
        return Some(LINUX_EINVAL);
    }
    let current = ELF_PROBE_EVENTFD_COUNTER.load(Ordering::SeqCst);
    let Some(next) = current.checked_add(increment) else {
        return Some(
            if ELF_PROBE_EVENTFD_FLAGS.load(Ordering::SeqCst) & LINUX_EFD_NONBLOCK as u32 != 0 {
                LINUX_EAGAIN
            } else {
                LINUX_ENOSYS
            },
        );
    };
    if next == u64::MAX {
        return Some(
            if ELF_PROBE_EVENTFD_FLAGS.load(Ordering::SeqCst) & LINUX_EFD_NONBLOCK as u32 != 0 {
                LINUX_EAGAIN
            } else {
                LINUX_ENOSYS
            },
        );
    }
    ELF_PROBE_EVENTFD_COUNTER.store(next, Ordering::SeqCst);
    Some(8)
}

fn linux_epoll_ctl(epoll: u64, operation: u64, descriptor: u64, event_address: u64) -> u64 {
    if !linux_epoll_descriptor(epoll) {
        return LINUX_EBADF;
    }
    if descriptor == epoll {
        return LINUX_EINVAL;
    }
    if !linux_descriptor_open(descriptor) {
        return LINUX_EBADF;
    }
    let current = ELF_PROBE_EPOLL_TARGET.load(Ordering::SeqCst);
    if operation == LINUX_EPOLL_CTL_DEL {
        if current != descriptor {
            return LINUX_ENOENT;
        }
        ELF_PROBE_EPOLL_TARGET.store(u64::MAX, Ordering::SeqCst);
        ELF_PROBE_EPOLL_EVENTS.store(0, Ordering::SeqCst);
        ELF_PROBE_EPOLL_DATA.store(0, Ordering::SeqCst);
        return 0;
    }
    if !matches!(operation, LINUX_EPOLL_CTL_ADD | LINUX_EPOLL_CTL_MOD) {
        return LINUX_EINVAL;
    }
    if operation == LINUX_EPOLL_CTL_ADD && current != u64::MAX {
        return LINUX_EEXIST;
    }
    if operation == LINUX_EPOLL_CTL_MOD && current != descriptor {
        return LINUX_ENOENT;
    }
    let mut event = [0_u8; 12];
    if !copy_from_linux_user(event_address, &mut event) {
        return LINUX_EFAULT;
    }
    let events = u32::from_ne_bytes(event[..4].try_into().unwrap());
    if events & !LINUX_EPOLL_SUPPORTED_EVENTS != 0 {
        return LINUX_EINVAL;
    }
    let data = u64::from_ne_bytes(event[4..].try_into().unwrap());
    ELF_PROBE_EPOLL_EVENTS.store(events, Ordering::SeqCst);
    ELF_PROBE_EPOLL_DATA.store(data, Ordering::SeqCst);
    ELF_PROBE_EPOLL_TARGET.store(descriptor, Ordering::SeqCst);
    0
}

fn linux_epoll_wait(epoll: u64, address: u64, maximum: u64, timeout: u64) -> u64 {
    if !linux_epoll_descriptor(epoll) {
        return LINUX_EBADF;
    }
    if maximum != 1 {
        return LINUX_EINVAL;
    }
    let target = ELF_PROBE_EPOLL_TARGET.load(Ordering::SeqCst);
    let requested = ELF_PROBE_EPOLL_EVENTS.load(Ordering::SeqCst) as u16;
    let ready = if target == u64::MAX {
        0
    } else {
        linux_descriptor_poll_events(target as i32, requested)
            & (LINUX_POLLIN | LINUX_POLLOUT | LINUX_POLLHUP)
    };
    if ready == 0 {
        return if timeout as u32 == 0 { 0 } else { LINUX_ENOSYS };
    }
    let mut event = [0_u8; 12];
    event[..4].copy_from_slice(&u32::from(ready).to_ne_bytes());
    event[4..].copy_from_slice(&ELF_PROBE_EPOLL_DATA.load(Ordering::SeqCst).to_ne_bytes());
    if !copy_to_linux_user(address, &event) {
        return LINUX_EFAULT;
    }
    1
}

fn linux_epoll_pwait(
    epoll: u64,
    address: u64,
    maximum: u64,
    timeout: u64,
    signal_mask_address: u64,
    signal_set_size: u64,
) -> u64 {
    if signal_mask_address != 0 {
        if signal_set_size != LINUX_SIGNAL_SET_SIZE {
            return LINUX_EINVAL;
        }
        let mut signal_mask = [0_u8; LINUX_SIGNAL_SET_SIZE as usize];
        if !copy_from_linux_user(signal_mask_address, &mut signal_mask) {
            return LINUX_EFAULT;
        }
    }
    linux_epoll_wait(epoll, address, maximum, timeout)
}

fn linux_epoll_pwait2(
    epoll: u64,
    address: u64,
    maximum: u64,
    timeout_address: u64,
    signal_mask_address: u64,
    signal_set_size: u64,
) -> u64 {
    let waits = if timeout_address == 0 {
        true
    } else {
        let mut timeout = [0_u8; 16];
        if !copy_from_linux_user(timeout_address, &mut timeout) {
            return LINUX_EFAULT;
        }
        let seconds = i64::from_ne_bytes(timeout[..8].try_into().unwrap());
        let nanoseconds = i64::from_ne_bytes(timeout[8..].try_into().unwrap());
        if seconds < 0 || !(0..NANOSECONDS_PER_SECOND as i64).contains(&nanoseconds) {
            return LINUX_EINVAL;
        }
        seconds != 0 || nanoseconds != 0
    };
    linux_epoll_pwait(
        epoll,
        address,
        maximum,
        u64::from(waits),
        signal_mask_address,
        signal_set_size,
    )
}

fn linux_descriptor_open(descriptor: u64) -> bool {
    linux_console_descriptor_open(descriptor)
        || linux_device_descriptor(descriptor) != 0
        || linux_epoll_descriptor(descriptor)
        || linux_eventfd_descriptor(descriptor)
}

fn linux_epoll_descriptor(descriptor: u64) -> bool {
    descriptor >= LINUX_AUX_DESCRIPTOR_FIRST
        && u64::from(ELF_PROBE_EPOLL_DESCRIPTOR.load(Ordering::SeqCst)) == descriptor
}

fn linux_eventfd_descriptor(descriptor: u64) -> bool {
    descriptor >= LINUX_AUX_DESCRIPTOR_FIRST
        && u64::from(ELF_PROBE_EVENTFD_DESCRIPTOR.load(Ordering::SeqCst)) == descriptor
}

fn linux_aux_descriptor_free(descriptor: u64) -> bool {
    (LINUX_AUX_DESCRIPTOR_FIRST..LINUX_AUX_DESCRIPTOR_LIMIT).contains(&descriptor)
        && linux_device_descriptor(descriptor) == 0
        && !linux_epoll_descriptor(descriptor)
        && !linux_eventfd_descriptor(descriptor)
}

fn linux_descriptor_poll_events(descriptor: i32, requested: u16) -> u16 {
    let Ok(descriptor) = u64::try_from(descriptor) else {
        return 0;
    };
    match linux_console_descriptor(descriptor) {
        LINUX_DESCRIPTOR_INPUT => return LINUX_POLLHUP,
        LINUX_DESCRIPTOR_OUTPUT => return requested & LINUX_POLLOUT,
        _ => {}
    }
    if linux_epoll_descriptor(descriptor) {
        let target = ELF_PROBE_EPOLL_TARGET.load(Ordering::SeqCst);
        let events = ELF_PROBE_EPOLL_EVENTS.load(Ordering::SeqCst) as u16;
        return if target != u64::MAX && linux_descriptor_poll_events(target as i32, events) != 0 {
            requested & LINUX_POLLIN
        } else {
            0
        };
    }
    if linux_eventfd_descriptor(descriptor) {
        let counter = ELF_PROBE_EVENTFD_COUNTER.load(Ordering::SeqCst);
        let mut returned = 0;
        if counter != 0 {
            returned |= requested & LINUX_POLLIN;
        }
        if counter < u64::MAX - 1 {
            returned |= requested & LINUX_POLLOUT;
        }
        return returned;
    }
    let device = linux_device_descriptor(descriptor);
    if device == 0 {
        return LINUX_POLLNVAL;
    }
    let access = device & 3;
    let mut returned = 0;
    if matches!(access, LINUX_DESCRIPTOR_INPUT | 3) {
        returned |= requested & LINUX_POLLIN;
    }
    if matches!(access, LINUX_DESCRIPTOR_OUTPUT | 3) {
        returned |= requested & LINUX_POLLOUT;
    }
    returned
}

fn linux_descriptor_write(descriptor: u64, address: u64, count: u64) -> u64 {
    if let Some(result) = linux_eventfd_write(descriptor, address, count) {
        return result;
    }
    let device = linux_device_descriptor(descriptor);
    let access = device & 3;
    if access == LINUX_DESCRIPTOR_OUTPUT || access == 3 {
        if device & LINUX_DEVICE_CONSOLE != 0 {
            linux_bounded_console_write(address, count)
        } else {
            count
        }
    } else {
        linux_console_write(descriptor, address, count)
    }
}

fn linux_descriptor_readv(descriptor: u64, vectors: u64, count: u64) -> u64 {
    let device = linux_device_descriptor(descriptor);
    let access = device & 3;
    if access != LINUX_DESCRIPTOR_INPUT
        && access != 3
        && linux_console_descriptor(descriptor) != LINUX_DESCRIPTOR_INPUT
    {
        return LINUX_EBADF;
    }
    let Ok(vector_count) = usize::try_from(count) else {
        return LINUX_EINVAL;
    };
    if !(1..=LINUX_IOV_LIMIT).contains(&vector_count) {
        return LINUX_EINVAL;
    }
    let mut table = [0_u8; LINUX_IOV_LIMIT * 16];
    if !copy_from_linux_user(vectors, &mut table[..vector_count * 16]) {
        return LINUX_EFAULT;
    }
    let mut ranges = [(0_u64, 0_usize); LINUX_IOV_LIMIT];
    let mut total = 0_usize;
    for (index, vector) in table[..vector_count * 16]
        .as_chunks::<16>()
        .0
        .iter()
        .enumerate()
    {
        let address = u64::from_ne_bytes(vector[..8].try_into().unwrap());
        let length = u64::from_ne_bytes(vector[8..].try_into().unwrap());
        let Ok(length) = usize::try_from(length) else {
            return LINUX_EINVAL;
        };
        let Some(next_total) = total.checked_add(length) else {
            return LINUX_EINVAL;
        };
        if length == 0
            || next_total > LINUX_WRITE_LIMIT
            || !linux_writable_user_range_contains(address, length)
        {
            return if length != 0 && next_total <= LINUX_WRITE_LIMIT {
                LINUX_EFAULT
            } else {
                LINUX_EINVAL
            };
        }
        ranges[index] = (address, length);
        total = next_total;
    }
    if device & LINUX_DEVICE_ZERO == 0 {
        return 0;
    }
    let zeros = [0_u8; LINUX_WRITE_LIMIT];
    for (address, length) in ranges[..vector_count].iter().copied() {
        if !copy_to_linux_user(address, &zeros[..length]) {
            return LINUX_EFAULT;
        }
    }
    total as u64
}

fn linux_console_write(descriptor: u64, address: u64, count: u64) -> u64 {
    if linux_console_descriptor(descriptor) != LINUX_DESCRIPTOR_OUTPUT {
        return LINUX_EBADF;
    }
    linux_bounded_console_write(address, count)
}

fn linux_bounded_console_write(address: u64, count: u64) -> u64 {
    let Ok(length) = usize::try_from(count) else {
        return LINUX_EINVAL;
    };
    if !(1..=LINUX_WRITE_LIMIT).contains(&length) {
        return LINUX_EINVAL;
    }
    let mut bytes = [0_u8; LINUX_WRITE_LIMIT];
    if !copy_from_linux_user(address, &mut bytes[..length]) {
        return LINUX_EFAULT;
    }
    crate::serial_write_bytes(&bytes[..length]);
    count
}

fn linux_descriptor_writev(descriptor: u64, vectors: u64, count: u64) -> u64 {
    let device = linux_device_descriptor(descriptor);
    let access = device & 3;
    let writable_device = device != 0 && (access == LINUX_DESCRIPTOR_OUTPUT || access == 3);
    let serial = linux_console_descriptor(descriptor) == LINUX_DESCRIPTOR_OUTPUT
        || writable_device && device & LINUX_DEVICE_CONSOLE != 0;
    let discard = writable_device && device & LINUX_DEVICE_CONSOLE == 0;
    if !serial && !discard {
        return LINUX_EBADF;
    }
    let Ok(vector_count) = usize::try_from(count) else {
        return LINUX_EINVAL;
    };
    if !(1..=LINUX_IOV_LIMIT).contains(&vector_count) {
        return LINUX_EINVAL;
    }
    let mut table = [0_u8; LINUX_IOV_LIMIT * 16];
    if !copy_from_linux_user(vectors, &mut table[..vector_count * 16]) {
        return LINUX_EFAULT;
    }
    let mut output = [0_u8; LINUX_WRITE_LIMIT];
    let mut total = 0_usize;
    for vector in table[..vector_count * 16].as_chunks::<16>().0 {
        let mut address_bytes = [0_u8; 8];
        address_bytes.copy_from_slice(&vector[..8]);
        let address = u64::from_ne_bytes(address_bytes);
        let mut length_bytes = [0_u8; 8];
        length_bytes.copy_from_slice(&vector[8..]);
        let length = u64::from_ne_bytes(length_bytes);
        let Ok(length) = usize::try_from(length) else {
            return LINUX_EINVAL;
        };
        let Some(end) = total.checked_add(length) else {
            return LINUX_EINVAL;
        };
        if length == 0 || end > LINUX_WRITE_LIMIT {
            return LINUX_EINVAL;
        }
        if serial && !copy_from_linux_user(address, &mut output[total..end]) {
            return LINUX_EFAULT;
        }
        total = end;
    }
    if serial {
        crate::serial_write_bytes(&output[..total]);
    }
    total as u64
}

fn linux_console_descriptor_open(descriptor: u64) -> bool {
    linux_console_descriptor(descriptor) != 0
}

fn linux_device_descriptor(descriptor: u64) -> u8 {
    descriptor
        .checked_sub(LINUX_AUX_DESCRIPTOR_FIRST)
        .and_then(|index| ELF_PROBE_DEVICE_DESCRIPTORS.get(index as usize))
        .map(|state| state.load(Ordering::SeqCst))
        .unwrap_or(0)
}

fn linux_directory_cursor(descriptor: u64) -> &'static AtomicU8 {
    &ELF_PROBE_DIRECTORY_CURSORS[(descriptor - LINUX_AUX_DESCRIPTOR_FIRST) as usize]
}

fn linux_console_descriptor(descriptor: u64) -> u8 {
    if descriptor >= 2 {
        return 0;
    }
    (ELF_PROBE_CONSOLE_DESCRIPTORS.load(Ordering::SeqCst) >> (descriptor * 2)) & 0b11
}

fn linux_console_close(descriptor: u64) -> u64 {
    if descriptor >= 2 {
        return LINUX_EBADF;
    }
    let shift = descriptor * 2;
    let mask = 0b11_u8 << shift;
    loop {
        let current = ELF_PROBE_CONSOLE_DESCRIPTORS.load(Ordering::SeqCst);
        if current & mask == 0 {
            return LINUX_EBADF;
        }
        if ELF_PROBE_CONSOLE_DESCRIPTORS
            .compare_exchange(current, current & !mask, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            ELF_PROBE_CONSOLE_CLOEXEC.fetch_and(!(1 << descriptor), Ordering::SeqCst);
            return 0;
        }
    }
}

fn linux_descriptor_close(descriptor: u64) -> u64 {
    if !(LINUX_AUX_DESCRIPTOR_FIRST..LINUX_AUX_DESCRIPTOR_LIMIT).contains(&descriptor) {
        return linux_console_close(descriptor);
    }
    if linux_epoll_descriptor(descriptor) {
        ELF_PROBE_EPOLL_DESCRIPTOR.store(0, Ordering::SeqCst);
        ELF_PROBE_EPOLL_TARGET.store(u64::MAX, Ordering::SeqCst);
        ELF_PROBE_EPOLL_EVENTS.store(0, Ordering::SeqCst);
        ELF_PROBE_EPOLL_DATA.store(0, Ordering::SeqCst);
        ELF_PROBE_CONSOLE_CLOEXEC.fetch_and(!(1 << descriptor), Ordering::SeqCst);
        return 0;
    }
    if linux_eventfd_descriptor(descriptor) {
        ELF_PROBE_EVENTFD_DESCRIPTOR.store(0, Ordering::SeqCst);
        ELF_PROBE_EVENTFD_COUNTER.store(0, Ordering::SeqCst);
        ELF_PROBE_EVENTFD_FLAGS.store(0, Ordering::SeqCst);
        ELF_PROBE_CONSOLE_CLOEXEC.fetch_and(!(1 << descriptor), Ordering::SeqCst);
        return 0;
    }
    if ELF_PROBE_DEVICE_DESCRIPTORS[(descriptor - LINUX_AUX_DESCRIPTOR_FIRST) as usize]
        .swap(0, Ordering::SeqCst)
        == 0
    {
        return LINUX_EBADF;
    }
    linux_directory_cursor(descriptor).store(0, Ordering::SeqCst);
    ELF_PROBE_CONSOLE_CLOEXEC.fetch_and(!(1 << descriptor), Ordering::SeqCst);
    0
}

fn linux_console_dup(descriptor: u64) -> u64 {
    linux_console_dup_at_least(descriptor, 0, false)
}

fn linux_console_dup_at_least(descriptor: u64, minimum: u64, close_on_exec: bool) -> u64 {
    if minimum >= 2 {
        return LINUX_EINVAL;
    }
    loop {
        let current = ELF_PROBE_CONSOLE_DESCRIPTORS.load(Ordering::SeqCst);
        let state = if descriptor < 2 {
            (current >> (descriptor * 2)) & 0b11
        } else {
            0
        };
        if state == 0 {
            return LINUX_EBADF;
        }
        let Some(target) = (minimum..2).find(|target| current & (0b11 << (target * 2)) == 0) else {
            return LINUX_EMFILE;
        };
        let shift = target * 2;
        if ELF_PROBE_CONSOLE_DESCRIPTORS
            .compare_exchange(
                current,
                current | (state << shift),
                Ordering::SeqCst,
                Ordering::SeqCst,
            )
            .is_ok()
        {
            let flag = 1 << target;
            if close_on_exec {
                ELF_PROBE_CONSOLE_CLOEXEC.fetch_or(flag, Ordering::SeqCst);
            } else {
                ELF_PROBE_CONSOLE_CLOEXEC.fetch_and(!flag, Ordering::SeqCst);
            }
            return target;
        }
    }
}

fn linux_console_dup2(source: u64, target: u64) -> u64 {
    if source >= 2 || target >= 2 {
        return LINUX_EBADF;
    }
    loop {
        let current = ELF_PROBE_CONSOLE_DESCRIPTORS.load(Ordering::SeqCst);
        let state = (current >> (source * 2)) & 0b11;
        if state == 0 {
            return LINUX_EBADF;
        }
        if source == target {
            return target;
        }
        let shift = target * 2;
        let mask = 0b11_u8 << shift;
        if ELF_PROBE_CONSOLE_DESCRIPTORS
            .compare_exchange(
                current,
                (current & !mask) | (state << shift),
                Ordering::SeqCst,
                Ordering::SeqCst,
            )
            .is_ok()
        {
            ELF_PROBE_CONSOLE_CLOEXEC.fetch_and(!(1 << target), Ordering::SeqCst);
            return target;
        }
    }
}

fn linux_console_fcntl(descriptor: u64, command: u64, argument: u64) -> u64 {
    let state = linux_console_descriptor(descriptor);
    if state == 0 {
        return LINUX_EBADF;
    }
    match command {
        LINUX_F_DUPFD => linux_console_dup_at_least(descriptor, argument, false),
        LINUX_F_DUPFD_CLOEXEC => linux_console_dup_at_least(descriptor, argument, true),
        LINUX_F_GETFD => {
            u64::from((ELF_PROBE_CONSOLE_CLOEXEC.load(Ordering::SeqCst) >> descriptor) & 1)
        }
        LINUX_F_SETFD => {
            let flag = 1 << descriptor;
            if argument & u64::from(LINUX_FD_CLOEXEC) != 0 {
                ELF_PROBE_CONSOLE_CLOEXEC.fetch_or(flag, Ordering::SeqCst);
            } else {
                ELF_PROBE_CONSOLE_CLOEXEC.fetch_and(!flag, Ordering::SeqCst);
            }
            0
        }
        LINUX_F_GETFL if state == LINUX_DESCRIPTOR_INPUT => 0,
        LINUX_F_GETFL if state == LINUX_DESCRIPTOR_OUTPUT => 1,
        _ => LINUX_EINVAL,
    }
}

fn linux_descriptor_fcntl(descriptor: u64, command: u64, argument: u64) -> u64 {
    let state = linux_device_descriptor(descriptor);
    if state == 0 {
        if linux_epoll_descriptor(descriptor) {
            return match command {
                LINUX_F_GETFD => {
                    u64::from((ELF_PROBE_CONSOLE_CLOEXEC.load(Ordering::SeqCst) >> descriptor) & 1)
                }
                LINUX_F_SETFD => {
                    if argument & u64::from(LINUX_FD_CLOEXEC) != 0 {
                        ELF_PROBE_CONSOLE_CLOEXEC.fetch_or(1 << descriptor, Ordering::SeqCst);
                    } else {
                        ELF_PROBE_CONSOLE_CLOEXEC.fetch_and(!(1 << descriptor), Ordering::SeqCst);
                    }
                    0
                }
                LINUX_F_GETFL => 2,
                _ => LINUX_EINVAL,
            };
        }
        if linux_eventfd_descriptor(descriptor) {
            return match command {
                LINUX_F_GETFD => {
                    u64::from((ELF_PROBE_CONSOLE_CLOEXEC.load(Ordering::SeqCst) >> descriptor) & 1)
                }
                LINUX_F_SETFD => {
                    if argument & u64::from(LINUX_FD_CLOEXEC) != 0 {
                        ELF_PROBE_CONSOLE_CLOEXEC.fetch_or(1 << descriptor, Ordering::SeqCst);
                    } else {
                        ELF_PROBE_CONSOLE_CLOEXEC.fetch_and(!(1 << descriptor), Ordering::SeqCst);
                    }
                    0
                }
                LINUX_F_GETFL => {
                    2 | u64::from(
                        ELF_PROBE_EVENTFD_FLAGS.load(Ordering::SeqCst) & LINUX_EFD_NONBLOCK as u32,
                    )
                }
                _ => LINUX_EINVAL,
            };
        }
        return linux_console_fcntl(descriptor, command, argument);
    }
    match command {
        LINUX_F_GETFD => {
            u64::from((ELF_PROBE_CONSOLE_CLOEXEC.load(Ordering::SeqCst) >> descriptor) & 1)
        }
        LINUX_F_SETFD => {
            let flag = 1 << descriptor;
            if argument & u64::from(LINUX_FD_CLOEXEC) != 0 {
                ELF_PROBE_CONSOLE_CLOEXEC.fetch_or(flag, Ordering::SeqCst);
            } else {
                ELF_PROBE_CONSOLE_CLOEXEC.fetch_and(!flag, Ordering::SeqCst);
            }
            0
        }
        LINUX_F_GETFL => u64::from((state & 3) - 1),
        _ => LINUX_EINVAL,
    }
}

fn linux_getdents64(descriptor: u64, address: u64, count: u64) -> u64 {
    if linux_device_descriptor(descriptor) & LINUX_DEVICE_DIRECTORY == 0 {
        return LINUX_ENOTDIR;
    }
    if linux_directory_cursor(descriptor).load(Ordering::SeqCst) != 0 {
        return 0;
    }
    let Ok(capacity) = usize::try_from(count) else {
        return LINUX_EINVAL;
    };
    let mut entries = [0_u8; 152];
    let mut used = 0_usize;
    for (inode, next, kind, name) in [
        (4_u64, 1_i64, 4_u8, &b"."[..]),
        (4, 2, 4, &b".."[..]),
        (1, 3, 2, &b"null"[..]),
        (2, 4, 2, &b"zero"[..]),
        (3, 5, 2, &b"console"[..]),
        (3, 6, 2, &b"tty"[..]),
    ] {
        if !append_linux_dirent(&mut entries, &mut used, inode, next, kind, name) {
            return LINUX_EINVAL;
        }
    }
    if capacity < used {
        return LINUX_EINVAL;
    }
    if !copy_to_linux_user(address, &entries[..used]) {
        return LINUX_EFAULT;
    }
    linux_directory_cursor(descriptor).store(1, Ordering::SeqCst);
    used as u64
}

fn append_linux_dirent(
    output: &mut [u8],
    used: &mut usize,
    inode: u64,
    next: i64,
    kind: u8,
    name: &[u8],
) -> bool {
    let record_size = (19 + name.len() + 1 + 7) & !7;
    let Some(end) = used.checked_add(record_size) else {
        return false;
    };
    let Some(record) = output.get_mut(*used..end) else {
        return false;
    };
    record[..8].copy_from_slice(&inode.to_ne_bytes());
    record[8..16].copy_from_slice(&next.to_ne_bytes());
    record[16..18].copy_from_slice(&(record_size as u16).to_ne_bytes());
    record[18] = kind;
    record[19..19 + name.len()].copy_from_slice(name);
    *used = end;
    true
}

fn linux_resource_limit(resource: u64) -> Option<[u8; 16]> {
    let value = match resource {
        LINUX_RLIMIT_STACK => PAGE_SIZE as u64,
        LINUX_RLIMIT_NOFILE => LINUX_AUX_DESCRIPTOR_LIMIT,
        _ => return None,
    };
    let mut limit = [0_u8; 16];
    limit[..8].copy_from_slice(&value.to_ne_bytes());
    limit[8..].copy_from_slice(&value.to_ne_bytes());
    Some(limit)
}

fn linux_console_stat() -> [u8; LINUX_STAT_SIZE] {
    linux_stat(LINUX_STAT_CHARACTER_DEVICE_MODE, 1)
}

fn linux_descriptor_stat(descriptor: u64) -> Option<[u8; LINUX_STAT_SIZE]> {
    if linux_console_descriptor_open(descriptor) {
        return Some(linux_console_stat());
    }
    let device = linux_device_descriptor(descriptor);
    (device != 0).then(|| {
        if device & LINUX_DEVICE_DIRECTORY != 0 {
            linux_stat(LINUX_STAT_DIRECTORY_MODE, 2)
        } else {
            linux_console_stat()
        }
    })
}

fn linux_stat(mode: u32, inode: u64) -> [u8; LINUX_STAT_SIZE] {
    let mut stat = [0_u8; LINUX_STAT_SIZE];
    stat[8..16].copy_from_slice(&inode.to_ne_bytes());
    stat[16..24].copy_from_slice(&1_u64.to_ne_bytes());
    stat[24..28].copy_from_slice(&mode.to_ne_bytes());
    stat[56..64].copy_from_slice(&(PAGE_SIZE as u64).to_ne_bytes());
    stat
}

fn copy_linux_cpu_location(cpu_address: u64, node_address: u64) -> bool {
    if (cpu_address != 0 && !linux_writable_user_range_contains(cpu_address, size_of::<u32>()))
        || (node_address != 0
            && !linux_writable_user_range_contains(node_address, size_of::<u32>()))
    {
        return false;
    }
    (cpu_address == 0 || copy_to_linux_user(cpu_address, &0_u32.to_ne_bytes()))
        && (node_address == 0 || copy_to_linux_user(node_address, &0_u32.to_ne_bytes()))
}

fn linux_writable_user_range_contains(address: u64, length: usize) -> bool {
    let base = ELF_PROBE_DATA.load(Ordering::SeqCst);
    let size = ELF_PROBE_DATA_SIZE.load(Ordering::SeqCst);
    let protected_page = ELF_PROBE_READ_ONLY_DATA_PAGE.load(Ordering::SeqCst);
    if linux_range_contains(base, size, address, length)
        && (protected_page == 0
            || !linux_ranges_overlap(address, length, protected_page, PAGE_SIZE))
    {
        return true;
    }
    let stack_base = ELF_PROBE_STACK.load(Ordering::SeqCst) & !(PAGE_SIZE as u64 - 1);
    if linux_range_contains(stack_base, PAGE_SIZE as u64, address, length) {
        return true;
    }
    let mmap_base = ELF_PROBE_MMAP.load(Ordering::SeqCst);
    let mmap_pages = ELF_PROBE_MMAP_PAGES.load(Ordering::SeqCst);
    if mmap_pages != 0
        && ELF_PROBE_MMAP_WRITABLE.load(Ordering::SeqCst)
        && linux_range_contains(
            mmap_base,
            u64::from(mmap_pages) * PAGE_SIZE as u64,
            address,
            length,
        )
    {
        return true;
    }
    let brk = ELF_PROBE_BRK.load(Ordering::SeqCst);
    let heap_base = ELF_PROBE_BRK_LIMIT
        .load(Ordering::SeqCst)
        .saturating_sub(PAGE_SIZE as u64);
    linux_range_contains(heap_base, brk.saturating_sub(heap_base), address, length)
}

fn linux_ranges_overlap(
    first: u64,
    first_length: usize,
    second: u64,
    second_length: usize,
) -> bool {
    let Some(first_end) = first.checked_add(first_length as u64) else {
        return true;
    };
    let Some(second_end) = second.checked_add(second_length as u64) else {
        return true;
    };
    first < second_end && second < first_end
}

fn linux_range_contains(base: u64, size: u64, address: u64, length: usize) -> bool {
    length != 0
        && address >= base
        && address
            .checked_add(length as u64)
            .is_some_and(|end| end <= base.saturating_add(size))
}

fn linux_readable_user_range_contains(address: u64, length: usize) -> bool {
    if linux_writable_user_range_contains(address, length) {
        return true;
    }
    let mmap_pages = ELF_PROBE_MMAP_PAGES.load(Ordering::SeqCst);
    mmap_pages != 0
        && linux_range_contains(
            ELF_PROBE_MMAP.load(Ordering::SeqCst),
            u64::from(mmap_pages) * PAGE_SIZE as u64,
            address,
            length,
        )
}

pub(crate) fn verify_linux_copy_bounds(base: u64, size: usize) -> bool {
    size >= LINUX_UTSNAME_SIZE
        && linux_range_contains(base, size as u64, base, LINUX_UTSNAME_SIZE)
        && linux_range_contains(
            base,
            size as u64,
            base + (size - LINUX_UTSNAME_SIZE) as u64,
            LINUX_UTSNAME_SIZE,
        )
        && !linux_range_contains(
            base,
            size as u64,
            base + (size - LINUX_UTSNAME_SIZE + 1) as u64,
            LINUX_UTSNAME_SIZE,
        )
        && !linux_range_contains(base, size as u64, base + size as u64, 1)
        && !linux_range_contains(base, size as u64, u64::MAX - 3, 8)
}

fn copy_from_linux_user(address: u64, output: &mut [u8]) -> bool {
    let code_base = ELF_PROBE_CODE_BASE.load(Ordering::SeqCst);
    let code_size = ELF_PROBE_CODE_SIZE.load(Ordering::SeqCst);
    let data_base = ELF_PROBE_DATA.load(Ordering::SeqCst);
    let data_size = ELF_PROBE_DATA_SIZE.load(Ordering::SeqCst);
    if !linux_range_contains(code_base, code_size, address, output.len())
        && !linux_range_contains(data_base, data_size, address, output.len())
        && !linux_readable_user_range_contains(address, output.len())
    {
        return false;
    }
    // SAFETY: the readable range is contained by one active probe VMA, which
    // remains mapped in the current root for the whole call.
    unsafe {
        with_supervisor_user_access(|| {
            for (offset, byte) in output.iter_mut().enumerate() {
                *byte = (address as *const u8).add(offset).read_volatile();
            }
        });
    }
    true
}

fn copy_linux_c_string(address: u64, output: &mut [u8]) -> Result<usize, u64> {
    for index in 0..output.len() {
        let Some(source) = address.checked_add(index as u64) else {
            return Err(LINUX_EFAULT);
        };
        if !copy_from_linux_user(source, &mut output[index..=index]) {
            return Err(LINUX_EFAULT);
        }
        if output[index] == 0 {
            return Ok(index);
        }
    }
    Err(LINUX_ENAMETOOLONG)
}

fn copy_linux_root_id_triple(first: u64, second: u64, third: u64) -> bool {
    let width = size_of::<u32>();
    if !linux_writable_user_range_contains(first, width)
        || !linux_writable_user_range_contains(second, width)
        || !linux_writable_user_range_contains(third, width)
    {
        return false;
    }
    let root = 0_u32.to_ne_bytes();
    copy_to_linux_user(first, &root)
        && copy_to_linux_user(second, &root)
        && copy_to_linux_user(third, &root)
}

fn linux_capget(header_address: u64, data_address: u64) -> u64 {
    let mut header = [0_u8; 8];
    if !copy_from_linux_user(header_address, &mut header) {
        return LINUX_EFAULT;
    }
    let version = u32::from_ne_bytes(header[..4].try_into().unwrap());
    if version != LINUX_CAPABILITY_VERSION_3 {
        return if copy_to_linux_user(header_address, &LINUX_CAPABILITY_VERSION_3.to_ne_bytes()) {
            LINUX_EINVAL
        } else {
            LINUX_EFAULT
        };
    }
    let pid = i32::from_ne_bytes(header[4..].try_into().unwrap());
    if !matches!(pid, 0 | 1) {
        return LINUX_ESRCH;
    }
    if data_address != 0 && !copy_to_linux_user(data_address, &[0_u8; LINUX_CAPABILITY_DATA_SIZE]) {
        return LINUX_EFAULT;
    }
    0
}

fn linux_capset(header_address: u64, data_address: u64) -> u64 {
    let mut header = [0_u8; 8];
    if !copy_from_linux_user(header_address, &mut header) {
        return LINUX_EFAULT;
    }
    if u32::from_ne_bytes(header[..4].try_into().unwrap()) != LINUX_CAPABILITY_VERSION_3 {
        return LINUX_EINVAL;
    }
    if i32::from_ne_bytes(header[4..].try_into().unwrap()) != 0 {
        return LINUX_EPERM;
    }
    let mut data = [0_u8; LINUX_CAPABILITY_DATA_SIZE];
    if !copy_from_linux_user(data_address, &mut data) {
        return LINUX_EFAULT;
    }
    if data.iter().any(|byte| *byte != 0) {
        LINUX_EPERM
    } else {
        0
    }
}

fn linux_set_scheduler(pid: u64, policy: u64, parameter_address: u64) -> u64 {
    if !matches!(pid, 0 | 1) {
        return LINUX_ESRCH;
    }
    if policy != 0 {
        return LINUX_EPERM;
    }
    let mut priority = [0_u8; 4];
    if !copy_from_linux_user(parameter_address, &mut priority) {
        return LINUX_EFAULT;
    }
    if priority == [0; 4] {
        0
    } else {
        LINUX_EINVAL
    }
}

fn linux_set_process_name(address: u64) -> u64 {
    let mut name = [0_u8; LINUX_PROCESS_NAME_SIZE];
    for index in 0..LINUX_PROCESS_NAME_SIZE - 1 {
        let Some(source) = address.checked_add(index as u64) else {
            return LINUX_EFAULT;
        };
        if !copy_from_linux_user(source, &mut name[index..=index]) {
            return LINUX_EFAULT;
        }
        if name[index] == 0 {
            break;
        }
    }
    let mut low = [0_u8; 8];
    let mut high = [0_u8; 8];
    low.copy_from_slice(&name[..8]);
    high.copy_from_slice(&name[8..]);
    ELF_PROBE_PROCESS_NAME_LOW.store(u64::from_ne_bytes(low), Ordering::SeqCst);
    ELF_PROBE_PROCESS_NAME_HIGH.store(u64::from_ne_bytes(high), Ordering::SeqCst);
    0
}

fn linux_get_process_name(address: u64) -> u64 {
    let mut name = [0_u8; LINUX_PROCESS_NAME_SIZE];
    name[..8].copy_from_slice(
        &ELF_PROBE_PROCESS_NAME_LOW
            .load(Ordering::SeqCst)
            .to_ne_bytes(),
    );
    name[8..].copy_from_slice(
        &ELF_PROBE_PROCESS_NAME_HIGH
            .load(Ordering::SeqCst)
            .to_ne_bytes(),
    );
    if copy_to_linux_user(address, &name) {
        0
    } else {
        LINUX_EFAULT
    }
}

fn linux_rt_sigaction(signal: u64, action: u64, old_action: u64, set_size: u64) -> u64 {
    if set_size != LINUX_SIGNAL_SET_SIZE || !(1..=LINUX_SIGNAL_COUNT as u64).contains(&signal) {
        return LINUX_EINVAL;
    }
    if action != 0 && matches!(signal, 9 | 19) {
        return LINUX_EINVAL;
    }
    if old_action != 0 && !linux_writable_user_range_contains(old_action, LINUX_SIGNAL_ACTION_SIZE)
    {
        return LINUX_EFAULT;
    }
    let parse_word = |bytes: &[u8; LINUX_SIGNAL_ACTION_SIZE], offset: usize| {
        let mut word = [0_u8; 8];
        word.copy_from_slice(&bytes[offset..offset + 8]);
        u64::from_ne_bytes(word)
    };
    let new_action = if action == 0 {
        None
    } else {
        let mut bytes = [0_u8; LINUX_SIGNAL_ACTION_SIZE];
        if !copy_from_linux_user(action, &mut bytes) {
            return LINUX_EFAULT;
        }
        Some([
            parse_word(&bytes, 0),
            parse_word(&bytes, 8),
            parse_word(&bytes, 16),
            parse_word(&bytes, 24) & !LINUX_UNBLOCKABLE_SIGNALS,
        ])
    };
    let index = signal as usize - 1;
    let current = [
        ELF_PROBE_SIGNAL_HANDLERS[index].load(Ordering::SeqCst),
        ELF_PROBE_SIGNAL_FLAGS[index].load(Ordering::SeqCst),
        ELF_PROBE_SIGNAL_RESTORERS[index].load(Ordering::SeqCst),
        ELF_PROBE_SIGNAL_ACTION_MASKS[index].load(Ordering::SeqCst),
    ];
    if old_action != 0 {
        let mut bytes = [0_u8; LINUX_SIGNAL_ACTION_SIZE];
        for (offset, word) in current.into_iter().enumerate() {
            bytes[offset * 8..offset * 8 + 8].copy_from_slice(&word.to_ne_bytes());
        }
        if !copy_to_linux_user(old_action, &bytes) {
            return LINUX_EFAULT;
        }
    }
    if let Some([handler, flags, restorer, mask]) = new_action {
        ELF_PROBE_SIGNAL_HANDLERS[index].store(handler, Ordering::SeqCst);
        ELF_PROBE_SIGNAL_FLAGS[index].store(flags, Ordering::SeqCst);
        ELF_PROBE_SIGNAL_RESTORERS[index].store(restorer, Ordering::SeqCst);
        ELF_PROBE_SIGNAL_ACTION_MASKS[index].store(mask, Ordering::SeqCst);
    }
    0
}

fn linux_sigaltstack(new_stack: u64, old_stack: u64) -> u64 {
    if old_stack != 0 && !linux_writable_user_range_contains(old_stack, LINUX_ALT_STACK_SIZE) {
        return LINUX_EFAULT;
    }
    let replacement = if new_stack == 0 {
        None
    } else {
        let mut bytes = [0_u8; LINUX_ALT_STACK_SIZE];
        if !copy_from_linux_user(new_stack, &mut bytes) {
            return LINUX_EFAULT;
        }
        let mut pointer_bytes = [0_u8; 8];
        let mut flags_bytes = [0_u8; 4];
        let mut size_bytes = [0_u8; 8];
        pointer_bytes.copy_from_slice(&bytes[..8]);
        flags_bytes.copy_from_slice(&bytes[8..12]);
        size_bytes.copy_from_slice(&bytes[16..24]);
        let pointer = u64::from_ne_bytes(pointer_bytes);
        let flags = u32::from_ne_bytes(flags_bytes);
        let size = u64::from_ne_bytes(size_bytes);
        match flags {
            LINUX_SS_DISABLE => Some((0, LINUX_SS_DISABLE, 0)),
            0 if size < LINUX_MIN_ALT_STACK_SIZE => return LINUX_ENOMEM,
            0 => {
                let Ok(length) = usize::try_from(size) else {
                    return LINUX_EFAULT;
                };
                if !linux_writable_user_range_contains(pointer, length) {
                    return LINUX_EFAULT;
                }
                Some((pointer, 0, size))
            }
            _ => return LINUX_EINVAL,
        }
    };
    if old_stack != 0 {
        let mut bytes = [0_u8; LINUX_ALT_STACK_SIZE];
        bytes[..8].copy_from_slice(
            &ELF_PROBE_ALT_STACK_POINTER
                .load(Ordering::SeqCst)
                .to_ne_bytes(),
        );
        bytes[8..12].copy_from_slice(
            &ELF_PROBE_ALT_STACK_FLAGS
                .load(Ordering::SeqCst)
                .to_ne_bytes(),
        );
        bytes[16..24].copy_from_slice(
            &ELF_PROBE_ALT_STACK_SIZE
                .load(Ordering::SeqCst)
                .to_ne_bytes(),
        );
        if !copy_to_linux_user(old_stack, &bytes) {
            return LINUX_EFAULT;
        }
    }
    if let Some((pointer, flags, size)) = replacement {
        ELF_PROBE_ALT_STACK_POINTER.store(pointer, Ordering::SeqCst);
        ELF_PROBE_ALT_STACK_FLAGS.store(flags, Ordering::SeqCst);
        ELF_PROBE_ALT_STACK_SIZE.store(size, Ordering::SeqCst);
    }
    0
}

fn linux_rt_sigprocmask(how: u64, set: u64, old_set: u64, set_size: u64) -> u64 {
    if set_size != LINUX_SIGNAL_SET_SIZE {
        return LINUX_EINVAL;
    }
    if old_set != 0 && !linux_writable_user_range_contains(old_set, LINUX_SIGNAL_SET_SIZE as usize)
    {
        return LINUX_EFAULT;
    }
    let requested = if set == 0 {
        None
    } else {
        let mut bytes = [0_u8; LINUX_SIGNAL_SET_SIZE as usize];
        if !copy_from_linux_user(set, &mut bytes) {
            return LINUX_EFAULT;
        }
        Some(u64::from_ne_bytes(bytes) & !LINUX_UNBLOCKABLE_SIGNALS)
    };
    let current = ELF_PROBE_SIGNAL_MASK.load(Ordering::SeqCst);
    let next = match requested {
        None => current,
        Some(mask) if how == LINUX_SIG_BLOCK => current | mask,
        Some(mask) if how == LINUX_SIG_UNBLOCK => current & !mask,
        Some(mask) if how == LINUX_SIG_SETMASK => mask,
        Some(_) => return LINUX_EINVAL,
    };
    if old_set != 0 && !copy_to_linux_user(old_set, &current.to_ne_bytes()) {
        return LINUX_EFAULT;
    }
    ELF_PROBE_SIGNAL_MASK.store(next, Ordering::SeqCst);
    0
}

fn copy_to_linux_user(address: u64, input: &[u8]) -> bool {
    if !linux_writable_user_range_contains(address, input.len()) {
        return false;
    }
    // SAFETY: the range is contained by one writable active probe VMA.
    unsafe {
        with_supervisor_user_access(|| {
            for (offset, byte) in input.iter().enumerate() {
                (address as *mut u8).add(offset).write_volatile(*byte);
            }
        });
    }
    true
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
        15 => create_auxiliary_service(caller_domain, capability, frame),
        16 => network_device_call(caller_domain, capability, frame),
        _ => u64::MAX,
    }
}

fn network_device_call(caller_domain: u64, capability: u64, frame: *const UserTrapFrame) -> u64 {
    let Some(frame) = (unsafe { frame.as_ref() }) else {
        return u64::MAX - 2;
    };
    let command = frame.first_argument();
    let argument = frame.second_argument();
    match command {
        control_abi::NETWORK_DEVICE_PROBE
            if argument == 0
                && domain::handle_permits(
                    caller_domain,
                    capability,
                    control_abi::NETWORK_DEVICE_OBJECT,
                    Rights::READ,
                ) =>
        {
            control_abi::NETWORK_DEVICE_DESCRIPTOR_WORD
        }
        control_abi::NETWORK_DEVICE_QUERY_LINK
            if argument == 0
                && domain::handle_permits(
                    caller_domain,
                    capability,
                    control_abi::NETWORK_DEVICE_OBJECT,
                    Rights::READ,
                ) =>
        {
            u64::from(network::link_is_up())
        }
        control_abi::NETWORK_DEVICE_SET_LINK
            if domain::handle_permits(
                caller_domain,
                capability,
                control_abi::NETWORK_DEVICE_OBJECT,
                Rights::WRITE,
            ) =>
        {
            network_result(network::set_link(argument))
        }
        control_abi::NETWORK_DEVICE_SET_IPV4
            if domain::handle_permits(
                caller_domain,
                capability,
                control_abi::NETWORK_DEVICE_OBJECT,
                Rights::WRITE,
            ) =>
        {
            network_result(network::set_ipv4(argument))
        }
        control_abi::NETWORK_DEVICE_QUERY_IPV4
            if argument == 0
                && domain::handle_permits(
                    caller_domain,
                    capability,
                    control_abi::NETWORK_DEVICE_OBJECT,
                    Rights::READ,
                ) =>
        {
            network::ipv4_configuration()
        }
        control_abi::NETWORK_DEVICE_SET_GATEWAY
            if domain::handle_permits(
                caller_domain,
                capability,
                control_abi::NETWORK_DEVICE_OBJECT,
                Rights::WRITE,
            ) =>
        {
            network_result(network::set_gateway(argument))
        }
        control_abi::NETWORK_DEVICE_QUERY_GATEWAY
            if argument == 0
                && domain::handle_permits(
                    caller_domain,
                    capability,
                    control_abi::NETWORK_DEVICE_OBJECT,
                    Rights::READ,
                ) =>
        {
            network::ipv4_gateway()
        }
        control_abi::NETWORK_DEVICE_QUERY_GENERATION
            if argument == 0
                && domain::handle_permits(
                    caller_domain,
                    capability,
                    control_abi::NETWORK_DEVICE_OBJECT,
                    Rights::READ,
                ) =>
        {
            network::generation()
        }
        control_abi::NETWORK_DEVICE_SET_ROUTE
            if domain::handle_permits(
                caller_domain,
                capability,
                control_abi::NETWORK_DEVICE_OBJECT,
                Rights::WRITE,
            ) =>
        {
            set_network_route(argument)
        }
        control_abi::NETWORK_DEVICE_QUERY_ROUTE
            if domain::handle_permits(
                caller_domain,
                capability,
                control_abi::NETWORK_DEVICE_OBJECT,
                Rights::READ,
            ) =>
        {
            query_network_route(argument)
        }
        control_abi::NETWORK_DEVICE_QUERY_SNAPSHOT
            if domain::handle_permits(
                caller_domain,
                capability,
                control_abi::NETWORK_DEVICE_OBJECT,
                Rights::READ,
            ) =>
        {
            query_network_snapshot(argument)
        }
        control_abi::NETWORK_DEVICE_APPLY
            if domain::handle_permits(
                caller_domain,
                capability,
                control_abi::NETWORK_DEVICE_OBJECT,
                Rights::WRITE,
            ) =>
        {
            apply_network_configuration(argument)
        }
        _ => u64::MAX - 2,
    }
}

fn apply_network_configuration(address: u64) -> u64 {
    let mut request = [0_u8; control_abi::NETWORK_APPLY_REQUEST_SIZE];
    if !address_space::copy_from_current_user(address, &mut request) {
        return u64::MAX - 2;
    }
    let version = network_request_word(&request, 0);
    let expected_generation = network_request_word(&request, 8);
    let link = network_request_word(&request, 16);
    let ipv4 = network_request_word(&request, 24);
    let gateway = network_request_word(&request, 32);
    let route_destination = network_request_word(&request, 40);
    let route_gateway = network_request_word(&request, 48);
    let reserved = network_request_word(&request, 56);
    if version != control_abi::NETWORK_APPLY_VERSION || reserved != 0 {
        return u64::MAX - 2;
    }
    match network::apply(
        expected_generation,
        network::Configuration {
            link,
            ipv4,
            gateway,
            route_destination,
            route_gateway,
        },
    ) {
        Ok(generation) => generation,
        Err(network::ApplyError::Stale) => control_abi::NETWORK_RESULT_STALE_GENERATION,
        Err(network::ApplyError::Invalid) => u64::MAX - 2,
    }
}

fn network_result(success: bool) -> u64 {
    if success {
        0
    } else {
        u64::MAX - 2
    }
}

fn network_request_word(
    request: &[u8; control_abi::NETWORK_APPLY_REQUEST_SIZE],
    offset: usize,
) -> u64 {
    u64::from_le_bytes([
        request[offset],
        request[offset + 1],
        request[offset + 2],
        request[offset + 3],
        request[offset + 4],
        request[offset + 5],
        request[offset + 6],
        request[offset + 7],
    ])
}

fn set_network_route(address: u64) -> u64 {
    let mut record = [0_u8; control_abi::NETWORK_ROUTE_RECORD_SIZE];
    if !address_space::copy_from_current_user(address, &mut record)
        || record[5..8].iter().any(|byte| *byte != 0)
        || record[12..].iter().any(|byte| *byte != 0)
    {
        return u64::MAX - 2;
    }
    let destination = u32::from_le_bytes([record[0], record[1], record[2], record[3]]);
    let prefix = record[4];
    let gateway = u32::from_le_bytes([record[8], record[9], record[10], record[11]]);
    network_result(network::set_route(destination, prefix, gateway))
}

fn query_network_route(address: u64) -> u64 {
    let (packed_destination, gateway) = network::route();
    let mut record = [0_u8; control_abi::NETWORK_ROUTE_RECORD_SIZE];
    record[0..4].copy_from_slice(&(packed_destination as u32).to_le_bytes());
    record[4] = (packed_destination >> 32) as u8;
    record[8..12].copy_from_slice(&(gateway as u32).to_le_bytes());
    if !address_space::copy_to_current_user(address, &record) {
        return u64::MAX - 2;
    }
    control_abi::NETWORK_ROUTE_RECORD_SIZE as u64
}

fn query_network_snapshot(address: u64) -> u64 {
    let state = network::snapshot();
    let words = [
        control_abi::NETWORK_SNAPSHOT_VERSION,
        state[0],
        state[1],
        state[2],
        state[3],
        state[4],
        state[5],
    ];
    let mut snapshot = [0_u8; control_abi::NETWORK_SNAPSHOT_SIZE];
    for (index, word) in words.iter().enumerate() {
        let offset = index * core::mem::size_of::<u64>();
        snapshot[offset..offset + 8].copy_from_slice(&word.to_le_bytes());
    }
    if !address_space::copy_to_current_user(address, &snapshot) {
        return u64::MAX - 2;
    }
    control_abi::NETWORK_SNAPSHOT_SIZE as u64
}

fn create_auxiliary_service(
    caller_domain: u64,
    capability: u64,
    frame: *const UserTrapFrame,
) -> u64 {
    let Some(frame) = (unsafe { frame.as_ref() }) else {
        return u64::MAX - 2;
    };
    if !domain::handle_permits(caller_domain, capability, 1, Rights::MANAGE)
        || frame.second_argument() != control_abi::MANAGEMENT_REQUEST_SIZE as u64
    {
        return u64::MAX - 2;
    }
    let mut bytes = [0_u8; control_abi::MANAGEMENT_REQUEST_SIZE];
    if !address_space::copy_from_current_user(frame.first_argument(), &mut bytes) {
        return u64::MAX - 2;
    }
    let Some(request) = control_abi::parse_management_request(&bytes) else {
        return u64::MAX - 2;
    };
    match request.action {
        control_abi::MANAGEMENT_CREATE_AUXILIARY => domain::create_auxiliary_service(
            caller_domain,
            capability,
            request.source,
            request.object,
            request.rights,
        )
        .unwrap_or(u64::MAX - 2),
        control_abi::MANAGEMENT_STOP_AUXILIARY
            if request.object == 0
                && request.rights == 0
                && address_space::stop_auxiliary_from_current_user(
                    caller_domain,
                    capability,
                    request.source,
                ) =>
        {
            0
        }
        control_abi::MANAGEMENT_LOAD_AUXILIARY
            if address_space::load_and_queue_auxiliary_from_current_user(
                request.source,
                request.object,
                request.rights,
            ) =>
        {
            0
        }
        control_abi::MANAGEMENT_INITIALIZE_AUXILIARY
            if address_space::initialize_staged_auxiliary_from_current_user(
                request.source,
                request.object,
                request.rights,
            ) =>
        {
            0
        }
        control_abi::MANAGEMENT_START_AUXILIARY
            if request.object == 0
                && request.rights == 0
                && scheduler::start_staged_auxiliary(request.source) =>
        {
            0
        }
        control_abi::MANAGEMENT_QUERY_AUXILIARY
            if request.object == 0
                && request.rights == 0
                && domain::auxiliary_service_is_ready(request.source) =>
        {
            scheduler::auxiliary_runtime_state(request.source).unwrap_or(u64::MAX - 2)
        }
        control_abi::MANAGEMENT_QUERY_AUXILIARY_FLAGS
            if request.object == 0
                && request.rights == 0
                && domain::auxiliary_service_is_ready(request.source) =>
        {
            scheduler::auxiliary_runtime_flags(request.source).unwrap_or(u64::MAX - 2)
        }
        _ => u64::MAX - 2,
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
        if service_side
            && domain::service_failure_policy(domain_id)
                == Some(domain::ServiceFailurePolicy::RestartManaged)
            && !domain::record_service_ready(domain_id)
        {
            return u64::MAX;
        }
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
    let Some(descriptor) = domain::delivered_fault_descriptor() else {
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
    let failure_policy = domain::service_failure_policy(caller_domain);
    let auxiliary_exit_status = if failure_policy == Some(domain::ServiceFailurePolicy::StopOnly) {
        let Ok(status) = u8::try_from(frame.first_argument()) else {
            return u64::MAX - 2;
        };
        Some(status)
    } else {
        None
    };
    let service_exit = failure_policy == Some(domain::ServiceFailurePolicy::RestartManaged);
    let close_wait = if failure_policy == Some(domain::ServiceFailurePolicy::StopOnly) {
        None
    } else {
        Some(scheduler::WaitReason::IpcReceive {
            object: 3,
            service_side: !service_exit,
            peer_domain: caller_domain,
        })
    };
    let action = scheduler::exit_runtime(frame, USER_CODE_SELECTOR, USER_DATA_SELECTOR, close_wait);
    if matches!(action, RuntimeAction::Invalid) || !domain::detach_thread(caller_domain) {
        return u64::MAX;
    }
    if service_exit
        && (!ipc::retire_service(caller_domain) || !domain::retire_clean_service(caller_domain))
    {
        return u64::MAX;
    }
    if failure_policy == Some(domain::ServiceFailurePolicy::StopOnly)
        && (!domain::begin_auxiliary_runtime_exit(
            caller_domain,
            auxiliary_exit_status.unwrap_or(0),
        ) || !address_space::activate_kernel_root_for_auxiliary(caller_domain)
            || !address_space::release_auxiliary_address_space(caller_domain)
            || !domain::retire_stopped_auxiliary_service(caller_domain))
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
    let failure_policy = fault_domain.and_then(domain::service_failure_policy);
    let service_fault = failure_policy == Some(domain::ServiceFailurePolicy::RestartManaged);
    let fault_address = if frame.is_page_fault() {
        let address: u64;
        unsafe {
            asm!("mov {}, cr2", out(reg) address, options(nomem, nostack, preserves_flags));
        }
        address
    } else {
        0
    };
    let fault_metadata = frame.metadata(fault_address);
    let action = scheduler::fault_runtime(
        frame,
        fault_address,
        USER_CODE_SELECTOR,
        USER_DATA_SELECTOR,
        fault_domain.and_then(|dead_domain| {
            (failure_policy != Some(domain::ServiceFailurePolicy::StopOnly)).then_some(
                scheduler::WaitReason::IpcReceive {
                    object: 3,
                    service_side: !service_fault,
                    peer_domain: dead_domain,
                },
            )
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
    if failure_policy == Some(domain::ServiceFailurePolicy::StopOnly)
        && (!domain::begin_auxiliary_runtime_fault(fault_domain, fault_metadata)
            || !address_space::activate_kernel_root_for_auxiliary(fault_domain)
            || !address_space::release_auxiliary_address_space(fault_domain)
            || !domain::retire_stopped_auxiliary_service(fault_domain))
    {
        return u64::MAX;
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
