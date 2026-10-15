use core::arch::{
    asm, global_asm,
    x86_64::{__cpuid, __cpuid_count},
};
use core::mem::size_of;
use core::sync::atomic::{AtomicBool, AtomicU16, AtomicU32, AtomicU64, AtomicU8, Ordering};

use super::{paging, timer};

use crate::{
    address_space,
    capability::Rights,
    control_abi, domain, image_store, initramfs, ipc, linux_mount, linux_process, linux_tmpfs,
    linux_user, memory, network, scheduler,
    scheduler::{
        RuntimeAction, UserFaultContext, UserFaultFrame, UserInterruptFrame, UserTrapFrame,
    },
    serial_write, serial_write_u64, volume,
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
pub(crate) const LINUX_MMAP_SLOT_COUNT: usize = 4;
const LINUX_RUNTIME_MMAP_SLOT_COUNT: usize = 32;
pub(crate) const LINUX_RUNTIME_STACK_PAGES: usize = 8;
pub(crate) const LINUX_RUNTIME_HEAP_PAGES: usize = 16;
const EXCEPTION_STACK_SIZE: usize = 16 * 1024;
const EXCEPTION_STACK_STORAGE_SIZE: usize = PAGE_SIZE + EXCEPTION_STACK_SIZE + PAGE_SIZE;
const EXCEPTION_STACK_TOP_OFFSET: usize = PAGE_SIZE + EXCEPTION_STACK_SIZE;
const CPUID_STRUCTURED_FEATURES: u32 = 7;
const CPUID_EXTENDED_STATE: u32 = 0x0d;
const CPUID_EXTENDED_MAX: u32 = 0x8000_0000;
const CPUID_EXTENDED_FEATURES: u32 = 0x8000_0001;
const CPUID_NX: u32 = 1 << 20;
const CPUID_SMEP: u32 = 1 << 7;
const CPUID_SMAP: u32 = 1 << 20;
const CPUID_UMIP: u32 = 1 << 2;
const CPUID_XSAVE: u32 = 1 << 26;
const CPUID_AVX: u32 = 1 << 28;
const CPUID_SSE: u32 = 1 << 25;
const CR0_MP: u64 = 1 << 1;
const CR0_EM: u64 = 1 << 2;
const CR0_NE: u64 = 1 << 5;
const CR4_OSFXSR: u64 = 1 << 9;
const CR4_OSXMMEXCPT: u64 = 1 << 10;
const CR4_OSXSAVE: u64 = 1 << 18;
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
const LINUX_OPEN_SYSCALL: u64 = 2;
const LINUX_CLOSE_SYSCALL: u64 = 3;
const LINUX_STAT_SYSCALL: u64 = 4;
const LINUX_FSTAT_SYSCALL: u64 = 5;
const LINUX_LSTAT_SYSCALL: u64 = 6;
const LINUX_POLL_SYSCALL: u64 = 7;
const LINUX_LSEEK_SYSCALL: u64 = 8;
const LINUX_MMAP_SYSCALL: u64 = 9;
const LINUX_MPROTECT_SYSCALL: u64 = 10;
const LINUX_MUNMAP_SYSCALL: u64 = 11;
const LINUX_BRK_SYSCALL: u64 = 12;
const LINUX_RT_SIGACTION_SYSCALL: u64 = 13;
const LINUX_RT_SIGPROCMASK_SYSCALL: u64 = 14;
const LINUX_RT_SIGRETURN_SYSCALL: u64 = 15;
const LINUX_RT_SIGPENDING_SYSCALL: u64 = 127;
const LINUX_RT_SIGTIMEDWAIT_SYSCALL: u64 = 128;
const LINUX_IOCTL_SYSCALL: u64 = 16;
const LINUX_PREAD64_SYSCALL: u64 = 17;
const LINUX_READV_SYSCALL: u64 = 19;
const LINUX_WRITEV_SYSCALL: u64 = 20;
const LINUX_ACCESS_SYSCALL: u64 = 21;
const LINUX_PIPE_SYSCALL: u64 = 22;
const LINUX_SCHED_YIELD_SYSCALL: u64 = 24;
const LINUX_MINCORE_SYSCALL: u64 = 27;
const LINUX_MADVISE_SYSCALL: u64 = 28;
const LINUX_DUP_SYSCALL: u64 = 32;
const LINUX_DUP2_SYSCALL: u64 = 33;
const LINUX_NANOSLEEP_SYSCALL: u64 = 35;
const LINUX_GETITIMER_SYSCALL: u64 = 36;
const LINUX_SETITIMER_SYSCALL: u64 = 38;
const LINUX_GETPID_SYSCALL: u64 = 39;
const LINUX_CLONE_SYSCALL: u64 = 56;
const LINUX_FORK_SYSCALL: u64 = 57;
const LINUX_VFORK_SYSCALL: u64 = 58;
const LINUX_EXECVE_SYSCALL: u64 = 59;
const LINUX_WAIT4_SYSCALL: u64 = 61;
const LINUX_KILL_SYSCALL: u64 = 62;
const LINUX_TKILL_SYSCALL: u64 = 200;
const LINUX_TGKILL_SYSCALL: u64 = 234;
const LINUX_CLONE_VM: u64 = 0x0000_0100;
const LINUX_CLONE_FS: u64 = 0x0000_0200;
const LINUX_CLONE_FILES: u64 = 0x0000_0400;
const LINUX_CLONE_SIGHAND: u64 = 0x0000_0800;
const LINUX_CLONE_THREAD: u64 = 0x0001_0000;
const LINUX_CLONE_SYSVSEM: u64 = 0x0004_0000;
const LINUX_CLONE_SETTLS: u64 = 0x0008_0000;
const LINUX_CLONE_PARENT_SETTID: u64 = 0x0010_0000;
const LINUX_CLONE_CHILD_CLEARTID: u64 = 0x0020_0000;
const LINUX_CLONE_CHILD_SETTID: u64 = 0x0100_0000;
const LINUX_BOUNDED_THREAD_CLONE_FLAGS: u64 = LINUX_CLONE_VM
    | LINUX_CLONE_FS
    | LINUX_CLONE_FILES
    | LINUX_CLONE_SIGHAND
    | LINUX_CLONE_THREAD
    | LINUX_CLONE_SYSVSEM
    | LINUX_CLONE_SETTLS
    | LINUX_CLONE_PARENT_SETTID
    | LINUX_CLONE_CHILD_CLEARTID
    | LINUX_CLONE_CHILD_SETTID;
const LINUX_EXIT_SYSCALL: u64 = 60;
const LINUX_UNAME_SYSCALL: u64 = 63;
const LINUX_FCNTL_SYSCALL: u64 = 72;
const LINUX_GETCWD_SYSCALL: u64 = 79;
const LINUX_CHDIR_SYSCALL: u64 = 80;
const LINUX_FCHDIR_SYSCALL: u64 = 81;
const LINUX_READLINK_SYSCALL: u64 = 89;
const LINUX_UMASK_SYSCALL: u64 = 95;
const LINUX_GETRLIMIT_SYSCALL: u64 = 97;
const LINUX_GETRUSAGE_SYSCALL: u64 = 98;
const LINUX_TIMES_SYSCALL: u64 = 100;
const LINUX_GETUID_SYSCALL: u64 = 102;
const LINUX_GETGID_SYSCALL: u64 = 104;
const LINUX_SETUID_SYSCALL: u64 = 105;
const LINUX_SETGID_SYSCALL: u64 = 106;
const LINUX_GETEUID_SYSCALL: u64 = 107;
const LINUX_GETEGID_SYSCALL: u64 = 108;
const LINUX_GETPPID_SYSCALL: u64 = 110;
const LINUX_GETPGRP_SYSCALL: u64 = 111;
const LINUX_GETGROUPS_SYSCALL: u64 = 115;
const LINUX_SETGROUPS_SYSCALL: u64 = 116;
const LINUX_SETREUID_SYSCALL: u64 = 113;
const LINUX_SETREGID_SYSCALL: u64 = 114;
const LINUX_SETRESUID_SYSCALL: u64 = 117;
const LINUX_GETRESUID_SYSCALL: u64 = 118;
const LINUX_SETRESGID_SYSCALL: u64 = 119;
const LINUX_GETRESGID_SYSCALL: u64 = 120;
const LINUX_GETPGID_SYSCALL: u64 = 121;
const LINUX_SETFSUID_SYSCALL: u64 = 122;
const LINUX_SETFSGID_SYSCALL: u64 = 123;
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
const LINUX_PIVOT_ROOT_SYSCALL: u64 = 155;
const LINUX_CHROOT_SYSCALL: u64 = 161;
const LINUX_MOUNT_SYSCALL: u64 = 165;
const LINUX_UMOUNT2_SYSCALL: u64 = 166;
const LINUX_SWAPON_SYSCALL: u64 = 167;
const LINUX_SWAPOFF_SYSCALL: u64 = 168;
const LINUX_REBOOT_SYSCALL: u64 = 169;
const LINUX_SETHOSTNAME_SYSCALL: u64 = 170;
const LINUX_SETDOMAINNAME_SYSCALL: u64 = 171;
const LINUX_IOPL_SYSCALL: u64 = 172;
const LINUX_IOPERM_SYSCALL: u64 = 173;
const LINUX_INIT_MODULE_SYSCALL: u64 = 175;
const LINUX_DELETE_MODULE_SYSCALL: u64 = 176;
const LINUX_GETTID_SYSCALL: u64 = 186;
const LINUX_FUTEX_SYSCALL: u64 = 202;
const LINUX_SCHED_SETAFFINITY_SYSCALL: u64 = 203;
const LINUX_SCHED_GETAFFINITY_SYSCALL: u64 = 204;
const LINUX_GETDENTS64_SYSCALL: u64 = 217;
const LINUX_SET_TID_ADDRESS_SYSCALL: u64 = 218;
const LINUX_CLOCK_GETTIME_SYSCALL: u64 = 228;
const LINUX_CLOCK_GETRES_SYSCALL: u64 = 229;
const LINUX_CLOCK_NANOSLEEP_SYSCALL: u64 = 230;
const LINUX_KEXEC_LOAD_SYSCALL: u64 = 246;
const LINUX_EXIT_GROUP_SYSCALL: u64 = 231;
const LINUX_EPOLL_WAIT_SYSCALL: u64 = 232;
const LINUX_EPOLL_CTL_SYSCALL: u64 = 233;
const LINUX_OPENAT_SYSCALL: u64 = 257;
const LINUX_NEWFSTATAT_SYSCALL: u64 = 262;
const LINUX_UNLINKAT_SYSCALL: u64 = 263;
const LINUX_READLINKAT_SYSCALL: u64 = 267;
const LINUX_FACCESSAT_SYSCALL: u64 = 269;
const LINUX_PPOLL_SYSCALL: u64 = 271;
const LINUX_SET_ROBUST_LIST_SYSCALL: u64 = 273;
const LINUX_GET_ROBUST_LIST_SYSCALL: u64 = 274;
const LINUX_UNSHARE_SYSCALL: u64 = 272;
const LINUX_EPOLL_PWAIT_SYSCALL: u64 = 281;
const LINUX_TIMERFD_CREATE_SYSCALL: u64 = 283;
const LINUX_EVENTFD_SYSCALL: u64 = 284;
const LINUX_TIMERFD_SETTIME_SYSCALL: u64 = 286;
const LINUX_TIMERFD_GETTIME_SYSCALL: u64 = 287;
const LINUX_EVENTFD2_SYSCALL: u64 = 290;
const LINUX_EPOLL_CREATE1_SYSCALL: u64 = 291;
const LINUX_DUP3_SYSCALL: u64 = 292;
const LINUX_PIPE2_SYSCALL: u64 = 293;
const LINUX_PRLIMIT64_SYSCALL: u64 = 302;
const LINUX_GETCPU_SYSCALL: u64 = 309;
const LINUX_PERF_EVENT_OPEN_SYSCALL: u64 = 298;
const LINUX_SETNS_SYSCALL: u64 = 308;
const LINUX_FINIT_MODULE_SYSCALL: u64 = 313;
const LINUX_KEXEC_FILE_LOAD_SYSCALL: u64 = 320;
const LINUX_BPF_SYSCALL: u64 = 321;
const LINUX_EXECVEAT_SYSCALL: u64 = 322;
const LINUX_STATX_SYSCALL: u64 = 332;
const LINUX_FACCESSAT2_SYSCALL: u64 = 439;
const LINUX_EPOLL_PWAIT2_SYSCALL: u64 = 441;
const LINUX_ARCH_SET_FS: u64 = 0x1002;
const LINUX_ARCH_GET_FS: u64 = 0x1003;
const LINUX_PROBE_EXIT_STATUS: u64 = 42;
const LINUX_CAPABILITY_VERSION_3: u32 = 0x2008_0522;
const LINUX_CAPABILITY_DATA_SIZE: usize = 24;
const LINUX_RETURN_TO_KERNEL: u64 = (-4096_i64) as u64;
const LINUX_RETURN_FROM_SIGNAL: u64 = (-4095_i64) as u64;
const LINUX_EPERM: u64 = u64::MAX;
const LINUX_EINTR: u64 = u64::MAX - 3;
const LINUX_E2BIG: u64 = u64::MAX - 6;
const LINUX_ENOEXEC: u64 = u64::MAX - 7;
const LINUX_EBADF: u64 = u64::MAX - 8;
const LINUX_ENOENT: u64 = u64::MAX - 1;
const LINUX_ESRCH: u64 = u64::MAX - 2;
const LINUX_ECHILD: u64 = u64::MAX - 9;
const LINUX_EFAULT: u64 = u64::MAX - 13;
const LINUX_EAGAIN: u64 = u64::MAX - 10;
const LINUX_EPIPE: u64 = u64::MAX - 31;
const LINUX_EACCES: u64 = u64::MAX - 12;
const LINUX_EBUSY: u64 = u64::MAX - 15;
const LINUX_EEXIST: u64 = u64::MAX - 16;
const LINUX_ENODEV: u64 = u64::MAX - 18;
const LINUX_ENOTDIR: u64 = u64::MAX - 19;
const LINUX_EISDIR: u64 = u64::MAX - 20;
const LINUX_ENOMEM: u64 = u64::MAX - 11;
const LINUX_ENOSPC: u64 = u64::MAX - 27;
const LINUX_ESPIPE: u64 = u64::MAX - 28;
const LINUX_EMFILE: u64 = u64::MAX - 23;
const LINUX_EINVAL: u64 = u64::MAX - 21;
const LINUX_ENOTTY: u64 = u64::MAX - 24;
const LINUX_ERANGE: u64 = u64::MAX - 33;
const LINUX_ENAMETOOLONG: u64 = u64::MAX - 35;
const LINUX_ENOSYS: u64 = u64::MAX - 37;
const LINUX_ETIMEDOUT: u64 = u64::MAX - 109;
const LINUX_RLIMIT_STACK: u64 = 3;
const LINUX_RLIMIT_NOFILE: u64 = 7;
const LINUX_PROT_READ: u64 = 1;
const LINUX_PROT_WRITE: u64 = 2;
const LINUX_PROT_EXEC: u64 = 4;
const LINUX_MAP_PRIVATE: u64 = 2;
const LINUX_MAP_FIXED: u64 = 0x10;
const LINUX_MAP_ANONYMOUS: u64 = 0x20;
const LINUX_MAP_FIXED_NOREPLACE: u64 = 0x10_0000;
const LINUX_MAIN_MAPPING_COUNT: usize = 8;
const LINUX_DEPENDENCY_MAPPING_COUNT: usize = 8;
const LINUX_DEPENDENCY_COUNT: usize = 2;
const LINUX_DEPENDENCY_STATE_COUNT: usize = LINUX_DEPENDENCY_COUNT * LINUX_DEPENDENCY_MAPPING_COUNT;
const LINUX_MADV_DONTNEED: u64 = 4;
const LINUX_STAT_SIZE: usize = 144;
const LINUX_STAT_CHARACTER_DEVICE_MODE: u32 = 0o020600;
const LINUX_STAT_DIRECTORY_MODE: u32 = 0o040555;
const LINUX_STAT_REGULAR_MODE: u32 = 0o100555;
const LINUX_STAT_FIFO_MODE: u32 = 0o010600;
const LINUX_STAT_ANONYMOUS_MODE: u32 = 0o600;
const LINUX_STATX_BASIC_STATS: u32 = 0x7ff;
const LINUX_STATX_RESERVED: u32 = 0x8000_0000;
const LINUX_WRITE_LIMIT: usize = 256;
const LINUX_IOV_LIMIT: usize = 4;
const LINUX_POLLFD_LIMIT: usize = 4;
const LINUX_AUX_DESCRIPTOR_FIRST: u64 = 2;
const LINUX_AUX_DESCRIPTOR_LIMIT: u64 = 6;
const LINUX_DESCRIPTOR_TABLE_LIMIT: u64 = 16;
const LINUX_DEVICE_DESCRIPTOR_COUNT: usize =
    LINUX_DESCRIPTOR_TABLE_LIMIT as usize - LINUX_AUX_DESCRIPTOR_FIRST as usize;
const LINUX_POLLIN: u16 = 1;
const LINUX_POLLOUT: u16 = 4;
const LINUX_POLLERR: u16 = 8;
const LINUX_POLLHUP: u16 = 0x10;
const LINUX_POLLNVAL: u16 = 0x20;
const LINUX_EPOLL_CTL_ADD: u64 = 1;
const LINUX_EPOLL_CTL_DEL: u64 = 2;
const LINUX_EPOLL_CTL_MOD: u64 = 3;
const LINUX_EPOLL_SUPPORTED_EVENTS: u32 = (LINUX_POLLIN | LINUX_POLLOUT) as u32;
const LINUX_EFD_SEMAPHORE: u64 = 1;
const LINUX_EFD_NONBLOCK: u64 = 0x800;
const LINUX_TFD_TIMER_ABSTIME: u64 = 1;
const LINUX_TIMER_ABSTIME: u64 = 1;
const LINUX_TFD_NONBLOCK: u64 = 0x800;
const LINUX_PIPE_CAPACITY: usize = 256;
const LINUX_PIPE_READV_WAIT_FLAG: u64 = 1 << 63;
const LINUX_PIPE_WRITEV_WAIT_FLAG: u64 = 1 << 63;
const LINUX_AT_FDCWD: u32 = (-100_i32) as u32;
const LINUX_AT_SYMLINK_NOFOLLOW: u64 = 0x100;
const LINUX_AT_EMPTY_PATH: u64 = 0x1000;
const LINUX_TCGETS: u64 = 0x5401;
const LINUX_TIOCGWINSZ: u64 = 0x5413;
const LINUX_FIONREAD: u64 = 0x541b;
const LINUX_DESCRIPTOR_INPUT: u16 = 1;
const LINUX_DESCRIPTOR_OUTPUT: u16 = 2;
const LINUX_DEVICE_ZERO: u16 = 1 << 2;
const LINUX_DEVICE_CONSOLE: u16 = 1 << 3;
const LINUX_DEVICE_DIRECTORY: u16 = 1 << 4;
const LINUX_FILE_INIT: u16 = 1 << 5;
const LINUX_FILE_INTERPRETER: u16 = 1 << 6;
const LINUX_FILE_DEPENDENCY: u16 = 1 << 7;
const LINUX_FILE_AUXILIARY_DEPENDENCY: u16 = LINUX_FILE_INTERPRETER | LINUX_FILE_DEPENDENCY;
const LINUX_FILE_ROOT_HOSTNAME: u16 = LINUX_FILE_INIT | LINUX_FILE_INTERPRETER;
const LINUX_FILE_ROOT_GENERIC: u16 = LINUX_FILE_INIT | LINUX_FILE_DEPENDENCY;
const LINUX_ROOT_MEASURED_DIRECTORY: u16 = LINUX_DEVICE_DIRECTORY | LINUX_FILE_INIT;
const LINUX_ROOT_DIRECTORY: u16 = LINUX_DEVICE_DIRECTORY | LINUX_FILE_INTERPRETER;
const LINUX_FILE_MASK: u16 = LINUX_FILE_INIT | LINUX_FILE_INTERPRETER | LINUX_FILE_DEPENDENCY;
const LINUX_FILE_PROC_STATUS: u16 = LINUX_FILE_MASK;
const LINUX_PROC_PID1_DIRECTORY: u16 = LINUX_DEVICE_DIRECTORY | LINUX_FILE_PROC_STATUS;
const LINUX_PROC_PID2_DIRECTORY_NODE: u16 = 0xfa00;
const LINUX_PROC_PID2_DIRECTORY: u16 =
    LINUX_DEVICE_DIRECTORY | LINUX_FILE_PROC_STATUS | LINUX_PROC_PID2_DIRECTORY_NODE;
const LINUX_PROC_PID2_STATUS_NODE: u16 = 0xf900;
const LINUX_FILE_PROC_PID2_STATUS: u16 = LINUX_FILE_MASK | LINUX_PROC_PID2_STATUS_NODE;
const LINUX_PROC_CPUINFO_NODE: u16 = 0xf800;
const LINUX_FILE_PROC_CPUINFO: u16 = LINUX_FILE_MASK | LINUX_PROC_CPUINFO_NODE;
const LINUX_PROC_MEMINFO_NODE: u16 = 0xf700;
const LINUX_FILE_PROC_MEMINFO: u16 = LINUX_FILE_MASK | LINUX_PROC_MEMINFO_NODE;
const LINUX_PROC_STAT_NODE: u16 = 0xf600;
const LINUX_FILE_PROC_STAT: u16 = LINUX_FILE_MASK | LINUX_PROC_STAT_NODE;
const LINUX_PROC_UPTIME_NODE: u16 = 0xf500;
const LINUX_FILE_PROC_UPTIME: u16 = LINUX_FILE_MASK | LINUX_PROC_UPTIME_NODE;
const LINUX_PROC_LOADAVG_NODE: u16 = 0xf400;
const LINUX_FILE_PROC_LOADAVG: u16 = LINUX_FILE_MASK | LINUX_PROC_LOADAVG_NODE;
const LINUX_PROC_FILESYSTEMS_NODE: u16 = 0xf300;
const LINUX_FILE_PROC_FILESYSTEMS: u16 = LINUX_FILE_MASK | LINUX_PROC_FILESYSTEMS_NODE;
const LINUX_PROC_CMDLINE_NODE: u16 = 0xf200;
const LINUX_FILE_PROC_CMDLINE: u16 = LINUX_FILE_MASK | LINUX_PROC_CMDLINE_NODE;
const LINUX_PROC_PID1_STAT_NODE: u16 = 0xf100;
const LINUX_FILE_PROC_PID1_STAT: u16 = LINUX_FILE_MASK | LINUX_PROC_PID1_STAT_NODE;
const LINUX_PROC_PID1_STATM_NODE: u16 = 0xf000;
const LINUX_FILE_PROC_PID1_STATM: u16 = LINUX_FILE_MASK | LINUX_PROC_PID1_STATM_NODE;
const LINUX_PROC_PID2_STAT_NODE: u16 = 0xef00;
const LINUX_FILE_PROC_PID2_STAT: u16 = LINUX_FILE_MASK | LINUX_PROC_PID2_STAT_NODE;
const LINUX_PROC_PID2_STATM_NODE: u16 = 0xee00;
const LINUX_FILE_PROC_PID2_STATM: u16 = LINUX_FILE_MASK | LINUX_PROC_PID2_STATM_NODE;
const LINUX_ROOT_NODE_SHIFT: u32 = 8;
const LINUX_ROOT_NODE_MASK: u16 = 0xff00;
const LINUX_MOUNTINFO_FILE_NODE: u16 = 0xfe00;
const LINUX_FILE_PROC_MOUNTINFO: u16 = LINUX_FILE_MASK | LINUX_MOUNTINFO_FILE_NODE;
const LINUX_UID_MAP_FILE_NODE: u16 = 0xfd00;
const LINUX_FILE_PROC_UID_MAP: u16 = LINUX_FILE_MASK | LINUX_UID_MAP_FILE_NODE;
const LINUX_GID_MAP_FILE_NODE: u16 = 0xfc00;
const LINUX_FILE_PROC_GID_MAP: u16 = LINUX_FILE_MASK | LINUX_GID_MAP_FILE_NODE;
const LINUX_SETGROUPS_FILE_NODE: u16 = 0xfb00;
const LINUX_FILE_PROC_SETGROUPS: u16 = LINUX_FILE_MASK | LINUX_SETGROUPS_FILE_NODE;
const LINUX_TMP_FILE_NODE: u16 = 0xff00;
const LINUX_FILE_TMP_SCRATCH: u16 = LINUX_FILE_MASK | LINUX_TMP_FILE_NODE;
const LINUX_PROC_PID1_STATUS: &[u8] = b"Name:\tinit\nState:\tR (running)\nPid:\t1\nPPid:\t0\n";
const LINUX_PROC_PID2_RUNNING_STATUS: &[u8] =
    b"Name:\tinit\nState:\tR (running)\nPid:\t2\nPPid:\t1\n";
const LINUX_PROC_PID2_ZOMBIE_STATUS: &[u8] =
    b"Name:\tinit\nState:\tZ (zombie)\nPid:\t2\nPPid:\t1\n";
const LINUX_PROC_CPUINFO: &[u8] = b"processor\t: 0\nvendor_id\t: GenuineIntel\ncpu family\t: 6\nmodel\t\t: 158\nmodel name\t: Ginux virtual x86-64 CPU\nstepping\t: 10\ncpu MHz\t\t: 1000.000\ncache size\t: 16384 KB\nphysical id\t: 0\nsiblings\t: 1\ncore id\t\t: 0\ncpu cores\t: 1\nfpu\t\t: yes\ncpuid level\t: 13\nflags\t\t: fpu tsc msr pae cx8 apic sep mtrr pge cmov pat clflush mmx fxsr sse sse2 syscall nx lm constant_tsc rep_good nopl xtopology cpuid tsc_known_freq pni ssse3 cx16 sse4_1 sse4_2 x2apic popcnt aes xsave avx rdrand hypervisor\nbogomips\t: 2000.00\nclflush size\t: 64\ncache_alignment\t: 64\naddress sizes\t: 46 bits physical, 48 bits virtual\n\n";
const LINUX_PROC_MEMINFO: &[u8] = b"MemTotal:        1048576 kB\nMemFree:          786432 kB\nMemAvailable:     786432 kB\nBuffers:               0 kB\nCached:            65536 kB\nSwapCached:            0 kB\nActive:            32768 kB\nInactive:          32768 kB\nSwapTotal:             0 kB\nSwapFree:              0 kB\nCommitLimit:       524288 kB\nCommitted_AS:      131072 kB\nVmallocTotal:   13743895347199 kB\nHugePages_Total:       0\nHugePages_Free:        0\nHugepagesize:       2048 kB\nDirectMap4k:      1048576 kB\n";
const LINUX_PROC_STAT: &[u8] = b"cpu  1 0 1 100 0 0 0 0 0 0\ncpu0 1 0 1 100 0 0 0 0 0 0\nintr 0\nctxt 1\nbtime 0\nprocesses 1\nprocs_running 1\nprocs_blocked 0\nsoftirq 0 0 0 0 0 0 0 0 0 0 0\n";
const LINUX_PROC_UPTIME: &[u8] = b"1.00 0.00\n";
const LINUX_PROC_LOADAVG: &[u8] = b"0.00 0.00 0.00 1/1 1\n";
const LINUX_PROC_FILESYSTEMS: &[u8] =
    b"nodev\tproc\nnodev\ttmpfs\nnodev\tdevtmpfs\n\text2\n\text3\n\text4\n\txfs\n";
const LINUX_PROC_CMDLINE: &[u8] = b"ginux immutable=1\n";
const LINUX_PROC_PID1_STAT: &[u8] = b"1 (init) S 0 1 1 0 -1 4194560 0 0 0 0 0 0 0 0 20 0 1 0 1 1048576 256 0 0 0 0 0 0 0 0 0 0 0 0 0 17 0 0 0 0 0 0 0 0 0 0 0 0 0\n";
const LINUX_PROC_PID1_STATM: &[u8] = b"256 64 32 16 0 32 0\n";
const LINUX_PROC_PID2_RUNNING_STAT: &[u8] = b"2 (init) R 1 1 1 0 -1 4194560 0 0 0 0 0 0 0 0 20 0 1 0 1 1048576 256 0 0 0 0 0 0 0 0 0 0 0 0 0 17 0 0 0 0 0 0 0 0 0 0 0 0 0\n";
const LINUX_PROC_PID2_ZOMBIE_STAT: &[u8] = b"2 (init) Z 1 1 1 0 -1 4194560 0 0 0 0 0 0 0 0 20 0 1 0 1 0 0 0 0 0 0 0 0 0 0 0 0 0 0 17 0 0 0 0 0 0 0 0 0 0 0 0 0\n";
const LINUX_PROC_PID2_STATM: &[u8] = b"256 64 32 16 0 32 0\n";
pub(crate) const LINUX_EXEC_MAPPING_LIMIT: usize = 8;
pub(crate) const LINUX_ACTIVE_IMAGE_MAPPING_LIMIT: usize = 440;
pub(crate) const LINUX_PROCESS_MAPPING_LIMIT: usize = LINUX_ACTIVE_IMAGE_MAPPING_LIMIT
    + 1 // primary stack
    + 1 // heap
    + 2 // primary and sibling TLS
    + 1 // sibling stack
    + LINUX_MMAP_SLOT_COUNT;
const LINUX_EXEC_VECTOR_LIMIT: usize = 8;
const LINUX_EXEC_STRING_LIMIT: usize = 64;
const LINUX_EXEC_STACK_SIZE: usize = 512;
const LINUX_PLATFORM: &[u8] = b"x86_64\0";
const LINUX_F_DUPFD: u64 = 0;
const LINUX_F_GETFD: u64 = 1;
const LINUX_F_SETFD: u64 = 2;
const LINUX_F_GETFL: u64 = 3;
const LINUX_F_SETFL: u64 = 4;
const LINUX_F_DUPFD_CLOEXEC: u64 = 1030;
const LINUX_FD_CLOEXEC: u8 = 1;
const LINUX_O_CLOEXEC: u64 = 0x80000;
const LINUX_O_DIRECTORY: u64 = 0x10000;
const LINUX_O_CREAT: u64 = 0x40;
const LINUX_O_EXCL: u64 = 0x80;
const LINUX_O_NONBLOCK: u64 = 0x800;
const LINUX_O_LARGEFILE: u64 = 0x8000;
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
const LINUX_SS_AUTODISARM: u32 = 1 << 31;
const LINUX_UNBLOCKABLE_SIGNALS: u64 = (1 << (9 - 1)) | (1 << (19 - 1));
const LINUX_STANDARD_SIGNAL_COUNT: u64 = 31;
const LINUX_SIGSEGV: u64 = 11;
const LINUX_SIGPIPE: u64 = 13;
const LINUX_SIGTERM: u64 = 15;
const LINUX_SI_USER: i32 = 0;
const LINUX_SI_TKILL: i32 = -6;
const LINUX_SI_KERNEL: i32 = 0x80;
const LINUX_SEGV_MAPERR: i32 = 1;
const LINUX_SEGV_ACCERR: i32 = 2;
const LINUX_SA_SIGINFO: u64 = 0x0000_0004;
const LINUX_SA_RESTORER: u64 = 0x0400_0000;
const LINUX_SA_ONSTACK: u64 = 0x0800_0000;
const LINUX_SA_NODEFER: u64 = 0x4000_0000;
const LINUX_SA_RESETHAND: u64 = 0x8000_0000;
const LINUX_SIGNAL_FRAME_SIZE: usize = 440;
const LINUX_SIGNAL_FRAME_INFO_OFFSET: u64 = 312;
const LINUX_SIGNAL_FRAME_CONTEXT_OFFSET: u64 = 8;
const LINUX_SIGNAL_RED_ZONE_SIZE: u64 = 128;
const LINUX_UC_SIGCONTEXT_SS: u64 = 0x2;
const LINUX_UC_STRICT_RESTORE_SS: u64 = 0x4;
const LINUX_SIGNAL_RESTORABLE_RFLAGS: u64 = 0x0005_0dd5;
const LINUX_SS_ONSTACK: u32 = 1;
const LINUX_UTSNAME_SIZE: usize = 6 * 65;
const LINUX_ROBUST_LIST_HEAD_SIZE: usize = 24;
const LINUX_CLEAR_CHILD_TID_OFFSET: u64 = 400;
const LINUX_ROBUST_LIST_OFFSET: u64 = 432;
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

const LINUX_RUNTIME_HOST_INSTANCE: u8 = control_abi::CONTAINER_LIMIT as u8;
const LINUX_RUNTIME_INSTANCE_LIMIT: usize = control_abi::CONTAINER_LIMIT + 1;
static LINUX_EXECUTION_INSTANCE: AtomicU8 = AtomicU8::new(LINUX_RUNTIME_HOST_INSTANCE);

fn linux_execution_instance() -> usize {
    let instance = usize::from(LINUX_EXECUTION_INSTANCE.load(Ordering::Relaxed));
    if instance < LINUX_RUNTIME_INSTANCE_LIMIT {
        instance
    } else {
        usize::from(LINUX_RUNTIME_HOST_INSTANCE)
    }
}

macro_rules! banked_atomic_integer {
    ($name:ident, $atomic:ty, $value:ty) => {
        struct $name {
            values: [$atomic; LINUX_RUNTIME_INSTANCE_LIMIT],
            states: [AtomicU8; LINUX_RUNTIME_INSTANCE_LIMIT],
            initial: $value,
        }

        #[allow(dead_code)]
        impl $name {
            const fn new(value: $value) -> Self {
                Self {
                    values: [const { <$atomic>::new(0) }; LINUX_RUNTIME_INSTANCE_LIMIT],
                    states: [const { AtomicU8::new(0) }; LINUX_RUNTIME_INSTANCE_LIMIT],
                    initial: value,
                }
            }

            fn current(&self) -> &$atomic {
                let instance = linux_execution_instance();
                if self.states[instance]
                    .compare_exchange(0, 1, Ordering::Acquire, Ordering::Acquire)
                    .is_ok()
                {
                    self.values[instance].store(self.initial, Ordering::Relaxed);
                    self.states[instance].store(2, Ordering::Release);
                } else {
                    while self.states[instance].load(Ordering::Acquire) != 2 {
                        core::hint::spin_loop();
                    }
                }
                &self.values[instance]
            }

            fn load(&self, order: Ordering) -> $value {
                self.current().load(order)
            }

            fn store(&self, value: $value, order: Ordering) {
                self.current().store(value, order);
            }

            fn swap(&self, value: $value, order: Ordering) -> $value {
                self.current().swap(value, order)
            }

            fn fetch_add(&self, value: $value, order: Ordering) -> $value {
                self.current().fetch_add(value, order)
            }

            fn fetch_and(&self, value: $value, order: Ordering) -> $value {
                self.current().fetch_and(value, order)
            }

            fn fetch_or(&self, value: $value, order: Ordering) -> $value {
                self.current().fetch_or(value, order)
            }

            fn compare_exchange(
                &self,
                current: $value,
                new: $value,
                success: Ordering,
                failure: Ordering,
            ) -> Result<$value, $value> {
                self.current()
                    .compare_exchange(current, new, success, failure)
            }
        }
    };
}

struct BankedAtomicBool {
    values: [AtomicBool; LINUX_RUNTIME_INSTANCE_LIMIT],
    states: [AtomicU8; LINUX_RUNTIME_INSTANCE_LIMIT],
    initial: bool,
}

#[allow(dead_code)]
impl BankedAtomicBool {
    const fn new(value: bool) -> Self {
        Self {
            values: [const { AtomicBool::new(false) }; LINUX_RUNTIME_INSTANCE_LIMIT],
            states: [const { AtomicU8::new(0) }; LINUX_RUNTIME_INSTANCE_LIMIT],
            initial: value,
        }
    }

    fn current(&self) -> &AtomicBool {
        let instance = linux_execution_instance();
        if self.states[instance]
            .compare_exchange(0, 1, Ordering::Acquire, Ordering::Acquire)
            .is_ok()
        {
            self.values[instance].store(self.initial, Ordering::Relaxed);
            self.states[instance].store(2, Ordering::Release);
        } else {
            while self.states[instance].load(Ordering::Acquire) != 2 {
                core::hint::spin_loop();
            }
        }
        &self.values[instance]
    }

    fn load(&self, order: Ordering) -> bool {
        self.current().load(order)
    }

    fn store(&self, value: bool, order: Ordering) {
        self.current().store(value, order);
    }

    fn swap(&self, value: bool, order: Ordering) -> bool {
        self.current().swap(value, order)
    }

    fn fetch_and(&self, value: bool, order: Ordering) -> bool {
        self.current().fetch_and(value, order)
    }

    fn fetch_or(&self, value: bool, order: Ordering) -> bool {
        self.current().fetch_or(value, order)
    }

    fn compare_exchange(
        &self,
        current: bool,
        new: bool,
        success: Ordering,
        failure: Ordering,
    ) -> Result<bool, bool> {
        self.current()
            .compare_exchange(current, new, success, failure)
    }
}

banked_atomic_integer!(BankedAtomicU8, AtomicU8, u8);
banked_atomic_integer!(BankedAtomicU16, AtomicU16, u16);
banked_atomic_integer!(BankedAtomicU32, AtomicU32, u32);
banked_atomic_integer!(BankedAtomicU64, AtomicU64, u64);
static CONTROL_DISCOVERY_OVERRIDE_DOMAIN: AtomicU64 = AtomicU64::new(0);
static CONTROL_DISCOVERY_OVERRIDE_WORD: AtomicU64 = AtomicU64::new(0);
static CONTROL_DESCRIPTOR_OVERRIDE_DOMAIN: AtomicU64 = AtomicU64::new(0);
const PROCESS_CHILD_PROBE_IDLE: u8 = 0;
const PROCESS_CHILD_PROBE_EXPECT_CONTEXT: u8 = 1;
const PROCESS_CHILD_PROBE_EXPECT_EXIT: u8 = 2;
const PROCESS_CHILD_PROBE_COMPLETE: u8 = 3;
static PROCESS_CHILD_PROBE_STATE: AtomicU8 = AtomicU8::new(PROCESS_CHILD_PROBE_IDLE);
static PROCESS_CHILD_CONTEXT_VALID: AtomicBool = AtomicBool::new(false);
static PROCESS_CHILD_EXECUTION_DIAGNOSTIC: AtomicU8 = AtomicU8::new(0);
static PROCESS_CHILD_EXIT_STATUS: AtomicU8 = AtomicU8::new(0);
static PROCESS_CHILD_EXPECTED_REGISTERS: [AtomicU64; 15] = [const { AtomicU64::new(0) }; 15];
static PROCESS_CHILD_EXPECTED_FS_BASE: AtomicU64 = AtomicU64::new(0);
static ELF_PROBE_ACTIVE: BankedAtomicBool = BankedAtomicBool::new(false);
static LINUX_RUNTIME_MODE: BankedAtomicBool = BankedAtomicBool::new(false);
static ELF_PROBE_COMPLETED: BankedAtomicBool = BankedAtomicBool::new(false);
static ELF_PROBE_TERMINATION_SIGNAL: BankedAtomicU32 = BankedAtomicU32::new(0);
static ELF_PROBE_EXEC_REPLACED: BankedAtomicBool = BankedAtomicBool::new(false);
static ELF_PROBE_EXEC_NODE: BankedAtomicU8 = BankedAtomicU8::new(0);
static LINUX_LIVE_RETURN_VALIDATED: BankedAtomicBool = BankedAtomicBool::new(false);
static LINUX_CHILD_EXIT_COUNT: BankedAtomicU32 = BankedAtomicU32::new(0);
static LINUX_PARENT_RESUMED: BankedAtomicBool = BankedAtomicBool::new(false);
static LINUX_FORK_REQUESTED: BankedAtomicBool = BankedAtomicBool::new(false);
static LINUX_PROCESS_CHILD_EXITED: BankedAtomicBool = BankedAtomicBool::new(false);
static LINUX_PROCESS_CHILD_STATUS: BankedAtomicU8 = BankedAtomicU8::new(0);
static LINUX_CURRENT_PROCESS_PID: BankedAtomicU32 = BankedAtomicU32::new(1);
static LINUX_PROCESS_WAIT_COUNT: BankedAtomicU32 = BankedAtomicU32::new(0);
static LIVE_FORK_PUBLIC_DIAGNOSTIC: BankedAtomicU8 = BankedAtomicU8::new(0);
static LINUX_PROCESS_SNAPSHOT_VALIDATED: BankedAtomicBool = BankedAtomicBool::new(false);
static LINUX_PROCESS_SNAPSHOT_STAGE: BankedAtomicU8 = BankedAtomicU8::new(0);
static LINUX_CLONE_DISPATCH_COUNT: BankedAtomicU32 = BankedAtomicU32::new(0);
static LINUX_FIRST_CHILD_TID: BankedAtomicU32 = BankedAtomicU32::new(0);
static LINUX_CLONE_REUSE_VALIDATED: BankedAtomicBool = BankedAtomicBool::new(false);
static LINUX_CHILD_GETTID_COUNT: BankedAtomicU32 = BankedAtomicU32::new(0);
static LINUX_YIELD_SWITCH_COUNT: BankedAtomicU32 = BankedAtomicU32::new(0);
static LINUX_CLONE_BUSY_REJECTION_COUNT: BankedAtomicU32 = BankedAtomicU32::new(0);
static LINUX_CLONE_INVALID_REJECTION_COUNT: BankedAtomicU32 = BankedAtomicU32::new(0);
static LINUX_CLONE_TID_FAULT_COUNT: BankedAtomicU32 = BankedAtomicU32::new(0);
static LINUX_FUTEX_WAIT_BLOCK_COUNT: BankedAtomicU32 = BankedAtomicU32::new(0);
static LINUX_FUTEX_EXPLICIT_WAKE_COUNT: BankedAtomicU32 = BankedAtomicU32::new(0);
static LINUX_FUTEX_EXIT_WAKE_COUNT: BankedAtomicU32 = BankedAtomicU32::new(0);
static LINUX_FUTEX_WAIT_TIMEOUT_COUNT: BankedAtomicU32 = BankedAtomicU32::new(0);
static LINUX_SLEEP_BLOCK_COUNT: BankedAtomicU32 = BankedAtomicU32::new(0);
static LINUX_SLEEP_WAKE_COUNT: BankedAtomicU32 = BankedAtomicU32::new(0);
static LINUX_SLEEP_INTERRUPT_COUNT: BankedAtomicU32 = BankedAtomicU32::new(0);
static LINUX_TIMER_WAIT_EXPIRY_COUNT: BankedAtomicU32 = BankedAtomicU32::new(0);
static LINUX_TIMER_WAIT_PREEMPTION_COUNT: BankedAtomicU32 = BankedAtomicU32::new(0);
static LINUX_POLL_WAIT_BLOCK_COUNT: BankedAtomicU32 = BankedAtomicU32::new(0);
static LINUX_POLL_WAIT_WAKE_COUNT: BankedAtomicU32 = BankedAtomicU32::new(0);
static LINUX_POLL_WAIT_TIMEOUT_COUNT: BankedAtomicU32 = BankedAtomicU32::new(0);
static LINUX_POLL_WAIT_INTERRUPT_COUNT: BankedAtomicU32 = BankedAtomicU32::new(0);
static LINUX_EPOLL_WAIT_BLOCK_COUNT: BankedAtomicU32 = BankedAtomicU32::new(0);
static LINUX_EPOLL_WAIT_WAKE_COUNT: BankedAtomicU32 = BankedAtomicU32::new(0);
static LINUX_EPOLL_WAIT_TIMEOUT_COUNT: BankedAtomicU32 = BankedAtomicU32::new(0);
static LINUX_EPOLL_WAIT_INTERRUPT_COUNT: BankedAtomicU32 = BankedAtomicU32::new(0);
static LINUX_EVENTFD_READ_BLOCK_COUNT: BankedAtomicU32 = BankedAtomicU32::new(0);
static LINUX_EVENTFD_READ_WAKE_COUNT: BankedAtomicU32 = BankedAtomicU32::new(0);
static LINUX_EVENTFD_READ_INTERRUPT_COUNT: BankedAtomicU32 = BankedAtomicU32::new(0);
static LINUX_EVENTFD_WRITE_BLOCK_COUNT: BankedAtomicU32 = BankedAtomicU32::new(0);
static LINUX_EVENTFD_WRITE_WAKE_COUNT: BankedAtomicU32 = BankedAtomicU32::new(0);
static LINUX_EVENTFD_WRITE_INTERRUPT_COUNT: BankedAtomicU32 = BankedAtomicU32::new(0);
static LINUX_PIPE_READ_BLOCK_COUNT: BankedAtomicU32 = BankedAtomicU32::new(0);
static LINUX_PIPE_READ_WAKE_COUNT: BankedAtomicU32 = BankedAtomicU32::new(0);
static LINUX_PIPE_READ_INTERRUPT_COUNT: BankedAtomicU32 = BankedAtomicU32::new(0);
static LINUX_PIPE_WRITE_BLOCK_COUNT: BankedAtomicU32 = BankedAtomicU32::new(0);
static LINUX_PIPE_WRITE_WAKE_COUNT: BankedAtomicU32 = BankedAtomicU32::new(0);
static LINUX_PIPE_WRITE_INTERRUPT_COUNT: BankedAtomicU32 = BankedAtomicU32::new(0);
static LINUX_TIMERFD_READ_BLOCK_COUNT: BankedAtomicU32 = BankedAtomicU32::new(0);
static LINUX_TIMERFD_READ_WAKE_COUNT: BankedAtomicU32 = BankedAtomicU32::new(0);
static LINUX_TIMERFD_READ_INTERRUPT_COUNT: BankedAtomicU32 = BankedAtomicU32::new(0);
static LINUX_PPOLL_WAIT_BLOCK_COUNT: BankedAtomicU32 = BankedAtomicU32::new(0);
static LINUX_PPOLL_WAIT_WAKE_COUNT: BankedAtomicU32 = BankedAtomicU32::new(0);
static LINUX_PPOLL_WAIT_TIMEOUT_COUNT: BankedAtomicU32 = BankedAtomicU32::new(0);
static LINUX_EPOLL_PWAIT_BLOCK_COUNT: BankedAtomicU32 = BankedAtomicU32::new(0);
static LINUX_EPOLL_PWAIT_WAKE_COUNT: BankedAtomicU32 = BankedAtomicU32::new(0);
static LINUX_EPOLL_PWAIT_TIMEOUT_COUNT: BankedAtomicU32 = BankedAtomicU32::new(0);
static LINUX_MASKED_WAIT_INTERRUPT_COUNT: BankedAtomicU32 = BankedAtomicU32::new(0);
static LINUX_SIGNAL_WAIT_INTERRUPT_COUNT: BankedAtomicU32 = BankedAtomicU32::new(0);
static ELF_PROBE_STACK: BankedAtomicU64 = BankedAtomicU64::new(0);
static ELF_PROBE_STACK_FRAME: BankedAtomicU64 = BankedAtomicU64::new(0);
static ELF_PROBE_STACK_GENERATION: BankedAtomicU32 = BankedAtomicU32::new(0);
static ELF_PROBE_HEAP_FRAME: BankedAtomicU64 = BankedAtomicU64::new(0);
static ELF_PROBE_HEAP_GENERATION: BankedAtomicU32 = BankedAtomicU32::new(0);
static ELF_PROBE_RECLAIM_HEAP_FRAME: BankedAtomicU64 = BankedAtomicU64::new(0);
static ELF_PROBE_RECLAIM_HEAP_GENERATION: BankedAtomicU32 = BankedAtomicU32::new(0);
static ELF_PROBE_CODE_BASE: BankedAtomicU64 = BankedAtomicU64::new(0);
static ELF_PROBE_CODE_SIZE: BankedAtomicU64 = BankedAtomicU64::new(0);
static ELF_PROBE_LOADER_CODE_BASE: BankedAtomicU64 = BankedAtomicU64::new(0);
static ELF_PROBE_LOADER_CODE_SIZE: BankedAtomicU64 = BankedAtomicU64::new(0);
static ELF_PROBE_DATA: BankedAtomicU64 = BankedAtomicU64::new(0);
static ELF_PROBE_DATA_SIZE: BankedAtomicU64 = BankedAtomicU64::new(0);
static ELF_PROBE_EXEC_ENTRY: BankedAtomicU64 = BankedAtomicU64::new(0);
static ELF_PROBE_EXEC_MAPPING_ADDRESSES: [BankedAtomicU64; LINUX_EXEC_MAPPING_LIMIT] =
    [const { BankedAtomicU64::new(0) }; LINUX_EXEC_MAPPING_LIMIT];
static ELF_PROBE_EXEC_MAPPING_FRAMES: [BankedAtomicU64; LINUX_EXEC_MAPPING_LIMIT] =
    [const { BankedAtomicU64::new(0) }; LINUX_EXEC_MAPPING_LIMIT];
static ELF_PROBE_EXEC_MAPPING_GENERATIONS: [BankedAtomicU32; LINUX_EXEC_MAPPING_LIMIT] =
    [const { BankedAtomicU32::new(0) }; LINUX_EXEC_MAPPING_LIMIT];
static ELF_PROBE_EXEC_MAPPING_FLAGS: [BankedAtomicU8; LINUX_EXEC_MAPPING_LIMIT] =
    [const { BankedAtomicU8::new(0) }; LINUX_EXEC_MAPPING_LIMIT];
static ELF_PROBE_ACTIVE_IMAGE_ADDRESSES: [BankedAtomicU64; LINUX_ACTIVE_IMAGE_MAPPING_LIMIT] =
    [const { BankedAtomicU64::new(0) }; LINUX_ACTIVE_IMAGE_MAPPING_LIMIT];
static ELF_PROBE_ACTIVE_IMAGE_FRAMES: [BankedAtomicU64; LINUX_ACTIVE_IMAGE_MAPPING_LIMIT] =
    [const { BankedAtomicU64::new(0) }; LINUX_ACTIVE_IMAGE_MAPPING_LIMIT];
static ELF_PROBE_ACTIVE_IMAGE_GENERATIONS: [BankedAtomicU32; LINUX_ACTIVE_IMAGE_MAPPING_LIMIT] =
    [const { BankedAtomicU32::new(0) }; LINUX_ACTIVE_IMAGE_MAPPING_LIMIT];
static ELF_PROBE_ACTIVE_IMAGE_FLAGS: [BankedAtomicU8; LINUX_ACTIVE_IMAGE_MAPPING_LIMIT] =
    [const { BankedAtomicU8::new(0) }; LINUX_ACTIVE_IMAGE_MAPPING_LIMIT];
static ELF_PROBE_ACTIVE_IMAGE_COUNT: BankedAtomicU16 = BankedAtomicU16::new(0);
static ELF_PROBE_RETIRED_IMAGE_ADDRESSES: [BankedAtomicU64; LINUX_ACTIVE_IMAGE_MAPPING_LIMIT] =
    [const { BankedAtomicU64::new(0) }; LINUX_ACTIVE_IMAGE_MAPPING_LIMIT];
static ELF_PROBE_RETIRED_IMAGE_FRAMES: [BankedAtomicU64; LINUX_ACTIVE_IMAGE_MAPPING_LIMIT] =
    [const { BankedAtomicU64::new(0) }; LINUX_ACTIVE_IMAGE_MAPPING_LIMIT];
static ELF_PROBE_RETIRED_IMAGE_GENERATIONS: [BankedAtomicU32; LINUX_ACTIVE_IMAGE_MAPPING_LIMIT] =
    [const { BankedAtomicU32::new(0) }; LINUX_ACTIVE_IMAGE_MAPPING_LIMIT];
static ELF_PROBE_RETIRED_IMAGE_FLAGS: [BankedAtomicU8; LINUX_ACTIVE_IMAGE_MAPPING_LIMIT] =
    [const { BankedAtomicU8::new(0) }; LINUX_ACTIVE_IMAGE_MAPPING_LIMIT];
static ELF_PROBE_RETIRED_IMAGE_COUNT: BankedAtomicU8 = BankedAtomicU8::new(0);
static ELF_PROBE_TLS: BankedAtomicU64 = BankedAtomicU64::new(0);
static ELF_PROBE_TLS_SIZE: BankedAtomicU64 = BankedAtomicU64::new(0);
static ELF_PROBE_TLS_FRAMES: [BankedAtomicU64; 2] = [const { BankedAtomicU64::new(0) }; 2];
static ELF_PROBE_TLS_GENERATIONS: [BankedAtomicU32; 2] = [const { BankedAtomicU32::new(0) }; 2];
static ELF_PROBE_RECLAIM_TLS_FRAMES: [BankedAtomicU64; 2] = [const { BankedAtomicU64::new(0) }; 2];
static ELF_PROBE_RECLAIM_TLS_GENERATIONS: [BankedAtomicU32; 2] =
    [const { BankedAtomicU32::new(0) }; 2];
static ELF_PROBE_SIBLING_FS_BASE: BankedAtomicU64 = BankedAtomicU64::new(0);
static ELF_PROBE_SIBLING_TLS: BankedAtomicU64 = BankedAtomicU64::new(0);
static ELF_PROBE_SIBLING_STACK: BankedAtomicU64 = BankedAtomicU64::new(0);
static ELF_PROBE_SIBLING_STACK_FRAME: BankedAtomicU64 = BankedAtomicU64::new(0);
static ELF_PROBE_SIBLING_STACK_GENERATION: BankedAtomicU32 = BankedAtomicU32::new(0);
static ELF_PROBE_RECLAIM_SIBLING_STACK_FRAME: BankedAtomicU64 = BankedAtomicU64::new(0);
static ELF_PROBE_RECLAIM_SIBLING_STACK_GENERATION: BankedAtomicU32 = BankedAtomicU32::new(0);
const LINUX_THREAD_LIMIT: usize = 2;
const LINUX_THREAD_FREE: u8 = 0;
const LINUX_THREAD_READY: u8 = 1;
const LINUX_THREAD_RUNNING: u8 = 2;
const LINUX_THREAD_BLOCKED: u8 = 3;
const LINUX_THREAD_SLOT_NONE: u8 = u8::MAX;
const LINUX_THREAD_WAIT_NONE: u8 = 0;
const LINUX_THREAD_WAIT_FUTEX: u8 = 1;
const LINUX_THREAD_WAIT_SIGNAL: u8 = 2;
const LINUX_THREAD_WAIT_SLEEP: u8 = 3;
const LINUX_THREAD_WAIT_POLL: u8 = 4;
const LINUX_THREAD_WAIT_EPOLL: u8 = 5;
const LINUX_THREAD_WAIT_EVENTFD_READ: u8 = 6;
const LINUX_THREAD_WAIT_PPOLL: u8 = 7;
const LINUX_THREAD_WAIT_EPOLL_PWAIT: u8 = 8;
const LINUX_THREAD_WAIT_PIPE_READ: u8 = 9;
const LINUX_THREAD_WAIT_TIMERFD_READ: u8 = 10;
const LINUX_THREAD_WAIT_PIPE_WRITE: u8 = 11;
const LINUX_THREAD_WAIT_EVENTFD_WRITE: u8 = 12;
const LINUX_SYSCALL_FRAME_REGISTERS: usize = 12;
static LINUX_THREAD_STATES: [BankedAtomicU8; LINUX_THREAD_LIMIT] =
    [const { BankedAtomicU8::new(LINUX_THREAD_FREE) }; LINUX_THREAD_LIMIT];
static LINUX_THREAD_GENERATIONS: [BankedAtomicU32; LINUX_THREAD_LIMIT] =
    [const { BankedAtomicU32::new(0) }; LINUX_THREAD_LIMIT];
static LINUX_THREAD_TIDS: [BankedAtomicU32; LINUX_THREAD_LIMIT] =
    [const { BankedAtomicU32::new(0) }; LINUX_THREAD_LIMIT];
static LINUX_THREAD_POINTERS: [BankedAtomicU64; LINUX_THREAD_LIMIT] =
    [const { BankedAtomicU64::new(0) }; LINUX_THREAD_LIMIT];
static LINUX_THREAD_TLS_EXTENTS: [BankedAtomicU64; LINUX_THREAD_LIMIT] =
    [const { BankedAtomicU64::new(0) }; LINUX_THREAD_LIMIT];
static LINUX_THREAD_TLS_SIZES: [BankedAtomicU64; LINUX_THREAD_LIMIT] =
    [const { BankedAtomicU64::new(0) }; LINUX_THREAD_LIMIT];
static LINUX_THREAD_STACK_BASES: [BankedAtomicU64; LINUX_THREAD_LIMIT] =
    [const { BankedAtomicU64::new(0) }; LINUX_THREAD_LIMIT];
static LINUX_THREAD_STACK_SIZES: [BankedAtomicU64; LINUX_THREAD_LIMIT] =
    [const { BankedAtomicU64::new(0) }; LINUX_THREAD_LIMIT];
static LINUX_THREAD_RIPS: [BankedAtomicU64; LINUX_THREAD_LIMIT] =
    [const { BankedAtomicU64::new(0) }; LINUX_THREAD_LIMIT];
static LINUX_THREAD_RSPS: [BankedAtomicU64; LINUX_THREAD_LIMIT] =
    [const { BankedAtomicU64::new(0) }; LINUX_THREAD_LIMIT];
static LINUX_THREAD_RFLAGS: [BankedAtomicU64; LINUX_THREAD_LIMIT] =
    [const { BankedAtomicU64::new(0) }; LINUX_THREAD_LIMIT];
static LINUX_THREAD_RAXS: [BankedAtomicU64; LINUX_THREAD_LIMIT] =
    [const { BankedAtomicU64::new(0) }; LINUX_THREAD_LIMIT];
static LINUX_THREAD_RCXS: [BankedAtomicU64; LINUX_THREAD_LIMIT] =
    [const { BankedAtomicU64::new(0) }; LINUX_THREAD_LIMIT];
static LINUX_THREAD_R11S: [BankedAtomicU64; LINUX_THREAD_LIMIT] =
    [const { BankedAtomicU64::new(0) }; LINUX_THREAD_LIMIT];
static LINUX_THREAD_REGISTERS: [[BankedAtomicU64; LINUX_SYSCALL_FRAME_REGISTERS];
    LINUX_THREAD_LIMIT] = [const { [const { BankedAtomicU64::new(0) }; LINUX_SYSCALL_FRAME_REGISTERS] };
    LINUX_THREAD_LIMIT];
static LINUX_THREAD_CLEAR_CHILD_TIDS: [BankedAtomicU64; LINUX_THREAD_LIMIT] =
    [const { BankedAtomicU64::new(0) }; LINUX_THREAD_LIMIT];
static LINUX_THREAD_ROBUST_LISTS: [BankedAtomicU64; LINUX_THREAD_LIMIT] =
    [const { BankedAtomicU64::new(0) }; LINUX_THREAD_LIMIT];
static LINUX_THREAD_SIGNAL_MASKS: [BankedAtomicU64; LINUX_THREAD_LIMIT] =
    [const { BankedAtomicU64::new(0) }; LINUX_THREAD_LIMIT];
static LINUX_THREAD_WAIT_KINDS: [BankedAtomicU8; LINUX_THREAD_LIMIT] =
    [const { BankedAtomicU8::new(LINUX_THREAD_WAIT_NONE) }; LINUX_THREAD_LIMIT];
static LINUX_THREAD_WAIT_KEYS: [BankedAtomicU64; LINUX_THREAD_LIMIT] =
    [const { BankedAtomicU64::new(0) }; LINUX_THREAD_LIMIT];
static LINUX_THREAD_WAIT_OUTPUTS: [BankedAtomicU64; LINUX_THREAD_LIMIT] =
    [const { BankedAtomicU64::new(0) }; LINUX_THREAD_LIMIT];
static LINUX_THREAD_WAIT_DEADLINES: [BankedAtomicU64; LINUX_THREAD_LIMIT] =
    [const { BankedAtomicU64::new(0) }; LINUX_THREAD_LIMIT];
static LINUX_THREAD_WAIT_PRIOR_SIGNAL_MASKS: [BankedAtomicU64; LINUX_THREAD_LIMIT] =
    [const { BankedAtomicU64::new(0) }; LINUX_THREAD_LIMIT];
static LINUX_THREAD_POLL_DESCRIPTORS: [[BankedAtomicU32; LINUX_POLLFD_LIMIT]; LINUX_THREAD_LIMIT] =
    [const { [const { BankedAtomicU32::new(0) }; LINUX_POLLFD_LIMIT] }; LINUX_THREAD_LIMIT];
static LINUX_THREAD_POLL_EVENTS: [[BankedAtomicU16; LINUX_POLLFD_LIMIT]; LINUX_THREAD_LIMIT] =
    [const { [const { BankedAtomicU16::new(0) }; LINUX_POLLFD_LIMIT] }; LINUX_THREAD_LIMIT];
static LINUX_THREAD_IOV_ADDRESSES: [[BankedAtomicU64; LINUX_IOV_LIMIT]; LINUX_THREAD_LIMIT] =
    [const { [const { BankedAtomicU64::new(0) }; LINUX_IOV_LIMIT] }; LINUX_THREAD_LIMIT];
static LINUX_THREAD_IOV_LENGTHS: [[BankedAtomicU64; LINUX_IOV_LIMIT]; LINUX_THREAD_LIMIT] =
    [const { [const { BankedAtomicU64::new(0) }; LINUX_IOV_LIMIT] }; LINUX_THREAD_LIMIT];
static LINUX_CURRENT_THREAD_SLOT: BankedAtomicU8 = BankedAtomicU8::new(LINUX_THREAD_SLOT_NONE);
static LINUX_NEXT_TID: BankedAtomicU32 = BankedAtomicU32::new(1);
static ELF_PROBE_RELRO: BankedAtomicU64 = BankedAtomicU64::new(0);
static ELF_PROBE_RELRO_SIZE: BankedAtomicU64 = BankedAtomicU64::new(0);
static ELF_PROBE_BRK: BankedAtomicU64 = BankedAtomicU64::new(0);
static ELF_PROBE_BRK_LIMIT: BankedAtomicU64 = BankedAtomicU64::new(0);
static ELF_PROBE_MMAP: BankedAtomicU64 = BankedAtomicU64::new(0);
static ELF_PROBE_MMAP_FRAMES: [BankedAtomicU64; LINUX_RUNTIME_MMAP_SLOT_COUNT] =
    [const { BankedAtomicU64::new(0) }; LINUX_RUNTIME_MMAP_SLOT_COUNT];
static ELF_PROBE_MMAP_GENERATIONS: [BankedAtomicU32; LINUX_RUNTIME_MMAP_SLOT_COUNT] =
    [const { BankedAtomicU32::new(0) }; LINUX_RUNTIME_MMAP_SLOT_COUNT];
static ELF_PROBE_PAGE_TABLE_FRAMES: [BankedAtomicU64; 6] = [const { BankedAtomicU64::new(0) }; 6];
static ELF_PROBE_PAGE_TABLE_GENERATIONS: [BankedAtomicU32; 6] =
    [const { BankedAtomicU32::new(0) }; 6];
static ELF_PROBE_RECLAIM_PAGE_TABLE_FRAMES: [BankedAtomicU64; 6] =
    [const { BankedAtomicU64::new(0) }; 6];
static ELF_PROBE_RECLAIM_PAGE_TABLE_GENERATIONS: [BankedAtomicU32; 6] =
    [const { BankedAtomicU32::new(0) }; 6];
static ELF_PROBE_MMAP_PAGES: [BankedAtomicU8; LINUX_RUNTIME_MMAP_SLOT_COUNT] =
    [const { BankedAtomicU8::new(0) }; LINUX_RUNTIME_MMAP_SLOT_COUNT];
static ELF_PROBE_MMAP_PROTECTION: [BankedAtomicU8; LINUX_RUNTIME_MMAP_SLOT_COUNT] =
    [const { BankedAtomicU8::new(0) }; LINUX_RUNTIME_MMAP_SLOT_COUNT];
static ELF_PROBE_MMAP_FILE: [BankedAtomicU8; LINUX_RUNTIME_MMAP_SLOT_COUNT] =
    [const { BankedAtomicU8::new(0) }; LINUX_RUNTIME_MMAP_SLOT_COUNT];
static ELF_PROBE_MAIN_ADDRESSES: [BankedAtomicU64; LINUX_MAIN_MAPPING_COUNT] =
    [const { BankedAtomicU64::new(0) }; LINUX_MAIN_MAPPING_COUNT];
static ELF_PROBE_MAIN_FRAMES: [BankedAtomicU64; LINUX_MAIN_MAPPING_COUNT] =
    [const { BankedAtomicU64::new(0) }; LINUX_MAIN_MAPPING_COUNT];
static ELF_PROBE_MAIN_OFFSETS: [BankedAtomicU64; LINUX_MAIN_MAPPING_COUNT] =
    [const { BankedAtomicU64::new(0) }; LINUX_MAIN_MAPPING_COUNT];
static ELF_PROBE_MAIN_FILE_SIZES: [BankedAtomicU64; LINUX_MAIN_MAPPING_COUNT] =
    [const { BankedAtomicU64::new(0) }; LINUX_MAIN_MAPPING_COUNT];
static ELF_PROBE_MAIN_PROTECTIONS: [BankedAtomicU8; LINUX_MAIN_MAPPING_COUNT] =
    [const { BankedAtomicU8::new(0) }; LINUX_MAIN_MAPPING_COUNT];
static ELF_PROBE_MAIN_GENERATIONS: [BankedAtomicU32; LINUX_MAIN_MAPPING_COUNT] =
    [const { BankedAtomicU32::new(0) }; LINUX_MAIN_MAPPING_COUNT];
static ELF_PROBE_MAIN_MAPPINGS: BankedAtomicU8 = BankedAtomicU8::new(0);
static ELF_PROBE_DEPENDENCY_BASE_MIN: [BankedAtomicU64; LINUX_DEPENDENCY_COUNT] =
    [const { BankedAtomicU64::new(0) }; LINUX_DEPENDENCY_COUNT];
static ELF_PROBE_DEPENDENCY_BASE_END: [BankedAtomicU64; LINUX_DEPENDENCY_COUNT] =
    [const { BankedAtomicU64::new(0) }; LINUX_DEPENDENCY_COUNT];
static ELF_PROBE_DEPENDENCY_SELECTED_BASE: [BankedAtomicU64; LINUX_DEPENDENCY_COUNT] =
    [const { BankedAtomicU64::new(0) }; LINUX_DEPENDENCY_COUNT];
static ELF_PROBE_DEPENDENCY_RELATIVES: [BankedAtomicU64; LINUX_DEPENDENCY_STATE_COUNT] =
    [const { BankedAtomicU64::new(0) }; LINUX_DEPENDENCY_STATE_COUNT];
static ELF_PROBE_DEPENDENCY_FRAMES: [BankedAtomicU64; LINUX_DEPENDENCY_STATE_COUNT] =
    [const { BankedAtomicU64::new(0) }; LINUX_DEPENDENCY_STATE_COUNT];
static ELF_PROBE_DEPENDENCY_GENERATIONS: [BankedAtomicU32; LINUX_DEPENDENCY_STATE_COUNT] =
    [const { BankedAtomicU32::new(0) }; LINUX_DEPENDENCY_STATE_COUNT];
static ELF_PROBE_DEPENDENCY_OFFSETS: [BankedAtomicU64; LINUX_DEPENDENCY_STATE_COUNT] =
    [const { BankedAtomicU64::new(0) }; LINUX_DEPENDENCY_STATE_COUNT];
static ELF_PROBE_DEPENDENCY_FILE_SIZES: [BankedAtomicU64; LINUX_DEPENDENCY_STATE_COUNT] =
    [const { BankedAtomicU64::new(0) }; LINUX_DEPENDENCY_STATE_COUNT];
static ELF_PROBE_DEPENDENCY_PROTECTIONS: [BankedAtomicU8; LINUX_DEPENDENCY_STATE_COUNT] =
    [const { BankedAtomicU8::new(0) }; LINUX_DEPENDENCY_STATE_COUNT];
static ELF_PROBE_DEPENDENCY_MAPPINGS: [BankedAtomicU8; LINUX_DEPENDENCY_COUNT] =
    [const { BankedAtomicU8::new(0) }; LINUX_DEPENDENCY_COUNT];
static ELF_PROBE_EXITED_CLEAR_CHILD_TID: BankedAtomicU64 = BankedAtomicU64::new(0);
static ELF_PROBE_EXITED_ROBUST_LIST: BankedAtomicU64 = BankedAtomicU64::new(0);
static ELF_PROBE_READ_ONLY_DATA_PAGE: BankedAtomicU64 = BankedAtomicU64::new(0);
static ELF_PROBE_UMASK: BankedAtomicU64 = BankedAtomicU64::new(0);
static ELF_PROBE_CONSOLE_DESCRIPTORS: BankedAtomicU8 = BankedAtomicU8::new(0);
static ELF_PROBE_CONSOLE_CLOEXEC: BankedAtomicU16 = BankedAtomicU16::new(0);
static ELF_PROBE_DEVICE_DESCRIPTORS: [BankedAtomicU16; LINUX_DEVICE_DESCRIPTOR_COUNT] =
    [const { BankedAtomicU16::new(0) }; LINUX_DEVICE_DESCRIPTOR_COUNT];
static ELF_PROBE_OCI_DESCRIPTORS: [BankedAtomicU16; LINUX_DEVICE_DESCRIPTOR_COUNT] =
    [const { BankedAtomicU16::new(0) }; LINUX_DEVICE_DESCRIPTOR_COUNT];
static ELF_PROBE_TMP_FILE_GENERATIONS: [BankedAtomicU32; LINUX_DEVICE_DESCRIPTOR_COUNT] =
    [const { BankedAtomicU32::new(0) }; LINUX_DEVICE_DESCRIPTOR_COUNT];
static ELF_PROBE_STDIO_DEVICE_DESCRIPTORS: [BankedAtomicU16; 2] =
    [const { BankedAtomicU16::new(0) }; 2];
static ELF_PROBE_STDIO_OCI_DESCRIPTORS: [BankedAtomicU16; 2] =
    [const { BankedAtomicU16::new(0) }; 2];
static ELF_PROBE_STDIO_TMP_FILE_GENERATIONS: [BankedAtomicU32; 2] =
    [const { BankedAtomicU32::new(0) }; 2];
static ELF_PROBE_STDIO_OFFSETS: [BankedAtomicU64; 2] = [const { BankedAtomicU64::new(0) }; 2];
static ELF_PROBE_CWD: BankedAtomicU16 = BankedAtomicU16::new(LINUX_ROOT_DIRECTORY);
static ELF_PROBE_OCI_CWD: BankedAtomicU16 = BankedAtomicU16::new(crate::oci_image::ROOT_HANDLE);
static ELF_PROBE_EPOLL_DESCRIPTOR: BankedAtomicU8 = BankedAtomicU8::new(0);
static ELF_PROBE_EPOLL_TARGET: BankedAtomicU64 = BankedAtomicU64::new(u64::MAX);
static ELF_PROBE_EPOLL_EVENTS: BankedAtomicU32 = BankedAtomicU32::new(0);
static ELF_PROBE_EPOLL_DATA: BankedAtomicU64 = BankedAtomicU64::new(0);
static ELF_PROBE_EVENTFD_DESCRIPTOR: BankedAtomicU8 = BankedAtomicU8::new(0);
static ELF_PROBE_EVENTFD_COUNTER: BankedAtomicU64 = BankedAtomicU64::new(0);
static ELF_PROBE_EVENTFD_FLAGS: BankedAtomicU32 = BankedAtomicU32::new(0);
static ELF_PROBE_TIMERFD_DESCRIPTOR: BankedAtomicU8 = BankedAtomicU8::new(0);
static ELF_PROBE_TIMERFD_FLAGS: BankedAtomicU32 = BankedAtomicU32::new(0);
static ELF_PROBE_TIMERFD_INTERVAL: BankedAtomicU64 = BankedAtomicU64::new(0);
static ELF_PROBE_TIMERFD_DEADLINE: BankedAtomicU64 = BankedAtomicU64::new(0);
static ELF_PROBE_PIPE_READ_DESCRIPTOR: BankedAtomicU8 = BankedAtomicU8::new(0);
static ELF_PROBE_PIPE_WRITE_DESCRIPTOR: BankedAtomicU8 = BankedAtomicU8::new(0);
static ELF_PROBE_PIPE_READ_FLAGS: BankedAtomicU32 = BankedAtomicU32::new(0);
static ELF_PROBE_PIPE_WRITE_FLAGS: BankedAtomicU32 = BankedAtomicU32::new(0);
static ELF_PROBE_PIPE_LENGTH: BankedAtomicU32 = BankedAtomicU32::new(0);
static ELF_PROBE_PIPE_BUFFER: [BankedAtomicU8; LINUX_PIPE_CAPACITY] =
    [const { BankedAtomicU8::new(0) }; LINUX_PIPE_CAPACITY];
static ELF_PROBE_DESCRIPTOR_OFFSETS: [BankedAtomicU64; LINUX_DEVICE_DESCRIPTOR_COUNT] =
    [const { BankedAtomicU64::new(0) }; LINUX_DEVICE_DESCRIPTOR_COUNT];
static ELF_PROBE_CLOCK_EPOCH: BankedAtomicU64 = BankedAtomicU64::new(0);
static ELF_PROBE_CLOCK_FREQUENCY: BankedAtomicU64 = BankedAtomicU64::new(0);
static ELF_PROBE_PROCESS_NAME_LOW: BankedAtomicU64 = BankedAtomicU64::new(0);
static ELF_PROBE_PROCESS_NAME_HIGH: BankedAtomicU64 = BankedAtomicU64::new(0);
static LINUX_RUNTIME_EXIT_STATUS: BankedAtomicU64 = BankedAtomicU64::new(u64::MAX);
static LINUX_RUNTIME_FIRST_UNSUPPORTED_SYSCALL: BankedAtomicU64 = BankedAtomicU64::new(u64::MAX);
const LINUX_RUNTIME_SYSCALL_TRACE_LIMIT: usize = 32;
static LINUX_RUNTIME_SYSCALL_TRACE: [BankedAtomicU64; LINUX_RUNTIME_SYSCALL_TRACE_LIMIT] =
    [const { BankedAtomicU64::new(u64::MAX) }; LINUX_RUNTIME_SYSCALL_TRACE_LIMIT];
static LINUX_RUNTIME_SYSCALL_TRACE_COUNT: BankedAtomicU64 = BankedAtomicU64::new(0);
static ELF_PROBE_NO_NEW_PRIVS: BankedAtomicBool = BankedAtomicBool::new(false);
static ELF_PROBE_SIGNAL_MASK: BankedAtomicU64 = BankedAtomicU64::new(0);
static ELF_PROBE_PENDING_SIGNALS: BankedAtomicU64 = BankedAtomicU64::new(0);
static ELF_PROBE_PENDING_SIGNAL_CODES: [BankedAtomicU32; LINUX_SIGNAL_COUNT] =
    [const { BankedAtomicU32::new(0) }; LINUX_SIGNAL_COUNT];
static ELF_PROBE_EXEC_IGNORED_SIGNALS: BankedAtomicU64 = BankedAtomicU64::new(0);
static ELF_PROBE_SIGNAL_HANDLERS: [BankedAtomicU64; LINUX_SIGNAL_COUNT] =
    [const { BankedAtomicU64::new(0) }; LINUX_SIGNAL_COUNT];
static ELF_PROBE_SIGNAL_FLAGS: [BankedAtomicU64; LINUX_SIGNAL_COUNT] =
    [const { BankedAtomicU64::new(0) }; LINUX_SIGNAL_COUNT];
static ELF_PROBE_SIGNAL_RESTORERS: [BankedAtomicU64; LINUX_SIGNAL_COUNT] =
    [const { BankedAtomicU64::new(0) }; LINUX_SIGNAL_COUNT];
static ELF_PROBE_SIGNAL_ACTION_MASKS: [BankedAtomicU64; LINUX_SIGNAL_COUNT] =
    [const { BankedAtomicU64::new(0) }; LINUX_SIGNAL_COUNT];
static ELF_PROBE_ALT_STACK_POINTER: BankedAtomicU64 = BankedAtomicU64::new(0);
static ELF_PROBE_ALT_STACK_SIZE: BankedAtomicU64 = BankedAtomicU64::new(0);
static ELF_PROBE_ALT_STACK_FLAGS: BankedAtomicU32 = BankedAtomicU32::new(LINUX_SS_DISABLE);
static LINUX_SIGNAL_ACTIVE: BankedAtomicBool = BankedAtomicBool::new(false);
static LINUX_SIGNAL_DELIVERY_COUNT: BankedAtomicU64 = BankedAtomicU64::new(0);
static LINUX_SIGNAL_RETURN_COUNT: BankedAtomicU64 = BankedAtomicU64::new(0);
static LINUX_SIGNAL_WAIT_BLOCK_COUNT: BankedAtomicU32 = BankedAtomicU32::new(0);
static LINUX_SIGNAL_WAIT_WAKE_COUNT: BankedAtomicU32 = BankedAtomicU32::new(0);
static LINUX_SIGNAL_WAIT_TIMEOUT_COUNT: BankedAtomicU32 = BankedAtomicU32::new(0);
static LINUX_SYSTEM_SUPERVISOR_WAIT_COUNT: BankedAtomicU32 = BankedAtomicU32::new(0);
static LINUX_SYSTEM_SUPERVISOR_SHUTDOWN_COUNT: BankedAtomicU32 = BankedAtomicU32::new(0);
const CONTAINER_SLOT_NONE: u8 = u8::MAX;
static CONTAINER_CONTROL_STATES: [AtomicU8; control_abi::CONTAINER_LIMIT] =
    [const { AtomicU8::new(0) }; control_abi::CONTAINER_LIMIT];
static CONTAINER_DOMAIN_IDS: [AtomicU64; control_abi::CONTAINER_LIMIT] =
    [const { AtomicU64::new(0) }; control_abi::CONTAINER_LIMIT];
static CONTAINER_CONFIG_FLAGS: [AtomicU16; control_abi::CONTAINER_LIMIT] =
    [const { AtomicU16::new(0) }; control_abi::CONTAINER_LIMIT];
static CONTAINER_CPU_COUNTS: [AtomicU16; control_abi::CONTAINER_LIMIT] =
    [const { AtomicU16::new(0) }; control_abi::CONTAINER_LIMIT];
static CONTAINER_MEMORY_MIB: [AtomicU64; control_abi::CONTAINER_LIMIT] =
    [const { AtomicU64::new(0) }; control_abi::CONTAINER_LIMIT];
static CONTAINER_NAME_SIZES: [AtomicU8; control_abi::CONTAINER_LIMIT] =
    [const { AtomicU8::new(0) }; control_abi::CONTAINER_LIMIT];
static CONTAINER_NAMES: [[AtomicU8; control_abi::CONTAINER_MAX_NAME_SIZE];
    control_abi::CONTAINER_LIMIT] =
    [const { [const { AtomicU8::new(0) }; control_abi::CONTAINER_MAX_NAME_SIZE] };
        control_abi::CONTAINER_LIMIT];
static CONTAINER_INIT_SIZES: [AtomicU8; control_abi::CONTAINER_LIMIT] =
    [const { AtomicU8::new(0) }; control_abi::CONTAINER_LIMIT];
static CONTAINER_INITS: [[AtomicU8; control_abi::CONTAINER_MAX_INIT_SIZE];
    control_abi::CONTAINER_LIMIT] =
    [const { [const { AtomicU8::new(0) }; control_abi::CONTAINER_MAX_INIT_SIZE] };
        control_abi::CONTAINER_LIMIT];
static CONTAINER_IMAGE_SIZES: [AtomicU8; control_abi::CONTAINER_LIMIT] =
    [const { AtomicU8::new(0) }; control_abi::CONTAINER_LIMIT];
static CONTAINER_IMAGES: [[AtomicU8; control_abi::CONTAINER_MAX_IMAGE_SIZE];
    control_abi::CONTAINER_LIMIT] =
    [const { [const { AtomicU8::new(0) }; control_abi::CONTAINER_MAX_IMAGE_SIZE] };
        control_abi::CONTAINER_LIMIT];
static CONTAINER_SELECTED_SLOT: AtomicU8 = AtomicU8::new(CONTAINER_SLOT_NONE);
static CONTAINER_ACTIVE_SLOT: AtomicU8 = AtomicU8::new(CONTAINER_SLOT_NONE);
static CONTAINER_RESTART_REQUESTED: AtomicBool = AtomicBool::new(false);
static CONTAINER_HOST_SHUTDOWN_REQUESTED: AtomicBool = AtomicBool::new(false);
static CONTAINER_MANAGEMENT_ENABLED: AtomicBool = AtomicBool::new(false);
static LINUX_ABI_TEST_RESULTS: AtomicU64 = AtomicU64::new(0);
static NETWORK_PING_REQUESTED: AtomicU64 = AtomicU64::new(0);
static NETWORK_PING_ACTIVE: AtomicU64 = AtomicU64::new(0);
static NETWORK_PING_RESULT: AtomicU8 = AtomicU8::new(0);
static NETWORK_DHCP_REQUESTED: AtomicBool = AtomicBool::new(false);
static NETWORK_DHCP_RESULT: AtomicU8 = AtomicU8::new(0);
static CONTAINER_PULL_REQUESTED: AtomicBool = AtomicBool::new(false);
static LINUX_RUNTIME_SUSPEND_REQUESTED: BankedAtomicBool = BankedAtomicBool::new(false);
static LINUX_RUNTIME_SUSPENDED: BankedAtomicBool = BankedAtomicBool::new(false);
static LINUX_RUNTIME_SUSPEND_ADDRESS: BankedAtomicU64 = BankedAtomicU64::new(0);
static LINUX_RUNTIME_SUSPEND_COUNT: BankedAtomicU64 = BankedAtomicU64::new(0);
static LINUX_RUNTIME_SUSPEND_RIP: BankedAtomicU64 = BankedAtomicU64::new(0);
static LINUX_RUNTIME_SUSPEND_RSP: BankedAtomicU64 = BankedAtomicU64::new(0);
static LINUX_RUNTIME_SUSPEND_RFLAGS: BankedAtomicU64 = BankedAtomicU64::new(0);
static LINUX_RUNTIME_SUSPEND_FS_BASE: BankedAtomicU64 = BankedAtomicU64::new(0);
static LINUX_RUNTIME_SUSPEND_REGISTERS: [BankedAtomicU64; LINUX_SYSCALL_FRAME_REGISTERS] =
    [const { BankedAtomicU64::new(0) }; LINUX_SYSCALL_FRAME_REGISTERS];
const CONTAINER_CONSOLE_CAPACITY: usize = 4096;
const CONTAINER_CONSOLE_DETACH: u8 = 0x1d;
static CONTAINER_CONSOLE_ATTACHED: [AtomicBool; control_abi::CONTAINER_LIMIT] =
    [const { AtomicBool::new(false) }; control_abi::CONTAINER_LIMIT];
static CONTAINER_STOP_REQUESTED: [AtomicBool; control_abi::CONTAINER_LIMIT] =
    [const { AtomicBool::new(false) }; control_abi::CONTAINER_LIMIT];

const EXTENDED_STATE_SIZE: usize = 1024;

#[derive(Clone, Copy)]
#[repr(C, align(64))]
struct ExtendedState([u8; EXTENDED_STATE_SIZE]);

static mut CONTAINER_EXTENDED_STATES: [ExtendedState; control_abi::CONTAINER_LIMIT] =
    [ExtendedState([0; EXTENDED_STATE_SIZE]); control_abi::CONTAINER_LIMIT];
static CONTAINER_EXTENDED_STATE_VALID: [AtomicBool; control_abi::CONTAINER_LIMIT] =
    [const { AtomicBool::new(false) }; control_abi::CONTAINER_LIMIT];
static EXTENDED_USER_STATE_ENABLED: AtomicBool = AtomicBool::new(false);
static CONTAINER_CONSOLE_CURSOR: [AtomicU64; control_abi::CONTAINER_LIMIT] =
    [const { AtomicU64::new(0) }; control_abi::CONTAINER_LIMIT];
static CONTAINER_CONSOLE_BYTES: [[AtomicU8; CONTAINER_CONSOLE_CAPACITY];
    control_abi::CONTAINER_LIMIT] = [const { [const { AtomicU8::new(0) }; CONTAINER_CONSOLE_CAPACITY] };
    control_abi::CONTAINER_LIMIT];
static ELF_PROBE_SYSTEM_INIT: AtomicBool = AtomicBool::new(false);
static SERVICE_STACK_SCRUBBED: AtomicBool = AtomicBool::new(false);
static SERVICE_FRAMES_TRANSFERRED: AtomicBool = AtomicBool::new(false);
static mut USER_RETURN_STACK: u64 = 0;
static mut LINUX_USER_RSP: u64 = 0;
static mut LINUX_USER_RIP: u64 = 0;
static mut LINUX_USER_RFLAGS: u64 = 0;
static mut LINUX_KERNEL_FRAME: u64 = 0;
static mut LINUX_SYSCALL_RESULT: u64 = 0;
static mut LINUX_SIGNAL_RAX: u64 = 0;
static mut LINUX_SIGNAL_RCX: u64 = 0;
static mut LINUX_SIGNAL_R11: u64 = 0;
static mut LINUX_SIGNAL_CONTEXT: LinuxSignalContext = LinuxSignalContext::ZERO;
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
    mov r14, [rsp + 72]
    mov rbx, rdx
    push {user_data}
    push rsi
    push 0x202
    push {user_code}
    push rdi
    iretq

    .global ginux_process_user_enter
ginux_process_user_enter:
    push rbx
    push rbp
    push r12
    push r13
    push r14
    push r15
    mov [rip + {user_return_stack}], rsp
    push {user_data}
    push rsi
    push rcx
    push {user_code}
    push rdi
    mov rax, rdx
    mov rbx, [rax + 8]
    mov rcx, [rax + 16]
    mov rdx, [rax + 24]
    mov rsi, [rax + 32]
    mov rdi, [rax + 40]
    mov rbp, [rax + 48]
    mov r8, [rax + 56]
    mov r9, [rax + 64]
    mov r10, [rax + 72]
    mov r11, [rax + 80]
    mov r12, [rax + 88]
    mov r13, [rax + 96]
    mov r14, [rax + 104]
    mov r15, [rax + 112]
    mov rax, [rax]
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
    push rbx
    push rbp
    push r12
    push r13
    push r14
    push r15
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
    cmp rax, {linux_return_from_signal}
    je ginux_linux_signal_return
    mov [rip + {linux_syscall_result}], rax
    mov rsp, [rip + {linux_kernel_frame}]
    pop r15
    pop r14
    pop r13
    pop r12
    pop rbp
    pop rbx
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
ginux_linux_signal_return:
    mov rsp, [rip + {linux_kernel_frame}]
    pop r15
    pop r14
    pop r13
    pop r12
    pop rbp
    pop rbx
    pop r10
    pop r9
    pop r8
    pop rdx
    pop rsi
    pop rdi
    push {user_data}
    push [rip + {linux_user_rsp}]
    push [rip + {linux_user_rflags}]
    push {user_code}
    push [rip + {linux_user_rip}]
    mov rax, [rip + {linux_signal_rax}]
    mov rcx, [rip + {linux_signal_rcx}]
    mov r11, [rip + {linux_signal_r11}]
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
    linux_syscall_handler = sym linux_syscall_handler,
    user_return_stack = sym USER_RETURN_STACK,
    linux_user_rsp = sym LINUX_USER_RSP,
    linux_user_rip = sym LINUX_USER_RIP,
    linux_user_rflags = sym LINUX_USER_RFLAGS,
    linux_kernel_frame = sym LINUX_KERNEL_FRAME,
    linux_syscall_result = sym LINUX_SYSCALL_RESULT,
    linux_signal_rax = sym LINUX_SIGNAL_RAX,
    linux_signal_rcx = sym LINUX_SIGNAL_RCX,
    linux_signal_r11 = sym LINUX_SIGNAL_R11,
    linux_return_to_kernel = const LINUX_RETURN_TO_KERNEL as i64,
    linux_return_from_signal = const LINUX_RETURN_FROM_SIGNAL as i64,
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
        root_handle: u64,
    );
    fn ginux_process_user_enter(entry: usize, stack: usize, general: *const u64, rflags: u64);
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

pub(crate) fn enable_extended_user_state() -> bool {
    let features = __cpuid(1);
    if features.edx & CPUID_SSE == 0
        || features.ecx & (CPUID_XSAVE | CPUID_AVX) != CPUID_XSAVE | CPUID_AVX
        || __cpuid(0).eax < CPUID_EXTENDED_STATE
        || __cpuid_count(CPUID_EXTENDED_STATE, 0).eax & 7 != 7
    {
        return false;
    }
    let mut cr0: u64;
    let mut cr4: u64;
    unsafe {
        asm!("mov {}, cr0", out(reg) cr0, options(nomem, nostack, preserves_flags));
        cr0 = (cr0 | CR0_MP | CR0_NE) & !CR0_EM;
        asm!("mov cr0, {}", in(reg) cr0, options(nomem, nostack, preserves_flags));
        asm!("mov {}, cr4", out(reg) cr4, options(nomem, nostack, preserves_flags));
        cr4 |= CR4_OSFXSR | CR4_OSXMMEXCPT | CR4_OSXSAVE;
        asm!("mov cr4, {}", in(reg) cr4, options(nomem, nostack, preserves_flags));
        asm!(
            "xsetbv",
            in("ecx") 0_u32,
            in("eax") 7_u32,
            in("edx") 0_u32,
            options(nostack, preserves_flags)
        );
    }
    let state = __cpuid_count(CPUID_EXTENDED_STATE, 0);
    let enabled = state.ebx as usize <= EXTENDED_STATE_SIZE && reset_extended_user_state();
    EXTENDED_USER_STATE_ENABLED.store(enabled, Ordering::Release);
    enabled
}

fn reset_extended_user_state() -> bool {
    let default_mxcsr = 0x1f80_u32;
    unsafe {
        asm!("fninit", options(nostack));
        asm!("ldmxcsr [{}]", in(reg) &default_mxcsr, options(nostack));
        asm!("vzeroall", options(nostack));
    }
    true
}

fn restore_container_extended_state() -> bool {
    if !EXTENDED_USER_STATE_ENABLED.load(Ordering::Acquire) {
        return true;
    }
    let Some(slot) = active_container_slot() else {
        return false;
    };
    if !CONTAINER_EXTENDED_STATE_VALID[slot].load(Ordering::Acquire) {
        return reset_extended_user_state();
    }
    let state = unsafe {
        core::ptr::addr_of!(CONTAINER_EXTENDED_STATES)
            .cast::<ExtendedState>()
            .add(slot)
    };
    unsafe {
        asm!(
            "xrstor64 [{}]",
            in(reg) state,
            in("eax") 7_u32,
            in("edx") 0_u32,
            options(nostack)
        );
    }
    true
}

fn save_container_extended_state() -> bool {
    if !EXTENDED_USER_STATE_ENABLED.load(Ordering::Acquire) {
        return true;
    }
    let Some(slot) = active_container_slot() else {
        return false;
    };
    let state = unsafe {
        core::ptr::addr_of_mut!(CONTAINER_EXTENDED_STATES)
            .cast::<ExtendedState>()
            .add(slot)
    };
    unsafe {
        asm!(
            "xsave64 [{}]",
            in(reg) state,
            in("eax") 7_u32,
            in("edx") 0_u32,
            options(nostack)
        );
    }
    CONTAINER_EXTENDED_STATE_VALID[slot].store(true, Ordering::Release);
    reset_extended_user_state()
}

fn discard_container_extended_state() -> bool {
    if !EXTENDED_USER_STATE_ENABLED.load(Ordering::Acquire) {
        return true;
    }
    let Some(slot) = active_container_slot() else {
        return false;
    };
    CONTAINER_EXTENDED_STATE_VALID[slot].store(false, Ordering::Release);
    reset_extended_user_state()
}

pub(crate) fn initialize_user_call_gate() -> bool {
    install_user_interrupt_gate(SYSCALL_VECTOR, ginux_syscall_entry as *const () as usize)
        && user_interrupt_surface_is_minimal()
}

pub(crate) unsafe fn with_supervisor_user_access<T>(operation: impl FnOnce() -> T) -> T {
    asm!("stac", options(nostack));
    let result = operation();
    asm!("clac", options(nostack));
    result
}

pub(crate) fn execute_linux_process_child(
    entry: usize,
    stack: usize,
    fs_base: u64,
    general: [u64; 15],
) -> Option<u8> {
    let entry_valid = entry != 0 && entry < 1 << 47;
    let stack_valid = stack != 0 && stack < 1 << 47;
    let fs_valid = fs_base < 1 << 47;
    let fork_return_valid = general[0] == 0;
    let scheduler_idle = scheduler::current_domain().is_none();
    let gate_installed =
        install_user_interrupt_gate(SYSCALL_VECTOR, ginux_syscall_entry as *const () as usize);
    let interrupt_surface_valid = user_interrupt_surface_is_minimal();
    let preconditions = u8::from(entry_valid)
        | (u8::from(stack_valid) << 1)
        | (u8::from(fs_valid) << 2)
        | (u8::from(fork_return_valid) << 3)
        | (u8::from(scheduler_idle) << 4)
        | (u8::from(gate_installed) << 5)
        | (u8::from(interrupt_surface_valid) << 6);
    PROCESS_CHILD_EXECUTION_DIAGNOSTIC.store(preconditions, Ordering::SeqCst);
    if preconditions != 0x7f {
        return None;
    }
    let saved_fs = active_fs_base();
    unsafe { write_msr(FS_BASE_MSR, fs_base) };
    if active_fs_base() != fs_base {
        unsafe { write_msr(FS_BASE_MSR, saved_fs) };
        return None;
    }
    for (expected, value) in PROCESS_CHILD_EXPECTED_REGISTERS.iter().zip(general) {
        expected.store(value, Ordering::SeqCst);
    }
    PROCESS_CHILD_EXPECTED_FS_BASE.store(fs_base, Ordering::SeqCst);
    PROCESS_CHILD_CONTEXT_VALID.store(false, Ordering::SeqCst);
    PROCESS_CHILD_EXIT_STATUS.store(0, Ordering::SeqCst);
    if PROCESS_CHILD_PROBE_STATE
        .compare_exchange(
            PROCESS_CHILD_PROBE_IDLE,
            PROCESS_CHILD_PROBE_EXPECT_CONTEXT,
            Ordering::SeqCst,
            Ordering::SeqCst,
        )
        .is_err()
    {
        unsafe { write_msr(FS_BASE_MSR, saved_fs) };
        return None;
    }
    // SAFETY: The caller has activated a root containing the validated RX
    // entry, writable stack, supervisor kernel mappings, and TSS stack. The
    // process entry stub installs the complete saved general-register array,
    // including the validated Linux fork-child RAX value. The first INT 0x80
    // validates that frame and returns to ring 3. A subsequent Linux exit(2)
    // trap captures its status and returns to the saved kernel stack because no
    // global native scheduler runtime is active.
    unsafe { ginux_process_user_enter(entry, stack, general.as_ptr(), 0x202) };
    let child_fs_preserved = active_fs_base() == fs_base;
    let probe_state = PROCESS_CHILD_PROBE_STATE.swap(PROCESS_CHILD_PROBE_IDLE, Ordering::SeqCst);
    let context_valid = PROCESS_CHILD_CONTEXT_VALID.swap(false, Ordering::SeqCst);
    let exit_status = PROCESS_CHILD_EXIT_STATUS.swap(0, Ordering::SeqCst);
    unsafe { write_msr(FS_BASE_MSR, saved_fs) };
    let fs_restored = active_fs_base() == saved_fs;
    PROCESS_CHILD_EXECUTION_DIAGNOSTIC.store(
        0x80 | u8::from(child_fs_preserved)
            | (u8::from(context_valid) << 1)
            | (u8::from(probe_state == PROCESS_CHILD_PROBE_COMPLETE) << 2)
            | (u8::from(fs_restored) << 3)
            | (u8::from(exit_status == 42) << 4),
        Ordering::SeqCst,
    );
    (child_fs_preserved
        && context_valid
        && probe_state == PROCESS_CHILD_PROBE_COMPLETE
        && fs_restored)
        .then_some(exit_status)
}

pub(crate) fn resume_live_linux_process(
    entry: usize,
    stack: usize,
    fs_base: u64,
    general: [u64; 15],
    rflags: u64,
) -> bool {
    if entry == 0 || entry >= 1 << 47 || stack == 0 || stack >= 1 << 47 || fs_base >= 1 << 47 {
        return false;
    }
    let saved_fs = active_fs_base();
    unsafe { write_msr(FS_BASE_MSR, fs_base) };
    if active_fs_base() != fs_base {
        unsafe { write_msr(FS_BASE_MSR, saved_fs) };
        return false;
    }
    // SAFETY: The caller activated the generation-checked process root and
    // validated this complete return context against its private mappings.
    // A terminating or deferred Linux syscall returns through the saved kernel
    // stack installed by this entry stub.
    unsafe { ginux_process_user_enter(entry, stack, general.as_ptr(), rflags) };
    let preserved = active_fs_base() == fs_base;
    unsafe { write_msr(FS_BASE_MSR, saved_fs) };
    preserved && active_fs_base() == saved_fs
}

pub(crate) fn reset_process_child_execution_diagnostic() {
    PROCESS_CHILD_EXECUTION_DIAGNOSTIC.store(0, Ordering::SeqCst);
}

pub(crate) fn verify_user_syscall(
    entry: usize,
    stack: usize,
    host_handle: u64,
    console_handle: u64,
    ipc_handle: u64,
    root_handle: u64,
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
    if !install_current_scheduler_fs_base() {
        return false;
    }
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
            root_handle,
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
    if !install_current_scheduler_fs_base() {
        return;
    }
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
            0,
        )
    };
}

pub(crate) fn execute_packet_consumer(
    entry: usize,
    stack: usize,
    packet_handle: u64,
    expected_sequence: u64,
    more_pending: bool,
    domain_id: u64,
    service_identity: u64,
) {
    if !install_current_scheduler_fs_base() {
        return;
    }
    unsafe {
        ginux_user_enter(
            entry,
            stack,
            expected_sequence,
            packet_handle,
            u64::from(more_pending),
            4,
            domain_id,
            service_identity,
            0,
        )
    };
}

pub(crate) fn execute_native_auxiliary(
    entry: usize,
    stack: usize,
    boot_info: u64,
    capability: u64,
    domain_id: u64,
    service_identity: u64,
) {
    if !install_current_scheduler_fs_base() {
        return;
    }
    unsafe {
        ginux_user_enter(
            entry,
            stack,
            boot_info,
            capability,
            0,
            4,
            domain_id,
            service_identity,
            0,
        )
    };
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn execute_native_controller(
    entry: usize,
    stack: usize,
    console_handle: u64,
    host_handle: u64,
    root_handle: u64,
    network_handle: u64,
    domain_id: u64,
    service_identity: u64,
) {
    if !install_current_scheduler_fs_base() {
        return;
    }
    unsafe {
        ginux_user_enter(
            entry,
            stack,
            console_handle,
            host_handle,
            network_handle,
            4,
            domain_id,
            service_identity,
            root_handle,
        )
    };
}

pub(crate) fn resume_native_controller(domain_id: u64) -> bool {
    let Some(context) = scheduler::resume_container_lifecycle(domain_id) else {
        return false;
    };
    let (Ok(entry), Ok(stack)) = (
        usize::try_from(context.instruction()),
        usize::try_from(context.stack()),
    ) else {
        return false;
    };
    if !activate_user_root(context.address_space_root()) || !install_user_fs_base(context.fs_base())
    {
        return false;
    }
    let general = context.general();
    // SAFETY: the scheduler retained the complete validated controller trap
    // context while its lifecycle request was serviced by the kernel.
    unsafe { ginux_process_user_enter(entry, stack, general.as_ptr(), context.flags()) };
    true
}

#[derive(Clone, Copy)]
pub(crate) struct LinuxFileMapping {
    pub(crate) address: usize,
    pub(crate) offset: u64,
    pub(crate) file_size: usize,
    pub(crate) writable: bool,
    pub(crate) executable: bool,
}

impl LinuxFileMapping {
    pub(crate) const EMPTY: Self = Self {
        address: 0,
        offset: 0,
        file_size: 0,
        writable: false,
        executable: false,
    };
}

#[derive(Clone, Copy)]
pub(crate) struct LinuxRelativeFileMapping {
    pub(crate) relative_address: usize,
    pub(crate) offset: u64,
    pub(crate) file_size: usize,
    pub(crate) writable: bool,
    pub(crate) executable: bool,
}

impl LinuxRelativeFileMapping {
    pub(crate) const EMPTY: Self = Self {
        relative_address: 0,
        offset: 0,
        file_size: 0,
        writable: false,
        executable: false,
    };
}

pub(crate) struct LinuxElfExecution {
    pub(crate) runtime: bool,
    pub(crate) entry: usize,
    pub(crate) page_table_frames: [memory::FrameLease; 6],
    pub(crate) page_table_frame_count: usize,
    pub(crate) stack: usize,
    pub(crate) stack_frame: memory::FrameLease,
    pub(crate) heap_frame: memory::FrameLease,
    pub(crate) tls_frames: [memory::FrameLease; 2],
    pub(crate) initial_fs_base: usize,
    pub(crate) tls: usize,
    pub(crate) tls_size: usize,
    pub(crate) sibling_fs_base: usize,
    pub(crate) sibling_tls: usize,
    pub(crate) sibling_stack: usize,
    pub(crate) sibling_stack_frame: memory::FrameLease,
    pub(crate) code: usize,
    pub(crate) code_size: usize,
    pub(crate) loader_code: usize,
    pub(crate) loader_code_size: usize,
    pub(crate) data: usize,
    pub(crate) data_size: usize,
    pub(crate) relro: usize,
    pub(crate) relro_size: usize,
    pub(crate) heap: usize,
    pub(crate) heap_size: usize,
    pub(crate) mmap: usize,
    pub(crate) main_mappings: [LinuxFileMapping; LINUX_MAIN_MAPPING_COUNT],
    pub(crate) dependency_base_min: [usize; LINUX_DEPENDENCY_COUNT],
    pub(crate) dependency_base_end: [usize; LINUX_DEPENDENCY_COUNT],
    pub(crate) dependency_mappings:
        [[LinuxRelativeFileMapping; LINUX_DEPENDENCY_MAPPING_COUNT]; LINUX_DEPENDENCY_COUNT],
    pub(crate) exec_expected: bool,
    pub(crate) system_init: bool,
    pub(crate) initial_image_count: usize,
    pub(crate) initial_image_mappings: [LinuxActiveImageMapping; LINUX_ACTIVE_IMAGE_MAPPING_LIMIT],
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) struct LinuxActiveImageMapping {
    pub(crate) address: usize,
    pub(crate) frame: memory::FrameLease,
    pub(crate) writable: bool,
    pub(crate) executable: bool,
}

impl LinuxActiveImageMapping {
    pub(crate) const EMPTY: Self = Self {
        address: 0,
        frame: memory::FrameLease::EMPTY,
        writable: false,
        executable: false,
    };
}

#[derive(Clone, Copy)]
pub(crate) struct LinuxProcessAddressSpaceSnapshot {
    hierarchy: [memory::FrameLease; 6],
    hierarchy_count: usize,
    mappings: [LinuxActiveImageMapping; LINUX_PROCESS_MAPPING_LIMIT],
    mapping_count: usize,
}

impl LinuxProcessAddressSpaceSnapshot {
    pub(crate) fn hierarchy(&self) -> &[memory::FrameLease] {
        &self.hierarchy[..self.hierarchy_count]
    }

    pub(crate) fn mappings(&self) -> &[LinuxActiveImageMapping] {
        &self.mappings[..self.mapping_count]
    }

    fn is_valid(&self) -> bool {
        self.hierarchy_count <= self.hierarchy.len()
            && self.mapping_count <= self.mappings.len()
            && self.hierarchy[self.hierarchy_count..]
                .iter()
                .all(|frame| *frame == memory::FrameLease::EMPTY)
            && self.mappings[self.mapping_count..]
                .iter()
                .all(|mapping| *mapping == LinuxActiveImageMapping::EMPTY)
            && address_space::validate_linux_process_address_space(
                self.hierarchy(),
                self.mappings(),
            )
    }
}

#[derive(Clone, Copy)]
pub(crate) struct LinuxExecMapping {
    pub(crate) address: usize,
    pub(crate) frame: u64,
    pub(crate) writable: bool,
    pub(crate) executable: bool,
}

impl LinuxExecMapping {
    const EMPTY: Self = Self {
        address: 0,
        frame: 0,
        writable: false,
        executable: false,
    };
}

pub(crate) fn linux_exec_mapping_plan(
    plan: &crate::linux_elf::LoadPlan<'_>,
) -> Option<([LinuxExecMapping; LINUX_EXEC_MAPPING_LIMIT], usize)> {
    let segments = [plan.read_only, Some(plan.executable), Some(plan.writable)];
    let mapping_count = segments
        .iter()
        .flatten()
        .try_fold(0_usize, |count, segment| {
            count.checked_add(segment.page_count)
        })?;
    if !(3..=LINUX_EXEC_MAPPING_LIMIT).contains(&mapping_count) {
        return None;
    }
    let mut mappings = [LinuxExecMapping::EMPTY; LINUX_EXEC_MAPPING_LIMIT];
    let mut index = 0;
    for segment in segments.into_iter().flatten() {
        for page in 0..segment.page_count {
            let address = segment
                .virtual_page
                .checked_add((page * PAGE_SIZE) as u64)
                .and_then(|address| usize::try_from(address).ok())?;
            if address == 0
                || mappings[..index]
                    .iter()
                    .any(|mapping| mapping.address == address)
            {
                return None;
            }
            mappings[index] = LinuxExecMapping {
                address,
                frame: 0,
                writable: segment.writable,
                executable: segment.executable,
            };
            index += 1;
        }
    }
    Some((mappings, mapping_count))
}

fn append_linux_active_image_mapping(
    address: u64,
    frame: memory::FrameLease,
    writable: bool,
    executable: bool,
) -> bool {
    if address == 0
        || !address.is_multiple_of(PAGE_SIZE as u64)
        || writable && executable
        || !memory::frames_owned_by(memory::FrameOwner::KERNEL, &[frame])
    {
        return false;
    }
    let count = usize::from(ELF_PROBE_ACTIVE_IMAGE_COUNT.load(Ordering::SeqCst));
    if count >= LINUX_ACTIVE_IMAGE_MAPPING_LIMIT
        || ELF_PROBE_ACTIVE_IMAGE_ADDRESSES[..count]
            .iter()
            .any(|candidate| candidate.load(Ordering::SeqCst) == address)
        || ELF_PROBE_ACTIVE_IMAGE_FRAMES[..count]
            .iter()
            .any(|candidate| candidate.load(Ordering::SeqCst) == frame.address())
        || ELF_PROBE_ACTIVE_IMAGE_ADDRESSES[count].load(Ordering::SeqCst) != 0
        || ELF_PROBE_ACTIVE_IMAGE_FRAMES[count].load(Ordering::SeqCst) != 0
        || ELF_PROBE_ACTIVE_IMAGE_GENERATIONS[count].load(Ordering::SeqCst) != 0
        || ELF_PROBE_ACTIVE_IMAGE_FLAGS[count].load(Ordering::SeqCst) != 0
    {
        return false;
    }
    ELF_PROBE_ACTIVE_IMAGE_ADDRESSES[count].store(address, Ordering::SeqCst);
    ELF_PROBE_ACTIVE_IMAGE_FRAMES[count].store(frame.address(), Ordering::SeqCst);
    ELF_PROBE_ACTIVE_IMAGE_GENERATIONS[count].store(frame.generation(), Ordering::SeqCst);
    ELF_PROBE_ACTIVE_IMAGE_FLAGS[count].store(
        u8::from(writable) | (u8::from(executable) << 1),
        Ordering::SeqCst,
    );
    ELF_PROBE_ACTIVE_IMAGE_COUNT.store((count + 1) as u16, Ordering::SeqCst);
    true
}

fn linux_active_image_mappings() -> Option<(
    [LinuxActiveImageMapping; LINUX_ACTIVE_IMAGE_MAPPING_LIMIT],
    usize,
)> {
    let count = usize::from(ELF_PROBE_ACTIVE_IMAGE_COUNT.load(Ordering::SeqCst));
    if !(1..=LINUX_ACTIVE_IMAGE_MAPPING_LIMIT).contains(&count) {
        return None;
    }
    let mut mappings = [LinuxActiveImageMapping::EMPTY; LINUX_ACTIVE_IMAGE_MAPPING_LIMIT];
    for index in 0..count {
        let address = ELF_PROBE_ACTIVE_IMAGE_ADDRESSES[index].load(Ordering::SeqCst);
        let frame = memory::FrameLease::from_parts(
            ELF_PROBE_ACTIVE_IMAGE_FRAMES[index].load(Ordering::SeqCst),
            ELF_PROBE_ACTIVE_IMAGE_GENERATIONS[index].load(Ordering::SeqCst),
        )?;
        let flags = ELF_PROBE_ACTIVE_IMAGE_FLAGS[index].load(Ordering::SeqCst);
        let address = usize::try_from(address).ok()?;
        if address == 0
            || !address.is_multiple_of(PAGE_SIZE)
            || flags & !3 != 0
            || flags == 3
            || !(memory::frames_owned_by(memory::FrameOwner::KERNEL, &[frame])
                || memory::frame_lease_is_runtime(frame))
            || mappings[..index]
                .iter()
                .any(|prior| prior.address == address || prior.frame == frame)
        {
            return None;
        }
        mappings[index] = LinuxActiveImageMapping {
            address,
            frame,
            writable: flags & 1 != 0,
            executable: flags & 2 != 0,
        };
    }
    if (count..LINUX_ACTIVE_IMAGE_MAPPING_LIMIT).any(|index| {
        ELF_PROBE_ACTIVE_IMAGE_ADDRESSES[index].load(Ordering::SeqCst) != 0
            || ELF_PROBE_ACTIVE_IMAGE_FRAMES[index].load(Ordering::SeqCst) != 0
            || ELF_PROBE_ACTIVE_IMAGE_GENERATIONS[index].load(Ordering::SeqCst) != 0
            || ELF_PROBE_ACTIVE_IMAGE_FLAGS[index].load(Ordering::SeqCst) != 0
    }) {
        return None;
    }
    Some((mappings, count))
}

fn append_linux_process_snapshot_mapping(
    mappings: &mut [LinuxActiveImageMapping; LINUX_PROCESS_MAPPING_LIMIT],
    count: &mut usize,
    mapping: LinuxActiveImageMapping,
) -> bool {
    if *count >= mappings.len()
        || mapping.address == 0
        || !mapping.address.is_multiple_of(PAGE_SIZE)
        || mapping.writable && mapping.executable
        || !(memory::frames_owned_by(memory::FrameOwner::KERNEL, &[mapping.frame])
            || memory::frame_lease_is_runtime(mapping.frame))
        || mappings[..*count]
            .iter()
            .any(|prior| prior.address == mapping.address || prior.frame == mapping.frame)
        || !address_space::current_linux_mapping_matches(
            mapping.address as u64,
            mapping.frame,
            mapping.writable,
            mapping.executable,
        )
    {
        return false;
    }
    mappings[*count] = mapping;
    *count += 1;
    true
}

pub(crate) fn live_linux_process_address_space_snapshot() -> Option<LinuxProcessAddressSpaceSnapshot>
{
    LINUX_PROCESS_SNAPSHOT_STAGE.store(1, Ordering::SeqCst);
    if !ELF_PROBE_ACTIVE.load(Ordering::SeqCst) || current_linux_thread_handle().is_none() {
        return None;
    }
    let mut hierarchy = [memory::FrameLease::EMPTY; 6];
    let mut hierarchy_count = 0;
    let mut hierarchy_ended = false;
    for index in 0..hierarchy.len() {
        let address = ELF_PROBE_PAGE_TABLE_FRAMES[index].load(Ordering::SeqCst);
        let generation = ELF_PROBE_PAGE_TABLE_GENERATIONS[index].load(Ordering::SeqCst);
        match memory::FrameLease::from_parts(address, generation) {
            Some(frame) if !hierarchy_ended => {
                hierarchy[index] = frame;
                hierarchy_count += 1;
            }
            None if address == 0 && generation == 0 => hierarchy_ended = true,
            _ => return None,
        }
    }
    if !(5..=6).contains(&hierarchy_count)
        || hierarchy[0].address() != address_space::current_root_address()
    {
        return None;
    }
    LINUX_PROCESS_SNAPSHOT_STAGE.store(2, Ordering::SeqCst);

    let (active, active_count) = linux_active_image_mappings()?;
    let mut mappings = [LinuxActiveImageMapping::EMPTY; LINUX_PROCESS_MAPPING_LIMIT];
    let mut mapping_count = 0;
    for (index, mapping) in active[..active_count].iter().copied().enumerate() {
        LINUX_PROCESS_SNAPSHOT_STAGE.store(30 + index as u8, Ordering::SeqCst);
        if !append_linux_process_snapshot_mapping(&mut mappings, &mut mapping_count, mapping) {
            if LINUX_RUNTIME_MODE.load(Ordering::SeqCst) {
                serial_write("ginux: snapshot rejected active mapping ");
                serial_write_u64(index as u64);
                serial_write(" address/frame/flags ");
                serial_write_u64(mapping.address as u64);
                serial_write(" ");
                serial_write_u64(mapping.frame.address());
                serial_write(" ");
                serial_write_u64(
                    u64::from(mapping.writable) | (u64::from(mapping.executable) << 1),
                );
                serial_write(" current ");
                serial_write_u64(
                    address_space::current_linux_mapping_frame_address(mapping.address as u64)
                        .unwrap_or(0),
                );
                serial_write("\r\n");
            }
            return None;
        }
    }
    LINUX_PROCESS_SNAPSHOT_STAGE.store(3, Ordering::SeqCst);

    let stack_address =
        usize::try_from(ELF_PROBE_STACK.load(Ordering::SeqCst) & !(PAGE_SIZE as u64 - 1)).ok()?;
    let stack_frame = memory::FrameLease::from_parts(
        ELF_PROBE_STACK_FRAME.load(Ordering::SeqCst),
        ELF_PROBE_STACK_GENERATION.load(Ordering::SeqCst),
    )?;
    if !append_linux_process_snapshot_mapping(
        &mut mappings,
        &mut mapping_count,
        LinuxActiveImageMapping {
            address: stack_address,
            frame: stack_frame,
            writable: true,
            executable: false,
        },
    ) {
        return None;
    }
    if LINUX_RUNTIME_MODE.load(Ordering::SeqCst) {
        for page in 1..LINUX_RUNTIME_STACK_PAGES {
            let address = stack_address.checked_sub(page * PAGE_SIZE)?;
            let frame_address = address_space::current_linux_mapping_frame_address(address as u64)?;
            let frame = memory::FrameLease::from_parts(frame_address, 1)?;
            if !memory::frame_lease_is_runtime(frame)
                || !append_linux_process_snapshot_mapping(
                    &mut mappings,
                    &mut mapping_count,
                    LinuxActiveImageMapping {
                        address,
                        frame,
                        writable: true,
                        executable: false,
                    },
                )
            {
                return None;
            }
        }
    }
    LINUX_PROCESS_SNAPSHOT_STAGE.store(4, Ordering::SeqCst);

    let heap_address = usize::try_from(ELF_PROBE_BRK_LIMIT.load(Ordering::SeqCst).checked_sub(
        (if LINUX_RUNTIME_MODE.load(Ordering::SeqCst) {
            LINUX_RUNTIME_HEAP_PAGES
        } else {
            1
        } * PAGE_SIZE) as u64,
    )?)
    .ok()?;
    let heap_frame = memory::FrameLease::from_parts(
        ELF_PROBE_HEAP_FRAME.load(Ordering::SeqCst),
        ELF_PROBE_HEAP_GENERATION.load(Ordering::SeqCst),
    )?;
    if !append_linux_process_snapshot_mapping(
        &mut mappings,
        &mut mapping_count,
        LinuxActiveImageMapping {
            address: heap_address,
            frame: heap_frame,
            writable: true,
            executable: false,
        },
    ) {
        return None;
    }
    if LINUX_RUNTIME_MODE.load(Ordering::SeqCst) {
        for page in 1..LINUX_RUNTIME_HEAP_PAGES {
            let address = heap_address.checked_add(page * PAGE_SIZE)?;
            let frame_address = address_space::current_linux_mapping_frame_address(address as u64)?;
            let frame = memory::FrameLease::from_parts(frame_address, 1)?;
            if !memory::frame_lease_is_runtime(frame)
                || !append_linux_process_snapshot_mapping(
                    &mut mappings,
                    &mut mapping_count,
                    LinuxActiveImageMapping {
                        address,
                        frame,
                        writable: true,
                        executable: false,
                    },
                )
            {
                return None;
            }
        }
    }
    LINUX_PROCESS_SNAPSHOT_STAGE.store(5, Ordering::SeqCst);

    let tls_addresses = [
        ELF_PROBE_TLS.load(Ordering::SeqCst),
        ELF_PROBE_SIBLING_TLS.load(Ordering::SeqCst),
    ];
    for (index, address) in tls_addresses.into_iter().enumerate() {
        let frame_address = ELF_PROBE_TLS_FRAMES[index].load(Ordering::SeqCst);
        let generation = ELF_PROBE_TLS_GENERATIONS[index].load(Ordering::SeqCst);
        if address == 0 {
            if frame_address != 0 || generation != 0 {
                return None;
            }
            continue;
        }
        let frame = memory::FrameLease::from_parts(frame_address, generation)?;
        if !append_linux_process_snapshot_mapping(
            &mut mappings,
            &mut mapping_count,
            LinuxActiveImageMapping {
                address: usize::try_from(address & !(PAGE_SIZE as u64 - 1)).ok()?,
                frame,
                writable: true,
                executable: false,
            },
        ) {
            return None;
        }
    }
    LINUX_PROCESS_SNAPSHOT_STAGE.store(6, Ordering::SeqCst);

    let sibling_stack = ELF_PROBE_SIBLING_STACK.load(Ordering::SeqCst);
    let sibling_stack_frame = ELF_PROBE_SIBLING_STACK_FRAME.load(Ordering::SeqCst);
    let sibling_stack_generation = ELF_PROBE_SIBLING_STACK_GENERATION.load(Ordering::SeqCst);
    if sibling_stack == 0 {
        if sibling_stack_frame != 0 || sibling_stack_generation != 0 {
            return None;
        }
    } else if !append_linux_process_snapshot_mapping(
        &mut mappings,
        &mut mapping_count,
        LinuxActiveImageMapping {
            address: usize::try_from(sibling_stack).ok()?,
            frame: memory::FrameLease::from_parts(sibling_stack_frame, sibling_stack_generation)?,
            writable: true,
            executable: false,
        },
    ) {
        return None;
    }
    LINUX_PROCESS_SNAPSHOT_STAGE.store(7, Ordering::SeqCst);

    let mmap_base = ELF_PROBE_MMAP.load(Ordering::SeqCst);
    for (slot, generation_cell) in ELF_PROBE_MMAP_GENERATIONS.iter().enumerate() {
        let area = linux_mmap_slot_area_index(slot);
        let frame_address = linux_mmap_slot_frame(slot);
        let generation = generation_cell.load(Ordering::SeqCst);
        let Some(area) = area else {
            if frame_address != 0 || generation != 0 {
                return None;
            }
            continue;
        };
        let protection = u64::from(ELF_PROBE_MMAP_PROTECTION[area].load(Ordering::SeqCst));
        let address = mmap_base.checked_add((slot * PAGE_SIZE) as u64)?;
        if protection == 0 {
            if frame_address != 0
                || generation != 0
                || !address_space::current_linux_page_absent(address)
            {
                return None;
            }
            continue;
        }
        if protection & !(LINUX_PROT_READ | LINUX_PROT_WRITE) != 0
            || !append_linux_process_snapshot_mapping(
                &mut mappings,
                &mut mapping_count,
                LinuxActiveImageMapping {
                    address: usize::try_from(address).ok()?,
                    frame: memory::FrameLease::from_parts(frame_address, generation)?,
                    writable: protection & LINUX_PROT_WRITE != 0,
                    executable: false,
                },
            )
        {
            return None;
        }
    }
    LINUX_PROCESS_SNAPSHOT_STAGE.store(8, Ordering::SeqCst);

    let snapshot = LinuxProcessAddressSpaceSnapshot {
        hierarchy,
        hierarchy_count,
        mappings,
        mapping_count,
    };
    let valid = snapshot.is_valid();
    if !valid {
        serial_write("ginux: Linux process snapshot expected/observed hierarchy/leaves ");
        serial_write_u64(snapshot.hierarchy_count as u64);
        serial_write(" ");
        serial_write_u64(snapshot.mapping_count as u64);
        if let Some((hierarchy, leaves)) =
            address_space::current_linux_process_user_tree_counts(snapshot.hierarchy())
        {
            serial_write(" ");
            serial_write_u64(hierarchy as u64);
            serial_write(" ");
            serial_write_u64(leaves as u64);
        }
        serial_write("\r\n");
    }
    LINUX_PROCESS_SNAPSHOT_STAGE.store(if valid { 9 } else { 8 }, Ordering::SeqCst);
    valid.then_some(snapshot)
}

fn verify_live_linux_process_snapshot_contract(
    snapshot: &LinuxProcessAddressSpaceSnapshot,
) -> bool {
    if !snapshot.is_valid() {
        return false;
    }
    let mut adversarial = *snapshot;
    adversarial.mapping_count = adversarial.mapping_count.saturating_sub(1);
    if adversarial.is_valid() {
        return false;
    }
    adversarial = *snapshot;
    adversarial.mapping_count = adversarial.mappings.len() + 1;
    if adversarial.is_valid() {
        return false;
    }
    adversarial = *snapshot;
    adversarial.mappings[0].writable = !adversarial.mappings[0].writable;
    if adversarial.is_valid() {
        return false;
    }
    adversarial = *snapshot;
    let Some(stale_generation) = adversarial.hierarchy[0].generation().checked_add(1) else {
        return false;
    };
    let Some(stale_lease) =
        memory::FrameLease::from_parts(adversarial.hierarchy[0].address(), stale_generation)
    else {
        return false;
    };
    adversarial.hierarchy[0] = stale_lease;
    !adversarial.is_valid()
}

fn record_live_linux_process_snapshot() -> bool {
    let valid = live_linux_process_address_space_snapshot()
        .is_some_and(|snapshot| verify_live_linux_process_snapshot_contract(&snapshot));
    if !valid {
        serial_write("ginux: Linux process snapshot failed at stage ");
        serial_write_u64(u64::from(
            LINUX_PROCESS_SNAPSHOT_STAGE.load(Ordering::SeqCst),
        ));
        serial_write("\r\n");
    }
    LINUX_PROCESS_SNAPSHOT_VALIDATED.store(valid, Ordering::SeqCst);
    valid
}

fn refresh_live_linux_process_registry(context: LinuxUserContext) -> bool {
    let Some(snapshot) = live_linux_process_address_space_snapshot() else {
        serial_write("ginux: live registry refresh snapshot failed at stage ");
        serial_write_u64(u64::from(
            LINUX_PROCESS_SNAPSHOT_STAGE.load(Ordering::SeqCst),
        ));
        serial_write("\r\n");
        return false;
    };
    linux_process::refresh_live_process_registry(
        &snapshot,
        context
            .registers
            .process_general(context.rax, context.rip, context.rflags),
        context.rip,
        context.rsp,
        context.rflags,
        context.fs_base,
    )
}

fn install_live_linux_process_registry(entry: usize, stack: usize, fs_base: u64) -> bool {
    let Some(snapshot) = live_linux_process_address_space_snapshot() else {
        serial_write("ginux: live process registry install snapshot failed at stage ");
        serial_write_u64(u64::from(
            LINUX_PROCESS_SNAPSHOT_STAGE.load(Ordering::SeqCst),
        ));
        serial_write("\r\n");
        return false;
    };
    let installed = linux_process::install_live_process_registry(
        &snapshot,
        [0; 15],
        entry as u64,
        stack as u64,
        0x202,
        fs_base,
    );
    if !installed {
        serial_write("ginux: live process registry install rejected snapshot\r\n");
    }
    installed
}

fn sync_live_linux_process_address_space(pid: u32) -> bool {
    for index in 0..ELF_PROBE_PAGE_TABLE_FRAMES.len() {
        let Some(frame) = linux_process::live_process_hierarchy_frame(pid, index) else {
            if index < 5 {
                serial_write("ginux: process address-space sync missing hierarchy frame\r\n");
                return false;
            }
            ELF_PROBE_PAGE_TABLE_FRAMES[index].store(0, Ordering::SeqCst);
            ELF_PROBE_PAGE_TABLE_GENERATIONS[index].store(0, Ordering::SeqCst);
            continue;
        };
        ELF_PROBE_PAGE_TABLE_FRAMES[index].store(frame.address(), Ordering::SeqCst);
        ELF_PROBE_PAGE_TABLE_GENERATIONS[index].store(frame.generation(), Ordering::SeqCst);
    }
    let active_count = usize::from(ELF_PROBE_ACTIVE_IMAGE_COUNT.load(Ordering::SeqCst));
    for index in 0..active_count {
        let address = ELF_PROBE_ACTIVE_IMAGE_ADDRESSES[index].load(Ordering::SeqCst) as usize;
        let Some(mapping) = linux_process::live_process_mapping(pid, address) else {
            serial_write("ginux: process address-space sync missing image mapping ");
            serial_write_u64(index as u64);
            serial_write("\r\n");
            return false;
        };
        ELF_PROBE_ACTIVE_IMAGE_FRAMES[index].store(mapping.frame.address(), Ordering::SeqCst);
        ELF_PROBE_ACTIVE_IMAGE_GENERATIONS[index]
            .store(mapping.frame.generation(), Ordering::SeqCst);
    }
    let stack_address = (ELF_PROBE_STACK.load(Ordering::SeqCst) as usize) & !(PAGE_SIZE - 1);
    let heap_address = ELF_PROBE_BRK_LIMIT.load(Ordering::SeqCst) as usize
        - if LINUX_RUNTIME_MODE.load(Ordering::SeqCst) {
            LINUX_RUNTIME_HEAP_PAGES * PAGE_SIZE
        } else {
            PAGE_SIZE
        };
    let (Some(stack), Some(heap)) = (
        linux_process::live_process_mapping(pid, stack_address),
        linux_process::live_process_mapping(pid, heap_address),
    ) else {
        serial_write("ginux: process address-space sync missing stack or heap mapping\r\n");
        return false;
    };
    ELF_PROBE_STACK_FRAME.store(stack.frame.address(), Ordering::SeqCst);
    ELF_PROBE_STACK_GENERATION.store(stack.frame.generation(), Ordering::SeqCst);
    ELF_PROBE_HEAP_FRAME.store(heap.frame.address(), Ordering::SeqCst);
    ELF_PROBE_HEAP_GENERATION.store(heap.frame.generation(), Ordering::SeqCst);
    true
}

#[inline(never)]
fn execute_deferred_live_fork() -> bool {
    if !LINUX_FORK_REQUESTED.swap(false, Ordering::SeqCst) {
        return LINUX_RUNTIME_MODE.load(Ordering::SeqCst)
            || linux_process::verify_live_fork_clone_transaction();
    }
    let parent_mmap_state = capture_linux_mmap_runtime_state();
    let Some(child) = linux_process::begin_live_fork() else {
        if LINUX_RUNTIME_MODE.load(Ordering::SeqCst) {
            serial_write("ginux: runtime fork preparation failed at stage ");
            serial_write_u64(u64::from(linux_process::live_fork_execution_stage()));
            serial_write("\r\n");
        }
        return false;
    };
    if child.pid == 1 || child.general[0] != 0 {
        if LINUX_RUNTIME_MODE.load(Ordering::SeqCst) {
            serial_write("ginux: runtime fork produced invalid child pid/rax ");
            serial_write_u64(u64::from(child.pid));
            serial_write(" ");
            serial_write_u64(child.general[0]);
            serial_write("\r\n");
        }
        return false;
    }
    LINUX_PROCESS_CHILD_EXITED.store(false, Ordering::SeqCst);
    LINUX_PROCESS_CHILD_STATUS.store(0, Ordering::SeqCst);
    LINUX_CURRENT_PROCESS_PID.store(child.pid, Ordering::SeqCst);
    let child_resumed = (!LINUX_RUNTIME_MODE.load(Ordering::SeqCst)
        || sync_live_linux_process_address_space(child.pid))
        && address_space::resume_live_linux_process(
            child.root,
            child.rip,
            child.rsp,
            child.fs_base,
            child.general,
            child.rflags,
        );
    let child_exited = LINUX_PROCESS_CHILD_EXITED.swap(false, Ordering::SeqCst);
    let child_status = LINUX_PROCESS_CHILD_STATUS.swap(0, Ordering::SeqCst);
    restore_linux_mmap_runtime_state(parent_mmap_state);
    if LINUX_RUNTIME_MODE.load(Ordering::SeqCst) && !(child_resumed && child_exited) {
        serial_write("ginux: runtime child return resumed/exited/status ");
        serial_write_u64(u64::from(child_resumed));
        serial_write(" ");
        serial_write_u64(u64::from(child_exited));
        serial_write(" ");
        serial_write_u64(u64::from(child_status));
        serial_write("\r\n");
    }
    let parent = (child_resumed && child_exited)
        .then(|| linux_process::complete_live_child_exit(child.pid, child_status))
        .flatten();
    let Some(parent) = parent else {
        if LINUX_RUNTIME_MODE.load(Ordering::SeqCst) {
            serial_write("ginux: runtime child completion failed at stage ");
            serial_write_u64(u64::from(linux_process::live_fork_execution_stage()));
            serial_write("\r\n");
        }
        return false;
    };
    if parent.pid != 1 || parent.general[0] != u64::from(child.pid) {
        return false;
    }
    LINUX_CURRENT_PROCESS_PID.store(1, Ordering::SeqCst);
    LIVE_FORK_PUBLIC_DIAGNOSTIC.store(0x20, Ordering::SeqCst);
    let parent_resumed = (!LINUX_RUNTIME_MODE.load(Ordering::SeqCst)
        || sync_live_linux_process_address_space(1))
        && address_space::resume_live_linux_process(
            parent.root,
            parent.rip,
            parent.rsp,
            parent.fs_base,
            parent.general,
            parent.rflags,
        );
    let parent_resumed = if parent_resumed
        && !LINUX_RUNTIME_MODE.load(Ordering::SeqCst)
        && ELF_PROBE_SYSTEM_INIT.load(Ordering::SeqCst)
    {
        resume_linux_system_supervisor()
    } else {
        parent_resumed
    };
    if LINUX_RUNTIME_MODE.load(Ordering::SeqCst) {
        return parent_resumed;
    }
    let wait_count = LINUX_PROCESS_WAIT_COUNT.load(Ordering::SeqCst);
    let valid = parent_resumed
        && child_status == LINUX_PROBE_EXIT_STATUS as u8
        && wait_count == 1
        && !LINUX_FORK_REQUESTED.load(Ordering::SeqCst);
    LIVE_FORK_PUBLIC_DIAGNOSTIC.store(
        u8::from(parent_resumed)
            | (u8::from(child_status == LINUX_PROBE_EXIT_STATUS as u8) << 1)
            | (u8::from(wait_count == 1) << 2)
            | (u8::from(ELF_PROBE_COMPLETED.load(Ordering::SeqCst)) << 3)
            | (u8::from(!ELF_PROBE_ACTIVE.load(Ordering::SeqCst)) << 4),
        Ordering::SeqCst,
    );
    if valid {
        serial_write("ginux: Linux PID 1 fork and wait4 lifecycle completed\r\n");
        serial_write("ginux: Linux fork inherited user namespace\r\n");
    }
    valid
}

fn resume_linux_system_supervisor() -> bool {
    if LINUX_SYSTEM_SUPERVISOR_WAIT_COUNT.load(Ordering::SeqCst) != 1
        || LINUX_SYSTEM_SUPERVISOR_SHUTDOWN_COUNT.load(Ordering::SeqCst) != 0
        || current_linux_thread_handle().is_some()
        || linux_process::live_process_status(1)
            != Some(linux_process::LiveProcessStatus {
                pid: 1,
                parent_pid: 0,
                state: linux_process::LiveProcessState::Running,
            })
    {
        return false;
    }
    if !CONTAINER_MANAGEMENT_ENABLED.load(Ordering::SeqCst) {
        return resume_linux_system_supervisor_legacy();
    }
    let Some(slot) = selected_container_slot() else {
        return false;
    };
    CONTAINER_ACTIVE_SLOT.store(slot as u8, Ordering::SeqCst);
    LINUX_EXECUTION_INSTANCE.store(slot as u8, Ordering::SeqCst);
    CONTAINER_CONTROL_STATES[slot]
        .store(control_abi::CONTAINER_STATE_RUNNING as u8, Ordering::SeqCst);
    crate::serial_write("ginux: configured runtime container reached ready state\r\n");
    let controller_completed = address_space::run_installed_interactive_controller();
    let shutdown_requested = CONTAINER_HOST_SHUTDOWN_REQUESTED.load(Ordering::SeqCst);
    if !controller_completed && !shutdown_requested {
        return false;
    }
    if !controller_completed {
        crate::serial_write(
            "ginux: authorized shutdown retained after controller teardown failure\r\n",
        );
    }
    if LINUX_SYSTEM_SUPERVISOR_SHUTDOWN_COUNT.load(Ordering::SeqCst) == 0
        && !shutdown_linux_system_supervisor(false)
    {
        return false;
    }
    LINUX_SYSTEM_SUPERVISOR_SHUTDOWN_COUNT.load(Ordering::SeqCst) == 1
}

fn resume_linux_system_supervisor_legacy() -> bool {
    if !wake_linux_signal_wait(LINUX_SIGTERM, LINUX_SI_KERNEL) {
        return false;
    }
    let Some(thread) = ready_linux_thread_handle() else {
        return false;
    };
    let Some(context) = linux_thread_return_context(thread) else {
        return false;
    };
    let Some(process) = linux_process::dispatch_live_init_with_return(LINUX_SIGTERM) else {
        return false;
    };
    if process.pid != 1
        || context.rax != LINUX_SIGTERM
        || process.general
            != context
                .registers
                .process_general(context.rax, context.rip, context.rflags)
        || process.rip != context.rip
        || process.rsp != context.rsp
        || process.rflags != context.rflags
        || process.fs_base != context.fs_base
        || !switch_linux_thread(thread)
    {
        return false;
    }
    LINUX_SYSTEM_SUPERVISOR_SHUTDOWN_COUNT.fetch_add(1, Ordering::SeqCst);
    address_space::resume_live_linux_process(
        process.root,
        process.rip,
        process.rsp,
        process.fs_base,
        process.general,
        process.rflags,
    )
}

fn shutdown_linux_system_supervisor(failed: bool) -> bool {
    if LINUX_SYSTEM_SUPERVISOR_WAIT_COUNT.load(Ordering::SeqCst) != 1
        || LINUX_SYSTEM_SUPERVISOR_SHUTDOWN_COUNT.load(Ordering::SeqCst) != 0
        || current_linux_thread_handle().is_some()
        || linux_process::live_process_status(1)
            != Some(linux_process::LiveProcessStatus {
                pid: 1,
                parent_pid: 0,
                state: linux_process::LiveProcessState::Running,
            })
        || !wake_linux_signal_wait(LINUX_SIGTERM, LINUX_SI_KERNEL)
    {
        return false;
    }
    let Some(thread) = ready_linux_thread_handle() else {
        return false;
    };
    let Some(context) = linux_thread_return_context(thread) else {
        return false;
    };
    let Some(process) = linux_process::dispatch_live_init_with_return(LINUX_SIGTERM) else {
        return false;
    };
    if process.pid != 1
        || context.rax != LINUX_SIGTERM
        || process.general
            != context
                .registers
                .process_general(context.rax, context.rip, context.rflags)
        || process.rip != context.rip
        || process.rsp != context.rsp
        || process.rflags != context.rflags
        || process.fs_base != context.fs_base
        || !switch_linux_thread(thread)
    {
        return false;
    }
    LINUX_SYSTEM_SUPERVISOR_SHUTDOWN_COUNT.fetch_add(1, Ordering::SeqCst);
    let resumed = address_space::resume_live_linux_process(
        process.root,
        process.rip,
        process.rsp,
        process.fs_base,
        process.general,
        process.rflags,
    );
    if resumed {
        let Some(slot) = active_container_slot() else {
            return false;
        };
        if CONTAINER_CONTROL_STATES[slot].load(Ordering::SeqCst)
            != control_abi::CONTAINER_STATE_RUNNING as u8
        {
            return resumed;
        }
        CONTAINER_CONTROL_STATES[slot].store(
            if failed {
                control_abi::CONTAINER_STATE_FAILED as u8
            } else {
                control_abi::CONTAINER_STATE_STOPPED as u8
            },
            Ordering::SeqCst,
        );
        CONTAINER_ACTIVE_SLOT.store(CONTAINER_SLOT_NONE, Ordering::SeqCst);
        CONTAINER_SELECTED_SLOT.store(CONTAINER_SLOT_NONE, Ordering::SeqCst);
    }
    resumed
}

pub(crate) fn take_container_restart_request() -> bool {
    CONTAINER_RESTART_REQUESTED.swap(false, Ordering::SeqCst)
}

pub(crate) fn take_container_host_shutdown_request() -> bool {
    CONTAINER_HOST_SHUTDOWN_REQUESTED.swap(false, Ordering::SeqCst)
}

pub(crate) fn fail_configured_container() {
    if let Some(slot) = selected_or_active_container_slot() {
        CONTAINER_CONTROL_STATES[slot]
            .store(control_abi::CONTAINER_STATE_FAILED as u8, Ordering::SeqCst);
    }
    CONTAINER_ACTIVE_SLOT.store(CONTAINER_SLOT_NONE, Ordering::SeqCst);
    CONTAINER_SELECTED_SLOT.store(CONTAINER_SLOT_NONE, Ordering::SeqCst);
    LINUX_EXECUTION_INSTANCE.store(LINUX_RUNTIME_HOST_INSTANCE, Ordering::SeqCst);
}

pub(crate) fn begin_configured_container() {
    if let Some(slot) = selected_container_slot() {
        let Some(container_domain) = container_domain_id(slot) else {
            CONTAINER_CONTROL_STATES[slot]
                .store(control_abi::CONTAINER_STATE_FAILED as u8, Ordering::SeqCst);
            return;
        };
        if domain::container_domain_limits(container_domain).is_none() {
            CONTAINER_CONTROL_STATES[slot]
                .store(control_abi::CONTAINER_STATE_FAILED as u8, Ordering::SeqCst);
            return;
        }
        if domain::container_network_mode(container_domain)
            != Some(domain::ContainerNetworkMode::HostShared)
        {
            CONTAINER_CONTROL_STATES[slot]
                .store(control_abi::CONTAINER_STATE_FAILED as u8, Ordering::SeqCst);
            return;
        }
        CONTAINER_ACTIVE_SLOT.store(slot as u8, Ordering::SeqCst);
        LINUX_EXECUTION_INSTANCE.store(slot as u8, Ordering::SeqCst);
        CONTAINER_CONTROL_STATES[slot]
            .store(control_abi::CONTAINER_STATE_RUNNING as u8, Ordering::SeqCst);
    }
}

pub(crate) fn stop_configured_container() {
    if let Some(slot) = active_container_slot() {
        if CONTAINER_CONTROL_STATES[slot].load(Ordering::SeqCst)
            == control_abi::CONTAINER_STATE_RUNNING as u8
        {
            CONTAINER_CONTROL_STATES[slot]
                .store(control_abi::CONTAINER_STATE_STOPPED as u8, Ordering::SeqCst);
        }
    }
    CONTAINER_ACTIVE_SLOT.store(CONTAINER_SLOT_NONE, Ordering::SeqCst);
    CONTAINER_SELECTED_SLOT.store(CONTAINER_SLOT_NONE, Ordering::SeqCst);
    LINUX_EXECUTION_INSTANCE.store(LINUX_RUNTIME_HOST_INSTANCE, Ordering::SeqCst);
}

pub(crate) fn suspend_configured_container() {
    CONTAINER_ACTIVE_SLOT.store(CONTAINER_SLOT_NONE, Ordering::Release);
    CONTAINER_SELECTED_SLOT.store(CONTAINER_SLOT_NONE, Ordering::Release);
    LINUX_EXECUTION_INSTANCE.store(LINUX_RUNTIME_HOST_INSTANCE, Ordering::Release);
}

pub(crate) fn enable_container_management() -> bool {
    CONTAINER_MANAGEMENT_ENABLED
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok()
}

fn linux_active_image_writable_index(address: u64) -> Option<usize> {
    let count = usize::from(ELF_PROBE_ACTIVE_IMAGE_COUNT.load(Ordering::SeqCst));
    if !(1..=LINUX_ACTIVE_IMAGE_MAPPING_LIMIT).contains(&count)
        || address == 0
        || !address.is_multiple_of(PAGE_SIZE as u64)
    {
        return None;
    }
    let index = ELF_PROBE_ACTIVE_IMAGE_ADDRESSES[..count]
        .iter()
        .position(|candidate| candidate.load(Ordering::SeqCst) == address)?;
    let frame = memory::FrameLease::from_parts(
        ELF_PROBE_ACTIVE_IMAGE_FRAMES[index].load(Ordering::SeqCst),
        ELF_PROBE_ACTIVE_IMAGE_GENERATIONS[index].load(Ordering::SeqCst),
    )?;
    (ELF_PROBE_ACTIVE_IMAGE_FLAGS[index].load(Ordering::SeqCst) == 1
        && memory::frames_owned_by(memory::FrameOwner::KERNEL, &[frame]))
    .then_some(index)
}

#[derive(Clone, Copy)]
struct LinuxTlsContext {
    thread_pointer: usize,
    extent: usize,
    extent_size: usize,
}

#[derive(Clone, Copy, Eq, PartialEq)]
struct LinuxThreadHandle {
    slot: u8,
    generation: u32,
    tid: u32,
}

#[derive(Clone, Copy)]
struct LinuxCloneRequest {
    flags: u64,
    child_stack: u64,
    parent_tid: u64,
    child_tid: u64,
    tls: u64,
}

#[derive(Clone, Copy, Eq, PartialEq)]
#[repr(C)]
struct LinuxSyscallFrame {
    r15: u64,
    r14: u64,
    r13: u64,
    r12: u64,
    rbp: u64,
    rbx: u64,
    r10: u64,
    r9: u64,
    r8: u64,
    rdx: u64,
    rsi: u64,
    rdi: u64,
}

impl LinuxSyscallFrame {
    const ZERO: Self = Self::from_array([0; LINUX_SYSCALL_FRAME_REGISTERS]);

    const fn from_array(registers: [u64; LINUX_SYSCALL_FRAME_REGISTERS]) -> Self {
        Self {
            r15: registers[0],
            r14: registers[1],
            r13: registers[2],
            r12: registers[3],
            rbp: registers[4],
            rbx: registers[5],
            r10: registers[6],
            r9: registers[7],
            r8: registers[8],
            rdx: registers[9],
            rsi: registers[10],
            rdi: registers[11],
        }
    }

    const fn into_array(self) -> [u64; LINUX_SYSCALL_FRAME_REGISTERS] {
        [
            self.r15, self.r14, self.r13, self.r12, self.rbp, self.rbx, self.r10, self.r9, self.r8,
            self.rdx, self.rsi, self.rdi,
        ]
    }

    fn argument(self, index: usize) -> Option<u64> {
        self.into_array().get(index.checked_add(6)?).copied()
    }

    fn process_general(self, rax: u64, rcx: u64, r11: u64) -> [u64; 15] {
        [
            rax, self.rbx, rcx, self.rdx, self.rsi, self.rdi, self.rbp, self.r8, self.r9, self.r10,
            r11, self.r12, self.r13, self.r14, self.r15,
        ]
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
struct LinuxSignalContext {
    registers: LinuxSyscallFrame,
    rax: u64,
    rcx: u64,
    r11: u64,
    rip: u64,
    rsp: u64,
    rflags: u64,
    signal_stack: u64,
    signal_restorer: u64,
    signal_mask: u64,
    alt_stack_pointer: u64,
    alt_stack_size: u64,
    alt_stack_flags: u32,
}

impl LinuxSignalContext {
    const ZERO: Self = Self {
        registers: LinuxSyscallFrame::ZERO,
        rax: 0,
        rcx: 0,
        r11: 0,
        rip: 0,
        rsp: 0,
        rflags: 0,
        signal_stack: 0,
        signal_restorer: 0,
        signal_mask: 0,
        alt_stack_pointer: 0,
        alt_stack_size: 0,
        alt_stack_flags: 0,
    };

    fn from_fault(
        context: UserFaultContext,
        signal_stack: u64,
        signal_restorer: u64,
        signal_mask: u64,
        alt_stack_flags: u32,
    ) -> Self {
        Self {
            registers: LinuxSyscallFrame {
                r15: context.r15,
                r14: context.r14,
                r13: context.r13,
                r12: context.r12,
                rbp: context.rbp,
                rbx: context.rbx,
                r10: context.r10,
                r9: context.r9,
                r8: context.r8,
                rdx: context.rdx,
                rsi: context.rsi,
                rdi: context.rdi,
            },
            rax: context.rax,
            rcx: context.rcx,
            r11: context.r11,
            rip: context.rip,
            rsp: context.rsp,
            rflags: context.rflags,
            signal_stack,
            signal_restorer,
            signal_mask,
            alt_stack_pointer: ELF_PROBE_ALT_STACK_POINTER.load(Ordering::SeqCst),
            alt_stack_size: ELF_PROBE_ALT_STACK_SIZE.load(Ordering::SeqCst),
            alt_stack_flags,
        }
    }

    fn user_context(self) -> LinuxUserContext {
        LinuxUserContext {
            registers: self.registers,
            rip: self.rip,
            rsp: self.rsp,
            rflags: self.rflags,
            rax: self.rax,
            fs_base: active_fs_base(),
        }
    }

    fn from_syscall(
        context: LinuxUserContext,
        return_registers: (u64, u64),
        signal_frame: (u64, u64),
        signal_mask: u64,
        alt_stack_flags: u32,
    ) -> Self {
        Self {
            registers: context.registers,
            rax: context.rax,
            rcx: return_registers.0,
            r11: return_registers.1,
            rip: context.rip,
            rsp: context.rsp,
            rflags: context.rflags,
            signal_stack: signal_frame.0,
            signal_restorer: signal_frame.1,
            signal_mask,
            alt_stack_pointer: ELF_PROBE_ALT_STACK_POINTER.load(Ordering::SeqCst),
            alt_stack_size: ELF_PROBE_ALT_STACK_SIZE.load(Ordering::SeqCst),
            alt_stack_flags,
        }
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
#[repr(C)]
struct LinuxSignalStack {
    pointer: u64,
    flags: u32,
    padding: u32,
    size: u64,
}

#[derive(Clone, Copy)]
#[repr(C)]
struct LinuxSignalMachineContext {
    r8: u64,
    r9: u64,
    r10: u64,
    r11: u64,
    r12: u64,
    r13: u64,
    r14: u64,
    r15: u64,
    rdi: u64,
    rsi: u64,
    rbp: u64,
    rbx: u64,
    rdx: u64,
    rax: u64,
    rcx: u64,
    rsp: u64,
    rip: u64,
    rflags: u64,
    cs: u16,
    gs: u16,
    fs: u16,
    ss: u16,
    error: u64,
    trap: u64,
    old_mask: u64,
    cr2: u64,
    fpstate: u64,
    reserved: [u64; 8],
}

#[derive(Clone, Copy)]
#[repr(C)]
struct LinuxSignalUserContext {
    flags: u64,
    link: u64,
    stack: LinuxSignalStack,
    machine: LinuxSignalMachineContext,
    signal_mask: u64,
}

#[derive(Clone, Copy)]
#[repr(C)]
struct LinuxSignalInformation {
    signal: i32,
    error: i32,
    code: i32,
    padding: i32,
    fields: [u8; 112],
}

impl LinuxSignalInformation {
    const ZERO: Self = Self {
        signal: 0,
        error: 0,
        code: 0,
        padding: 0,
        fields: [0; 112],
    };

    const fn generated(signal: u64, code: i32) -> Self {
        let mut fields = [0; 112];
        let sender = 1_u32.to_ne_bytes();
        fields[0] = sender[0];
        fields[1] = sender[1];
        fields[2] = sender[2];
        fields[3] = sender[3];
        Self {
            signal: signal as i32,
            error: 0,
            code,
            padding: 0,
            fields,
        }
    }

    const fn fault(signal: u64, code: i32, address: u64) -> Self {
        let mut fields = [0; 112];
        let address = address.to_ne_bytes();
        fields[0] = address[0];
        fields[1] = address[1];
        fields[2] = address[2];
        fields[3] = address[3];
        fields[4] = address[4];
        fields[5] = address[5];
        fields[6] = address[6];
        fields[7] = address[7];
        Self {
            signal: signal as i32,
            error: 0,
            code,
            padding: 0,
            fields,
        }
    }
}

#[derive(Clone, Copy)]
#[repr(C)]
struct LinuxRtSignalFrame {
    restorer: u64,
    context: LinuxSignalUserContext,
    information: LinuxSignalInformation,
}

struct LinuxRestoredSignalContext {
    user: LinuxUserContext,
    signal_mask: u64,
    rcx: u64,
    r11: u64,
}

const _: [(); 24] = [(); size_of::<LinuxSignalStack>()];
const _: [(); 256] = [(); size_of::<LinuxSignalMachineContext>()];
const _: [(); 304] = [(); size_of::<LinuxSignalUserContext>()];
const _: [(); 128] = [(); size_of::<LinuxSignalInformation>()];
const _: [(); LINUX_SIGNAL_FRAME_SIZE] = [(); size_of::<LinuxRtSignalFrame>()];
const _: [(); LINUX_SIGNAL_FRAME_CONTEXT_OFFSET as usize] =
    [(); core::mem::offset_of!(LinuxRtSignalFrame, context)];
const _: [(); LINUX_SIGNAL_FRAME_INFO_OFFSET as usize] =
    [(); core::mem::offset_of!(LinuxRtSignalFrame, information)];

#[derive(Clone, Copy, Eq, PartialEq)]
struct LinuxUserContext {
    registers: LinuxSyscallFrame,
    rip: u64,
    rsp: u64,
    rflags: u64,
    rax: u64,
    fs_base: u64,
}

#[derive(Clone, Copy)]
struct LinuxCloneResult {
    parent_rax: u64,
    child: LinuxUserContext,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum LinuxClonePublishError {
    InvalidRequest,
    TidFault,
}

impl LinuxUserContext {
    fn is_valid(self) -> bool {
        const REQUIRED_RFLAGS: u64 = (1 << 1) | (1 << 9);
        const FORBIDDEN_RFLAGS: u64 = (3 << 12) | (1 << 14) | (1 << 17);
        self.rip != 0
            && self.rip < 1 << 47
            && self.rsp != 0
            && self.rsp < 1 << 47
            && self.rflags & REQUIRED_RFLAGS == REQUIRED_RFLAGS
            && self.rflags & FORBIDDEN_RFLAGS == 0
            && self.fs_base < 1 << 47
    }
}

impl LinuxTlsContext {
    fn is_valid(self) -> bool {
        if self.extent_size == 0 {
            return self.thread_pointer == 0 && self.extent == 0;
        }
        self.thread_pointer != 0
            && self.extent != 0
            && self.thread_pointer >= self.extent
            && self
                .extent
                .checked_add(self.extent_size)
                .is_some_and(|end| {
                    end <= 1 << 47
                        && self
                            .thread_pointer
                            .checked_add(16)
                            .is_some_and(|tcb_end| tcb_end <= end)
                })
    }
}

pub(crate) fn verify_linux_tls_context_contract() -> bool {
    LinuxTlsContext {
        thread_pointer: 0,
        extent: 0,
        extent_size: 0,
    }
    .is_valid()
        && LinuxTlsContext {
            thread_pointer: 0x1030,
            extent: 0x1000,
            extent_size: 0x40,
        }
        .is_valid()
        && !LinuxTlsContext {
            thread_pointer: 0x1030,
            extent: 0,
            extent_size: 0,
        }
        .is_valid()
        && !LinuxTlsContext {
            thread_pointer: 0,
            extent: 0x1000,
            extent_size: 0x40,
        }
        .is_valid()
        && !LinuxTlsContext {
            thread_pointer: 0x0ff8,
            extent: 0x1000,
            extent_size: 0x40,
        }
        .is_valid()
        && !LinuxTlsContext {
            thread_pointer: 0x1038,
            extent: 0x1000,
            extent_size: 0x40,
        }
        .is_valid()
        && !LinuxTlsContext {
            thread_pointer: (1 << 47) - 8,
            extent: (1 << 47) - 0x20,
            extent_size: 0x40,
        }
        .is_valid()
        && !LinuxTlsContext {
            thread_pointer: usize::MAX - 8,
            extent: usize::MAX - 0x20,
            extent_size: 0x40,
        }
        .is_valid()
}

fn reserve_linux_thread(
    context: LinuxTlsContext,
    stack_base: usize,
    stack_size: usize,
) -> Option<LinuxThreadHandle> {
    if !context.is_valid()
        || stack_base == 0
        || !stack_base.is_multiple_of(PAGE_SIZE)
        || stack_size != PAGE_SIZE && stack_size != LINUX_RUNTIME_STACK_PAGES * PAGE_SIZE
        || stack_base
            .checked_add(stack_size)
            .is_none_or(|end| end > 1 << 47)
    {
        return None;
    }
    let slot = LINUX_THREAD_STATES
        .iter()
        .position(|state| state.load(Ordering::SeqCst) == LINUX_THREAD_FREE)?;
    let generation = LINUX_THREAD_GENERATIONS[slot]
        .load(Ordering::SeqCst)
        .checked_add(1)?;
    let tid = LINUX_NEXT_TID.load(Ordering::SeqCst);
    let next_tid = tid.checked_add(1)?;
    let inherited_signal_mask = current_linux_thread_slot().map_or_else(
        || ELF_PROBE_SIGNAL_MASK.load(Ordering::SeqCst),
        |current| LINUX_THREAD_SIGNAL_MASKS[current].load(Ordering::SeqCst),
    );
    LINUX_THREAD_GENERATIONS[slot].store(generation, Ordering::SeqCst);
    LINUX_THREAD_TIDS[slot].store(tid, Ordering::SeqCst);
    LINUX_THREAD_POINTERS[slot].store(context.thread_pointer as u64, Ordering::SeqCst);
    LINUX_THREAD_TLS_EXTENTS[slot].store(context.extent as u64, Ordering::SeqCst);
    LINUX_THREAD_TLS_SIZES[slot].store(context.extent_size as u64, Ordering::SeqCst);
    LINUX_THREAD_STACK_BASES[slot].store(stack_base as u64, Ordering::SeqCst);
    LINUX_THREAD_STACK_SIZES[slot].store(stack_size as u64, Ordering::SeqCst);
    LINUX_THREAD_CLEAR_CHILD_TIDS[slot].store(0, Ordering::SeqCst);
    LINUX_THREAD_ROBUST_LISTS[slot].store(0, Ordering::SeqCst);
    LINUX_THREAD_SIGNAL_MASKS[slot].store(inherited_signal_mask, Ordering::SeqCst);
    LINUX_THREAD_WAIT_KINDS[slot].store(LINUX_THREAD_WAIT_NONE, Ordering::SeqCst);
    LINUX_THREAD_WAIT_KEYS[slot].store(0, Ordering::SeqCst);
    LINUX_THREAD_WAIT_OUTPUTS[slot].store(0, Ordering::SeqCst);
    LINUX_THREAD_WAIT_DEADLINES[slot].store(0, Ordering::SeqCst);
    LINUX_THREAD_WAIT_PRIOR_SIGNAL_MASKS[slot].store(0, Ordering::SeqCst);
    LINUX_NEXT_TID.store(next_tid, Ordering::SeqCst);
    LINUX_THREAD_STATES[slot].store(LINUX_THREAD_READY, Ordering::SeqCst);
    Some(LinuxThreadHandle {
        slot: slot as u8,
        generation,
        tid,
    })
}

fn linux_thread_context(handle: LinuxThreadHandle) -> Option<LinuxTlsContext> {
    let slot = usize::from(handle.slot);
    if slot >= LINUX_THREAD_LIMIT
        || LINUX_THREAD_STATES[slot].load(Ordering::SeqCst) == LINUX_THREAD_FREE
        || LINUX_THREAD_GENERATIONS[slot].load(Ordering::SeqCst) != handle.generation
        || LINUX_THREAD_TIDS[slot].load(Ordering::SeqCst) != handle.tid
    {
        return None;
    }
    Some(LinuxTlsContext {
        thread_pointer: usize::try_from(LINUX_THREAD_POINTERS[slot].load(Ordering::SeqCst)).ok()?,
        extent: usize::try_from(LINUX_THREAD_TLS_EXTENTS[slot].load(Ordering::SeqCst)).ok()?,
        extent_size: usize::try_from(LINUX_THREAD_TLS_SIZES[slot].load(Ordering::SeqCst)).ok()?,
    })
}

fn construct_linux_clone_context(
    handle: LinuxThreadHandle,
    request: LinuxCloneRequest,
    parent_registers: LinuxSyscallFrame,
    return_rip: u64,
    return_rflags: u64,
    parent_writable_base: u64,
    parent_writable_size: usize,
) -> Option<LinuxCloneResult> {
    let slot = usize::from(handle.slot);
    let context = linux_thread_context(handle)?;
    if request.flags != LINUX_BOUNDED_THREAD_CLONE_FLAGS
        || slot >= LINUX_THREAD_LIMIT
        || LINUX_THREAD_STATES[slot].load(Ordering::SeqCst) != LINUX_THREAD_READY
        || current_linux_thread_slot() == Some(slot)
        || request.tls != context.thread_pointer as u64
        || request.child_stack & 15 != 0
        || LINUX_THREAD_STACK_BASES[slot]
            .load(Ordering::SeqCst)
            .checked_add(LINUX_THREAD_STACK_SIZES[slot].load(Ordering::SeqCst))
            != Some(request.child_stack)
        || request.parent_tid & 3 != 0
        || request.child_tid & 3 != 0
        || !linux_range_contains(
            parent_writable_base,
            parent_writable_size as u64,
            request.parent_tid,
            size_of::<u32>(),
        )
        || !linux_range_contains(
            parent_writable_base,
            parent_writable_size as u64,
            request.child_tid,
            size_of::<u32>(),
        )
    {
        return None;
    }
    let child = LinuxUserContext {
        registers: parent_registers,
        rip: return_rip,
        rsp: request.child_stack,
        rflags: return_rflags,
        rax: 0,
        fs_base: request.tls,
    };
    if !child.is_valid() {
        return None;
    }
    for (saved, value) in LINUX_THREAD_REGISTERS[slot]
        .iter()
        .zip(child.registers.into_array())
    {
        saved.store(value, Ordering::SeqCst);
    }
    LINUX_THREAD_RIPS[slot].store(child.rip, Ordering::SeqCst);
    LINUX_THREAD_RSPS[slot].store(child.rsp, Ordering::SeqCst);
    LINUX_THREAD_RFLAGS[slot].store(child.rflags, Ordering::SeqCst);
    LINUX_THREAD_RAXS[slot].store(child.rax, Ordering::SeqCst);
    LINUX_THREAD_RCXS[slot].store(child.rip, Ordering::SeqCst);
    LINUX_THREAD_R11S[slot].store(child.rflags, Ordering::SeqCst);
    LINUX_THREAD_CLEAR_CHILD_TIDS[slot].store(request.child_tid, Ordering::SeqCst);
    Some(LinuxCloneResult {
        parent_rax: u64::from(handle.tid),
        child,
    })
}

fn publish_linux_clone(
    handle: LinuxThreadHandle,
    request: LinuxCloneRequest,
    parent_registers: LinuxSyscallFrame,
    return_rip: u64,
    return_rflags: u64,
    parent_writable_base: u64,
    parent_writable_size: usize,
) -> Result<LinuxCloneResult, LinuxClonePublishError> {
    let result = construct_linux_clone_context(
        handle,
        request,
        parent_registers,
        return_rip,
        return_rflags,
        parent_writable_base,
        parent_writable_size,
    )
    .ok_or(LinuxClonePublishError::InvalidRequest)?;
    if !linux_writable_user_range_contains(request.parent_tid, size_of::<u32>())
        || !linux_writable_user_range_contains(request.child_tid, size_of::<u32>())
    {
        clear_linux_thread_execution(usize::from(handle.slot));
        return Err(LinuxClonePublishError::TidFault);
    }
    // SAFETY: construction and the active current-thread copy boundary admit
    // both aligned words before either write. Interrupts remain disabled, so
    // their mappings cannot change between validation and publication.
    unsafe {
        with_supervisor_user_access(|| {
            (request.parent_tid as *mut u32).write_volatile(handle.tid);
            (request.child_tid as *mut u32).write_volatile(handle.tid);
        });
    }
    Ok(result)
}

fn linux_thread_return_context(handle: LinuxThreadHandle) -> Option<LinuxUserContext> {
    let slot = usize::from(handle.slot);
    let tls = linux_thread_context(handle)?;
    let mut registers = [0_u64; LINUX_SYSCALL_FRAME_REGISTERS];
    for (value, saved) in registers.iter_mut().zip(&LINUX_THREAD_REGISTERS[slot]) {
        *value = saved.load(Ordering::SeqCst);
    }
    let context = LinuxUserContext {
        registers: LinuxSyscallFrame::from_array(registers),
        rip: LINUX_THREAD_RIPS[slot].load(Ordering::SeqCst),
        rsp: LINUX_THREAD_RSPS[slot].load(Ordering::SeqCst),
        rflags: LINUX_THREAD_RFLAGS[slot].load(Ordering::SeqCst),
        rax: LINUX_THREAD_RAXS[slot].load(Ordering::SeqCst),
        fs_base: tls.thread_pointer as u64,
    };
    context.is_valid().then_some(context)
}

fn save_linux_thread_return_context(handle: LinuxThreadHandle, context: LinuxUserContext) -> bool {
    let slot = usize::from(handle.slot);
    let Some(tls) = linux_thread_context(handle) else {
        return false;
    };
    let stack_base = LINUX_THREAD_STACK_BASES[slot].load(Ordering::SeqCst);
    let Some(stack_end) =
        stack_base.checked_add(LINUX_THREAD_STACK_SIZES[slot].load(Ordering::SeqCst))
    else {
        return false;
    };
    if !context.is_valid()
        || context.fs_base != tls.thread_pointer as u64
        || context.rsp < stack_base
        || context.rsp > stack_end
    {
        return false;
    }
    for (saved, value) in LINUX_THREAD_REGISTERS[slot]
        .iter()
        .zip(context.registers.into_array())
    {
        saved.store(value, Ordering::SeqCst);
    }
    LINUX_THREAD_RIPS[slot].store(context.rip, Ordering::SeqCst);
    LINUX_THREAD_RSPS[slot].store(context.rsp, Ordering::SeqCst);
    LINUX_THREAD_RFLAGS[slot].store(context.rflags, Ordering::SeqCst);
    LINUX_THREAD_RAXS[slot].store(context.rax, Ordering::SeqCst);
    LINUX_THREAD_RCXS[slot].store(context.rip, Ordering::SeqCst);
    LINUX_THREAD_R11S[slot].store(context.rflags, Ordering::SeqCst);
    true
}

fn restore_linux_thread_return_context(
    handle: LinuxThreadHandle,
    snapshot: Option<LinuxUserContext>,
) {
    if let Some(context) = snapshot {
        let _ = save_linux_thread_return_context(handle, context);
    } else {
        clear_linux_thread_execution(usize::from(handle.slot));
    }
}

fn install_linux_thread_return(
    handle: LinuxThreadHandle,
    registers: &mut LinuxSyscallFrame,
    rip: &mut u64,
    rsp: &mut u64,
    rflags: &mut u64,
    rax: &mut u64,
) -> bool {
    let Some(context) = linux_thread_return_context(handle) else {
        return false;
    };
    if !switch_linux_thread(handle) {
        return false;
    }
    *registers = context.registers;
    *rip = context.rip;
    *rsp = context.rsp;
    *rflags = context.rflags;
    *rax = context.rax;
    true
}

fn switch_linux_thread(handle: LinuxThreadHandle) -> bool {
    let slot = usize::from(handle.slot);
    let Some(context) = linux_thread_context(handle) else {
        return false;
    };
    if LINUX_THREAD_STATES[slot].load(Ordering::SeqCst) != LINUX_THREAD_READY
        || !install_linux_tls_context(context)
    {
        return false;
    }
    let prior = LINUX_CURRENT_THREAD_SLOT.load(Ordering::SeqCst);
    if prior != LINUX_THREAD_SLOT_NONE {
        let prior = usize::from(prior);
        if prior >= LINUX_THREAD_LIMIT
            || LINUX_THREAD_STATES[prior].load(Ordering::SeqCst) != LINUX_THREAD_RUNNING
        {
            return false;
        }
        LINUX_THREAD_SIGNAL_MASKS[prior].store(
            ELF_PROBE_SIGNAL_MASK.load(Ordering::SeqCst),
            Ordering::SeqCst,
        );
        LINUX_THREAD_STATES[prior].store(LINUX_THREAD_READY, Ordering::SeqCst);
    }
    ELF_PROBE_SIGNAL_MASK.store(
        LINUX_THREAD_SIGNAL_MASKS[slot].load(Ordering::SeqCst),
        Ordering::SeqCst,
    );
    LINUX_THREAD_STATES[slot].store(LINUX_THREAD_RUNNING, Ordering::SeqCst);
    LINUX_CURRENT_THREAD_SLOT.store(handle.slot, Ordering::SeqCst);
    true
}

fn retire_linux_thread(handle: LinuxThreadHandle) -> bool {
    let slot = usize::from(handle.slot);
    if linux_thread_context(handle).is_none()
        || LINUX_CURRENT_THREAD_SLOT.load(Ordering::SeqCst) == handle.slot
        || LINUX_THREAD_STATES[slot].load(Ordering::SeqCst) != LINUX_THREAD_READY
    {
        return false;
    }
    clear_linux_thread_slot(slot);
    true
}

fn retire_current_linux_thread() -> bool {
    let slot = LINUX_CURRENT_THREAD_SLOT.swap(LINUX_THREAD_SLOT_NONE, Ordering::SeqCst);
    let slot = usize::from(slot);
    if slot >= LINUX_THREAD_LIMIT
        || LINUX_THREAD_STATES[slot].load(Ordering::SeqCst) != LINUX_THREAD_RUNNING
    {
        return false;
    }
    clear_linux_thread_slot(slot);
    true
}

fn retire_all_linux_threads() {
    LINUX_CURRENT_THREAD_SLOT.store(LINUX_THREAD_SLOT_NONE, Ordering::SeqCst);
    for slot in 0..LINUX_THREAD_LIMIT {
        clear_linux_thread_slot(slot);
    }
}

fn clear_linux_thread_slot(slot: usize) {
    let stack_base = LINUX_THREAD_STACK_BASES[slot].load(Ordering::SeqCst);
    let stack_size = LINUX_THREAD_STACK_SIZES[slot].load(Ordering::SeqCst);
    if stack_base != 0 && stack_size == PAGE_SIZE as u64 {
        // SAFETY: each live slot owns this exact writable user stack mapping.
        // Retirement runs on the kernel syscall stack with interrupts disabled.
        unsafe {
            with_supervisor_user_access(|| {
                (stack_base as *mut u8).write_bytes(0, PAGE_SIZE);
            });
        }
    }
    LINUX_THREAD_TIDS[slot].store(0, Ordering::SeqCst);
    LINUX_THREAD_POINTERS[slot].store(0, Ordering::SeqCst);
    LINUX_THREAD_TLS_EXTENTS[slot].store(0, Ordering::SeqCst);
    LINUX_THREAD_TLS_SIZES[slot].store(0, Ordering::SeqCst);
    LINUX_THREAD_STACK_BASES[slot].store(0, Ordering::SeqCst);
    LINUX_THREAD_STACK_SIZES[slot].store(0, Ordering::SeqCst);
    clear_linux_thread_execution(slot);
    LINUX_THREAD_ROBUST_LISTS[slot].store(0, Ordering::SeqCst);
    LINUX_THREAD_SIGNAL_MASKS[slot].store(0, Ordering::SeqCst);
    LINUX_THREAD_WAIT_KINDS[slot].store(LINUX_THREAD_WAIT_NONE, Ordering::SeqCst);
    LINUX_THREAD_WAIT_KEYS[slot].store(0, Ordering::SeqCst);
    LINUX_THREAD_WAIT_OUTPUTS[slot].store(0, Ordering::SeqCst);
    LINUX_THREAD_WAIT_DEADLINES[slot].store(0, Ordering::SeqCst);
    LINUX_THREAD_WAIT_PRIOR_SIGNAL_MASKS[slot].store(0, Ordering::SeqCst);
    for descriptor in &LINUX_THREAD_POLL_DESCRIPTORS[slot] {
        descriptor.store(0, Ordering::SeqCst);
    }
    for events in &LINUX_THREAD_POLL_EVENTS[slot] {
        events.store(0, Ordering::SeqCst);
    }
    clear_linux_iov_snapshot(slot);
    LINUX_THREAD_STATES[slot].store(LINUX_THREAD_FREE, Ordering::SeqCst);
}

fn clear_linux_thread_execution(slot: usize) {
    for register in &LINUX_THREAD_REGISTERS[slot] {
        register.store(0, Ordering::SeqCst);
    }
    LINUX_THREAD_RIPS[slot].store(0, Ordering::SeqCst);
    LINUX_THREAD_RSPS[slot].store(0, Ordering::SeqCst);
    LINUX_THREAD_RFLAGS[slot].store(0, Ordering::SeqCst);
    LINUX_THREAD_RAXS[slot].store(0, Ordering::SeqCst);
    LINUX_THREAD_RCXS[slot].store(0, Ordering::SeqCst);
    LINUX_THREAD_R11S[slot].store(0, Ordering::SeqCst);
    LINUX_THREAD_CLEAR_CHILD_TIDS[slot].store(0, Ordering::SeqCst);
}

fn current_linux_thread_slot() -> Option<usize> {
    let slot = usize::from(LINUX_CURRENT_THREAD_SLOT.load(Ordering::SeqCst));
    (slot < LINUX_THREAD_LIMIT
        && LINUX_THREAD_STATES[slot].load(Ordering::SeqCst) == LINUX_THREAD_RUNNING)
        .then_some(slot)
}

fn store_current_linux_signal_mask(mask: u64) {
    ELF_PROBE_SIGNAL_MASK.store(mask, Ordering::SeqCst);
    if let Some(slot) = current_linux_thread_slot() {
        LINUX_THREAD_SIGNAL_MASKS[slot].store(mask, Ordering::SeqCst);
    }
}

fn linux_thread_handle(slot: usize) -> Option<LinuxThreadHandle> {
    if slot >= LINUX_THREAD_LIMIT
        || LINUX_THREAD_STATES[slot].load(Ordering::SeqCst) == LINUX_THREAD_FREE
    {
        return None;
    }
    let generation = LINUX_THREAD_GENERATIONS[slot].load(Ordering::SeqCst);
    let tid = LINUX_THREAD_TIDS[slot].load(Ordering::SeqCst);
    (generation != 0 && tid != 0).then_some(LinuxThreadHandle {
        slot: slot as u8,
        generation,
        tid,
    })
}

fn current_linux_thread_handle() -> Option<LinuxThreadHandle> {
    linux_thread_handle(current_linux_thread_slot()?)
}

fn linux_thread_handle_by_tid(tid: u64) -> Option<LinuxThreadHandle> {
    let tid = u32::try_from(tid).ok()?;
    (0..LINUX_THREAD_LIMIT).find_map(|slot| {
        (LINUX_THREAD_TIDS[slot].load(Ordering::SeqCst) == tid)
            .then(|| linux_thread_handle(slot))
            .flatten()
    })
}

fn ready_linux_thread_handle() -> Option<LinuxThreadHandle> {
    (0..LINUX_THREAD_LIMIT).find_map(|slot| {
        (LINUX_THREAD_STATES[slot].load(Ordering::SeqCst) == LINUX_THREAD_READY)
            .then(|| linux_thread_handle(slot))
            .flatten()
            .filter(|handle| linux_thread_return_context(*handle).is_some())
    })
}

fn block_current_linux_thread(kind: u8, key: u64, output: u64, deadline: u64) -> bool {
    let Some(slot) = current_linux_thread_slot() else {
        return false;
    };
    if !matches!(
        kind,
        LINUX_THREAD_WAIT_FUTEX
            | LINUX_THREAD_WAIT_SIGNAL
            | LINUX_THREAD_WAIT_SLEEP
            | LINUX_THREAD_WAIT_POLL
            | LINUX_THREAD_WAIT_EPOLL
            | LINUX_THREAD_WAIT_EVENTFD_READ
            | LINUX_THREAD_WAIT_PPOLL
            | LINUX_THREAD_WAIT_EPOLL_PWAIT
            | LINUX_THREAD_WAIT_PIPE_READ
            | LINUX_THREAD_WAIT_TIMERFD_READ
            | LINUX_THREAD_WAIT_PIPE_WRITE
            | LINUX_THREAD_WAIT_EVENTFD_WRITE
    ) || key == 0
        || LINUX_THREAD_STATES[slot].load(Ordering::SeqCst) != LINUX_THREAD_RUNNING
        || LINUX_THREAD_WAIT_KINDS[slot].load(Ordering::SeqCst) != LINUX_THREAD_WAIT_NONE
        || LINUX_THREAD_WAIT_KEYS[slot].load(Ordering::SeqCst) != 0
        || LINUX_THREAD_WAIT_OUTPUTS[slot].load(Ordering::SeqCst) != 0
        || LINUX_THREAD_WAIT_DEADLINES[slot].load(Ordering::SeqCst) != 0
    {
        return false;
    }
    LINUX_THREAD_WAIT_KEYS[slot].store(key, Ordering::SeqCst);
    LINUX_THREAD_WAIT_OUTPUTS[slot].store(output, Ordering::SeqCst);
    LINUX_THREAD_WAIT_DEADLINES[slot].store(deadline, Ordering::SeqCst);
    LINUX_THREAD_WAIT_KINDS[slot].store(kind, Ordering::SeqCst);
    LINUX_THREAD_STATES[slot].store(LINUX_THREAD_BLOCKED, Ordering::SeqCst);
    LINUX_CURRENT_THREAD_SLOT.store(LINUX_THREAD_SLOT_NONE, Ordering::SeqCst);
    true
}

fn wake_linux_futex(address: u64, limit: u32) -> u32 {
    if address == 0 || limit == 0 {
        return 0;
    }
    let mut woken = 0_u32;
    for slot in 0..LINUX_THREAD_LIMIT {
        if woken == limit {
            break;
        }
        if LINUX_THREAD_STATES[slot].load(Ordering::SeqCst) == LINUX_THREAD_BLOCKED
            && LINUX_THREAD_WAIT_KINDS[slot].load(Ordering::SeqCst) == LINUX_THREAD_WAIT_FUTEX
            && LINUX_THREAD_WAIT_KEYS[slot].load(Ordering::SeqCst) == address
        {
            LINUX_THREAD_WAIT_KINDS[slot].store(LINUX_THREAD_WAIT_NONE, Ordering::SeqCst);
            LINUX_THREAD_WAIT_KEYS[slot].store(0, Ordering::SeqCst);
            LINUX_THREAD_WAIT_OUTPUTS[slot].store(0, Ordering::SeqCst);
            LINUX_THREAD_WAIT_DEADLINES[slot].store(0, Ordering::SeqCst);
            LINUX_THREAD_STATES[slot].store(LINUX_THREAD_READY, Ordering::SeqCst);
            woken += 1;
        }
    }
    woken
}

fn current_linux_tid() -> Option<u32> {
    current_linux_thread_slot()
        .map(|slot| LINUX_THREAD_TIDS[slot].load(Ordering::SeqCst))
        .filter(|tid| *tid != 0)
}

fn set_current_linux_clear_child_tid(address: u64) -> bool {
    current_linux_thread_slot().is_some_and(|slot| {
        LINUX_THREAD_CLEAR_CHILD_TIDS[slot].store(address, Ordering::SeqCst);
        true
    })
}

fn current_linux_clear_child_tid() -> Option<u64> {
    current_linux_thread_slot()
        .map(|slot| LINUX_THREAD_CLEAR_CHILD_TIDS[slot].load(Ordering::SeqCst))
}

fn set_current_linux_robust_list(address: u64) -> bool {
    current_linux_thread_slot().is_some_and(|slot| {
        LINUX_THREAD_ROBUST_LISTS[slot].store(address, Ordering::SeqCst);
        true
    })
}

fn current_linux_robust_list() -> Option<u64> {
    current_linux_thread_slot().map(|slot| LINUX_THREAD_ROBUST_LISTS[slot].load(Ordering::SeqCst))
}

fn finish_linux_thread_table() -> bool {
    let clean = LINUX_CURRENT_THREAD_SLOT.load(Ordering::SeqCst) == LINUX_THREAD_SLOT_NONE
        && LINUX_THREAD_STATES
            .iter()
            .all(|state| state.load(Ordering::SeqCst) == LINUX_THREAD_FREE)
        && LINUX_THREAD_TIDS
            .iter()
            .all(|tid| tid.load(Ordering::SeqCst) == 0)
        && LINUX_THREAD_POINTERS
            .iter()
            .all(|value| value.load(Ordering::SeqCst) == 0)
        && LINUX_THREAD_TLS_EXTENTS
            .iter()
            .all(|value| value.load(Ordering::SeqCst) == 0)
        && LINUX_THREAD_TLS_SIZES
            .iter()
            .all(|value| value.load(Ordering::SeqCst) == 0)
        && LINUX_THREAD_STACK_BASES
            .iter()
            .all(|value| value.load(Ordering::SeqCst) == 0)
        && LINUX_THREAD_STACK_SIZES
            .iter()
            .all(|value| value.load(Ordering::SeqCst) == 0)
        && LINUX_THREAD_RIPS
            .iter()
            .all(|value| value.load(Ordering::SeqCst) == 0)
        && LINUX_THREAD_RSPS
            .iter()
            .all(|value| value.load(Ordering::SeqCst) == 0)
        && LINUX_THREAD_RFLAGS
            .iter()
            .all(|value| value.load(Ordering::SeqCst) == 0)
        && LINUX_THREAD_RAXS
            .iter()
            .all(|value| value.load(Ordering::SeqCst) == 0)
        && LINUX_THREAD_RCXS
            .iter()
            .all(|value| value.load(Ordering::SeqCst) == 0)
        && LINUX_THREAD_R11S
            .iter()
            .all(|value| value.load(Ordering::SeqCst) == 0)
        && LINUX_THREAD_REGISTERS.iter().all(|registers| {
            registers
                .iter()
                .all(|value| value.load(Ordering::SeqCst) == 0)
        })
        && LINUX_THREAD_CLEAR_CHILD_TIDS
            .iter()
            .all(|value| value.load(Ordering::SeqCst) == 0)
        && LINUX_THREAD_ROBUST_LISTS
            .iter()
            .all(|value| value.load(Ordering::SeqCst) == 0)
        && LINUX_THREAD_SIGNAL_MASKS
            .iter()
            .all(|value| value.load(Ordering::SeqCst) == 0)
        && LINUX_THREAD_WAIT_KINDS
            .iter()
            .all(|value| value.load(Ordering::SeqCst) == LINUX_THREAD_WAIT_NONE)
        && LINUX_THREAD_WAIT_KEYS
            .iter()
            .all(|value| value.load(Ordering::SeqCst) == 0)
        && LINUX_THREAD_WAIT_OUTPUTS
            .iter()
            .all(|value| value.load(Ordering::SeqCst) == 0)
        && LINUX_THREAD_WAIT_DEADLINES
            .iter()
            .all(|value| value.load(Ordering::SeqCst) == 0)
        && LINUX_THREAD_WAIT_PRIOR_SIGNAL_MASKS
            .iter()
            .all(|value| value.load(Ordering::SeqCst) == 0)
        && LINUX_THREAD_POLL_DESCRIPTORS.iter().all(|descriptors| {
            descriptors
                .iter()
                .all(|value| value.load(Ordering::SeqCst) == 0)
        })
        && LINUX_THREAD_POLL_EVENTS
            .iter()
            .all(|events| events.iter().all(|value| value.load(Ordering::SeqCst) == 0))
        && LINUX_THREAD_IOV_ADDRESSES.iter().all(|addresses| {
            addresses
                .iter()
                .all(|value| value.load(Ordering::SeqCst) == 0)
        })
        && LINUX_THREAD_IOV_LENGTHS.iter().all(|lengths| {
            lengths
                .iter()
                .all(|value| value.load(Ordering::SeqCst) == 0)
        });
    for slot in 0..LINUX_THREAD_LIMIT {
        clear_linux_thread_slot(slot);
    }
    LINUX_CURRENT_THREAD_SLOT.store(LINUX_THREAD_SLOT_NONE, Ordering::SeqCst);
    LINUX_NEXT_TID.store(1, Ordering::SeqCst);
    clean
}

pub(crate) struct LinuxElfResult {
    pub(crate) valid: bool,
    pub(crate) suspended: bool,
    pub(crate) exec_replaced: bool,
    pub(crate) exec_node: u8,
    pub(crate) active_image_mapping_count: usize,
    pub(crate) dependency_bases: [u64; LINUX_DEPENDENCY_COUNT],
    pub(crate) main_frames: [Option<memory::FrameLease>; LINUX_MAIN_MAPPING_COUNT],
    pub(crate) dependency_frames:
        [[Option<memory::FrameLease>; LINUX_DEPENDENCY_MAPPING_COUNT]; LINUX_DEPENDENCY_COUNT],
    pub(crate) exec_mapping_count: usize,
    pub(crate) exec_frames: [Option<memory::FrameLease>; LINUX_EXEC_MAPPING_LIMIT],
    pub(crate) retired_image_count: usize,
    pub(crate) retired_image_frames: [Option<memory::FrameLease>; LINUX_ACTIVE_IMAGE_MAPPING_LIMIT],
}

impl LinuxElfResult {
    pub(crate) const FAILED: Self = Self {
        valid: false,
        suspended: false,
        exec_replaced: false,
        exec_node: 0,
        active_image_mapping_count: 0,
        dependency_bases: [0; LINUX_DEPENDENCY_COUNT],
        main_frames: [None; LINUX_MAIN_MAPPING_COUNT],
        dependency_frames: [[None; LINUX_DEPENDENCY_MAPPING_COUNT]; LINUX_DEPENDENCY_COUNT],
        exec_mapping_count: 0,
        exec_frames: [None; LINUX_EXEC_MAPPING_LIMIT],
        retired_image_count: 0,
        retired_image_frames: [None; LINUX_ACTIVE_IMAGE_MAPPING_LIMIT],
    };
}

#[inline(never)]
fn finalize_linux_exec_heap_frame(
    address: usize,
    initial: memory::FrameLease,
    replaced: bool,
) -> bool {
    let current = memory::FrameLease::from_parts(
        ELF_PROBE_HEAP_FRAME.swap(0, Ordering::SeqCst),
        ELF_PROBE_HEAP_GENERATION.swap(0, Ordering::SeqCst),
    );
    current.is_some_and(|frame| {
        let valid =
            address_space::current_linux_mapping_matches(address as u64, frame, true, false)
                && if replaced {
                    frame != initial
                        && !memory::frames_owned_by(memory::FrameOwner::KERNEL, &[initial])
                        && ELF_PROBE_RECLAIM_HEAP_FRAME.load(Ordering::SeqCst) == 0
                        && ELF_PROBE_RECLAIM_HEAP_GENERATION.load(Ordering::SeqCst) == 0
                } else {
                    frame == initial
                };
        if valid && replaced {
            ELF_PROBE_RECLAIM_HEAP_FRAME.store(frame.address(), Ordering::SeqCst);
            ELF_PROBE_RECLAIM_HEAP_GENERATION.store(frame.generation(), Ordering::SeqCst);
        }
        valid
    })
}

pub(crate) fn take_linux_exec_heap_frame() -> Option<memory::FrameLease> {
    memory::FrameLease::from_parts(
        ELF_PROBE_RECLAIM_HEAP_FRAME.swap(0, Ordering::SeqCst),
        ELF_PROBE_RECLAIM_HEAP_GENERATION.swap(0, Ordering::SeqCst),
    )
}

#[inline(never)]
fn finalize_linux_exec_tls_frames(
    addresses: [usize; 2],
    initial: [memory::FrameLease; 2],
    replaced: bool,
) -> bool {
    let mut valid = true;
    for index in 0..2 {
        let current = memory::FrameLease::from_parts(
            ELF_PROBE_TLS_FRAMES[index].swap(0, Ordering::SeqCst),
            ELF_PROBE_TLS_GENERATIONS[index].swap(0, Ordering::SeqCst),
        );
        if initial[index] == memory::FrameLease::EMPTY {
            valid &= !replaced && current.is_none();
            continue;
        }
        valid &= current.is_some_and(|frame| {
            let replacement_valid = frame != initial[index]
                && !memory::frames_owned_by(memory::FrameOwner::KERNEL, &[initial[index]])
                && ELF_PROBE_RECLAIM_TLS_FRAMES[index].load(Ordering::SeqCst) == 0
                && ELF_PROBE_RECLAIM_TLS_GENERATIONS[index].load(Ordering::SeqCst) == 0;
            let mapping_valid = address_space::current_linux_mapping_matches(
                addresses[index] as u64,
                frame,
                true,
                false,
            );
            if replaced && replacement_valid {
                ELF_PROBE_RECLAIM_TLS_FRAMES[index].store(frame.address(), Ordering::SeqCst);
                ELF_PROBE_RECLAIM_TLS_GENERATIONS[index]
                    .store(frame.generation(), Ordering::SeqCst);
            }
            mapping_valid
                && if replaced {
                    replacement_valid
                } else {
                    frame == initial[index]
                }
        });
    }
    valid
}

pub(crate) fn take_linux_exec_tls_frames() -> [Option<memory::FrameLease>; 2] {
    core::array::from_fn(|index| {
        memory::FrameLease::from_parts(
            ELF_PROBE_RECLAIM_TLS_FRAMES[index].swap(0, Ordering::SeqCst),
            ELF_PROBE_RECLAIM_TLS_GENERATIONS[index].swap(0, Ordering::SeqCst),
        )
    })
}

#[inline(never)]
fn finalize_linux_exec_sibling_stack_frame(
    address: usize,
    initial: memory::FrameLease,
    replaced: bool,
) -> bool {
    let current = memory::FrameLease::from_parts(
        ELF_PROBE_SIBLING_STACK_FRAME.swap(0, Ordering::SeqCst),
        ELF_PROBE_SIBLING_STACK_GENERATION.swap(0, Ordering::SeqCst),
    );
    if initial == memory::FrameLease::EMPTY {
        return !replaced && current.is_none();
    }
    current.is_some_and(|frame| {
        let replacement_valid = frame != initial
            && !memory::frames_owned_by(memory::FrameOwner::KERNEL, &[initial])
            && ELF_PROBE_RECLAIM_SIBLING_STACK_FRAME.load(Ordering::SeqCst) == 0
            && ELF_PROBE_RECLAIM_SIBLING_STACK_GENERATION.load(Ordering::SeqCst) == 0;
        let mapping_valid =
            address_space::current_linux_mapping_matches(address as u64, frame, true, false);
        if replaced && replacement_valid {
            ELF_PROBE_RECLAIM_SIBLING_STACK_FRAME.store(frame.address(), Ordering::SeqCst);
            ELF_PROBE_RECLAIM_SIBLING_STACK_GENERATION.store(frame.generation(), Ordering::SeqCst);
        }
        mapping_valid
            && if replaced {
                replacement_valid
            } else {
                frame == initial
            }
    })
}

pub(crate) fn take_linux_exec_sibling_stack_frame() -> Option<memory::FrameLease> {
    memory::FrameLease::from_parts(
        ELF_PROBE_RECLAIM_SIBLING_STACK_FRAME.swap(0, Ordering::SeqCst),
        ELF_PROBE_RECLAIM_SIBLING_STACK_GENERATION.swap(0, Ordering::SeqCst),
    )
}

#[inline(never)]
pub(crate) fn finalize_linux_exec_page_table_frames(
    initial: [memory::FrameLease; 6],
    count: usize,
    replaced: bool,
) -> bool {
    if !(5..=6).contains(&count) {
        return false;
    }
    let mut valid = true;
    for index in 0..6 {
        let current = memory::FrameLease::from_parts(
            ELF_PROBE_PAGE_TABLE_FRAMES[index].swap(0, Ordering::SeqCst),
            ELF_PROBE_PAGE_TABLE_GENERATIONS[index].swap(0, Ordering::SeqCst),
        );
        if index >= count {
            valid &= current.is_none();
            continue;
        }
        valid &= current.is_some_and(|frame| {
            let replacement_valid = frame != initial[index]
                && memory::frames_owned_by(memory::FrameOwner::KERNEL, &[frame])
                && !memory::frames_owned_by(memory::FrameOwner::KERNEL, &[initial[index]])
                && ELF_PROBE_RECLAIM_PAGE_TABLE_FRAMES[index].load(Ordering::SeqCst) == 0
                && ELF_PROBE_RECLAIM_PAGE_TABLE_GENERATIONS[index].load(Ordering::SeqCst) == 0;
            if replaced && replacement_valid {
                ELF_PROBE_RECLAIM_PAGE_TABLE_FRAMES[index].store(frame.address(), Ordering::SeqCst);
                ELF_PROBE_RECLAIM_PAGE_TABLE_GENERATIONS[index]
                    .store(frame.generation(), Ordering::SeqCst);
            }
            if replaced {
                replacement_valid
                    && (index != 0 || address_space::current_root_address() == frame.address())
            } else {
                frame == initial[index]
            }
        });
    }
    valid
}

pub(crate) fn take_linux_exec_page_table_frames() -> [Option<memory::FrameLease>; 6] {
    core::array::from_fn(|index| {
        memory::FrameLease::from_parts(
            ELF_PROBE_RECLAIM_PAGE_TABLE_FRAMES[index].swap(0, Ordering::SeqCst),
            ELF_PROBE_RECLAIM_PAGE_TABLE_GENERATIONS[index].swap(0, Ordering::SeqCst),
        )
    })
}

pub(crate) fn verify_linux_elf_entry(execution: &LinuxElfExecution) -> LinuxElfResult {
    if LINUX_RUNTIME_MODE.load(Ordering::SeqCst) {
        return LinuxElfResult::FAILED;
    }
    let Some(heap_limit) = execution.heap.checked_add(execution.heap_size) else {
        return LinuxElfResult::FAILED;
    };
    let stack_page = execution.stack & !(PAGE_SIZE - 1);
    let stack_size = if execution.runtime {
        LINUX_RUNTIME_STACK_PAGES * PAGE_SIZE
    } else {
        PAGE_SIZE
    };
    let Some(stack_base) = stack_page.checked_sub(stack_size - PAGE_SIZE) else {
        return LinuxElfResult::FAILED;
    };
    let page_table_contract_valid = (5..=execution.page_table_frames.len())
        .contains(&execution.page_table_frame_count)
        && execution.page_table_frames[execution.page_table_frame_count..]
            .iter()
            .all(|frame| *frame == memory::FrameLease::EMPTY)
        && execution.page_table_frames[..execution.page_table_frame_count]
            .iter()
            .enumerate()
            .all(|(index, frame)| {
                memory::frames_owned_by(memory::FrameOwner::KERNEL, &[*frame])
                    && execution.page_table_frames[..index]
                        .iter()
                        .all(|prior| prior != frame)
            })
        && execution.page_table_frames[0].address() == address_space::current_root_address()
        && [
            stack_base,
            execution.heap,
            execution.mmap,
            execution.tls & !(PAGE_SIZE - 1),
            execution.sibling_tls & !(PAGE_SIZE - 1),
            execution.sibling_stack,
            execution.code,
            execution.loader_code,
            execution.data,
            execution.relro,
        ]
        .into_iter()
        .filter(|address| *address != 0)
        .all(|address| {
            address_space::current_linux_hierarchy_matches(
                address as u64,
                &execution.page_table_frames[..execution.page_table_frame_count],
            )
        })
        && execution.initial_image_count <= LINUX_ACTIVE_IMAGE_MAPPING_LIMIT
        && execution.initial_image_mappings[..execution.initial_image_count]
            .iter()
            .all(|mapping| {
                address_space::current_linux_hierarchy_matches(
                    mapping.address as u64,
                    &execution.page_table_frames[..execution.page_table_frame_count],
                )
            })
        && ELF_PROBE_RECLAIM_PAGE_TABLE_FRAMES
            .iter()
            .all(|frame| frame.load(Ordering::SeqCst) == 0)
        && ELF_PROBE_RECLAIM_PAGE_TABLE_GENERATIONS
            .iter()
            .all(|generation| generation.load(Ordering::SeqCst) == 0);
    let stale_page_table_lease_rejected = execution.page_table_frames[0]
        .generation()
        .checked_add(1)
        .and_then(|generation| {
            memory::FrameLease::from_parts(execution.page_table_frames[0].address(), generation)
        })
        .is_some_and(|stale| {
            !memory::frames_owned_by(memory::FrameOwner::KERNEL, &[stale])
                && memory::frames_owned_by(
                    memory::FrameOwner::KERNEL,
                    &[execution.page_table_frames[0]],
                )
        });
    let stack_contract_valid = execution.stack != 0
        && stack_base != 0
        && execution.stack < stack_base + stack_size
        && memory::frames_owned_by(memory::FrameOwner::KERNEL, &[execution.stack_frame])
        && address_space::current_linux_mapping_matches(
            stack_page as u64,
            execution.stack_frame,
            true,
            false,
        );
    let heap_contract_valid = execution.heap != 0
        && execution.heap.is_multiple_of(PAGE_SIZE)
        && execution.heap_size
            == if execution.runtime {
                LINUX_RUNTIME_HEAP_PAGES * PAGE_SIZE
            } else {
                PAGE_SIZE
            }
        && execution.heap_frame != execution.stack_frame
        && address_space::current_linux_mapping_matches(
            execution.heap as u64,
            execution.heap_frame,
            true,
            false,
        );
    let initial_image_valid = if execution.initial_image_count == 0 {
        execution
            .initial_image_mappings
            .iter()
            .all(|mapping| *mapping == LinuxActiveImageMapping::EMPTY)
    } else {
        (1..=LINUX_ACTIVE_IMAGE_MAPPING_LIMIT).contains(&execution.initial_image_count)
            && execution.initial_image_mappings[..execution.initial_image_count]
                .iter()
                .enumerate()
                .all(|(index, mapping)| {
                    mapping.address != 0
                        && mapping.address.is_multiple_of(PAGE_SIZE)
                        && !(mapping.writable && mapping.executable)
                        && (memory::frames_owned_by(memory::FrameOwner::KERNEL, &[mapping.frame])
                            || memory::frame_lease_is_runtime(mapping.frame))
                        && execution.initial_image_mappings[..index]
                            .iter()
                            .all(|prior| {
                                prior.address != mapping.address && prior.frame != mapping.frame
                            })
                })
            && execution.initial_image_mappings[execution.initial_image_count..]
                .iter()
                .all(|mapping| *mapping == LinuxActiveImageMapping::EMPTY)
    };
    let retired_image_state_clean = ELF_PROBE_RETIRED_IMAGE_COUNT.load(Ordering::SeqCst) == 0
        && (0..LINUX_ACTIVE_IMAGE_MAPPING_LIMIT).all(|index| {
            ELF_PROBE_RETIRED_IMAGE_ADDRESSES[index].load(Ordering::SeqCst) == 0
                && ELF_PROBE_RETIRED_IMAGE_FRAMES[index].load(Ordering::SeqCst) == 0
                && ELF_PROBE_RETIRED_IMAGE_GENERATIONS[index].load(Ordering::SeqCst) == 0
                && ELF_PROBE_RETIRED_IMAGE_FLAGS[index].load(Ordering::SeqCst) == 0
        });
    let heap_reclaim_state_clean = ELF_PROBE_RECLAIM_HEAP_FRAME.load(Ordering::SeqCst) == 0
        && ELF_PROBE_RECLAIM_HEAP_GENERATION.load(Ordering::SeqCst) == 0;
    let mmap_contract_valid = execution.mmap != 0
        && execution.mmap.is_multiple_of(PAGE_SIZE)
        && ELF_PROBE_MMAP_FRAMES
            .iter()
            .all(|frame| frame.load(Ordering::SeqCst) == 0)
        && ELF_PROBE_MMAP_GENERATIONS
            .iter()
            .all(|generation| generation.load(Ordering::SeqCst) == 0);
    if !page_table_contract_valid
        || !stale_page_table_lease_rejected
        || !stack_contract_valid
        || !heap_contract_valid
        || !initial_image_valid
        || !retired_image_state_clean
        || !heap_reclaim_state_clean
        || !mmap_contract_valid
        || execution.exec_expected && execution.initial_image_count == 0
    {
        if execution.runtime {
            serial_write("ginux: Alpine runtime contract rejection:");
            for valid in [
                page_table_contract_valid,
                stale_page_table_lease_rejected,
                stack_contract_valid,
                heap_contract_valid,
                initial_image_valid,
                retired_image_state_clean,
                heap_reclaim_state_clean,
                mmap_contract_valid,
            ] {
                serial_write(" ");
                serial_write_u64(u64::from(valid));
            }
            serial_write("\r\n");
        }
        return LinuxElfResult::FAILED;
    }
    let initial_tls = LinuxTlsContext {
        thread_pointer: execution.initial_fs_base,
        extent: execution.tls,
        extent_size: execution.tls_size,
    };
    let sibling_tls = LinuxTlsContext {
        thread_pointer: execution.sibling_fs_base,
        extent: execution.sibling_tls,
        extent_size: execution.tls_size,
    };
    let tls_contract_valid = initial_tls.is_valid()
        && sibling_tls.is_valid()
        && (execution.tls_size == 0 && execution.sibling_stack == 0
            || execution.tls_size != 0
                && execution.sibling_stack != 0
                && execution.sibling_stack.is_multiple_of(PAGE_SIZE)
                && execution
                    .sibling_stack
                    .checked_add(PAGE_SIZE)
                    .is_some_and(|end| end <= 1 << 47)
                && execution.sibling_stack != (execution.stack & !(PAGE_SIZE - 1)))
        && (execution.tls_size == 0
            || !linux_ranges_overlap(
                initial_tls.extent as u64,
                initial_tls.extent_size,
                sibling_tls.extent as u64,
                sibling_tls.extent_size,
            ));
    let tls_frame_contract_valid = if execution.tls_size == 0 {
        execution.tls_frames == [memory::FrameLease::EMPTY; 2]
            && execution.sibling_stack_frame == memory::FrameLease::EMPTY
    } else {
        execution.tls_frames[0] != execution.tls_frames[1]
            && execution
                .tls_frames
                .iter()
                .all(|frame| *frame != execution.stack_frame && *frame != execution.heap_frame)
            && address_space::current_linux_mapping_matches(
                (execution.tls & !(PAGE_SIZE - 1)) as u64,
                execution.tls_frames[0],
                true,
                false,
            )
            && address_space::current_linux_mapping_matches(
                (execution.sibling_tls & !(PAGE_SIZE - 1)) as u64,
                execution.tls_frames[1],
                true,
                false,
            )
            && execution.sibling_stack_frame != execution.stack_frame
            && execution.sibling_stack_frame != execution.heap_frame
            && !execution
                .tls_frames
                .contains(&execution.sibling_stack_frame)
            && address_space::current_linux_mapping_matches(
                execution.sibling_stack as u64,
                execution.sibling_stack_frame,
                true,
                false,
            )
    };
    let tls_reclaim_state_clean = (0..2).all(|index| {
        ELF_PROBE_RECLAIM_TLS_FRAMES[index].load(Ordering::SeqCst) == 0
            && ELF_PROBE_RECLAIM_TLS_GENERATIONS[index].load(Ordering::SeqCst) == 0
    });
    let sibling_stack_reclaim_state_clean =
        ELF_PROBE_RECLAIM_SIBLING_STACK_FRAME.load(Ordering::SeqCst) == 0
            && ELF_PROBE_RECLAIM_SIBLING_STACK_GENERATION.load(Ordering::SeqCst) == 0;
    if !tls_contract_valid
        || !tls_frame_contract_valid
        || !tls_reclaim_state_clean
        || !sibling_stack_reclaim_state_clean
    {
        if execution.runtime {
            serial_write("ginux: Alpine TLS contract rejection\r\n");
        }
        return LinuxElfResult::FAILED;
    }
    if ELF_PROBE_ACTIVE.swap(true, Ordering::SeqCst) {
        return LinuxElfResult::FAILED;
    }
    let Some(clock_frequency) = timer::monotonic_frequency()
        .filter(|frequency| *frequency <= u64::MAX / NANOSECONDS_PER_SECOND)
    else {
        if execution.runtime {
            serial_write("ginux: Alpine clock setup failed\r\n");
        }
        ELF_PROBE_ACTIVE.store(false, Ordering::SeqCst);
        return LinuxElfResult::FAILED;
    };
    let Some(saved_msrs) = configure_linux_syscall() else {
        if execution.runtime {
            serial_write("ginux: Alpine syscall setup failed\r\n");
        }
        ELF_PROBE_ACTIVE.store(false, Ordering::SeqCst);
        return LinuxElfResult::FAILED;
    };
    let linux_thread_lifecycle_valid = finish_linux_thread_table()
        && (|| -> Option<bool> {
            let initial_stack = execution.stack & !(PAGE_SIZE - 1);
            let initial_thread = reserve_linux_thread(initial_tls, stack_base, stack_size)?;
            if initial_thread.tid != 1 || !switch_linux_thread(initial_thread) {
                return Some(false);
            }
            if execution.tls_size == 0 {
                return Some(true);
            }
            let sibling_thread =
                reserve_linux_thread(sibling_tls, execution.sibling_stack, PAGE_SIZE)?;
            let capacity_bounded =
                reserve_linux_thread(sibling_tls, execution.sibling_stack, PAGE_SIZE).is_none();
            if sibling_thread.tid != 2
                || !capacity_bounded
                || LINUX_THREAD_STACK_BASES[usize::from(sibling_thread.slot)].load(Ordering::SeqCst)
                    != execution.sibling_stack as u64
                || LINUX_THREAD_STACK_SIZES[usize::from(sibling_thread.slot)].load(Ordering::SeqCst)
                    != PAGE_SIZE as u64
                || !set_current_linux_clear_child_tid(0x1110)
                || !set_current_linux_robust_list(0x1120)
                || !switch_linux_thread(sibling_thread)
                || current_linux_tid() != Some(2)
                || current_linux_clear_child_tid() != Some(0)
                || current_linux_robust_list() != Some(0)
                || !set_current_linux_clear_child_tid(0x2210)
                || !set_current_linux_robust_list(0x2220)
                || !switch_linux_thread(initial_thread)
                || current_linux_tid() != Some(1)
                || current_linux_clear_child_tid() != Some(0x1110)
                || current_linux_robust_list() != Some(0x1120)
                || LINUX_THREAD_CLEAR_CHILD_TIDS[usize::from(sibling_thread.slot)]
                    .load(Ordering::SeqCst)
                    != 0x2210
                || LINUX_THREAD_ROBUST_LISTS[usize::from(sibling_thread.slot)]
                    .load(Ordering::SeqCst)
                    != 0x2220
                || !set_current_linux_clear_child_tid(0)
                || !set_current_linux_robust_list(0)
            {
                return Some(false);
            }
            let clone_request = LinuxCloneRequest {
                flags: LINUX_BOUNDED_THREAD_CLONE_FLAGS,
                child_stack: execution.sibling_stack.checked_add(PAGE_SIZE)? as u64,
                parent_tid: initial_stack.checked_add(64)? as u64,
                child_tid: initial_stack.checked_add(68)? as u64,
                tls: execution.sibling_fs_base as u64,
            };
            let proof_registers = LinuxSyscallFrame::from_array([
                0x15,
                0x14,
                0x13,
                0x12,
                0xb0,
                0xb1,
                0x10,
                9,
                8,
                clone_request.parent_tid,
                clone_request.child_stack,
                clone_request.flags,
            ]);
            let invalid_flags_rejected = construct_linux_clone_context(
                sibling_thread,
                LinuxCloneRequest {
                    flags: clone_request.flags | (1 << 63),
                    ..clone_request
                },
                proof_registers,
                execution.entry as u64,
                0x202,
                initial_stack as u64,
                PAGE_SIZE,
            )
            .is_none();
            let invalid_stack_rejected = construct_linux_clone_context(
                sibling_thread,
                LinuxCloneRequest {
                    child_stack: clone_request.child_stack - 16,
                    ..clone_request
                },
                proof_registers,
                execution.entry as u64,
                0x202,
                initial_stack as u64,
                PAGE_SIZE,
            )
            .is_none();
            let clone = construct_linux_clone_context(
                sibling_thread,
                clone_request,
                proof_registers,
                execution.entry as u64,
                0x202,
                initial_stack as u64,
                PAGE_SIZE,
            )?;
            let selected = linux_thread_return_context(sibling_thread)?;
            let mut installed_registers = LinuxSyscallFrame::ZERO;
            let mut installed_rip = 0;
            let mut installed_rsp = 0;
            let mut installed_rflags = 0;
            let mut installed_rax = u64::MAX;
            let return_installed = install_linux_thread_return(
                sibling_thread,
                &mut installed_registers,
                &mut installed_rip,
                &mut installed_rsp,
                &mut installed_rflags,
                &mut installed_rax,
            );
            let sibling_copy_state =
                linux_writable_user_range_contains(
                    execution.sibling_tls as u64,
                    execution.tls_size,
                ) && !linux_writable_user_range_contains(execution.tls as u64, execution.tls_size);
            let initial_restored = switch_linux_thread(initial_thread);
            let initial_copy_state =
                linux_writable_user_range_contains(execution.tls as u64, execution.tls_size)
                    && !linux_writable_user_range_contains(
                        execution.sibling_tls as u64,
                        execution.tls_size,
                    );
            let sibling_slot = usize::from(sibling_thread.slot);
            if !invalid_flags_rejected
                || !invalid_stack_rejected
                || clone.parent_rax != u64::from(sibling_thread.tid)
                || clone.child.rip != execution.entry as u64
                || clone.child.rsp != clone_request.child_stack
                || clone.child.rflags != 0x202
                || clone.child.rax != 0
                || clone.child.fs_base != execution.sibling_fs_base as u64
                || clone.child.registers != proof_registers
                || selected != clone.child
                || !return_installed
                || installed_registers != clone.child.registers
                || installed_rip != clone.child.rip
                || installed_rsp != clone.child.rsp
                || installed_rflags != clone.child.rflags
                || installed_rax != clone.child.rax
                || !sibling_copy_state
                || !initial_restored
                || !initial_copy_state
                || LINUX_THREAD_RIPS[sibling_slot].load(Ordering::SeqCst) != clone.child.rip
                || LINUX_THREAD_RSPS[sibling_slot].load(Ordering::SeqCst) != clone.child.rsp
                || LINUX_THREAD_RFLAGS[sibling_slot].load(Ordering::SeqCst) != clone.child.rflags
                || LINUX_THREAD_RAXS[sibling_slot].load(Ordering::SeqCst) != 0
                || LINUX_THREAD_CLEAR_CHILD_TIDS[sibling_slot].load(Ordering::SeqCst)
                    != clone_request.child_tid
                || !retire_linux_thread(sibling_thread)
            {
                return Some(false);
            }
            let mut stale_registers = proof_registers;
            let mut stale_rip = 0x1111;
            let mut stale_rsp = 0x2222;
            let mut stale_rflags = 0x3333;
            let mut stale_rax = 0x4444;
            let stale_install_rejected = !install_linux_thread_return(
                sibling_thread,
                &mut stale_registers,
                &mut stale_rip,
                &mut stale_rsp,
                &mut stale_rflags,
                &mut stale_rax,
            ) && stale_registers == proof_registers
                && stale_rip == 0x1111
                && stale_rsp == 0x2222
                && stale_rflags == 0x3333
                && stale_rax == 0x4444
                && current_linux_tid() == Some(1);
            let replacement =
                reserve_linux_thread(sibling_tls, execution.sibling_stack, PAGE_SIZE)?;
            Some(
                stale_install_rejected
                    && replacement.tid == 3
                    && replacement.slot == sibling_thread.slot
                    && replacement.generation != sibling_thread.generation
                    && LINUX_THREAD_CLEAR_CHILD_TIDS[usize::from(replacement.slot)]
                        .load(Ordering::SeqCst)
                        == 0
                    && LINUX_THREAD_ROBUST_LISTS[usize::from(replacement.slot)]
                        .load(Ordering::SeqCst)
                        == 0
                    && LINUX_THREAD_STACK_BASES[usize::from(replacement.slot)]
                        .load(Ordering::SeqCst)
                        == execution.sibling_stack as u64
                    && LINUX_THREAD_RIPS[usize::from(replacement.slot)].load(Ordering::SeqCst) == 0
                    && LINUX_THREAD_RSPS[usize::from(replacement.slot)].load(Ordering::SeqCst) == 0
                    && LINUX_THREAD_RFLAGS[usize::from(replacement.slot)].load(Ordering::SeqCst)
                        == 0
                    && LINUX_THREAD_RAXS[usize::from(replacement.slot)].load(Ordering::SeqCst) == 0
                    && linux_thread_return_context(sibling_thread).is_none()
                    && linux_thread_return_context(replacement).is_none()
                    && !switch_linux_thread(sibling_thread)
                    && retire_linux_thread(replacement),
            )
        })()
        .unwrap_or(false);
    if !linux_thread_lifecycle_valid {
        if execution.runtime {
            serial_write("ginux: Alpine initial thread rejection\r\n");
        }
        let _ = restore_linux_syscall(saved_msrs);
        let _ = finish_linux_thread_table();
        ELF_PROBE_ACTIVE.store(false, Ordering::SeqCst);
        return LinuxElfResult::FAILED;
    }
    ELF_PROBE_COMPLETED.store(false, Ordering::SeqCst);
    ELF_PROBE_TERMINATION_SIGNAL.store(0, Ordering::SeqCst);
    ELF_PROBE_EXEC_REPLACED.store(false, Ordering::SeqCst);
    ELF_PROBE_EXEC_NODE.store(0, Ordering::SeqCst);
    LINUX_LIVE_RETURN_VALIDATED.store(false, Ordering::SeqCst);
    LINUX_CHILD_EXIT_COUNT.store(0, Ordering::SeqCst);
    LINUX_PARENT_RESUMED.store(false, Ordering::SeqCst);
    LINUX_FORK_REQUESTED.store(false, Ordering::SeqCst);
    LINUX_PROCESS_CHILD_EXITED.store(false, Ordering::SeqCst);
    LINUX_PROCESS_CHILD_STATUS.store(0, Ordering::SeqCst);
    LINUX_CURRENT_PROCESS_PID.store(1, Ordering::SeqCst);
    LINUX_PROCESS_WAIT_COUNT.store(0, Ordering::SeqCst);
    LIVE_FORK_PUBLIC_DIAGNOSTIC.store(0, Ordering::SeqCst);
    LINUX_PROCESS_SNAPSHOT_VALIDATED.store(false, Ordering::SeqCst);
    LINUX_CLONE_DISPATCH_COUNT.store(0, Ordering::SeqCst);
    LINUX_FIRST_CHILD_TID.store(0, Ordering::SeqCst);
    LINUX_CLONE_REUSE_VALIDATED.store(false, Ordering::SeqCst);
    LINUX_CHILD_GETTID_COUNT.store(0, Ordering::SeqCst);
    LINUX_YIELD_SWITCH_COUNT.store(0, Ordering::SeqCst);
    LINUX_CLONE_BUSY_REJECTION_COUNT.store(0, Ordering::SeqCst);
    LINUX_CLONE_INVALID_REJECTION_COUNT.store(0, Ordering::SeqCst);
    LINUX_CLONE_TID_FAULT_COUNT.store(0, Ordering::SeqCst);
    LINUX_FUTEX_WAIT_BLOCK_COUNT.store(0, Ordering::SeqCst);
    LINUX_FUTEX_EXPLICIT_WAKE_COUNT.store(0, Ordering::SeqCst);
    LINUX_FUTEX_EXIT_WAKE_COUNT.store(0, Ordering::SeqCst);
    LINUX_FUTEX_WAIT_TIMEOUT_COUNT.store(0, Ordering::SeqCst);
    LINUX_SLEEP_BLOCK_COUNT.store(0, Ordering::SeqCst);
    LINUX_SLEEP_WAKE_COUNT.store(0, Ordering::SeqCst);
    LINUX_SLEEP_INTERRUPT_COUNT.store(0, Ordering::SeqCst);
    LINUX_TIMER_WAIT_EXPIRY_COUNT.store(0, Ordering::SeqCst);
    LINUX_TIMER_WAIT_PREEMPTION_COUNT.store(0, Ordering::SeqCst);
    LINUX_POLL_WAIT_BLOCK_COUNT.store(0, Ordering::SeqCst);
    LINUX_POLL_WAIT_WAKE_COUNT.store(0, Ordering::SeqCst);
    LINUX_POLL_WAIT_TIMEOUT_COUNT.store(0, Ordering::SeqCst);
    LINUX_POLL_WAIT_INTERRUPT_COUNT.store(0, Ordering::SeqCst);
    LINUX_EPOLL_WAIT_BLOCK_COUNT.store(0, Ordering::SeqCst);
    LINUX_EPOLL_WAIT_WAKE_COUNT.store(0, Ordering::SeqCst);
    LINUX_EPOLL_WAIT_TIMEOUT_COUNT.store(0, Ordering::SeqCst);
    LINUX_EPOLL_WAIT_INTERRUPT_COUNT.store(0, Ordering::SeqCst);
    LINUX_EVENTFD_READ_BLOCK_COUNT.store(0, Ordering::SeqCst);
    LINUX_EVENTFD_READ_WAKE_COUNT.store(0, Ordering::SeqCst);
    LINUX_EVENTFD_READ_INTERRUPT_COUNT.store(0, Ordering::SeqCst);
    LINUX_EVENTFD_WRITE_BLOCK_COUNT.store(0, Ordering::SeqCst);
    LINUX_EVENTFD_WRITE_WAKE_COUNT.store(0, Ordering::SeqCst);
    LINUX_EVENTFD_WRITE_INTERRUPT_COUNT.store(0, Ordering::SeqCst);
    LINUX_PIPE_READ_BLOCK_COUNT.store(0, Ordering::SeqCst);
    LINUX_PIPE_READ_WAKE_COUNT.store(0, Ordering::SeqCst);
    LINUX_PIPE_READ_INTERRUPT_COUNT.store(0, Ordering::SeqCst);
    LINUX_PIPE_WRITE_BLOCK_COUNT.store(0, Ordering::SeqCst);
    LINUX_PIPE_WRITE_WAKE_COUNT.store(0, Ordering::SeqCst);
    LINUX_PIPE_WRITE_INTERRUPT_COUNT.store(0, Ordering::SeqCst);
    LINUX_TIMERFD_READ_BLOCK_COUNT.store(0, Ordering::SeqCst);
    LINUX_TIMERFD_READ_WAKE_COUNT.store(0, Ordering::SeqCst);
    LINUX_TIMERFD_READ_INTERRUPT_COUNT.store(0, Ordering::SeqCst);
    LINUX_PPOLL_WAIT_BLOCK_COUNT.store(0, Ordering::SeqCst);
    LINUX_PPOLL_WAIT_WAKE_COUNT.store(0, Ordering::SeqCst);
    LINUX_PPOLL_WAIT_TIMEOUT_COUNT.store(0, Ordering::SeqCst);
    LINUX_EPOLL_PWAIT_BLOCK_COUNT.store(0, Ordering::SeqCst);
    LINUX_EPOLL_PWAIT_WAKE_COUNT.store(0, Ordering::SeqCst);
    LINUX_EPOLL_PWAIT_TIMEOUT_COUNT.store(0, Ordering::SeqCst);
    LINUX_MASKED_WAIT_INTERRUPT_COUNT.store(0, Ordering::SeqCst);
    LINUX_SIGNAL_WAIT_INTERRUPT_COUNT.store(0, Ordering::SeqCst);
    LINUX_SYSTEM_SUPERVISOR_WAIT_COUNT.store(0, Ordering::SeqCst);
    LINUX_SYSTEM_SUPERVISOR_SHUTDOWN_COUNT.store(0, Ordering::SeqCst);
    ELF_PROBE_SYSTEM_INIT.store(execution.system_init, Ordering::SeqCst);
    ELF_PROBE_SIBLING_FS_BASE.store(execution.sibling_fs_base as u64, Ordering::SeqCst);
    ELF_PROBE_SIBLING_TLS.store(execution.sibling_tls as u64, Ordering::SeqCst);
    ELF_PROBE_SIBLING_STACK.store(execution.sibling_stack as u64, Ordering::SeqCst);
    ELF_PROBE_SIBLING_STACK_FRAME.store(execution.sibling_stack_frame.address(), Ordering::SeqCst);
    ELF_PROBE_SIBLING_STACK_GENERATION
        .store(execution.sibling_stack_frame.generation(), Ordering::SeqCst);
    for index in 0..ELF_PROBE_PAGE_TABLE_FRAMES.len() {
        let frame = execution.page_table_frames[index];
        ELF_PROBE_PAGE_TABLE_FRAMES[index].store(frame.address(), Ordering::SeqCst);
        ELF_PROBE_PAGE_TABLE_GENERATIONS[index].store(frame.generation(), Ordering::SeqCst);
    }
    ELF_PROBE_STACK.store(execution.stack as u64, Ordering::SeqCst);
    ELF_PROBE_STACK_FRAME.store(execution.stack_frame.address(), Ordering::SeqCst);
    ELF_PROBE_STACK_GENERATION.store(execution.stack_frame.generation(), Ordering::SeqCst);
    ELF_PROBE_CODE_BASE.store(execution.code as u64, Ordering::SeqCst);
    ELF_PROBE_CODE_SIZE.store(execution.code_size as u64, Ordering::SeqCst);
    ELF_PROBE_LOADER_CODE_BASE.store(execution.loader_code as u64, Ordering::SeqCst);
    ELF_PROBE_LOADER_CODE_SIZE.store(execution.loader_code_size as u64, Ordering::SeqCst);
    ELF_PROBE_DATA.store(execution.data as u64, Ordering::SeqCst);
    ELF_PROBE_DATA_SIZE.store(execution.data_size as u64, Ordering::SeqCst);
    ELF_PROBE_EXEC_ENTRY.store(0, Ordering::SeqCst);
    for index in 0..LINUX_EXEC_MAPPING_LIMIT {
        ELF_PROBE_EXEC_MAPPING_ADDRESSES[index].store(0, Ordering::SeqCst);
        ELF_PROBE_EXEC_MAPPING_FRAMES[index].store(0, Ordering::SeqCst);
        ELF_PROBE_EXEC_MAPPING_GENERATIONS[index].store(0, Ordering::SeqCst);
        ELF_PROBE_EXEC_MAPPING_FLAGS[index].store(0, Ordering::SeqCst);
    }
    for (index, mapping) in execution.initial_image_mappings.iter().copied().enumerate() {
        ELF_PROBE_ACTIVE_IMAGE_ADDRESSES[index].store(mapping.address as u64, Ordering::SeqCst);
        ELF_PROBE_ACTIVE_IMAGE_FRAMES[index].store(mapping.frame.address(), Ordering::SeqCst);
        ELF_PROBE_ACTIVE_IMAGE_GENERATIONS[index]
            .store(mapping.frame.generation(), Ordering::SeqCst);
        ELF_PROBE_ACTIVE_IMAGE_FLAGS[index].store(
            u8::from(mapping.writable) | (u8::from(mapping.executable) << 1),
            Ordering::SeqCst,
        );
    }
    ELF_PROBE_ACTIVE_IMAGE_COUNT.store(execution.initial_image_count as u16, Ordering::SeqCst);
    ELF_PROBE_RELRO.store(execution.relro as u64, Ordering::SeqCst);
    ELF_PROBE_RELRO_SIZE.store(execution.relro_size as u64, Ordering::SeqCst);
    ELF_PROBE_BRK.store(execution.heap as u64, Ordering::SeqCst);
    ELF_PROBE_BRK_LIMIT.store(heap_limit as u64, Ordering::SeqCst);
    ELF_PROBE_HEAP_FRAME.store(execution.heap_frame.address(), Ordering::SeqCst);
    ELF_PROBE_HEAP_GENERATION.store(execution.heap_frame.generation(), Ordering::SeqCst);
    for (index, frame) in execution.tls_frames.iter().copied().enumerate() {
        ELF_PROBE_TLS_FRAMES[index].store(frame.address(), Ordering::SeqCst);
        ELF_PROBE_TLS_GENERATIONS[index].store(frame.generation(), Ordering::SeqCst);
    }
    ELF_PROBE_MMAP.store(execution.mmap as u64, Ordering::SeqCst);
    for frame in &ELF_PROBE_MMAP_FRAMES {
        frame.store(0, Ordering::SeqCst);
    }
    for generation in &ELF_PROBE_MMAP_GENERATIONS {
        generation.store(0, Ordering::SeqCst);
    }
    for index in 0..LINUX_RUNTIME_MMAP_SLOT_COUNT {
        ELF_PROBE_MMAP_PAGES[index].store(0, Ordering::SeqCst);
        ELF_PROBE_MMAP_PROTECTION[index].store(0, Ordering::SeqCst);
        ELF_PROBE_MMAP_FILE[index].store(0, Ordering::SeqCst);
    }
    for (index, mapping) in execution.main_mappings.iter().copied().enumerate() {
        let protection = LINUX_PROT_READ
            | if mapping.writable {
                LINUX_PROT_WRITE
            } else {
                0
            }
            | if mapping.executable {
                LINUX_PROT_EXEC
            } else {
                0
            };
        ELF_PROBE_MAIN_ADDRESSES[index].store(mapping.address as u64, Ordering::SeqCst);
        ELF_PROBE_MAIN_FRAMES[index].store(0, Ordering::SeqCst);
        ELF_PROBE_MAIN_GENERATIONS[index].store(0, Ordering::SeqCst);
        ELF_PROBE_MAIN_OFFSETS[index].store(mapping.offset, Ordering::SeqCst);
        ELF_PROBE_MAIN_FILE_SIZES[index].store(mapping.file_size as u64, Ordering::SeqCst);
        ELF_PROBE_MAIN_PROTECTIONS[index].store(protection as u8, Ordering::SeqCst);
    }
    ELF_PROBE_MAIN_MAPPINGS.store(0, Ordering::SeqCst);
    for (dependency, mappings) in execution.dependency_mappings.iter().enumerate() {
        ELF_PROBE_DEPENDENCY_BASE_MIN[dependency].store(
            execution.dependency_base_min[dependency] as u64,
            Ordering::SeqCst,
        );
        ELF_PROBE_DEPENDENCY_BASE_END[dependency].store(
            execution.dependency_base_end[dependency] as u64,
            Ordering::SeqCst,
        );
        ELF_PROBE_DEPENDENCY_SELECTED_BASE[dependency].store(0, Ordering::SeqCst);
        for (index, mapping) in mappings.iter().copied().enumerate() {
            let state = dependency * LINUX_DEPENDENCY_MAPPING_COUNT + index;
            let protection = LINUX_PROT_READ
                | if mapping.writable {
                    LINUX_PROT_WRITE
                } else {
                    0
                }
                | if mapping.executable {
                    LINUX_PROT_EXEC
                } else {
                    0
                };
            ELF_PROBE_DEPENDENCY_RELATIVES[state]
                .store(mapping.relative_address as u64, Ordering::SeqCst);
            ELF_PROBE_DEPENDENCY_FRAMES[state].store(0, Ordering::SeqCst);
            ELF_PROBE_DEPENDENCY_GENERATIONS[state].store(0, Ordering::SeqCst);
            ELF_PROBE_DEPENDENCY_OFFSETS[state].store(mapping.offset, Ordering::SeqCst);
            ELF_PROBE_DEPENDENCY_FILE_SIZES[state]
                .store(mapping.file_size as u64, Ordering::SeqCst);
            ELF_PROBE_DEPENDENCY_PROTECTIONS[state].store(protection as u8, Ordering::SeqCst);
        }
        ELF_PROBE_DEPENDENCY_MAPPINGS[dependency].store(0, Ordering::SeqCst);
    }
    ELF_PROBE_EXITED_CLEAR_CHILD_TID.store(0, Ordering::SeqCst);
    ELF_PROBE_EXITED_ROBUST_LIST.store(0, Ordering::SeqCst);
    ELF_PROBE_READ_ONLY_DATA_PAGE.store(0, Ordering::SeqCst);
    ELF_PROBE_UMASK.store(0o022, Ordering::SeqCst);
    ELF_PROBE_CONSOLE_DESCRIPTORS.store(
        (LINUX_DESCRIPTOR_INPUT | (LINUX_DESCRIPTOR_OUTPUT << 2)) as u8,
        Ordering::SeqCst,
    );
    ELF_PROBE_CONSOLE_CLOEXEC.store(0, Ordering::SeqCst);
    for descriptor in &ELF_PROBE_DEVICE_DESCRIPTORS {
        descriptor.store(0, Ordering::SeqCst);
    }
    for descriptor in &ELF_PROBE_OCI_DESCRIPTORS {
        descriptor.store(0, Ordering::SeqCst);
    }
    for generation in &ELF_PROBE_TMP_FILE_GENERATIONS {
        generation.store(0, Ordering::SeqCst);
    }
    for descriptor in &ELF_PROBE_STDIO_DEVICE_DESCRIPTORS {
        descriptor.store(0, Ordering::SeqCst);
    }
    for descriptor in &ELF_PROBE_STDIO_OCI_DESCRIPTORS {
        descriptor.store(0, Ordering::SeqCst);
    }
    for generation in &ELF_PROBE_STDIO_TMP_FILE_GENERATIONS {
        generation.store(0, Ordering::SeqCst);
    }
    for offset in &ELF_PROBE_STDIO_OFFSETS {
        offset.store(0, Ordering::SeqCst);
    }
    ELF_PROBE_CWD.store(LINUX_ROOT_DIRECTORY, Ordering::SeqCst);
    ELF_PROBE_OCI_CWD.store(crate::oci_image::ROOT_HANDLE, Ordering::SeqCst);
    ELF_PROBE_EPOLL_DESCRIPTOR.store(0, Ordering::SeqCst);
    ELF_PROBE_EPOLL_TARGET.store(u64::MAX, Ordering::SeqCst);
    ELF_PROBE_EPOLL_EVENTS.store(0, Ordering::SeqCst);
    ELF_PROBE_EPOLL_DATA.store(0, Ordering::SeqCst);
    ELF_PROBE_EVENTFD_DESCRIPTOR.store(0, Ordering::SeqCst);
    ELF_PROBE_EVENTFD_COUNTER.store(0, Ordering::SeqCst);
    ELF_PROBE_EVENTFD_FLAGS.store(0, Ordering::SeqCst);
    ELF_PROBE_TIMERFD_DESCRIPTOR.store(0, Ordering::SeqCst);
    ELF_PROBE_TIMERFD_FLAGS.store(0, Ordering::SeqCst);
    ELF_PROBE_TIMERFD_INTERVAL.store(0, Ordering::SeqCst);
    ELF_PROBE_TIMERFD_DEADLINE.store(0, Ordering::SeqCst);
    ELF_PROBE_PIPE_READ_DESCRIPTOR.store(0, Ordering::SeqCst);
    ELF_PROBE_PIPE_WRITE_DESCRIPTOR.store(0, Ordering::SeqCst);
    ELF_PROBE_PIPE_READ_FLAGS.store(0, Ordering::SeqCst);
    ELF_PROBE_PIPE_WRITE_FLAGS.store(0, Ordering::SeqCst);
    ELF_PROBE_PIPE_LENGTH.store(0, Ordering::SeqCst);
    for byte in &ELF_PROBE_PIPE_BUFFER {
        byte.store(0, Ordering::SeqCst);
    }
    let _ = linux_tmpfs::teardown();
    for cursor in &ELF_PROBE_DESCRIPTOR_OFFSETS {
        cursor.store(0, Ordering::SeqCst);
    }
    ELF_PROBE_CLOCK_FREQUENCY.store(clock_frequency, Ordering::SeqCst);
    ELF_PROBE_CLOCK_EPOCH.store(timer::timestamp_counter(), Ordering::SeqCst);
    ELF_PROBE_PROCESS_NAME_LOW.store(LINUX_INITIAL_PROCESS_NAME, Ordering::SeqCst);
    ELF_PROBE_PROCESS_NAME_HIGH.store(0, Ordering::SeqCst);
    LINUX_RUNTIME_EXIT_STATUS.store(u64::MAX, Ordering::SeqCst);
    LINUX_RUNTIME_FIRST_UNSUPPORTED_SYSCALL.store(u64::MAX, Ordering::SeqCst);
    LINUX_RUNTIME_SYSCALL_TRACE_COUNT.store(0, Ordering::SeqCst);
    for syscall in &LINUX_RUNTIME_SYSCALL_TRACE {
        syscall.store(u64::MAX, Ordering::SeqCst);
    }
    ELF_PROBE_NO_NEW_PRIVS.store(false, Ordering::SeqCst);
    store_current_linux_signal_mask(0);
    ELF_PROBE_PENDING_SIGNALS.store(0, Ordering::SeqCst);
    ELF_PROBE_EXEC_IGNORED_SIGNALS.store(0, Ordering::SeqCst);
    for signal in 0..LINUX_SIGNAL_COUNT {
        ELF_PROBE_PENDING_SIGNAL_CODES[signal].store(0, Ordering::SeqCst);
        ELF_PROBE_SIGNAL_HANDLERS[signal].store(0, Ordering::SeqCst);
        ELF_PROBE_SIGNAL_FLAGS[signal].store(0, Ordering::SeqCst);
        ELF_PROBE_SIGNAL_RESTORERS[signal].store(0, Ordering::SeqCst);
        ELF_PROBE_SIGNAL_ACTION_MASKS[signal].store(0, Ordering::SeqCst);
    }
    ELF_PROBE_ALT_STACK_POINTER.store(0, Ordering::SeqCst);
    ELF_PROBE_ALT_STACK_SIZE.store(0, Ordering::SeqCst);
    ELF_PROBE_ALT_STACK_FLAGS.store(LINUX_SS_DISABLE, Ordering::SeqCst);
    LINUX_SIGNAL_ACTIVE.store(false, Ordering::SeqCst);
    LINUX_SIGNAL_DELIVERY_COUNT.store(0, Ordering::SeqCst);
    LINUX_SIGNAL_RETURN_COUNT.store(0, Ordering::SeqCst);
    unsafe { core::ptr::addr_of_mut!(LINUX_SIGNAL_CONTEXT).write(LinuxSignalContext::ZERO) };
    let linux_clone_publication_valid = if execution.tls_size == 0 {
        true
    } else {
        (|| -> Option<bool> {
            let initial_stack = execution.stack & !(PAGE_SIZE - 1);
            let parent_tid = initial_stack.checked_add(128)? as u64;
            let child_tid = initial_stack.checked_add(132)? as u64;
            let child = reserve_linux_thread(sibling_tls, execution.sibling_stack, PAGE_SIZE)?;
            let request = LinuxCloneRequest {
                flags: LINUX_BOUNDED_THREAD_CLONE_FLAGS,
                child_stack: execution.sibling_stack.checked_add(PAGE_SIZE)? as u64,
                parent_tid,
                child_tid,
                tls: execution.sibling_fs_base as u64,
            };
            let rollback_request = LinuxCloneRequest {
                parent_tid: execution.sibling_stack as u64,
                child_tid: execution.sibling_stack.checked_add(4)? as u64,
                ..request
            };
            let rollback_rejected = matches!(
                publish_linux_clone(
                    child,
                    rollback_request,
                    LinuxSyscallFrame::ZERO,
                    execution.entry as u64,
                    0x202,
                    execution.sibling_stack as u64,
                    PAGE_SIZE,
                ),
                Err(LinuxClonePublishError::TidFault)
            );
            let slot = usize::from(child.slot);
            let rollback_clean = LINUX_THREAD_RIPS[slot].load(Ordering::SeqCst) == 0
                && LINUX_THREAD_RSPS[slot].load(Ordering::SeqCst) == 0
                && LINUX_THREAD_RFLAGS[slot].load(Ordering::SeqCst) == 0
                && LINUX_THREAD_RAXS[slot].load(Ordering::SeqCst) == 0
                && LINUX_THREAD_CLEAR_CHILD_TIDS[slot].load(Ordering::SeqCst) == 0
                && LINUX_THREAD_REGISTERS[slot]
                    .iter()
                    .all(|register| register.load(Ordering::SeqCst) == 0);
            // SAFETY: both words are inside the live initial stack mapping and
            // SMAP is relaxed only for these bounded reads and writes.
            let (saved_parent_tid, saved_child_tid) = unsafe {
                with_supervisor_user_access(|| {
                    (
                        (parent_tid as *const u32).read_volatile(),
                        (child_tid as *const u32).read_volatile(),
                    )
                })
            };
            let published = publish_linux_clone(
                child,
                request,
                LinuxSyscallFrame::ZERO,
                execution.entry as u64,
                0x202,
                initial_stack as u64,
                PAGE_SIZE,
            )
            .ok()?;
            let selected = linux_thread_return_context(child)?;
            // SAFETY: the same two live, bounded stack words are inspected and
            // restored before userspace receives its initial stack.
            let publication_visible = unsafe {
                with_supervisor_user_access(|| {
                    let visible = (parent_tid as *const u32).read_volatile() == child.tid
                        && (child_tid as *const u32).read_volatile() == child.tid;
                    (parent_tid as *mut u32).write_volatile(saved_parent_tid);
                    (child_tid as *mut u32).write_volatile(saved_child_tid);
                    visible
                })
            };
            Some(
                rollback_rejected
                    && rollback_clean
                    && published.parent_rax == u64::from(child.tid)
                    && selected == published.child
                    && publication_visible
                    && LINUX_THREAD_CLEAR_CHILD_TIDS[slot].load(Ordering::SeqCst) == child_tid
                    && retire_linux_thread(child)
                    && LINUX_THREAD_RIPS[slot].load(Ordering::SeqCst) == 0
                    && LINUX_THREAD_CLEAR_CHILD_TIDS[slot].load(Ordering::SeqCst) == 0,
            )
        })()
        .unwrap_or(false)
    };
    let tls_copy_bounds_valid = execution.tls_size == 0
        || linux_writable_user_range_contains(execution.tls as u64, execution.tls_size)
            && !linux_writable_user_range_contains(
                execution.sibling_tls as u64,
                execution.tls_size,
            )
            && !linux_writable_user_range_contains(execution.sibling_stack as u64, PAGE_SIZE)
            && execution
                .tls
                .checked_sub(1)
                .is_some_and(|before| !linux_writable_user_range_contains(before as u64, 1))
            && execution
                .tls
                .checked_add(execution.tls_size - 1)
                .is_some_and(|last| !linux_writable_user_range_contains(last as u64, 2));
    let tls_context_switch_valid = if execution.tls_size == 0 {
        true
    } else {
        let sibling_installed = install_linux_tls_context(sibling_tls);
        let sibling_valid = sibling_installed
            && linux_writable_user_range_contains(execution.sibling_tls as u64, execution.tls_size)
            && !linux_writable_user_range_contains(execution.tls as u64, execution.tls_size)
            && unsafe {
                with_supervisor_user_access(|| {
                    (execution.sibling_fs_base as *const u64).read_volatile()
                        == execution.sibling_fs_base as u64
                })
            };
        let initial_restored = install_linux_tls_context(initial_tls);
        sibling_valid
            && initial_restored
            && linux_writable_user_range_contains(execution.tls as u64, execution.tls_size)
            && !linux_writable_user_range_contains(execution.sibling_tls as u64, execution.tls_size)
    };
    let linux_wait_timer_armed = execution.tls_size == 0 || timer::arm_linux_wait_timer();
    if !linux_wait_timer_armed {
        if execution.runtime {
            serial_write("ginux: OCI wait timer setup failed\r\n");
        }
        let _ = restore_linux_syscall(saved_msrs);
        let _ = finish_linux_thread_table();
        ELF_PROBE_ACTIVE.store(false, Ordering::SeqCst);
        return LinuxElfResult::FAILED;
    }
    if !linux_mount::activate_initial() {
        if execution.runtime {
            serial_write("ginux: OCI mount setup failed\r\n");
        }
        let _ = restore_linux_syscall(saved_msrs);
        let _ = finish_linux_thread_table();
        ELF_PROBE_ACTIVE.store(false, Ordering::SeqCst);
        return LinuxElfResult::FAILED;
    }
    LINUX_RUNTIME_MODE.store(execution.runtime, Ordering::SeqCst);
    if execution.runtime {
        LINUX_RUNTIME_SUSPEND_REQUESTED.store(false, Ordering::Release);
        LINUX_RUNTIME_SUSPENDED.store(false, Ordering::Release);
    }
    if !install_live_linux_process_registry(execution.entry, execution.stack, active_fs_base()) {
        LINUX_RUNTIME_MODE.store(false, Ordering::SeqCst);
        let _ = linux_mount::deactivate_initial();
        let _ = restore_linux_syscall(saved_msrs);
        let _ = finish_linux_thread_table();
        ELF_PROBE_ACTIVE.store(false, Ordering::SeqCst);
        return LinuxElfResult::FAILED;
    }
    if execution.runtime && !restore_container_extended_state() {
        let _ = linux_process::finish_runtime_process_registry();
        let _ = linux_mount::deactivate_initial();
        let _ = restore_linux_syscall(saved_msrs);
        let _ = finish_linux_thread_table();
        ELF_PROBE_ACTIVE.store(false, Ordering::SeqCst);
        return LinuxElfResult::FAILED;
    }
    unsafe { ginux_user_enter(execution.entry, execution.stack, 0, 0, 0, 0, 0, 0, 0) };
    if execution.runtime {
        if LINUX_RUNTIME_SUSPENDED.load(Ordering::Acquire) {
            let state_saved = save_container_extended_state();
            let registry_suspended =
                state_saved && linux_process::suspend_runtime_process_registry();
            let mount_deactivated = registry_suspended && linux_mount::deactivate_initial();
            let msrs_restored = restore_linux_syscall(saved_msrs);
            LINUX_RUNTIME_MODE.store(false, Ordering::Release);
            return LinuxElfResult {
                valid: mount_deactivated && msrs_restored,
                suspended: mount_deactivated && msrs_restored,
                ..LinuxElfResult::FAILED
            };
        }
        return finish_linux_runtime(execution.mmap as u64, saved_msrs);
    }
    let live_fork_executed = execute_deferred_live_fork();
    let live_process_registry_finished =
        live_fork_executed && linux_process::finish_live_process_registry();
    if !live_process_registry_finished {
        serial_write("ginux: live Linux fork stages: ");
        serial_write_u64(u64::from(live_fork_executed));
        serial_write(" ");
        serial_write_u64(u64::from(linux_process::live_fork_execution_stage()));
        serial_write(" ");
        serial_write_u64(u64::from(linux_process::live_fork_child_diagnostic()));
        serial_write(" ");
        serial_write_u64(u64::from(
            PROCESS_CHILD_EXECUTION_DIAGNOSTIC.load(Ordering::SeqCst),
        ));
        serial_write(" ");
        serial_write_u64(u64::from(
            LIVE_FORK_PUBLIC_DIAGNOSTIC.load(Ordering::SeqCst),
        ));
        serial_write(" ");
        serial_write_u64(u64::from(LINUX_PROCESS_WAIT_COUNT.load(Ordering::SeqCst)));
        serial_write("\r\n");
    }
    let linux_wait_timer_finished = execution.tls_size == 0 || timer::finish_linux_wait_timer();
    let msrs_restored = restore_linux_syscall(saved_msrs);
    let linux_thread_table_released = finish_linux_thread_table();
    let inactive = !ELF_PROBE_ACTIVE.swap(false, Ordering::SeqCst);
    let completed = ELF_PROBE_COMPLETED.load(Ordering::SeqCst);
    let termination_signal = ELF_PROBE_TERMINATION_SIGNAL.swap(0, Ordering::SeqCst);
    let exec_replaced = ELF_PROBE_EXEC_REPLACED.swap(false, Ordering::SeqCst);
    let exec_node = ELF_PROBE_EXEC_NODE.swap(0, Ordering::SeqCst);
    let selected_static_exec_plan = if exec_replaced {
        initramfs::published_file_by_node(exec_node)
            .and_then(crate::linux_elf::measured_static_exec_plan)
    } else {
        None
    };
    let selected_interpreted_exec_plan = if exec_replaced
        && selected_static_exec_plan.is_none()
        && initramfs::published_file_name_by_node(exec_node) == Some(b"linux/init")
    {
        linux_measured_interpreted_exec_plan()
    } else {
        None
    };
    let selected_exec_plan = selected_static_exec_plan.as_ref().or_else(|| {
        selected_interpreted_exec_plan
            .as_ref()
            .map(|plan| &plan.process.interpreter)
    });
    let selected_runtime_plan = selected_static_exec_plan.as_ref().or_else(|| {
        selected_interpreted_exec_plan
            .as_ref()
            .map(|plan| &plan.process.main)
    });
    let selected_exec_mappings = selected_exec_plan
        .as_ref()
        .and_then(|plan| linux_exec_mapping_plan(plan));
    let interpreted_exec_replaced = selected_interpreted_exec_plan.is_some();
    let live_return_validated = LINUX_LIVE_RETURN_VALIDATED.swap(false, Ordering::SeqCst);
    let child_exit_count = LINUX_CHILD_EXIT_COUNT.swap(0, Ordering::SeqCst);
    let parent_resumed = LINUX_PARENT_RESUMED.swap(false, Ordering::SeqCst);
    let process_snapshot_validated = LINUX_PROCESS_SNAPSHOT_VALIDATED.swap(false, Ordering::SeqCst);
    let clone_dispatch_count = LINUX_CLONE_DISPATCH_COUNT.swap(0, Ordering::SeqCst);
    let first_child_tid = LINUX_FIRST_CHILD_TID.swap(0, Ordering::SeqCst);
    let clone_reuse_validated = LINUX_CLONE_REUSE_VALIDATED.swap(false, Ordering::SeqCst);
    let child_gettid_count = LINUX_CHILD_GETTID_COUNT.swap(0, Ordering::SeqCst);
    let yield_switch_count = LINUX_YIELD_SWITCH_COUNT.swap(0, Ordering::SeqCst);
    let clone_busy_rejection_count = LINUX_CLONE_BUSY_REJECTION_COUNT.swap(0, Ordering::SeqCst);
    let clone_invalid_rejection_count =
        LINUX_CLONE_INVALID_REJECTION_COUNT.swap(0, Ordering::SeqCst);
    let clone_tid_fault_count = LINUX_CLONE_TID_FAULT_COUNT.swap(0, Ordering::SeqCst);
    let futex_wait_block_count = LINUX_FUTEX_WAIT_BLOCK_COUNT.swap(0, Ordering::SeqCst);
    let futex_explicit_wake_count = LINUX_FUTEX_EXPLICIT_WAKE_COUNT.swap(0, Ordering::SeqCst);
    let futex_exit_wake_count = LINUX_FUTEX_EXIT_WAKE_COUNT.swap(0, Ordering::SeqCst);
    let futex_wait_timeout_count = LINUX_FUTEX_WAIT_TIMEOUT_COUNT.swap(0, Ordering::SeqCst);
    let sleep_block_count = LINUX_SLEEP_BLOCK_COUNT.swap(0, Ordering::SeqCst);
    let sleep_wake_count = LINUX_SLEEP_WAKE_COUNT.swap(0, Ordering::SeqCst);
    let sleep_interrupt_count = LINUX_SLEEP_INTERRUPT_COUNT.swap(0, Ordering::SeqCst);
    let timer_wait_expiry_count = LINUX_TIMER_WAIT_EXPIRY_COUNT.swap(0, Ordering::SeqCst);
    let timer_wait_preemption_count = LINUX_TIMER_WAIT_PREEMPTION_COUNT.swap(0, Ordering::SeqCst);
    let poll_wait_block_count = LINUX_POLL_WAIT_BLOCK_COUNT.swap(0, Ordering::SeqCst);
    let poll_wait_wake_count = LINUX_POLL_WAIT_WAKE_COUNT.swap(0, Ordering::SeqCst);
    let poll_wait_timeout_count = LINUX_POLL_WAIT_TIMEOUT_COUNT.swap(0, Ordering::SeqCst);
    let poll_wait_interrupt_count = LINUX_POLL_WAIT_INTERRUPT_COUNT.swap(0, Ordering::SeqCst);
    let epoll_wait_block_count = LINUX_EPOLL_WAIT_BLOCK_COUNT.swap(0, Ordering::SeqCst);
    let epoll_wait_wake_count = LINUX_EPOLL_WAIT_WAKE_COUNT.swap(0, Ordering::SeqCst);
    let epoll_wait_timeout_count = LINUX_EPOLL_WAIT_TIMEOUT_COUNT.swap(0, Ordering::SeqCst);
    let epoll_wait_interrupt_count = LINUX_EPOLL_WAIT_INTERRUPT_COUNT.swap(0, Ordering::SeqCst);
    let eventfd_read_block_count = LINUX_EVENTFD_READ_BLOCK_COUNT.swap(0, Ordering::SeqCst);
    let eventfd_read_wake_count = LINUX_EVENTFD_READ_WAKE_COUNT.swap(0, Ordering::SeqCst);
    let eventfd_read_interrupt_count = LINUX_EVENTFD_READ_INTERRUPT_COUNT.swap(0, Ordering::SeqCst);
    let eventfd_write_block_count = LINUX_EVENTFD_WRITE_BLOCK_COUNT.swap(0, Ordering::SeqCst);
    let eventfd_write_wake_count = LINUX_EVENTFD_WRITE_WAKE_COUNT.swap(0, Ordering::SeqCst);
    let eventfd_write_interrupt_count =
        LINUX_EVENTFD_WRITE_INTERRUPT_COUNT.swap(0, Ordering::SeqCst);
    let pipe_read_block_count = LINUX_PIPE_READ_BLOCK_COUNT.swap(0, Ordering::SeqCst);
    let pipe_read_wake_count = LINUX_PIPE_READ_WAKE_COUNT.swap(0, Ordering::SeqCst);
    let pipe_read_interrupt_count = LINUX_PIPE_READ_INTERRUPT_COUNT.swap(0, Ordering::SeqCst);
    let pipe_write_block_count = LINUX_PIPE_WRITE_BLOCK_COUNT.swap(0, Ordering::SeqCst);
    let pipe_write_wake_count = LINUX_PIPE_WRITE_WAKE_COUNT.swap(0, Ordering::SeqCst);
    let pipe_write_interrupt_count = LINUX_PIPE_WRITE_INTERRUPT_COUNT.swap(0, Ordering::SeqCst);
    let timerfd_read_block_count = LINUX_TIMERFD_READ_BLOCK_COUNT.swap(0, Ordering::SeqCst);
    let timerfd_read_wake_count = LINUX_TIMERFD_READ_WAKE_COUNT.swap(0, Ordering::SeqCst);
    let timerfd_read_interrupt_count = LINUX_TIMERFD_READ_INTERRUPT_COUNT.swap(0, Ordering::SeqCst);
    let ppoll_wait_block_count = LINUX_PPOLL_WAIT_BLOCK_COUNT.swap(0, Ordering::SeqCst);
    let ppoll_wait_wake_count = LINUX_PPOLL_WAIT_WAKE_COUNT.swap(0, Ordering::SeqCst);
    let ppoll_wait_timeout_count = LINUX_PPOLL_WAIT_TIMEOUT_COUNT.swap(0, Ordering::SeqCst);
    let epoll_pwait_block_count = LINUX_EPOLL_PWAIT_BLOCK_COUNT.swap(0, Ordering::SeqCst);
    let epoll_pwait_wake_count = LINUX_EPOLL_PWAIT_WAKE_COUNT.swap(0, Ordering::SeqCst);
    let epoll_pwait_timeout_count = LINUX_EPOLL_PWAIT_TIMEOUT_COUNT.swap(0, Ordering::SeqCst);
    let masked_wait_interrupt_count = LINUX_MASKED_WAIT_INTERRUPT_COUNT.swap(0, Ordering::SeqCst);
    let signal_wait_interrupt_count = LINUX_SIGNAL_WAIT_INTERRUPT_COUNT.swap(0, Ordering::SeqCst);
    let signal_delivery_count = LINUX_SIGNAL_DELIVERY_COUNT.swap(0, Ordering::SeqCst);
    let signal_return_count = LINUX_SIGNAL_RETURN_COUNT.swap(0, Ordering::SeqCst);
    let signal_wait_block_count = LINUX_SIGNAL_WAIT_BLOCK_COUNT.swap(0, Ordering::SeqCst);
    let signal_wait_wake_count = LINUX_SIGNAL_WAIT_WAKE_COUNT.swap(0, Ordering::SeqCst);
    let signal_wait_timeout_count = LINUX_SIGNAL_WAIT_TIMEOUT_COUNT.swap(0, Ordering::SeqCst);
    let system_supervisor_wait_count = LINUX_SYSTEM_SUPERVISOR_WAIT_COUNT.swap(0, Ordering::SeqCst);
    let system_supervisor_shutdown_count =
        LINUX_SYSTEM_SUPERVISOR_SHUTDOWN_COUNT.swap(0, Ordering::SeqCst);
    let system_init = ELF_PROBE_SYSTEM_INIT.swap(false, Ordering::SeqCst);
    let signal_probe_executed = exec_replaced
        && initramfs::published_file_name_by_node(exec_node) == Some(b"bin/signal-probe");
    let termination_signal_valid = termination_signal
        == if signal_probe_executed {
            LINUX_SIGPIPE as u32
        } else {
            0
        };
    let expected_signal_count = if signal_probe_executed {
        10
    } else if execution.tls_size != 0 {
        16
    } else {
        2
    };
    let signal_lifecycle_valid = !LINUX_SIGNAL_ACTIVE.swap(false, Ordering::SeqCst)
        && signal_delivery_count == expected_signal_count
        && signal_return_count == expected_signal_count
        && unsafe { core::ptr::addr_of!(LINUX_SIGNAL_CONTEXT).read() == LinuxSignalContext::ZERO };
    let system_supervisor_valid = if system_init {
        system_supervisor_wait_count == 1 && system_supervisor_shutdown_count == 1
    } else {
        system_supervisor_wait_count == 0 && system_supervisor_shutdown_count == 0
    };
    if execution.tls_size != 0
        && !(child_exit_count == 2
            && parent_resumed
            && clone_dispatch_count == 2
            && first_child_tid != 0
            && clone_reuse_validated
            && child_gettid_count == 2
            && yield_switch_count == 19
            && clone_busy_rejection_count == 2
            && clone_invalid_rejection_count == 0
            && clone_tid_fault_count == 1
            && futex_wait_block_count == 4
            && futex_explicit_wake_count == 2
            && futex_exit_wake_count == 1
            && futex_wait_timeout_count == 1
            && sleep_block_count == 2
            && sleep_wake_count == 1
            && sleep_interrupt_count == 1
            && timer_wait_expiry_count == 8
            && timer_wait_preemption_count == 9
            && poll_wait_block_count == 4
            && poll_wait_wake_count == 1
            && poll_wait_timeout_count == 2
            && poll_wait_interrupt_count == 1
            && epoll_wait_block_count == 2
            && epoll_wait_wake_count == 1
            && epoll_wait_timeout_count == 0
            && epoll_wait_interrupt_count == 1
            && eventfd_read_block_count == 2
            && eventfd_read_wake_count == 1
            && eventfd_read_interrupt_count == 1
            && eventfd_write_block_count == 3
            && eventfd_write_wake_count == 2
            && eventfd_write_interrupt_count == 1
            && pipe_read_block_count == 3
            && pipe_read_wake_count == 2
            && pipe_read_interrupt_count == 1
            && pipe_write_block_count == 6
            && pipe_write_wake_count == 4
            && pipe_write_interrupt_count == 2
            && timerfd_read_block_count == 2
            && timerfd_read_wake_count == 1
            && timerfd_read_interrupt_count == 1
            && ppoll_wait_block_count == 3
            && ppoll_wait_wake_count == 0
            && ppoll_wait_timeout_count == 2
            && epoll_pwait_block_count == 2
            && epoll_pwait_wake_count == 0
            && epoll_pwait_timeout_count == 1
            && masked_wait_interrupt_count == 2
            && signal_wait_block_count == 3
            && signal_wait_wake_count == 1
            && signal_wait_timeout_count == 1
            && signal_wait_interrupt_count == 1)
    {
        serial_write("ginux: Linux thread transition state:");
        for state in [
            child_exit_count == 2,
            parent_resumed,
            clone_dispatch_count == 2,
            first_child_tid != 0,
            clone_reuse_validated,
            child_gettid_count == 2,
            yield_switch_count == 19,
            clone_busy_rejection_count == 2,
            clone_invalid_rejection_count == 0,
            clone_tid_fault_count == 1,
            futex_wait_block_count == 4,
            futex_explicit_wake_count == 2,
            futex_exit_wake_count == 1,
            futex_wait_timeout_count == 1,
            sleep_block_count == 2,
            sleep_wake_count == 1,
            sleep_interrupt_count == 1,
            timer_wait_expiry_count == 8,
            timer_wait_preemption_count == 9,
            poll_wait_block_count == 4,
            poll_wait_wake_count == 1,
            poll_wait_timeout_count == 2,
            poll_wait_interrupt_count == 1,
            epoll_wait_block_count == 2,
            epoll_wait_wake_count == 1,
            epoll_wait_timeout_count == 0,
            epoll_wait_interrupt_count == 1,
            eventfd_read_block_count == 2,
            eventfd_read_wake_count == 1,
            eventfd_read_interrupt_count == 1,
            eventfd_write_block_count == 3,
            eventfd_write_wake_count == 2,
            eventfd_write_interrupt_count == 1,
            pipe_read_block_count == 3,
            pipe_read_wake_count == 2,
            pipe_read_interrupt_count == 1,
            pipe_write_block_count == 6,
            pipe_write_wake_count == 4,
            pipe_write_interrupt_count == 2,
            timerfd_read_block_count == 2,
            timerfd_read_wake_count == 1,
            timerfd_read_interrupt_count == 1,
            ppoll_wait_block_count == 3,
            ppoll_wait_wake_count == 0,
            ppoll_wait_timeout_count == 2,
            epoll_pwait_block_count == 2,
            epoll_pwait_wake_count == 0,
            epoll_pwait_timeout_count == 1,
            masked_wait_interrupt_count == 2,
            signal_wait_block_count == 3,
            signal_wait_wake_count == 1,
            signal_wait_timeout_count == 1,
            signal_wait_interrupt_count == 1,
        ] {
            serial_write(" ");
            serial_write_u64(u64::from(state));
        }
        serial_write("\r\n");
        serial_write("ginux: Linux thread transition counts:");
        for count in [
            child_exit_count,
            clone_dispatch_count,
            first_child_tid,
            child_gettid_count,
            yield_switch_count,
            clone_busy_rejection_count,
            clone_invalid_rejection_count,
            clone_tid_fault_count,
            futex_wait_block_count,
            futex_explicit_wake_count,
            futex_exit_wake_count,
            futex_wait_timeout_count,
            sleep_block_count,
            sleep_wake_count,
            sleep_interrupt_count,
            timer_wait_expiry_count,
            timer_wait_preemption_count,
            poll_wait_block_count,
            poll_wait_wake_count,
            poll_wait_timeout_count,
            poll_wait_interrupt_count,
            epoll_wait_block_count,
            epoll_wait_wake_count,
            epoll_wait_timeout_count,
            epoll_wait_interrupt_count,
            eventfd_read_block_count,
            eventfd_read_wake_count,
            eventfd_read_interrupt_count,
            eventfd_write_block_count,
            eventfd_write_wake_count,
            eventfd_write_interrupt_count,
            pipe_read_block_count,
            pipe_read_wake_count,
            pipe_read_interrupt_count,
            pipe_write_block_count,
            pipe_write_wake_count,
            pipe_write_interrupt_count,
            timerfd_read_block_count,
            timerfd_read_wake_count,
            timerfd_read_interrupt_count,
            ppoll_wait_block_count,
            ppoll_wait_wake_count,
            ppoll_wait_timeout_count,
            epoll_pwait_block_count,
            epoll_pwait_wake_count,
            epoll_pwait_timeout_count,
            masked_wait_interrupt_count,
            signal_wait_block_count,
            signal_wait_wake_count,
            signal_wait_timeout_count,
            signal_wait_interrupt_count,
        ] {
            serial_write(" ");
            serial_write_u64(u64::from(count));
        }
        serial_write("\r\n");
    }
    let umask_restored = ELF_PROBE_UMASK.swap(0, Ordering::SeqCst) == 0o022;
    let descriptor_snapshot = [
        u64::from(ELF_PROBE_CONSOLE_DESCRIPTORS.load(Ordering::SeqCst)),
        u64::from(ELF_PROBE_CONSOLE_CLOEXEC.load(Ordering::SeqCst)),
        u64::from(ELF_PROBE_DEVICE_DESCRIPTORS[0].load(Ordering::SeqCst)),
        u64::from(ELF_PROBE_DEVICE_DESCRIPTORS[1].load(Ordering::SeqCst)),
        u64::from(ELF_PROBE_DEVICE_DESCRIPTORS[2].load(Ordering::SeqCst)),
        u64::from(ELF_PROBE_DEVICE_DESCRIPTORS[3].load(Ordering::SeqCst)),
        u64::from(ELF_PROBE_EPOLL_DESCRIPTOR.load(Ordering::SeqCst)),
        ELF_PROBE_EPOLL_TARGET.load(Ordering::SeqCst),
        u64::from(ELF_PROBE_EPOLL_EVENTS.load(Ordering::SeqCst)),
        ELF_PROBE_EPOLL_DATA.load(Ordering::SeqCst),
        ELF_PROBE_DESCRIPTOR_OFFSETS[0].load(Ordering::SeqCst),
        ELF_PROBE_DESCRIPTOR_OFFSETS[1].load(Ordering::SeqCst),
        ELF_PROBE_DESCRIPTOR_OFFSETS[2].load(Ordering::SeqCst),
        ELF_PROBE_DESCRIPTOR_OFFSETS[3].load(Ordering::SeqCst),
        u64::from(ELF_PROBE_EVENTFD_DESCRIPTOR.load(Ordering::SeqCst)),
        ELF_PROBE_EVENTFD_COUNTER.load(Ordering::SeqCst),
        u64::from(ELF_PROBE_EVENTFD_FLAGS.load(Ordering::SeqCst)),
        u64::from(ELF_PROBE_TIMERFD_DESCRIPTOR.load(Ordering::SeqCst)),
        u64::from(ELF_PROBE_TIMERFD_FLAGS.load(Ordering::SeqCst)),
        ELF_PROBE_TIMERFD_INTERVAL.load(Ordering::SeqCst),
        ELF_PROBE_TIMERFD_DEADLINE.load(Ordering::SeqCst),
        u64::from(ELF_PROBE_PIPE_READ_DESCRIPTOR.load(Ordering::SeqCst)),
        u64::from(ELF_PROBE_PIPE_WRITE_DESCRIPTOR.load(Ordering::SeqCst)),
        u64::from(ELF_PROBE_PIPE_READ_FLAGS.load(Ordering::SeqCst)),
        u64::from(ELF_PROBE_PIPE_WRITE_FLAGS.load(Ordering::SeqCst)),
        u64::from(ELF_PROBE_PIPE_LENGTH.load(Ordering::SeqCst)),
    ];
    let mut devices_closed = true;
    for descriptor in &ELF_PROBE_DEVICE_DESCRIPTORS {
        devices_closed &= descriptor.swap(0, Ordering::SeqCst) == 0;
    }
    let mut oci_descriptors_closed = true;
    for descriptor in &ELF_PROBE_OCI_DESCRIPTORS {
        oci_descriptors_closed &= descriptor.swap(0, Ordering::SeqCst) == 0;
    }
    let mut offsets_reset = true;
    for offset in &ELF_PROBE_DESCRIPTOR_OFFSETS {
        offsets_reset &= offset.swap(0, Ordering::SeqCst) == 0;
    }
    let descriptor_state_valid = ELF_PROBE_CONSOLE_DESCRIPTORS.swap(0, Ordering::SeqCst) == 0
        && ELF_PROBE_CONSOLE_CLOEXEC.swap(0, Ordering::SeqCst) == 0
        && ELF_PROBE_CWD.swap(LINUX_ROOT_DIRECTORY, Ordering::SeqCst) == LINUX_ROOT_DIRECTORY
        && ELF_PROBE_OCI_CWD.swap(crate::oci_image::ROOT_HANDLE, Ordering::SeqCst)
            == crate::oci_image::ROOT_HANDLE
        && devices_closed
        && oci_descriptors_closed
        && ELF_PROBE_EPOLL_DESCRIPTOR.swap(0, Ordering::SeqCst) == 0
        && ELF_PROBE_EPOLL_TARGET.swap(u64::MAX, Ordering::SeqCst) == u64::MAX
        && ELF_PROBE_EPOLL_EVENTS.swap(0, Ordering::SeqCst) == 0
        && ELF_PROBE_EPOLL_DATA.swap(0, Ordering::SeqCst) == 0
        && ELF_PROBE_EVENTFD_DESCRIPTOR.swap(0, Ordering::SeqCst) == 0
        && ELF_PROBE_EVENTFD_COUNTER.swap(0, Ordering::SeqCst) == 0
        && ELF_PROBE_EVENTFD_FLAGS.swap(0, Ordering::SeqCst) == 0
        && ELF_PROBE_TIMERFD_DESCRIPTOR.swap(0, Ordering::SeqCst) == 0
        && ELF_PROBE_TIMERFD_FLAGS.swap(0, Ordering::SeqCst) == 0
        && ELF_PROBE_TIMERFD_INTERVAL.swap(0, Ordering::SeqCst) == 0
        && ELF_PROBE_TIMERFD_DEADLINE.swap(0, Ordering::SeqCst) == 0
        && ELF_PROBE_PIPE_READ_DESCRIPTOR.swap(0, Ordering::SeqCst) == 0
        && ELF_PROBE_PIPE_WRITE_DESCRIPTOR.swap(0, Ordering::SeqCst) == 0
        && ELF_PROBE_PIPE_READ_FLAGS.swap(0, Ordering::SeqCst) == 0
        && ELF_PROBE_PIPE_WRITE_FLAGS.swap(0, Ordering::SeqCst) == 0
        && ELF_PROBE_PIPE_LENGTH.swap(0, Ordering::SeqCst) == 0
        && offsets_reset;
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
    let mmap_inactive = (0..LINUX_RUNTIME_MMAP_SLOT_COUNT)
        .all(|index| ELF_PROBE_MMAP_PAGES[index].swap(0, Ordering::SeqCst) == 0);
    let mmap_protection_cleared = (0..LINUX_RUNTIME_MMAP_SLOT_COUNT)
        .all(|index| ELF_PROBE_MMAP_PROTECTION[index].swap(0, Ordering::SeqCst) == 0);
    let mmap_file_released = (0..LINUX_RUNTIME_MMAP_SLOT_COUNT)
        .all(|index| ELF_PROBE_MMAP_FILE[index].swap(0, Ordering::SeqCst) == 0);
    let mmap_address_valid = ELF_PROBE_MMAP.swap(0, Ordering::SeqCst) == execution.mmap as u64;
    let mmap_frames_valid = ELF_PROBE_MMAP_FRAMES
        .iter()
        .all(|frame| frame.swap(0, Ordering::SeqCst) == 0)
        && ELF_PROBE_MMAP_GENERATIONS
            .iter()
            .all(|generation| generation.swap(0, Ordering::SeqCst) == 0);
    let mmap_state_valid = mmap_inactive
        && mmap_protection_cleared
        && mmap_file_released
        && mmap_address_valid
        && mmap_frames_valid;
    let expected_main_mappings = execution
        .main_mappings
        .iter()
        .enumerate()
        .fold(0_u8, |mask, (index, mapping)| {
            mask | if mapping.address == 0 { 0 } else { 1 << index }
        });
    let observed_main_mappings = ELF_PROBE_MAIN_MAPPINGS.swap(0, Ordering::SeqCst);
    let main_frames = core::array::from_fn(|index| {
        memory::FrameLease::from_parts(
            ELF_PROBE_MAIN_FRAMES[index].swap(0, Ordering::SeqCst),
            ELF_PROBE_MAIN_GENERATIONS[index].swap(0, Ordering::SeqCst),
        )
    });
    let mut main_mapping_contract_valid = observed_main_mappings == expected_main_mappings;
    for (index, mapping) in execution.main_mappings.iter().enumerate() {
        let expected_protection = LINUX_PROT_READ
            | if mapping.writable {
                LINUX_PROT_WRITE
            } else {
                0
            }
            | if mapping.executable {
                LINUX_PROT_EXEC
            } else {
                0
            };
        main_mapping_contract_valid &= ELF_PROBE_MAIN_ADDRESSES[index].swap(0, Ordering::SeqCst)
            == mapping.address as u64
            && ELF_PROBE_MAIN_OFFSETS[index].swap(0, Ordering::SeqCst) == mapping.offset
            && ELF_PROBE_MAIN_FILE_SIZES[index].swap(0, Ordering::SeqCst)
                == mapping.file_size as u64
            && u64::from(ELF_PROBE_MAIN_PROTECTIONS[index].swap(0, Ordering::SeqCst))
                == expected_protection
            && match (mapping.address, main_frames[index]) {
                (0, None) => true,
                (0, Some(_)) | (_, None) => false,
                (_, Some(frame)) => memory::frames_owned_by(memory::FrameOwner::KERNEL, &[frame]),
            };
    }
    let mut observed_dependency_bases = [0_u64; LINUX_DEPENDENCY_COUNT];
    let mut dependency_frames = [[None; LINUX_DEPENDENCY_MAPPING_COUNT]; LINUX_DEPENDENCY_COUNT];
    let mut dependency_mappings_valid = true;
    for dependency in 0..LINUX_DEPENDENCY_COUNT {
        let expected_mappings = execution.dependency_mappings[dependency]
            .iter()
            .enumerate()
            .fold(0_u8, |mask, (index, mapping)| {
                mask | if mapping.file_size == 0 {
                    0
                } else {
                    1 << index
                }
            });
        let observed_mappings = ELF_PROBE_DEPENDENCY_MAPPINGS[dependency].swap(0, Ordering::SeqCst);
        let observed_base_min = ELF_PROBE_DEPENDENCY_BASE_MIN[dependency].swap(0, Ordering::SeqCst);
        let observed_base_end = ELF_PROBE_DEPENDENCY_BASE_END[dependency].swap(0, Ordering::SeqCst);
        let observed_base =
            ELF_PROBE_DEPENDENCY_SELECTED_BASE[dependency].swap(0, Ordering::SeqCst);
        observed_dependency_bases[dependency] = observed_base;
        dependency_mappings_valid &= observed_mappings == expected_mappings
            && observed_base_min == execution.dependency_base_min[dependency] as u64
            && observed_base_end == execution.dependency_base_end[dependency] as u64
            && if expected_mappings == 0 {
                observed_base == 0
            } else {
                observed_base >= execution.dependency_base_min[dependency] as u64
                    && observed_base < execution.dependency_base_end[dependency] as u64
                    && observed_base.is_multiple_of(PAGE_SIZE as u64)
            };
        for (index, mapping) in execution.dependency_mappings[dependency].iter().enumerate() {
            let state = dependency * LINUX_DEPENDENCY_MAPPING_COUNT + index;
            let frame = memory::FrameLease::from_parts(
                ELF_PROBE_DEPENDENCY_FRAMES[state].swap(0, Ordering::SeqCst),
                ELF_PROBE_DEPENDENCY_GENERATIONS[state].swap(0, Ordering::SeqCst),
            );
            dependency_frames[dependency][index] = frame;
            let expected_protection = LINUX_PROT_READ
                | if mapping.writable {
                    LINUX_PROT_WRITE
                } else {
                    0
                }
                | if mapping.executable {
                    LINUX_PROT_EXEC
                } else {
                    0
                };
            dependency_mappings_valid &= ELF_PROBE_DEPENDENCY_RELATIVES[state]
                .swap(0, Ordering::SeqCst)
                == mapping.relative_address as u64
                && ELF_PROBE_DEPENDENCY_OFFSETS[state].swap(0, Ordering::SeqCst) == mapping.offset
                && ELF_PROBE_DEPENDENCY_FILE_SIZES[state].swap(0, Ordering::SeqCst)
                    == mapping.file_size as u64
                && u64::from(ELF_PROBE_DEPENDENCY_PROTECTIONS[state].swap(0, Ordering::SeqCst))
                    == expected_protection
                && match (mapping.file_size, frame) {
                    (0, None) => true,
                    (0, Some(_)) | (_, None) => false,
                    (_, Some(frame)) => {
                        memory::frames_owned_by(memory::FrameOwner::KERNEL, &[frame])
                    }
                };
        }
        if observed_mappings != expected_mappings {
            serial_write("ginux: Linux dependency mapping contract mismatch: ");
            serial_write_u64(dependency as u64);
            serial_write(" ");
            serial_write_u64(u64::from(observed_mappings));
            serial_write(" ");
            serial_write_u64(u64::from(expected_mappings));
            serial_write("\r\n");
        }
    }
    let expected_process_name = if exec_replaced {
        linux_exec_process_name(exec_node).unwrap_or([0; 2])
    } else {
        [LINUX_INITIAL_PROCESS_NAME, 0]
    };
    let process_name_restored = ELF_PROBE_PROCESS_NAME_LOW.swap(0, Ordering::SeqCst)
        == expected_process_name[0]
        && ELF_PROBE_PROCESS_NAME_HIGH.swap(0, Ordering::SeqCst) == expected_process_name[1];
    let executable_identity_restored = if exec_replaced {
        selected_exec_plan.is_some()
    } else {
        exec_node == 0
    };
    let no_new_privileges_enforced = ELF_PROBE_NO_NEW_PRIVS.swap(false, Ordering::SeqCst);
    let signal_mask_restored = ELF_PROBE_SIGNAL_MASK.swap(0, Ordering::SeqCst) == 0;
    let pending_signals_cleared = ELF_PROBE_PENDING_SIGNALS.swap(0, Ordering::SeqCst) == 0;
    let pending_signal_codes_cleared = ELF_PROBE_PENDING_SIGNAL_CODES
        .iter()
        .all(|code| code.swap(0, Ordering::SeqCst) == 0);
    let ignored_signals = ELF_PROBE_EXEC_IGNORED_SIGNALS.swap(0, Ordering::SeqCst);
    let signal_actions_restored = (exec_replaced || ignored_signals == 0)
        && (0..LINUX_SIGNAL_COUNT).all(|signal| {
            let expected_handler = (ignored_signals >> signal) & 1;
            ELF_PROBE_SIGNAL_HANDLERS[signal].swap(0, Ordering::SeqCst) == expected_handler
                && ELF_PROBE_SIGNAL_FLAGS[signal].swap(0, Ordering::SeqCst) == 0
                && ELF_PROBE_SIGNAL_RESTORERS[signal].swap(0, Ordering::SeqCst) == 0
                && ELF_PROBE_SIGNAL_ACTION_MASKS[signal].swap(0, Ordering::SeqCst) == 0
        });
    let alt_stack_disabled = ELF_PROBE_ALT_STACK_POINTER.swap(0, Ordering::SeqCst) == 0
        && ELF_PROBE_ALT_STACK_SIZE.swap(0, Ordering::SeqCst) == 0
        && ELF_PROBE_ALT_STACK_FLAGS.swap(LINUX_SS_DISABLE, Ordering::SeqCst) == LINUX_SS_DISABLE;
    let expected_clear_child_tid = execution.data as u64 + LINUX_CLEAR_CHILD_TID_OFFSET;
    let expected_robust_list = execution.data as u64 + LINUX_ROBUST_LIST_OFFSET;
    let robust_list_cleared =
        ELF_PROBE_EXITED_ROBUST_LIST.swap(0, Ordering::SeqCst) == expected_robust_list;
    let protected_page = ELF_PROBE_READ_ONLY_DATA_PAGE.swap(0, Ordering::SeqCst);
    let clear_child_tid = ELF_PROBE_EXITED_CLEAR_CHILD_TID.swap(0, Ordering::SeqCst);
    let mut cleared_tid = [0xff_u8; size_of::<u32>()];
    let child_tid_cleared = clear_child_tid == expected_clear_child_tid
        && (exec_replaced
            || copy_from_linux_user(clear_child_tid, &mut cleared_tid)
                && cleared_tid == [0; size_of::<u32>()]);
    // SAFETY: the loader owns the mapped heap page through this verification,
    // and SMAP access is enabled only around the bounded read.
    let heap_contents_valid = unsafe {
        with_supervisor_user_access(|| {
            if exec_replaced {
                core::slice::from_raw_parts(execution.heap as *const u8, PAGE_SIZE)
                    .iter()
                    .all(|byte| *byte == 0)
            } else {
                (execution.heap as *const u64).read_volatile() == 0x4845_4150_4252_4b21
            }
        })
    };
    let observed_stack = ELF_PROBE_STACK.swap(0, Ordering::SeqCst);
    let current_stack_frame = memory::FrameLease::from_parts(
        ELF_PROBE_STACK_FRAME.swap(0, Ordering::SeqCst),
        ELF_PROBE_STACK_GENERATION.swap(0, Ordering::SeqCst),
    );
    let stack_frame_state_valid = observed_stack == execution.stack as u64
        && current_stack_frame.is_some_and(|frame| {
            address_space::current_linux_mapping_matches(
                (execution.stack & !(PAGE_SIZE - 1)) as u64,
                frame,
                true,
                false,
            ) && if exec_replaced {
                frame != execution.stack_frame
                    && !memory::frames_owned_by(
                        memory::FrameOwner::KERNEL,
                        &[execution.stack_frame],
                    )
            } else {
                frame == execution.stack_frame
            }
        });
    let exec_stack_frame = current_stack_frame.filter(|frame| *frame != execution.stack_frame);
    let heap_frame_state_valid =
        finalize_linux_exec_heap_frame(execution.heap, execution.heap_frame, exec_replaced);
    let expected_code = if exec_replaced {
        selected_runtime_plan
            .as_ref()
            .map_or(0, |plan| plan.executable.virtual_page as usize)
    } else {
        execution.code
    };
    let expected_code_size = if exec_replaced {
        selected_runtime_plan
            .as_ref()
            .map_or(0, |plan| plan.executable.page_count * PAGE_SIZE)
    } else {
        execution.code_size
    };
    let expected_data = if exec_replaced {
        selected_runtime_plan
            .as_ref()
            .map_or(0, |plan| plan.writable.virtual_page as usize)
    } else {
        execution.data
    };
    let expected_data_size = if exec_replaced {
        selected_runtime_plan
            .as_ref()
            .map_or(0, |plan| plan.writable.page_count * PAGE_SIZE)
    } else {
        execution.data_size
    };
    let runtime_image_state_valid = ELF_PROBE_CODE_BASE.swap(0, Ordering::SeqCst)
        == expected_code as u64
        && ELF_PROBE_CODE_SIZE.swap(0, Ordering::SeqCst) == expected_code_size as u64
        && ELF_PROBE_LOADER_CODE_BASE.swap(0, Ordering::SeqCst)
            == if exec_replaced {
                selected_interpreted_exec_plan
                    .as_ref()
                    .map_or(0, |plan| plan.process.interpreter.executable.virtual_page)
            } else {
                execution.loader_code as u64
            }
        && ELF_PROBE_LOADER_CODE_SIZE.swap(0, Ordering::SeqCst)
            == if exec_replaced {
                selected_interpreted_exec_plan.as_ref().map_or(0, |plan| {
                    (plan.process.interpreter.executable.page_count * PAGE_SIZE) as u64
                })
            } else {
                execution.loader_code_size as u64
            }
        && ELF_PROBE_DATA.swap(0, Ordering::SeqCst) == expected_data as u64
        && ELF_PROBE_DATA_SIZE.swap(0, Ordering::SeqCst) == expected_data_size as u64;
    let exec_frames = core::array::from_fn(|index| {
        memory::FrameLease::from_parts(
            ELF_PROBE_EXEC_MAPPING_FRAMES[index].swap(0, Ordering::SeqCst),
            ELF_PROBE_EXEC_MAPPING_GENERATIONS[index].swap(0, Ordering::SeqCst),
        )
    });
    let exec_frame_state_valid = if exec_replaced {
        selected_exec_mappings.is_some_and(|(_, count)| {
            exec_frames[..count].iter().all(Option::is_some)
                && exec_frames[count..].iter().all(Option::is_none)
        }) && exec_frames
            .iter()
            .flatten()
            .all(|frame| memory::frames_owned_by(memory::FrameOwner::KERNEL, &[*frame]))
    } else {
        exec_frames.iter().all(Option::is_none)
    };
    let observed_exec_entry = ELF_PROBE_EXEC_ENTRY.swap(0, Ordering::SeqCst);
    let observed_exec_addresses: [u64; LINUX_EXEC_MAPPING_LIMIT] = core::array::from_fn(|index| {
        ELF_PROBE_EXEC_MAPPING_ADDRESSES[index].swap(0, Ordering::SeqCst)
    });
    let observed_exec_flags: [u8; LINUX_EXEC_MAPPING_LIMIT] =
        core::array::from_fn(|index| ELF_PROBE_EXEC_MAPPING_FLAGS[index].swap(0, Ordering::SeqCst));
    let exec_image_contract_valid = match selected_exec_mappings {
        Some((mappings, mapping_count)) => {
            selected_exec_plan
                .as_ref()
                .is_some_and(|plan| observed_exec_entry == plan.entry)
                && mappings[..mapping_count]
                    .iter()
                    .enumerate()
                    .all(|(index, mapping)| {
                        observed_exec_addresses[index] == mapping.address as u64
                            && observed_exec_flags[index]
                                == u8::from(mapping.writable) | (u8::from(mapping.executable) << 1)
                    })
                && observed_exec_addresses[mapping_count..]
                    .iter()
                    .all(|address| *address == 0)
                && observed_exec_flags[mapping_count..]
                    .iter()
                    .all(|flags| *flags == 0)
        }
        None => {
            observed_exec_entry == 0
                && observed_exec_addresses == [0; LINUX_EXEC_MAPPING_LIMIT]
                && observed_exec_flags == [0; LINUX_EXEC_MAPPING_LIMIT]
        }
    };
    let active_image_count = usize::from(ELF_PROBE_ACTIVE_IMAGE_COUNT.swap(0, Ordering::SeqCst));
    let active_image_addresses: [u64; LINUX_ACTIVE_IMAGE_MAPPING_LIMIT] =
        core::array::from_fn(|index| {
            ELF_PROBE_ACTIVE_IMAGE_ADDRESSES[index].swap(0, Ordering::SeqCst)
        });
    let active_image_frames: [Option<memory::FrameLease>; LINUX_ACTIVE_IMAGE_MAPPING_LIMIT] =
        core::array::from_fn(|index| {
            memory::FrameLease::from_parts(
                ELF_PROBE_ACTIVE_IMAGE_FRAMES[index].swap(0, Ordering::SeqCst),
                ELF_PROBE_ACTIVE_IMAGE_GENERATIONS[index].swap(0, Ordering::SeqCst),
            )
        });
    let active_image_flags: [u8; LINUX_ACTIVE_IMAGE_MAPPING_LIMIT] =
        core::array::from_fn(|index| ELF_PROBE_ACTIVE_IMAGE_FLAGS[index].swap(0, Ordering::SeqCst));
    let retired_image_count = usize::from(ELF_PROBE_RETIRED_IMAGE_COUNT.swap(0, Ordering::SeqCst));
    let retired_image_addresses: [u64; LINUX_ACTIVE_IMAGE_MAPPING_LIMIT] =
        core::array::from_fn(|index| {
            ELF_PROBE_RETIRED_IMAGE_ADDRESSES[index].swap(0, Ordering::SeqCst)
        });
    let retired_image_frames: [Option<memory::FrameLease>; LINUX_ACTIVE_IMAGE_MAPPING_LIMIT] =
        core::array::from_fn(|index| {
            memory::FrameLease::from_parts(
                ELF_PROBE_RETIRED_IMAGE_FRAMES[index].swap(0, Ordering::SeqCst),
                ELF_PROBE_RETIRED_IMAGE_GENERATIONS[index].swap(0, Ordering::SeqCst),
            )
        });
    let retired_image_flags: [u8; LINUX_ACTIVE_IMAGE_MAPPING_LIMIT] =
        core::array::from_fn(|index| {
            ELF_PROBE_RETIRED_IMAGE_FLAGS[index].swap(0, Ordering::SeqCst)
        });
    let mut expected_active = [LinuxActiveImageMapping::EMPTY; LINUX_ACTIVE_IMAGE_MAPPING_LIMIT];
    let mut expected_active_count = 0;
    let active_seed_complete = if let Some((mappings, mapping_count)) = selected_exec_mappings {
        mappings[..mapping_count]
            .iter()
            .enumerate()
            .all(|(index, mapping)| {
                exec_frames[index].is_some_and(|frame| {
                    expected_active[index] = LinuxActiveImageMapping {
                        address: mapping.address,
                        frame,
                        writable: mapping.writable,
                        executable: mapping.executable,
                    };
                    expected_active_count += 1;
                    true
                })
            })
    } else {
        for mapping in &execution.initial_image_mappings[..execution.initial_image_count] {
            expected_active[expected_active_count] = LinuxActiveImageMapping {
                writable: mapping.writable && mapping.address as u64 != protected_page,
                ..*mapping
            };
            expected_active_count += 1;
        }
        true
    };
    let mut append_active = |mapping: LinuxActiveImageMapping| {
        if mapping == LinuxActiveImageMapping::EMPTY
            || expected_active_count == expected_active.len()
            || expected_active[..expected_active_count]
                .iter()
                .any(|prior| prior.address == mapping.address || prior.frame == mapping.frame)
        {
            return false;
        }
        expected_active[expected_active_count] = mapping;
        expected_active_count += 1;
        true
    };
    let main_complete = execution
        .main_mappings
        .iter()
        .enumerate()
        .filter(|(_, mapping)| mapping.address != 0)
        .all(|(index, mapping)| {
            main_frames[index].is_some_and(|frame| {
                append_active(LinuxActiveImageMapping {
                    address: mapping.address,
                    frame,
                    writable: mapping.writable && mapping.address as u64 != protected_page,
                    executable: mapping.executable,
                })
            })
        });
    let dependencies_complete =
        execution
            .dependency_mappings
            .iter()
            .enumerate()
            .all(|(dependency, mappings)| {
                mappings
                    .iter()
                    .enumerate()
                    .filter(|(_, mapping)| mapping.file_size != 0)
                    .all(|(index, mapping)| {
                        observed_dependency_bases[dependency]
                            .checked_add(mapping.relative_address as u64)
                            .and_then(|address| usize::try_from(address).ok())
                            .zip(dependency_frames[dependency][index])
                            .is_some_and(|(address, frame)| {
                                append_active(LinuxActiveImageMapping {
                                    address,
                                    frame,
                                    writable: mapping.writable && address as u64 != protected_page,
                                    executable: mapping.executable,
                                })
                            })
                    })
            });
    let active_image_state_valid = active_seed_complete
        && main_complete
        && dependencies_complete
        && active_image_count == expected_active_count
        && (0..active_image_count).all(|index| {
            let flags = active_image_flags[index];
            flags & !3 == 0
                && flags != 3
                && active_image_frames[index].is_some_and(|frame| {
                    expected_active[..expected_active_count].contains(&LinuxActiveImageMapping {
                        address: active_image_addresses[index] as usize,
                        frame,
                        writable: flags & 1 != 0,
                        executable: flags & 2 != 0,
                    })
                })
        })
        && active_image_addresses[active_image_count..]
            .iter()
            .all(|address| *address == 0)
        && active_image_frames[active_image_count..]
            .iter()
            .all(Option::is_none)
        && active_image_flags[active_image_count..]
            .iter()
            .all(|flags| *flags == 0);
    let exec_contract_state_valid = exec_image_contract_valid && active_image_state_valid;
    let mut retired_image_leases = [memory::FrameLease::EMPTY; LINUX_ACTIVE_IMAGE_MAPPING_LIMIT];
    for (output, frame) in retired_image_leases
        .iter_mut()
        .zip(retired_image_frames.iter().flatten())
    {
        *output = *frame;
    }
    let old_image_retirement_valid = if exec_replaced {
        (execution.initial_image_count..=LINUX_ACTIVE_IMAGE_MAPPING_LIMIT)
            .contains(&retired_image_count)
            && (0..retired_image_count).all(|index| {
                let flags = retired_image_flags[index];
                retired_image_frames[index].is_some_and(|frame| {
                    retired_image_addresses[index] != 0
                        && retired_image_addresses[index].is_multiple_of(PAGE_SIZE as u64)
                        && flags & !3 == 0
                        && flags != 3
                        && !memory::frames_owned_by(memory::FrameOwner::KERNEL, &[frame])
                        && (0..index).all(|prior| {
                            retired_image_addresses[prior] != retired_image_addresses[index]
                                && retired_image_frames[prior] != Some(frame)
                        })
                })
            })
            && execution.initial_image_mappings[..execution.initial_image_count]
                .iter()
                .all(|mapping| {
                    (0..retired_image_count).any(|index| {
                        retired_image_addresses[index] == mapping.address as u64
                            && retired_image_frames[index] == Some(mapping.frame)
                            && retired_image_flags[index]
                                == u8::from(
                                    mapping.writable && mapping.address as u64 != protected_page,
                                ) | (u8::from(mapping.executable) << 1)
                    })
                })
            && retired_image_addresses[retired_image_count..]
                .iter()
                .all(|address| *address == 0)
            && retired_image_frames[retired_image_count..]
                .iter()
                .all(Option::is_none)
            && retired_image_flags[retired_image_count..]
                .iter()
                .all(|flags| *flags == 0)
            && !memory::scrub_and_release_frames(
                memory::FrameOwner::KERNEL,
                &retired_image_leases[..retired_image_count],
            )
    } else {
        retired_image_count == 0
            && retired_image_addresses.iter().all(|address| *address == 0)
            && retired_image_frames.iter().all(Option::is_none)
            && retired_image_flags.iter().all(|flags| *flags == 0)
            && execution.initial_image_mappings[..execution.initial_image_count]
                .iter()
                .all(|mapping| {
                    memory::frames_owned_by(memory::FrameOwner::KERNEL, &[mapping.frame])
                })
    };
    let tls_state_valid = ELF_PROBE_TLS.swap(0, Ordering::SeqCst) == execution.tls as u64
        && ELF_PROBE_TLS_SIZE.swap(0, Ordering::SeqCst) == execution.tls_size as u64;
    let tls_frame_state_valid = finalize_linux_exec_tls_frames(
        [
            execution.tls & !(PAGE_SIZE - 1),
            execution.sibling_tls & !(PAGE_SIZE - 1),
        ],
        execution.tls_frames,
        interpreted_exec_replaced,
    );
    let sibling_stack_frame_state_valid = finalize_linux_exec_sibling_stack_frame(
        execution.sibling_stack,
        execution.sibling_stack_frame,
        interpreted_exec_replaced,
    );
    let sibling_thread_resources_cleared = ELF_PROBE_SIBLING_FS_BASE.swap(0, Ordering::SeqCst)
        == execution.sibling_fs_base as u64
        && ELF_PROBE_SIBLING_TLS.swap(0, Ordering::SeqCst) == execution.sibling_tls as u64
        && ELF_PROBE_SIBLING_STACK.swap(0, Ordering::SeqCst) == execution.sibling_stack as u64;
    ELF_PROBE_RELRO.store(0, Ordering::SeqCst);
    ELF_PROBE_RELRO_SIZE.store(0, Ordering::SeqCst);
    let heap_break_state_valid = ELF_PROBE_BRK.swap(0, Ordering::SeqCst) == execution.heap as u64
        && ELF_PROBE_BRK_LIMIT.swap(0, Ordering::SeqCst) == heap_limit as u64;
    let mount_namespace_valid =
        linux_mount::current_root_is_read_only() && linux_mount::deactivate_initial();
    let valid = inactive
        && completed
        && exec_replaced == execution.exec_expected
        && live_return_validated
        && process_snapshot_validated
        && mount_namespace_valid
        && (execution.tls_size == 0
            || child_exit_count == 2
                && parent_resumed
                && clone_dispatch_count == 2
                && first_child_tid != 0
                && clone_reuse_validated
                && child_gettid_count == 2
                && yield_switch_count == 19
                && clone_busy_rejection_count == 2
                && clone_invalid_rejection_count == 0
                && clone_tid_fault_count == 1
                && futex_wait_block_count == 4
                && futex_explicit_wake_count == 2
                && futex_exit_wake_count == 1
                && futex_wait_timeout_count == 1
                && sleep_block_count == 2
                && sleep_wake_count == 1
                && sleep_interrupt_count == 1
                && timer_wait_expiry_count == 8
                && timer_wait_preemption_count == 9
                && poll_wait_block_count == 4
                && poll_wait_wake_count == 1
                && poll_wait_timeout_count == 2
                && poll_wait_interrupt_count == 1
                && epoll_wait_block_count == 2
                && epoll_wait_wake_count == 1
                && epoll_wait_timeout_count == 0
                && epoll_wait_interrupt_count == 1
                && eventfd_read_block_count == 2
                && eventfd_read_wake_count == 1
                && eventfd_read_interrupt_count == 1
                && eventfd_write_block_count == 3
                && eventfd_write_wake_count == 2
                && eventfd_write_interrupt_count == 1
                && pipe_read_block_count == 3
                && pipe_read_wake_count == 2
                && pipe_read_interrupt_count == 1
                && pipe_write_block_count == 6
                && pipe_write_wake_count == 4
                && pipe_write_interrupt_count == 2
                && timerfd_read_block_count == 2
                && timerfd_read_wake_count == 1
                && timerfd_read_interrupt_count == 1
                && ppoll_wait_block_count == 3
                && ppoll_wait_wake_count == 0
                && ppoll_wait_timeout_count == 2
                && epoll_pwait_block_count == 2
                && epoll_pwait_wake_count == 0
                && epoll_pwait_timeout_count == 1
                && masked_wait_interrupt_count == 2
                && signal_wait_block_count == 3
                && signal_wait_wake_count == 1
                && signal_wait_timeout_count == 1
                && signal_wait_interrupt_count == 1)
        && child_tid_cleared
        && robust_list_cleared
        && protected_page == execution.relro as u64
        && heap_contents_valid
        && heap_frame_state_valid
        && heap_break_state_valid
        && umask_restored
        && descriptor_state_valid
        && clock_state_valid
        && mmap_state_valid
        && main_mapping_contract_valid
        && dependency_mappings_valid
        && runtime_image_state_valid
        && exec_contract_state_valid
        && exec_frame_state_valid
        && stack_frame_state_valid
        && old_image_retirement_valid
        && tls_state_valid
        && tls_frame_state_valid
        && sibling_stack_frame_state_valid
        && sibling_thread_resources_cleared
        && tls_copy_bounds_valid
        && tls_context_switch_valid
        && linux_clone_publication_valid
        && linux_thread_table_released
        && linux_wait_timer_finished
        && process_name_restored
        && executable_identity_restored
        && no_new_privileges_enforced
        && signal_lifecycle_valid
        && system_supervisor_valid
        && termination_signal_valid
        && signal_mask_restored
        && pending_signals_cleared
        && pending_signal_codes_cleared
        && signal_actions_restored
        && alt_stack_disabled
        && live_process_registry_finished
        && msrs_restored;
    if !valid {
        serial_write("ginux: Linux final state:");
        for state in [
            inactive,
            completed,
            exec_replaced == execution.exec_expected,
            live_return_validated,
            process_snapshot_validated,
            child_tid_cleared,
            robust_list_cleared,
            protected_page == execution.relro as u64,
            heap_contents_valid,
            heap_frame_state_valid,
            heap_break_state_valid,
            umask_restored,
            descriptor_state_valid,
            clock_state_valid,
            mmap_state_valid,
            main_mapping_contract_valid,
            dependency_mappings_valid,
            runtime_image_state_valid,
            exec_contract_state_valid,
            exec_frame_state_valid,
            stack_frame_state_valid,
            old_image_retirement_valid,
            tls_state_valid,
            tls_frame_state_valid,
            sibling_stack_frame_state_valid,
            sibling_thread_resources_cleared,
            tls_copy_bounds_valid,
            tls_context_switch_valid,
            linux_clone_publication_valid,
            linux_thread_table_released,
            linux_wait_timer_finished,
            process_name_restored,
            executable_identity_restored,
            no_new_privileges_enforced,
            signal_lifecycle_valid,
            system_supervisor_valid,
            termination_signal_valid,
            signal_mask_restored,
            pending_signals_cleared,
            pending_signal_codes_cleared,
            signal_actions_restored,
            alt_stack_disabled,
            live_process_registry_finished,
            mount_namespace_valid,
            msrs_restored,
        ] {
            serial_write(" ");
            serial_write_u64(u64::from(state));
        }
        serial_write("\r\n");
    }
    let exec_mapping_count = selected_exec_mappings.map_or(0, |(_, count)| count);
    let mut returned_exec_frames = exec_frames;
    if exec_replaced && exec_mapping_count < LINUX_EXEC_MAPPING_LIMIT {
        returned_exec_frames[exec_mapping_count] = exec_stack_frame;
    }
    LinuxElfResult {
        valid,
        suspended: false,
        exec_replaced,
        exec_node,
        active_image_mapping_count: active_image_count,
        dependency_bases: observed_dependency_bases,
        main_frames,
        dependency_frames,
        exec_mapping_count,
        exec_frames: returned_exec_frames,
        retired_image_count,
        retired_image_frames,
    }
}

fn install_linux_tls_context(context: LinuxTlsContext) -> bool {
    if !context.is_valid() {
        return false;
    }
    if context.extent_size == 0 {
        unsafe { write_msr(FS_BASE_MSR, 0) };
        if unsafe { read_msr(FS_BASE_MSR) } != 0 {
            return false;
        }
        ELF_PROBE_TLS.store(0, Ordering::SeqCst);
        ELF_PROBE_TLS_SIZE.store(0, Ordering::SeqCst);
        return true;
    }
    unsafe { write_msr(FS_BASE_MSR, context.thread_pointer as u64) };
    if unsafe { read_msr(FS_BASE_MSR) } != context.thread_pointer as u64 {
        return false;
    }
    ELF_PROBE_TLS.store(context.extent as u64, Ordering::SeqCst);
    ELF_PROBE_TLS_SIZE.store(context.extent_size as u64, Ordering::SeqCst);
    true
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

/// Removes a temporary kernel interrupt gate while interrupts are disabled.
pub(crate) fn clear_interrupt_gate(vector: u8) -> bool {
    if vector <= 32 || vector == SYSCALL_VECTOR || vector == u8::MAX {
        return false;
    }
    let idt = core::ptr::addr_of_mut!(IDT).cast::<IdtEntry>();
    // SAFETY: The caller has quiesced the interrupt source and keeps
    // interrupts disabled while replacing this non-reserved gate.
    unsafe {
        idt.add(usize::from(vector)).write(IdtEntry::MISSING);
        idt.add(usize::from(vector)).read().is_missing()
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
    match PROCESS_CHILD_PROBE_STATE.load(Ordering::SeqCst) {
        PROCESS_CHILD_PROBE_EXPECT_CONTEXT => {
            let valid = (!frame.is_null())
                .then(|| unsafe { &*frame })
                .is_some_and(|frame| {
                    frame
                        .process_general_registers(number)
                        .into_iter()
                        .zip(&PROCESS_CHILD_EXPECTED_REGISTERS)
                        .all(|(observed, expected)| observed == expected.load(Ordering::SeqCst))
                        && active_fs_base() == PROCESS_CHILD_EXPECTED_FS_BASE.load(Ordering::SeqCst)
                });
            PROCESS_CHILD_CONTEXT_VALID.store(valid, Ordering::SeqCst);
            PROCESS_CHILD_PROBE_STATE.store(
                if valid {
                    PROCESS_CHILD_PROBE_EXPECT_EXIT
                } else {
                    PROCESS_CHILD_PROBE_IDLE
                },
                Ordering::SeqCst,
            );
            return if valid { 0 } else { u64::MAX };
        }
        PROCESS_CHILD_PROBE_EXPECT_EXIT => {
            let exit_status = (!frame.is_null())
                .then(|| unsafe { &*frame })
                .filter(|_| {
                    number == LINUX_EXIT_SYSCALL
                        && active_fs_base() == PROCESS_CHILD_EXPECTED_FS_BASE.load(Ordering::SeqCst)
                })
                .and_then(|frame| u8::try_from(frame.process_general_registers(number)[5]).ok());
            if let Some(status) = exit_status {
                PROCESS_CHILD_EXIT_STATUS.store(status, Ordering::SeqCst);
                PROCESS_CHILD_PROBE_STATE.store(PROCESS_CHILD_PROBE_COMPLETE, Ordering::SeqCst);
            } else {
                PROCESS_CHILD_PROBE_STATE.store(PROCESS_CHILD_PROBE_IDLE, Ordering::SeqCst);
            }
            return u64::MAX;
        }
        _ => {}
    }
    let Some(caller_domain) = scheduler::current_domain() else {
        return u64::MAX;
    };
    let _ = timer::cancel_packet_wait_timer_for_domain(caller_domain);
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
    registers: *mut LinuxSyscallFrame,
    user_stack: u64,
    user_rip: u64,
    user_rflags: u64,
) -> u64 {
    if registers.is_null() {
        ELF_PROBE_ACTIVE.store(false, Ordering::SeqCst);
        return LINUX_RETURN_TO_KERNEL;
    }
    // SAFETY: the assembly entry stub passes the aligned base of the exact
    // twelve-register frame represented by LinuxSyscallFrame.
    let saved_registers = unsafe { registers.read() };
    let syscall_return = LinuxUserContext {
        registers: saved_registers,
        rip: user_rip,
        rsp: user_stack,
        rflags: user_rflags,
        rax: 0,
        fs_base: active_fs_base(),
    };
    let register = |index: usize| saved_registers.argument(index).unwrap_or(0);
    let count = register(3);
    let buffer = register(4);
    let first_argument = register(5);
    let code_start = ELF_PROBE_CODE_BASE.load(Ordering::SeqCst);
    let code_end = code_start.saturating_add(ELF_PROBE_CODE_SIZE.load(Ordering::SeqCst));
    let loader_start = ELF_PROBE_LOADER_CODE_BASE.load(Ordering::SeqCst);
    let loader_end = loader_start.saturating_add(ELF_PROBE_LOADER_CODE_SIZE.load(Ordering::SeqCst));
    if !ELF_PROBE_ACTIVE.load(Ordering::SeqCst)
        || !linux_user_stack_pointer_valid(user_stack)
        || (!(code_start..code_end).contains(&user_rip)
            && !(loader_start..loader_end).contains(&user_rip))
        || user_rflags & ((3 << 12) | (1 << 17)) != 0
    {
        if LINUX_RUNTIME_MODE.load(Ordering::SeqCst) {
            serial_write("ginux: Alpine invalid syscall context:");
            for value in [number, user_stack, user_rip, user_rflags] {
                serial_write(" ");
                serial_write_u64(value);
            }
            serial_write("\r\n");
        }
        ELF_PROBE_ACTIVE.store(false, Ordering::SeqCst);
        return LINUX_RETURN_TO_KERNEL;
    }
    let runtime_mode = LINUX_RUNTIME_MODE.load(Ordering::SeqCst);
    if runtime_mode {
        let index = LINUX_RUNTIME_SYSCALL_TRACE_COUNT.fetch_add(1, Ordering::SeqCst) as usize
            % LINUX_RUNTIME_SYSCALL_TRACE_LIMIT;
        LINUX_RUNTIME_SYSCALL_TRACE[index].store(number, Ordering::SeqCst);
    }
    if !runtime_mode
        && current_linux_process_pid() == 1
        && current_linux_tid() == Some(1)
        && !refresh_live_linux_process_registry(syscall_return)
    {
        if LINUX_RUNTIME_MODE.load(Ordering::SeqCst) {
            serial_write("ginux: live process registry refresh failed\r\n");
        }
        ELF_PROBE_ACTIVE.store(false, Ordering::SeqCst);
        return LINUX_RETURN_TO_KERNEL;
    }
    if current_linux_tid() == Some(1) && LINUX_CHILD_EXIT_COUNT.load(Ordering::SeqCst) != 0 {
        LINUX_PARENT_RESUMED.store(true, Ordering::SeqCst);
    }
    expire_linux_waits();
    match number {
        LINUX_READ_SYSCALL => linux_descriptor_read_live_return(
            registers,
            syscall_return,
            first_argument,
            buffer,
            count,
        ),
        LINUX_PREAD64_SYSCALL => {
            linux_regular_file_read_at(first_argument, buffer, count, register(0))
        }
        LINUX_WRITE_SYSCALL => linux_descriptor_write_live_return(
            registers,
            syscall_return,
            first_argument,
            buffer,
            count,
        ),
        LINUX_SCHED_YIELD_SYSCALL => linux_sched_yield_live_return(
            registers,
            saved_registers,
            user_stack,
            user_rip,
            user_rflags,
        ),
        LINUX_CLOSE_SYSCALL => {
            linux_descriptor_close_live_return(registers, syscall_return, first_argument)
        }
        LINUX_POLL_SYSCALL => {
            linux_poll_live_return(registers, syscall_return, first_argument, buffer, count)
        }
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
            if linux_eventfd_descriptor(first_argument)
                || linux_timerfd_descriptor(first_argument)
                || linux_pipe_descriptor(first_argument)
            {
                return LINUX_ESPIPE;
            }
            let device = linux_device_descriptor(first_argument);
            if device & LINUX_DEVICE_CONSOLE != 0 {
                LINUX_ESPIPE
            } else if linux_oci_entry(first_argument).is_some()
                || linux_regular_file(device).is_some()
                || linux_tmp_file(device)
            {
                linux_regular_file_seek(first_argument, device, buffer, count)
            } else if device & LINUX_DEVICE_DIRECTORY != 0 {
                if buffer == 0 && count == 0 {
                    linux_descriptor_offset(first_argument).store(0, Ordering::SeqCst);
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
            if !first_argument.is_multiple_of(PAGE_SIZE as u64) || buffer == 0 {
                return LINUX_EINVAL;
            }
            let relro = ELF_PROBE_RELRO.load(Ordering::SeqCst);
            let relro_size = ELF_PROBE_RELRO_SIZE.load(Ordering::SeqCst);
            let relro_active_index = linux_active_image_writable_index(first_argument);
            if LINUX_RUNTIME_MODE.load(Ordering::SeqCst)
                && count == LINUX_PROT_READ
                && buffer.is_multiple_of(PAGE_SIZE as u64)
                && address_space::protect_current_linux_range_read_only(first_argument, buffer)
            {
                let end = first_argument.saturating_add(buffer);
                let active_count = usize::from(ELF_PROBE_ACTIVE_IMAGE_COUNT.load(Ordering::SeqCst));
                for index in 0..active_count {
                    let address = ELF_PROBE_ACTIVE_IMAGE_ADDRESSES[index].load(Ordering::SeqCst);
                    if (first_argument..end).contains(&address) {
                        ELF_PROBE_ACTIVE_IMAGE_FLAGS[index].fetch_and(!1, Ordering::SeqCst);
                    }
                }
                return 0;
            }
            if count == LINUX_PROT_READ
                && first_argument == relro
                && buffer == PAGE_SIZE as u64
                && relro_size != 0
                && relro_size <= PAGE_SIZE as u64
                && ELF_PROBE_READ_ONLY_DATA_PAGE.load(Ordering::SeqCst) == 0
            {
                if let Some(relro_active_index) = relro_active_index {
                    if address_space::protect_current_user_page_read_only(first_argument) {
                        ELF_PROBE_ACTIVE_IMAGE_FLAGS[relro_active_index].store(0, Ordering::SeqCst);
                        ELF_PROBE_READ_ONLY_DATA_PAGE.store(first_argument, Ordering::SeqCst);
                        return 0;
                    }
                }
            }
            linux_mprotect_mmap(first_argument, buffer, count)
        }
        LINUX_IOCTL_SYSCALL => {
            if linux_pipe_descriptor(first_argument) {
                if buffer != LINUX_FIONREAD {
                    return LINUX_ENOTTY;
                }
                let available = ELF_PROBE_PIPE_LENGTH.load(Ordering::SeqCst);
                if !copy_to_linux_user(count, &available.to_ne_bytes()) {
                    return LINUX_EFAULT;
                }
                return 0;
            }
            let device = linux_device_descriptor(first_argument);
            if device != 0 && device & LINUX_DEVICE_CONSOLE == 0 {
                return LINUX_ENOTTY;
            }
            if device == 0 && !linux_console_descriptor_open(first_argument) {
                return if linux_descriptor_open(first_argument) {
                    LINUX_ENOTTY
                } else {
                    LINUX_EBADF
                };
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
        LINUX_READV_SYSCALL => linux_descriptor_readv_live_return(
            registers,
            syscall_return,
            first_argument,
            buffer,
            count,
        ),
        LINUX_WRITEV_SYSCALL => linux_descriptor_writev_live_return(
            registers,
            syscall_return,
            first_argument,
            buffer,
            count,
        ),
        LINUX_PIPE_SYSCALL => linux_pipe(first_argument, 0),
        LINUX_MINCORE_SYSCALL => linux_mincore(first_argument, buffer, count),
        LINUX_MADVISE_SYSCALL => linux_madvise(first_argument, buffer, count),
        LINUX_DUP_SYSCALL => linux_console_dup(first_argument),
        LINUX_DUP2_SYSCALL => linux_console_dup2(first_argument, buffer),
        LINUX_DUP3_SYSCALL if count == 0 => linux_console_dup2(first_argument, buffer),
        LINUX_DUP3_SYSCALL => LINUX_EINVAL,
        LINUX_NANOSLEEP_SYSCALL => {
            linux_nanosleep_live_return(registers, syscall_return, first_argument, buffer)
        }
        LINUX_CLOCK_NANOSLEEP_SYSCALL => linux_clock_nanosleep_live_return(
            registers,
            syscall_return,
            first_argument,
            buffer,
            count,
            register(0),
        ),
        LINUX_GETITIMER_SYSCALL => linux_getitimer(first_argument, buffer),
        LINUX_SETITIMER_SYSCALL => linux_setitimer(first_argument, buffer, count),
        LINUX_EPOLL_WAIT_SYSCALL => linux_epoll_wait_live_return(
            registers,
            syscall_return,
            LinuxEpollWaitInput {
                epoll: first_argument,
                address: buffer,
                maximum: count,
            },
            register(0),
        ),
        LINUX_EPOLL_CTL_SYSCALL => linux_epoll_ctl(first_argument, buffer, count, register(0)),
        LINUX_FCNTL_SYSCALL => linux_descriptor_fcntl(first_argument, buffer, count),
        LINUX_BRK_SYSCALL => {
            let current = ELF_PROBE_BRK.load(Ordering::SeqCst);
            let limit = ELF_PROBE_BRK_LIMIT.load(Ordering::SeqCst);
            let base = limit.saturating_sub(
                (if LINUX_RUNTIME_MODE.load(Ordering::SeqCst) {
                    LINUX_RUNTIME_HEAP_PAGES
                } else {
                    1
                } * PAGE_SIZE) as u64,
            );
            if first_argument == 0 {
                current
            } else if (base..=limit).contains(&first_argument) {
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
            let result = linux_rt_sigprocmask(first_argument, buffer, count, register(0));
            if result == 0 {
                linux_deliver_pending_signal(registers, syscall_return, (user_rip, user_rflags))
            } else {
                result
            }
        }
        LINUX_RT_SIGRETURN_SYSCALL => linux_rt_sigreturn(registers, user_stack),
        LINUX_RT_SIGPENDING_SYSCALL => linux_rt_sigpending(first_argument, buffer),
        LINUX_RT_SIGTIMEDWAIT_SYSCALL => linux_rt_sigtimedwait_live_return(
            registers,
            syscall_return,
            first_argument,
            buffer,
            count,
            register(0),
        ),
        LINUX_CLONE_SYSCALL => linux_clone_thread(saved_registers, user_rip, user_rflags),
        LINUX_FORK_SYSCALL | LINUX_VFORK_SYSCALL => {
            if current_linux_process_pid() != 1
                || current_linux_tid() != Some(1)
                || (runtime_mode && !refresh_live_linux_process_registry(syscall_return))
                || LINUX_FORK_REQUESTED
                    .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
                    .is_err()
            {
                LINUX_EAGAIN
            } else {
                LINUX_RETURN_TO_KERNEL
            }
        }
        LINUX_EXECVE_SYSCALL => linux_execve(first_argument, buffer, count),
        LINUX_KILL_SYSCALL => linux_kill(
            first_argument,
            (buffer, LINUX_SI_USER),
            registers,
            syscall_return,
            (user_rip, user_rflags),
        ),
        LINUX_TGKILL_SYSCALL => match linux_tgkill_identity(first_argument, buffer) {
            Ok(target) if current_linux_thread_handle() == Some(target) => linux_kill(
                1,
                (count, LINUX_SI_TKILL),
                registers,
                syscall_return,
                (user_rip, user_rflags),
            ),
            Ok(target) => linux_interrupt_blocked_wait(
                target,
                (count, LINUX_SI_TKILL),
                registers,
                syscall_return,
            ),
            Err(error) => error,
        },
        LINUX_TKILL_SYSCALL => match linux_tkill_identity(first_argument) {
            Ok(()) => linux_kill(
                1,
                (buffer, LINUX_SI_TKILL),
                registers,
                syscall_return,
                (user_rip, user_rflags),
            ),
            Err(error) => error,
        },
        LINUX_EXECVEAT_SYSCALL => {
            linux_execveat(first_argument, buffer, count, register(0), register(2))
        }
        LINUX_GETPID_SYSCALL => u64::from(current_linux_process_pid()),
        LINUX_GETTID_SYSCALL => {
            let Some(tid) = current_linux_tid() else {
                return LINUX_ESRCH;
            };
            if tid != 1 {
                LINUX_CHILD_GETTID_COUNT.fetch_add(1, Ordering::SeqCst);
            }
            u64::from(tid)
        }
        LINUX_GETPPID_SYSCALL => u64::from((current_linux_process_pid() != 1) as u32),
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
            if !set_current_linux_clear_child_tid(first_argument) {
                return LINUX_ESRCH;
            }
            current_linux_tid().map_or(LINUX_ESRCH, u64::from)
        }
        LINUX_SET_ROBUST_LIST_SYSCALL => {
            if buffer != LINUX_ROBUST_LIST_HEAD_SIZE as u64 {
                return LINUX_EINVAL;
            }
            if !linux_writable_user_range_contains(first_argument, LINUX_ROBUST_LIST_HEAD_SIZE) {
                return LINUX_EFAULT;
            }
            if !set_current_linux_robust_list(first_argument) {
                return LINUX_ESRCH;
            }
            0
        }
        LINUX_GET_ROBUST_LIST_SYSCALL => {
            let current_tid = current_linux_tid().map_or(0, u64::from);
            if first_argument != 0 && first_argument != current_tid {
                return LINUX_ESRCH;
            }
            if !copy_linux_robust_list(buffer, count) {
                return LINUX_EFAULT;
            }
            0
        }
        LINUX_GETUID_SYSCALL => {
            current_linux_credentials().map_or(LINUX_ESRCH, |value| u64::from(value.uid))
        }
        LINUX_GETGID_SYSCALL => {
            current_linux_credentials().map_or(LINUX_ESRCH, |value| u64::from(value.gid))
        }
        LINUX_GETEUID_SYSCALL => {
            current_linux_credentials().map_or(LINUX_ESRCH, |value| u64::from(value.euid))
        }
        LINUX_GETEGID_SYSCALL => {
            current_linux_credentials().map_or(LINUX_ESRCH, |value| u64::from(value.egid))
        }
        LINUX_GETGROUPS_SYSCALL => {
            let Some(credentials) = current_linux_credentials() else {
                return LINUX_ESRCH;
            };
            if (first_argument as i32) < 0 {
                return LINUX_EINVAL;
            }
            if first_argument == 0 {
                return 1;
            }
            if !copy_to_linux_user(buffer, &credentials.supplementary_gid.to_ne_bytes()) {
                return LINUX_EFAULT;
            }
            1
        }
        LINUX_SETGROUPS_SYSCALL => {
            let Some(namespace) = current_linux_user_namespace() else {
                return LINUX_ESRCH;
            };
            if linux_user::setgroups_allowed(namespace) {
                LINUX_ENOSYS
            } else {
                LINUX_EPERM
            }
        }
        LINUX_GETRESUID_SYSCALL | LINUX_GETRESGID_SYSCALL => {
            let Some(credentials) = current_linux_credentials() else {
                return LINUX_ESRCH;
            };
            let ids = if number == LINUX_GETRESUID_SYSCALL {
                (credentials.uid, credentials.euid, credentials.suid)
            } else {
                (credentials.gid, credentials.egid, credentials.sgid)
            };
            if !copy_linux_id_triple(first_argument, buffer, count, ids) {
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
                && (buffer == ELF_PROBE_DATA.load(Ordering::SeqCst)
                    || LINUX_RUNTIME_MODE.load(Ordering::SeqCst)
                        && address_space::current_linux_range_mapped(
                            buffer,
                            size_of::<u64>(),
                            true,
                        )) =>
        {
            // SAFETY: proof fixtures accept only their declared data base;
            // runtime images accept a mapped writable user TLS location. The
            // supervisor FS base is restored before returning to the kernel.
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
        LINUX_GETCWD_SYSCALL => linux_getcwd(first_argument, buffer),
        LINUX_CHDIR_SYSCALL => linux_chdir(first_argument),
        LINUX_FCHDIR_SYSCALL => linux_fchdir(first_argument),
        LINUX_READLINK_SYSCALL => linux_readlink_self(first_argument, buffer, count),
        LINUX_OPEN_SYSCALL => linux_openat(u64::from(LINUX_AT_FDCWD), first_argument, buffer),
        LINUX_OPENAT_SYSCALL => linux_openat(first_argument, buffer, count),
        LINUX_STAT_SYSCALL => {
            linux_newfstatat(u64::from(LINUX_AT_FDCWD), first_argument, buffer, 0)
        }
        LINUX_LSTAT_SYSCALL => linux_newfstatat(
            u64::from(LINUX_AT_FDCWD),
            first_argument,
            buffer,
            LINUX_AT_SYMLINK_NOFOLLOW,
        ),
        LINUX_NEWFSTATAT_SYSCALL => linux_newfstatat(first_argument, buffer, count, register(0)),
        LINUX_UNLINKAT_SYSCALL => linux_unlinkat(first_argument, buffer, count),
        LINUX_READLINKAT_SYSCALL => {
            if first_argument as u32 != LINUX_AT_FDCWD {
                return LINUX_EBADF;
            }
            linux_readlink_self(buffer, count, register(0))
        }
        LINUX_EPOLL_CREATE1_SYSCALL => linux_epoll_create1(first_argument),
        LINUX_PIPE2_SYSCALL => linux_pipe(first_argument, buffer),
        LINUX_TIMERFD_CREATE_SYSCALL => linux_timerfd_create(first_argument, buffer),
        LINUX_EVENTFD_SYSCALL => linux_eventfd(first_argument, 0),
        LINUX_EVENTFD2_SYSCALL => linux_eventfd(first_argument, buffer),
        LINUX_TIMERFD_SETTIME_SYSCALL => {
            linux_timerfd_settime(first_argument, buffer, count, register(0))
        }
        LINUX_TIMERFD_GETTIME_SYSCALL => linux_timerfd_gettime(first_argument, buffer),
        LINUX_EPOLL_PWAIT_SYSCALL => linux_epoll_pwait_live_return(
            registers,
            syscall_return,
            LinuxEpollWaitInput {
                epoll: first_argument,
                address: buffer,
                maximum: count,
            },
            register(0),
            LinuxSignalMaskInput {
                address: register(2),
                size: register(1),
            },
        ),
        LINUX_EPOLL_PWAIT2_SYSCALL => linux_epoll_pwait2_live_return(
            registers,
            syscall_return,
            LinuxEpollWaitInput {
                epoll: first_argument,
                address: buffer,
                maximum: count,
            },
            register(0),
            LinuxSignalMaskInput {
                address: register(2),
                size: register(1),
            },
        ),
        LINUX_PPOLL_SYSCALL => linux_ppoll_live_return(
            registers,
            syscall_return,
            first_argument,
            buffer,
            count,
            register(0),
            register(2),
        ),
        LINUX_FACCESSAT_SYSCALL => linux_faccessat(first_argument, buffer, count, 0),
        LINUX_FACCESSAT2_SYSCALL => linux_faccessat(first_argument, buffer, count, register(0)),
        LINUX_ACCESS_SYSCALL => {
            linux_faccessat(u64::from(LINUX_AT_FDCWD), first_argument, buffer, 0)
        }
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
        LINUX_FUTEX_SYSCALL => linux_futex_live_return(
            registers,
            LinuxUserContext {
                registers: saved_registers,
                rip: user_rip,
                rsp: user_stack,
                rflags: user_rflags,
                rax: 0,
                fs_base: active_fs_base(),
            },
            first_argument,
            buffer,
            count,
            register(0),
        ),
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
        LINUX_STATX_SYSCALL => linux_statx(first_argument, buffer, count, register(0), register(2)),
        LINUX_WAIT4_SYSCALL => linux_wait4(first_argument, buffer, count, register(0)),
        LINUX_EXIT_SYSCALL | LINUX_EXIT_GROUP_SYSCALL
            if first_argument <= u64::from(u8::MAX) && current_linux_process_pid() != 1 =>
        {
            LINUX_PROCESS_CHILD_STATUS.store(first_argument as u8, Ordering::SeqCst);
            LINUX_PROCESS_CHILD_EXITED.store(true, Ordering::SeqCst);
            LINUX_RETURN_TO_KERNEL
        }
        LINUX_EXIT_SYSCALL | LINUX_EXIT_GROUP_SYSCALL
            if LINUX_RUNTIME_MODE.load(Ordering::SeqCst)
                && first_argument <= u64::from(u8::MAX) =>
        {
            LINUX_RUNTIME_EXIT_STATUS.store(first_argument, Ordering::SeqCst);
            retire_all_linux_threads();
            ELF_PROBE_COMPLETED.store(true, Ordering::SeqCst);
            ELF_PROBE_ACTIVE.store(false, Ordering::SeqCst);
            LINUX_RETURN_TO_KERNEL
        }
        LINUX_EXIT_SYSCALL
            if first_argument == LINUX_PROBE_EXIT_STATUS && current_linux_tid() != Some(1) =>
        {
            linux_exit_child_to_parent(registers)
        }
        LINUX_EXIT_SYSCALL | LINUX_EXIT_GROUP_SYSCALL
            if first_argument == LINUX_PROBE_EXIT_STATUS =>
        {
            if !record_live_linux_process_snapshot() {
                return LINUX_ESRCH;
            }
            if ELF_PROBE_EXEC_REPLACED.load(Ordering::SeqCst) {
                if current_linux_clear_child_tid() != Some(0)
                    || current_linux_robust_list() != Some(0)
                    || !linux_close_all_descriptors()
                    || !linux_tmpfs::teardown()
                    || !retire_current_linux_thread()
                {
                    return LINUX_ESRCH;
                }
                if initramfs::published_file_name_by_node(
                    ELF_PROBE_EXEC_NODE.load(Ordering::SeqCst),
                ) == Some(b"linux/init")
                {
                    serial_write("DYNEXEC\r\n");
                }
                ELF_PROBE_COMPLETED.store(true, Ordering::SeqCst);
                ELF_PROBE_ACTIVE.store(false, Ordering::SeqCst);
                return LINUX_RETURN_TO_KERNEL;
            }
            let clear_child_tid = current_linux_clear_child_tid().unwrap_or(0);
            let robust_list = current_linux_robust_list().unwrap_or(0);
            if clear_child_tid == 0 || !copy_to_linux_user(clear_child_tid, &0_u32.to_ne_bytes()) {
                return LINUX_EFAULT;
            }
            if robust_list == 0
                || !set_current_linux_clear_child_tid(0)
                || !set_current_linux_robust_list(0)
                || !linux_close_all_descriptors()
                || !linux_tmpfs::teardown()
            {
                return LINUX_ESRCH;
            }
            ELF_PROBE_EXITED_CLEAR_CHILD_TID.store(clear_child_tid, Ordering::SeqCst);
            ELF_PROBE_EXITED_ROBUST_LIST.store(robust_list, Ordering::SeqCst);
            if !retire_current_linux_thread() {
                return LINUX_ESRCH;
            }
            ELF_PROBE_COMPLETED.store(true, Ordering::SeqCst);
            ELF_PROBE_ACTIVE.store(false, Ordering::SeqCst);
            LINUX_RETURN_TO_KERNEL
        }
        number if linux_privileged_operation(number).is_some() => {
            linux_deny_privileged_operation(number)
        }
        _ => {
            let _ = LINUX_RUNTIME_FIRST_UNSUPPORTED_SYSCALL.compare_exchange(
                u64::MAX,
                number,
                Ordering::SeqCst,
                Ordering::SeqCst,
            );
            LINUX_ENOSYS
        }
    }
}

pub(crate) fn linux_runtime_diagnostics() -> (Option<u8>, Option<u64>) {
    let status = LINUX_RUNTIME_EXIT_STATUS.load(Ordering::SeqCst);
    let syscall = LINUX_RUNTIME_FIRST_UNSUPPORTED_SYSCALL.load(Ordering::SeqCst);
    (
        u8::try_from(status).ok(),
        (syscall != u64::MAX).then_some(syscall),
    )
}

pub(crate) fn write_linux_runtime_syscall_trace() {
    let count = LINUX_RUNTIME_SYSCALL_TRACE_COUNT.load(Ordering::SeqCst) as usize;
    let retained = count.min(LINUX_RUNTIME_SYSCALL_TRACE_LIMIT);
    let start = count.saturating_sub(retained);
    crate::serial_write_bytes(b"ginux: OCI init recent syscalls:");
    for offset in 0..retained {
        let index = (start + offset) % LINUX_RUNTIME_SYSCALL_TRACE_LIMIT;
        crate::serial_write_bytes(b" ");
        crate::serial_write_u64(LINUX_RUNTIME_SYSCALL_TRACE[index].load(Ordering::SeqCst));
    }
    crate::serial_write_bytes(b"\r\n");
}

fn linux_privileged_operation(number: u64) -> Option<linux_user::PrivilegedOperation> {
    use linux_user::PrivilegedOperation;
    match number {
        LINUX_SETUID_SYSCALL
        | LINUX_SETGID_SYSCALL
        | LINUX_SETREUID_SYSCALL
        | LINUX_SETREGID_SYSCALL
        | LINUX_SETRESUID_SYSCALL
        | LINUX_SETRESGID_SYSCALL
        | LINUX_SETFSUID_SYSCALL
        | LINUX_SETFSGID_SYSCALL => Some(PrivilegedOperation::ChangeIdentity),
        LINUX_CHROOT_SYSCALL | LINUX_PIVOT_ROOT_SYSCALL => Some(PrivilegedOperation::ChangeRoot),
        LINUX_MOUNT_SYSCALL
        | LINUX_UMOUNT2_SYSCALL
        | LINUX_SWAPON_SYSCALL
        | LINUX_SWAPOFF_SYSCALL => Some(PrivilegedOperation::ManageMounts),
        LINUX_REBOOT_SYSCALL
        | LINUX_SETHOSTNAME_SYSCALL
        | LINUX_SETDOMAINNAME_SYSCALL
        | LINUX_INIT_MODULE_SYSCALL
        | LINUX_DELETE_MODULE_SYSCALL
        | LINUX_KEXEC_LOAD_SYSCALL
        | LINUX_FINIT_MODULE_SYSCALL
        | LINUX_KEXEC_FILE_LOAD_SYSCALL => Some(PrivilegedOperation::ManageKernel),
        LINUX_UNSHARE_SYSCALL | LINUX_SETNS_SYSCALL => Some(PrivilegedOperation::ManageNamespaces),
        LINUX_IOPL_SYSCALL | LINUX_IOPERM_SYSCALL | LINUX_PERF_EVENT_OPEN_SYSCALL => {
            Some(PrivilegedOperation::RawIo)
        }
        LINUX_BPF_SYSCALL => Some(PrivilegedOperation::LoadBpf),
        _ => None,
    }
}

fn linux_deny_privileged_operation(number: u64) -> u64 {
    let Some(namespace) = current_linux_user_namespace() else {
        return LINUX_ESRCH;
    };
    let Some(operation) = linux_privileged_operation(number) else {
        return LINUX_ENOSYS;
    };
    if linux_user::privileged_operation_denied(namespace, operation) {
        LINUX_EPERM
    } else {
        LINUX_ENOSYS
    }
}

fn linux_clone_thread(registers: LinuxSyscallFrame, user_rip: u64, user_rflags: u64) -> u64 {
    if current_linux_tid() != Some(1) {
        return LINUX_EAGAIN;
    }
    if ready_linux_thread_handle().is_some() {
        LINUX_CLONE_BUSY_REJECTION_COUNT.fetch_add(1, Ordering::SeqCst);
        return LINUX_EAGAIN;
    }
    let sibling_fs = ELF_PROBE_SIBLING_FS_BASE.load(Ordering::SeqCst);
    let sibling_tls = ELF_PROBE_SIBLING_TLS.load(Ordering::SeqCst);
    let sibling_stack = ELF_PROBE_SIBLING_STACK.load(Ordering::SeqCst);
    let tls_size = ELF_PROBE_TLS_SIZE.load(Ordering::SeqCst);
    let request = LinuxCloneRequest {
        flags: registers.rdi,
        child_stack: registers.rsi,
        parent_tid: registers.rdx,
        child_tid: registers.r8,
        tls: registers.r9,
    };
    if sibling_fs == 0
        || sibling_tls == 0
        || sibling_stack == 0
        || tls_size == 0
        || request.child_stack != sibling_stack + PAGE_SIZE as u64
        || request.tls != sibling_fs
    {
        return LINUX_EINVAL;
    }
    let sibling_context = LinuxTlsContext {
        thread_pointer: sibling_fs as usize,
        extent: sibling_tls as usize,
        extent_size: tls_size as usize,
    };
    let prior_dispatch_count = LINUX_CLONE_DISPATCH_COUNT.load(Ordering::SeqCst);
    // SAFETY: the authenticated sibling stack remains mapped RW/NX for this
    // process, and SMAP is relaxed only for the bounded pre-reuse inspection.
    let reused_stack_scrubbed = prior_dispatch_count == 1
        && unsafe {
            with_supervisor_user_access(|| {
                core::slice::from_raw_parts(sibling_stack as *const u8, PAGE_SIZE)
                    .iter()
                    .all(|byte| *byte == 0)
            })
        };
    let Some(child) = reserve_linux_thread(sibling_context, sibling_stack as usize, PAGE_SIZE)
    else {
        return LINUX_EAGAIN;
    };
    let data_base = ELF_PROBE_DATA.load(Ordering::SeqCst);
    let Ok(data_size) = usize::try_from(ELF_PROBE_DATA_SIZE.load(Ordering::SeqCst)) else {
        let _ = retire_linux_thread(child);
        return LINUX_EFAULT;
    };
    let result = match publish_linux_clone(
        child,
        request,
        registers,
        user_rip,
        user_rflags,
        data_base,
        data_size,
    ) {
        Ok(result) => result,
        Err(error) => {
            let _ = retire_linux_thread(child);
            return match error {
                LinuxClonePublishError::InvalidRequest => {
                    LINUX_CLONE_INVALID_REJECTION_COUNT.fetch_add(1, Ordering::SeqCst);
                    LINUX_EINVAL
                }
                LinuxClonePublishError::TidFault => {
                    LINUX_CLONE_TID_FAULT_COUNT.fetch_add(1, Ordering::SeqCst);
                    LINUX_EFAULT
                }
            };
        }
    };
    let dispatch_index = LINUX_CLONE_DISPATCH_COUNT.fetch_add(1, Ordering::SeqCst);
    if dispatch_index == 0 {
        LINUX_FIRST_CHILD_TID.store(child.tid, Ordering::SeqCst);
    } else if dispatch_index == 1
        && reused_stack_scrubbed
        && child.tid > LINUX_FIRST_CHILD_TID.load(Ordering::SeqCst)
    {
        LINUX_CLONE_REUSE_VALIDATED.store(true, Ordering::SeqCst);
    }
    result.parent_rax
}

fn linux_exit_child_to_parent(output: *mut LinuxSyscallFrame) -> u64 {
    let clear_child_tid = current_linux_clear_child_tid().unwrap_or(0);
    if clear_child_tid != 0 && !copy_to_linux_user(clear_child_tid, &0_u32.to_ne_bytes()) {
        return LINUX_EFAULT;
    }
    let woken = wake_linux_futex(clear_child_tid, 1);
    if !set_current_linux_clear_child_tid(0)
        || !set_current_linux_robust_list(0)
        || !retire_current_linux_thread()
    {
        return LINUX_ESRCH;
    }
    let Some(parent) = ready_linux_thread_handle() else {
        return LINUX_ESRCH;
    };
    let mut registers = LinuxSyscallFrame::ZERO;
    let mut rip = 0;
    let mut rsp = 0;
    let mut rflags = 0;
    let mut rax = 0;
    if !install_linux_thread_return(
        parent,
        &mut registers,
        &mut rip,
        &mut rsp,
        &mut rflags,
        &mut rax,
    ) {
        return LINUX_ESRCH;
    }
    let context = LinuxUserContext {
        registers,
        rip,
        rsp,
        rflags,
        rax,
        fs_base: active_fs_base(),
    };
    if !write_linux_live_return(output, context) {
        return LINUX_EINVAL;
    }
    if woken == 1 {
        LINUX_FUTEX_EXIT_WAKE_COUNT.fetch_add(1, Ordering::SeqCst);
    }
    LINUX_CHILD_EXIT_COUNT.fetch_add(1, Ordering::SeqCst);
    context.rax
}

fn current_linux_process_pid() -> u32 {
    LINUX_CURRENT_PROCESS_PID.load(Ordering::SeqCst)
}

fn current_linux_user_namespace() -> Option<linux_user::UserNamespaceHandle> {
    linux_process::live_process_user_namespace(current_linux_process_pid())
}

fn current_linux_credentials() -> Option<linux_user::Credentials> {
    linux_user::credentials(current_linux_user_namespace()?)
}

fn linux_wait4(pid: u64, status_address: u64, options: u64, rusage: u64) -> u64 {
    if current_linux_process_pid() != 1 || current_linux_tid() != Some(1) {
        return LINUX_ECHILD;
    }
    if options != 0 || rusage != 0 {
        return LINUX_EINVAL;
    }
    if status_address != 0 && !linux_writable_user_range_contains(status_address, size_of::<u32>())
    {
        return LINUX_EFAULT;
    }
    let waited = if pid == u64::MAX {
        linux_process::wait_live_child_any(1)
    } else {
        u32::try_from(pid)
            .ok()
            .and_then(|pid| linux_process::wait_live_child(1, pid))
    };
    let Some((waited, status)) = waited else {
        return LINUX_ECHILD;
    };
    let encoded = u32::from(status) << 8;
    if status_address != 0 && !copy_to_linux_user(status_address, &encoded.to_ne_bytes()) {
        return LINUX_EFAULT;
    }
    LINUX_PROCESS_WAIT_COUNT.fetch_add(1, Ordering::SeqCst);
    u64::from(waited)
}

fn linux_sched_yield_live_return(
    output: *mut LinuxSyscallFrame,
    registers: LinuxSyscallFrame,
    user_stack: u64,
    user_rip: u64,
    user_rflags: u64,
) -> u64 {
    let Some(current) = current_linux_thread_handle() else {
        return LINUX_ESRCH;
    };
    let Some(tls) = linux_thread_context(current) else {
        return LINUX_ESRCH;
    };
    let caller = LinuxUserContext {
        registers,
        rip: user_rip,
        rsp: user_stack,
        rflags: user_rflags,
        rax: 0,
        fs_base: tls.thread_pointer as u64,
    };
    if !save_linux_thread_return_context(current, caller) {
        return LINUX_EINVAL;
    }
    let target = ready_linux_thread_handle().unwrap_or(current);
    let mut target_registers = LinuxSyscallFrame::ZERO;
    let mut target_rip = 0;
    let mut target_rsp = 0;
    let mut target_rflags = 0;
    let mut target_rax = 0;
    if target == current {
        if !install_linux_tls_context(tls) {
            return LINUX_EINVAL;
        }
        target_registers = caller.registers;
        target_rip = caller.rip;
        target_rsp = caller.rsp;
        target_rflags = caller.rflags;
        target_rax = caller.rax;
    } else if !install_linux_thread_return(
        target,
        &mut target_registers,
        &mut target_rip,
        &mut target_rsp,
        &mut target_rflags,
        &mut target_rax,
    ) {
        return LINUX_EINVAL;
    } else {
        LINUX_YIELD_SWITCH_COUNT.fetch_add(1, Ordering::SeqCst);
    }
    let selected = LinuxUserContext {
        registers: target_registers,
        rip: target_rip,
        rsp: target_rsp,
        rflags: target_rflags,
        rax: target_rax,
        fs_base: active_fs_base(),
    };
    if !write_linux_live_return(output, selected) {
        return LINUX_EINVAL;
    }
    LINUX_LIVE_RETURN_VALIDATED.store(true, Ordering::SeqCst);
    selected.rax
}

fn write_linux_live_return(output: *mut LinuxSyscallFrame, context: LinuxUserContext) -> bool {
    if output.is_null() || !context.is_valid() {
        return false;
    }
    // SAFETY: the entry assembly passes the live, aligned syscall frame and
    // reads these return globals only after this handler returns. Interrupts
    // remain disabled for the serialized update.
    unsafe {
        output.write(context.registers);
        LINUX_USER_RSP = context.rsp;
        LINUX_USER_RIP = context.rip;
        LINUX_USER_RFLAGS = context.rflags;
    }
    // SAFETY: the same serialized frame and globals remain exclusively owned
    // by this syscall entry until the handler returns.
    unsafe {
        output.read() == context.registers
            && LINUX_USER_RSP == context.rsp
            && LINUX_USER_RIP == context.rip
            && LINUX_USER_RFLAGS == context.rflags
    }
}

fn save_suspended_linux_runtime(context: LinuxUserContext, address: u64, count: u64) -> bool {
    if !context.is_valid()
        || address == 0
        || count == 0
        || LINUX_RUNTIME_SUSPENDED.swap(true, Ordering::AcqRel)
    {
        return false;
    }
    for (stored, value) in LINUX_RUNTIME_SUSPEND_REGISTERS
        .iter()
        .zip(context.registers.into_array())
    {
        stored.store(value, Ordering::Release);
    }
    LINUX_RUNTIME_SUSPEND_ADDRESS.store(address, Ordering::Release);
    LINUX_RUNTIME_SUSPEND_COUNT.store(count, Ordering::Release);
    LINUX_RUNTIME_SUSPEND_RIP.store(context.rip, Ordering::Release);
    LINUX_RUNTIME_SUSPEND_RSP.store(context.rsp, Ordering::Release);
    LINUX_RUNTIME_SUSPEND_RFLAGS.store(context.rflags, Ordering::Release);
    LINUX_RUNTIME_SUSPEND_FS_BASE.store(context.fs_base, Ordering::Release);
    true
}

pub(crate) fn resume_suspended_linux_runtime(mmap: u64) -> LinuxElfResult {
    if !LINUX_RUNTIME_SUSPENDED.load(Ordering::Acquire) {
        return LinuxElfResult::FAILED;
    }
    let Some(slot) = active_container_slot() else {
        return LinuxElfResult::FAILED;
    };
    let stopping = CONTAINER_STOP_REQUESTED[slot].swap(false, Ordering::AcqRel);
    if !stopping && !CONTAINER_CONSOLE_ATTACHED[slot].load(Ordering::Acquire) {
        return LinuxElfResult::FAILED;
    }
    let address = LINUX_RUNTIME_SUSPEND_ADDRESS.load(Ordering::Acquire);
    let count = LINUX_RUNTIME_SUSPEND_COUNT.load(Ordering::Acquire);
    if address == 0 || count == 0 {
        return LinuxElfResult::FAILED;
    }
    if !stopping {
        let byte = crate::serial_read_byte_blocking();
        if !copy_to_linux_user(address, &[byte]) {
            return LinuxElfResult::FAILED;
        }
    }
    let registers = LinuxSyscallFrame::from_array(core::array::from_fn(|index| {
        LINUX_RUNTIME_SUSPEND_REGISTERS[index].load(Ordering::Acquire)
    }));
    let context = LinuxUserContext {
        registers,
        rip: LINUX_RUNTIME_SUSPEND_RIP.load(Ordering::Acquire),
        rsp: LINUX_RUNTIME_SUSPEND_RSP.load(Ordering::Acquire),
        rflags: LINUX_RUNTIME_SUSPEND_RFLAGS.load(Ordering::Acquire),
        rax: u64::from(!stopping),
        fs_base: LINUX_RUNTIME_SUSPEND_FS_BASE.load(Ordering::Acquire),
    };
    if !context.is_valid() {
        return LinuxElfResult::FAILED;
    }
    if !linux_mount::activate_initial() {
        return LinuxElfResult::FAILED;
    }
    if !linux_process::resume_runtime_process_registry() {
        let _ = linux_mount::deactivate_initial();
        return LinuxElfResult::FAILED;
    }
    let Some(saved_msrs) = configure_linux_syscall() else {
        let _ = linux_process::suspend_runtime_process_registry();
        let _ = linux_mount::deactivate_initial();
        return LinuxElfResult::FAILED;
    };
    if !restore_container_extended_state() {
        let _ = linux_process::finish_runtime_process_registry();
        let _ = linux_mount::deactivate_initial();
        let _ = restore_linux_syscall(saved_msrs);
        return LinuxElfResult::FAILED;
    }
    LINUX_RUNTIME_SUSPENDED.store(false, Ordering::Release);
    LINUX_RUNTIME_MODE.store(true, Ordering::Release);
    let resumed = resume_live_linux_process(
        context.rip as usize,
        context.rsp as usize,
        context.fs_base,
        context
            .registers
            .process_general(context.rax, context.rip, context.rflags),
        context.rflags,
    );
    if LINUX_RUNTIME_SUSPENDED.load(Ordering::Acquire) {
        let state_saved = save_container_extended_state();
        let registry_suspended = state_saved && linux_process::suspend_runtime_process_registry();
        let mount_deactivated = registry_suspended && linux_mount::deactivate_initial();
        let msrs_restored = restore_linux_syscall(saved_msrs);
        LINUX_RUNTIME_MODE.store(false, Ordering::Release);
        return LinuxElfResult {
            valid: resumed && mount_deactivated && msrs_restored,
            suspended: resumed && mount_deactivated && msrs_restored,
            ..LinuxElfResult::FAILED
        };
    }
    if !resumed {
        let _ = linux_process::finish_runtime_process_registry();
        let _ = linux_mount::deactivate_initial();
        let _ = restore_linux_syscall(saved_msrs);
        LINUX_RUNTIME_MODE.store(false, Ordering::Release);
        return LinuxElfResult::FAILED;
    }
    finish_linux_runtime(mmap, saved_msrs)
}

fn finish_linux_runtime(mmap: u64, saved_msrs: [u64; 5]) -> LinuxElfResult {
    let mut forks_completed = true;
    while LINUX_FORK_REQUESTED.load(Ordering::SeqCst) {
        if !execute_deferred_live_fork() {
            forks_completed = false;
            break;
        }
    }
    if forks_completed && LINUX_RUNTIME_SUSPENDED.load(Ordering::Acquire) {
        let state_saved = save_container_extended_state();
        let registry_suspended = state_saved && linux_process::suspend_runtime_process_registry();
        let mount_deactivated = registry_suspended && linux_mount::deactivate_initial();
        let msrs_restored = restore_linux_syscall(saved_msrs);
        LINUX_RUNTIME_MODE.store(false, Ordering::Release);
        return LinuxElfResult {
            valid: mount_deactivated && msrs_restored,
            suspended: mount_deactivated && msrs_restored,
            ..LinuxElfResult::FAILED
        };
    }
    let completed = ELF_PROBE_COMPLETED.swap(false, Ordering::SeqCst);
    let extended_state_discarded = discard_container_extended_state();
    let mmap_released = release_linux_runtime_mmaps(mmap);
    let descriptors_closed = linux_close_all_descriptors();
    let registry_finished = linux_process::finish_runtime_process_registry();
    let mount_deactivated = linux_mount::deactivate_initial();
    let msrs_restored = restore_linux_syscall(saved_msrs);
    let threads_released = finish_linux_thread_table();
    let inactive = !ELF_PROBE_ACTIVE.swap(false, Ordering::SeqCst);
    LINUX_RUNTIME_MODE.store(false, Ordering::SeqCst);
    let valid = forks_completed
        && extended_state_discarded
        && completed
        && mmap_released
        && descriptors_closed
        && registry_finished
        && mount_deactivated
        && msrs_restored
        && threads_released
        && inactive;
    if !valid {
        serial_write("ginux: OCI runtime cleanup failed\r\n");
    }
    LinuxElfResult {
        valid,
        ..LinuxElfResult::FAILED
    }
}

fn linux_user_stack_pointer_valid(user_stack: u64) -> bool {
    let Some(slot) = current_linux_thread_slot() else {
        return false;
    };
    let base = LINUX_THREAD_STACK_BASES[slot].load(Ordering::SeqCst);
    let size = LINUX_THREAD_STACK_SIZES[slot].load(Ordering::SeqCst);
    let primary = base != 0
        && size != 0
        && base
            .checked_add(size)
            .is_some_and(|end| user_stack >= base && user_stack <= end);
    if primary {
        return true;
    }
    if !LINUX_SIGNAL_ACTIVE.load(Ordering::SeqCst) {
        return false;
    }
    // SAFETY: active signal delivery owns this context until the serialized
    // rt_sigreturn or abort path clears active and the record.
    let signal = unsafe { core::ptr::addr_of!(LINUX_SIGNAL_CONTEXT).read() };
    let alt_base = signal.alt_stack_pointer;
    let alt_size = signal.alt_stack_size;
    let frame_uses_alt_stack = alt_base != 0
        && alt_size != 0
        && alt_base
            .checked_add(alt_size)
            .is_some_and(|end| signal.signal_stack >= alt_base && signal.signal_stack < end);
    frame_uses_alt_stack
        && alt_base
            .checked_add(alt_size)
            .is_some_and(|end| user_stack >= alt_base && user_stack <= end)
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

fn linux_elapsed_nanoseconds() -> Option<u64> {
    let (seconds, nanoseconds) = linux_elapsed_time()?;
    seconds
        .checked_mul(NANOSECONDS_PER_SECOND)
        .and_then(|value| value.checked_add(nanoseconds))
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

fn linux_nanosleep_live_return(
    output: *mut LinuxSyscallFrame,
    caller: LinuxUserContext,
    request: u64,
    remaining: u64,
) -> u64 {
    linux_clock_nanosleep_live_return(output, caller, LINUX_CLOCK_MONOTONIC, 0, request, remaining)
}

fn linux_clock_nanosleep_live_return(
    output: *mut LinuxSyscallFrame,
    caller: LinuxUserContext,
    clock: u64,
    flags: u64,
    request: u64,
    remaining: u64,
) -> u64 {
    if clock != LINUX_CLOCK_MONOTONIC || flags & !LINUX_TIMER_ABSTIME != 0 {
        return LINUX_EINVAL;
    }
    let mut request_bytes = [0_u8; 16];
    if !copy_from_linux_user(request, &mut request_bytes) {
        return LINUX_EFAULT;
    }
    let Some(requested) = linux_timespec_nanoseconds(&request_bytes) else {
        return LINUX_EINVAL;
    };
    let Some(now) = linux_elapsed_nanoseconds() else {
        return LINUX_ENOSYS;
    };
    let absolute = flags & LINUX_TIMER_ABSTIME != 0;
    let deadline = if absolute {
        requested
    } else {
        let Some(deadline) = now.checked_add(requested) else {
            return LINUX_EINVAL;
        };
        deadline
    };
    if deadline <= now {
        return 0;
    }
    let remaining = if absolute { 0 } else { remaining };
    if remaining != 0 && !linux_writable_user_range_contains(remaining, 16) {
        return LINUX_EFAULT;
    }
    let Some(current) = current_linux_thread_handle() else {
        return LINUX_ESRCH;
    };
    let Some(target) = ready_linux_thread_handle() else {
        return LINUX_ENOSYS;
    };
    let Some(tls) = linux_thread_context(current) else {
        return LINUX_ESRCH;
    };
    if !save_linux_thread_return_context(current, caller)
        || !block_current_linux_thread(LINUX_THREAD_WAIT_SLEEP, 1, remaining, deadline)
    {
        return LINUX_EINVAL;
    }
    let mut target_registers = LinuxSyscallFrame::ZERO;
    let mut target_rip = 0;
    let mut target_rsp = 0;
    let mut target_rflags = 0;
    let mut target_rax = 0;
    if !install_linux_thread_return(
        target,
        &mut target_registers,
        &mut target_rip,
        &mut target_rsp,
        &mut target_rflags,
        &mut target_rax,
    ) {
        let slot = usize::from(current.slot);
        LINUX_THREAD_WAIT_KINDS[slot].store(LINUX_THREAD_WAIT_NONE, Ordering::SeqCst);
        LINUX_THREAD_WAIT_KEYS[slot].store(0, Ordering::SeqCst);
        LINUX_THREAD_WAIT_OUTPUTS[slot].store(0, Ordering::SeqCst);
        LINUX_THREAD_WAIT_DEADLINES[slot].store(0, Ordering::SeqCst);
        LINUX_THREAD_STATES[slot].store(LINUX_THREAD_RUNNING, Ordering::SeqCst);
        LINUX_CURRENT_THREAD_SLOT.store(current.slot, Ordering::SeqCst);
        let _ = install_linux_tls_context(tls);
        return LINUX_EINVAL;
    }
    let selected = LinuxUserContext {
        registers: target_registers,
        rip: target_rip,
        rsp: target_rsp,
        rflags: target_rflags,
        rax: target_rax,
        fs_base: active_fs_base(),
    };
    if !write_linux_live_return(output, selected) {
        return LINUX_EINVAL;
    }
    LINUX_LIVE_RETURN_VALIDATED.store(true, Ordering::SeqCst);
    LINUX_SLEEP_BLOCK_COUNT.fetch_add(1, Ordering::SeqCst);
    selected.rax
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

fn linux_futex_live_return(
    output: *mut LinuxSyscallFrame,
    caller: LinuxUserContext,
    address: u64,
    operation: u64,
    value: u64,
    timeout: u64,
) -> u64 {
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
                return LINUX_EAGAIN;
            }
            let Some(current) = current_linux_thread_handle() else {
                return LINUX_ESRCH;
            };
            let Some(target) = ready_linux_thread_handle() else {
                return LINUX_ENOSYS;
            };
            let Some(tls) = linux_thread_context(current) else {
                return LINUX_ESRCH;
            };
            let deadline = if timeout == 0 {
                0
            } else {
                let mut timeout_bytes = [0_u8; 16];
                if !copy_from_linux_user(timeout, &mut timeout_bytes) {
                    return LINUX_EFAULT;
                }
                let Some(duration) = linux_timespec_nanoseconds(&timeout_bytes) else {
                    return LINUX_EINVAL;
                };
                if duration == 0 {
                    return LINUX_ETIMEDOUT;
                }
                let Some(deadline) =
                    linux_elapsed_nanoseconds().and_then(|now| now.checked_add(duration))
                else {
                    return LINUX_EINVAL;
                };
                deadline
            };
            if !save_linux_thread_return_context(current, caller)
                || !block_current_linux_thread(LINUX_THREAD_WAIT_FUTEX, address, 0, deadline)
            {
                return LINUX_EINVAL;
            }
            let mut target_registers = LinuxSyscallFrame::ZERO;
            let mut target_rip = 0;
            let mut target_rsp = 0;
            let mut target_rflags = 0;
            let mut target_rax = 0;
            if !install_linux_thread_return(
                target,
                &mut target_registers,
                &mut target_rip,
                &mut target_rsp,
                &mut target_rflags,
                &mut target_rax,
            ) {
                let slot = usize::from(current.slot);
                LINUX_THREAD_WAIT_KINDS[slot].store(LINUX_THREAD_WAIT_NONE, Ordering::SeqCst);
                LINUX_THREAD_WAIT_KEYS[slot].store(0, Ordering::SeqCst);
                LINUX_THREAD_WAIT_OUTPUTS[slot].store(0, Ordering::SeqCst);
                LINUX_THREAD_WAIT_DEADLINES[slot].store(0, Ordering::SeqCst);
                LINUX_THREAD_STATES[slot].store(LINUX_THREAD_RUNNING, Ordering::SeqCst);
                LINUX_CURRENT_THREAD_SLOT.store(current.slot, Ordering::SeqCst);
                let _ = install_linux_tls_context(tls);
                return LINUX_EINVAL;
            }
            let selected = LinuxUserContext {
                registers: target_registers,
                rip: target_rip,
                rsp: target_rsp,
                rflags: target_rflags,
                rax: target_rax,
                fs_base: active_fs_base(),
            };
            if !write_linux_live_return(output, selected) {
                return LINUX_EINVAL;
            }
            LINUX_LIVE_RETURN_VALIDATED.store(true, Ordering::SeqCst);
            LINUX_FUTEX_WAIT_BLOCK_COUNT.fetch_add(1, Ordering::SeqCst);
            selected.rax
        }
        1 | 129 => {
            let woken = wake_linux_futex(address, u32::try_from(value).unwrap_or(u32::MAX));
            if woken != 0 {
                LINUX_FUTEX_EXPLICIT_WAKE_COUNT.fetch_add(1, Ordering::SeqCst);
            }
            u64::from(woken)
        }
        _ => LINUX_EINVAL,
    }
}

fn linux_mmap_slot_frame(slot: usize) -> u64 {
    ELF_PROBE_MMAP_FRAMES
        .get(slot)
        .map_or(0, |frame| frame.load(Ordering::SeqCst))
}

fn linux_mmap_slot_lease(slot: usize) -> Option<memory::FrameLease> {
    memory::FrameLease::from_parts(
        linux_mmap_slot_frame(slot),
        ELF_PROBE_MMAP_GENERATIONS.get(slot)?.load(Ordering::SeqCst),
    )
}

fn linux_mmap_store_slot_lease(slot: usize, lease: memory::FrameLease) {
    let Some(frame) = ELF_PROBE_MMAP_FRAMES.get(slot) else {
        return;
    };
    frame.store(lease.address(), Ordering::SeqCst);
    ELF_PROBE_MMAP_GENERATIONS[slot].store(lease.generation(), Ordering::SeqCst);
}

fn linux_mmap_clear_slot_lease(slot: usize) {
    let Some(frame) = ELF_PROBE_MMAP_FRAMES.get(slot) else {
        return;
    };
    frame.store(0, Ordering::SeqCst);
    ELF_PROBE_MMAP_GENERATIONS[slot].store(0, Ordering::SeqCst);
}

fn linux_allocate_mmap_leases(
    pages: usize,
) -> Option<[memory::FrameLease; LINUX_RUNTIME_MMAP_SLOT_COUNT]> {
    if !(1..=LINUX_RUNTIME_MMAP_SLOT_COUNT).contains(&pages) {
        return None;
    }
    let mut leases = [memory::FrameLease::EMPTY; LINUX_RUNTIME_MMAP_SLOT_COUNT];
    for index in 0..pages {
        let Some([lease]) = memory::allocate_frames::<1>(memory::FrameOwner::KERNEL) else {
            if index != 0 {
                let _ =
                    memory::scrub_and_release_frames(memory::FrameOwner::KERNEL, &leases[..index]);
            }
            return None;
        };
        leases[index] = lease;
    }
    Some(leases)
}

fn linux_mmap_unmap_and_release_slots(start_slot: usize, pages: usize) -> bool {
    let base = ELF_PROBE_MMAP.load(Ordering::SeqCst);
    let mut leases = [None; LINUX_RUNTIME_MMAP_SLOT_COUNT];
    for slot in start_slot..start_slot + pages {
        let Some(area) = linux_mmap_slot_area_index(slot) else {
            continue;
        };
        let address = base + (slot * PAGE_SIZE) as u64;
        let protection = u64::from(ELF_PROBE_MMAP_PROTECTION[area].load(Ordering::SeqCst));
        if protection == 0 {
            if linux_mmap_slot_frame(slot) != 0
                || ELF_PROBE_MMAP_GENERATIONS[slot].load(Ordering::SeqCst) != 0
                || !address_space::current_linux_page_absent(address)
            {
                return false;
            }
        } else {
            let Some(lease) = linux_mmap_slot_lease(slot) else {
                return false;
            };
            if !memory::frame_leases_releasable(memory::FrameOwner::KERNEL, &[lease])
                || !address_space::current_linux_mapping_matches(
                    address,
                    lease,
                    protection & LINUX_PROT_WRITE != 0,
                    false,
                )
            {
                return false;
            }
            leases[slot - start_slot] = Some(lease);
        }
    }
    for (offset, lease) in leases[..pages].iter().copied().enumerate() {
        let Some(lease) = lease else {
            continue;
        };
        let slot = start_slot + offset;
        let address = base + (slot * PAGE_SIZE) as u64;
        if !address_space::unmap_current_linux_page(address, lease.address())
            || !address_space::prepare_linux_frame(lease.address(), &[])
            || !memory::release_frames(memory::FrameOwner::KERNEL, [lease])
        {
            return false;
        }
        linux_mmap_clear_slot_lease(slot);
    }
    true
}

fn release_linux_runtime_mmaps(expected_base: u64) -> bool {
    let address_valid = ELF_PROBE_MMAP.load(Ordering::SeqCst) == expected_base;
    let mut areas_released = true;
    for (start_slot, pages) in ELF_PROBE_MMAP_PAGES.iter().enumerate() {
        let pages = usize::from(pages.load(Ordering::SeqCst));
        if pages == 0 {
            continue;
        }
        areas_released &= pages <= LINUX_RUNTIME_MMAP_SLOT_COUNT - start_slot
            && linux_mmap_unmap_and_release_slots(start_slot, pages);
    }

    let mut leases_released = true;
    for (slot, pages) in ELF_PROBE_MMAP_PAGES.iter().enumerate() {
        pages.store(0, Ordering::SeqCst);
        ELF_PROBE_MMAP_PROTECTION[slot].store(0, Ordering::SeqCst);
        ELF_PROBE_MMAP_FILE[slot].store(0, Ordering::SeqCst);
        leases_released &= ELF_PROBE_MMAP_FRAMES[slot].load(Ordering::SeqCst) == 0
            && ELF_PROBE_MMAP_GENERATIONS[slot].load(Ordering::SeqCst) == 0;
    }
    let address_cleared = ELF_PROBE_MMAP.swap(0, Ordering::SeqCst) == expected_base;
    address_valid && address_cleared && areas_released && leases_released
}

fn linux_mmap_slot_occupied(slot: usize) -> bool {
    linux_mmap_slot_area_index(slot).is_some()
}

fn linux_mmap_slot_area_index(slot: usize) -> Option<usize> {
    if slot >= LINUX_RUNTIME_MMAP_SLOT_COUNT {
        return None;
    }
    (0..LINUX_RUNTIME_MMAP_SLOT_COUNT).find(|start| {
        let pages = usize::from(ELF_PROBE_MMAP_PAGES[*start].load(Ordering::SeqCst));
        pages != 0
            && pages <= LINUX_RUNTIME_MMAP_SLOT_COUNT - *start
            && (*start..*start + pages).contains(&slot)
    })
}

fn linux_mmap_slot_attributes() -> [Option<(u8, u8)>; LINUX_RUNTIME_MMAP_SLOT_COUNT] {
    core::array::from_fn(|slot| {
        let area = linux_mmap_slot_area_index(slot)?;
        Some((
            ELF_PROBE_MMAP_PROTECTION[area].load(Ordering::SeqCst),
            ELF_PROBE_MMAP_FILE[area].load(Ordering::SeqCst),
        ))
    })
}

#[derive(Clone, Copy, Eq, PartialEq)]
struct LinuxMmapRuntimeState {
    base: u64,
    pages: [u8; LINUX_RUNTIME_MMAP_SLOT_COUNT],
    protection: [u8; LINUX_RUNTIME_MMAP_SLOT_COUNT],
    file: [u8; LINUX_RUNTIME_MMAP_SLOT_COUNT],
    frames: [u64; LINUX_RUNTIME_MMAP_SLOT_COUNT],
    generations: [u32; LINUX_RUNTIME_MMAP_SLOT_COUNT],
}

fn capture_linux_mmap_runtime_state() -> LinuxMmapRuntimeState {
    LinuxMmapRuntimeState {
        base: ELF_PROBE_MMAP.load(Ordering::SeqCst),
        pages: core::array::from_fn(|index| ELF_PROBE_MMAP_PAGES[index].load(Ordering::SeqCst)),
        protection: core::array::from_fn(|index| {
            ELF_PROBE_MMAP_PROTECTION[index].load(Ordering::SeqCst)
        }),
        file: core::array::from_fn(|index| ELF_PROBE_MMAP_FILE[index].load(Ordering::SeqCst)),
        frames: core::array::from_fn(|index| ELF_PROBE_MMAP_FRAMES[index].load(Ordering::SeqCst)),
        generations: core::array::from_fn(|index| {
            ELF_PROBE_MMAP_GENERATIONS[index].load(Ordering::SeqCst)
        }),
    }
}

fn restore_linux_mmap_runtime_state(state: LinuxMmapRuntimeState) {
    ELF_PROBE_MMAP.store(state.base, Ordering::SeqCst);
    for index in 0..LINUX_RUNTIME_MMAP_SLOT_COUNT {
        ELF_PROBE_MMAP_PAGES[index].store(state.pages[index], Ordering::SeqCst);
        ELF_PROBE_MMAP_PROTECTION[index].store(state.protection[index], Ordering::SeqCst);
        ELF_PROBE_MMAP_FILE[index].store(state.file[index], Ordering::SeqCst);
        ELF_PROBE_MMAP_FRAMES[index].store(state.frames[index], Ordering::SeqCst);
        ELF_PROBE_MMAP_GENERATIONS[index].store(state.generations[index], Ordering::SeqCst);
    }
}

pub(crate) fn verify_linux_mmap_process_restore() -> bool {
    let original = capture_linux_mmap_runtime_state();
    let mut parent = original;
    parent.base = 0x7f00_1000;
    parent.pages[0] = 2;
    parent.protection[0] = (LINUX_PROT_READ | LINUX_PROT_WRITE) as u8;
    parent.frames[0] = 0x20_0000;
    parent.generations[0] = 7;
    parent.frames[1] = 0x21_0000;
    parent.generations[1] = 8;
    restore_linux_mmap_runtime_state(parent);

    let mut child = parent;
    child.pages[0] = 0;
    child.protection[0] = 0;
    child.frames[0] = 0x30_0000;
    child.generations[0] = 11;
    restore_linux_mmap_runtime_state(child);
    restore_linux_mmap_runtime_state(parent);
    let restored = capture_linux_mmap_runtime_state() == parent;
    restore_linux_mmap_runtime_state(original);
    restored && capture_linux_mmap_runtime_state() == original
}

fn linux_mmap_rebuild_areas(slots: [Option<(u8, u8)>; LINUX_RUNTIME_MMAP_SLOT_COUNT]) {
    for index in 0..LINUX_RUNTIME_MMAP_SLOT_COUNT {
        ELF_PROBE_MMAP_PAGES[index].store(0, Ordering::SeqCst);
        ELF_PROBE_MMAP_PROTECTION[index].store(0, Ordering::SeqCst);
        ELF_PROBE_MMAP_FILE[index].store(0, Ordering::SeqCst);
    }
    let mut start = 0;
    while start < LINUX_RUNTIME_MMAP_SLOT_COUNT {
        let Some((protection, file)) = slots[start] else {
            start += 1;
            continue;
        };
        let mut end = start + 1;
        while end < LINUX_RUNTIME_MMAP_SLOT_COUNT && slots[end] == Some((protection, file)) {
            end += 1;
        }
        ELF_PROBE_MMAP_PAGES[start].store((end - start) as u8, Ordering::SeqCst);
        ELF_PROBE_MMAP_PROTECTION[start].store(protection, Ordering::SeqCst);
        ELF_PROBE_MMAP_FILE[start].store(file, Ordering::SeqCst);
        start = end;
    }
}

fn linux_mmap_range_slots(address: u64, length: u64) -> Option<(usize, usize)> {
    let base = ELF_PROBE_MMAP.load(Ordering::SeqCst);
    if !address.is_multiple_of(PAGE_SIZE as u64) || address < base || length == 0 {
        return None;
    }
    let start = usize::try_from((address - base) / PAGE_SIZE as u64).ok()?;
    let pages = usize::try_from(length.div_ceil(PAGE_SIZE as u64)).ok()?;
    let limit = linux_mmap_slot_limit();
    (start < limit && start.checked_add(pages).is_some_and(|end| end <= limit))
        .then_some((start, pages))
}

fn linux_mmap_slot_limit() -> usize {
    if LINUX_RUNTIME_MODE.load(Ordering::SeqCst) {
        LINUX_RUNTIME_MMAP_SLOT_COUNT
    } else {
        LINUX_MMAP_SLOT_COUNT
    }
}

fn linux_mprotect_mmap(address: u64, length: u64, protection: u64) -> u64 {
    if protection != 0
        && protection != LINUX_PROT_READ
        && protection != (LINUX_PROT_READ | LINUX_PROT_WRITE)
    {
        return LINUX_EINVAL;
    }
    let Some((start_slot, pages)) = linux_mmap_range_slots(address, length) else {
        return LINUX_EINVAL;
    };
    let target = protection as u8;
    let mut slots = linux_mmap_slot_attributes();
    if slots[start_slot..start_slot + pages]
        .iter()
        .any(|slot| slot.is_none())
    {
        return LINUX_EINVAL;
    }
    if slots[start_slot..start_slot + pages]
        .iter()
        .all(|slot| slot.is_some_and(|(current, _)| current == target))
    {
        for offset in 0..pages {
            let slot = start_slot + offset;
            let Some((current, _)) = slots[slot] else {
                return LINUX_EINVAL;
            };
            let page = address + (offset * PAGE_SIZE) as u64;
            if current == 0 {
                if linux_mmap_slot_frame(slot) != 0
                    || ELF_PROBE_MMAP_GENERATIONS[slot].load(Ordering::SeqCst) != 0
                    || !address_space::current_linux_page_absent(page)
                {
                    return LINUX_EINVAL;
                }
            } else {
                let Some(lease) = linux_mmap_slot_lease(slot) else {
                    return LINUX_EINVAL;
                };
                if !address_space::current_linux_mapping_matches(
                    page,
                    lease,
                    u64::from(current) & LINUX_PROT_WRITE != 0,
                    false,
                ) {
                    return LINUX_EINVAL;
                }
            }
        }
        return 0;
    }
    let all_reserved = slots[start_slot..start_slot + pages]
        .iter()
        .all(|slot| slot == &Some((0, 0)));
    if all_reserved {
        for offset in 0..pages {
            let slot = start_slot + offset;
            if linux_mmap_slot_frame(slot) != 0
                || ELF_PROBE_MMAP_GENERATIONS[slot].load(Ordering::SeqCst) != 0
                || !address_space::current_linux_page_absent(address + (offset * PAGE_SIZE) as u64)
            {
                return LINUX_EINVAL;
            }
        }
        let Some(leases) = linux_allocate_mmap_leases(pages) else {
            return LINUX_ENOMEM;
        };
        for lease in &leases[..pages] {
            if !address_space::prepare_linux_frame(lease.address(), &[]) {
                let _ =
                    memory::scrub_and_release_frames(memory::FrameOwner::KERNEL, &leases[..pages]);
                return LINUX_ENOMEM;
            }
        }
        if !address_space::map_current_linux_pages(
            address,
            &leases[..pages],
            protection & LINUX_PROT_WRITE != 0,
        ) {
            let _ = memory::scrub_and_release_frames(memory::FrameOwner::KERNEL, &leases[..pages]);
            return LINUX_ENOMEM;
        }
        for (offset, lease) in leases[..pages].iter().copied().enumerate() {
            linux_mmap_store_slot_lease(start_slot + offset, lease);
            slots[start_slot + offset] = Some((target, 0));
        }
        linux_mmap_rebuild_areas(slots);
        return 0;
    }
    if protection == LINUX_PROT_READ
        && slots[start_slot..start_slot + pages]
            .iter()
            .all(|slot| slot == &Some(((LINUX_PROT_READ | LINUX_PROT_WRITE) as u8, 0)))
    {
        let mut frames = [0_u64; LINUX_RUNTIME_MMAP_SLOT_COUNT];
        for (offset, frame) in frames.iter_mut().enumerate().take(pages) {
            *frame = linux_mmap_slot_frame(start_slot + offset);
        }
        if address_space::protect_current_linux_pages_read_only(address, &frames[..pages]) {
            for slot in &mut slots[start_slot..start_slot + pages] {
                *slot = Some((LINUX_PROT_READ as u8, 0));
            }
            linux_mmap_rebuild_areas(slots);
            return 0;
        }
    }
    LINUX_EINVAL
}

fn linux_mmap(address: u64, length: u64, protection: u64, flags: u64, fd: u64, offset: u64) -> u64 {
    if length == 0 {
        return LINUX_EINVAL;
    }
    let Ok(pages) = u8::try_from(length.div_ceil(PAGE_SIZE as u64)) else {
        return LINUX_EINVAL;
    };
    let slot_limit = linux_mmap_slot_limit();
    if usize::from(pages) > slot_limit {
        return LINUX_EINVAL;
    }
    let anonymous_flags = LINUX_MAP_PRIVATE | LINUX_MAP_ANONYMOUS;
    let fixed_anonymous_flags = anonymous_flags | LINUX_MAP_FIXED_NOREPLACE;
    let replace_anonymous_flags = anonymous_flags | LINUX_MAP_FIXED;
    let fixed_anonymous = flags == fixed_anonymous_flags;
    let replace_anonymous = flags == replace_anonymous_flags;
    if address != 0 && !fixed_anonymous && !replace_anonymous {
        return linux_mmap_fixed_file(address, length, protection, flags, fd, offset);
    }
    if address == 0 && (fixed_anonymous || replace_anonymous) {
        return LINUX_EINVAL;
    }
    let start_slot = if fixed_anonymous || replace_anonymous {
        let Some((start, range_pages)) = linux_mmap_range_slots(address, length) else {
            return LINUX_EINVAL;
        };
        if range_pages != usize::from(pages) {
            return LINUX_EINVAL;
        }
        start
    } else {
        let Some(start) = (0..=slot_limit - usize::from(pages)).find(|start| {
            (*start..*start + usize::from(pages)).all(|slot| !linux_mmap_slot_occupied(slot))
        }) else {
            return LINUX_ENOMEM;
        };
        start
    };
    if !replace_anonymous
        && (start_slot..start_slot + usize::from(pages)).any(linux_mmap_slot_occupied)
    {
        return if fixed_anonymous {
            LINUX_EEXIST
        } else {
            LINUX_ENOMEM
        };
    }
    let mapping = ELF_PROBE_MMAP.load(Ordering::SeqCst) + (start_slot * PAGE_SIZE) as u64;
    if mapping == 0
        || (fixed_anonymous || replace_anonymous) && mapping != address
        || !replace_anonymous
            && (start_slot..start_slot + usize::from(pages)).any(|slot| {
                linux_mmap_slot_frame(slot) != 0
                    || ELF_PROBE_MMAP_GENERATIONS[slot].load(Ordering::SeqCst) != 0
            })
    {
        return LINUX_ENOMEM;
    }

    let (file_kind, file_source) =
        if flags == anonymous_flags || fixed_anonymous || replace_anonymous {
            if protection != 0
                && protection != LINUX_PROT_READ
                && protection != (LINUX_PROT_READ | LINUX_PROT_WRITE)
                || fd != u64::MAX
                || offset != 0
            {
                return LINUX_EINVAL;
            }
            (0, None)
        } else {
            if protection != LINUX_PROT_READ || flags != LINUX_MAP_PRIVATE {
                return LINUX_EINVAL;
            }
            let state = linux_device_descriptor(fd);
            let Some(file) = linux_regular_file(state) else {
                return if linux_descriptor_open(fd) {
                    LINUX_ENODEV
                } else {
                    LINUX_EBADF
                };
            };
            if linux_proc_file(state) {
                return LINUX_ENODEV;
            }
            if state & 3 != LINUX_DESCRIPTOR_INPUT || !offset.is_multiple_of(PAGE_SIZE as u64) {
                return LINUX_EINVAL;
            }
            let Ok(start) = usize::try_from(offset) else {
                return LINUX_EINVAL;
            };
            let mapped_length = usize::from(pages) * PAGE_SIZE;
            if start >= file.len()
                || start
                    .checked_add(mapped_length)
                    .is_none_or(|end| end > file.len().next_multiple_of(PAGE_SIZE))
            {
                return LINUX_EINVAL;
            }
            (state & LINUX_FILE_MASK, Some((file, start)))
        };

    if protection == 0 {
        if replace_anonymous {
            let mut slots = linux_mmap_slot_attributes();
            let mut old_leases = [None; LINUX_RUNTIME_MMAP_SLOT_COUNT];
            let mut old_writable = [false; LINUX_RUNTIME_MMAP_SLOT_COUNT];
            let mut released = [memory::FrameLease::EMPTY; LINUX_RUNTIME_MMAP_SLOT_COUNT];
            let mut released_count = 0;
            for offset in 0..usize::from(pages) {
                let slot = start_slot + offset;
                if let Some((old_protection, _)) = slots[slot] {
                    if old_protection != 0 {
                        let Some(lease) = linux_mmap_slot_lease(slot) else {
                            return LINUX_ENOMEM;
                        };
                        old_leases[offset] = Some(lease);
                        old_writable[offset] = u64::from(old_protection) & LINUX_PROT_WRITE != 0;
                        released[released_count] = lease;
                        released_count += 1;
                    } else if linux_mmap_slot_frame(slot) != 0
                        || ELF_PROBE_MMAP_GENERATIONS[slot].load(Ordering::SeqCst) != 0
                    {
                        return LINUX_ENOMEM;
                    }
                } else if linux_mmap_slot_frame(slot) != 0
                    || ELF_PROBE_MMAP_GENERATIONS[slot].load(Ordering::SeqCst) != 0
                {
                    return LINUX_ENOMEM;
                }
            }
            if released_count != 0
                && !memory::frame_leases_releasable(
                    memory::FrameOwner::KERNEL,
                    &released[..released_count],
                )
                || !address_space::reserve_current_linux_pages(
                    mapping,
                    &old_leases[..usize::from(pages)],
                    &old_writable[..usize::from(pages)],
                )
            {
                return LINUX_ENOMEM;
            }
            for offset in 0..usize::from(pages) {
                let slot = start_slot + offset;
                linux_mmap_clear_slot_lease(slot);
                slots[slot] = Some((0, 0));
            }
            linux_mmap_rebuild_areas(slots);
            if released_count != 0
                && !memory::scrub_and_release_frames(
                    memory::FrameOwner::KERNEL,
                    &released[..released_count],
                )
            {
                return LINUX_ENOMEM;
            }
            return mapping;
        }
        ELF_PROBE_MMAP_PAGES[start_slot].store(pages, Ordering::SeqCst);
        ELF_PROBE_MMAP_PROTECTION[start_slot].store(0, Ordering::SeqCst);
        ELF_PROBE_MMAP_FILE[start_slot].store(0, Ordering::SeqCst);
        return mapping;
    }

    let Some(leases) = linux_allocate_mmap_leases(usize::from(pages)) else {
        return LINUX_ENOMEM;
    };
    for offset in 0..usize::from(pages) {
        let bytes = file_source.map_or(&[][..], |(file, start)| {
            let page_start = start + offset * PAGE_SIZE;
            &file[page_start..page_start.saturating_add(PAGE_SIZE).min(file.len())]
        });
        if !address_space::prepare_linux_frame(leases[offset].address(), bytes) {
            let _ = memory::scrub_and_release_frames(
                memory::FrameOwner::KERNEL,
                &leases[..usize::from(pages)],
            );
            return LINUX_ENOMEM;
        }
    }
    if replace_anonymous {
        let mut slots = linux_mmap_slot_attributes();
        let mut old_leases = [None; LINUX_RUNTIME_MMAP_SLOT_COUNT];
        let mut old_writable = [false; LINUX_RUNTIME_MMAP_SLOT_COUNT];
        let mut released = [memory::FrameLease::EMPTY; LINUX_RUNTIME_MMAP_SLOT_COUNT];
        let mut released_count = 0;
        for offset in 0..usize::from(pages) {
            let slot = start_slot + offset;
            match slots[slot] {
                Some((old_protection, _)) => {
                    if old_protection == 0 {
                        if linux_mmap_slot_frame(slot) != 0
                            || ELF_PROBE_MMAP_GENERATIONS[slot].load(Ordering::SeqCst) != 0
                            || !address_space::current_linux_page_absent(
                                mapping + (offset * PAGE_SIZE) as u64,
                            )
                        {
                            let _ = memory::scrub_and_release_frames(
                                memory::FrameOwner::KERNEL,
                                &leases[..usize::from(pages)],
                            );
                            return LINUX_ENOMEM;
                        }
                    } else {
                        let Some(lease) = linux_mmap_slot_lease(slot) else {
                            let _ = memory::scrub_and_release_frames(
                                memory::FrameOwner::KERNEL,
                                &leases[..usize::from(pages)],
                            );
                            return LINUX_ENOMEM;
                        };
                        old_leases[offset] = Some(lease);
                        old_writable[offset] = u64::from(old_protection) & LINUX_PROT_WRITE != 0;
                        released[released_count] = lease;
                        released_count += 1;
                    }
                }
                None => {
                    if linux_mmap_slot_frame(slot) != 0
                        || ELF_PROBE_MMAP_GENERATIONS[slot].load(Ordering::SeqCst) != 0
                    {
                        let _ = memory::scrub_and_release_frames(
                            memory::FrameOwner::KERNEL,
                            &leases[..usize::from(pages)],
                        );
                        return LINUX_ENOMEM;
                    }
                }
            }
        }
        if released_count != 0
            && !memory::frame_leases_releasable(
                memory::FrameOwner::KERNEL,
                &released[..released_count],
            )
            || !address_space::replace_current_linux_pages(
                mapping,
                &old_leases[..usize::from(pages)],
                &old_writable[..usize::from(pages)],
                &leases[..usize::from(pages)],
                protection & LINUX_PROT_WRITE != 0,
            )
        {
            let _ = memory::scrub_and_release_frames(
                memory::FrameOwner::KERNEL,
                &leases[..usize::from(pages)],
            );
            return LINUX_ENOMEM;
        }
        for (offset, lease) in leases[..usize::from(pages)].iter().copied().enumerate() {
            let slot = start_slot + offset;
            linux_mmap_store_slot_lease(slot, lease);
            slots[slot] = Some((protection as u8, 0));
        }
        linux_mmap_rebuild_areas(slots);
        if released_count != 0
            && !memory::scrub_and_release_frames(
                memory::FrameOwner::KERNEL,
                &released[..released_count],
            )
        {
            return LINUX_ENOMEM;
        }
        return mapping;
    }
    let writable = protection & LINUX_PROT_WRITE != 0;
    let map_page = |address, frame| {
        if writable {
            address_space::map_current_linux_page(address, frame)
        } else {
            address_space::map_current_linux_read_only_page(address, frame)
        }
    };
    let mut mapped = 0;
    while mapped < usize::from(pages) {
        if !map_page(
            mapping + (mapped * PAGE_SIZE) as u64,
            leases[mapped].address(),
        ) {
            for (rollback, lease) in leases.iter().enumerate().take(mapped) {
                let _ = address_space::unmap_current_linux_page(
                    mapping + (rollback * PAGE_SIZE) as u64,
                    lease.address(),
                );
            }
            let _ = memory::scrub_and_release_frames(
                memory::FrameOwner::KERNEL,
                &leases[..usize::from(pages)],
            );
            return LINUX_ENOMEM;
        }
        mapped += 1;
    }
    for (offset, lease) in leases[..usize::from(pages)].iter().copied().enumerate() {
        linux_mmap_store_slot_lease(start_slot + offset, lease);
    }
    ELF_PROBE_MMAP_PAGES[start_slot].store(pages, Ordering::SeqCst);
    ELF_PROBE_MMAP_PROTECTION[start_slot].store(protection as u8, Ordering::SeqCst);
    ELF_PROBE_MMAP_FILE[start_slot].store(file_kind as u8, Ordering::SeqCst);
    mapping
}

fn linux_mmap_fixed_file(
    address: u64,
    length: u64,
    protection: u64,
    flags: u64,
    fd: u64,
    offset: u64,
) -> u64 {
    if length != PAGE_SIZE as u64 || flags != (LINUX_MAP_PRIVATE | LINUX_MAP_FIXED_NOREPLACE) {
        return LINUX_EINVAL;
    }
    let state = linux_device_descriptor(fd);
    let Some(file) = linux_regular_file(state) else {
        return if linux_descriptor_open(fd) {
            LINUX_ENODEV
        } else {
            LINUX_EBADF
        };
    };
    if linux_proc_file(state) {
        return LINUX_ENODEV;
    }
    if state & 3 != LINUX_DESCRIPTOR_INPUT {
        return LINUX_EACCES;
    }

    let file_kind = state & LINUX_FILE_MASK;
    let dependency = match file_kind {
        LINUX_FILE_DEPENDENCY => Some(0),
        LINUX_FILE_AUXILIARY_DEPENDENCY => Some(1),
        _ => None,
    };
    let (
        bit,
        expected_offset,
        file_size,
        expected_protection,
        mapping_state,
        relative_address,
        selectable_base,
        mapping_index,
    ) = if file_kind == LINUX_FILE_INIT {
        let Some(index) = ELF_PROBE_MAIN_ADDRESSES
            .iter()
            .position(|candidate| candidate.load(Ordering::SeqCst) == address)
        else {
            return LINUX_EINVAL;
        };
        (
            1_u8 << index,
            ELF_PROBE_MAIN_OFFSETS[index].load(Ordering::SeqCst),
            ELF_PROBE_MAIN_FILE_SIZES[index].load(Ordering::SeqCst),
            u64::from(ELF_PROBE_MAIN_PROTECTIONS[index].load(Ordering::SeqCst)),
            &ELF_PROBE_MAIN_MAPPINGS,
            0,
            false,
            index,
        )
    } else if let Some(dependency) = dependency {
        let state_start = dependency * LINUX_DEPENDENCY_MAPPING_COUNT;
        let Some(index) = (0..LINUX_DEPENDENCY_MAPPING_COUNT).position(|index| {
            let state = state_start + index;
            ELF_PROBE_DEPENDENCY_FILE_SIZES[state].load(Ordering::SeqCst) != 0
                && ELF_PROBE_DEPENDENCY_OFFSETS[state].load(Ordering::SeqCst) == offset
                && u64::from(ELF_PROBE_DEPENDENCY_PROTECTIONS[state].load(Ordering::SeqCst))
                    == protection
        }) else {
            return LINUX_EINVAL;
        };
        let state = state_start + index;
        (
            1_u8 << index,
            ELF_PROBE_DEPENDENCY_OFFSETS[state].load(Ordering::SeqCst),
            ELF_PROBE_DEPENDENCY_FILE_SIZES[state].load(Ordering::SeqCst),
            u64::from(ELF_PROBE_DEPENDENCY_PROTECTIONS[state].load(Ordering::SeqCst)),
            &ELF_PROBE_DEPENDENCY_MAPPINGS[dependency],
            ELF_PROBE_DEPENDENCY_RELATIVES[state].load(Ordering::SeqCst),
            true,
            index,
        )
    } else {
        return LINUX_EACCES;
    };
    if address == 0
        || file_size == 0
        || file_size > PAGE_SIZE as u64
        || offset != expected_offset
        || protection != expected_protection
        || protection & (LINUX_PROT_WRITE | LINUX_PROT_EXEC) == (LINUX_PROT_WRITE | LINUX_PROT_EXEC)
    {
        return LINUX_EINVAL;
    }
    if selectable_base {
        let Some(candidate_base) = address.checked_sub(relative_address) else {
            return LINUX_EINVAL;
        };
        let dependency = dependency.unwrap_or(0);
        let base_min = ELF_PROBE_DEPENDENCY_BASE_MIN[dependency].load(Ordering::SeqCst);
        let base_end = ELF_PROBE_DEPENDENCY_BASE_END[dependency].load(Ordering::SeqCst);
        if candidate_base < base_min
            || candidate_base >= base_end
            || !candidate_base.is_multiple_of(PAGE_SIZE as u64)
        {
            return LINUX_EINVAL;
        }
        let selected_base = match ELF_PROBE_DEPENDENCY_SELECTED_BASE[dependency].compare_exchange(
            0,
            candidate_base,
            Ordering::SeqCst,
            Ordering::SeqCst,
        ) {
            Ok(_) => candidate_base,
            Err(selected) => selected,
        };
        if selected_base != candidate_base {
            return LINUX_EINVAL;
        }
    }
    let Ok(start) = usize::try_from(offset) else {
        return LINUX_EINVAL;
    };
    let Ok(file_size) = usize::try_from(file_size) else {
        return LINUX_EINVAL;
    };
    if start >= file.len() {
        return LINUX_EINVAL;
    }
    let file_size = if dependency.is_some() {
        file_size.min(file.len() - start)
    } else {
        file_size
    };
    let Some(end) = start
        .checked_add(file_size)
        .filter(|end| *end <= file.len())
    else {
        return LINUX_EINVAL;
    };
    let Some([lease]) = memory::allocate_frames::<1>(memory::FrameOwner::KERNEL) else {
        return LINUX_ENOMEM;
    };
    let frame = lease.address();
    if mapping_state.load(Ordering::SeqCst) & bit != 0 {
        let _ = memory::release_frames(memory::FrameOwner::KERNEL, [lease]);
        return LINUX_EEXIST;
    }
    if !address_space::prepare_linux_frame(frame, &file[start..end]) {
        let _ = memory::release_frames(memory::FrameOwner::KERNEL, [lease]);
        return LINUX_ENOMEM;
    }
    let writable = protection & LINUX_PROT_WRITE != 0;
    let executable = protection & LINUX_PROT_EXEC != 0;
    if !address_space::map_current_linux_file_page(address, frame, writable, executable) {
        let scrubbed = address_space::prepare_linux_frame(frame, &[]);
        if scrubbed {
            let _ = memory::release_frames(memory::FrameOwner::KERNEL, [lease]);
        }
        return LINUX_EEXIST;
    }
    if !append_linux_active_image_mapping(address, lease, writable, executable) {
        let unmapped = address_space::unmap_current_linux_page(address, frame);
        let scrubbed = unmapped && address_space::prepare_linux_frame(frame, &[]);
        if scrubbed {
            let _ = memory::release_frames(memory::FrameOwner::KERNEL, [lease]);
        }
        return LINUX_ENOMEM;
    }
    let (frame_state, generation_state) = if file_kind == LINUX_FILE_INIT {
        (
            &ELF_PROBE_MAIN_FRAMES[mapping_index],
            &ELF_PROBE_MAIN_GENERATIONS[mapping_index],
        )
    } else {
        let state = dependency.unwrap_or(0) * LINUX_DEPENDENCY_MAPPING_COUNT + mapping_index;
        (
            &ELF_PROBE_DEPENDENCY_FRAMES[state],
            &ELF_PROBE_DEPENDENCY_GENERATIONS[state],
        )
    };
    frame_state.store(lease.address(), Ordering::SeqCst);
    generation_state.store(lease.generation(), Ordering::SeqCst);
    mapping_state.fetch_or(bit, Ordering::SeqCst);
    address
}

fn linux_munmap(address: u64, length: u64) -> u64 {
    let Some((start_slot, range_pages)) = linux_mmap_range_slots(address, length) else {
        return LINUX_EINVAL;
    };
    let mut slots = linux_mmap_slot_attributes();
    if !linux_mmap_unmap_and_release_slots(start_slot, range_pages) {
        return LINUX_EINVAL;
    }
    for slot in &mut slots[start_slot..start_slot + range_pages] {
        *slot = None;
    }
    linux_mmap_rebuild_areas(slots);
    0
}

fn linux_madvise(address: u64, length: u64, advice: u64) -> u64 {
    let Some((start_slot, range_pages)) = linux_mmap_range_slots(address, length) else {
        return LINUX_EINVAL;
    };
    let areas: [Option<usize>; LINUX_RUNTIME_MMAP_SLOT_COUNT] = core::array::from_fn(|offset| {
        (offset < range_pages)
            .then(|| linux_mmap_slot_area_index(start_slot + offset))
            .flatten()
    });
    if advice != LINUX_MADV_DONTNEED
        || areas[..range_pages].iter().any(|area| {
            area.is_none_or(|index| {
                ELF_PROBE_MMAP_FILE[index].load(Ordering::SeqCst) != 0
                    || ELF_PROBE_MMAP_PROTECTION[index].load(Ordering::SeqCst) == 0
            })
        })
    {
        return LINUX_EINVAL;
    }
    for offset in 0..range_pages {
        if !address_space::zero_current_linux_page(
            address + (offset * PAGE_SIZE) as u64,
            linux_mmap_slot_frame(start_slot + offset),
        ) {
            return LINUX_EINVAL;
        }
    }
    0
}

fn linux_mincore(address: u64, length: u64, vector: u64) -> u64 {
    let Some((start_slot, range_pages)) = linux_mmap_range_slots(address, length) else {
        return LINUX_EINVAL;
    };
    let mut residency = [0_u8; LINUX_RUNTIME_MMAP_SLOT_COUNT];
    for (offset, resident) in residency.iter_mut().enumerate().take(range_pages) {
        let slot = start_slot + offset;
        let Some(area) = linux_mmap_slot_area_index(slot) else {
            return LINUX_ENOMEM;
        };
        let protection = u64::from(ELF_PROBE_MMAP_PROTECTION[area].load(Ordering::SeqCst));
        if protection == 0 {
            if linux_mmap_slot_frame(slot) != 0
                || ELF_PROBE_MMAP_GENERATIONS[slot].load(Ordering::SeqCst) != 0
                || !address_space::current_linux_page_absent(address + (offset * PAGE_SIZE) as u64)
            {
                return LINUX_EINVAL;
            }
        } else {
            let Some(lease) = linux_mmap_slot_lease(slot) else {
                return LINUX_EINVAL;
            };
            if !address_space::current_linux_mapping_matches(
                address + (offset * PAGE_SIZE) as u64,
                lease,
                protection & LINUX_PROT_WRITE != 0,
                false,
            ) {
                return LINUX_EINVAL;
            }
            *resident = 1;
        }
    }
    if !copy_to_linux_user(vector, &residency[..range_pages]) {
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
    let head = current_linux_robust_list().unwrap_or(0).to_ne_bytes();
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
    let exec_node = ELF_PROBE_EXEC_NODE.load(Ordering::SeqCst);
    let mut measured_path = [0_u8; 64];
    let executable = if exec_node == 0 {
        b"/init" as &[u8]
    } else {
        let Some(name) = initramfs::published_file_name_by_node(exec_node) else {
            return LINUX_ESRCH;
        };
        if name == b"linux/init" {
            b"/init" as &[u8]
        } else {
            measured_path[0] = b'/';
            let Some(destination) = measured_path.get_mut(1..1 + name.len()) else {
                return LINUX_ENAMETOOLONG;
            };
            destination.copy_from_slice(name);
            &measured_path[..1 + name.len()]
        }
    };
    let output_length = usize::try_from(output_size)
        .unwrap_or(usize::MAX)
        .min(executable.len());
    if !copy_to_linux_user(output_address, &executable[..output_length]) {
        return LINUX_EFAULT;
    }
    output_length as u64
}

fn linux_runtime_file_kind(path: &[u8]) -> Option<u16> {
    if !linux_mount::current_root_is_read_only() {
        return None;
    }
    if let Some(name) = path.strip_prefix(b"/proc/") {
        if let Some(kind) = linux_proc_root_file_kind(name) {
            return Some(kind);
        }
    }
    if let Some(name) = path.strip_prefix(b"/proc/self/") {
        return linux_proc_pid_file_kind(current_linux_process_pid(), name);
    }
    if path == b"/proc/1/status" {
        return Some(LINUX_FILE_PROC_STATUS);
    }
    if path == b"/proc/2/status" && linux_process::live_process_status(2).is_some() {
        return Some(LINUX_FILE_PROC_PID2_STATUS);
    }
    if let Some(name) = path.strip_prefix(b"/proc/1/") {
        return linux_proc_pid_file_kind(1, name);
    }
    if let Some(name) = path.strip_prefix(b"/proc/2/") {
        return linux_proc_pid_file_kind(2, name);
    }
    if path == b"/proc/1/mountinfo" {
        return Some(LINUX_FILE_PROC_MOUNTINFO);
    }
    if path == b"/proc/1/uid_map" {
        return Some(LINUX_FILE_PROC_UID_MAP);
    }
    if path == b"/proc/1/gid_map" {
        return Some(LINUX_FILE_PROC_GID_MAP);
    }
    if path == b"/proc/1/setgroups" {
        return Some(LINUX_FILE_PROC_SETGROUPS);
    }
    if path == b"/tmp/ginux-ready" && linux_tmpfs::open(b"ginux-ready").is_some() {
        return Some(LINUX_FILE_TMP_SCRATCH);
    }
    let (kind, root_name) = match path {
        b"/init" => (LINUX_FILE_INIT, b"linux/init" as &[u8]),
        b"/lib64/ld-ginux.so" => (LINUX_FILE_INTERPRETER, b"linux/ld-ginux.so" as &[u8]),
        b"/lib64/libginux.so" => (LINUX_FILE_DEPENDENCY, b"linux/libginux.so" as &[u8]),
        b"/lib64/libginux-aux.so" => (
            LINUX_FILE_AUXILIARY_DEPENDENCY,
            b"linux/libginux-aux.so" as &[u8],
        ),
        _ => {
            let root_name = path.strip_prefix(b"/")?;
            if root_name.starts_with(b"linux/") {
                return None;
            }
            let kind = if root_name == b"etc/hostname" {
                LINUX_FILE_ROOT_HOSTNAME
            } else {
                LINUX_FILE_ROOT_GENERIC
            };
            (kind, root_name)
        }
    };
    let node = initramfs::published_regular_file_node(root_name)?;
    Some(kind | (u16::from(node) << LINUX_ROOT_NODE_SHIFT))
}

fn linux_proc_root_file_kind(name: &[u8]) -> Option<u16> {
    match name {
        b"cpuinfo" => Some(LINUX_FILE_PROC_CPUINFO),
        b"meminfo" => Some(LINUX_FILE_PROC_MEMINFO),
        b"stat" => Some(LINUX_FILE_PROC_STAT),
        b"uptime" => Some(LINUX_FILE_PROC_UPTIME),
        b"loadavg" => Some(LINUX_FILE_PROC_LOADAVG),
        b"filesystems" => Some(LINUX_FILE_PROC_FILESYSTEMS),
        b"cmdline" => Some(LINUX_FILE_PROC_CMDLINE),
        _ => None,
    }
}

fn linux_proc_pid_file_kind(pid: u32, name: &[u8]) -> Option<u16> {
    linux_process::live_process_status(pid)?;
    match (pid, name) {
        (1, b"status") => Some(LINUX_FILE_PROC_STATUS),
        (1, b"stat") => Some(LINUX_FILE_PROC_PID1_STAT),
        (1, b"statm") => Some(LINUX_FILE_PROC_PID1_STATM),
        (1, b"mountinfo") => Some(LINUX_FILE_PROC_MOUNTINFO),
        (1, b"uid_map") => Some(LINUX_FILE_PROC_UID_MAP),
        (1, b"gid_map") => Some(LINUX_FILE_PROC_GID_MAP),
        (1, b"setgroups") => Some(LINUX_FILE_PROC_SETGROUPS),
        (2, b"status") => Some(LINUX_FILE_PROC_PID2_STATUS),
        (2, b"stat") => Some(LINUX_FILE_PROC_PID2_STAT),
        (2, b"statm") => Some(LINUX_FILE_PROC_PID2_STATM),
        _ => None,
    }
}

fn linux_root_directory_kind(path: &[u8]) -> Option<u16> {
    if !linux_mount::current_root_is_read_only() {
        return None;
    }
    match path {
        b"/" => Some(LINUX_ROOT_DIRECTORY),
        b"/dev" => Some(LINUX_DEVICE_DIRECTORY),
        b"/proc/self" => match current_linux_process_pid() {
            1 => Some(LINUX_PROC_PID1_DIRECTORY),
            2 if linux_process::live_process_status(2).is_some() => Some(LINUX_PROC_PID2_DIRECTORY),
            _ => None,
        },
        b"/proc/1" => Some(LINUX_PROC_PID1_DIRECTORY),
        b"/proc/2" if linux_process::live_process_status(2).is_some() => {
            Some(LINUX_PROC_PID2_DIRECTORY)
        }
        _ => {
            let name = path.strip_prefix(b"/")?;
            let node = initramfs::published_directory_node(name)?;
            Some(LINUX_ROOT_MEASURED_DIRECTORY | (u16::from(node) << LINUX_ROOT_NODE_SHIFT))
        }
    }
}

fn linux_measured_path_kind(path: &[u8]) -> Option<u16> {
    if !linux_mount::current_root_is_read_only() {
        return None;
    }
    if path == b"dev" {
        return Some(LINUX_DEVICE_DIRECTORY);
    }
    if let Some(name) = path.strip_prefix(b"proc/") {
        if let Some(kind) = linux_proc_root_file_kind(name) {
            return Some(kind);
        }
    }
    if let Some(name) = path.strip_prefix(b"proc/self/") {
        return linux_proc_pid_file_kind(current_linux_process_pid(), name);
    }
    if path == b"proc/self" {
        return match current_linux_process_pid() {
            1 => Some(LINUX_PROC_PID1_DIRECTORY),
            2 if linux_process::live_process_status(2).is_some() => Some(LINUX_PROC_PID2_DIRECTORY),
            _ => None,
        };
    }
    if path == b"proc/1" {
        return Some(LINUX_PROC_PID1_DIRECTORY);
    }
    if path == b"proc/2" && linux_process::live_process_status(2).is_some() {
        return Some(LINUX_PROC_PID2_DIRECTORY);
    }
    if path == b"proc/1/status" {
        return Some(LINUX_FILE_PROC_STATUS);
    }
    if path == b"proc/2/status" && linux_process::live_process_status(2).is_some() {
        return Some(LINUX_FILE_PROC_PID2_STATUS);
    }
    if let Some(name) = path.strip_prefix(b"proc/1/") {
        return linux_proc_pid_file_kind(1, name);
    }
    if let Some(name) = path.strip_prefix(b"proc/2/") {
        return linux_proc_pid_file_kind(2, name);
    }
    if path == b"proc/1/mountinfo" {
        return Some(LINUX_FILE_PROC_MOUNTINFO);
    }
    if path == b"proc/1/uid_map" {
        return Some(LINUX_FILE_PROC_UID_MAP);
    }
    if path == b"proc/1/gid_map" {
        return Some(LINUX_FILE_PROC_GID_MAP);
    }
    if path == b"proc/1/setgroups" {
        return Some(LINUX_FILE_PROC_SETGROUPS);
    }
    if path == b"tmp/ginux-ready" && linux_tmpfs::open(b"ginux-ready").is_some() {
        return Some(LINUX_FILE_TMP_SCRATCH);
    }
    if let Some(node) = initramfs::published_directory_node(path) {
        return Some(LINUX_ROOT_MEASURED_DIRECTORY | (u16::from(node) << LINUX_ROOT_NODE_SHIFT));
    }
    let mut absolute = [0_u8; 64];
    absolute[0] = b'/';
    absolute.get_mut(1..1 + path.len())?.copy_from_slice(path);
    linux_runtime_file_kind(&absolute[..1 + path.len()])
}

fn linux_virtual_mount_path_at(directory: u64, path: &[u8]) -> Option<u16> {
    let relative = path.strip_prefix(b"/").unwrap_or(path);
    if relative != b"proc" && !relative.starts_with(b"proc/") {
        return None;
    }
    let from_root = path.starts_with(b"/")
        || if directory as u32 == LINUX_AT_FDCWD {
            ELF_PROBE_CWD.load(Ordering::SeqCst) == LINUX_ROOT_DIRECTORY
                && matches!(
                    ELF_PROBE_OCI_CWD.load(Ordering::SeqCst),
                    0 | crate::oci_image::ROOT_HANDLE
                )
        } else {
            linux_device_descriptor(directory) & !3 == LINUX_ROOT_DIRECTORY
                && matches!(
                    linux_oci_descriptor(directory),
                    0 | crate::oci_image::ROOT_HANDLE
                )
        };
    from_root
        .then(|| linux_measured_path_kind(relative))
        .flatten()
}

fn linux_normalized_relative_path(path: &[u8]) -> bool {
    !path.is_empty()
        && path
            .split(|byte| *byte == b'/')
            .all(|component| !component.is_empty() && component != b"." && component != b"..")
}

fn linux_getcwd(address: u64, capacity: u64) -> u64 {
    let state = ELF_PROBE_CWD.load(Ordering::SeqCst);
    let mut output = [0_u8; 64];
    output[0] = b'/';
    let oci_cwd = ELF_PROBE_OCI_CWD.load(Ordering::SeqCst);
    let length = if LINUX_RUNTIME_MODE.load(Ordering::SeqCst) && oci_cwd != 0 {
        let Some(path_length) = crate::oci_image::copy_path(oci_cwd, &mut output[1..]) else {
            return LINUX_ENOENT;
        };
        1 + path_length
    } else if state == LINUX_ROOT_DIRECTORY {
        1
    } else if state == LINUX_DEVICE_DIRECTORY {
        output[1..4].copy_from_slice(b"dev");
        4
    } else {
        let Some(path) = linux_measured_directory_path(state) else {
            return LINUX_ENOENT;
        };
        let Some(destination) = output.get_mut(1..1 + path.len()) else {
            return LINUX_ERANGE;
        };
        destination.copy_from_slice(path);
        1 + path.len()
    };
    let required = length + 1;
    if capacity < required as u64 {
        return LINUX_ERANGE;
    }
    if !copy_to_linux_user(address, &output[..required]) {
        return LINUX_EFAULT;
    }
    required as u64
}

fn linux_chdir(path_address: u64) -> u64 {
    let mut path = [0_u8; 32];
    let length = match copy_linux_c_string(path_address, &mut path) {
        Ok(length) => length,
        Err(error) => return error,
    };
    let path = &path[..length];
    if LINUX_RUNTIME_MODE.load(Ordering::SeqCst) {
        if let Some(kind) = linux_virtual_mount_path_at(u64::from(LINUX_AT_FDCWD), path) {
            if kind & LINUX_DEVICE_DIRECTORY == 0 {
                return LINUX_ENOTDIR;
            }
            ELF_PROBE_OCI_CWD.store(0, Ordering::SeqCst);
            ELF_PROBE_CWD.store(kind & !3, Ordering::SeqCst);
            return 0;
        }
        let Some((handle, entry)) = linux_oci_path_at(u64::from(LINUX_AT_FDCWD), path) else {
            return LINUX_ENOENT;
        };
        if entry.is_some_and(|entry| entry.kind != ginux_oci::EntryKind::Directory) {
            return LINUX_ENOTDIR;
        }
        ELF_PROBE_OCI_CWD.store(handle, Ordering::SeqCst);
        ELF_PROBE_CWD.store(LINUX_ROOT_DIRECTORY, Ordering::SeqCst);
        return 0;
    }
    let kind = if path.starts_with(b"/") {
        linux_root_directory_kind(path)
    } else {
        match linux_relative_path_kind(u64::from(LINUX_AT_FDCWD), path) {
            Ok(kind) => kind,
            Err(error) => return error,
        }
    };
    let Some(kind) = kind else {
        return LINUX_ENOENT;
    };
    if kind & LINUX_DEVICE_DIRECTORY == 0 {
        return LINUX_ENOTDIR;
    }
    ELF_PROBE_CWD.store(kind & !3, Ordering::SeqCst);
    0
}

fn linux_fchdir(descriptor: u64) -> u64 {
    let state = linux_device_descriptor(descriptor);
    if state == 0 {
        return LINUX_EBADF;
    }
    if state & LINUX_DEVICE_DIRECTORY == 0 {
        return LINUX_ENOTDIR;
    }
    if LINUX_RUNTIME_MODE.load(Ordering::SeqCst) {
        let handle = linux_oci_descriptor(descriptor);
        if handle == 0 {
            return LINUX_ENOENT;
        }
        ELF_PROBE_OCI_CWD.store(handle, Ordering::SeqCst);
    }
    ELF_PROBE_CWD.store(state & !3, Ordering::SeqCst);
    0
}

fn linux_exec_vector(address: u64, require_nonempty: bool) -> Result<([u64; 8], usize), u64> {
    let mut pointers = [0_u64; LINUX_EXEC_VECTOR_LIMIT];
    for index in 0..=LINUX_EXEC_VECTOR_LIMIT {
        let Some(pointer_address) = address.checked_add((index * size_of::<u64>()) as u64) else {
            return Err(LINUX_EFAULT);
        };
        let mut bytes = [0_u8; size_of::<u64>()];
        if !copy_from_linux_user(pointer_address, &mut bytes) {
            return Err(LINUX_EFAULT);
        }
        let pointer = u64::from_ne_bytes(bytes);
        if pointer == 0 {
            if require_nonempty && index == 0 {
                return Err(LINUX_E2BIG);
            }
            return Ok((pointers, index));
        }
        if index == LINUX_EXEC_VECTOR_LIMIT {
            return Err(LINUX_E2BIG);
        }
        pointers[index] = pointer;
    }
    Err(LINUX_E2BIG)
}

fn linux_exec_root_node(directory: u64, path: &[u8]) -> Result<u8, u64> {
    if !linux_mount::current_root_is_read_only() {
        return Err(LINUX_ENOENT);
    }
    let mut root_name = [0_u8; 64];
    let root_name_length = if let Some(absolute) = path.strip_prefix(b"/") {
        if !linux_normalized_relative_path(absolute) {
            return Err(LINUX_ENOENT);
        }
        let Some(destination) = root_name.get_mut(..absolute.len()) else {
            return Err(LINUX_ENAMETOOLONG);
        };
        destination.copy_from_slice(absolute);
        absolute.len()
    } else {
        if !linux_normalized_relative_path(path) {
            return Err(LINUX_ENOENT);
        }
        let directory_state = if directory as u32 == LINUX_AT_FDCWD {
            ELF_PROBE_CWD.load(Ordering::SeqCst)
        } else {
            let state = linux_device_descriptor(directory);
            if state == 0 {
                return Err(LINUX_EBADF);
            }
            if state & LINUX_DEVICE_DIRECTORY == 0 {
                return Err(LINUX_ENOTDIR);
            }
            state & !3
        };
        let Some(directory) = linux_measured_directory_path(directory_state) else {
            return Err(LINUX_ENOENT);
        };
        let separator = usize::from(!directory.is_empty());
        let Some(length) = directory
            .len()
            .checked_add(separator)
            .and_then(|length| length.checked_add(path.len()))
        else {
            return Err(LINUX_ENAMETOOLONG);
        };
        let Some(destination) = root_name.get_mut(..length) else {
            return Err(LINUX_ENAMETOOLONG);
        };
        destination[..directory.len()].copy_from_slice(directory);
        if separator != 0 {
            destination[directory.len()] = b'/';
        }
        destination[directory.len() + separator..].copy_from_slice(path);
        length
    };
    let root_name = match &root_name[..root_name_length] {
        b"init" => b"linux/init" as &[u8],
        name => name,
    };
    let Some((mode, _)) = initramfs::published_lookup(root_name) else {
        return Err(LINUX_ENOENT);
    };
    if mode & 0o170000 != 0o100000 || mode & 0o111 == 0 {
        return Err(LINUX_EACCES);
    }
    initramfs::published_regular_file_node(root_name).ok_or(LINUX_ENOENT)
}

fn linux_execve(path_address: u64, arguments_address: u64, environment_address: u64) -> u64 {
    linux_execve_at(
        u64::from(LINUX_AT_FDCWD),
        path_address,
        arguments_address,
        environment_address,
        false,
    )
}

fn linux_exec_descriptor_node(descriptor: u64) -> Result<u8, u64> {
    let state = linux_device_descriptor(descriptor);
    if state == 0 {
        return Err(LINUX_EBADF);
    }
    if state & LINUX_DEVICE_DIRECTORY != 0 {
        return Err(LINUX_EACCES);
    }
    let node = ((state & LINUX_ROOT_NODE_MASK) >> LINUX_ROOT_NODE_SHIFT) as u8;
    let Some(name) = initramfs::published_file_name_by_node(node) else {
        return Err(LINUX_EACCES);
    };
    let Some((mode, _)) = initramfs::published_lookup(name) else {
        return Err(LINUX_ENOENT);
    };
    if mode & 0o111 == 0 {
        return Err(LINUX_EACCES);
    }
    Ok(node)
}

fn linux_exec_process_name(node: u8) -> Option<[u64; 2]> {
    let name = initramfs::published_file_name_by_node(node)?;
    let basename = name.rsplit(|byte| *byte == b'/').next()?;
    if basename.is_empty() || basename.len() > 15 {
        return None;
    }
    let mut bytes = [0_u8; 16];
    bytes[..basename.len()].copy_from_slice(basename);
    Some([
        u64::from_ne_bytes(bytes[..8].try_into().ok()?),
        u64::from_ne_bytes(bytes[8..].try_into().ok()?),
    ])
}

fn linux_measured_interpreted_exec_plan(
) -> Option<crate::linux_elf::DependencyGraphLoadPlan<'static, 'static, 'static>> {
    let file = |name: &[u8]| {
        initramfs::published_regular_file_node(name).and_then(initramfs::published_file_by_node)
    };
    crate::linux_elf::measured_interpreted_exec_plan(
        file(b"linux/init")?,
        file(b"linux/ld-ginux.so")?,
        [file(b"linux/libginux.so")?, file(b"linux/libginux-aux.so")?],
    )
}

fn release_linux_exec_frames(
    frames: &mut [Option<memory::FrameLease>; LINUX_EXEC_MAPPING_LIMIT],
) -> bool {
    let mut scrubbed = true;
    let mut released = true;
    for slot in frames {
        if let Some(frame) = *slot {
            let frame_scrubbed = address_space::prepare_linux_frame(frame.address(), &[]);
            scrubbed &= frame_scrubbed;
            if frame_scrubbed {
                let frame_released = memory::release_frames(memory::FrameOwner::KERNEL, [frame]);
                released &= frame_released;
                if frame_released {
                    *slot = None;
                }
            } else {
                released = false;
            }
        }
    }
    scrubbed && released
}

#[inline(never)]
fn replace_current_linux_image_with_hierarchy(
    mappings: &[LinuxExecMapping],
    old_mappings: &[LinuxActiveImageMapping],
    hierarchy_count: usize,
) -> bool {
    let old: [Option<memory::FrameLease>; 6] = core::array::from_fn(|index| {
        memory::FrameLease::from_parts(
            ELF_PROBE_PAGE_TABLE_FRAMES[index].load(Ordering::SeqCst),
            ELF_PROBE_PAGE_TABLE_GENERATIONS[index].load(Ordering::SeqCst),
        )
    });
    match (hierarchy_count, old) {
        (5, [Some(root), Some(pdpt), Some(directory), Some(main), Some(read_only), None]) => {
            replace_current_linux_image_hierarchy(
                mappings,
                old_mappings,
                [root, pdpt, directory, main, read_only],
            )
        }
        (
            6,
            [Some(root), Some(pdpt), Some(directory), Some(read_only), Some(main), Some(interpreter)],
        ) => replace_current_linux_image_hierarchy(
            mappings,
            old_mappings,
            [root, pdpt, directory, read_only, main, interpreter],
        ),
        _ => false,
    }
}

#[inline(never)]
fn replace_current_linux_image_hierarchy<const N: usize>(
    mappings: &[LinuxExecMapping],
    old_mappings: &[LinuxActiveImageMapping],
    old: [memory::FrameLease; N],
) -> bool {
    if address_space::current_root_address() != old[0].address()
        || !memory::frames_owned_by(memory::FrameOwner::KERNEL, &old)
        || old
            .iter()
            .enumerate()
            .any(|(index, frame)| old[..index].contains(frame))
    {
        return false;
    }
    let Some(new) = memory::allocate_frames::<N>(memory::FrameOwner::KERNEL) else {
        return false;
    };
    if !new
        .iter()
        .all(|frame| address_space::prepare_linux_frame(frame.address(), &[]))
        || !address_space::replace_current_linux_image_and_hierarchy(
            mappings,
            old_mappings,
            &old,
            &new,
        )
    {
        let _ = memory::scrub_and_release_frames(memory::FrameOwner::KERNEL, &new);
        return false;
    }
    for (index, frame) in new.into_iter().enumerate() {
        ELF_PROBE_PAGE_TABLE_FRAMES[index].store(frame.address(), Ordering::SeqCst);
        ELF_PROBE_PAGE_TABLE_GENERATIONS[index].store(frame.generation(), Ordering::SeqCst);
    }
    true
}

fn linux_execveat(
    directory: u64,
    path_address: u64,
    arguments_address: u64,
    environment_address: u64,
    flags: u64,
) -> u64 {
    if flags & !LINUX_AT_EMPTY_PATH != 0 {
        return LINUX_EINVAL;
    }
    linux_execve_at(
        directory,
        path_address,
        arguments_address,
        environment_address,
        flags & LINUX_AT_EMPTY_PATH != 0,
    )
}

fn linux_oci_execve(path: &[u8], arguments_address: u64, environment_address: u64) -> u64 {
    let Some(plan) = crate::oci_image::process_plan(path) else {
        return LINUX_ENOEXEC;
    };
    let startup = crate::linux_elf::relocatable_startup(&plan);
    let (argument_pointers, argument_count) = match linux_exec_vector(arguments_address, true) {
        Ok(vector) => vector,
        Err(error) => return error,
    };
    let (environment_pointers, environment_count) =
        match linux_exec_vector(environment_address, false) {
            Ok(vector) => vector,
            Err(error) => return error,
        };
    let stack_page = ELF_PROBE_STACK.load(Ordering::SeqCst) & !(PAGE_SIZE as u64 - 1);
    let new_stack = stack_page + PAGE_SIZE as u64 - LINUX_EXEC_STACK_SIZE as u64;
    let mut stack_image = [0_u8; LINUX_EXEC_STACK_SIZE];
    let table_words =
        1 + argument_count + 1 + environment_count + 1 + crate::linux_elf::AUXILIARY_VECTOR_WORDS;
    let mut string_offset = table_words * size_of::<u64>();
    stack_image[..8].copy_from_slice(&(argument_count as u64).to_ne_bytes());
    let mut string = [0_u8; LINUX_EXEC_STRING_LIMIT];
    let mut argument_zero_address = 0;
    for (index, pointer) in argument_pointers[..argument_count].iter().enumerate() {
        string.fill(0);
        let length = match copy_linux_c_string(*pointer, &mut string) {
            Ok(length) => length + 1,
            Err(LINUX_ENAMETOOLONG) => return LINUX_E2BIG,
            Err(error) => return error,
        };
        let Some(end) = string_offset.checked_add(length) else {
            return LINUX_E2BIG;
        };
        let Some(output) = stack_image.get_mut(string_offset..end) else {
            return LINUX_E2BIG;
        };
        output.copy_from_slice(&string[..length]);
        let address = new_stack + string_offset as u64;
        stack_image[(1 + index) * 8..(2 + index) * 8].copy_from_slice(&address.to_ne_bytes());
        if index == 0 {
            argument_zero_address = address;
        }
        string_offset = end;
    }
    let environment_word = 1 + argument_count + 1;
    for (index, pointer) in environment_pointers[..environment_count].iter().enumerate() {
        string.fill(0);
        let length = match copy_linux_c_string(*pointer, &mut string) {
            Ok(length) => length + 1,
            Err(LINUX_ENAMETOOLONG) => return LINUX_E2BIG,
            Err(error) => return error,
        };
        let Some(end) = string_offset.checked_add(length) else {
            return LINUX_E2BIG;
        };
        let Some(output) = stack_image.get_mut(string_offset..end) else {
            return LINUX_E2BIG;
        };
        output.copy_from_slice(&string[..length]);
        stack_image[(environment_word + index) * 8..(environment_word + index + 1) * 8]
            .copy_from_slice(&(new_stack + string_offset as u64).to_ne_bytes());
        string_offset = end;
    }
    let platform_offset = string_offset;
    let Some(platform_end) = platform_offset.checked_add(LINUX_PLATFORM.len()) else {
        return LINUX_E2BIG;
    };
    let Some(output) = stack_image.get_mut(platform_offset..platform_end) else {
        return LINUX_E2BIG;
    };
    output.copy_from_slice(LINUX_PLATFORM);
    let auxiliary_word = environment_word + environment_count + 1;
    for (index, value) in crate::linux_elf::auxiliary_vector(
        startup,
        argument_zero_address,
        new_stack + platform_offset as u64,
    )
    .into_iter()
    .enumerate()
    {
        stack_image[(auxiliary_word + index) * 8..(auxiliary_word + index + 1) * 8]
            .copy_from_slice(&value.to_ne_bytes());
    }

    let pid = current_linux_process_pid();
    for image in [&plan.main, &plan.interpreter] {
        for segment in image.segments[..image.segment_count].iter().flatten() {
            for page in 0..segment.page_count {
                let Some(address) = segment
                    .virtual_page
                    .checked_add((page * PAGE_SIZE) as u64)
                    .and_then(|address| usize::try_from(address).ok())
                else {
                    return LINUX_ENOEXEC;
                };
                let Some(mapping) = linux_process::live_process_mapping(pid, address) else {
                    return LINUX_ENOEXEC;
                };
                if !address_space::set_current_linux_mapping_permissions(
                    address as u64,
                    mapping.frame,
                    segment.writable,
                    segment.executable,
                ) || !linux_process::set_live_process_mapping_permissions(
                    pid,
                    address,
                    segment.writable,
                    segment.executable,
                ) || segment.writable
                    && !address_space::materialize_linux_exec_frame(mapping.frame, *segment, page)
                {
                    return LINUX_ENOEXEC;
                }
                if let Some(index) = (0..usize::from(
                    ELF_PROBE_ACTIVE_IMAGE_COUNT.load(Ordering::SeqCst),
                ))
                    .find(|index| {
                        ELF_PROBE_ACTIVE_IMAGE_ADDRESSES[*index].load(Ordering::SeqCst)
                            == address as u64
                    })
                {
                    ELF_PROBE_ACTIVE_IMAGE_FLAGS[index].store(
                        u8::from(segment.writable) | (u8::from(segment.executable) << 1),
                        Ordering::SeqCst,
                    );
                }
            }
        }
    }
    let heap_base =
        ELF_PROBE_BRK_LIMIT.load(Ordering::SeqCst) as usize - LINUX_RUNTIME_HEAP_PAGES * PAGE_SIZE;
    for page in 0..LINUX_RUNTIME_HEAP_PAGES {
        let Some(mapping) = linux_process::live_process_mapping(pid, heap_base + page * PAGE_SIZE)
        else {
            return LINUX_ENOEXEC;
        };
        if !address_space::prepare_linux_frame(mapping.frame.address(), &[]) {
            return LINUX_ENOMEM;
        }
    }
    for page in 0..LINUX_RUNTIME_STACK_PAGES {
        let address = stack_page as usize - page * PAGE_SIZE;
        let Some(mapping) = linux_process::live_process_mapping(pid, address) else {
            return LINUX_ENOEXEC;
        };
        if !address_space::prepare_linux_frame(mapping.frame.address(), &[]) {
            return LINUX_ENOMEM;
        }
    }
    let Some(stack_mapping) = linux_process::live_process_mapping(pid, stack_page as usize) else {
        return LINUX_ENOEXEC;
    };
    if !address_space::materialize_linux_exec_stack_frame(
        stack_mapping.frame,
        PAGE_SIZE - LINUX_EXEC_STACK_SIZE,
        &stack_image,
    ) {
        return LINUX_ENOMEM;
    }
    let process_name = path.rsplit(|byte| *byte == b'/').next().unwrap_or(path);
    if process_name.is_empty() || process_name.len() > 15 {
        return LINUX_ENOEXEC;
    }
    let mut name = [0_u8; 16];
    name[..process_name.len()].copy_from_slice(process_name);
    ELF_PROBE_PROCESS_NAME_LOW.store(
        u64::from_ne_bytes(name[..8].try_into().unwrap_or([0; 8])),
        Ordering::SeqCst,
    );
    ELF_PROBE_PROCESS_NAME_HIGH.store(
        u64::from_ne_bytes(name[8..].try_into().unwrap_or([0; 8])),
        Ordering::SeqCst,
    );
    ELF_PROBE_BRK.store(heap_base as u64, Ordering::SeqCst);
    let Some(main_code) = plan.main.segments[..plan.main.segment_count]
        .iter()
        .flatten()
        .find(|segment| segment.executable)
    else {
        return LINUX_ENOEXEC;
    };
    let Some(loader_code) = plan.interpreter.segments[..plan.interpreter.segment_count]
        .iter()
        .flatten()
        .find(|segment| segment.executable)
    else {
        return LINUX_ENOEXEC;
    };
    ELF_PROBE_CODE_BASE.store(main_code.virtual_page, Ordering::SeqCst);
    ELF_PROBE_CODE_SIZE.store((main_code.page_count * PAGE_SIZE) as u64, Ordering::SeqCst);
    ELF_PROBE_LOADER_CODE_BASE.store(loader_code.virtual_page, Ordering::SeqCst);
    ELF_PROBE_LOADER_CODE_SIZE.store(
        (loader_code.page_count * PAGE_SIZE) as u64,
        Ordering::SeqCst,
    );
    let close_on_exec = ELF_PROBE_CONSOLE_CLOEXEC.load(Ordering::SeqCst);
    for descriptor in 0..LINUX_AUX_DESCRIPTOR_LIMIT {
        if close_on_exec & (1 << descriptor) != 0 {
            let _ = linux_descriptor_close(descriptor);
        }
    }
    let _ = set_current_linux_clear_child_tid(0);
    let _ = set_current_linux_robust_list(0);
    if !install_linux_tls_context(LinuxTlsContext {
        thread_pointer: 0,
        extent: 0,
        extent_size: 0,
    }) {
        return LINUX_ENOMEM;
    }
    unsafe {
        LINUX_USER_RIP = startup.initial_entry;
        LINUX_USER_RSP = new_stack;
    }
    0
}

fn linux_execve_at(
    directory: u64,
    path_address: u64,
    arguments_address: u64,
    environment_address: u64,
    allow_empty_path: bool,
) -> u64 {
    if current_linux_tid() != Some(1)
        || LINUX_THREAD_STATES
            .iter()
            .filter(|state| state.load(Ordering::SeqCst) != LINUX_THREAD_FREE)
            .count()
            != 1
        || ELF_PROBE_EXEC_REPLACED.load(Ordering::SeqCst)
    {
        if LINUX_RUNTIME_MODE.load(Ordering::SeqCst) && current_linux_process_pid() != 1 {
            serial_write("ginux: OCI child exec precondition rejected\r\n");
        }
        return LINUX_EAGAIN;
    }
    let mut path = [0_u8; 64];
    let path_length = match copy_linux_c_string(path_address, &mut path) {
        Ok(length) => length,
        Err(error) => return error,
    };
    if LINUX_RUNTIME_MODE.load(Ordering::SeqCst) && current_linux_process_pid() != 1 {
        let result = linux_oci_execve(&path[..path_length], arguments_address, environment_address);
        if result != 0 {
            serial_write("ginux: OCI child exec rejected path/error ");
            crate::serial_write_bytes(&path[..path_length]);
            serial_write(" ");
            serial_write_u64(result);
            serial_write("\r\n");
        }
        return result;
    }
    let exec_node = match if path_length == 0 {
        if allow_empty_path {
            linux_exec_descriptor_node(directory)
        } else {
            Err(LINUX_ENOENT)
        }
    } else {
        linux_exec_root_node(directory, &path[..path_length])
    } {
        Ok(node) => node,
        Err(error) => return error,
    };
    let Some(exec_bytes) = initramfs::published_file_by_node(exec_node) else {
        return LINUX_ENOENT;
    };
    let static_exec_plan = crate::linux_elf::measured_static_exec_plan(exec_bytes);
    let interpreted_exec_plan = if static_exec_plan.is_none()
        && initramfs::published_file_name_by_node(exec_node) == Some(b"linux/init")
    {
        linux_measured_interpreted_exec_plan()
    } else {
        None
    };
    let Some(exec_plan) = static_exec_plan.as_ref().or_else(|| {
        interpreted_exec_plan
            .as_ref()
            .map(|plan| &plan.process.interpreter)
    }) else {
        return LINUX_ENOEXEC;
    };
    let Some(process_name) = linux_exec_process_name(exec_node) else {
        return LINUX_ENOEXEC;
    };
    if interpreted_exec_plan.is_some() {
        serial_write("ginux: measured-root interpreted Linux exec admitted\r\n");
    }
    let (argument_pointers, argument_count) = match linux_exec_vector(arguments_address, true) {
        Ok(vector) => vector,
        Err(error) => return error,
    };
    let (environment_pointers, environment_count) =
        match linux_exec_vector(environment_address, false) {
            Ok(vector) => vector,
            Err(error) => return error,
        };
    let close_on_exec = ELF_PROBE_CONSOLE_CLOEXEC.load(Ordering::SeqCst);
    if close_on_exec >> LINUX_AUX_DESCRIPTOR_LIMIT != 0
        || (0..LINUX_AUX_DESCRIPTOR_LIMIT).any(|descriptor| {
            close_on_exec & (1 << descriptor) != 0 && !linux_descriptor_open(descriptor)
        })
    {
        return LINUX_ESRCH;
    }
    let Some((mut active_image_mappings, active_image_count)) = linux_active_image_mappings()
    else {
        return LINUX_ENOEXEC;
    };
    let startup = interpreted_exec_plan.as_ref().map_or_else(
        || crate::linux_elf::static_startup(exec_plan),
        |plan| crate::linux_elf::interpreted_startup(&plan.process),
    );
    let entry = startup.initial_entry;
    let Some((mapping_templates, mapping_count)) = linux_exec_mapping_plan(exec_plan) else {
        return LINUX_ENOEXEC;
    };
    let interpreted = interpreted_exec_plan.is_some();
    let ancillary_mapping_count = 2 + 3 * usize::from(interpreted);
    if mapping_count > LINUX_EXEC_MAPPING_LIMIT - ancillary_mapping_count {
        return LINUX_E2BIG;
    }
    let stack_base = ELF_PROBE_STACK.load(Ordering::SeqCst) & !(PAGE_SIZE as u64 - 1);
    let heap_base = ELF_PROBE_BRK_LIMIT
        .load(Ordering::SeqCst)
        .saturating_sub(PAGE_SIZE as u64);
    let new_stack = stack_base + PAGE_SIZE as u64 - LINUX_EXEC_STACK_SIZE as u64;
    let mut stack_image = [0_u8; LINUX_EXEC_STACK_SIZE];
    if active_image_count > LINUX_ACTIVE_IMAGE_MAPPING_LIMIT - ancillary_mapping_count {
        return LINUX_E2BIG;
    }
    let Some(old_stack_frame) = memory::FrameLease::from_parts(
        ELF_PROBE_STACK_FRAME.load(Ordering::SeqCst),
        ELF_PROBE_STACK_GENERATION.load(Ordering::SeqCst),
    ) else {
        return LINUX_ENOEXEC;
    };
    if stack_base == 0
        || !address_space::current_linux_mapping_matches(stack_base, old_stack_frame, true, false)
    {
        return LINUX_ENOEXEC;
    }
    if (0..LINUX_RUNTIME_MMAP_SLOT_COUNT).any(|index| {
        ELF_PROBE_MMAP_PAGES[index].load(Ordering::SeqCst) != 0
            || ELF_PROBE_MMAP_PROTECTION[index].load(Ordering::SeqCst) != 0
            || ELF_PROBE_MMAP_FILE[index].load(Ordering::SeqCst) != 0
            || linux_mmap_slot_frame(index) != 0
            || ELF_PROBE_MMAP_GENERATIONS[index].load(Ordering::SeqCst) != 0
    }) {
        return LINUX_ENOEXEC;
    }
    let interpreted_tls = if let Some(plan) = interpreted_exec_plan.as_ref() {
        let Some(initial) = current_linux_thread_handle().and_then(linux_thread_context) else {
            return LINUX_ESRCH;
        };
        let sibling = ELF_PROBE_SIBLING_FS_BASE.load(Ordering::SeqCst);
        let thread_pointers = [initial.thread_pointer as u64, sibling];
        if sibling == 0 || !address_space::current_linux_tls_rematerializable(plan, thread_pointers)
        {
            return LINUX_ENOEXEC;
        }
        Some((initial, thread_pointers))
    } else {
        None
    };
    let table_words =
        1 + argument_count + 1 + environment_count + 1 + crate::linux_elf::AUXILIARY_VECTOR_WORDS;
    let mut string_offset = table_words * size_of::<u64>();
    stack_image[..8].copy_from_slice(&(argument_count as u64).to_ne_bytes());
    let mut string = [0_u8; LINUX_EXEC_STRING_LIMIT];
    let mut argument_zero_address = 0;
    for (index, pointer) in argument_pointers[..argument_count].iter().enumerate() {
        string.fill(0);
        let length = match copy_linux_c_string(*pointer, &mut string) {
            Ok(length) => length + 1,
            Err(LINUX_ENAMETOOLONG) => return LINUX_E2BIG,
            Err(error) => return error,
        };
        let Some(end) = string_offset.checked_add(length) else {
            return LINUX_E2BIG;
        };
        let Some(output) = stack_image.get_mut(string_offset..end) else {
            return LINUX_E2BIG;
        };
        output.copy_from_slice(&string[..length]);
        let word = 1 + index;
        stack_image[word * 8..word * 8 + 8]
            .copy_from_slice(&(new_stack + string_offset as u64).to_ne_bytes());
        if index == 0 {
            argument_zero_address = new_stack + string_offset as u64;
        }
        string_offset = end;
    }
    let environment_word = 1 + argument_count + 1;
    for (index, pointer) in environment_pointers[..environment_count].iter().enumerate() {
        string.fill(0);
        let length = match copy_linux_c_string(*pointer, &mut string) {
            Ok(length) => length + 1,
            Err(LINUX_ENAMETOOLONG) => return LINUX_E2BIG,
            Err(error) => return error,
        };
        let Some(end) = string_offset.checked_add(length) else {
            return LINUX_E2BIG;
        };
        let Some(output) = stack_image.get_mut(string_offset..end) else {
            return LINUX_E2BIG;
        };
        output.copy_from_slice(&string[..length]);
        let word = environment_word + index;
        stack_image[word * 8..word * 8 + 8]
            .copy_from_slice(&(new_stack + string_offset as u64).to_ne_bytes());
        string_offset = end;
    }
    let auxiliary_word = environment_word + environment_count + 1;
    let platform_offset = string_offset;
    let Some(platform_end) = platform_offset.checked_add(LINUX_PLATFORM.len()) else {
        return LINUX_E2BIG;
    };
    let Some(output) = stack_image.get_mut(platform_offset..platform_end) else {
        return LINUX_E2BIG;
    };
    output.copy_from_slice(LINUX_PLATFORM);
    let mut auxiliary = crate::linux_elf::auxiliary_vector(
        startup,
        argument_zero_address,
        new_stack + platform_offset as u64,
    );
    if interpreted {
        auxiliary[33] = 1;
    }
    for (index, value) in auxiliary.into_iter().enumerate() {
        let word = auxiliary_word + index;
        stack_image[word * 8..word * 8 + 8].copy_from_slice(&value.to_ne_bytes());
    }
    let mut exec_frames = [None; LINUX_EXEC_MAPPING_LIMIT];
    let segments = [
        exec_plan.read_only,
        Some(exec_plan.executable),
        Some(exec_plan.writable),
    ];
    let mut mapping_index = 0;
    for segment in segments.into_iter().flatten() {
        for page in 0..segment.page_count {
            let Some([frame]) = memory::allocate_frames::<1>(memory::FrameOwner::KERNEL) else {
                let _ = release_linux_exec_frames(&mut exec_frames);
                return LINUX_ENOMEM;
            };
            exec_frames[mapping_index] = Some(frame);
            if !address_space::materialize_linux_exec_frame(frame, segment, page) {
                let _ = release_linux_exec_frames(&mut exec_frames);
                return LINUX_ENOMEM;
            }
            mapping_index += 1;
        }
    }
    if mapping_index != mapping_count {
        let _ = release_linux_exec_frames(&mut exec_frames);
        return LINUX_ENOEXEC;
    }
    {
        let Some([frame]) = memory::allocate_frames::<1>(memory::FrameOwner::KERNEL) else {
            let _ = release_linux_exec_frames(&mut exec_frames);
            return LINUX_ENOMEM;
        };
        exec_frames[mapping_count] = Some(frame);
        if !address_space::materialize_linux_exec_stack_frame(
            frame,
            PAGE_SIZE - LINUX_EXEC_STACK_SIZE,
            &stack_image,
        ) {
            let _ = release_linux_exec_frames(&mut exec_frames);
            return LINUX_ENOMEM;
        }
    }
    if interpreted {
        let Some([frame]) = memory::allocate_frames::<1>(memory::FrameOwner::KERNEL) else {
            let _ = release_linux_exec_frames(&mut exec_frames);
            return LINUX_ENOMEM;
        };
        exec_frames[mapping_count + 4] = Some(frame);
        if !address_space::prepare_linux_frame(frame.address(), &[]) {
            let _ = release_linux_exec_frames(&mut exec_frames);
            return LINUX_ENOMEM;
        }
    }
    if let Some((_, thread_pointers)) = interpreted_tls {
        let Some(frames) = memory::allocate_frames::<2>(memory::FrameOwner::KERNEL) else {
            let _ = release_linux_exec_frames(&mut exec_frames);
            return LINUX_ENOMEM;
        };
        exec_frames[mapping_count + 2] = Some(frames[0]);
        exec_frames[mapping_count + 3] = Some(frames[1]);
        if !address_space::materialize_linux_exec_tls_frames(
            interpreted_exec_plan
                .as_ref()
                .unwrap_or_else(|| unreachable!()),
            thread_pointers,
            frames,
        ) {
            let _ = release_linux_exec_frames(&mut exec_frames);
            return LINUX_ENOMEM;
        }
    }
    {
        let Some([frame]) = memory::allocate_frames::<1>(memory::FrameOwner::KERNEL) else {
            let _ = release_linux_exec_frames(&mut exec_frames);
            return LINUX_ENOMEM;
        };
        exec_frames[mapping_count + 1] = Some(frame);
        if !address_space::prepare_linux_frame(frame.address(), &[]) {
            let _ = release_linux_exec_frames(&mut exec_frames);
            return LINUX_ENOMEM;
        }
    }
    let mut mappings: [LinuxExecMapping; LINUX_EXEC_MAPPING_LIMIT] =
        core::array::from_fn(|index| LinuxExecMapping {
            frame: exec_frames[index].map_or(0, memory::FrameLease::address),
            ..mapping_templates[index]
        });
    let transaction_mapping_count = mapping_count + ancillary_mapping_count;
    if let Some(frame) = exec_frames[mapping_count] {
        mappings[mapping_count] = LinuxExecMapping {
            address: stack_base as usize,
            frame: frame.address(),
            writable: true,
            executable: false,
        };
    }
    if let Some(frame) = exec_frames[mapping_count + 1] {
        mappings[mapping_count + 1] = LinuxExecMapping {
            address: heap_base as usize,
            frame: frame.address(),
            writable: true,
            executable: false,
        };
    }
    if interpreted {
        if let Some((_, thread_pointers)) = interpreted_tls {
            for (offset, thread_pointer) in thread_pointers.into_iter().enumerate() {
                if let Some(frame) = exec_frames[mapping_count + 2 + offset] {
                    mappings[mapping_count + 2 + offset] = LinuxExecMapping {
                        address: (thread_pointer & !(PAGE_SIZE as u64 - 1)) as usize,
                        frame: frame.address(),
                        writable: true,
                        executable: false,
                    };
                }
            }
        }
        if let Some(frame) = exec_frames[mapping_count + 4] {
            mappings[mapping_count + 4] = LinuxExecMapping {
                address: ELF_PROBE_SIBLING_STACK.load(Ordering::SeqCst) as usize,
                frame: frame.address(),
                writable: true,
                executable: false,
            };
        }
    }
    let old_mapping_count = active_image_count + ancillary_mapping_count;
    active_image_mappings[active_image_count] = LinuxActiveImageMapping {
        address: stack_base as usize,
        frame: old_stack_frame,
        writable: true,
        executable: false,
    };
    let Some(old_heap_frame) = memory::FrameLease::from_parts(
        ELF_PROBE_HEAP_FRAME.load(Ordering::SeqCst),
        ELF_PROBE_HEAP_GENERATION.load(Ordering::SeqCst),
    ) else {
        let _ = release_linux_exec_frames(&mut exec_frames);
        return LINUX_ENOEXEC;
    };
    if old_heap_frame == old_stack_frame
        || heap_base == 0
        || !address_space::current_linux_mapping_matches(heap_base, old_heap_frame, true, false)
    {
        let _ = release_linux_exec_frames(&mut exec_frames);
        return LINUX_ENOEXEC;
    }
    active_image_mappings[active_image_count + 1] = LinuxActiveImageMapping {
        address: heap_base as usize,
        frame: old_heap_frame,
        writable: true,
        executable: false,
    };
    if let Some((_, thread_pointers)) = interpreted_tls {
        for (index, thread_pointer) in thread_pointers.into_iter().enumerate() {
            let Some(frame) = memory::FrameLease::from_parts(
                ELF_PROBE_TLS_FRAMES[index].load(Ordering::SeqCst),
                ELF_PROBE_TLS_GENERATIONS[index].load(Ordering::SeqCst),
            ) else {
                let _ = release_linux_exec_frames(&mut exec_frames);
                return LINUX_ENOEXEC;
            };
            let address = thread_pointer & !(PAGE_SIZE as u64 - 1);
            if active_image_mappings[..active_image_count + 2 + index]
                .iter()
                .any(|mapping| mapping.frame == frame || mapping.address == address as usize)
                || !address_space::current_linux_mapping_matches(address, frame, true, false)
            {
                let _ = release_linux_exec_frames(&mut exec_frames);
                return LINUX_ENOEXEC;
            }
            active_image_mappings[active_image_count + 2 + index] = LinuxActiveImageMapping {
                address: address as usize,
                frame,
                writable: true,
                executable: false,
            };
        }
    }
    if interpreted {
        let Some(frame) = memory::FrameLease::from_parts(
            ELF_PROBE_SIBLING_STACK_FRAME.load(Ordering::SeqCst),
            ELF_PROBE_SIBLING_STACK_GENERATION.load(Ordering::SeqCst),
        ) else {
            let _ = release_linux_exec_frames(&mut exec_frames);
            return LINUX_ENOEXEC;
        };
        let address = ELF_PROBE_SIBLING_STACK.load(Ordering::SeqCst);
        if address == 0
            || !(address as usize).is_multiple_of(PAGE_SIZE)
            || active_image_mappings[..active_image_count + 4]
                .iter()
                .any(|mapping| mapping.frame == frame || mapping.address == address as usize)
            || !address_space::current_linux_mapping_matches(address, frame, true, false)
        {
            let _ = release_linux_exec_frames(&mut exec_frames);
            return LINUX_ENOEXEC;
        }
        active_image_mappings[active_image_count + 4] = LinuxActiveImageMapping {
            address: address as usize,
            frame,
            writable: true,
            executable: false,
        };
    }
    let clear_child_tid = current_linux_clear_child_tid().unwrap_or(0);
    let robust_list = current_linux_robust_list().unwrap_or(0);
    let mut previous_tid = [0_u8; size_of::<u32>()];
    if clear_child_tid == 0
        || robust_list == 0
        || !copy_from_linux_user(clear_child_tid, &mut previous_tid)
        || !copy_to_linux_user(clear_child_tid, &0_u32.to_ne_bytes())
    {
        let _ = release_linux_exec_frames(&mut exec_frames);
        return LINUX_EFAULT;
    }
    if !set_current_linux_clear_child_tid(0) || !set_current_linux_robust_list(0) {
        let _ = set_current_linux_clear_child_tid(clear_child_tid);
        let _ = set_current_linux_robust_list(robust_list);
        let _ = copy_to_linux_user(clear_child_tid, &previous_tid);
        let _ = release_linux_exec_frames(&mut exec_frames);
        return LINUX_ESRCH;
    }
    let image_replaced = if interpreted {
        replace_current_linux_image_with_hierarchy(
            &mappings[..transaction_mapping_count],
            &active_image_mappings[..old_mapping_count],
            6,
        )
    } else {
        replace_current_linux_image_with_hierarchy(
            &mappings[..transaction_mapping_count],
            &active_image_mappings[..old_mapping_count],
            5,
        )
    };
    if !image_replaced {
        let _ = set_current_linux_clear_child_tid(clear_child_tid);
        let _ = set_current_linux_robust_list(robust_list);
        let _ = copy_to_linux_user(clear_child_tid, &previous_tid);
        let _ = release_linux_exec_frames(&mut exec_frames);
        return LINUX_ENOMEM;
    }
    if let Some(frame) = exec_frames[mapping_count] {
        ELF_PROBE_STACK_FRAME.store(frame.address(), Ordering::SeqCst);
        ELF_PROBE_STACK_GENERATION.store(frame.generation(), Ordering::SeqCst);
    }
    if let Some(frame) = exec_frames[mapping_count + 1] {
        ELF_PROBE_HEAP_FRAME.store(frame.address(), Ordering::SeqCst);
        ELF_PROBE_HEAP_GENERATION.store(frame.generation(), Ordering::SeqCst);
    }
    if interpreted {
        for index in 0..2 {
            if let Some(frame) = exec_frames[mapping_count + 2 + index] {
                ELF_PROBE_TLS_FRAMES[index].store(frame.address(), Ordering::SeqCst);
                ELF_PROBE_TLS_GENERATIONS[index].store(frame.generation(), Ordering::SeqCst);
            }
        }
        if let Some(frame) = exec_frames[mapping_count + 4] {
            ELF_PROBE_SIBLING_STACK_FRAME.store(frame.address(), Ordering::SeqCst);
            ELF_PROBE_SIBLING_STACK_GENERATION.store(frame.generation(), Ordering::SeqCst);
        }
    }
    ELF_PROBE_BRK.store(heap_base, Ordering::SeqCst);
    let next_tls = interpreted_tls.map_or(
        LinuxTlsContext {
            thread_pointer: 0,
            extent: 0,
            extent_size: 0,
        },
        |(initial, _)| initial,
    );
    if !install_linux_tls_context(next_tls) {
        return LINUX_ENOMEM;
    }
    if interpreted {
        serial_write("ginux: interpreted Linux exec image committed\r\n");
    }
    for (index, mapping) in active_image_mappings[..active_image_count]
        .iter()
        .enumerate()
    {
        ELF_PROBE_RETIRED_IMAGE_ADDRESSES[index].store(mapping.address as u64, Ordering::SeqCst);
        ELF_PROBE_RETIRED_IMAGE_FRAMES[index].store(mapping.frame.address(), Ordering::SeqCst);
        ELF_PROBE_RETIRED_IMAGE_GENERATIONS[index]
            .store(mapping.frame.generation(), Ordering::SeqCst);
        ELF_PROBE_RETIRED_IMAGE_FLAGS[index].store(
            u8::from(mapping.writable) | (u8::from(mapping.executable) << 1),
            Ordering::SeqCst,
        );
    }
    ELF_PROBE_RETIRED_IMAGE_COUNT.store(active_image_count as u8, Ordering::SeqCst);
    if interpreted {
        ELF_PROBE_MAIN_MAPPINGS.store(0, Ordering::SeqCst);
        for index in 0..LINUX_MAIN_MAPPING_COUNT {
            ELF_PROBE_MAIN_FRAMES[index].store(0, Ordering::SeqCst);
            ELF_PROBE_MAIN_GENERATIONS[index].store(0, Ordering::SeqCst);
        }
        for dependency in 0..LINUX_DEPENDENCY_COUNT {
            ELF_PROBE_DEPENDENCY_MAPPINGS[dependency].store(0, Ordering::SeqCst);
            ELF_PROBE_DEPENDENCY_SELECTED_BASE[dependency].store(0, Ordering::SeqCst);
            for index in 0..LINUX_DEPENDENCY_MAPPING_COUNT {
                let state = dependency * LINUX_DEPENDENCY_MAPPING_COUNT + index;
                ELF_PROBE_DEPENDENCY_FRAMES[state].store(0, Ordering::SeqCst);
                ELF_PROBE_DEPENDENCY_GENERATIONS[state].store(0, Ordering::SeqCst);
            }
        }
        ELF_PROBE_READ_ONLY_DATA_PAGE.store(0, Ordering::SeqCst);
    }
    for descriptor in 0..LINUX_AUX_DESCRIPTOR_LIMIT {
        if close_on_exec & (1 << descriptor) != 0 {
            let _ = linux_descriptor_close(descriptor);
        }
    }
    let ignored_signals = (0..LINUX_SIGNAL_COUNT).fold(0_u64, |mask, signal| {
        mask | (u64::from(ELF_PROBE_SIGNAL_HANDLERS[signal].load(Ordering::SeqCst) == 1) << signal)
    });
    ELF_PROBE_EXEC_IGNORED_SIGNALS.store(ignored_signals, Ordering::SeqCst);
    for signal in 0..LINUX_SIGNAL_COUNT {
        if ELF_PROBE_SIGNAL_HANDLERS[signal].load(Ordering::SeqCst) != 1 {
            ELF_PROBE_SIGNAL_HANDLERS[signal].store(0, Ordering::SeqCst);
            ELF_PROBE_SIGNAL_FLAGS[signal].store(0, Ordering::SeqCst);
            ELF_PROBE_SIGNAL_RESTORERS[signal].store(0, Ordering::SeqCst);
            ELF_PROBE_SIGNAL_ACTION_MASKS[signal].store(0, Ordering::SeqCst);
        }
    }
    ELF_PROBE_ALT_STACK_POINTER.store(0, Ordering::SeqCst);
    ELF_PROBE_ALT_STACK_SIZE.store(0, Ordering::SeqCst);
    ELF_PROBE_ALT_STACK_FLAGS.store(LINUX_SS_DISABLE, Ordering::SeqCst);
    ELF_PROBE_PROCESS_NAME_LOW.store(process_name[0], Ordering::SeqCst);
    ELF_PROBE_PROCESS_NAME_HIGH.store(process_name[1], Ordering::SeqCst);
    ELF_PROBE_EXITED_CLEAR_CHILD_TID.store(clear_child_tid, Ordering::SeqCst);
    ELF_PROBE_EXITED_ROBUST_LIST.store(robust_list, Ordering::SeqCst);
    let runtime_plan = interpreted_exec_plan
        .as_ref()
        .map_or(exec_plan, |plan| &plan.process.main);
    ELF_PROBE_CODE_BASE.store(runtime_plan.executable.virtual_page, Ordering::SeqCst);
    ELF_PROBE_CODE_SIZE.store(
        (runtime_plan.executable.page_count * PAGE_SIZE) as u64,
        Ordering::SeqCst,
    );
    ELF_PROBE_LOADER_CODE_BASE.store(
        if interpreted {
            exec_plan.executable.virtual_page
        } else {
            0
        },
        Ordering::SeqCst,
    );
    ELF_PROBE_LOADER_CODE_SIZE.store(
        if interpreted {
            (exec_plan.executable.page_count * PAGE_SIZE) as u64
        } else {
            0
        },
        Ordering::SeqCst,
    );
    ELF_PROBE_DATA.store(runtime_plan.writable.virtual_page, Ordering::SeqCst);
    ELF_PROBE_DATA_SIZE.store(
        (runtime_plan.writable.page_count * PAGE_SIZE) as u64,
        Ordering::SeqCst,
    );
    ELF_PROBE_RELRO.store(
        runtime_plan.relro.map_or(0, |relro| relro.virtual_address),
        Ordering::SeqCst,
    );
    ELF_PROBE_RELRO_SIZE.store(
        runtime_plan.relro.map_or(0, |relro| relro.memory_size),
        Ordering::SeqCst,
    );
    unsafe {
        LINUX_USER_RIP = entry;
        LINUX_USER_RSP = new_stack;
    }
    ELF_PROBE_EXEC_NODE.store(exec_node, Ordering::SeqCst);
    ELF_PROBE_EXEC_ENTRY.store(entry, Ordering::SeqCst);
    for (index, (frame, mapping)) in exec_frames[..mapping_count]
        .iter()
        .flatten()
        .zip(&mappings[..mapping_count])
        .enumerate()
    {
        ELF_PROBE_EXEC_MAPPING_ADDRESSES[index].store(mapping.address as u64, Ordering::SeqCst);
        ELF_PROBE_EXEC_MAPPING_FRAMES[index].store(frame.address(), Ordering::SeqCst);
        ELF_PROBE_EXEC_MAPPING_GENERATIONS[index].store(frame.generation(), Ordering::SeqCst);
        ELF_PROBE_EXEC_MAPPING_FLAGS[index].store(
            u8::from(mapping.writable) | (u8::from(mapping.executable) << 1),
            Ordering::SeqCst,
        );
        ELF_PROBE_ACTIVE_IMAGE_ADDRESSES[index].store(mapping.address as u64, Ordering::SeqCst);
        ELF_PROBE_ACTIVE_IMAGE_FRAMES[index].store(frame.address(), Ordering::SeqCst);
        ELF_PROBE_ACTIVE_IMAGE_GENERATIONS[index].store(frame.generation(), Ordering::SeqCst);
        ELF_PROBE_ACTIVE_IMAGE_FLAGS[index].store(
            u8::from(mapping.writable) | (u8::from(mapping.executable) << 1),
            Ordering::SeqCst,
        );
    }
    for index in mapping_count..LINUX_ACTIVE_IMAGE_MAPPING_LIMIT {
        ELF_PROBE_ACTIVE_IMAGE_ADDRESSES[index].store(0, Ordering::SeqCst);
        ELF_PROBE_ACTIVE_IMAGE_FRAMES[index].store(0, Ordering::SeqCst);
        ELF_PROBE_ACTIVE_IMAGE_GENERATIONS[index].store(0, Ordering::SeqCst);
        ELF_PROBE_ACTIVE_IMAGE_FLAGS[index].store(0, Ordering::SeqCst);
    }
    ELF_PROBE_ACTIVE_IMAGE_COUNT.store(mapping_count as u16, Ordering::SeqCst);
    ELF_PROBE_EXEC_REPLACED.store(true, Ordering::SeqCst);
    serial_write("ginux: measured Linux exec image replaced atomically\r\n");
    0
}

fn linux_root_directory_inode(kind: u16) -> Option<u64> {
    let role = kind & (LINUX_DEVICE_DIRECTORY | LINUX_FILE_MASK);
    if role == LINUX_DEVICE_DIRECTORY {
        return Some(4);
    }
    if role == LINUX_ROOT_DIRECTORY {
        return Some(11);
    }
    if role == LINUX_PROC_PID1_DIRECTORY {
        return match kind & LINUX_ROOT_NODE_MASK {
            0 => Some(16),
            LINUX_PROC_PID2_DIRECTORY_NODE => Some(23),
            _ => None,
        };
    }
    let node = (kind & LINUX_ROOT_NODE_MASK) >> LINUX_ROOT_NODE_SHIFT;
    (role == LINUX_ROOT_MEASURED_DIRECTORY && node != 0).then_some(64 + u64::from(node))
}

fn linux_measured_directory_path(kind: u16) -> Option<&'static [u8]> {
    if !linux_mount::current_root_is_read_only() {
        return None;
    }
    let role = kind & (LINUX_DEVICE_DIRECTORY | LINUX_FILE_MASK);
    if role == LINUX_ROOT_DIRECTORY {
        return Some(b"");
    }
    if role == LINUX_PROC_PID1_DIRECTORY {
        return match kind & LINUX_ROOT_NODE_MASK {
            0 => Some(b"proc/1"),
            LINUX_PROC_PID2_DIRECTORY_NODE => Some(b"proc/2"),
            _ => None,
        };
    }
    let node = ((kind & LINUX_ROOT_NODE_MASK) >> LINUX_ROOT_NODE_SHIFT) as u8;
    (role == LINUX_ROOT_MEASURED_DIRECTORY)
        .then(|| initramfs::published_directory_by_node(node))
        .flatten()
}

fn linux_relative_path_kind(directory: u64, path: &[u8]) -> Result<Option<u16>, u64> {
    if path.starts_with(b"/") {
        return Ok(None);
    }
    if !linux_normalized_relative_path(path) {
        return Err(LINUX_ENOENT);
    }
    let state = if directory as u32 == LINUX_AT_FDCWD {
        ELF_PROBE_CWD.load(Ordering::SeqCst)
    } else {
        let state = linux_device_descriptor(directory);
        if state == 0 {
            return Err(LINUX_EBADF);
        }
        state
    };
    if state & LINUX_DEVICE_DIRECTORY == 0 {
        return Err(LINUX_ENOTDIR);
    }
    let Some(parent) = linux_measured_directory_path(state) else {
        return Err(LINUX_ENOENT);
    };
    let mut measured_path = [0_u8; 64];
    let Some(output) = measured_path.get_mut(..parent.len()) else {
        return Err(LINUX_ENOENT);
    };
    output.copy_from_slice(parent);
    let mut length = parent.len();
    if !parent.is_empty() {
        measured_path[length] = b'/';
        length += 1;
    }
    let Some(output) = measured_path.get_mut(length..length + path.len()) else {
        return Err(LINUX_ENOENT);
    };
    output.copy_from_slice(path);
    length += path.len();
    linux_measured_path_kind(&measured_path[..length])
        .map(Some)
        .ok_or(LINUX_ENOENT)
}

fn linux_oci_path_at(
    directory: u64,
    path: &[u8],
) -> Option<(u16, Option<ginux_oci::Entry<'static>>)> {
    if !LINUX_RUNTIME_MODE.load(Ordering::SeqCst) {
        return None;
    }
    let directory = if path.starts_with(b"/") {
        crate::oci_image::ROOT_HANDLE
    } else if directory as u32 == LINUX_AT_FDCWD {
        ELF_PROBE_OCI_CWD.load(Ordering::SeqCst)
    } else {
        linux_oci_descriptor(directory)
    };
    (directory != 0)
        .then(|| crate::oci_image::lookup_at(directory, path))
        .flatten()
}

fn linux_oci_metadata_path_at(
    directory: u64,
    path: &[u8],
) -> Option<(u16, Option<ginux_oci::Entry<'static>>)> {
    if !LINUX_RUNTIME_MODE.load(Ordering::SeqCst) {
        return None;
    }
    let directory = if path.starts_with(b"/") {
        crate::oci_image::ROOT_HANDLE
    } else if directory as u32 == LINUX_AT_FDCWD {
        ELF_PROBE_OCI_CWD.load(Ordering::SeqCst)
    } else {
        linux_oci_descriptor(directory)
    };
    (directory != 0)
        .then(|| crate::oci_image::lookup_metadata_at(directory, path))
        .flatten()
}

fn linux_oci_descriptor(descriptor: u64) -> u16 {
    if let Some(handle) = usize::try_from(descriptor)
        .ok()
        .and_then(|index| ELF_PROBE_STDIO_OCI_DESCRIPTORS.get(index))
    {
        return handle.load(Ordering::SeqCst);
    }
    descriptor
        .checked_sub(LINUX_AUX_DESCRIPTOR_FIRST)
        .and_then(|index| ELF_PROBE_OCI_DESCRIPTORS.get(index as usize))
        .map(|handle| handle.load(Ordering::SeqCst))
        .unwrap_or(0)
}

fn linux_oci_entry(descriptor: u64) -> Option<ginux_oci::Entry<'static>> {
    crate::oci_image::entry(linux_oci_descriptor(descriptor))
}

fn linux_oci_inode(handle: u16) -> u64 {
    1024 + u64::from(handle)
}

fn linux_oci_mode(entry: ginux_oci::Entry<'_>) -> u32 {
    let kind = match entry.kind {
        ginux_oci::EntryKind::File => 0o100000,
        ginux_oci::EntryKind::Hardlink => 0o100000,
        ginux_oci::EntryKind::Directory => 0o040000,
        ginux_oci::EntryKind::Symlink => 0o120000,
    };
    kind | entry.mode & 0o7777
}

fn linux_oci_stat(handle: u16, entry: Option<ginux_oci::Entry<'_>>) -> [u8; LINUX_STAT_SIZE] {
    let mode = entry.map_or(LINUX_STAT_DIRECTORY_MODE, linux_oci_mode);
    let mut stat = linux_stat(mode, linux_oci_inode(handle));
    let size = entry.map_or(0, |entry| entry.data.len() as u64);
    stat[48..56].copy_from_slice(&size.to_ne_bytes());
    stat[64..72].copy_from_slice(&size.div_ceil(512).to_ne_bytes());
    stat
}

fn linux_openat(directory: u64, path_address: u64, flags: u64) -> u64 {
    let mut path = [0_u8; 256];
    let path_length = match copy_linux_c_string(path_address, &mut path) {
        Ok(length) => length,
        Err(error) => return error,
    };
    let path = &path[..path_length];
    let access = flags & 3;
    if access == 3
        || flags
            & !(3
                | LINUX_O_CLOEXEC
                | LINUX_O_DIRECTORY
                | LINUX_O_CREAT
                | LINUX_O_EXCL
                | LINUX_O_LARGEFILE)
            != 0
        || flags & LINUX_O_EXCL != 0 && flags & LINUX_O_CREAT == 0
    {
        return LINUX_EINVAL;
    }
    let tmp_path = path == b"/tmp/ginux-ready";
    if tmp_path && flags & LINUX_O_DIRECTORY != 0 {
        return LINUX_ENOTDIR;
    }
    if flags & LINUX_O_CREAT != 0 && !tmp_path {
        return LINUX_EACCES;
    }
    let mut tmp_handle = None;
    let mut tmp_created = false;
    let mut oci_handle = 0;
    let kind = if tmp_path {
        let existing = linux_tmpfs::open(b"ginux-ready");
        if existing.is_some()
            && flags & (LINUX_O_CREAT | LINUX_O_EXCL) == (LINUX_O_CREAT | LINUX_O_EXCL)
        {
            return LINUX_EEXIST;
        }
        tmp_handle = existing.or_else(|| {
            let created = (flags & LINUX_O_CREAT != 0)
                .then(|| linux_tmpfs::create(b"ginux-ready"))
                .flatten();
            tmp_created = created.is_some();
            created
        });
        if tmp_handle.is_none() {
            return LINUX_ENOENT;
        }
        LINUX_FILE_TMP_SCRATCH
    } else if let Some(kind) = linux_virtual_mount_path_at(directory, path) {
        kind
    } else if let Some((handle, entry)) = linux_oci_path_at(directory, path) {
        oci_handle = handle;
        match entry.map(|entry| entry.kind) {
            None | Some(ginux_oci::EntryKind::Directory) => LINUX_ROOT_MEASURED_DIRECTORY,
            Some(ginux_oci::EntryKind::File) => LINUX_FILE_ROOT_GENERIC,
            Some(ginux_oci::EntryKind::Hardlink) => LINUX_FILE_ROOT_GENERIC,
            Some(ginux_oci::EntryKind::Symlink) => return LINUX_EINVAL,
        }
    } else {
        match linux_relative_path_kind(directory, path) {
            Ok(Some(kind)) => kind,
            Ok(None) => match path {
                b"/dev/null" => 0,
                b"/dev/zero" => LINUX_DEVICE_ZERO,
                b"/dev/console" | b"/dev/tty" => LINUX_DEVICE_CONSOLE,
                path => match linux_root_directory_kind(path)
                    .or_else(|| linux_runtime_file_kind(path))
                {
                    Some(kind) => kind,
                    None => return LINUX_ENOENT,
                },
            },
            Err(error) => return error,
        }
    };
    if kind & LINUX_DEVICE_DIRECTORY != 0 && access != 0 {
        return LINUX_EISDIR;
    }
    if kind & LINUX_DEVICE_DIRECTORY == 0 && flags & LINUX_O_DIRECTORY != 0 {
        return LINUX_ENOTDIR;
    }
    if kind & LINUX_FILE_MASK != 0 && access != 0 && !linux_tmp_file(kind) {
        return LINUX_EACCES;
    }
    let state = kind | (access + 1) as u16;
    let Some(descriptor) = (LINUX_AUX_DESCRIPTOR_FIRST..LINUX_AUX_DESCRIPTOR_LIMIT)
        .find(|descriptor| linux_aux_descriptor_free(*descriptor))
    else {
        if tmp_created {
            let _ = tmp_handle.is_some_and(|handle| linux_tmpfs::unlink(b"ginux-ready", handle));
        }
        return LINUX_EMFILE;
    };
    ELF_PROBE_DEVICE_DESCRIPTORS[(descriptor - LINUX_AUX_DESCRIPTOR_FIRST) as usize]
        .store(state, Ordering::SeqCst);
    ELF_PROBE_OCI_DESCRIPTORS[(descriptor - LINUX_AUX_DESCRIPTOR_FIRST) as usize]
        .store(oci_handle, Ordering::SeqCst);
    ELF_PROBE_TMP_FILE_GENERATIONS[(descriptor - LINUX_AUX_DESCRIPTOR_FIRST) as usize].store(
        tmp_handle.map_or(0, linux_tmpfs::generation),
        Ordering::SeqCst,
    );
    if flags & LINUX_O_CLOEXEC != 0 {
        ELF_PROBE_CONSOLE_CLOEXEC.fetch_or(1 << descriptor, Ordering::SeqCst);
    } else {
        ELF_PROBE_CONSOLE_CLOEXEC.fetch_and(!(1 << descriptor), Ordering::SeqCst);
    }
    linux_descriptor_offset(descriptor).store(0, Ordering::SeqCst);
    descriptor
}

fn linux_newfstatat(directory: u64, path_address: u64, stat_address: u64, flags: u64) -> u64 {
    if flags & !LINUX_AT_SYMLINK_NOFOLLOW != 0 {
        return LINUX_EINVAL;
    }
    let mut path = [0_u8; 256];
    let path_length = match copy_linux_c_string(path_address, &mut path) {
        Ok(length) => length,
        Err(error) => return error,
    };
    let path = &path[..path_length];
    if let Some(kind) = linux_virtual_mount_path_at(directory, path) {
        let stat = if kind & LINUX_DEVICE_DIRECTORY != 0 {
            linux_stat(
                LINUX_STAT_DIRECTORY_MODE,
                linux_root_directory_inode(kind).unwrap_or(0),
            )
        } else {
            linux_regular_file_stat(kind)
        };
        return if copy_to_linux_user(stat_address, &stat) {
            0
        } else {
            LINUX_EFAULT
        };
    }
    {
        let oci = if flags & LINUX_AT_SYMLINK_NOFOLLOW != 0 {
            linux_oci_metadata_path_at(directory, path)
        } else {
            linux_oci_path_at(directory, path)
        };
        if let Some((handle, entry)) = oci {
            let stat = linux_oci_stat(handle, entry);
            return if copy_to_linux_user(stat_address, &stat) {
                0
            } else {
                LINUX_EFAULT
            };
        }
    }
    let relative_kind = match linux_relative_path_kind(directory, path) {
        Ok(kind) => kind,
        Err(error) => return error,
    };
    let stat = match (relative_kind, path) {
        (Some(kind), _) if kind & LINUX_DEVICE_DIRECTORY != 0 => linux_stat(
            LINUX_STAT_DIRECTORY_MODE,
            linux_root_directory_inode(kind).unwrap_or(0),
        ),
        (Some(kind), _) => linux_regular_file_stat(kind),
        (None, b"/dev/null" | b"/dev/zero" | b"/dev/console" | b"/dev/tty") => {
            linux_stat(LINUX_STAT_CHARACTER_DEVICE_MODE, 1)
        }
        (None, path) if linux_root_directory_kind(path).is_some() => linux_stat(
            LINUX_STAT_DIRECTORY_MODE,
            linux_root_directory_inode(linux_root_directory_kind(path).unwrap_or(0)).unwrap_or(0),
        ),
        (None, path) => match linux_runtime_file_kind(path) {
            Some(kind) => linux_regular_file_stat(kind),
            None => return LINUX_ENOENT,
        },
    };
    if !copy_to_linux_user(stat_address, &stat) {
        return LINUX_EFAULT;
    }
    0
}

fn linux_unlinkat(directory: u64, path_address: u64, flags: u64) -> u64 {
    if flags != 0 {
        return LINUX_EINVAL;
    }
    let mut path = [0_u8; 32];
    let path_length = match copy_linux_c_string(path_address, &mut path) {
        Ok(length) => length,
        Err(error) => return error,
    };
    let path = &path[..path_length];
    if directory as u32 != LINUX_AT_FDCWD || path != b"/tmp/ginux-ready" {
        return LINUX_ENOENT;
    }
    let Some(handle) = linux_tmpfs::open(b"ginux-ready") else {
        return LINUX_ENOENT;
    };
    let generation = linux_tmpfs::generation(handle);
    if (LINUX_AUX_DESCRIPTOR_FIRST..LINUX_AUX_DESCRIPTOR_LIMIT).any(|descriptor| {
        linux_tmp_file(linux_device_descriptor(descriptor))
            && ELF_PROBE_TMP_FILE_GENERATIONS[(descriptor - LINUX_AUX_DESCRIPTOR_FIRST) as usize]
                .load(Ordering::SeqCst)
                == generation
    }) {
        return LINUX_EBUSY;
    }
    if linux_tmpfs::unlink(b"ginux-ready", handle) {
        0
    } else {
        LINUX_ENOENT
    }
}

fn linux_faccessat(directory: u64, path_address: u64, mode: u64, flags: u64) -> u64 {
    if flags != 0 || mode & !7 != 0 {
        return LINUX_EINVAL;
    }
    let mut path = [0_u8; 256];
    let path_length = match copy_linux_c_string(path_address, &mut path) {
        Ok(length) => length,
        Err(error) => return error,
    };
    let path = &path[..path_length];
    if let Some(kind) = linux_virtual_mount_path_at(directory, path) {
        let denied = if kind & LINUX_DEVICE_DIRECTORY != 0 {
            mode & 2 != 0
        } else {
            mode & 3 != 0
        };
        return if denied { LINUX_EACCES } else { 0 };
    }
    {
        if let Some((_, entry)) = linux_oci_path_at(directory, path) {
            let permissions = entry.map_or(0o555, |entry| entry.mode);
            let denied = mode & 4 != 0 && permissions & 0o444 == 0
                || mode & 2 != 0 && permissions & 0o222 == 0
                || mode & 1 != 0 && permissions & 0o111 == 0;
            return if denied { LINUX_EACCES } else { 0 };
        }
    }
    let relative_kind = match linux_relative_path_kind(directory, path) {
        Ok(kind) => kind,
        Err(error) => return error,
    };
    if let Some(kind) = relative_kind {
        let denied = if linux_tmp_file(kind) {
            mode & 1 != 0
        } else if linux_proc_file(kind) {
            mode & 3 != 0
        } else {
            mode & 2 != 0
        };
        return if denied { LINUX_EACCES } else { 0 };
    }
    let directory = match path {
        b"/dev/null" | b"/dev/zero" | b"/dev/console" | b"/dev/tty" => false,
        path if linux_root_directory_kind(path).is_some() => true,
        path if linux_runtime_file_kind(path).is_some() => {
            let kind = linux_runtime_file_kind(path).unwrap_or(0);
            let denied = if linux_tmp_file(kind) {
                mode & 1 != 0
            } else if linux_proc_file(kind) {
                mode & 3 != 0
            } else {
                mode & 2 != 0
            };
            if denied {
                return LINUX_EACCES;
            }
            return 0;
        }
        _ => return LINUX_ENOENT,
    };
    if !directory && mode & 1 != 0 || directory && mode & 2 != 0 {
        LINUX_EACCES
    } else {
        0
    }
}

fn linux_statx(directory: u64, path_address: u64, flags: u64, mask: u64, stat_address: u64) -> u64 {
    if flags != 0 || mask & u64::from(LINUX_STATX_RESERVED) != 0 {
        return LINUX_EINVAL;
    }
    let mut path = [0_u8; 256];
    let path_length = match copy_linux_c_string(path_address, &mut path) {
        Ok(length) => length,
        Err(error) => return error,
    };
    let path = &path[..path_length];
    if let Some(kind) = linux_virtual_mount_path_at(directory, path) {
        let (inode, mode, size) = if kind & LINUX_DEVICE_DIRECTORY != 0 {
            (
                linux_root_directory_inode(kind).unwrap_or(0),
                LINUX_STAT_DIRECTORY_MODE,
                0,
            )
        } else {
            (
                linux_regular_file_inode(kind).unwrap_or(0),
                linux_regular_file_mode(kind),
                linux_file_size(kind),
            )
        };
        let mut stat = [0_u8; 256];
        stat[..4].copy_from_slice(&LINUX_STATX_BASIC_STATS.to_ne_bytes());
        stat[4..8].copy_from_slice(&(PAGE_SIZE as u32).to_ne_bytes());
        stat[16..20].copy_from_slice(&1_u32.to_ne_bytes());
        stat[28..30].copy_from_slice(&(mode as u16).to_ne_bytes());
        stat[32..40].copy_from_slice(&inode.to_ne_bytes());
        stat[40..48].copy_from_slice(&size.to_ne_bytes());
        stat[48..56].copy_from_slice(&size.div_ceil(512).to_ne_bytes());
        return if copy_to_linux_user(stat_address, &stat) {
            0
        } else {
            LINUX_EFAULT
        };
    }
    {
        if let Some((handle, entry)) = linux_oci_path_at(directory, path) {
            let mode = entry.map_or(LINUX_STAT_DIRECTORY_MODE, linux_oci_mode);
            let size = entry.map_or(0, |entry| entry.data.len() as u64);
            let mut stat = [0_u8; 256];
            stat[..4].copy_from_slice(&LINUX_STATX_BASIC_STATS.to_ne_bytes());
            stat[4..8].copy_from_slice(&(PAGE_SIZE as u32).to_ne_bytes());
            stat[16..20].copy_from_slice(&1_u32.to_ne_bytes());
            stat[28..30].copy_from_slice(&(mode as u16).to_ne_bytes());
            stat[32..40].copy_from_slice(&linux_oci_inode(handle).to_ne_bytes());
            stat[40..48].copy_from_slice(&size.to_ne_bytes());
            stat[48..56].copy_from_slice(&size.div_ceil(512).to_ne_bytes());
            return if copy_to_linux_user(stat_address, &stat) {
                0
            } else {
                LINUX_EFAULT
            };
        }
    }
    let relative_kind = match linux_relative_path_kind(directory, path) {
        Ok(kind) => kind,
        Err(error) => return error,
    };
    let (inode, minor, mode, size) = match (relative_kind, path) {
        (Some(kind), _) if kind & LINUX_DEVICE_DIRECTORY != 0 => (
            linux_root_directory_inode(kind).unwrap_or(0),
            0,
            LINUX_STAT_DIRECTORY_MODE,
            0,
        ),
        (Some(kind), _) => (
            linux_regular_file_inode(kind).unwrap_or(0),
            0,
            linux_regular_file_mode(kind),
            linux_file_size(kind),
        ),
        (None, b"/dev/null") => (1_u64, 3_u32, LINUX_STAT_CHARACTER_DEVICE_MODE, 0),
        (None, b"/dev/zero") => (2, 5, LINUX_STAT_CHARACTER_DEVICE_MODE, 0),
        (None, b"/dev/console") => (3, 1, LINUX_STAT_CHARACTER_DEVICE_MODE, 0),
        (None, b"/dev/tty") => (3, 0, LINUX_STAT_CHARACTER_DEVICE_MODE, 0),
        (None, path) if linux_root_directory_kind(path).is_some() => (
            linux_root_directory_inode(linux_root_directory_kind(path).unwrap_or(0)).unwrap_or(0),
            0,
            LINUX_STAT_DIRECTORY_MODE,
            0,
        ),
        (None, path) => match linux_runtime_file_kind(path) {
            Some(kind) => (
                linux_regular_file_inode(kind).unwrap_or(0),
                0,
                linux_regular_file_mode(kind),
                linux_file_size(kind),
            ),
            None => return LINUX_ENOENT,
        },
    };
    let mut stat = [0_u8; 256];
    stat[..4].copy_from_slice(&LINUX_STATX_BASIC_STATS.to_ne_bytes());
    stat[4..8].copy_from_slice(&(PAGE_SIZE as u32).to_ne_bytes());
    stat[16..20].copy_from_slice(&1_u32.to_ne_bytes());
    stat[28..30].copy_from_slice(&(mode as u16).to_ne_bytes());
    stat[32..40].copy_from_slice(&inode.to_ne_bytes());
    stat[40..48].copy_from_slice(&size.to_ne_bytes());
    stat[48..56].copy_from_slice(&size.div_ceil(512).to_ne_bytes());
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
    if let Some(result) = linux_timerfd_read(descriptor, address, count) {
        return result;
    }
    if let Some(result) = linux_pipe_read(descriptor, address, count) {
        return result;
    }
    if u16::from(linux_console_descriptor(descriptor)) == LINUX_DESCRIPTOR_INPUT {
        if !LINUX_RUNTIME_MODE.load(Ordering::SeqCst) || count == 0 {
            return 0;
        }
        let Some(byte) = container_console_read_byte() else {
            return 0;
        };
        if !copy_to_linux_user(address, &[byte]) {
            return LINUX_EFAULT;
        }
        return 1;
    }
    if let Some(entry) = linux_oci_entry(descriptor) {
        if entry.kind == ginux_oci::EntryKind::Directory {
            return LINUX_EISDIR;
        }
        let state = linux_device_descriptor(descriptor);
        if state & 3 != LINUX_DESCRIPTOR_INPUT {
            return LINUX_EBADF;
        }
        let offset = linux_descriptor_offset(descriptor).load(Ordering::SeqCst);
        let Ok(start) = usize::try_from(offset) else {
            return 0;
        };
        if start >= entry.data.len() || count == 0 {
            return 0;
        }
        let end = start
            .saturating_add(usize::try_from(count).unwrap_or(usize::MAX))
            .min(entry.data.len());
        if !copy_to_linux_user(address, &entry.data[start..end]) {
            return LINUX_EFAULT;
        }
        linux_descriptor_offset(descriptor).store(end as u64, Ordering::SeqCst);
        return (end - start) as u64;
    }
    let device = linux_device_descriptor(descriptor);
    if linux_tmp_file(device) {
        return linux_tmp_read(descriptor, device, address, count);
    }
    if let Some(file) = linux_regular_file(device) {
        if device & 3 != LINUX_DESCRIPTOR_INPUT {
            return LINUX_EBADF;
        }
        let offset = linux_descriptor_offset(descriptor).load(Ordering::SeqCst);
        let Ok(start) = usize::try_from(offset) else {
            return 0;
        };
        if start >= file.len() || count == 0 {
            return 0;
        }
        let requested = usize::try_from(count).unwrap_or(usize::MAX);
        let end = start.saturating_add(requested).min(file.len());
        if !copy_to_linux_user(address, &file[start..end]) {
            return LINUX_EFAULT;
        }
        linux_descriptor_offset(descriptor).store(end as u64, Ordering::SeqCst);
        return (end - start) as u64;
    }
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
    } else {
        LINUX_EBADF
    }
}

fn linux_regular_file_read_at(descriptor: u64, address: u64, count: u64, offset: u64) -> u64 {
    let state = linux_device_descriptor(descriptor);
    if let Some(entry) = linux_oci_entry(descriptor) {
        if state & 3 != LINUX_DESCRIPTOR_INPUT {
            return LINUX_EBADF;
        }
        let Ok(start) = usize::try_from(offset) else {
            return 0;
        };
        if start >= entry.data.len() || count == 0 {
            return 0;
        }
        let end = start
            .saturating_add(usize::try_from(count).unwrap_or(usize::MAX))
            .min(entry.data.len());
        if !copy_to_linux_user(address, &entry.data[start..end]) {
            return LINUX_EFAULT;
        }
        return (end - start) as u64;
    }
    let Some(file) = linux_regular_file(state) else {
        return if linux_descriptor_open(descriptor) {
            LINUX_ESPIPE
        } else {
            LINUX_EBADF
        };
    };
    if state & 3 != LINUX_DESCRIPTOR_INPUT {
        return LINUX_EBADF;
    }
    if (offset as i64) < 0 {
        return LINUX_EINVAL;
    }
    let Ok(start) = usize::try_from(offset) else {
        return 0;
    };
    if start >= file.len() || count == 0 {
        return 0;
    }
    let requested = usize::try_from(count).unwrap_or(usize::MAX);
    let end = start.saturating_add(requested).min(file.len());
    if !copy_to_linux_user(address, &file[start..end]) {
        return LINUX_EFAULT;
    }
    (end - start) as u64
}

fn clear_linux_poll_snapshot(slot: usize) {
    for descriptor in &LINUX_THREAD_POLL_DESCRIPTORS[slot] {
        descriptor.store(0, Ordering::SeqCst);
    }
    for events in &LINUX_THREAD_POLL_EVENTS[slot] {
        events.store(0, Ordering::SeqCst);
    }
}

fn clear_linux_iov_snapshot(slot: usize) {
    for address in &LINUX_THREAD_IOV_ADDRESSES[slot] {
        address.store(0, Ordering::SeqCst);
    }
    for length in &LINUX_THREAD_IOV_LENGTHS[slot] {
        length.store(0, Ordering::SeqCst);
    }
}

fn complete_linux_poll_wait(slot: usize, timed_out: bool) -> bool {
    let kind = if slot < LINUX_THREAD_LIMIT {
        LINUX_THREAD_WAIT_KINDS[slot].load(Ordering::SeqCst)
    } else {
        LINUX_THREAD_WAIT_NONE
    };
    if slot >= LINUX_THREAD_LIMIT
        || LINUX_THREAD_STATES[slot].load(Ordering::SeqCst) != LINUX_THREAD_BLOCKED
        || !matches!(kind, LINUX_THREAD_WAIT_POLL | LINUX_THREAD_WAIT_PPOLL)
    {
        return false;
    }
    let count = LINUX_THREAD_WAIT_OUTPUTS[slot].load(Ordering::SeqCst) as usize;
    if count > LINUX_POLLFD_LIMIT {
        return false;
    }
    let mut pollfds = [0_u8; LINUX_POLLFD_LIMIT * 8];
    let mut ready = 0_u64;
    for index in 0..count {
        let descriptor = LINUX_THREAD_POLL_DESCRIPTORS[slot][index].load(Ordering::SeqCst);
        let events = LINUX_THREAD_POLL_EVENTS[slot][index].load(Ordering::SeqCst);
        let returned = linux_descriptor_poll_events(descriptor as i32, events);
        let offset = index * 8;
        pollfds[offset..offset + 4].copy_from_slice(&descriptor.to_ne_bytes());
        pollfds[offset + 4..offset + 6].copy_from_slice(&events.to_ne_bytes());
        pollfds[offset + 6..offset + 8].copy_from_slice(&returned.to_ne_bytes());
        ready += u64::from(returned != 0);
    }
    if ready == 0 && !timed_out {
        return false;
    }
    let address = LINUX_THREAD_WAIT_KEYS[slot].load(Ordering::SeqCst);
    let result = if count == 0 || copy_to_linux_user(address, &pollfds[..count * 8]) {
        ready
    } else {
        LINUX_EFAULT
    };
    LINUX_THREAD_RAXS[slot].store(result, Ordering::SeqCst);
    LINUX_THREAD_WAIT_KINDS[slot].store(LINUX_THREAD_WAIT_NONE, Ordering::SeqCst);
    LINUX_THREAD_WAIT_KEYS[slot].store(0, Ordering::SeqCst);
    LINUX_THREAD_WAIT_OUTPUTS[slot].store(0, Ordering::SeqCst);
    LINUX_THREAD_WAIT_DEADLINES[slot].store(0, Ordering::SeqCst);
    if kind == LINUX_THREAD_WAIT_PPOLL {
        LINUX_THREAD_SIGNAL_MASKS[slot].store(
            LINUX_THREAD_WAIT_PRIOR_SIGNAL_MASKS[slot].swap(0, Ordering::SeqCst),
            Ordering::SeqCst,
        );
    }
    clear_linux_poll_snapshot(slot);
    LINUX_THREAD_STATES[slot].store(LINUX_THREAD_READY, Ordering::SeqCst);
    match (kind, timed_out) {
        (LINUX_THREAD_WAIT_POLL, true) => {
            LINUX_POLL_WAIT_TIMEOUT_COUNT.fetch_add(1, Ordering::SeqCst);
        }
        (LINUX_THREAD_WAIT_POLL, false) => {
            LINUX_POLL_WAIT_WAKE_COUNT.fetch_add(1, Ordering::SeqCst);
        }
        (LINUX_THREAD_WAIT_PPOLL, true) => {
            LINUX_PPOLL_WAIT_TIMEOUT_COUNT.fetch_add(1, Ordering::SeqCst);
        }
        (LINUX_THREAD_WAIT_PPOLL, false) => {
            LINUX_PPOLL_WAIT_WAKE_COUNT.fetch_add(1, Ordering::SeqCst);
        }
        _ => {}
    }
    true
}

fn wake_linux_poll_waits() -> u32 {
    let mut woken = 0;
    for slot in 0..LINUX_THREAD_LIMIT {
        woken += u32::from(complete_linux_poll_wait(slot, false));
    }
    woken
}

fn linux_poll_live_return(
    output: *mut LinuxSyscallFrame,
    caller: LinuxUserContext,
    address: u64,
    count: u64,
    timeout: u64,
) -> u64 {
    let timeout = timeout as u32 as i32;
    let duration = if timeout < 0 {
        None
    } else {
        let Some(duration) = (timeout as u64).checked_mul(1_000_000) else {
            return LINUX_EINVAL;
        };
        Some(duration)
    };
    linux_poll_wait_live_return(
        output,
        caller,
        address,
        count,
        duration,
        LINUX_THREAD_WAIT_POLL,
        None,
    )
}

fn linux_poll_wait_live_return(
    output: *mut LinuxSyscallFrame,
    caller: LinuxUserContext,
    address: u64,
    count: u64,
    duration: Option<u64>,
    wait_kind: u8,
    temporary_signal_mask: Option<u64>,
) -> u64 {
    let Ok(descriptor_count) = usize::try_from(count) else {
        return LINUX_EINVAL;
    };
    if descriptor_count > LINUX_POLLFD_LIMIT {
        return LINUX_EINVAL;
    }
    let mut pollfds = [0_u8; LINUX_POLLFD_LIMIT * 8];
    let active = &mut pollfds[..descriptor_count * 8];
    if descriptor_count != 0
        && (!copy_from_linux_user(address, active)
            || !linux_writable_user_range_contains(address, active.len()))
    {
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
    if descriptor_count == 0 && duration == Some(0) {
        return 0;
    }
    if ready != 0 || duration == Some(0) {
        return if copy_to_linux_user(address, active) {
            ready
        } else {
            LINUX_EFAULT
        };
    }
    let Some(current) = current_linux_thread_handle() else {
        return LINUX_ESRCH;
    };
    let Some(target) = ready_linux_thread_handle() else {
        return LINUX_ENOSYS;
    };
    let Some(tls) = linux_thread_context(current) else {
        return LINUX_ESRCH;
    };
    let deadline = if let Some(duration) = duration {
        let Some(deadline) = linux_elapsed_nanoseconds().and_then(|now| now.checked_add(duration))
        else {
            return LINUX_EINVAL;
        };
        deadline
    } else {
        0
    };
    let slot = usize::from(current.slot);
    for (index, pollfd) in active.as_chunks::<8>().0.iter().enumerate() {
        LINUX_THREAD_POLL_DESCRIPTORS[slot][index].store(
            u32::from_ne_bytes(pollfd[..4].try_into().unwrap()),
            Ordering::SeqCst,
        );
        LINUX_THREAD_POLL_EVENTS[slot][index].store(
            u16::from_ne_bytes(pollfd[4..6].try_into().unwrap()),
            Ordering::SeqCst,
        );
    }
    if !save_linux_thread_return_context(current, caller)
        || !block_current_linux_thread(wait_kind, address, descriptor_count as u64, deadline)
    {
        clear_linux_poll_snapshot(slot);
        return LINUX_EINVAL;
    }
    let prior_signal_mask = ELF_PROBE_SIGNAL_MASK.load(Ordering::SeqCst);
    if let Some(mask) = temporary_signal_mask {
        LINUX_THREAD_WAIT_PRIOR_SIGNAL_MASKS[slot].store(prior_signal_mask, Ordering::SeqCst);
        LINUX_THREAD_SIGNAL_MASKS[slot].store(mask, Ordering::SeqCst);
    }
    let mut target_registers = LinuxSyscallFrame::ZERO;
    let mut target_rip = 0;
    let mut target_rsp = 0;
    let mut target_rflags = 0;
    let mut target_rax = 0;
    if !install_linux_thread_return(
        target,
        &mut target_registers,
        &mut target_rip,
        &mut target_rsp,
        &mut target_rflags,
        &mut target_rax,
    ) {
        LINUX_THREAD_WAIT_KINDS[slot].store(LINUX_THREAD_WAIT_NONE, Ordering::SeqCst);
        LINUX_THREAD_WAIT_KEYS[slot].store(0, Ordering::SeqCst);
        LINUX_THREAD_WAIT_OUTPUTS[slot].store(0, Ordering::SeqCst);
        LINUX_THREAD_WAIT_DEADLINES[slot].store(0, Ordering::SeqCst);
        LINUX_THREAD_WAIT_PRIOR_SIGNAL_MASKS[slot].store(0, Ordering::SeqCst);
        clear_linux_poll_snapshot(slot);
        LINUX_THREAD_STATES[slot].store(LINUX_THREAD_RUNNING, Ordering::SeqCst);
        LINUX_CURRENT_THREAD_SLOT.store(current.slot, Ordering::SeqCst);
        let _ = install_linux_tls_context(tls);
        if temporary_signal_mask.is_some() {
            store_current_linux_signal_mask(prior_signal_mask);
        }
        return LINUX_EINVAL;
    }
    let selected = LinuxUserContext {
        registers: target_registers,
        rip: target_rip,
        rsp: target_rsp,
        rflags: target_rflags,
        rax: target_rax,
        fs_base: active_fs_base(),
    };
    if !write_linux_live_return(output, selected) {
        return LINUX_EINVAL;
    }
    LINUX_LIVE_RETURN_VALIDATED.store(true, Ordering::SeqCst);
    if wait_kind == LINUX_THREAD_WAIT_PPOLL {
        LINUX_PPOLL_WAIT_BLOCK_COUNT.fetch_add(1, Ordering::SeqCst);
    } else {
        LINUX_POLL_WAIT_BLOCK_COUNT.fetch_add(1, Ordering::SeqCst);
    }
    selected.rax
}

fn linux_ppoll_live_return(
    output: *mut LinuxSyscallFrame,
    caller: LinuxUserContext,
    address: u64,
    count: u64,
    timeout_address: u64,
    signal_mask_address: u64,
    signal_set_size: u64,
) -> u64 {
    let duration = if timeout_address == 0 {
        None
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
        let Some(duration) = (seconds as u64)
            .checked_mul(NANOSECONDS_PER_SECOND)
            .and_then(|seconds| seconds.checked_add(nanoseconds as u64))
        else {
            return LINUX_EINVAL;
        };
        Some(duration)
    };
    let temporary_signal_mask = match linux_temporary_signal_mask(LinuxSignalMaskInput {
        address: signal_mask_address,
        size: signal_set_size,
    }) {
        Ok(mask) => mask,
        Err(error) => return error,
    };
    linux_poll_wait_live_return(
        output,
        caller,
        address,
        count,
        duration,
        LINUX_THREAD_WAIT_PPOLL,
        Some(temporary_signal_mask),
    )
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
    wake_linux_descriptor_waits();
    Some(8)
}

fn complete_linux_eventfd_read_wait(slot: usize) -> bool {
    if slot >= LINUX_THREAD_LIMIT
        || LINUX_THREAD_STATES[slot].load(Ordering::SeqCst) != LINUX_THREAD_BLOCKED
        || LINUX_THREAD_WAIT_KINDS[slot].load(Ordering::SeqCst) != LINUX_THREAD_WAIT_EVENTFD_READ
    {
        return false;
    }
    let descriptor = LINUX_THREAD_WAIT_OUTPUTS[slot].load(Ordering::SeqCst);
    let address = LINUX_THREAD_WAIT_KEYS[slot].load(Ordering::SeqCst);
    let result = linux_eventfd_read(descriptor, address, 8).unwrap_or(LINUX_EBADF);
    if result == LINUX_ENOSYS {
        return false;
    }
    LINUX_THREAD_RAXS[slot].store(result, Ordering::SeqCst);
    LINUX_THREAD_WAIT_KINDS[slot].store(LINUX_THREAD_WAIT_NONE, Ordering::SeqCst);
    LINUX_THREAD_WAIT_KEYS[slot].store(0, Ordering::SeqCst);
    LINUX_THREAD_WAIT_OUTPUTS[slot].store(0, Ordering::SeqCst);
    LINUX_THREAD_WAIT_DEADLINES[slot].store(0, Ordering::SeqCst);
    LINUX_THREAD_STATES[slot].store(LINUX_THREAD_READY, Ordering::SeqCst);
    LINUX_EVENTFD_READ_WAKE_COUNT.fetch_add(1, Ordering::SeqCst);
    true
}

fn wake_linux_eventfd_read_waits() -> u32 {
    let mut woken = 0;
    for slot in 0..LINUX_THREAD_LIMIT {
        woken += u32::from(complete_linux_eventfd_read_wait(slot));
    }
    woken
}

fn complete_linux_eventfd_write_wait(slot: usize) -> bool {
    if slot >= LINUX_THREAD_LIMIT
        || LINUX_THREAD_STATES[slot].load(Ordering::SeqCst) != LINUX_THREAD_BLOCKED
        || LINUX_THREAD_WAIT_KINDS[slot].load(Ordering::SeqCst) != LINUX_THREAD_WAIT_EVENTFD_WRITE
    {
        return false;
    }
    let increment = LINUX_THREAD_WAIT_KEYS[slot].load(Ordering::SeqCst);
    let descriptor = LINUX_THREAD_WAIT_OUTPUTS[slot].load(Ordering::SeqCst);
    let result = linux_eventfd_write_value(descriptor, increment, false).unwrap_or(LINUX_EBADF);
    if result == LINUX_ENOSYS {
        return false;
    }
    LINUX_THREAD_RAXS[slot].store(result, Ordering::SeqCst);
    LINUX_THREAD_WAIT_KINDS[slot].store(LINUX_THREAD_WAIT_NONE, Ordering::SeqCst);
    LINUX_THREAD_WAIT_KEYS[slot].store(0, Ordering::SeqCst);
    LINUX_THREAD_WAIT_OUTPUTS[slot].store(0, Ordering::SeqCst);
    LINUX_THREAD_WAIT_DEADLINES[slot].store(0, Ordering::SeqCst);
    LINUX_THREAD_STATES[slot].store(LINUX_THREAD_READY, Ordering::SeqCst);
    LINUX_EVENTFD_WRITE_WAKE_COUNT.fetch_add(1, Ordering::SeqCst);
    wake_linux_descriptor_waits();
    true
}

fn wake_linux_eventfd_write_waits() -> u32 {
    let mut woken = 0;
    for slot in 0..LINUX_THREAD_LIMIT {
        woken += u32::from(complete_linux_eventfd_write_wait(slot));
    }
    woken
}

fn complete_linux_pipe_read_wait(slot: usize) -> bool {
    if slot >= LINUX_THREAD_LIMIT
        || LINUX_THREAD_STATES[slot].load(Ordering::SeqCst) != LINUX_THREAD_BLOCKED
        || LINUX_THREAD_WAIT_KINDS[slot].load(Ordering::SeqCst) != LINUX_THREAD_WAIT_PIPE_READ
    {
        return false;
    }
    let packed = LINUX_THREAD_WAIT_OUTPUTS[slot].load(Ordering::SeqCst);
    let descriptor = packed & 0xff;
    let count = (packed & !LINUX_PIPE_READV_WAIT_FLAG) >> 8;
    let address = LINUX_THREAD_WAIT_KEYS[slot].load(Ordering::SeqCst);
    let result = if packed & LINUX_PIPE_READV_WAIT_FLAG != 0 {
        let Ok(vector_count) = usize::try_from(count) else {
            return false;
        };
        if !(1..=LINUX_IOV_LIMIT).contains(&vector_count) {
            return false;
        }
        let mut ranges = [(0_u64, 0_usize); LINUX_IOV_LIMIT];
        for (index, range) in ranges[..vector_count].iter_mut().enumerate() {
            let Ok(length) =
                usize::try_from(LINUX_THREAD_IOV_LENGTHS[slot][index].load(Ordering::SeqCst))
            else {
                return false;
            };
            *range = (
                LINUX_THREAD_IOV_ADDRESSES[slot][index].load(Ordering::SeqCst),
                length,
            );
        }
        linux_pipe_readv_ranges(descriptor, &ranges[..vector_count])
    } else {
        linux_pipe_read(descriptor, address, count).unwrap_or(LINUX_EBADF)
    };
    if result == LINUX_ENOSYS {
        return false;
    }
    LINUX_THREAD_RAXS[slot].store(result, Ordering::SeqCst);
    LINUX_THREAD_WAIT_KINDS[slot].store(LINUX_THREAD_WAIT_NONE, Ordering::SeqCst);
    LINUX_THREAD_WAIT_KEYS[slot].store(0, Ordering::SeqCst);
    LINUX_THREAD_WAIT_OUTPUTS[slot].store(0, Ordering::SeqCst);
    LINUX_THREAD_WAIT_DEADLINES[slot].store(0, Ordering::SeqCst);
    clear_linux_iov_snapshot(slot);
    LINUX_THREAD_STATES[slot].store(LINUX_THREAD_READY, Ordering::SeqCst);
    LINUX_PIPE_READ_WAKE_COUNT.fetch_add(1, Ordering::SeqCst);
    true
}

fn wake_linux_pipe_read_waits() -> u32 {
    let mut woken = 0;
    for slot in 0..LINUX_THREAD_LIMIT {
        woken += u32::from(complete_linux_pipe_read_wait(slot));
    }
    woken
}

fn complete_linux_pipe_write_wait(slot: usize) -> bool {
    if slot >= LINUX_THREAD_LIMIT
        || LINUX_THREAD_STATES[slot].load(Ordering::SeqCst) != LINUX_THREAD_BLOCKED
        || LINUX_THREAD_WAIT_KINDS[slot].load(Ordering::SeqCst) != LINUX_THREAD_WAIT_PIPE_WRITE
    {
        return false;
    }
    let packed = LINUX_THREAD_WAIT_OUTPUTS[slot].load(Ordering::SeqCst);
    let descriptor = packed & 0xff;
    let count = (packed & !LINUX_PIPE_WRITEV_WAIT_FLAG) >> 8;
    let address = LINUX_THREAD_WAIT_KEYS[slot].load(Ordering::SeqCst);
    let result = if packed & LINUX_PIPE_WRITEV_WAIT_FLAG != 0 {
        let Ok(vector_count) = usize::try_from(count) else {
            return false;
        };
        if !(1..=LINUX_IOV_LIMIT).contains(&vector_count) {
            return false;
        }
        let mut ranges = [(0_u64, 0_usize); LINUX_IOV_LIMIT];
        for (index, range) in ranges[..vector_count].iter_mut().enumerate() {
            let Ok(length) =
                usize::try_from(LINUX_THREAD_IOV_LENGTHS[slot][index].load(Ordering::SeqCst))
            else {
                return false;
            };
            *range = (
                LINUX_THREAD_IOV_ADDRESSES[slot][index].load(Ordering::SeqCst),
                length,
            );
        }
        linux_pipe_writev_ranges(descriptor, &ranges[..vector_count])
    } else {
        linux_pipe_write(descriptor, address, count).unwrap_or(LINUX_EBADF)
    };
    if result == LINUX_ENOSYS {
        return false;
    }
    LINUX_THREAD_RAXS[slot].store(result, Ordering::SeqCst);
    LINUX_THREAD_WAIT_KINDS[slot].store(LINUX_THREAD_WAIT_NONE, Ordering::SeqCst);
    LINUX_THREAD_WAIT_KEYS[slot].store(0, Ordering::SeqCst);
    LINUX_THREAD_WAIT_OUTPUTS[slot].store(0, Ordering::SeqCst);
    LINUX_THREAD_WAIT_DEADLINES[slot].store(0, Ordering::SeqCst);
    clear_linux_iov_snapshot(slot);
    LINUX_THREAD_STATES[slot].store(LINUX_THREAD_READY, Ordering::SeqCst);
    LINUX_PIPE_WRITE_WAKE_COUNT.fetch_add(1, Ordering::SeqCst);
    true
}

fn wake_linux_pipe_write_waits() -> u32 {
    let mut woken = 0;
    for slot in 0..LINUX_THREAD_LIMIT {
        woken += u32::from(complete_linux_pipe_write_wait(slot));
    }
    woken
}

fn blocked_linux_pipe_writer() -> Option<LinuxThreadHandle> {
    (0..LINUX_THREAD_LIMIT).find_map(|slot| {
        (LINUX_THREAD_STATES[slot].load(Ordering::SeqCst) == LINUX_THREAD_BLOCKED
            && LINUX_THREAD_WAIT_KINDS[slot].load(Ordering::SeqCst) == LINUX_THREAD_WAIT_PIPE_WRITE)
            .then(|| linux_thread_handle(slot))
            .flatten()
    })
}

fn complete_linux_timerfd_read_wait(slot: usize) -> bool {
    if slot >= LINUX_THREAD_LIMIT
        || LINUX_THREAD_STATES[slot].load(Ordering::SeqCst) != LINUX_THREAD_BLOCKED
        || LINUX_THREAD_WAIT_KINDS[slot].load(Ordering::SeqCst) != LINUX_THREAD_WAIT_TIMERFD_READ
    {
        return false;
    }
    let descriptor = LINUX_THREAD_WAIT_OUTPUTS[slot].load(Ordering::SeqCst);
    let address = LINUX_THREAD_WAIT_KEYS[slot].load(Ordering::SeqCst);
    let result = linux_timerfd_read(descriptor, address, 8).unwrap_or(LINUX_EBADF);
    if result == LINUX_ENOSYS {
        LINUX_THREAD_WAIT_DEADLINES[slot].store(
            ELF_PROBE_TIMERFD_DEADLINE.load(Ordering::SeqCst),
            Ordering::SeqCst,
        );
        return false;
    }
    LINUX_THREAD_RAXS[slot].store(result, Ordering::SeqCst);
    LINUX_THREAD_WAIT_KINDS[slot].store(LINUX_THREAD_WAIT_NONE, Ordering::SeqCst);
    LINUX_THREAD_WAIT_KEYS[slot].store(0, Ordering::SeqCst);
    LINUX_THREAD_WAIT_OUTPUTS[slot].store(0, Ordering::SeqCst);
    LINUX_THREAD_WAIT_DEADLINES[slot].store(0, Ordering::SeqCst);
    LINUX_THREAD_STATES[slot].store(LINUX_THREAD_READY, Ordering::SeqCst);
    LINUX_TIMERFD_READ_WAKE_COUNT.fetch_add(1, Ordering::SeqCst);
    true
}

fn wake_linux_timerfd_read_waits() -> u32 {
    let mut woken = 0;
    for slot in 0..LINUX_THREAD_LIMIT {
        woken += u32::from(complete_linux_timerfd_read_wait(slot));
    }
    woken
}

fn linux_descriptor_read_live_return(
    output: *mut LinuxSyscallFrame,
    caller: LinuxUserContext,
    descriptor: u64,
    address: u64,
    count: u64,
) -> u64 {
    let immediate = linux_descriptor_read(descriptor, address, count);
    if LINUX_RUNTIME_MODE.load(Ordering::Acquire)
        && u16::from(linux_console_descriptor(descriptor)) == LINUX_DESCRIPTOR_INPUT
        && LINUX_RUNTIME_SUSPEND_REQUESTED.swap(false, Ordering::AcqRel)
    {
        if count == 0 || !save_suspended_linux_runtime(caller, address, count) {
            return LINUX_EINVAL;
        }
        return LINUX_RETURN_TO_KERNEL;
    }
    if immediate != LINUX_ENOSYS
        || !linux_eventfd_descriptor(descriptor)
            && !linux_pipe_read_descriptor(descriptor)
            && !linux_timerfd_descriptor(descriptor)
    {
        return immediate;
    }
    let (kind, wait_count) = if linux_eventfd_descriptor(descriptor) {
        if count != 8 {
            return LINUX_EINVAL;
        }
        (LINUX_THREAD_WAIT_EVENTFD_READ, 8)
    } else if linux_timerfd_descriptor(descriptor) {
        if count != 8 {
            return LINUX_EINVAL;
        }
        (LINUX_THREAD_WAIT_TIMERFD_READ, 8)
    } else {
        (
            LINUX_THREAD_WAIT_PIPE_READ,
            count.min(LINUX_PIPE_CAPACITY as u64),
        )
    };
    if wait_count == 0 || !linux_writable_user_range_contains(address, wait_count as usize) {
        return LINUX_EFAULT;
    }
    let wait_output = if kind == LINUX_THREAD_WAIT_PIPE_READ {
        wait_count << 8 | descriptor
    } else {
        descriptor
    };
    linux_blocking_descriptor_io_live_return(
        output,
        caller,
        kind,
        address,
        wait_output,
        if kind == LINUX_THREAD_WAIT_TIMERFD_READ {
            ELF_PROBE_TIMERFD_DEADLINE.load(Ordering::SeqCst)
        } else {
            0
        },
        None,
    )
}

fn linux_descriptor_readv_live_return(
    output: *mut LinuxSyscallFrame,
    caller: LinuxUserContext,
    descriptor: u64,
    vectors: u64,
    count: u64,
) -> u64 {
    let immediate = linux_descriptor_readv(descriptor, vectors, count);
    if LINUX_RUNTIME_MODE.load(Ordering::Acquire)
        && u16::from(linux_console_descriptor(descriptor)) == LINUX_DESCRIPTOR_INPUT
        && LINUX_RUNTIME_SUSPEND_REQUESTED.swap(false, Ordering::AcqRel)
    {
        let (ranges, vector_count) = match linux_readv_ranges(vectors, count) {
            Ok(snapshot) => snapshot,
            Err(error) => return error,
        };
        if vector_count == 0 || !save_suspended_linux_runtime(caller, ranges[0].0, 1) {
            return LINUX_EINVAL;
        }
        return LINUX_RETURN_TO_KERNEL;
    }
    if immediate != LINUX_ENOSYS || !linux_pipe_read_descriptor(descriptor) {
        return immediate;
    }
    let (ranges, vector_count) = match linux_readv_ranges(vectors, count) {
        Ok(snapshot) => snapshot,
        Err(error) => return error,
    };
    linux_blocking_descriptor_io_live_return(
        output,
        caller,
        LINUX_THREAD_WAIT_PIPE_READ,
        vectors,
        LINUX_PIPE_READV_WAIT_FLAG | (vector_count as u64) << 8 | descriptor,
        0,
        Some(&ranges[..vector_count]),
    )
}

fn linux_descriptor_write_live_return(
    output: *mut LinuxSyscallFrame,
    caller: LinuxUserContext,
    descriptor: u64,
    address: u64,
    count: u64,
) -> u64 {
    let immediate = linux_descriptor_write(descriptor, address, count);
    if immediate == LINUX_EPIPE && linux_pipe_write_descriptor(descriptor) {
        return linux_broken_pipe_live_return(output, caller);
    }
    if immediate != LINUX_ENOSYS
        || !linux_pipe_write_descriptor(descriptor) && !linux_eventfd_descriptor(descriptor)
    {
        return immediate;
    }
    if linux_eventfd_descriptor(descriptor) {
        if count != 8 {
            return LINUX_EINVAL;
        }
        let mut bytes = [0_u8; 8];
        if !copy_from_linux_user(address, &mut bytes) {
            return LINUX_EFAULT;
        }
        let increment = u64::from_ne_bytes(bytes);
        if increment == 0 || increment == u64::MAX {
            return LINUX_EINVAL;
        }
        return linux_blocking_descriptor_io_live_return(
            output,
            caller,
            LINUX_THREAD_WAIT_EVENTFD_WRITE,
            increment,
            descriptor,
            0,
            None,
        );
    }
    let Ok(write_count) = usize::try_from(count) else {
        return LINUX_EINVAL;
    };
    if write_count == 0
        || write_count > LINUX_PIPE_CAPACITY
        || !linux_readable_user_range_contains(address, write_count)
    {
        return if write_count == 0 || write_count > LINUX_PIPE_CAPACITY {
            LINUX_EINVAL
        } else {
            LINUX_EFAULT
        };
    }
    linux_blocking_descriptor_io_live_return(
        output,
        caller,
        LINUX_THREAD_WAIT_PIPE_WRITE,
        address,
        count << 8 | descriptor,
        0,
        None,
    )
}

fn linux_descriptor_writev_live_return(
    output: *mut LinuxSyscallFrame,
    caller: LinuxUserContext,
    descriptor: u64,
    vectors: u64,
    count: u64,
) -> u64 {
    let immediate = linux_descriptor_writev(descriptor, vectors, count);
    if immediate == LINUX_EPIPE && linux_pipe_write_descriptor(descriptor) {
        return linux_broken_pipe_live_return(output, caller);
    }
    if immediate != LINUX_ENOSYS || !linux_pipe_write_descriptor(descriptor) {
        return immediate;
    }
    let (ranges, vector_count) = match linux_writev_ranges(vectors, count) {
        Ok(snapshot) => snapshot,
        Err(error) => return error,
    };
    linux_blocking_descriptor_io_live_return(
        output,
        caller,
        LINUX_THREAD_WAIT_PIPE_WRITE,
        vectors,
        LINUX_PIPE_WRITEV_WAIT_FLAG | (vector_count as u64) << 8 | descriptor,
        0,
        Some(&ranges[..vector_count]),
    )
}

fn linux_broken_pipe_live_return(
    output: *mut LinuxSyscallFrame,
    mut interrupted: LinuxUserContext,
) -> u64 {
    interrupted.rax = LINUX_EPIPE;
    let result = linux_kill(
        1,
        (LINUX_SIGPIPE, LINUX_SI_KERNEL),
        output,
        interrupted,
        (interrupted.rip, interrupted.rflags),
    );
    if result == LINUX_RETURN_TO_KERNEL || result != 0 {
        result
    } else if LINUX_SIGNAL_ACTIVE.load(Ordering::SeqCst) {
        0
    } else {
        LINUX_EPIPE
    }
}

fn linux_blocking_descriptor_io_live_return(
    output: *mut LinuxSyscallFrame,
    caller: LinuxUserContext,
    kind: u8,
    wait_key: u64,
    wait_output: u64,
    deadline: u64,
    iov_snapshot: Option<&[(u64, usize)]>,
) -> u64 {
    let Some(current) = current_linux_thread_handle() else {
        return LINUX_ESRCH;
    };
    let Some(target) = ready_linux_thread_handle() else {
        return LINUX_ENOSYS;
    };
    let Some(tls) = linux_thread_context(current) else {
        return LINUX_ESRCH;
    };
    let slot = usize::from(current.slot);
    if let Some(ranges) = iov_snapshot {
        for (index, (address, length)) in ranges.iter().copied().enumerate() {
            LINUX_THREAD_IOV_ADDRESSES[slot][index].store(address, Ordering::SeqCst);
            LINUX_THREAD_IOV_LENGTHS[slot][index].store(length as u64, Ordering::SeqCst);
        }
    }
    if !save_linux_thread_return_context(current, caller)
        || !block_current_linux_thread(kind, wait_key, wait_output, deadline)
    {
        clear_linux_iov_snapshot(slot);
        return LINUX_EINVAL;
    }
    let mut target_registers = LinuxSyscallFrame::ZERO;
    let mut target_rip = 0;
    let mut target_rsp = 0;
    let mut target_rflags = 0;
    let mut target_rax = 0;
    if !install_linux_thread_return(
        target,
        &mut target_registers,
        &mut target_rip,
        &mut target_rsp,
        &mut target_rflags,
        &mut target_rax,
    ) {
        LINUX_THREAD_WAIT_KINDS[slot].store(LINUX_THREAD_WAIT_NONE, Ordering::SeqCst);
        LINUX_THREAD_WAIT_KEYS[slot].store(0, Ordering::SeqCst);
        LINUX_THREAD_WAIT_OUTPUTS[slot].store(0, Ordering::SeqCst);
        LINUX_THREAD_WAIT_DEADLINES[slot].store(0, Ordering::SeqCst);
        clear_linux_iov_snapshot(slot);
        LINUX_THREAD_STATES[slot].store(LINUX_THREAD_RUNNING, Ordering::SeqCst);
        LINUX_CURRENT_THREAD_SLOT.store(current.slot, Ordering::SeqCst);
        let _ = install_linux_tls_context(tls);
        return LINUX_EINVAL;
    }
    let selected = LinuxUserContext {
        registers: target_registers,
        rip: target_rip,
        rsp: target_rsp,
        rflags: target_rflags,
        rax: target_rax,
        fs_base: active_fs_base(),
    };
    if !write_linux_live_return(output, selected) {
        return LINUX_EINVAL;
    }
    LINUX_LIVE_RETURN_VALIDATED.store(true, Ordering::SeqCst);
    if kind == LINUX_THREAD_WAIT_PIPE_READ {
        LINUX_PIPE_READ_BLOCK_COUNT.fetch_add(1, Ordering::SeqCst);
    } else if kind == LINUX_THREAD_WAIT_PIPE_WRITE {
        LINUX_PIPE_WRITE_BLOCK_COUNT.fetch_add(1, Ordering::SeqCst);
    } else if kind == LINUX_THREAD_WAIT_EVENTFD_WRITE {
        LINUX_EVENTFD_WRITE_BLOCK_COUNT.fetch_add(1, Ordering::SeqCst);
    } else if kind == LINUX_THREAD_WAIT_TIMERFD_READ {
        LINUX_TIMERFD_READ_BLOCK_COUNT.fetch_add(1, Ordering::SeqCst);
    } else {
        LINUX_EVENTFD_READ_BLOCK_COUNT.fetch_add(1, Ordering::SeqCst);
    }
    selected.rax
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
    linux_eventfd_write_value(descriptor, u64::from_ne_bytes(bytes), true)
}

fn linux_eventfd_write_value(descriptor: u64, increment: u64, wake: bool) -> Option<u64> {
    if !linux_eventfd_descriptor(descriptor) {
        return None;
    }
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
    if wake {
        wake_linux_descriptor_waits();
    }
    Some(8)
}

fn linux_timerfd_create(clock: u64, flags: u64) -> u64 {
    if clock != LINUX_CLOCK_MONOTONIC || flags & !(LINUX_TFD_NONBLOCK | LINUX_O_CLOEXEC) != 0 {
        return LINUX_EINVAL;
    }
    if ELF_PROBE_TIMERFD_DESCRIPTOR.load(Ordering::SeqCst) != 0 {
        return LINUX_EMFILE;
    }
    let Some(descriptor) = (LINUX_AUX_DESCRIPTOR_FIRST..LINUX_AUX_DESCRIPTOR_LIMIT)
        .find(|descriptor| linux_aux_descriptor_free(*descriptor))
    else {
        return LINUX_EMFILE;
    };
    ELF_PROBE_TIMERFD_FLAGS.store(flags as u32, Ordering::SeqCst);
    ELF_PROBE_TIMERFD_INTERVAL.store(0, Ordering::SeqCst);
    ELF_PROBE_TIMERFD_DEADLINE.store(0, Ordering::SeqCst);
    ELF_PROBE_TIMERFD_DESCRIPTOR.store(descriptor as u8, Ordering::SeqCst);
    if flags & LINUX_O_CLOEXEC != 0 {
        ELF_PROBE_CONSOLE_CLOEXEC.fetch_or(1 << descriptor, Ordering::SeqCst);
    } else {
        ELF_PROBE_CONSOLE_CLOEXEC.fetch_and(!(1 << descriptor), Ordering::SeqCst);
    }
    descriptor
}

fn linux_timerfd_settime(descriptor: u64, flags: u64, new_address: u64, old_address: u64) -> u64 {
    if !linux_timerfd_descriptor(descriptor) {
        return LINUX_EBADF;
    }
    if flags & !LINUX_TFD_TIMER_ABSTIME != 0 {
        return LINUX_EINVAL;
    }
    if old_address != 0 && !linux_writable_user_range_contains(old_address, 32) {
        return LINUX_EFAULT;
    }
    let mut specification = [0_u8; 32];
    if !copy_from_linux_user(new_address, &mut specification) {
        return LINUX_EFAULT;
    }
    let Some(interval) = linux_timespec_nanoseconds(&specification[..16]) else {
        return LINUX_EINVAL;
    };
    let Some(value) = linux_timespec_nanoseconds(&specification[16..]) else {
        return LINUX_EINVAL;
    };
    let Some(now) = linux_elapsed_nanoseconds() else {
        return LINUX_ENOSYS;
    };
    let old = linux_timerfd_specification(now);
    let deadline = if value == 0 {
        0
    } else if flags & LINUX_TFD_TIMER_ABSTIME != 0 {
        value
    } else {
        let Some(deadline) = now.checked_add(value) else {
            return LINUX_EINVAL;
        };
        deadline
    };
    if old_address != 0 && !copy_to_linux_user(old_address, &old) {
        return LINUX_EFAULT;
    }
    ELF_PROBE_TIMERFD_INTERVAL.store(interval, Ordering::SeqCst);
    ELF_PROBE_TIMERFD_DEADLINE.store(deadline, Ordering::SeqCst);
    wake_linux_descriptor_waits();
    0
}

fn linux_timerfd_gettime(descriptor: u64, address: u64) -> u64 {
    if !linux_timerfd_descriptor(descriptor) {
        return LINUX_EBADF;
    }
    let Some(now) = linux_elapsed_nanoseconds() else {
        return LINUX_ENOSYS;
    };
    let specification = linux_timerfd_specification(now);
    if !copy_to_linux_user(address, &specification) {
        return LINUX_EFAULT;
    }
    0
}

fn linux_timerfd_read(descriptor: u64, address: u64, count: u64) -> Option<u64> {
    if !linux_timerfd_descriptor(descriptor) {
        return None;
    }
    if count != 8 {
        return Some(LINUX_EINVAL);
    }
    let Some(now) = linux_elapsed_nanoseconds() else {
        return Some(LINUX_ENOSYS);
    };
    let deadline = ELF_PROBE_TIMERFD_DEADLINE.load(Ordering::SeqCst);
    if deadline == 0 || now < deadline {
        return Some(
            if ELF_PROBE_TIMERFD_FLAGS.load(Ordering::SeqCst) & LINUX_TFD_NONBLOCK as u32 != 0 {
                LINUX_EAGAIN
            } else {
                LINUX_ENOSYS
            },
        );
    }
    let interval = ELF_PROBE_TIMERFD_INTERVAL.load(Ordering::SeqCst);
    let expirations = now
        .saturating_sub(deadline)
        .checked_div(interval)
        .map_or(1, |elapsed| elapsed + 1);
    if !copy_to_linux_user(address, &expirations.to_ne_bytes()) {
        return Some(LINUX_EFAULT);
    }
    let next = if interval == 0 {
        0
    } else {
        deadline.saturating_add(expirations.saturating_mul(interval))
    };
    ELF_PROBE_TIMERFD_DEADLINE.store(next, Ordering::SeqCst);
    Some(8)
}

fn linux_timespec_nanoseconds(timespec: &[u8]) -> Option<u64> {
    let seconds = i64::from_ne_bytes(timespec[..8].try_into().ok()?);
    let nanoseconds = i64::from_ne_bytes(timespec[8..16].try_into().ok()?);
    if seconds < 0 || !(0..NANOSECONDS_PER_SECOND as i64).contains(&nanoseconds) {
        return None;
    }
    (seconds as u64)
        .checked_mul(NANOSECONDS_PER_SECOND)
        .and_then(|value| value.checked_add(nanoseconds as u64))
}

fn linux_timerfd_specification(now: u64) -> [u8; 32] {
    let interval = ELF_PROBE_TIMERFD_INTERVAL.load(Ordering::SeqCst);
    let deadline = ELF_PROBE_TIMERFD_DEADLINE.load(Ordering::SeqCst);
    let remaining = if deadline == 0 {
        0
    } else if now < deadline {
        deadline - now
    } else if interval == 0 {
        0
    } else {
        interval - (now - deadline) % interval
    };
    let mut specification = [0_u8; 32];
    linux_store_timespec(&mut specification[..16], interval);
    linux_store_timespec(&mut specification[16..], remaining);
    specification
}

fn linux_store_timespec(timespec: &mut [u8], nanoseconds: u64) {
    timespec[..8].copy_from_slice(&(nanoseconds / NANOSECONDS_PER_SECOND).to_ne_bytes());
    timespec[8..16].copy_from_slice(&(nanoseconds % NANOSECONDS_PER_SECOND).to_ne_bytes());
}

fn linux_pipe(address: u64, flags: u64) -> u64 {
    if flags & !(LINUX_O_NONBLOCK | LINUX_O_CLOEXEC) != 0 {
        return LINUX_EINVAL;
    }
    if ELF_PROBE_PIPE_READ_DESCRIPTOR.load(Ordering::SeqCst) != 0
        || ELF_PROBE_PIPE_WRITE_DESCRIPTOR.load(Ordering::SeqCst) != 0
    {
        return LINUX_EMFILE;
    }
    let mut descriptors = [0_u64; 2];
    let mut found = 0;
    for descriptor in LINUX_AUX_DESCRIPTOR_FIRST..LINUX_AUX_DESCRIPTOR_LIMIT {
        if linux_aux_descriptor_free(descriptor) {
            descriptors[found] = descriptor;
            found += 1;
            if found == descriptors.len() {
                break;
            }
        }
    }
    if found != descriptors.len() {
        return LINUX_EMFILE;
    }
    let mut output = [0_u8; 8];
    output[..4].copy_from_slice(&(descriptors[0] as u32).to_ne_bytes());
    output[4..].copy_from_slice(&(descriptors[1] as u32).to_ne_bytes());
    if !copy_to_linux_user(address, &output) {
        return LINUX_EFAULT;
    }
    for byte in &ELF_PROBE_PIPE_BUFFER {
        byte.store(0, Ordering::SeqCst);
    }
    ELF_PROBE_PIPE_LENGTH.store(0, Ordering::SeqCst);
    ELF_PROBE_PIPE_READ_FLAGS.store(flags as u32, Ordering::SeqCst);
    ELF_PROBE_PIPE_WRITE_FLAGS.store(flags as u32, Ordering::SeqCst);
    ELF_PROBE_PIPE_READ_DESCRIPTOR.store(descriptors[0] as u8, Ordering::SeqCst);
    ELF_PROBE_PIPE_WRITE_DESCRIPTOR.store(descriptors[1] as u8, Ordering::SeqCst);
    for descriptor in descriptors {
        if flags & LINUX_O_CLOEXEC != 0 {
            ELF_PROBE_CONSOLE_CLOEXEC.fetch_or(1 << descriptor, Ordering::SeqCst);
        } else {
            ELF_PROBE_CONSOLE_CLOEXEC.fetch_and(!(1 << descriptor), Ordering::SeqCst);
        }
    }
    0
}

fn linux_pipe_read(descriptor: u64, address: u64, count: u64) -> Option<u64> {
    if !linux_pipe_read_descriptor(descriptor) {
        return None;
    }
    if count == 0 {
        return Some(0);
    }
    let length = ELF_PROBE_PIPE_LENGTH.load(Ordering::SeqCst) as usize;
    if length == 0 {
        if ELF_PROBE_PIPE_WRITE_DESCRIPTOR.load(Ordering::SeqCst) == 0 {
            return Some(0);
        }
        return Some(
            if ELF_PROBE_PIPE_READ_FLAGS.load(Ordering::SeqCst) & LINUX_O_NONBLOCK as u32 != 0 {
                LINUX_EAGAIN
            } else {
                LINUX_ENOSYS
            },
        );
    }
    let requested = usize::try_from(count).unwrap_or(usize::MAX);
    let returned = length.min(requested);
    let mut output = [0_u8; LINUX_PIPE_CAPACITY];
    for (target, source) in output[..returned]
        .iter_mut()
        .zip(&ELF_PROBE_PIPE_BUFFER[..returned])
    {
        *target = source.load(Ordering::SeqCst);
    }
    if !copy_to_linux_user(address, &output[..returned]) {
        return Some(LINUX_EFAULT);
    }
    for index in 0..length - returned {
        let value = ELF_PROBE_PIPE_BUFFER[index + returned].load(Ordering::SeqCst);
        ELF_PROBE_PIPE_BUFFER[index].store(value, Ordering::SeqCst);
    }
    for byte in &ELF_PROBE_PIPE_BUFFER[length - returned..length] {
        byte.store(0, Ordering::SeqCst);
    }
    ELF_PROBE_PIPE_LENGTH.store((length - returned) as u32, Ordering::SeqCst);
    wake_linux_descriptor_waits();
    Some(returned as u64)
}

fn linux_pipe_write(descriptor: u64, address: u64, count: u64) -> Option<u64> {
    if !linux_pipe_write_descriptor(descriptor) {
        return None;
    }
    if count == 0 {
        return Some(0);
    }
    if ELF_PROBE_PIPE_READ_DESCRIPTOR.load(Ordering::SeqCst) == 0 {
        return Some(LINUX_EPIPE);
    }
    let Ok(requested) = usize::try_from(count) else {
        return Some(LINUX_EINVAL);
    };
    if requested > LINUX_PIPE_CAPACITY {
        return Some(LINUX_EINVAL);
    }
    let length = ELF_PROBE_PIPE_LENGTH.load(Ordering::SeqCst) as usize;
    if requested > LINUX_PIPE_CAPACITY - length {
        return Some(
            if ELF_PROBE_PIPE_WRITE_FLAGS.load(Ordering::SeqCst) & LINUX_O_NONBLOCK as u32 != 0 {
                LINUX_EAGAIN
            } else {
                LINUX_ENOSYS
            },
        );
    }
    let mut input = [0_u8; LINUX_PIPE_CAPACITY];
    if !copy_from_linux_user(address, &mut input[..requested]) {
        return Some(LINUX_EFAULT);
    }
    for (target, source) in ELF_PROBE_PIPE_BUFFER[length..length + requested]
        .iter()
        .zip(&input[..requested])
    {
        target.store(*source, Ordering::SeqCst);
    }
    ELF_PROBE_PIPE_LENGTH.store((length + requested) as u32, Ordering::SeqCst);
    wake_linux_descriptor_waits();
    Some(count)
}

fn linux_pipe_writev(descriptor: u64, vectors: u64, count: u64) -> Option<u64> {
    if !linux_pipe_write_descriptor(descriptor) {
        return None;
    }
    let (ranges, vector_count) = match linux_writev_ranges(vectors, count) {
        Ok(snapshot) => snapshot,
        Err(error) => return Some(error),
    };
    Some(linux_pipe_writev_ranges(
        descriptor,
        &ranges[..vector_count],
    ))
}

fn linux_writev_ranges(
    vectors: u64,
    count: u64,
) -> Result<([(u64, usize); LINUX_IOV_LIMIT], usize), u64> {
    let Ok(vector_count) = usize::try_from(count) else {
        return Err(LINUX_EINVAL);
    };
    if !(1..=LINUX_IOV_LIMIT).contains(&vector_count) {
        return Err(LINUX_EINVAL);
    }
    let mut table = [0_u8; LINUX_IOV_LIMIT * 16];
    if !copy_from_linux_user(vectors, &mut table[..vector_count * 16]) {
        return Err(LINUX_EFAULT);
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
            return Err(LINUX_EINVAL);
        };
        let Some(end) = total.checked_add(length) else {
            return Err(LINUX_EINVAL);
        };
        if length == 0 || end > LINUX_PIPE_CAPACITY {
            return Err(LINUX_EINVAL);
        }
        if !linux_readable_user_range_contains(address, length) {
            return Err(LINUX_EFAULT);
        }
        ranges[index] = (address, length);
        total = end;
    }
    Ok((ranges, vector_count))
}

fn linux_pipe_writev_ranges(descriptor: u64, ranges: &[(u64, usize)]) -> u64 {
    if !linux_pipe_write_descriptor(descriptor) {
        return LINUX_EBADF;
    }
    if ELF_PROBE_PIPE_READ_DESCRIPTOR.load(Ordering::SeqCst) == 0 {
        return LINUX_EPIPE;
    }
    let mut total = 0_usize;
    for (_, length) in ranges.iter().copied() {
        let Some(end) = total.checked_add(length) else {
            return LINUX_EINVAL;
        };
        if length == 0 || end > LINUX_PIPE_CAPACITY {
            return LINUX_EINVAL;
        }
        total = end;
    }
    let current = ELF_PROBE_PIPE_LENGTH.load(Ordering::SeqCst) as usize;
    if total > LINUX_PIPE_CAPACITY - current {
        return if ELF_PROBE_PIPE_WRITE_FLAGS.load(Ordering::SeqCst) & LINUX_O_NONBLOCK as u32 != 0 {
            LINUX_EAGAIN
        } else {
            LINUX_ENOSYS
        };
    }
    let mut input = [0_u8; LINUX_PIPE_CAPACITY];
    let mut copied = 0;
    for (address, length) in ranges.iter().copied() {
        if !copy_from_linux_user(address, &mut input[copied..copied + length]) {
            return LINUX_EFAULT;
        }
        copied += length;
    }
    for (target, source) in ELF_PROBE_PIPE_BUFFER[current..current + total]
        .iter()
        .zip(&input[..total])
    {
        target.store(*source, Ordering::SeqCst);
    }
    ELF_PROBE_PIPE_LENGTH.store((current + total) as u32, Ordering::SeqCst);
    wake_linux_descriptor_waits();
    total as u64
}

fn linux_pipe_readv(descriptor: u64, vectors: u64, count: u64) -> Option<u64> {
    if !linux_pipe_read_descriptor(descriptor) {
        return None;
    }
    let (ranges, vector_count) = match linux_readv_ranges(vectors, count) {
        Ok(snapshot) => snapshot,
        Err(error) => return Some(error),
    };
    Some(linux_pipe_readv_ranges(descriptor, &ranges[..vector_count]))
}

fn linux_readv_ranges(
    vectors: u64,
    count: u64,
) -> Result<([(u64, usize); LINUX_IOV_LIMIT], usize), u64> {
    let Ok(vector_count) = usize::try_from(count) else {
        return Err(LINUX_EINVAL);
    };
    if !(1..=LINUX_IOV_LIMIT).contains(&vector_count) {
        return Err(LINUX_EINVAL);
    }
    let mut table = [0_u8; LINUX_IOV_LIMIT * 16];
    if !copy_from_linux_user(vectors, &mut table[..vector_count * 16]) {
        return Err(LINUX_EFAULT);
    }
    let mut ranges = [(0_u64, 0_usize); LINUX_IOV_LIMIT];
    let mut capacity = 0_usize;
    for (index, vector) in table[..vector_count * 16]
        .as_chunks::<16>()
        .0
        .iter()
        .enumerate()
    {
        let address = u64::from_ne_bytes(vector[..8].try_into().unwrap());
        let length = u64::from_ne_bytes(vector[8..].try_into().unwrap());
        let Ok(length) = usize::try_from(length) else {
            return Err(LINUX_EINVAL);
        };
        let Some(end) = capacity.checked_add(length) else {
            return Err(LINUX_EINVAL);
        };
        if length == 0 || end > LINUX_PIPE_CAPACITY {
            return Err(LINUX_EINVAL);
        }
        if !linux_writable_user_range_contains(address, length) {
            return Err(LINUX_EFAULT);
        }
        ranges[index] = (address, length);
        capacity = end;
    }
    Ok((ranges, vector_count))
}

fn linux_pipe_readv_ranges(descriptor: u64, ranges: &[(u64, usize)]) -> u64 {
    if !linux_pipe_read_descriptor(descriptor) || ranges.is_empty() {
        return LINUX_EBADF;
    }
    let mut capacity = 0_usize;
    for (address, range_length) in ranges.iter().copied() {
        let Some(end) = capacity.checked_add(range_length) else {
            return LINUX_EINVAL;
        };
        if range_length == 0
            || end > LINUX_PIPE_CAPACITY
            || !linux_writable_user_range_contains(address, range_length)
        {
            return if range_length == 0 || end > LINUX_PIPE_CAPACITY {
                LINUX_EINVAL
            } else {
                LINUX_EFAULT
            };
        }
        capacity = end;
    }
    let length = ELF_PROBE_PIPE_LENGTH.load(Ordering::SeqCst) as usize;
    if length == 0 {
        if ELF_PROBE_PIPE_WRITE_DESCRIPTOR.load(Ordering::SeqCst) == 0 {
            return 0;
        }
        return if ELF_PROBE_PIPE_READ_FLAGS.load(Ordering::SeqCst) & LINUX_O_NONBLOCK as u32 != 0 {
            LINUX_EAGAIN
        } else {
            LINUX_ENOSYS
        };
    }
    let returned = length.min(capacity);
    let mut output = [0_u8; LINUX_PIPE_CAPACITY];
    for (target, source) in output[..returned]
        .iter_mut()
        .zip(&ELF_PROBE_PIPE_BUFFER[..returned])
    {
        *target = source.load(Ordering::SeqCst);
    }
    let mut copied = 0;
    for (address, range_length) in ranges.iter().copied() {
        let amount = range_length.min(returned - copied);
        if amount == 0 {
            break;
        }
        if !copy_to_linux_user(address, &output[copied..copied + amount]) {
            return LINUX_EFAULT;
        }
        copied += amount;
    }
    for index in 0..length - returned {
        let value = ELF_PROBE_PIPE_BUFFER[index + returned].load(Ordering::SeqCst);
        ELF_PROBE_PIPE_BUFFER[index].store(value, Ordering::SeqCst);
    }
    for byte in &ELF_PROBE_PIPE_BUFFER[length - returned..length] {
        byte.store(0, Ordering::SeqCst);
    }
    ELF_PROBE_PIPE_LENGTH.store((length - returned) as u32, Ordering::SeqCst);
    wake_linux_descriptor_waits();
    returned as u64
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
    wake_linux_descriptor_waits();
    0
}

fn linux_epoll_event(epoll: u64) -> Result<Option<[u8; 12]>, u64> {
    if !linux_epoll_descriptor(epoll) {
        return Err(LINUX_EBADF);
    }
    let target = ELF_PROBE_EPOLL_TARGET.load(Ordering::SeqCst);
    let requested = ELF_PROBE_EPOLL_EVENTS.load(Ordering::SeqCst) as u16;
    let ready = if target == u64::MAX {
        0
    } else {
        linux_descriptor_poll_events(target as i32, requested)
            & (LINUX_POLLIN | LINUX_POLLOUT | LINUX_POLLERR | LINUX_POLLHUP)
    };
    if ready == 0 {
        return Ok(None);
    }
    let mut event = [0_u8; 12];
    event[..4].copy_from_slice(&u32::from(ready).to_ne_bytes());
    event[4..].copy_from_slice(&ELF_PROBE_EPOLL_DATA.load(Ordering::SeqCst).to_ne_bytes());
    Ok(Some(event))
}

fn complete_linux_epoll_wait(slot: usize, timed_out: bool) -> bool {
    let kind = if slot < LINUX_THREAD_LIMIT {
        LINUX_THREAD_WAIT_KINDS[slot].load(Ordering::SeqCst)
    } else {
        LINUX_THREAD_WAIT_NONE
    };
    if slot >= LINUX_THREAD_LIMIT
        || LINUX_THREAD_STATES[slot].load(Ordering::SeqCst) != LINUX_THREAD_BLOCKED
        || !matches!(
            kind,
            LINUX_THREAD_WAIT_EPOLL | LINUX_THREAD_WAIT_EPOLL_PWAIT
        )
    {
        return false;
    }
    let epoll = LINUX_THREAD_WAIT_OUTPUTS[slot].load(Ordering::SeqCst);
    let event = linux_epoll_event(epoll);
    if matches!(event, Ok(None)) && !timed_out {
        return false;
    }
    let address = LINUX_THREAD_WAIT_KEYS[slot].load(Ordering::SeqCst);
    let result = match event {
        Ok(Some(event)) if copy_to_linux_user(address, &event) => 1,
        Ok(Some(_)) => LINUX_EFAULT,
        Ok(None) => 0,
        Err(error) => error,
    };
    LINUX_THREAD_RAXS[slot].store(result, Ordering::SeqCst);
    LINUX_THREAD_WAIT_KINDS[slot].store(LINUX_THREAD_WAIT_NONE, Ordering::SeqCst);
    LINUX_THREAD_WAIT_KEYS[slot].store(0, Ordering::SeqCst);
    LINUX_THREAD_WAIT_OUTPUTS[slot].store(0, Ordering::SeqCst);
    LINUX_THREAD_WAIT_DEADLINES[slot].store(0, Ordering::SeqCst);
    if kind == LINUX_THREAD_WAIT_EPOLL_PWAIT {
        LINUX_THREAD_SIGNAL_MASKS[slot].store(
            LINUX_THREAD_WAIT_PRIOR_SIGNAL_MASKS[slot].swap(0, Ordering::SeqCst),
            Ordering::SeqCst,
        );
    }
    LINUX_THREAD_STATES[slot].store(LINUX_THREAD_READY, Ordering::SeqCst);
    match (kind, timed_out) {
        (LINUX_THREAD_WAIT_EPOLL, true) => {
            LINUX_EPOLL_WAIT_TIMEOUT_COUNT.fetch_add(1, Ordering::SeqCst);
        }
        (LINUX_THREAD_WAIT_EPOLL, false) => {
            LINUX_EPOLL_WAIT_WAKE_COUNT.fetch_add(1, Ordering::SeqCst);
        }
        (LINUX_THREAD_WAIT_EPOLL_PWAIT, true) => {
            LINUX_EPOLL_PWAIT_TIMEOUT_COUNT.fetch_add(1, Ordering::SeqCst);
        }
        (LINUX_THREAD_WAIT_EPOLL_PWAIT, false) => {
            LINUX_EPOLL_PWAIT_WAKE_COUNT.fetch_add(1, Ordering::SeqCst);
        }
        _ => {}
    }
    true
}

fn wake_linux_epoll_waits() -> u32 {
    let mut woken = 0;
    for slot in 0..LINUX_THREAD_LIMIT {
        woken += u32::from(complete_linux_epoll_wait(slot, false));
    }
    woken
}

fn wake_linux_descriptor_waits() -> u32 {
    wake_linux_poll_waits()
        + wake_linux_epoll_waits()
        + wake_linux_eventfd_read_waits()
        + wake_linux_eventfd_write_waits()
        + wake_linux_pipe_read_waits()
        + wake_linux_pipe_write_waits()
        + wake_linux_timerfd_read_waits()
}

#[derive(Clone, Copy)]
struct LinuxEpollWaitInput {
    epoll: u64,
    address: u64,
    maximum: u64,
}

#[derive(Clone, Copy)]
struct LinuxSignalMaskInput {
    address: u64,
    size: u64,
}

fn linux_epoll_wait_live_return(
    output: *mut LinuxSyscallFrame,
    caller: LinuxUserContext,
    input: LinuxEpollWaitInput,
    timeout: u64,
) -> u64 {
    let timeout = timeout as u32 as i32;
    let duration = if timeout < 0 {
        None
    } else {
        let Some(duration) = (timeout as u64).checked_mul(1_000_000) else {
            return LINUX_EINVAL;
        };
        Some(duration)
    };
    linux_epoll_wait_duration_live_return(
        output,
        caller,
        input,
        duration,
        LINUX_THREAD_WAIT_EPOLL,
        None,
    )
}

fn linux_epoll_wait_duration_live_return(
    output: *mut LinuxSyscallFrame,
    caller: LinuxUserContext,
    input: LinuxEpollWaitInput,
    duration: Option<u64>,
    wait_kind: u8,
    temporary_signal_mask: Option<u64>,
) -> u64 {
    if input.maximum != 1 {
        return LINUX_EINVAL;
    }
    match linux_epoll_event(input.epoll) {
        Ok(Some(event)) => {
            return if copy_to_linux_user(input.address, &event) {
                1
            } else {
                LINUX_EFAULT
            };
        }
        Ok(None) if duration == Some(0) => return 0,
        Ok(None) => {}
        Err(error) => return error,
    }
    if !linux_writable_user_range_contains(input.address, 12) {
        return LINUX_EFAULT;
    }
    let Some(current) = current_linux_thread_handle() else {
        return LINUX_ESRCH;
    };
    let Some(target) = ready_linux_thread_handle() else {
        return LINUX_ENOSYS;
    };
    let Some(tls) = linux_thread_context(current) else {
        return LINUX_ESRCH;
    };
    let deadline = if let Some(duration) = duration {
        let Some(deadline) = linux_elapsed_nanoseconds().and_then(|now| now.checked_add(duration))
        else {
            return LINUX_EINVAL;
        };
        deadline
    } else {
        0
    };
    let slot = usize::from(current.slot);
    if !save_linux_thread_return_context(current, caller)
        || !block_current_linux_thread(wait_kind, input.address, input.epoll, deadline)
    {
        return LINUX_EINVAL;
    }
    let prior_signal_mask = ELF_PROBE_SIGNAL_MASK.load(Ordering::SeqCst);
    if let Some(mask) = temporary_signal_mask {
        LINUX_THREAD_WAIT_PRIOR_SIGNAL_MASKS[slot].store(prior_signal_mask, Ordering::SeqCst);
        LINUX_THREAD_SIGNAL_MASKS[slot].store(mask, Ordering::SeqCst);
    }
    let mut target_registers = LinuxSyscallFrame::ZERO;
    let mut target_rip = 0;
    let mut target_rsp = 0;
    let mut target_rflags = 0;
    let mut target_rax = 0;
    if !install_linux_thread_return(
        target,
        &mut target_registers,
        &mut target_rip,
        &mut target_rsp,
        &mut target_rflags,
        &mut target_rax,
    ) {
        LINUX_THREAD_WAIT_KINDS[slot].store(LINUX_THREAD_WAIT_NONE, Ordering::SeqCst);
        LINUX_THREAD_WAIT_KEYS[slot].store(0, Ordering::SeqCst);
        LINUX_THREAD_WAIT_OUTPUTS[slot].store(0, Ordering::SeqCst);
        LINUX_THREAD_WAIT_DEADLINES[slot].store(0, Ordering::SeqCst);
        LINUX_THREAD_WAIT_PRIOR_SIGNAL_MASKS[slot].store(0, Ordering::SeqCst);
        LINUX_THREAD_STATES[slot].store(LINUX_THREAD_RUNNING, Ordering::SeqCst);
        LINUX_CURRENT_THREAD_SLOT.store(current.slot, Ordering::SeqCst);
        let _ = install_linux_tls_context(tls);
        if temporary_signal_mask.is_some() {
            store_current_linux_signal_mask(prior_signal_mask);
        }
        return LINUX_EINVAL;
    }
    let selected = LinuxUserContext {
        registers: target_registers,
        rip: target_rip,
        rsp: target_rsp,
        rflags: target_rflags,
        rax: target_rax,
        fs_base: active_fs_base(),
    };
    if !write_linux_live_return(output, selected) {
        return LINUX_EINVAL;
    }
    LINUX_LIVE_RETURN_VALIDATED.store(true, Ordering::SeqCst);
    if wait_kind == LINUX_THREAD_WAIT_EPOLL_PWAIT {
        LINUX_EPOLL_PWAIT_BLOCK_COUNT.fetch_add(1, Ordering::SeqCst);
    } else {
        LINUX_EPOLL_WAIT_BLOCK_COUNT.fetch_add(1, Ordering::SeqCst);
    }
    selected.rax
}

fn linux_temporary_signal_mask(input: LinuxSignalMaskInput) -> Result<u64, u64> {
    if input.address == 0 {
        return Ok(ELF_PROBE_SIGNAL_MASK.load(Ordering::SeqCst));
    }
    if input.size != LINUX_SIGNAL_SET_SIZE {
        return Err(LINUX_EINVAL);
    }
    let mut signal_mask = [0_u8; LINUX_SIGNAL_SET_SIZE as usize];
    if !copy_from_linux_user(input.address, &mut signal_mask) {
        return Err(LINUX_EFAULT);
    }
    Ok(u64::from_ne_bytes(signal_mask) & !LINUX_UNBLOCKABLE_SIGNALS)
}

fn linux_epoll_pwait_live_return(
    output: *mut LinuxSyscallFrame,
    caller: LinuxUserContext,
    input: LinuxEpollWaitInput,
    timeout: u64,
    signal_mask: LinuxSignalMaskInput,
) -> u64 {
    let signal_mask = match linux_temporary_signal_mask(signal_mask) {
        Ok(mask) => mask,
        Err(error) => return error,
    };
    let timeout = timeout as u32 as i32;
    let duration = if timeout < 0 {
        None
    } else {
        let Some(duration) = (timeout as u64).checked_mul(1_000_000) else {
            return LINUX_EINVAL;
        };
        Some(duration)
    };
    linux_epoll_wait_duration_live_return(
        output,
        caller,
        input,
        duration,
        LINUX_THREAD_WAIT_EPOLL_PWAIT,
        Some(signal_mask),
    )
}

fn linux_epoll_pwait2_live_return(
    output: *mut LinuxSyscallFrame,
    caller: LinuxUserContext,
    input: LinuxEpollWaitInput,
    timeout_address: u64,
    signal_mask: LinuxSignalMaskInput,
) -> u64 {
    let duration = if timeout_address == 0 {
        None
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
        let Some(duration) = (seconds as u64)
            .checked_mul(NANOSECONDS_PER_SECOND)
            .and_then(|seconds| seconds.checked_add(nanoseconds as u64))
        else {
            return LINUX_EINVAL;
        };
        Some(duration)
    };
    let signal_mask = match linux_temporary_signal_mask(signal_mask) {
        Ok(mask) => mask,
        Err(error) => return error,
    };
    linux_epoll_wait_duration_live_return(
        output,
        caller,
        input,
        duration,
        LINUX_THREAD_WAIT_EPOLL_PWAIT,
        Some(signal_mask),
    )
}

fn linux_descriptor_open(descriptor: u64) -> bool {
    linux_console_descriptor_open(descriptor)
        || linux_device_descriptor(descriptor) != 0
        || linux_epoll_descriptor(descriptor)
        || linux_eventfd_descriptor(descriptor)
        || linux_timerfd_descriptor(descriptor)
        || linux_pipe_descriptor(descriptor)
}

fn linux_epoll_descriptor(descriptor: u64) -> bool {
    descriptor >= LINUX_AUX_DESCRIPTOR_FIRST
        && u64::from(ELF_PROBE_EPOLL_DESCRIPTOR.load(Ordering::SeqCst)) == descriptor
}

fn linux_eventfd_descriptor(descriptor: u64) -> bool {
    descriptor >= LINUX_AUX_DESCRIPTOR_FIRST
        && u64::from(ELF_PROBE_EVENTFD_DESCRIPTOR.load(Ordering::SeqCst)) == descriptor
}

fn linux_timerfd_descriptor(descriptor: u64) -> bool {
    descriptor >= LINUX_AUX_DESCRIPTOR_FIRST
        && u64::from(ELF_PROBE_TIMERFD_DESCRIPTOR.load(Ordering::SeqCst)) == descriptor
}

fn linux_pipe_read_descriptor(descriptor: u64) -> bool {
    descriptor >= LINUX_AUX_DESCRIPTOR_FIRST
        && u64::from(ELF_PROBE_PIPE_READ_DESCRIPTOR.load(Ordering::SeqCst)) == descriptor
}

fn linux_pipe_write_descriptor(descriptor: u64) -> bool {
    descriptor >= LINUX_AUX_DESCRIPTOR_FIRST
        && u64::from(ELF_PROBE_PIPE_WRITE_DESCRIPTOR.load(Ordering::SeqCst)) == descriptor
}

fn linux_pipe_descriptor(descriptor: u64) -> bool {
    linux_pipe_read_descriptor(descriptor) || linux_pipe_write_descriptor(descriptor)
}

fn linux_aux_descriptor_free(descriptor: u64) -> bool {
    (LINUX_AUX_DESCRIPTOR_FIRST..LINUX_AUX_DESCRIPTOR_LIMIT).contains(&descriptor)
        && linux_device_descriptor(descriptor) == 0
        && !linux_epoll_descriptor(descriptor)
        && !linux_eventfd_descriptor(descriptor)
        && !linux_timerfd_descriptor(descriptor)
        && !linux_pipe_descriptor(descriptor)
}

fn linux_descriptor_poll_events(descriptor: i32, requested: u16) -> u16 {
    let Ok(descriptor) = u64::try_from(descriptor) else {
        return 0;
    };
    match u16::from(linux_console_descriptor(descriptor)) {
        LINUX_DESCRIPTOR_INPUT => {
            return if LINUX_RUNTIME_MODE.load(Ordering::SeqCst) {
                requested & LINUX_POLLIN
            } else {
                LINUX_POLLHUP
            };
        }
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
    if linux_timerfd_descriptor(descriptor) {
        let deadline = ELF_PROBE_TIMERFD_DEADLINE.load(Ordering::SeqCst);
        let ready = deadline != 0 && linux_elapsed_nanoseconds().is_some_and(|now| now >= deadline);
        return if ready { requested & LINUX_POLLIN } else { 0 };
    }
    if linux_pipe_read_descriptor(descriptor) {
        let mut returned = 0;
        if ELF_PROBE_PIPE_LENGTH.load(Ordering::SeqCst) != 0 {
            returned |= requested & LINUX_POLLIN;
        }
        if ELF_PROBE_PIPE_WRITE_DESCRIPTOR.load(Ordering::SeqCst) == 0 {
            returned |= LINUX_POLLHUP;
        }
        return returned;
    }
    if linux_pipe_write_descriptor(descriptor) {
        if ELF_PROBE_PIPE_READ_DESCRIPTOR.load(Ordering::SeqCst) == 0 {
            return LINUX_POLLERR;
        }
        return if ELF_PROBE_PIPE_LENGTH.load(Ordering::SeqCst) < LINUX_PIPE_CAPACITY as u32 {
            requested & LINUX_POLLOUT
        } else {
            0
        };
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
    if descriptor == 2 && LINUX_RUNTIME_MODE.load(Ordering::SeqCst) {
        return linux_bounded_console_write(address, count);
    }
    if let Some(result) = linux_eventfd_write(descriptor, address, count) {
        return result;
    }
    if linux_timerfd_descriptor(descriptor) {
        return LINUX_EINVAL;
    }
    if let Some(result) = linux_pipe_write(descriptor, address, count) {
        return result;
    }
    let device = linux_device_descriptor(descriptor);
    if linux_tmp_file(device) {
        return linux_tmp_write(descriptor, device, address, count);
    }
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
    if let Some(result) = linux_pipe_readv(descriptor, vectors, count) {
        return result;
    }
    let device = linux_device_descriptor(descriptor);
    if linux_tmp_file(device) {
        return LINUX_ENOSYS;
    }
    let access = device & 3;
    if access != LINUX_DESCRIPTOR_INPUT
        && access != 3
        && u16::from(linux_console_descriptor(descriptor)) != LINUX_DESCRIPTOR_INPUT
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
    if u16::from(linux_console_descriptor(descriptor)) == LINUX_DESCRIPTOR_INPUT {
        if !LINUX_RUNTIME_MODE.load(Ordering::SeqCst) {
            return 0;
        }
        let Some(byte) = container_console_read_byte() else {
            return 0;
        };
        if !copy_to_linux_user(ranges[0].0, &[byte]) {
            return LINUX_EFAULT;
        }
        return 1;
    }
    if let Some(file) = linux_regular_file(device) {
        let offset = linux_descriptor_offset(descriptor).load(Ordering::SeqCst);
        let Ok(start) = usize::try_from(offset) else {
            return 0;
        };
        let returned = file.len().saturating_sub(start).min(total);
        let mut copied = 0;
        for (address, length) in ranges[..vector_count].iter().copied() {
            let length = length.min(returned - copied);
            if length == 0 {
                break;
            }
            if !copy_to_linux_user(address, &file[start + copied..start + copied + length]) {
                return LINUX_EFAULT;
            }
            copied += length;
        }
        linux_descriptor_offset(descriptor).store((start + returned) as u64, Ordering::SeqCst);
        return returned as u64;
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
    if u16::from(linux_console_descriptor(descriptor)) != LINUX_DESCRIPTOR_OUTPUT {
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
    container_console_write(&bytes[..length]);
    count
}

fn linux_descriptor_writev(descriptor: u64, vectors: u64, count: u64) -> u64 {
    if let Some(result) = linux_pipe_writev(descriptor, vectors, count) {
        return result;
    }
    let device = linux_device_descriptor(descriptor);
    if linux_tmp_file(device) {
        return LINUX_ENOSYS;
    }
    let access = device & 3;
    let writable_device = device != 0 && (access == LINUX_DESCRIPTOR_OUTPUT || access == 3);
    let serial = descriptor == 2 && LINUX_RUNTIME_MODE.load(Ordering::SeqCst)
        || u16::from(linux_console_descriptor(descriptor)) == LINUX_DESCRIPTOR_OUTPUT
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
        container_console_write(&output[..total]);
    }
    total as u64
}

fn container_console_read_byte() -> Option<u8> {
    let slot = active_container_slot()?;
    if !CONTAINER_CONSOLE_ATTACHED[slot].load(Ordering::Acquire) {
        LINUX_RUNTIME_SUSPEND_REQUESTED.store(true, Ordering::Release);
        return None;
    }
    let byte = crate::serial_read_byte_blocking();
    if byte == CONTAINER_CONSOLE_DETACH {
        CONTAINER_CONSOLE_ATTACHED[slot].store(false, Ordering::Release);
        LINUX_RUNTIME_SUSPEND_REQUESTED.store(true, Ordering::Release);
        crate::serial_write_bytes(
            b"\r\n[detached from container; use 'container attach NAME' to return]\r\n",
        );
        None
    } else {
        Some(byte)
    }
}

fn container_console_write(bytes: &[u8]) {
    if !LINUX_RUNTIME_MODE.load(Ordering::Acquire) {
        crate::serial_write_bytes(bytes);
        return;
    }
    let Some(slot) = active_container_slot() else {
        crate::serial_write_bytes(bytes);
        return;
    };
    let start = CONTAINER_CONSOLE_CURSOR[slot].load(Ordering::Relaxed);
    for (offset, byte) in bytes.iter().copied().enumerate() {
        let index = start.wrapping_add(offset as u64) as usize % CONTAINER_CONSOLE_CAPACITY;
        CONTAINER_CONSOLE_BYTES[slot][index].store(byte, Ordering::Relaxed);
    }
    CONTAINER_CONSOLE_CURSOR[slot].store(start.wrapping_add(bytes.len() as u64), Ordering::Release);
    if CONTAINER_CONSOLE_ATTACHED[slot].load(Ordering::Acquire) {
        crate::serial_write_bytes(bytes);
    }
}

fn reset_container_console(slot: usize, attached: bool) {
    CONTAINER_CONSOLE_CURSOR[slot].store(0, Ordering::Release);
    CONTAINER_CONSOLE_ATTACHED[slot].store(attached, Ordering::Release);
}

fn redraw_container_console(slot: usize) {
    crate::serial_write_bytes(b"\x1b[2J\x1b[H");
    let cursor = CONTAINER_CONSOLE_CURSOR[slot].load(Ordering::Acquire);
    let count = cursor.min(CONTAINER_CONSOLE_CAPACITY as u64);
    let start = cursor.wrapping_sub(count);
    for offset in 0..count {
        let index = start.wrapping_add(offset) as usize % CONTAINER_CONSOLE_CAPACITY;
        crate::serial_write_bytes(&[CONTAINER_CONSOLE_BYTES[slot][index].load(Ordering::Relaxed)]);
    }
}

fn linux_console_descriptor_open(descriptor: u64) -> bool {
    linux_console_descriptor(descriptor) != 0
}

fn linux_device_descriptor(descriptor: u64) -> u16 {
    if let Some(state) = usize::try_from(descriptor)
        .ok()
        .and_then(|index| ELF_PROBE_STDIO_DEVICE_DESCRIPTORS.get(index))
    {
        return state.load(Ordering::SeqCst);
    }
    descriptor
        .checked_sub(LINUX_AUX_DESCRIPTOR_FIRST)
        .and_then(|index| ELF_PROBE_DEVICE_DESCRIPTORS.get(index as usize))
        .map(|state| state.load(Ordering::SeqCst))
        .unwrap_or(0)
}

fn linux_tmp_file(state: u16) -> bool {
    state & LINUX_DEVICE_DIRECTORY == 0
        && state & LINUX_FILE_MASK == LINUX_FILE_MASK
        && state & LINUX_ROOT_NODE_MASK == LINUX_TMP_FILE_NODE
}

fn linux_tmp_descriptor_handle(descriptor: u64, state: u16) -> Option<linux_tmpfs::FileHandle> {
    if !linux_tmp_file(state) {
        return None;
    }
    let generation = if let Some(generation) = usize::try_from(descriptor)
        .ok()
        .and_then(|index| ELF_PROBE_STDIO_TMP_FILE_GENERATIONS.get(index))
    {
        generation.load(Ordering::SeqCst)
    } else {
        let index = descriptor.checked_sub(LINUX_AUX_DESCRIPTOR_FIRST)? as usize;
        ELF_PROBE_TMP_FILE_GENERATIONS
            .get(index)?
            .load(Ordering::SeqCst)
    };
    linux_tmpfs::from_generation(generation)
}

fn linux_proc_file(state: u16) -> bool {
    let kind = state & !3;
    matches!(
        kind,
        LINUX_FILE_PROC_STATUS
            | LINUX_FILE_PROC_PID2_STATUS
            | LINUX_FILE_PROC_MOUNTINFO
            | LINUX_FILE_PROC_UID_MAP
            | LINUX_FILE_PROC_GID_MAP
            | LINUX_FILE_PROC_SETGROUPS
            | LINUX_FILE_PROC_CPUINFO
            | LINUX_FILE_PROC_MEMINFO
            | LINUX_FILE_PROC_STAT
            | LINUX_FILE_PROC_UPTIME
            | LINUX_FILE_PROC_LOADAVG
            | LINUX_FILE_PROC_FILESYSTEMS
            | LINUX_FILE_PROC_CMDLINE
            | LINUX_FILE_PROC_PID1_STAT
            | LINUX_FILE_PROC_PID1_STATM
            | LINUX_FILE_PROC_PID2_STAT
            | LINUX_FILE_PROC_PID2_STATM
    )
}

fn linux_mountinfo_file(state: u16) -> bool {
    state & !3 == LINUX_FILE_PROC_MOUNTINFO
}

fn linux_user_namespace_file(state: u16) -> Option<&'static [u8]> {
    current_linux_user_namespace()?;
    match state & !3 {
        LINUX_FILE_PROC_UID_MAP => Some(linux_user::UID_MAP),
        LINUX_FILE_PROC_GID_MAP => Some(linux_user::GID_MAP),
        LINUX_FILE_PROC_SETGROUPS => Some(linux_user::SETGROUPS),
        _ => None,
    }
}

fn linux_regular_file(state: u16) -> Option<&'static [u8]> {
    if !linux_mount::current_root_is_read_only() {
        return None;
    }
    if state & LINUX_DEVICE_DIRECTORY != 0 {
        return None;
    }
    if linux_mountinfo_file(state) {
        return linux_mount::current_mountinfo();
    }
    if let Some(file) = linux_user_namespace_file(state) {
        return Some(file);
    }
    if state & !3 == LINUX_FILE_PROC_STATUS {
        return Some(LINUX_PROC_PID1_STATUS);
    }
    if state & !3 == LINUX_FILE_PROC_PID2_STATUS {
        let status = linux_process::live_process_status(2)?;
        if status.pid != 2 || status.parent_pid != 1 {
            return None;
        }
        return match status.state {
            linux_process::LiveProcessState::Running => Some(LINUX_PROC_PID2_RUNNING_STATUS),
            linux_process::LiveProcessState::Zombie => Some(LINUX_PROC_PID2_ZOMBIE_STATUS),
        };
    }
    match state & !3 {
        LINUX_FILE_PROC_CPUINFO => return Some(LINUX_PROC_CPUINFO),
        LINUX_FILE_PROC_MEMINFO => return Some(LINUX_PROC_MEMINFO),
        LINUX_FILE_PROC_STAT => return Some(LINUX_PROC_STAT),
        LINUX_FILE_PROC_UPTIME => return Some(LINUX_PROC_UPTIME),
        LINUX_FILE_PROC_LOADAVG => return Some(LINUX_PROC_LOADAVG),
        LINUX_FILE_PROC_FILESYSTEMS => return Some(LINUX_PROC_FILESYSTEMS),
        LINUX_FILE_PROC_CMDLINE => return Some(LINUX_PROC_CMDLINE),
        LINUX_FILE_PROC_PID1_STAT => return Some(LINUX_PROC_PID1_STAT),
        LINUX_FILE_PROC_PID1_STATM => return Some(LINUX_PROC_PID1_STATM),
        LINUX_FILE_PROC_PID2_STATM => {
            return linux_process::live_process_status(2).map(|_| LINUX_PROC_PID2_STATM)
        }
        LINUX_FILE_PROC_PID2_STAT => {
            let status = linux_process::live_process_status(2)?;
            return match status.state {
                linux_process::LiveProcessState::Running => Some(LINUX_PROC_PID2_RUNNING_STAT),
                linux_process::LiveProcessState::Zombie => Some(LINUX_PROC_PID2_ZOMBIE_STAT),
            };
        }
        _ => {}
    }
    if linux_tmp_file(state) {
        return None;
    }
    let node = ((state & LINUX_ROOT_NODE_MASK) >> LINUX_ROOT_NODE_SHIFT) as u8;
    if node != 0 {
        return initramfs::published_file_by_node(node);
    }
    None
}

fn linux_regular_file_inode(state: u16) -> Option<u64> {
    if state & LINUX_DEVICE_DIRECTORY != 0 {
        return None;
    }
    if state & !3 == LINUX_FILE_PROC_STATUS {
        return Some(17);
    }
    if state & !3 == LINUX_FILE_PROC_PID2_STATUS {
        return linux_process::live_process_status(2).map(|_| 24);
    }
    if linux_mountinfo_file(state) {
        return Some(19);
    }
    match state & !3 {
        LINUX_FILE_PROC_UID_MAP => return Some(20),
        LINUX_FILE_PROC_GID_MAP => return Some(21),
        LINUX_FILE_PROC_SETGROUPS => return Some(22),
        _ => {}
    }
    match state & !3 {
        LINUX_FILE_PROC_CPUINFO => return Some(25),
        LINUX_FILE_PROC_MEMINFO => return Some(26),
        LINUX_FILE_PROC_STAT => return Some(27),
        LINUX_FILE_PROC_UPTIME => return Some(28),
        LINUX_FILE_PROC_LOADAVG => return Some(29),
        LINUX_FILE_PROC_FILESYSTEMS => return Some(30),
        LINUX_FILE_PROC_CMDLINE => return Some(31),
        LINUX_FILE_PROC_PID1_STAT => return Some(32),
        LINUX_FILE_PROC_PID1_STATM => return Some(33),
        LINUX_FILE_PROC_PID2_STAT => return linux_process::live_process_status(2).map(|_| 34),
        LINUX_FILE_PROC_PID2_STATM => return linux_process::live_process_status(2).map(|_| 35),
        _ => {}
    }
    if linux_tmp_file(state) {
        return linux_tmpfs::open(b"ginux-ready").map(linux_tmpfs::inode);
    }
    match state & LINUX_FILE_MASK {
        LINUX_FILE_INIT => Some(5),
        LINUX_FILE_INTERPRETER => Some(6),
        LINUX_FILE_DEPENDENCY => Some(7),
        LINUX_FILE_AUXILIARY_DEPENDENCY => Some(8),
        LINUX_FILE_ROOT_HOSTNAME => Some(9),
        LINUX_FILE_ROOT_GENERIC => {
            let node = (state & LINUX_ROOT_NODE_MASK) >> LINUX_ROOT_NODE_SHIFT;
            (node != 0).then_some(32 + u64::from(node))
        }
        _ => None,
    }
}

fn linux_regular_file_seek(descriptor: u64, state: u16, offset: u64, whence: u64) -> u64 {
    let base = match whence {
        0 => 0_i128,
        1 => i128::from(linux_descriptor_offset(descriptor).load(Ordering::SeqCst)),
        2 => i128::from(
            linux_oci_entry(descriptor)
                .map_or_else(|| linux_file_size(state), |entry| entry.data.len() as u64),
        ),
        _ => return LINUX_EINVAL,
    };
    let next = base + i128::from(offset as i64);
    let Ok(next) = u64::try_from(next) else {
        return LINUX_EINVAL;
    };
    linux_descriptor_offset(descriptor).store(next, Ordering::SeqCst);
    next
}

fn linux_tmp_read(descriptor: u64, state: u16, address: u64, count: u64) -> u64 {
    if state & 3 != LINUX_DESCRIPTOR_INPUT && state & 3 != 3 {
        return LINUX_EBADF;
    }
    let Some(handle) = linux_tmp_descriptor_handle(descriptor, state) else {
        return LINUX_EBADF;
    };
    let Ok(start) = usize::try_from(linux_descriptor_offset(descriptor).load(Ordering::SeqCst))
    else {
        return LINUX_EINVAL;
    };
    let requested = usize::try_from(count)
        .unwrap_or(usize::MAX)
        .min(linux_tmpfs::FILE_CAPACITY);
    let mut output = [0_u8; linux_tmpfs::FILE_CAPACITY];
    let Some(length) = linux_tmpfs::read(handle, start, &mut output[..requested]) else {
        return LINUX_EBADF;
    };
    if !copy_to_linux_user(address, &output[..length]) {
        return LINUX_EFAULT;
    }
    linux_descriptor_offset(descriptor).store((start + length) as u64, Ordering::SeqCst);
    length as u64
}

fn linux_tmp_write(descriptor: u64, state: u16, address: u64, count: u64) -> u64 {
    if state & 3 != LINUX_DESCRIPTOR_OUTPUT && state & 3 != 3 {
        return LINUX_EBADF;
    }
    let Ok(start) = usize::try_from(linux_descriptor_offset(descriptor).load(Ordering::SeqCst))
    else {
        return LINUX_EINVAL;
    };
    let Ok(length) = usize::try_from(count) else {
        return LINUX_EINVAL;
    };
    let Some(end) = start.checked_add(length) else {
        return LINUX_EINVAL;
    };
    if end > linux_tmpfs::FILE_CAPACITY {
        return LINUX_ENOSPC;
    }
    if length == 0 {
        return 0;
    }
    let Some(handle) = linux_tmp_descriptor_handle(descriptor, state) else {
        return LINUX_EBADF;
    };
    let mut input = [0_u8; linux_tmpfs::FILE_CAPACITY];
    if !copy_from_linux_user(address, &mut input[..length]) {
        return LINUX_EFAULT;
    }
    if linux_tmpfs::write(handle, start, &input[..length]) != Some(length) {
        return LINUX_EBADF;
    }
    linux_descriptor_offset(descriptor).store(end as u64, Ordering::SeqCst);
    count
}

fn linux_descriptor_offset(descriptor: u64) -> &'static BankedAtomicU64 {
    if descriptor < 2 {
        return &ELF_PROBE_STDIO_OFFSETS[descriptor as usize];
    }
    &ELF_PROBE_DESCRIPTOR_OFFSETS[(descriptor - LINUX_AUX_DESCRIPTOR_FIRST) as usize]
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
    linux_descriptor_close_with_wake(descriptor, true)
}

fn linux_close_all_descriptors() -> bool {
    for descriptor in 0..LINUX_DESCRIPTOR_TABLE_LIMIT {
        if linux_descriptor_open(descriptor) && linux_descriptor_close(descriptor) != 0 {
            return false;
        }
    }
    (0..LINUX_DESCRIPTOR_TABLE_LIMIT).all(|descriptor| !linux_descriptor_open(descriptor))
}

fn linux_descriptor_close_with_wake(descriptor: u64, wake_waiters: bool) -> u64 {
    if descriptor < 2 && linux_device_descriptor(descriptor) != 0 {
        let index = descriptor as usize;
        ELF_PROBE_STDIO_DEVICE_DESCRIPTORS[index].store(0, Ordering::SeqCst);
        ELF_PROBE_STDIO_OCI_DESCRIPTORS[index].store(0, Ordering::SeqCst);
        ELF_PROBE_STDIO_TMP_FILE_GENERATIONS[index].store(0, Ordering::SeqCst);
        ELF_PROBE_STDIO_OFFSETS[index].store(0, Ordering::SeqCst);
        ELF_PROBE_CONSOLE_CLOEXEC.fetch_and(!(1 << descriptor), Ordering::SeqCst);
        return 0;
    }
    if !(LINUX_AUX_DESCRIPTOR_FIRST..LINUX_DESCRIPTOR_TABLE_LIMIT).contains(&descriptor) {
        return linux_console_close(descriptor);
    }
    if linux_epoll_descriptor(descriptor) {
        ELF_PROBE_EPOLL_DESCRIPTOR.store(0, Ordering::SeqCst);
        ELF_PROBE_EPOLL_TARGET.store(u64::MAX, Ordering::SeqCst);
        ELF_PROBE_EPOLL_EVENTS.store(0, Ordering::SeqCst);
        ELF_PROBE_EPOLL_DATA.store(0, Ordering::SeqCst);
        ELF_PROBE_CONSOLE_CLOEXEC.fetch_and(!(1 << descriptor), Ordering::SeqCst);
        if wake_waiters {
            wake_linux_descriptor_waits();
        }
        return 0;
    }
    if linux_eventfd_descriptor(descriptor) {
        ELF_PROBE_EVENTFD_DESCRIPTOR.store(0, Ordering::SeqCst);
        ELF_PROBE_EVENTFD_COUNTER.store(0, Ordering::SeqCst);
        ELF_PROBE_EVENTFD_FLAGS.store(0, Ordering::SeqCst);
        ELF_PROBE_CONSOLE_CLOEXEC.fetch_and(!(1 << descriptor), Ordering::SeqCst);
        if wake_waiters {
            wake_linux_descriptor_waits();
        }
        return 0;
    }
    if linux_timerfd_descriptor(descriptor) {
        ELF_PROBE_TIMERFD_DESCRIPTOR.store(0, Ordering::SeqCst);
        ELF_PROBE_TIMERFD_FLAGS.store(0, Ordering::SeqCst);
        ELF_PROBE_TIMERFD_INTERVAL.store(0, Ordering::SeqCst);
        ELF_PROBE_TIMERFD_DEADLINE.store(0, Ordering::SeqCst);
        ELF_PROBE_CONSOLE_CLOEXEC.fetch_and(!(1 << descriptor), Ordering::SeqCst);
        if wake_waiters {
            wake_linux_descriptor_waits();
        }
        return 0;
    }
    if linux_pipe_read_descriptor(descriptor) {
        ELF_PROBE_PIPE_READ_DESCRIPTOR.store(0, Ordering::SeqCst);
        ELF_PROBE_PIPE_READ_FLAGS.store(0, Ordering::SeqCst);
        ELF_PROBE_PIPE_LENGTH.store(0, Ordering::SeqCst);
        for byte in &ELF_PROBE_PIPE_BUFFER {
            byte.store(0, Ordering::SeqCst);
        }
        ELF_PROBE_CONSOLE_CLOEXEC.fetch_and(!(1 << descriptor), Ordering::SeqCst);
        if wake_waiters {
            wake_linux_descriptor_waits();
        }
        return 0;
    }
    if linux_pipe_write_descriptor(descriptor) {
        ELF_PROBE_PIPE_WRITE_DESCRIPTOR.store(0, Ordering::SeqCst);
        ELF_PROBE_PIPE_WRITE_FLAGS.store(0, Ordering::SeqCst);
        if ELF_PROBE_PIPE_READ_DESCRIPTOR.load(Ordering::SeqCst) == 0 {
            ELF_PROBE_PIPE_LENGTH.store(0, Ordering::SeqCst);
        }
        ELF_PROBE_CONSOLE_CLOEXEC.fetch_and(!(1 << descriptor), Ordering::SeqCst);
        if wake_waiters {
            wake_linux_descriptor_waits();
        }
        return 0;
    }
    if ELF_PROBE_DEVICE_DESCRIPTORS[(descriptor - LINUX_AUX_DESCRIPTOR_FIRST) as usize]
        .swap(0, Ordering::SeqCst)
        == 0
    {
        return LINUX_EBADF;
    }
    ELF_PROBE_OCI_DESCRIPTORS[(descriptor - LINUX_AUX_DESCRIPTOR_FIRST) as usize]
        .store(0, Ordering::SeqCst);
    ELF_PROBE_TMP_FILE_GENERATIONS[(descriptor - LINUX_AUX_DESCRIPTOR_FIRST) as usize]
        .store(0, Ordering::SeqCst);
    linux_descriptor_offset(descriptor).store(0, Ordering::SeqCst);
    ELF_PROBE_CONSOLE_CLOEXEC.fetch_and(!(1 << descriptor), Ordering::SeqCst);
    0
}

fn linux_descriptor_close_live_return(
    output: *mut LinuxSyscallFrame,
    caller: LinuxUserContext,
    descriptor: u64,
) -> u64 {
    let closes_pipe_reader = linux_pipe_read_descriptor(descriptor);
    let blocked_writer = closes_pipe_reader.then(blocked_linux_pipe_writer).flatten();
    let result = linux_descriptor_close_with_wake(descriptor, false);
    if result != 0 {
        return result;
    }
    let Some(target) = blocked_writer else {
        wake_linux_descriptor_waits();
        return 0;
    };
    let slot = usize::from(target.slot);
    let signal_bit = 1 << (LINUX_SIGPIPE - 1);
    let signal_mask = LINUX_THREAD_SIGNAL_MASKS[slot].load(Ordering::SeqCst);
    let index = LINUX_SIGPIPE as usize - 1;
    let handler = ELF_PROBE_SIGNAL_HANDLERS[index].load(Ordering::SeqCst);
    if signal_mask & signal_bit != 0 {
        ELF_PROBE_PENDING_SIGNAL_CODES[index].store(LINUX_SI_KERNEL as u32, Ordering::SeqCst);
        ELF_PROBE_PENDING_SIGNALS.fetch_or(signal_bit, Ordering::SeqCst);
        wake_linux_descriptor_waits();
        return 0;
    }
    if handler == 1 {
        wake_linux_descriptor_waits();
        return 0;
    }
    if handler == 0 {
        return linux_terminate_by_default_signal(LINUX_SIGPIPE);
    }
    let delivered = linux_deliver_signal_to_blocked_wait(
        target,
        (LINUX_SIGPIPE, LINUX_SI_KERNEL),
        output,
        caller,
        LINUX_EPIPE,
        true,
    );
    if delivered != 0 {
        ELF_PROBE_PENDING_SIGNAL_CODES[index].store(LINUX_SI_KERNEL as u32, Ordering::SeqCst);
        ELF_PROBE_PENDING_SIGNALS.fetch_or(signal_bit, Ordering::SeqCst);
    }
    wake_linux_descriptor_waits();
    0
}

fn linux_console_dup(descriptor: u64) -> u64 {
    linux_console_dup_at_least(descriptor, 0, false)
}

fn linux_console_dup_at_least(descriptor: u64, minimum: u64, close_on_exec: bool) -> u64 {
    if minimum >= 2 {
        if !LINUX_RUNTIME_MODE.load(Ordering::SeqCst) {
            return LINUX_EINVAL;
        }
        let state = linux_console_descriptor(descriptor);
        if state == 0 || minimum >= LINUX_DESCRIPTOR_TABLE_LIMIT {
            return if state == 0 {
                LINUX_EBADF
            } else {
                LINUX_EINVAL
            };
        }
        let Some(target) =
            (minimum..LINUX_DESCRIPTOR_TABLE_LIMIT).find(|target| !linux_descriptor_open(*target))
        else {
            return LINUX_EMFILE;
        };
        let index = (target - LINUX_AUX_DESCRIPTOR_FIRST) as usize;
        ELF_PROBE_DEVICE_DESCRIPTORS[index]
            .store(LINUX_DEVICE_CONSOLE | u16::from(state), Ordering::SeqCst);
        if close_on_exec {
            ELF_PROBE_CONSOLE_CLOEXEC.fetch_or(1 << target, Ordering::SeqCst);
        }
        return target;
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
    let device = linux_device_descriptor(source);
    if device != 0 && target < 2 {
        if source == target {
            return target;
        }
        if linux_descriptor_open(target) && linux_descriptor_close(target) != 0 {
            return LINUX_EBADF;
        }
        let source_offset = linux_descriptor_offset(source).load(Ordering::SeqCst);
        let source_oci = linux_oci_descriptor(source);
        let source_tmp = source
            .checked_sub(LINUX_AUX_DESCRIPTOR_FIRST)
            .and_then(|index| ELF_PROBE_TMP_FILE_GENERATIONS.get(index as usize))
            .map(|generation| generation.load(Ordering::SeqCst))
            .unwrap_or(0);
        let index = target as usize;
        if device & LINUX_DEVICE_CONSOLE != 0 {
            let shift = target * 2;
            ELF_PROBE_CONSOLE_DESCRIPTORS.fetch_or((device as u8 & 3) << shift, Ordering::SeqCst);
            ELF_PROBE_CONSOLE_CLOEXEC.fetch_and(!(1 << target), Ordering::SeqCst);
            return target;
        }
        ELF_PROBE_STDIO_DEVICE_DESCRIPTORS[index].store(device, Ordering::SeqCst);
        ELF_PROBE_STDIO_OCI_DESCRIPTORS[index].store(source_oci, Ordering::SeqCst);
        ELF_PROBE_STDIO_TMP_FILE_GENERATIONS[index].store(source_tmp, Ordering::SeqCst);
        ELF_PROBE_STDIO_OFFSETS[index].store(source_offset, Ordering::SeqCst);
        ELF_PROBE_CONSOLE_CLOEXEC.fetch_and(!(1 << target), Ordering::SeqCst);
        return target;
    }
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

fn linux_device_dup_at_least(source: u64, minimum: u64, close_on_exec: bool) -> u64 {
    let state = linux_device_descriptor(source);
    if state == 0 || minimum >= LINUX_DESCRIPTOR_TABLE_LIMIT {
        return if state == 0 {
            LINUX_EBADF
        } else {
            LINUX_EINVAL
        };
    }
    let Some(target) = (minimum..LINUX_DESCRIPTOR_TABLE_LIMIT)
        .find(|descriptor| !linux_descriptor_open(*descriptor))
    else {
        return LINUX_EMFILE;
    };
    let source_oci = linux_oci_descriptor(source);
    let source_tmp = source
        .checked_sub(LINUX_AUX_DESCRIPTOR_FIRST)
        .and_then(|index| ELF_PROBE_TMP_FILE_GENERATIONS.get(index as usize))
        .map(|generation| generation.load(Ordering::SeqCst))
        .unwrap_or(0);
    let target_index = (target - LINUX_AUX_DESCRIPTOR_FIRST) as usize;
    ELF_PROBE_DEVICE_DESCRIPTORS[target_index].store(state, Ordering::SeqCst);
    ELF_PROBE_OCI_DESCRIPTORS[target_index].store(source_oci, Ordering::SeqCst);
    ELF_PROBE_TMP_FILE_GENERATIONS[target_index].store(source_tmp, Ordering::SeqCst);
    linux_descriptor_offset(target).store(
        linux_descriptor_offset(source).load(Ordering::SeqCst),
        Ordering::SeqCst,
    );
    if close_on_exec {
        ELF_PROBE_CONSOLE_CLOEXEC.fetch_or(1 << target, Ordering::SeqCst);
    }
    target
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
        LINUX_F_GETFL if u16::from(state) == LINUX_DESCRIPTOR_INPUT => 0,
        LINUX_F_GETFL if u16::from(state) == LINUX_DESCRIPTOR_OUTPUT => 1,
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
                LINUX_F_SETFL => {
                    linux_descriptor_set_nonblocking(&ELF_PROBE_EVENTFD_FLAGS, argument)
                }
                _ => LINUX_EINVAL,
            };
        }
        if linux_timerfd_descriptor(descriptor) {
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
                        ELF_PROBE_TIMERFD_FLAGS.load(Ordering::SeqCst) & LINUX_TFD_NONBLOCK as u32,
                    )
                }
                LINUX_F_SETFL => {
                    linux_descriptor_set_nonblocking(&ELF_PROBE_TIMERFD_FLAGS, argument)
                }
                _ => LINUX_EINVAL,
            };
        }
        if linux_pipe_descriptor(descriptor) {
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
                    let access = u64::from(linux_pipe_write_descriptor(descriptor));
                    let flags = if access == 0 {
                        &ELF_PROBE_PIPE_READ_FLAGS
                    } else {
                        &ELF_PROBE_PIPE_WRITE_FLAGS
                    };
                    access | u64::from(flags.load(Ordering::SeqCst) & LINUX_O_NONBLOCK as u32)
                }
                LINUX_F_SETFL => {
                    let flags = if linux_pipe_read_descriptor(descriptor) {
                        &ELF_PROBE_PIPE_READ_FLAGS
                    } else {
                        &ELF_PROBE_PIPE_WRITE_FLAGS
                    };
                    linux_descriptor_set_nonblocking(flags, argument)
                }
                _ => LINUX_EINVAL,
            };
        }
        return linux_console_fcntl(descriptor, command, argument);
    }
    match command {
        LINUX_F_DUPFD if LINUX_RUNTIME_MODE.load(Ordering::SeqCst) => {
            linux_device_dup_at_least(descriptor, argument, false)
        }
        LINUX_F_DUPFD_CLOEXEC if LINUX_RUNTIME_MODE.load(Ordering::SeqCst) => {
            linux_device_dup_at_least(descriptor, argument, true)
        }
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

fn linux_descriptor_set_nonblocking(flags: &BankedAtomicU32, argument: u64) -> u64 {
    if argument & !(3 | LINUX_O_NONBLOCK) != 0 {
        return LINUX_EINVAL;
    }
    let current = flags.load(Ordering::SeqCst);
    flags.store(
        (current & !(LINUX_O_NONBLOCK as u32)) | (argument as u32 & LINUX_O_NONBLOCK as u32),
        Ordering::SeqCst,
    );
    0
}

fn linux_getdents64(descriptor: u64, address: u64, count: u64) -> u64 {
    let state = linux_device_descriptor(descriptor);
    if state & LINUX_DEVICE_DIRECTORY == 0 {
        return LINUX_ENOTDIR;
    }
    let offset = linux_descriptor_offset(descriptor).load(Ordering::SeqCst);
    let Ok(capacity) = usize::try_from(count) else {
        return LINUX_EINVAL;
    };
    let mut entries = [0_u8; PAGE_SIZE];
    let output_capacity = capacity.min(entries.len());
    let mut used = 0_usize;
    let next_offset;
    let oci_directory = linux_oci_descriptor(descriptor);
    if oci_directory != 0 {
        let initial = offset == 0;
        let mut cursor = if initial {
            0
        } else {
            let Some(cursor) = offset
                .checked_sub(2)
                .and_then(|cursor| usize::try_from(cursor).ok())
            else {
                return LINUX_EINVAL;
            };
            cursor
        };
        if initial {
            for (inode, next, name) in [
                (linux_oci_inode(oci_directory), 1_i64, &b"."[..]),
                (
                    linux_oci_inode(crate::oci_image::ROOT_HANDLE),
                    2,
                    &b".."[..],
                ),
            ] {
                if !append_linux_dirent(
                    &mut entries[..output_capacity],
                    &mut used,
                    inode,
                    next,
                    4,
                    name,
                ) {
                    return LINUX_EINVAL;
                }
            }
        }
        loop {
            let Some(child) = crate::oci_image::next_child(oci_directory, cursor) else {
                return LINUX_EINVAL;
            };
            let Some(child) = child else {
                break;
            };
            let kind = match child.entry.kind {
                ginux_oci::EntryKind::Directory => 4,
                ginux_oci::EntryKind::File => 8,
                ginux_oci::EntryKind::Hardlink => 8,
                ginux_oci::EntryKind::Symlink => 10,
            };
            let Ok(next) = i64::try_from(child.next_cursor + 2) else {
                return LINUX_EINVAL;
            };
            if !append_linux_dirent(
                &mut entries[..output_capacity],
                &mut used,
                linux_oci_inode(child.handle),
                next,
                kind,
                child.name,
            ) {
                if used == 0 {
                    return LINUX_EINVAL;
                }
                break;
            }
            cursor = child.next_cursor;
        }
        if used == 0 {
            return 0;
        }
        next_offset = u64::try_from(cursor + 2).unwrap_or(u64::MAX);
    } else if let Some(directory) = linux_measured_directory_path(state) {
        let directory_inode = linux_root_directory_inode(state).unwrap_or(0);
        let parent_inode = if directory.is_empty() {
            directory_inode
        } else if directory == b"proc/1" || directory == b"proc/2" {
            initramfs::published_directory_node(b"proc")
                .map(|node| 64 + u64::from(node))
                .unwrap_or(11)
        } else {
            11
        };
        let initial = offset == 0;
        let mut cursor = if initial {
            0
        } else {
            let Some(cursor) = offset
                .checked_sub(2)
                .and_then(|cursor| usize::try_from(cursor).ok())
            else {
                return LINUX_EINVAL;
            };
            cursor
        };
        if initial {
            for (inode, next, name) in [
                (directory_inode, 1_i64, &b"."[..]),
                (parent_inode, 2, &b".."[..]),
            ] {
                if !append_linux_dirent(
                    &mut entries[..output_capacity],
                    &mut used,
                    inode,
                    next,
                    4,
                    name,
                ) {
                    return LINUX_EINVAL;
                }
            }
        }
        if directory == b"proc/1" {
            if cursor > 7 {
                return LINUX_EINVAL;
            }
            for (index, inode, name) in [
                (0_usize, 17_u64, &b"status"[..]),
                (1_usize, 19_u64, &b"mountinfo"[..]),
                (2_usize, 20_u64, &b"uid_map"[..]),
                (3_usize, 21_u64, &b"gid_map"[..]),
                (4_usize, 22_u64, &b"setgroups"[..]),
                (5_usize, 32_u64, &b"stat"[..]),
                (6_usize, 33_u64, &b"statm"[..]),
            ] {
                if cursor == index {
                    if !append_linux_dirent(
                        &mut entries[..output_capacity],
                        &mut used,
                        inode,
                        (index + 3) as i64,
                        8,
                        name,
                    ) {
                        if used == 0 {
                            return LINUX_EINVAL;
                        }
                        break;
                    }
                    cursor += 1;
                }
            }
        } else if directory == b"proc/2" {
            if cursor > 3 {
                return LINUX_EINVAL;
            }
            if linux_process::live_process_status(2).is_none() {
                return LINUX_ENOENT;
            }
            for (index, inode, name) in [
                (0_usize, 24_u64, &b"status"[..]),
                (1_usize, 34_u64, &b"stat"[..]),
                (2_usize, 35_u64, &b"statm"[..]),
            ] {
                if cursor == index {
                    if !append_linux_dirent(
                        &mut entries[..output_capacity],
                        &mut used,
                        inode,
                        (index + 3) as i64,
                        8,
                        name,
                    ) {
                        if used == 0 {
                            return LINUX_EINVAL;
                        }
                        break;
                    }
                    cursor += 1;
                }
            }
        } else if directory == b"proc" {
            let child_live = linux_process::live_process_status(2).is_some();
            let mut full = false;
            let root_files = [
                (25_u64, &b"cpuinfo"[..]),
                (26_u64, &b"meminfo"[..]),
                (27_u64, &b"stat"[..]),
                (28_u64, &b"uptime"[..]),
                (29_u64, &b"loadavg"[..]),
                (30_u64, &b"filesystems"[..]),
                (31_u64, &b"cmdline"[..]),
            ];
            let limit = root_files.len() + 1 + usize::from(child_live);
            if cursor > limit {
                return LINUX_EINVAL;
            }
            for (index, (inode, name)) in root_files.iter().enumerate() {
                if cursor == index {
                    if !append_linux_dirent(
                        &mut entries[..output_capacity],
                        &mut used,
                        *inode,
                        (index + 3) as i64,
                        8,
                        name,
                    ) {
                        if used == 0 {
                            return LINUX_EINVAL;
                        }
                        full = true;
                        break;
                    }
                    cursor += 1;
                }
            }
            if !full {
                for (index, inode, name) in [
                    (root_files.len(), 16_u64, &b"1"[..]),
                    (root_files.len() + 1, 23_u64, &b"2"[..]),
                ] {
                    if cursor == index && (name == b"1" || child_live) {
                        if !append_linux_dirent(
                            &mut entries[..output_capacity],
                            &mut used,
                            inode,
                            (index + 3) as i64,
                            4,
                            name,
                        ) {
                            if used == 0 {
                                return LINUX_EINVAL;
                            }
                            break;
                        }
                        cursor += 1;
                    }
                }
            }
        } else if directory == b"tmp" {
            if cursor > 1 {
                return LINUX_EINVAL;
            }
            if cursor == 0 {
                let entry = linux_tmpfs::entry()
                    .map(|(handle, name)| (linux_tmpfs::inode(handle), 8, name));
                if let Some((inode, kind, name)) = entry {
                    if !append_linux_dirent(
                        &mut entries[..output_capacity],
                        &mut used,
                        inode,
                        3,
                        kind,
                        name,
                    ) {
                        return LINUX_EINVAL;
                    }
                    cursor = 1;
                }
            }
        } else {
            loop {
                let Some(child) = initramfs::published_next_child(directory, cursor) else {
                    return LINUX_EINVAL;
                };
                let Some(child) = child else {
                    break;
                };
                let kind = match child.mode & 0o170000 {
                    0o040000 => 4,
                    0o100000 => 8,
                    0o120000 => 10,
                    _ => return LINUX_EINVAL,
                };
                let child_inode = match (directory, child.name) {
                    (b"", b"dev") => 4,
                    (b"", b"etc") => 10,
                    (b"", b"proc") => 12,
                    (b"", b"sys") => 13,
                    (b"", b"tmp") => 14,
                    (b"", b"init") => 5,
                    (b"etc", b"hostname") => 9,
                    _ => {
                        let Ok(node) = u64::try_from(child.next_cursor) else {
                            return LINUX_EINVAL;
                        };
                        let Some(inode) = 32_u64.checked_add(node) else {
                            return LINUX_EINVAL;
                        };
                        inode
                    }
                };
                let Some(cookie) = child.next_cursor.checked_add(2) else {
                    return LINUX_EINVAL;
                };
                let Ok(next) = i64::try_from(cookie) else {
                    return LINUX_EINVAL;
                };
                if !append_linux_dirent(
                    &mut entries[..output_capacity],
                    &mut used,
                    child_inode,
                    next,
                    kind,
                    child.name,
                ) {
                    if used == 0 {
                        return LINUX_EINVAL;
                    }
                    break;
                }
                cursor = child.next_cursor;
            }
        }
        if used == 0 {
            return 0;
        }
        let Some(cookie) = cursor.checked_add(2) else {
            return LINUX_EINVAL;
        };
        let Ok(cookie) = u64::try_from(cookie) else {
            return LINUX_EINVAL;
        };
        next_offset = cookie;
    } else {
        if offset != 0 {
            return 0;
        }
        for (inode, next, kind, name) in [
            (4_u64, 1_i64, 4_u8, &b"."[..]),
            (4, 2, 4, &b".."[..]),
            (1, 3, 2, &b"null"[..]),
            (2, 4, 2, &b"zero"[..]),
            (3, 5, 2, &b"console"[..]),
            (3, 6, 2, &b"tty"[..]),
        ] {
            if !append_linux_dirent(
                &mut entries[..output_capacity],
                &mut used,
                inode,
                next,
                kind,
                name,
            ) {
                return LINUX_EINVAL;
            }
        }
        next_offset = 1;
    }
    if capacity < used {
        return LINUX_EINVAL;
    }
    if !copy_to_linux_user(address, &entries[..used]) {
        return LINUX_EFAULT;
    }
    linux_descriptor_offset(descriptor).store(next_offset, Ordering::SeqCst);
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
        LINUX_RLIMIT_NOFILE if LINUX_RUNTIME_MODE.load(Ordering::SeqCst) => {
            LINUX_DESCRIPTOR_TABLE_LIMIT
        }
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
    if linux_pipe_descriptor(descriptor) {
        return Some(linux_stat(LINUX_STAT_FIFO_MODE, 3));
    }
    if linux_epoll_descriptor(descriptor)
        || linux_eventfd_descriptor(descriptor)
        || linux_timerfd_descriptor(descriptor)
    {
        return Some(linux_stat(LINUX_STAT_ANONYMOUS_MODE, 4));
    }
    let oci_handle = linux_oci_descriptor(descriptor);
    if oci_handle != 0 {
        let entry = (oci_handle != crate::oci_image::ROOT_HANDLE)
            .then(|| crate::oci_image::entry(oci_handle))
            .flatten();
        return Some(linux_oci_stat(oci_handle, entry));
    }
    let device = linux_device_descriptor(descriptor);
    (device != 0).then(|| {
        if device & LINUX_DEVICE_DIRECTORY != 0 {
            let inode = linux_root_directory_inode(device).unwrap_or(0);
            linux_stat(LINUX_STAT_DIRECTORY_MODE, inode)
        } else if device & LINUX_FILE_MASK != 0 {
            linux_regular_file_stat(device)
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

fn linux_regular_file_stat(state: u16) -> [u8; LINUX_STAT_SIZE] {
    let mut stat = linux_stat(
        linux_regular_file_mode(state),
        linux_regular_file_inode(state).unwrap_or(0),
    );
    let size = linux_file_size(state);
    stat[48..56].copy_from_slice(&size.to_ne_bytes());
    stat[64..72].copy_from_slice(&size.div_ceil(512).to_ne_bytes());
    stat
}

fn linux_regular_file_mode(state: u16) -> u32 {
    if linux_tmp_file(state) {
        0o100600
    } else if linux_proc_file(state) {
        0o100444
    } else {
        LINUX_STAT_REGULAR_MODE
    }
}

fn linux_file_size(state: u16) -> u64 {
    if linux_tmp_file(state) {
        linux_tmpfs::open(b"ginux-ready")
            .and_then(linux_tmpfs::size)
            .map_or(0, |size| size as u64)
    } else {
        linux_regular_file(state).map_or(0, |file| file.len() as u64)
    }
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
    linux_user_range_covered(address, length, true)
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
    linux_user_range_covered(address, length, false)
}

fn linux_user_area_end(base: u64, size: u64, address: u64) -> Option<u64> {
    let end = base.checked_add(size)?;
    (size != 0 && (base..end).contains(&address)).then_some(end)
}

fn extend_linux_user_area(next: &mut u64, base: u64, size: u64, address: u64) {
    if let Some(end) = linux_user_area_end(base, size, address) {
        *next = (*next).max(end);
    }
}

fn linux_user_range_covered(address: u64, length: usize, writable: bool) -> bool {
    if LINUX_RUNTIME_MODE.load(Ordering::SeqCst) {
        return address_space::current_linux_range_mapped(address, length, writable);
    }
    let Some(target) = address.checked_add(length as u64) else {
        return false;
    };
    if length == 0 {
        return false;
    }
    let data_base = ELF_PROBE_DATA.load(Ordering::SeqCst);
    let data_size = ELF_PROBE_DATA_SIZE.load(Ordering::SeqCst);
    let protected_page = ELF_PROBE_READ_ONLY_DATA_PAGE.load(Ordering::SeqCst);
    let tls_base = ELF_PROBE_TLS.load(Ordering::SeqCst);
    let tls_size = ELF_PROBE_TLS_SIZE.load(Ordering::SeqCst);
    let stack_base = ELF_PROBE_STACK.load(Ordering::SeqCst) & !(PAGE_SIZE as u64 - 1);
    let mmap_base = ELF_PROBE_MMAP.load(Ordering::SeqCst);
    let brk = ELF_PROBE_BRK.load(Ordering::SeqCst);
    let heap_base = ELF_PROBE_BRK_LIMIT
        .load(Ordering::SeqCst)
        .saturating_sub(PAGE_SIZE as u64);
    let mut current = address;
    while current < target {
        let mut next = current;
        if writable {
            if let Some(data_end) = linux_user_area_end(data_base, data_size, current) {
                let protected_end = protected_page.checked_add(PAGE_SIZE as u64);
                if protected_page == 0
                    || current < protected_page
                    || protected_end.is_some_and(|end| current >= end)
                {
                    next = next.max(if protected_page != 0 && current < protected_page {
                        data_end.min(protected_page)
                    } else {
                        data_end
                    });
                }
            }
        } else {
            extend_linux_user_area(&mut next, data_base, data_size, current);
            extend_linux_user_area(
                &mut next,
                ELF_PROBE_CODE_BASE.load(Ordering::SeqCst),
                ELF_PROBE_CODE_SIZE.load(Ordering::SeqCst),
                current,
            );
            extend_linux_user_area(
                &mut next,
                ELF_PROBE_LOADER_CODE_BASE.load(Ordering::SeqCst),
                ELF_PROBE_LOADER_CODE_SIZE.load(Ordering::SeqCst),
                current,
            );
        }
        extend_linux_user_area(&mut next, tls_base, tls_size, current);
        extend_linux_user_area(&mut next, stack_base, PAGE_SIZE as u64, current);
        for index in 0..LINUX_RUNTIME_MMAP_SLOT_COUNT {
            let pages = ELF_PROBE_MMAP_PAGES[index].load(Ordering::SeqCst);
            let protection = u64::from(ELF_PROBE_MMAP_PROTECTION[index].load(Ordering::SeqCst));
            let required = if writable {
                LINUX_PROT_WRITE
            } else {
                LINUX_PROT_READ
            };
            if pages != 0 && protection & required != 0 {
                extend_linux_user_area(
                    &mut next,
                    mmap_base + (index * PAGE_SIZE) as u64,
                    u64::from(pages) * PAGE_SIZE as u64,
                    current,
                );
            }
        }
        extend_linux_user_area(&mut next, heap_base, brk.saturating_sub(heap_base), current);
        if next == current {
            return false;
        }
        current = next.min(target);
    }
    true
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
    if !linux_readable_user_range_contains(address, output.len()) {
        return false;
    }
    // SAFETY: every byte is covered by a contiguous chain of declared readable
    // areas that remain mapped in the current root for the whole call.
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

fn copy_linux_id_triple(first: u64, second: u64, third: u64, ids: (u32, u32, u32)) -> bool {
    let width = size_of::<u32>();
    if !linux_writable_user_range_contains(first, width)
        || !linux_writable_user_range_contains(second, width)
        || !linux_writable_user_range_contains(third, width)
    {
        return false;
    }
    copy_to_linux_user(first, &ids.0.to_ne_bytes())
        && copy_to_linux_user(second, &ids.1.to_ne_bytes())
        && copy_to_linux_user(third, &ids.2.to_ne_bytes())
}

fn linux_capget(header_address: u64, data_address: u64) -> u64 {
    let Some(namespace) = current_linux_user_namespace() else {
        return LINUX_ESRCH;
    };
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
    if pid != 0 && u32::try_from(pid).ok() != Some(current_linux_process_pid()) {
        return LINUX_ESRCH;
    }
    if !linux_user::has_no_linux_capabilities(namespace) {
        return LINUX_EPERM;
    }
    if data_address != 0 && !copy_to_linux_user(data_address, &[0_u8; LINUX_CAPABILITY_DATA_SIZE]) {
        return LINUX_EFAULT;
    }
    0
}

fn linux_capset(header_address: u64, data_address: u64) -> u64 {
    let Some(namespace) = current_linux_user_namespace() else {
        return LINUX_ESRCH;
    };
    let mut header = [0_u8; 8];
    if !copy_from_linux_user(header_address, &mut header) {
        return LINUX_EFAULT;
    }
    if u32::from_ne_bytes(header[..4].try_into().unwrap()) != LINUX_CAPABILITY_VERSION_3 {
        return LINUX_EINVAL;
    }
    if i32::from_ne_bytes(header[4..].try_into().unwrap()) != 0
        || !linux_user::has_no_linux_capabilities(namespace)
    {
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
    if new_stack != 0 && ELF_PROBE_ALT_STACK_FLAGS.load(Ordering::SeqCst) == LINUX_SS_ONSTACK {
        return LINUX_EPERM;
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
            0 | LINUX_SS_AUTODISARM if size < LINUX_MIN_ALT_STACK_SIZE => return LINUX_ENOMEM,
            0 | LINUX_SS_AUTODISARM => {
                let Ok(length) = usize::try_from(size) else {
                    return LINUX_EFAULT;
                };
                if !linux_writable_user_range_contains(pointer, length) {
                    return LINUX_EFAULT;
                }
                Some((pointer, flags, size))
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
    store_current_linux_signal_mask(next);
    0
}

fn linux_rt_sigpending(set: u64, set_size: u64) -> u64 {
    if set_size != LINUX_SIGNAL_SET_SIZE {
        return LINUX_EINVAL;
    }
    let pending = ELF_PROBE_PENDING_SIGNALS.load(Ordering::SeqCst)
        & ELF_PROBE_SIGNAL_MASK.load(Ordering::SeqCst);
    if copy_to_linux_user(set, &pending.to_ne_bytes()) {
        0
    } else {
        LINUX_EFAULT
    }
}

fn linux_rt_sigtimedwait(set: u64, information: u64, timeout: u64, set_size: u64) -> u64 {
    if set_size != LINUX_SIGNAL_SET_SIZE {
        return LINUX_EINVAL;
    }
    let mut set_bytes = [0_u8; LINUX_SIGNAL_SET_SIZE as usize];
    if !copy_from_linux_user(set, &mut set_bytes) {
        return LINUX_EFAULT;
    }
    let requested = u64::from_ne_bytes(set_bytes) & !LINUX_UNBLOCKABLE_SIGNALS;
    let polls = if timeout == 0 {
        false
    } else {
        let mut timeout_bytes = [0_u8; 16];
        if !copy_from_linux_user(timeout, &mut timeout_bytes) {
            return LINUX_EFAULT;
        }
        let seconds = i64::from_ne_bytes(timeout_bytes[..8].try_into().unwrap());
        let nanoseconds = i64::from_ne_bytes(timeout_bytes[8..].try_into().unwrap());
        if seconds < 0 || !(0..NANOSECONDS_PER_SECOND as i64).contains(&nanoseconds) {
            return LINUX_EINVAL;
        }
        seconds == 0 && nanoseconds == 0
    };
    let matching = ELF_PROBE_PENDING_SIGNALS.load(Ordering::SeqCst) & requested;
    if matching == 0 {
        return if polls { LINUX_EAGAIN } else { LINUX_ENOSYS };
    }
    if information != 0
        && !linux_writable_user_range_contains(information, size_of::<LinuxSignalInformation>())
    {
        return LINUX_EFAULT;
    }
    let signal = u64::from(matching.trailing_zeros()) + 1;
    let index = signal as usize - 1;
    let signal_bit = 1 << (signal - 1);
    let signal_information = LinuxSignalInformation::generated(
        signal,
        ELF_PROBE_PENDING_SIGNAL_CODES[index].load(Ordering::SeqCst) as i32,
    );
    if information != 0 {
        // SAFETY: every byte in this repr(C) value is initialized, including
        // padding and the complete signal-specific payload.
        let bytes = unsafe {
            core::slice::from_raw_parts(
                core::ptr::from_ref(&signal_information).cast::<u8>(),
                size_of::<LinuxSignalInformation>(),
            )
        };
        if !copy_to_linux_user(information, bytes) {
            return LINUX_EFAULT;
        }
    }
    ELF_PROBE_PENDING_SIGNALS.fetch_and(!signal_bit, Ordering::SeqCst);
    ELF_PROBE_PENDING_SIGNAL_CODES[index].store(0, Ordering::SeqCst);
    signal
}

fn linux_rt_sigtimedwait_live_return(
    output: *mut LinuxSyscallFrame,
    caller: LinuxUserContext,
    set: u64,
    information: u64,
    timeout: u64,
    set_size: u64,
) -> u64 {
    let immediate = linux_rt_sigtimedwait(set, information, timeout, set_size);
    if immediate != LINUX_ENOSYS {
        return immediate;
    }
    let deadline = if timeout == 0 {
        0
    } else {
        let mut timeout_bytes = [0_u8; 16];
        if !copy_from_linux_user(timeout, &mut timeout_bytes) {
            return LINUX_EFAULT;
        }
        let Some(duration) = linux_timespec_nanoseconds(&timeout_bytes).filter(|value| *value != 0)
        else {
            return LINUX_EINVAL;
        };
        let Some(deadline) = linux_elapsed_nanoseconds().and_then(|now| now.checked_add(duration))
        else {
            return LINUX_EINVAL;
        };
        deadline
    };
    let mut set_bytes = [0_u8; LINUX_SIGNAL_SET_SIZE as usize];
    if !copy_from_linux_user(set, &mut set_bytes) {
        return LINUX_EFAULT;
    }
    let requested = u64::from_ne_bytes(set_bytes) & !LINUX_UNBLOCKABLE_SIGNALS;
    if requested == 0 {
        return LINUX_ENOSYS;
    }
    if information != 0
        && !linux_writable_user_range_contains(information, size_of::<LinuxSignalInformation>())
    {
        return LINUX_EFAULT;
    }
    let Some(current) = current_linux_thread_handle() else {
        return LINUX_ESRCH;
    };
    if ELF_PROBE_SYSTEM_INIT.load(Ordering::SeqCst)
        && current_linux_process_pid() == 1
        && current_linux_tid() == Some(1)
        && requested == 1 << (LINUX_SIGTERM - 1)
        && timeout == 0
        && information != 0
        && ELF_PROBE_SIGNAL_MASK.load(Ordering::SeqCst) & requested == requested
    {
        if LINUX_SYSTEM_SUPERVISOR_WAIT_COUNT.load(Ordering::SeqCst) != 0
            || !save_linux_thread_return_context(current, caller)
            || !block_current_linux_thread(LINUX_THREAD_WAIT_SIGNAL, requested, information, 0)
        {
            return LINUX_EINVAL;
        }
        LINUX_SIGNAL_WAIT_BLOCK_COUNT.fetch_add(1, Ordering::SeqCst);
        LINUX_SYSTEM_SUPERVISOR_WAIT_COUNT.fetch_add(1, Ordering::SeqCst);
        return LINUX_RETURN_TO_KERNEL;
    }
    let Some(target) = ready_linux_thread_handle() else {
        return LINUX_ENOSYS;
    };
    let Some(tls) = linux_thread_context(current) else {
        return LINUX_ESRCH;
    };
    if !save_linux_thread_return_context(current, caller)
        || !block_current_linux_thread(LINUX_THREAD_WAIT_SIGNAL, requested, information, deadline)
    {
        return LINUX_EINVAL;
    }
    let mut target_registers = LinuxSyscallFrame::ZERO;
    let mut target_rip = 0;
    let mut target_rsp = 0;
    let mut target_rflags = 0;
    let mut target_rax = 0;
    if !install_linux_thread_return(
        target,
        &mut target_registers,
        &mut target_rip,
        &mut target_rsp,
        &mut target_rflags,
        &mut target_rax,
    ) {
        let slot = usize::from(current.slot);
        LINUX_THREAD_WAIT_KINDS[slot].store(LINUX_THREAD_WAIT_NONE, Ordering::SeqCst);
        LINUX_THREAD_WAIT_KEYS[slot].store(0, Ordering::SeqCst);
        LINUX_THREAD_WAIT_OUTPUTS[slot].store(0, Ordering::SeqCst);
        LINUX_THREAD_WAIT_DEADLINES[slot].store(0, Ordering::SeqCst);
        LINUX_THREAD_STATES[slot].store(LINUX_THREAD_RUNNING, Ordering::SeqCst);
        LINUX_CURRENT_THREAD_SLOT.store(current.slot, Ordering::SeqCst);
        let _ = install_linux_tls_context(tls);
        return LINUX_EINVAL;
    }
    let selected = LinuxUserContext {
        registers: target_registers,
        rip: target_rip,
        rsp: target_rsp,
        rflags: target_rflags,
        rax: target_rax,
        fs_base: active_fs_base(),
    };
    if !write_linux_live_return(output, selected) {
        return LINUX_EINVAL;
    }
    LINUX_LIVE_RETURN_VALIDATED.store(true, Ordering::SeqCst);
    LINUX_SIGNAL_WAIT_BLOCK_COUNT.fetch_add(1, Ordering::SeqCst);
    selected.rax
}

fn wake_linux_signal_wait(signal: u64, signal_code: i32) -> bool {
    let signal_bit = 1_u64 << (signal - 1);
    for slot in 0..LINUX_THREAD_LIMIT {
        if LINUX_THREAD_STATES[slot].load(Ordering::SeqCst) != LINUX_THREAD_BLOCKED
            || LINUX_THREAD_WAIT_KINDS[slot].load(Ordering::SeqCst) != LINUX_THREAD_WAIT_SIGNAL
            || LINUX_THREAD_WAIT_KEYS[slot].load(Ordering::SeqCst) & signal_bit == 0
        {
            continue;
        }
        let information = LINUX_THREAD_WAIT_OUTPUTS[slot].load(Ordering::SeqCst);
        if information != 0 {
            let value = LinuxSignalInformation::generated(signal, signal_code);
            // SAFETY: LinuxSignalInformation is repr(C), and every byte of
            // this generated value, including padding, is initialized.
            let bytes = unsafe {
                core::slice::from_raw_parts(
                    core::ptr::from_ref(&value).cast::<u8>(),
                    size_of::<LinuxSignalInformation>(),
                )
            };
            if !copy_to_linux_user(information, bytes) {
                return false;
            }
        }
        LINUX_THREAD_RAXS[slot].store(signal, Ordering::SeqCst);
        LINUX_THREAD_WAIT_KINDS[slot].store(LINUX_THREAD_WAIT_NONE, Ordering::SeqCst);
        LINUX_THREAD_WAIT_KEYS[slot].store(0, Ordering::SeqCst);
        LINUX_THREAD_WAIT_OUTPUTS[slot].store(0, Ordering::SeqCst);
        LINUX_THREAD_WAIT_DEADLINES[slot].store(0, Ordering::SeqCst);
        LINUX_THREAD_STATES[slot].store(LINUX_THREAD_READY, Ordering::SeqCst);
        LINUX_SIGNAL_WAIT_WAKE_COUNT.fetch_add(1, Ordering::SeqCst);
        return true;
    }
    false
}

fn expire_linux_waits() -> u32 {
    let Some(now) = linux_elapsed_nanoseconds() else {
        return 0;
    };
    let mut expired = 0;
    for slot in 0..LINUX_THREAD_LIMIT {
        let deadline = LINUX_THREAD_WAIT_DEADLINES[slot].load(Ordering::SeqCst);
        if deadline == 0
            || now < deadline
            || LINUX_THREAD_STATES[slot].load(Ordering::SeqCst) != LINUX_THREAD_BLOCKED
        {
            continue;
        }
        if matches!(
            LINUX_THREAD_WAIT_KINDS[slot].load(Ordering::SeqCst),
            LINUX_THREAD_WAIT_POLL | LINUX_THREAD_WAIT_PPOLL
        ) {
            if complete_linux_poll_wait(slot, true) {
                expired += 1;
            }
            continue;
        }
        if matches!(
            LINUX_THREAD_WAIT_KINDS[slot].load(Ordering::SeqCst),
            LINUX_THREAD_WAIT_EPOLL | LINUX_THREAD_WAIT_EPOLL_PWAIT
        ) {
            if complete_linux_epoll_wait(slot, true) {
                expired += 1;
            }
            continue;
        }
        if LINUX_THREAD_WAIT_KINDS[slot].load(Ordering::SeqCst) == LINUX_THREAD_WAIT_TIMERFD_READ {
            if complete_linux_timerfd_read_wait(slot) {
                expired += 1;
            }
            continue;
        }
        let (result, counter) = match LINUX_THREAD_WAIT_KINDS[slot].load(Ordering::SeqCst) {
            LINUX_THREAD_WAIT_FUTEX => (LINUX_ETIMEDOUT, &LINUX_FUTEX_WAIT_TIMEOUT_COUNT),
            LINUX_THREAD_WAIT_SIGNAL => (LINUX_EAGAIN, &LINUX_SIGNAL_WAIT_TIMEOUT_COUNT),
            LINUX_THREAD_WAIT_SLEEP => (0, &LINUX_SLEEP_WAKE_COUNT),
            _ => continue,
        };
        LINUX_THREAD_RAXS[slot].store(result, Ordering::SeqCst);
        LINUX_THREAD_WAIT_KINDS[slot].store(LINUX_THREAD_WAIT_NONE, Ordering::SeqCst);
        LINUX_THREAD_WAIT_KEYS[slot].store(0, Ordering::SeqCst);
        LINUX_THREAD_WAIT_OUTPUTS[slot].store(0, Ordering::SeqCst);
        LINUX_THREAD_WAIT_DEADLINES[slot].store(0, Ordering::SeqCst);
        LINUX_THREAD_STATES[slot].store(LINUX_THREAD_READY, Ordering::SeqCst);
        counter.fetch_add(1, Ordering::SeqCst);
        expired += 1;
    }
    expired
}

fn save_linux_interrupt_context(
    handle: LinuxThreadHandle,
    context: UserFaultContext,
    fs_base: u64,
) -> bool {
    let saved = LinuxUserContext {
        registers: LinuxSyscallFrame {
            r15: context.r15,
            r14: context.r14,
            r13: context.r13,
            r12: context.r12,
            rbp: context.rbp,
            rbx: context.rbx,
            r10: context.r10,
            r9: context.r9,
            r8: context.r8,
            rdx: context.rdx,
            rsi: context.rsi,
            rdi: context.rdi,
        },
        rip: context.rip,
        rsp: context.rsp,
        rflags: context.rflags,
        rax: context.rax,
        fs_base,
    };
    if !save_linux_thread_return_context(handle, saved) {
        return false;
    }
    let slot = usize::from(handle.slot);
    LINUX_THREAD_RCXS[slot].store(context.rcx, Ordering::SeqCst);
    LINUX_THREAD_R11S[slot].store(context.r11, Ordering::SeqCst);
    true
}

fn linux_interrupt_context(handle: LinuxThreadHandle) -> Option<UserFaultContext> {
    let slot = usize::from(handle.slot);
    let saved = linux_thread_return_context(handle)?;
    Some(UserFaultContext {
        r15: saved.registers.r15,
        r14: saved.registers.r14,
        r13: saved.registers.r13,
        r12: saved.registers.r12,
        r11: LINUX_THREAD_R11S[slot].load(Ordering::SeqCst),
        r10: saved.registers.r10,
        r9: saved.registers.r9,
        r8: saved.registers.r8,
        rbp: saved.registers.rbp,
        rdi: saved.registers.rdi,
        rsi: saved.registers.rsi,
        rdx: saved.registers.rdx,
        rcx: LINUX_THREAD_RCXS[slot].load(Ordering::SeqCst),
        rbx: saved.registers.rbx,
        rax: saved.rax,
        rip: saved.rip,
        rflags: saved.rflags,
        rsp: saved.rsp,
    })
}

pub(crate) fn expire_linux_waits_from_timer(frame: &mut UserInterruptFrame) {
    let descriptor_wakes = wake_linux_descriptor_waits();
    let expired = expire_linux_waits();
    if descriptor_wakes == 0 && expired == 0 {
        return;
    }
    LINUX_TIMER_WAIT_EXPIRY_COUNT.fetch_add(expired, Ordering::SeqCst);
    let (Some(current), Some(target), Some(interrupted)) = (
        current_linux_thread_handle(),
        ready_linux_thread_handle(),
        frame.user_context(USER_CODE_SELECTOR, USER_DATA_SELECTOR),
    ) else {
        return;
    };
    let Some(target_context) = linux_interrupt_context(target) else {
        return;
    };
    let mut restored = *frame;
    if !restored.restore_user_context(target_context, USER_CODE_SELECTOR, USER_DATA_SELECTOR)
        || !save_linux_interrupt_context(current, interrupted, active_fs_base())
        || !switch_linux_thread(target)
    {
        return;
    }
    *frame = restored;
    LINUX_TIMER_WAIT_PREEMPTION_COUNT.fetch_add(1, Ordering::SeqCst);
}

fn linux_executable_user_address(address: u64) -> bool {
    let code = ELF_PROBE_CODE_BASE.load(Ordering::SeqCst);
    let code_end = code.saturating_add(ELF_PROBE_CODE_SIZE.load(Ordering::SeqCst));
    let loader = ELF_PROBE_LOADER_CODE_BASE.load(Ordering::SeqCst);
    let loader_end = loader.saturating_add(ELF_PROBE_LOADER_CODE_SIZE.load(Ordering::SeqCst));
    (code..code_end).contains(&address) || (loader..loader_end).contains(&address)
}

fn linux_signal_action_deliverable(handler: u64, flags: u64, restorer: u64) -> bool {
    let supported = LINUX_SA_SIGINFO
        | LINUX_SA_RESTORER
        | LINUX_SA_ONSTACK
        | LINUX_SA_NODEFER
        | LINUX_SA_RESETHAND;
    handler > 1
        && flags & LINUX_SA_RESTORER != 0
        && flags & !supported == 0
        && linux_executable_user_address(handler)
        && linux_executable_user_address(restorer)
}

fn linux_signal_stack(stack_top: u64, use_alt_stack: bool) -> Option<u64> {
    let frame_top = if use_alt_stack {
        stack_top
    } else {
        stack_top.checked_sub(LINUX_SIGNAL_RED_ZONE_SIZE)?
    };
    let signal_stack = frame_top
        .checked_sub(LINUX_SIGNAL_FRAME_SIZE as u64)?
        .checked_div(16)?
        .checked_mul(16)?
        .checked_sub(8)?;
    if use_alt_stack && signal_stack < ELF_PROBE_ALT_STACK_POINTER.load(Ordering::SeqCst) {
        None
    } else {
        Some(signal_stack)
    }
}

fn linux_rt_signal_frame(
    context: LinuxSignalContext,
    flags: u64,
    information: LinuxSignalInformation,
    trap: u64,
    error: u64,
    cr2: u64,
) -> LinuxRtSignalFrame {
    let registers = context.registers;
    let alt_stack_flags = context.alt_stack_flags;
    let alt_stack_disabled = alt_stack_flags == LINUX_SS_DISABLE;
    LinuxRtSignalFrame {
        restorer: context.signal_restorer,
        context: LinuxSignalUserContext {
            flags: LINUX_UC_SIGCONTEXT_SS | LINUX_UC_STRICT_RESTORE_SS,
            link: 0,
            stack: LinuxSignalStack {
                pointer: if alt_stack_disabled {
                    0
                } else {
                    context.alt_stack_pointer
                },
                flags: alt_stack_flags,
                padding: 0,
                size: if alt_stack_disabled {
                    0
                } else {
                    context.alt_stack_size
                },
            },
            machine: LinuxSignalMachineContext {
                r8: registers.r8,
                r9: registers.r9,
                r10: registers.r10,
                r11: context.r11,
                r12: registers.r12,
                r13: registers.r13,
                r14: registers.r14,
                r15: registers.r15,
                rdi: registers.rdi,
                rsi: registers.rsi,
                rbp: registers.rbp,
                rbx: registers.rbx,
                rdx: registers.rdx,
                rax: context.rax,
                rcx: context.rcx,
                rsp: context.rsp,
                rip: context.rip,
                rflags: context.rflags,
                cs: USER_CODE_SELECTOR,
                gs: 0,
                fs: 0,
                ss: USER_DATA_SELECTOR,
                error,
                trap,
                old_mask: context.signal_mask,
                cr2,
                fpstate: 0,
                reserved: [0; 8],
            },
            signal_mask: context.signal_mask,
        },
        information: if flags & LINUX_SA_SIGINFO != 0 {
            information
        } else {
            LinuxSignalInformation::ZERO
        },
    }
}

fn copy_linux_signal_frame(address: u64, frame: &LinuxRtSignalFrame) -> bool {
    // SAFETY: every field, including ABI padding and reserved bytes, is
    // initialized above and the resulting byte view is used only for copy-out.
    let bytes = unsafe {
        core::slice::from_raw_parts(
            core::ptr::from_ref(frame).cast::<u8>(),
            size_of::<LinuxRtSignalFrame>(),
        )
    };
    copy_to_linux_user(address, bytes)
}

fn copy_linux_signal_frame_from_user(address: u64, frame: &mut LinuxRtSignalFrame) -> bool {
    // SAFETY: the destination is a fully initialized, repr(C) frame and the
    // mutable byte view spans exactly that object for bounded copy-in.
    let bytes = unsafe {
        core::slice::from_raw_parts_mut(
            core::ptr::from_mut(frame).cast::<u8>(),
            size_of::<LinuxRtSignalFrame>(),
        )
    };
    copy_from_linux_user(address, bytes)
}

fn linux_restored_signal_context(
    saved: LinuxSignalContext,
    returned: &LinuxRtSignalFrame,
) -> Option<LinuxRestoredSignalContext> {
    let expected = linux_rt_signal_frame(saved, 0, LinuxSignalInformation::ZERO, 0, 0, 0);
    let context = &returned.context;
    let machine = &context.machine;
    if returned.restorer != saved.signal_restorer
        || context.flags != LINUX_UC_SIGCONTEXT_SS | LINUX_UC_STRICT_RESTORE_SS
        || context.link != 0
        || context.stack != expected.context.stack
        || machine.cs != USER_CODE_SELECTOR
        || machine.gs != 0
        || machine.fs != 0
        || machine.ss != USER_DATA_SELECTOR
        || machine.fpstate != 0
        || machine.reserved != [0; 8]
    {
        return None;
    }
    let restored = LinuxUserContext {
        registers: LinuxSyscallFrame {
            r15: machine.r15,
            r14: machine.r14,
            r13: machine.r13,
            r12: machine.r12,
            rbp: machine.rbp,
            rbx: machine.rbx,
            r10: machine.r10,
            r9: machine.r9,
            r8: machine.r8,
            rdx: machine.rdx,
            rsi: machine.rsi,
            rdi: machine.rdi,
        },
        rip: machine.rip,
        rsp: machine.rsp,
        rflags: (saved.rflags & !LINUX_SIGNAL_RESTORABLE_RFLAGS)
            | (machine.rflags & LINUX_SIGNAL_RESTORABLE_RFLAGS),
        rax: machine.rax,
        fs_base: active_fs_base(),
    };
    restored.is_valid().then_some(LinuxRestoredSignalContext {
        user: restored,
        signal_mask: context.signal_mask & !LINUX_UNBLOCKABLE_SIGNALS,
        rcx: machine.rcx,
        r11: machine.r11,
    })
}

fn linux_apply_delivered_signal_action(index: usize, signal_bit: u64, old_mask: u64, flags: u64) {
    let action_mask = ELF_PROBE_SIGNAL_ACTION_MASKS[index].load(Ordering::SeqCst);
    let automatic_mask = if flags & LINUX_SA_NODEFER == 0 {
        signal_bit
    } else {
        0
    };
    store_current_linux_signal_mask(old_mask | action_mask | automatic_mask);
    if flags & LINUX_SA_RESETHAND != 0 {
        ELF_PROBE_SIGNAL_HANDLERS[index].store(0, Ordering::SeqCst);
        ELF_PROBE_SIGNAL_FLAGS[index].store(0, Ordering::SeqCst);
        ELF_PROBE_SIGNAL_RESTORERS[index].store(0, Ordering::SeqCst);
        ELF_PROBE_SIGNAL_ACTION_MASKS[index].store(0, Ordering::SeqCst);
    }
}

fn linux_default_signal_terminates(signal: u64) -> bool {
    signal == LINUX_SIGPIPE
}

fn linux_terminate_by_default_signal(signal: u64) -> u64 {
    if !record_live_linux_process_snapshot() {
        return LINUX_ESRCH;
    }
    for descriptor in 0..LINUX_AUX_DESCRIPTOR_LIMIT {
        let _ = linux_descriptor_close(descriptor);
    }
    retire_all_linux_threads();
    ELF_PROBE_SIGNAL_MASK.store(0, Ordering::SeqCst);
    ELF_PROBE_PENDING_SIGNALS.store(0, Ordering::SeqCst);
    ELF_PROBE_EXEC_IGNORED_SIGNALS.store(0, Ordering::SeqCst);
    for index in 0..LINUX_SIGNAL_COUNT {
        ELF_PROBE_PENDING_SIGNAL_CODES[index].store(0, Ordering::SeqCst);
        ELF_PROBE_SIGNAL_HANDLERS[index].store(0, Ordering::SeqCst);
        ELF_PROBE_SIGNAL_FLAGS[index].store(0, Ordering::SeqCst);
        ELF_PROBE_SIGNAL_RESTORERS[index].store(0, Ordering::SeqCst);
        ELF_PROBE_SIGNAL_ACTION_MASKS[index].store(0, Ordering::SeqCst);
    }
    ELF_PROBE_ALT_STACK_POINTER.store(0, Ordering::SeqCst);
    ELF_PROBE_ALT_STACK_SIZE.store(0, Ordering::SeqCst);
    ELF_PROBE_ALT_STACK_FLAGS.store(LINUX_SS_DISABLE, Ordering::SeqCst);
    ELF_PROBE_TERMINATION_SIGNAL.store(signal as u32, Ordering::SeqCst);
    ELF_PROBE_COMPLETED.store(true, Ordering::SeqCst);
    ELF_PROBE_ACTIVE.store(false, Ordering::SeqCst);
    LINUX_RETURN_TO_KERNEL
}

fn linux_kill(
    pid: u64,
    generated: (u64, i32),
    output: *mut LinuxSyscallFrame,
    interrupted: LinuxUserContext,
    return_registers: (u64, u64),
) -> u64 {
    let (signal, signal_code) = generated;
    if !matches!(pid, 0 | 1) {
        return LINUX_ESRCH;
    }
    if signal == 0 {
        return 0;
    }
    if !(1..=LINUX_SIGNAL_COUNT as u64).contains(&signal) {
        return LINUX_EINVAL;
    }
    if signal > LINUX_STANDARD_SIGNAL_COUNT {
        return LINUX_ENOSYS;
    }
    let index = signal as usize - 1;
    let signal_bit = 1 << (signal - 1);
    if wake_linux_signal_wait(signal, signal_code) {
        return 0;
    }
    let handler = ELF_PROBE_SIGNAL_HANDLERS[index].load(Ordering::SeqCst);
    if handler == 1 {
        return 0;
    }
    let old_mask = ELF_PROBE_SIGNAL_MASK.load(Ordering::SeqCst);
    if old_mask & signal_bit != 0 {
        if ELF_PROBE_PENDING_SIGNALS.load(Ordering::SeqCst) & signal_bit == 0 {
            ELF_PROBE_PENDING_SIGNAL_CODES[index].store(signal_code as u32, Ordering::SeqCst);
        }
        ELF_PROBE_PENDING_SIGNALS.fetch_or(signal_bit, Ordering::SeqCst);
        return 0;
    }
    if handler == 0 {
        return if linux_default_signal_terminates(signal)
            && !LINUX_SIGNAL_ACTIVE.load(Ordering::SeqCst)
        {
            linux_terminate_by_default_signal(signal)
        } else {
            LINUX_ENOSYS
        };
    }
    if output.is_null()
        || current_linux_thread_handle().is_none()
        || LINUX_SIGNAL_ACTIVE.load(Ordering::SeqCst)
    {
        return LINUX_ENOSYS;
    }
    let flags = ELF_PROBE_SIGNAL_FLAGS[index].load(Ordering::SeqCst);
    let restorer = ELF_PROBE_SIGNAL_RESTORERS[index].load(Ordering::SeqCst);
    if !linux_signal_action_deliverable(handler, flags, restorer) {
        return LINUX_ENOSYS;
    }
    let old_alt_stack_flags = ELF_PROBE_ALT_STACK_FLAGS.load(Ordering::SeqCst);
    let use_alt_stack = flags & LINUX_SA_ONSTACK != 0
        && old_alt_stack_flags & (LINUX_SS_DISABLE | LINUX_SS_ONSTACK) == 0;
    let stack_top = if use_alt_stack {
        let pointer = ELF_PROBE_ALT_STACK_POINTER.load(Ordering::SeqCst);
        let size = ELF_PROBE_ALT_STACK_SIZE.load(Ordering::SeqCst);
        let Some(top) = pointer.checked_add(size) else {
            return LINUX_EFAULT;
        };
        top
    } else {
        interrupted.rsp
    };
    let Some(signal_stack) = linux_signal_stack(stack_top, use_alt_stack) else {
        return LINUX_EFAULT;
    };
    let context = LinuxSignalContext::from_syscall(
        interrupted,
        return_registers,
        (signal_stack, restorer),
        old_mask,
        old_alt_stack_flags,
    );
    if !context.user_context().is_valid() {
        return LINUX_EINVAL;
    }
    let signal_frame = linux_rt_signal_frame(
        context,
        flags,
        LinuxSignalInformation::generated(signal, signal_code),
        0,
        0,
        0,
    );
    if !copy_linux_signal_frame(signal_stack, &signal_frame) {
        return LINUX_EFAULT;
    }
    let mut handler_registers = interrupted.registers;
    handler_registers.rdi = signal;
    handler_registers.rsi = signal_stack + LINUX_SIGNAL_FRAME_INFO_OFFSET;
    handler_registers.rdx = signal_stack + LINUX_SIGNAL_FRAME_CONTEXT_OFFSET;
    let handler_context = LinuxUserContext {
        registers: handler_registers,
        rip: handler,
        rsp: signal_stack,
        rflags: interrupted.rflags & !((1 << 8) | (1 << 10) | (1 << 16)),
        rax: 0,
        fs_base: active_fs_base(),
    };
    if !handler_context.is_valid() || !write_linux_live_return(output, handler_context) {
        return LINUX_EINVAL;
    }
    // SAFETY: syscall entry is serialized with interrupts disabled until its
    // return context enters the handler.
    unsafe { core::ptr::addr_of_mut!(LINUX_SIGNAL_CONTEXT).write(context) };
    linux_apply_delivered_signal_action(index, signal_bit, old_mask, flags);
    if use_alt_stack {
        if old_alt_stack_flags & LINUX_SS_AUTODISARM != 0 {
            ELF_PROBE_ALT_STACK_POINTER.store(0, Ordering::SeqCst);
            ELF_PROBE_ALT_STACK_SIZE.store(0, Ordering::SeqCst);
            ELF_PROBE_ALT_STACK_FLAGS.store(LINUX_SS_DISABLE, Ordering::SeqCst);
        } else {
            ELF_PROBE_ALT_STACK_FLAGS.store(LINUX_SS_ONSTACK, Ordering::SeqCst);
        }
    }
    LINUX_SIGNAL_ACTIVE.store(true, Ordering::SeqCst);
    LINUX_SIGNAL_DELIVERY_COUNT.fetch_add(1, Ordering::SeqCst);
    0
}

fn linux_interrupt_blocked_wait(
    target: LinuxThreadHandle,
    generated: (u64, i32),
    output: *mut LinuxSyscallFrame,
    sender_return: LinuxUserContext,
) -> u64 {
    linux_deliver_signal_to_blocked_wait(
        target,
        generated,
        output,
        sender_return,
        LINUX_EINTR,
        false,
    )
}

fn linux_deliver_signal_to_blocked_wait(
    target: LinuxThreadHandle,
    generated: (u64, i32),
    output: *mut LinuxSyscallFrame,
    sender_return: LinuxUserContext,
    interrupted_result: u64,
    completed: bool,
) -> u64 {
    let (signal, signal_code) = generated;
    if signal == 0 {
        return 0;
    }
    if !(1..=LINUX_SIGNAL_COUNT as u64).contains(&signal) {
        return LINUX_EINVAL;
    }
    if signal > LINUX_STANDARD_SIGNAL_COUNT {
        return LINUX_ENOSYS;
    }
    let Some(sender) = current_linux_thread_handle() else {
        return LINUX_ESRCH;
    };
    let Some(sender_tls) = linux_thread_context(sender) else {
        return LINUX_ESRCH;
    };
    let saved_sender = linux_thread_return_context(sender);
    let slot = usize::from(target.slot);
    let wait_kind = LINUX_THREAD_WAIT_KINDS[slot].load(Ordering::SeqCst);
    if sender == target
        || LINUX_THREAD_STATES[slot].load(Ordering::SeqCst) != LINUX_THREAD_BLOCKED
        || !matches!(
            wait_kind,
            LINUX_THREAD_WAIT_SIGNAL
                | LINUX_THREAD_WAIT_SLEEP
                | LINUX_THREAD_WAIT_POLL
                | LINUX_THREAD_WAIT_EPOLL
                | LINUX_THREAD_WAIT_EVENTFD_READ
                | LINUX_THREAD_WAIT_PIPE_READ
                | LINUX_THREAD_WAIT_TIMERFD_READ
                | LINUX_THREAD_WAIT_PIPE_WRITE
                | LINUX_THREAD_WAIT_EVENTFD_WRITE
                | LINUX_THREAD_WAIT_PPOLL
                | LINUX_THREAD_WAIT_EPOLL_PWAIT
        )
        || output.is_null()
        || LINUX_SIGNAL_ACTIVE.load(Ordering::SeqCst)
    {
        return LINUX_ENOSYS;
    }
    let index = signal as usize - 1;
    let signal_bit = 1 << (signal - 1);
    let wait_mask = LINUX_THREAD_SIGNAL_MASKS[slot].load(Ordering::SeqCst);
    if wait_mask & signal_bit != 0
        || wait_kind == LINUX_THREAD_WAIT_SIGNAL
            && LINUX_THREAD_WAIT_KEYS[slot].load(Ordering::SeqCst) & signal_bit != 0
    {
        return LINUX_ENOSYS;
    }
    let handler = ELF_PROBE_SIGNAL_HANDLERS[index].load(Ordering::SeqCst);
    if handler == 1 {
        return 0;
    }
    let flags = ELF_PROBE_SIGNAL_FLAGS[index].load(Ordering::SeqCst);
    let restorer = ELF_PROBE_SIGNAL_RESTORERS[index].load(Ordering::SeqCst);
    if handler == 0 || !linux_signal_action_deliverable(handler, flags, restorer) {
        return LINUX_ENOSYS;
    }
    let Some(mut interrupted) = linux_thread_return_context(target) else {
        return LINUX_ESRCH;
    };
    let saved_interrupted = interrupted;
    interrupted.rax = interrupted_result;
    let masked_wait = matches!(
        wait_kind,
        LINUX_THREAD_WAIT_PPOLL | LINUX_THREAD_WAIT_EPOLL_PWAIT
    );
    let restored_mask = if masked_wait {
        LINUX_THREAD_WAIT_PRIOR_SIGNAL_MASKS[slot].load(Ordering::SeqCst)
    } else {
        wait_mask
    };
    let old_alt_stack_flags = ELF_PROBE_ALT_STACK_FLAGS.load(Ordering::SeqCst);
    let use_alt_stack = flags & LINUX_SA_ONSTACK != 0
        && old_alt_stack_flags & (LINUX_SS_DISABLE | LINUX_SS_ONSTACK) == 0;
    let stack_top = if use_alt_stack {
        let pointer = ELF_PROBE_ALT_STACK_POINTER.load(Ordering::SeqCst);
        let size = ELF_PROBE_ALT_STACK_SIZE.load(Ordering::SeqCst);
        let Some(top) = pointer.checked_add(size) else {
            return LINUX_EFAULT;
        };
        top
    } else {
        interrupted.rsp
    };
    let Some(signal_stack) = linux_signal_stack(stack_top, use_alt_stack) else {
        return LINUX_EFAULT;
    };
    let target_stack_valid = if use_alt_stack {
        linux_writable_user_range_contains(signal_stack, LINUX_SIGNAL_FRAME_SIZE)
    } else {
        linux_range_contains(
            LINUX_THREAD_STACK_BASES[slot].load(Ordering::SeqCst),
            LINUX_THREAD_STACK_SIZES[slot].load(Ordering::SeqCst),
            signal_stack,
            LINUX_SIGNAL_FRAME_SIZE,
        )
    };
    if !target_stack_valid {
        return LINUX_EFAULT;
    }
    let context = LinuxSignalContext::from_syscall(
        interrupted,
        (
            LINUX_THREAD_RCXS[slot].load(Ordering::SeqCst),
            LINUX_THREAD_R11S[slot].load(Ordering::SeqCst),
        ),
        (signal_stack, restorer),
        restored_mask,
        old_alt_stack_flags,
    );
    let signal_frame = linux_rt_signal_frame(
        context,
        flags,
        LinuxSignalInformation::generated(signal, signal_code),
        0,
        0,
        0,
    );
    let mut handler_registers = interrupted.registers;
    handler_registers.rdi = signal;
    handler_registers.rsi = signal_stack + LINUX_SIGNAL_FRAME_INFO_OFFSET;
    handler_registers.rdx = signal_stack + LINUX_SIGNAL_FRAME_CONTEXT_OFFSET;
    let handler_context = LinuxUserContext {
        registers: handler_registers,
        rip: handler,
        rsp: signal_stack,
        rflags: interrupted.rflags & !((1 << 8) | (1 << 10) | (1 << 16)),
        rax: 0,
        fs_base: interrupted.fs_base,
    };
    if !context.user_context().is_valid() || !handler_context.is_valid() {
        return LINUX_EINVAL;
    }
    let sleep_remaining = if wait_kind == LINUX_THREAD_WAIT_SLEEP {
        let address = LINUX_THREAD_WAIT_OUTPUTS[slot].load(Ordering::SeqCst);
        if address == 0 {
            None
        } else {
            let Some(now) = linux_elapsed_nanoseconds() else {
                return LINUX_ENOSYS;
            };
            let deadline = LINUX_THREAD_WAIT_DEADLINES[slot].load(Ordering::SeqCst);
            let mut replaced = [0_u8; 16];
            if deadline == 0 || !copy_from_linux_user(address, &mut replaced) {
                return LINUX_EFAULT;
            }
            let mut remaining = [0_u8; 16];
            linux_store_timespec(&mut remaining, deadline.saturating_sub(now));
            Some((address, replaced, remaining))
        }
    } else {
        None
    };
    let mut replaced = [0_u8; LINUX_SIGNAL_FRAME_SIZE];
    if !copy_from_linux_user(signal_stack, &mut replaced) {
        return LINUX_EFAULT;
    }
    if let Some((address, _, remaining)) = &sleep_remaining {
        if !copy_to_linux_user(*address, remaining) {
            return LINUX_EFAULT;
        }
    }
    if !copy_linux_signal_frame(signal_stack, &signal_frame) {
        let _ = copy_to_linux_user(signal_stack, &replaced);
        if let Some((address, original, _)) = &sleep_remaining {
            let _ = copy_to_linux_user(*address, original);
        }
        return LINUX_EFAULT;
    }
    if !save_linux_thread_return_context(sender, sender_return) {
        let _ = copy_to_linux_user(signal_stack, &replaced);
        if let Some((address, original, _)) = &sleep_remaining {
            let _ = copy_to_linux_user(*address, original);
        }
        return LINUX_EINVAL;
    }
    if !save_linux_thread_return_context(target, interrupted) {
        restore_linux_thread_return_context(sender, saved_sender);
        let _ = copy_to_linux_user(signal_stack, &replaced);
        if let Some((address, original, _)) = &sleep_remaining {
            let _ = copy_to_linux_user(*address, original);
        }
        return LINUX_EINVAL;
    }
    if !write_linux_live_return(output, handler_context) {
        let _ = save_linux_thread_return_context(target, saved_interrupted);
        restore_linux_thread_return_context(sender, saved_sender);
        let _ = copy_to_linux_user(signal_stack, &replaced);
        if let Some((address, original, _)) = &sleep_remaining {
            let _ = copy_to_linux_user(*address, original);
        }
        return LINUX_EINVAL;
    }
    LINUX_THREAD_SIGNAL_MASKS[slot].store(restored_mask, Ordering::SeqCst);
    LINUX_THREAD_STATES[slot].store(LINUX_THREAD_READY, Ordering::SeqCst);
    if !switch_linux_thread(target) {
        let _ = save_linux_thread_return_context(target, saved_interrupted);
        restore_linux_thread_return_context(sender, saved_sender);
        LINUX_THREAD_SIGNAL_MASKS[slot].store(wait_mask, Ordering::SeqCst);
        LINUX_THREAD_STATES[slot].store(LINUX_THREAD_BLOCKED, Ordering::SeqCst);
        let _ = install_linux_tls_context(sender_tls);
        let _ = write_linux_live_return(output, sender_return);
        let _ = copy_to_linux_user(signal_stack, &replaced);
        if let Some((address, original, _)) = &sleep_remaining {
            let _ = copy_to_linux_user(*address, original);
        }
        return LINUX_EINVAL;
    }
    LINUX_THREAD_WAIT_KINDS[slot].store(LINUX_THREAD_WAIT_NONE, Ordering::SeqCst);
    LINUX_THREAD_WAIT_KEYS[slot].store(0, Ordering::SeqCst);
    LINUX_THREAD_WAIT_OUTPUTS[slot].store(0, Ordering::SeqCst);
    LINUX_THREAD_WAIT_DEADLINES[slot].store(0, Ordering::SeqCst);
    if masked_wait {
        LINUX_THREAD_WAIT_PRIOR_SIGNAL_MASKS[slot].store(0, Ordering::SeqCst);
    }
    if matches!(wait_kind, LINUX_THREAD_WAIT_POLL | LINUX_THREAD_WAIT_PPOLL) {
        clear_linux_poll_snapshot(slot);
    }
    if matches!(
        wait_kind,
        LINUX_THREAD_WAIT_PIPE_READ | LINUX_THREAD_WAIT_PIPE_WRITE
    ) {
        clear_linux_iov_snapshot(slot);
    }
    // SAFETY: the switch above made the target the sole running Linux thread,
    // and the live return already names its validated handler context.
    unsafe { core::ptr::addr_of_mut!(LINUX_SIGNAL_CONTEXT).write(context) };
    linux_apply_delivered_signal_action(index, signal_bit, restored_mask, flags);
    if use_alt_stack {
        if old_alt_stack_flags & LINUX_SS_AUTODISARM != 0 {
            ELF_PROBE_ALT_STACK_POINTER.store(0, Ordering::SeqCst);
            ELF_PROBE_ALT_STACK_SIZE.store(0, Ordering::SeqCst);
            ELF_PROBE_ALT_STACK_FLAGS.store(LINUX_SS_DISABLE, Ordering::SeqCst);
        } else {
            ELF_PROBE_ALT_STACK_FLAGS.store(LINUX_SS_ONSTACK, Ordering::SeqCst);
        }
    }
    LINUX_SIGNAL_ACTIVE.store(true, Ordering::SeqCst);
    LINUX_SIGNAL_DELIVERY_COUNT.fetch_add(1, Ordering::SeqCst);
    if completed && wait_kind == LINUX_THREAD_WAIT_PIPE_WRITE {
        LINUX_PIPE_WRITE_WAKE_COUNT.fetch_add(1, Ordering::SeqCst);
    } else if wait_kind == LINUX_THREAD_WAIT_SIGNAL {
        LINUX_SIGNAL_WAIT_INTERRUPT_COUNT.fetch_add(1, Ordering::SeqCst);
    } else if wait_kind == LINUX_THREAD_WAIT_SLEEP {
        LINUX_SLEEP_INTERRUPT_COUNT.fetch_add(1, Ordering::SeqCst);
    } else if wait_kind == LINUX_THREAD_WAIT_POLL {
        LINUX_POLL_WAIT_INTERRUPT_COUNT.fetch_add(1, Ordering::SeqCst);
    } else if wait_kind == LINUX_THREAD_WAIT_EPOLL {
        LINUX_EPOLL_WAIT_INTERRUPT_COUNT.fetch_add(1, Ordering::SeqCst);
    } else if wait_kind == LINUX_THREAD_WAIT_EVENTFD_READ {
        LINUX_EVENTFD_READ_INTERRUPT_COUNT.fetch_add(1, Ordering::SeqCst);
    } else if wait_kind == LINUX_THREAD_WAIT_PIPE_READ {
        LINUX_PIPE_READ_INTERRUPT_COUNT.fetch_add(1, Ordering::SeqCst);
    } else if wait_kind == LINUX_THREAD_WAIT_PIPE_WRITE {
        LINUX_PIPE_WRITE_INTERRUPT_COUNT.fetch_add(1, Ordering::SeqCst);
    } else if wait_kind == LINUX_THREAD_WAIT_EVENTFD_WRITE {
        LINUX_EVENTFD_WRITE_INTERRUPT_COUNT.fetch_add(1, Ordering::SeqCst);
    } else if wait_kind == LINUX_THREAD_WAIT_TIMERFD_READ {
        LINUX_TIMERFD_READ_INTERRUPT_COUNT.fetch_add(1, Ordering::SeqCst);
    } else {
        LINUX_MASKED_WAIT_INTERRUPT_COUNT.fetch_add(1, Ordering::SeqCst);
    }
    0
}

fn linux_tgkill_identity(thread_group: u64, thread: u64) -> Result<LinuxThreadHandle, u64> {
    if thread_group == 0
        || thread == 0
        || thread_group > i32::MAX as u64
        || thread > i32::MAX as u64
    {
        return Err(LINUX_EINVAL);
    }
    if thread_group != 1 {
        return Err(LINUX_ESRCH);
    }
    linux_thread_handle_by_tid(thread).ok_or(LINUX_ESRCH)
}

fn linux_tkill_identity(thread: u64) -> Result<(), u64> {
    if thread == 0 || thread > i32::MAX as u64 {
        return Err(LINUX_EINVAL);
    }
    if current_linux_tid().map(u64::from) != Some(thread) {
        return Err(LINUX_ESRCH);
    }
    Ok(())
}

fn linux_deliver_pending_signal(
    output: *mut LinuxSyscallFrame,
    interrupted: LinuxUserContext,
    return_registers: (u64, u64),
) -> u64 {
    if LINUX_SIGNAL_ACTIVE.load(Ordering::SeqCst) {
        return 0;
    }
    let standard_signals = (1_u64 << LINUX_STANDARD_SIGNAL_COUNT) - 1;
    let deliverable = ELF_PROBE_PENDING_SIGNALS.load(Ordering::SeqCst)
        & !ELF_PROBE_SIGNAL_MASK.load(Ordering::SeqCst)
        & standard_signals;
    if deliverable == 0 {
        return 0;
    }
    let signal = u64::from(deliverable.trailing_zeros()) + 1;
    let index = signal as usize - 1;
    let signal_bit = 1 << (signal - 1);
    let result = linux_kill(
        1,
        (
            signal,
            ELF_PROBE_PENDING_SIGNAL_CODES[index].load(Ordering::SeqCst) as i32,
        ),
        output,
        interrupted,
        return_registers,
    );
    if result == 0 {
        ELF_PROBE_PENDING_SIGNALS.fetch_and(!signal_bit, Ordering::SeqCst);
        ELF_PROBE_PENDING_SIGNAL_CODES[index].store(0, Ordering::SeqCst);
    }
    result
}

fn deliver_linux_sigsegv(frame: &mut UserFaultFrame, fault: domain::FaultMetadata) -> bool {
    if !frame.is_page_fault()
        || current_linux_thread_handle().is_none()
        || LINUX_SIGNAL_ACTIVE.load(Ordering::SeqCst)
    {
        return false;
    }
    let signal_bit = 1 << (LINUX_SIGSEGV - 1);
    let old_mask = ELF_PROBE_SIGNAL_MASK.load(Ordering::SeqCst);
    if old_mask & signal_bit != 0 {
        return false;
    }
    let index = LINUX_SIGSEGV as usize - 1;
    let handler = ELF_PROBE_SIGNAL_HANDLERS[index].load(Ordering::SeqCst);
    let flags = ELF_PROBE_SIGNAL_FLAGS[index].load(Ordering::SeqCst);
    let restorer = ELF_PROBE_SIGNAL_RESTORERS[index].load(Ordering::SeqCst);
    if !linux_signal_action_deliverable(handler, flags, restorer) {
        return false;
    }
    let interrupted = frame.context();
    let old_alt_stack_flags = ELF_PROBE_ALT_STACK_FLAGS.load(Ordering::SeqCst);
    let use_alt_stack = flags & LINUX_SA_ONSTACK != 0
        && old_alt_stack_flags & (LINUX_SS_DISABLE | LINUX_SS_ONSTACK) == 0;
    let stack_top = if use_alt_stack {
        let pointer = ELF_PROBE_ALT_STACK_POINTER.load(Ordering::SeqCst);
        let size = ELF_PROBE_ALT_STACK_SIZE.load(Ordering::SeqCst);
        let Some(top) = pointer.checked_add(size) else {
            return false;
        };
        top
    } else {
        interrupted.rsp
    };
    let Some(signal_stack) = linux_signal_stack(stack_top, use_alt_stack) else {
        return false;
    };
    let context = LinuxSignalContext::from_fault(
        interrupted,
        signal_stack,
        restorer,
        old_mask,
        old_alt_stack_flags,
    );
    if !context.user_context().is_valid() {
        return false;
    }
    let signal_frame = linux_rt_signal_frame(
        context,
        flags,
        LinuxSignalInformation::fault(
            LINUX_SIGSEGV,
            if fault.error & 1 == 0 {
                LINUX_SEGV_MAPERR
            } else {
                LINUX_SEGV_ACCERR
            },
            fault.address,
        ),
        fault.vector,
        fault.error,
        fault.address,
    );
    if !copy_linux_signal_frame(signal_stack, &signal_frame) {
        return false;
    }
    if !frame.enter_signal_handler(
        handler,
        signal_stack,
        LINUX_SIGSEGV,
        (
            signal_stack + LINUX_SIGNAL_FRAME_INFO_OFFSET,
            signal_stack + LINUX_SIGNAL_FRAME_CONTEXT_OFFSET,
        ),
        USER_CODE_SELECTOR,
        USER_DATA_SELECTOR,
    ) {
        return false;
    }
    // SAFETY: interrupts are disabled from exception entry through IRETQ, and
    // only one bounded Linux signal frame may be active at a time.
    unsafe { core::ptr::addr_of_mut!(LINUX_SIGNAL_CONTEXT).write(context) };
    linux_apply_delivered_signal_action(index, signal_bit, old_mask, flags);
    if use_alt_stack {
        if old_alt_stack_flags & LINUX_SS_AUTODISARM != 0 {
            ELF_PROBE_ALT_STACK_POINTER.store(0, Ordering::SeqCst);
            ELF_PROBE_ALT_STACK_SIZE.store(0, Ordering::SeqCst);
            ELF_PROBE_ALT_STACK_FLAGS.store(LINUX_SS_DISABLE, Ordering::SeqCst);
        } else {
            ELF_PROBE_ALT_STACK_FLAGS.store(LINUX_SS_ONSTACK, Ordering::SeqCst);
        }
    }
    LINUX_SIGNAL_ACTIVE.store(true, Ordering::SeqCst);
    LINUX_SIGNAL_DELIVERY_COUNT.fetch_add(1, Ordering::SeqCst);
    true
}

fn linux_rt_sigreturn(output: *mut LinuxSyscallFrame, user_stack: u64) -> u64 {
    if output.is_null() || !LINUX_SIGNAL_ACTIVE.load(Ordering::SeqCst) {
        return LINUX_EINVAL;
    }
    // SAFETY: signal delivery serializes this context while interrupts remain
    // disabled; active is cleared before another delivery can publish state.
    let context = unsafe { core::ptr::addr_of!(LINUX_SIGNAL_CONTEXT).read() };
    if context.signal_stack.checked_add(8) != Some(user_stack) {
        return LINUX_EINVAL;
    }
    let mut returned = linux_rt_signal_frame(context, 0, LinuxSignalInformation::ZERO, 0, 0, 0);
    if !copy_linux_signal_frame_from_user(context.signal_stack, &mut returned) {
        return LINUX_EINVAL;
    }
    let Some(restored) = linux_restored_signal_context(context, &returned) else {
        return LINUX_EINVAL;
    };
    if !write_linux_live_return(output, restored.user) {
        return LINUX_EINVAL;
    }
    // SAFETY: this serialized return consumes the sole active record before
    // checking whether another unblocked pending signal should replace it.
    unsafe { core::ptr::addr_of_mut!(LINUX_SIGNAL_CONTEXT).write(LinuxSignalContext::ZERO) };
    store_current_linux_signal_mask(restored.signal_mask);
    ELF_PROBE_ALT_STACK_POINTER.store(context.alt_stack_pointer, Ordering::SeqCst);
    ELF_PROBE_ALT_STACK_SIZE.store(context.alt_stack_size, Ordering::SeqCst);
    ELF_PROBE_ALT_STACK_FLAGS.store(context.alt_stack_flags, Ordering::SeqCst);
    LINUX_SIGNAL_ACTIVE.store(false, Ordering::SeqCst);
    LINUX_SIGNAL_RETURN_COUNT.fetch_add(1, Ordering::SeqCst);
    let pending_result =
        linux_deliver_pending_signal(output, restored.user, (restored.rcx, restored.r11));
    if pending_result == 0 && LINUX_SIGNAL_ACTIVE.load(Ordering::SeqCst) {
        return 0;
    }
    // SAFETY: the dedicated IRETQ epilogue consumes these values before this
    // serialized syscall entry can be reused.
    unsafe {
        LINUX_SIGNAL_RAX = restored.user.rax;
        LINUX_SIGNAL_RCX = restored.rcx;
        LINUX_SIGNAL_R11 = restored.r11;
    }
    LINUX_RETURN_FROM_SIGNAL
}

fn abort_linux_signal() {
    if LINUX_SIGNAL_ACTIVE.swap(false, Ordering::SeqCst) {
        // SAFETY: the active context is exclusively owned by this serialized
        // Linux execution while interrupts remain disabled.
        let context = unsafe { core::ptr::addr_of!(LINUX_SIGNAL_CONTEXT).read() };
        store_current_linux_signal_mask(context.signal_mask);
        ELF_PROBE_ALT_STACK_POINTER.store(context.alt_stack_pointer, Ordering::SeqCst);
        ELF_PROBE_ALT_STACK_SIZE.store(context.alt_stack_size, Ordering::SeqCst);
        ELF_PROBE_ALT_STACK_FLAGS.store(context.alt_stack_flags, Ordering::SeqCst);
        unsafe { core::ptr::addr_of_mut!(LINUX_SIGNAL_CONTEXT).write(LinuxSignalContext::ZERO) };
    }
}

fn copy_to_linux_user(address: u64, input: &[u8]) -> bool {
    if !linux_writable_user_range_contains(address, input.len()) {
        return false;
    }
    // SAFETY: every byte is covered by a contiguous chain of declared writable
    // areas that remain mapped in the current root for the whole call.
    unsafe {
        with_supervisor_user_access(|| {
            for (offset, byte) in input.iter().enumerate() {
                (address as *mut u8).add(offset).write_volatile(*byte);
            }
        });
    }
    true
}

pub(crate) fn force_next_control_discovery(domain_id: u64, discovery_word: u64) -> bool {
    if domain_id == 0
        || CONTROL_DISCOVERY_OVERRIDE_DOMAIN
            .compare_exchange(0, u64::MAX, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
    {
        return false;
    }
    CONTROL_DISCOVERY_OVERRIDE_WORD.store(discovery_word, Ordering::Release);
    CONTROL_DISCOVERY_OVERRIDE_DOMAIN.store(domain_id, Ordering::Release);
    true
}

pub(crate) fn force_next_control_descriptor_reserved(domain_id: u64) -> bool {
    domain_id != 0
        && CONTROL_DESCRIPTOR_OVERRIDE_DOMAIN
            .compare_exchange(0, domain_id, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
}

fn dispatch_charged_call(
    number: u64,
    capability: u64,
    caller_domain: u64,
    frame: *mut UserTrapFrame,
) -> u64 {
    match number {
        0 => {
            if CONTROL_DISCOVERY_OVERRIDE_DOMAIN
                .compare_exchange(caller_domain, 0, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                CONTROL_DISCOVERY_OVERRIDE_WORD.load(Ordering::Acquire)
            } else {
                control_abi::DISCOVERY_WORD
            }
        }
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
                timer::mark_scheduler_preemption_ready();
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
        10 => write_abi_descriptor(caller_domain, frame),
        11 => write_fault_descriptor(caller_domain, capability, frame),
        12 => write_lifecycle_descriptor(caller_domain, capability, frame),
        13 => write_ipc_delivery_descriptor(caller_domain, capability, frame),
        14 => write_lifecycle_identity_descriptor(caller_domain, capability, frame),
        15 => create_auxiliary_service(caller_domain, capability, frame),
        16 => network_device_call(caller_domain, capability, frame),
        17 => root_device_call(caller_domain, capability, frame),
        18 => volume_device_call(caller_domain, capability, frame),
        19 => block_device_call(caller_domain, capability, frame),
        20 => network_wait_call(caller_domain, capability, frame),
        control_abi::CONSOLE_READ_LINE_OPERATION => {
            console_read_line(caller_domain, capability, frame)
        }
        ginux_native_abi::NATIVE_CONSOLE_READ_BYTE_OPERATION => {
            console_read_byte(caller_domain, capability, frame)
        }
        control_abi::HOST_INSPECT_OPERATION => host_inspect(caller_domain, capability, frame),
        control_abi::CONTAINER_CONTROL_OPERATION => {
            container_control(caller_domain, capability, frame)
        }
        control_abi::LINUX_ABI_TEST_OPERATION => {
            linux_abi_test_results(caller_domain, capability, frame)
        }
        ginux_native_abi::NATIVE_IMAGE_STORE_OPERATION => {
            image_store_call(caller_domain, capability, frame)
        }
        ginux_native_abi::NATIVE_PROGRESS_OPERATION => {
            native_progress_call(caller_domain, capability, frame)
        }
        control_abi::PACKET_PUBLISH_OPERATION => {
            packet_publish_call(caller_domain, capability, frame)
        }
        control_abi::PACKET_CONSUME_OPERATION => {
            packet_consume_call(caller_domain, capability, frame)
        }
        control_abi::PACKET_WAIT_OPERATION => {
            packet_wait_call(caller_domain, capability, frame, None)
        }
        control_abi::PACKET_WAIT_TIMEOUT_OPERATION => {
            let timeout_ms = unsafe { frame.as_ref() }.map(UserTrapFrame::second_argument);
            packet_wait_call(caller_domain, capability, frame, timeout_ms)
        }
        control_abi::PACKET_SPACE_WAIT_OPERATION => {
            packet_space_wait_call(caller_domain, capability, frame)
        }
        control_abi::PACKET_EGRESS_CLAIM_OPERATION => {
            packet_egress_claim_call(caller_domain, capability, frame)
        }
        control_abi::PACKET_EGRESS_ACK_OPERATION => {
            packet_egress_ack_call(caller_domain, capability, frame)
        }
        control_abi::PACKET_EGRESS_WAIT_OPERATION => packet_wait_call_for_object(
            caller_domain,
            capability,
            frame,
            None,
            control_abi::PACKET_EGRESS_OBJECT,
        ),
        control_abi::PACKET_EGRESS_SPACE_WAIT_OPERATION => packet_space_wait_call_for_object(
            caller_domain,
            capability,
            frame,
            control_abi::PACKET_EGRESS_OBJECT,
        ),
        control_abi::PACKET_EGRESS_DRAIN_WAIT_OPERATION => {
            packet_egress_drain_wait_call(caller_domain, capability, frame)
        }
        _ => u64::MAX,
    }
}

fn native_progress_call(caller_domain: u64, capability: u64, frame: *const UserTrapFrame) -> u64 {
    if !domain::handle_permits(
        caller_domain,
        capability,
        ginux_native_abi::NATIVE_IMAGE_STORE_OBJECT,
        Rights::WRITE,
    ) {
        return u64::MAX - 2;
    }
    let Some(frame) = (unsafe { frame.as_ref() }) else {
        return u64::MAX - 2;
    };
    let Ok(size) = usize::try_from(frame.second_argument()) else {
        return u64::MAX - 2;
    };
    if size == 0 || size > ginux_native_abi::NATIVE_PROGRESS_MAX_SIZE {
        return u64::MAX - 2;
    }
    let mut bytes = [0; ginux_native_abi::NATIVE_PROGRESS_MAX_SIZE];
    if !address_space::copy_from_current_user(frame.first_argument(), &mut bytes[..size])
        || !bytes[..size]
            .iter()
            .all(|byte| byte.is_ascii_graphic() || *byte == b' ')
    {
        return u64::MAX - 2;
    }
    if oci_progress_is_console_visible(&bytes[..size]) {
        serial_write("ginux: OCI ");
        crate::serial_write_bytes(&bytes[..size]);
        serial_write("\r\n");
    }
    if domain::renew_auxiliary_call_epoch(caller_domain) {
        size as u64
    } else {
        u64::MAX - 2
    }
}

fn oci_progress_is_console_visible(message: &[u8]) -> bool {
    message == b"authenticating with registry"
        || message == b"pulling image manifest"
        || message == b"pulling image configuration"
        || message == b"image configuration verified"
        || message.starts_with(b"manifest selected for ")
        || message.starts_with(b"layer ")
        || contains_bytes(message, b"failed")
        || contains_bytes(message, b"rejected")
        || contains_bytes(message, b"unavailable")
        || contains_bytes(message, b"exceeded")
}

fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty()
        && haystack
            .windows(needle.len())
            .any(|window| window == needle)
}

fn image_store_call(caller_domain: u64, capability: u64, frame: *const UserTrapFrame) -> u64 {
    let failed = ginux_native_abi::NATIVE_IMAGE_STORE_RESULT_FAILED;
    let Some(frame) = (unsafe { frame.as_ref() }) else {
        return failed;
    };
    if !domain::handle_permits(
        caller_domain,
        capability,
        ginux_native_abi::NATIVE_IMAGE_STORE_OBJECT,
        Rights::WRITE,
    ) {
        return failed;
    }
    let mut request = [0_u8; ginux_native_abi::NATIVE_IMAGE_STORE_REQUEST_SIZE as usize];
    if !address_space::copy_from_current_user(frame.first_argument(), &mut request)
        || u32::from_le_bytes(request[0..4].try_into().unwrap())
            != ginux_native_abi::NATIVE_IMAGE_STORE_REQUEST_SIZE
        || u16::from_le_bytes(request[4..6].try_into().unwrap())
            != ginux_native_abi::NATIVE_IMAGE_STORE_REQUEST_VERSION
        || request[36..40] != [0; 4]
    {
        return failed;
    }
    let command = u16::from_le_bytes(request[6..8].try_into().unwrap());
    let raw_handle = u64::from_le_bytes(request[8..16].try_into().unwrap());
    let expected_length = u64::from_le_bytes(request[16..24].try_into().unwrap());
    let buffer_address = u64::from_le_bytes(request[24..32].try_into().unwrap());
    let buffer_size = u32::from_le_bytes(request[32..36].try_into().unwrap());
    let digest: [u8; 32] = request[40..72].try_into().unwrap();
    let result = match command {
        ginux_native_abi::NATIVE_IMAGE_STORE_STAGE
            if raw_handle == 0
                && buffer_address == 0
                && buffer_size == 0
                && expected_length != 0
                && digest != [0; 32] =>
        {
            let Ok(expected_length) = usize::try_from(expected_length) else {
                return failed;
            };
            image_store::stage(digest, expected_length)
                .map(ginux_image_store::StagingHandle::into_raw)
                .unwrap_or(failed)
        }
        ginux_native_abi::NATIVE_IMAGE_STORE_APPEND
            if expected_length == 0
                && digest == [0; 32]
                && (1..=ginux_native_abi::NATIVE_IMAGE_STORE_CHUNK_LIMIT)
                    .contains(&buffer_size) =>
        {
            let Some(handle) = ginux_image_store::StagingHandle::from_raw(raw_handle) else {
                return failed;
            };
            let mut bytes = [0_u8; ginux_native_abi::NATIVE_IMAGE_STORE_CHUNK_LIMIT as usize];
            let size = buffer_size as usize;
            if !address_space::copy_from_current_user(buffer_address, &mut bytes[..size]) {
                return failed;
            }
            image_store::append(handle, &bytes[..size])
                .map(|written| written as u64)
                .unwrap_or(failed)
        }
        ginux_native_abi::NATIVE_IMAGE_STORE_PUBLISH
            if expected_length == 0
                && buffer_address == 0
                && buffer_size == 0
                && digest == [0; 32] =>
        {
            let Some(handle) = ginux_image_store::StagingHandle::from_raw(raw_handle) else {
                return failed;
            };
            image_store::publish(handle).map_or(failed, |_| 0)
        }
        ginux_native_abi::NATIVE_IMAGE_STORE_ABORT
            if expected_length == 0
                && buffer_address == 0
                && buffer_size == 0
                && digest == [0; 32] =>
        {
            let Some(handle) = ginux_image_store::StagingHandle::from_raw(raw_handle) else {
                return failed;
            };
            image_store::abort(handle).map_or(failed, |()| 0)
        }
        _ => failed,
    };
    if result != failed && !domain::renew_auxiliary_call_epoch(caller_domain) {
        failed
    } else {
        result
    }
}

struct PacketRequest {
    buffer_address: u64,
    buffer_size: usize,
    expected_sequence: u64,
}

fn packet_request(address: u64) -> Option<PacketRequest> {
    let mut bytes = [0_u8; control_abi::PACKET_REQUEST_SIZE];
    if !address_space::copy_from_current_user(address, &mut bytes)
        || u32::from_le_bytes(bytes[0..4].try_into().ok()?) as usize
            != control_abi::PACKET_REQUEST_SIZE
        || u16::from_le_bytes(bytes[4..6].try_into().ok()?) != control_abi::PACKET_REQUEST_VERSION
        || bytes[6..8] != [0, 0]
        || bytes[20..24] != [0, 0, 0, 0]
    {
        return None;
    }
    Some(PacketRequest {
        buffer_address: u64::from_le_bytes(bytes[8..16].try_into().ok()?),
        buffer_size: usize::try_from(u32::from_le_bytes(bytes[16..20].try_into().ok()?)).ok()?,
        expected_sequence: u64::from_le_bytes(bytes[24..32].try_into().ok()?),
    })
}

fn packet_publish_call(caller_domain: u64, capability: u64, frame: *const UserTrapFrame) -> u64 {
    let Some(frame) = (unsafe { frame.as_ref() }) else {
        return u64::MAX - 2;
    };
    let Some(request) = packet_request(frame.first_argument()) else {
        return u64::MAX - 2;
    };
    let ingress = domain::handle_permits(
        caller_domain,
        capability,
        control_abi::PACKET_OBJECT,
        Rights::WRITE,
    );
    let egress = domain::handle_permits(
        caller_domain,
        capability,
        control_abi::PACKET_EGRESS_OBJECT,
        Rights::WRITE,
    );
    if request.expected_sequence != 0
        || !(1..=control_abi::PACKET_MAX_SIZE).contains(&request.buffer_size)
        || ingress == egress
    {
        return u64::MAX - 2;
    }
    let mut packet = [0_u8; control_abi::PACKET_MAX_SIZE];
    if !address_space::copy_from_current_user(
        request.buffer_address,
        &mut packet[..request.buffer_size],
    ) {
        return u64::MAX - 2;
    }
    let result = if egress {
        crate::packet::publish_egress(caller_domain, &packet[..request.buffer_size])
    } else {
        crate::packet::publish(caller_domain, &packet[..request.buffer_size])
    };
    match result {
        Ok(sequence) => {
            if ingress {
                if scheduler::wake_packet_waiter(caller_domain, sequence) {
                    let _ = timer::cancel_packet_wait_timer_for_sequence(sequence);
                }
            } else {
                let _ = scheduler::wake_egress_waiter(caller_domain, sequence);
            }
            timer::mark_scheduler_preemption_ready();
            sequence
        }
        Err(_) => u64::MAX - 2,
    }
}

fn packet_consume_call(caller_domain: u64, capability: u64, frame: *const UserTrapFrame) -> u64 {
    let Some(frame) = (unsafe { frame.as_ref() }) else {
        return u64::MAX - 2;
    };
    let Some(request) = packet_request(frame.first_argument()) else {
        return u64::MAX - 2;
    };
    let ingress = domain::handle_permits(
        caller_domain,
        capability,
        control_abi::PACKET_OBJECT,
        Rights::READ,
    );
    let egress = domain::handle_permits(
        caller_domain,
        capability,
        control_abi::PACKET_EGRESS_OBJECT,
        Rights::READ,
    );
    if request.expected_sequence == 0
        || !(1..=control_abi::PACKET_MAX_SIZE).contains(&request.buffer_size)
        || ingress == egress
    {
        return u64::MAX - 2;
    }
    let result = if egress {
        crate::packet::consume_egress_with(
            request.expected_sequence,
            request.buffer_size,
            |_, packet| address_space::copy_to_current_user(request.buffer_address, packet),
        )
    } else {
        crate::packet::consume_with(
            request.expected_sequence,
            request.buffer_size,
            |_, packet| address_space::copy_to_current_user(request.buffer_address, packet),
        )
    };
    match result {
        Ok(metadata) => {
            let woke = if ingress {
                scheduler::wake_packet_space_waiter(caller_domain)
            } else {
                scheduler::wake_egress_space_waiter(caller_domain)
            };
            if woke {
                timer::mark_scheduler_preemption_ready();
            }
            metadata.length as u64
        }
        Err(crate::packet::PacketError::Stale) => control_abi::PACKET_RESULT_STALE,
        Err(_) => u64::MAX - 2,
    }
}

fn packet_egress_claim_call(
    caller_domain: u64,
    capability: u64,
    frame: *const UserTrapFrame,
) -> u64 {
    let Some(frame) = (unsafe { frame.as_ref() }) else {
        return u64::MAX - 2;
    };
    let Some(request) = packet_request(frame.first_argument()) else {
        return u64::MAX - 2;
    };
    if request.expected_sequence == 0
        || !(1..=control_abi::PACKET_MAX_SIZE).contains(&request.buffer_size)
        || !domain::handle_permits(
            caller_domain,
            capability,
            control_abi::PACKET_EGRESS_OBJECT,
            Rights::READ,
        )
    {
        return u64::MAX - 2;
    }
    match crate::packet::claim_egress_with(
        caller_domain,
        request.expected_sequence,
        request.buffer_size,
        |_, packet| address_space::copy_to_current_user(request.buffer_address, packet),
    ) {
        Ok(metadata) => metadata.length as u64,
        Err(crate::packet::PacketError::Stale) => control_abi::PACKET_RESULT_STALE,
        Err(_) => u64::MAX - 2,
    }
}

fn packet_egress_ack_call(caller_domain: u64, capability: u64, frame: *const UserTrapFrame) -> u64 {
    let Some(frame) = (unsafe { frame.as_ref() }) else {
        return u64::MAX - 2;
    };
    let Some(request) = packet_request(frame.first_argument()) else {
        return u64::MAX - 2;
    };
    if request.buffer_address != 0
        || request.buffer_size != 0
        || request.expected_sequence == 0
        || !domain::handle_permits(
            caller_domain,
            capability,
            control_abi::PACKET_EGRESS_OBJECT,
            Rights::READ,
        )
        || !address_space::auxiliary_network_egress_transmit_complete(caller_domain)
    {
        return u64::MAX - 2;
    }
    match crate::packet::acknowledge_egress(caller_domain, request.expected_sequence) {
        Ok(_) => {
            if !address_space::record_auxiliary_network_egress_ack(caller_domain) {
                return u64::MAX - 2;
            }
            if scheduler::wake_egress_space_waiter(caller_domain) {
                timer::mark_scheduler_preemption_ready();
            }
            if crate::packet::egress_is_empty()
                && scheduler::wake_egress_drain_waiter(caller_domain)
            {
                timer::mark_scheduler_preemption_ready();
            }
            0
        }
        Err(crate::packet::PacketError::Stale) => control_abi::PACKET_RESULT_STALE,
        Err(_) => u64::MAX - 2,
    }
}

fn packet_wait_call(
    caller_domain: u64,
    capability: u64,
    frame: *mut UserTrapFrame,
    timeout_ms: Option<u64>,
) -> u64 {
    packet_wait_call_for_object(
        caller_domain,
        capability,
        frame,
        timeout_ms,
        control_abi::PACKET_OBJECT,
    )
}

fn packet_wait_call_for_object(
    caller_domain: u64,
    capability: u64,
    frame: *mut UserTrapFrame,
    timeout_ms: Option<u64>,
    object: u64,
) -> u64 {
    let Some(frame) = (unsafe { frame.as_mut() }) else {
        return u64::MAX - 2;
    };
    let Some(request) = packet_request(frame.first_argument()) else {
        return u64::MAX - 2;
    };
    if request.buffer_address != 0
        || request.buffer_size != 0
        || timeout_ms.is_some_and(|timeout| timeout > control_abi::PACKET_WAIT_MAX_TIMEOUT_MS)
    {
        return u64::MAX - 2;
    }
    if request.expected_sequence == 0 {
        return u64::MAX - 2;
    }
    if !domain::handle_permits(caller_domain, capability, object, Rights::READ) {
        return u64::MAX - 2;
    }
    let state = if object == control_abi::PACKET_OBJECT {
        crate::packet::wait_state(request.expected_sequence)
    } else if object == control_abi::PACKET_EGRESS_OBJECT && timeout_ms.is_none() {
        crate::packet::egress_wait_state(request.expected_sequence)
    } else {
        return u64::MAX - 2;
    };
    match state {
        crate::packet::PacketWait::Ready => request.expected_sequence,
        crate::packet::PacketWait::Stale => control_abi::PACKET_RESULT_STALE,
        crate::packet::PacketWait::Empty => {
            if object == control_abi::PACKET_EGRESS_OBJECT
                && timeout_ms.is_none()
                && !crate::packet::egress_producer_is_bound()
                && domain::take_auxiliary_drain_request(caller_domain)
            {
                return control_abi::PACKET_RESULT_DRAINED;
            }
            if timeout_ms == Some(0) {
                return control_abi::PACKET_RESULT_TIMED_OUT;
            }
            let wait_available = if object == control_abi::PACKET_OBJECT {
                scheduler::packet_wait_is_available(request.expected_sequence)
            } else {
                scheduler::egress_wait_is_available(request.expected_sequence)
            };
            if !wait_available {
                return u64::MAX - 2;
            }
            if timeout_ms.is_some_and(|timeout| {
                !timer::arm_packet_wait_timer(caller_domain, request.expected_sequence, timeout)
            }) {
                return u64::MAX - 2;
            }
            let reason = scheduler::WaitReason::PacketAvailable {
                object,
                sequence: request.expected_sequence,
            };
            let action = scheduler::block_runtime(
                frame,
                USER_CODE_SELECTOR,
                USER_DATA_SELECTOR,
                reason,
                active_fs_base(),
            );
            if !matches!(action, RuntimeAction::Invalid) {
                return resume_action(action);
            }
            if timeout_ms.is_some()
                && scheduler::block_runtime_idle(
                    frame,
                    USER_CODE_SELECTOR,
                    USER_DATA_SELECTOR,
                    reason,
                    active_fs_base(),
                )
            {
                while !timer::packet_wait_timed_out(caller_domain, request.expected_sequence) {
                    unsafe { asm!("sti", "hlt", "cli", options(nomem, nostack)) };
                }
                return if timer::finish_packet_wait_timer(caller_domain, request.expected_sequence)
                    == Some(true)
                    && scheduler::current_domain() == Some(caller_domain)
                {
                    control_abi::PACKET_RESULT_TIMED_OUT
                } else {
                    u64::MAX - 2
                };
            }
            let _ = timer::cancel_packet_wait_timer_for_domain(caller_domain);
            u64::MAX - 2
        }
    }
}

fn packet_space_wait_call(caller_domain: u64, capability: u64, frame: *mut UserTrapFrame) -> u64 {
    packet_space_wait_call_for_object(caller_domain, capability, frame, control_abi::PACKET_OBJECT)
}

fn packet_space_wait_call_for_object(
    caller_domain: u64,
    capability: u64,
    frame: *mut UserTrapFrame,
    object: u64,
) -> u64 {
    let Some(frame) = (unsafe { frame.as_mut() }) else {
        return u64::MAX - 2;
    };
    if frame.first_argument() != 0
        || frame.second_argument() != 0
        || !domain::handle_permits(caller_domain, capability, object, Rights::WRITE)
    {
        return u64::MAX - 2;
    }
    let state = if object == control_abi::PACKET_OBJECT {
        crate::packet::space_state(caller_domain)
    } else if object == control_abi::PACKET_EGRESS_OBJECT {
        crate::packet::egress_space_state(caller_domain)
    } else {
        return u64::MAX - 2;
    };
    match state {
        crate::packet::PacketSpace::Ready => 0,
        crate::packet::PacketSpace::InvalidProducer
        | crate::packet::PacketSpace::SequenceExhausted => u64::MAX - 2,
        crate::packet::PacketSpace::Full => {
            let wait_available = if object == control_abi::PACKET_OBJECT {
                scheduler::packet_space_wait_is_available()
            } else {
                scheduler::egress_space_wait_is_available()
            };
            if !wait_available {
                return u64::MAX - 2;
            }
            let action = scheduler::block_runtime(
                frame,
                USER_CODE_SELECTOR,
                USER_DATA_SELECTOR,
                scheduler::WaitReason::PacketSpaceAvailable { object },
                active_fs_base(),
            );
            if matches!(action, RuntimeAction::Invalid) {
                u64::MAX - 2
            } else {
                resume_action(action)
            }
        }
    }
}

fn packet_egress_drain_wait_call(
    caller_domain: u64,
    capability: u64,
    frame: *mut UserTrapFrame,
) -> u64 {
    let Some(frame) = (unsafe { frame.as_mut() }) else {
        return u64::MAX - 2;
    };
    if frame.first_argument() != 0
        || frame.second_argument() != 0
        || !domain::handle_permits(
            caller_domain,
            capability,
            control_abi::PACKET_EGRESS_OBJECT,
            Rights::WRITE,
        )
    {
        return u64::MAX - 2;
    }
    if crate::packet::egress_is_empty() {
        return 0;
    }
    if !scheduler::egress_drain_wait_is_available() {
        return u64::MAX - 2;
    }
    let action = scheduler::block_runtime(
        frame,
        USER_CODE_SELECTOR,
        USER_DATA_SELECTOR,
        scheduler::WaitReason::PacketQueueDrained {
            object: control_abi::PACKET_EGRESS_OBJECT,
        },
        active_fs_base(),
    );
    if matches!(action, RuntimeAction::Invalid) {
        u64::MAX - 2
    } else {
        resume_action(action)
    }
}

fn network_wait_call(caller_domain: u64, capability: u64, frame: *mut UserTrapFrame) -> u64 {
    let Some(frame) = (unsafe { frame.as_mut() }) else {
        return u64::MAX - 2;
    };
    let command = frame.first_argument();
    let request = frame.second_argument();
    if !matches!(
        command,
        control_abi::NETWORK_DEVICE_WAIT_TX_NOTIFIED | control_abi::NETWORK_DEVICE_WAIT_RX_POSTED
    ) || request == 0
        || !domain::handle_permits(
            caller_domain,
            capability,
            control_abi::NETWORK_DEVICE_OBJECT,
            Rights::WRITE,
        )
        || !address_space::auxiliary_network_wait_ready(caller_domain, command, request)
    {
        return u64::MAX - 2;
    }
    let reason = scheduler::WaitReason::DeviceCompletion {
        object: control_abi::NETWORK_DEVICE_OBJECT,
        domain: caller_domain,
        request,
    };
    if !scheduler::block_runtime_idle(
        frame,
        USER_CODE_SELECTOR,
        USER_DATA_SELECTOR,
        reason,
        active_fs_base(),
    ) {
        return u64::MAX - 2;
    }
    match address_space::complete_auxiliary_network_wait(caller_domain, command, request) {
        address_space::AuxiliaryDeviceWaitCompletion::Completed => {
            if domain::renew_auxiliary_call_epoch(caller_domain) {
                0
            } else {
                u64::MAX - 2
            }
        }
        address_space::AuxiliaryDeviceWaitCompletion::Cancelled => {
            control_abi::NETWORK_DEVICE_RESULT_CANCELLED
        }
        address_space::AuxiliaryDeviceWaitCompletion::TimedOut
        | address_space::AuxiliaryDeviceWaitCompletion::Failed => {
            let _ = scheduler::wake_device_completion(
                caller_domain,
                control_abi::NETWORK_DEVICE_OBJECT,
                request,
                u64::MAX - 2,
            );
            u64::MAX - 2
        }
    }
}

fn block_device_call(caller_domain: u64, capability: u64, frame: *mut UserTrapFrame) -> u64 {
    let Some(frame) = (unsafe { frame.as_mut() }) else {
        return u64::MAX - 2;
    };
    let command = frame.first_argument();
    let request = frame.second_argument();
    let notified = match command {
        control_abi::BLOCK_DEVICE_WAIT => false,
        control_abi::BLOCK_DEVICE_WAIT_NOTIFIED => true,
        _ => return u64::MAX - 2,
    };
    if request == 0
        || !domain::handle_permits(
            caller_domain,
            capability,
            control_abi::BLOCK_DEVICE_OBJECT,
            Rights::WRITE,
        )
        || !address_space::auxiliary_device_wait_ready(caller_domain, request)
    {
        return u64::MAX - 2;
    }
    let reason = scheduler::WaitReason::DeviceCompletion {
        object: control_abi::BLOCK_DEVICE_OBJECT,
        domain: caller_domain,
        request,
    };
    if !scheduler::block_runtime_idle(
        frame,
        USER_CODE_SELECTOR,
        USER_DATA_SELECTOR,
        reason,
        active_fs_base(),
    ) {
        return u64::MAX - 2;
    }
    match address_space::complete_auxiliary_device_wait(caller_domain, request, notified) {
        address_space::AuxiliaryDeviceWaitCompletion::Completed => 0,
        address_space::AuxiliaryDeviceWaitCompletion::Cancelled => {
            control_abi::BLOCK_DEVICE_RESULT_CANCELLED
        }
        address_space::AuxiliaryDeviceWaitCompletion::TimedOut => u64::MAX - 2,
        address_space::AuxiliaryDeviceWaitCompletion::Failed => {
            let _ = scheduler::wake_device_completion(
                caller_domain,
                control_abi::BLOCK_DEVICE_OBJECT,
                request,
                u64::MAX - 2,
            );
            u64::MAX - 2
        }
    }
}

fn volume_device_call(caller_domain: u64, capability: u64, frame: *const UserTrapFrame) -> u64 {
    let Some(frame) = (unsafe { frame.as_ref() }) else {
        return u64::MAX - 2;
    };
    let command = frame.first_argument();
    let argument = frame.second_argument();
    match command {
        control_abi::VOLUME_PROBE
            if argument == 0
                && domain::handle_permits(
                    caller_domain,
                    capability,
                    control_abi::VOLUME_OBJECT,
                    Rights::READ,
                ) =>
        {
            control_abi::VOLUME_DESCRIPTOR_WORD
        }
        control_abi::VOLUME_QUERY_GENERATION
            if argument == 0
                && domain::handle_permits(
                    caller_domain,
                    capability,
                    control_abi::VOLUME_OBJECT,
                    Rights::READ,
                ) =>
        {
            volume::live_generation().unwrap_or(u64::MAX - 2)
        }
        control_abi::VOLUME_READ
            if domain::handle_permits(
                caller_domain,
                capability,
                control_abi::VOLUME_OBJECT,
                Rights::READ,
            ) =>
        {
            volume_read(argument)
        }
        control_abi::VOLUME_COMMIT
            if domain::handle_permits(
                caller_domain,
                capability,
                control_abi::VOLUME_OBJECT,
                Rights::WRITE,
            ) =>
        {
            volume_commit(argument)
        }
        _ => u64::MAX - 2,
    }
}

fn volume_read(address: u64) -> u64 {
    let Some(request) = volume_request(address) else {
        return u64::MAX - 2;
    };
    let Ok(capacity) = usize::try_from(request.size) else {
        return u64::MAX - 2;
    };
    if !(1..=control_abi::VOLUME_MAX_TRANSFER_SIZE).contains(&capacity) {
        return u64::MAX - 2;
    }
    let mut output = [0_u8; control_abi::VOLUME_MAX_TRANSFER_SIZE];
    match volume::read_live(request.expected_generation, &mut output[..capacity]) {
        Ok(size)
            if address_space::copy_to_current_user(request.buffer_address, &output[..size]) =>
        {
            size as u64
        }
        Ok(_) => u64::MAX - 2,
        Err(volume::LiveError::Stale) => control_abi::VOLUME_RESULT_STALE_GENERATION,
        Err(_) => u64::MAX - 2,
    }
}

fn volume_commit(address: u64) -> u64 {
    let Some(request) = volume_request(address) else {
        return u64::MAX - 2;
    };
    let Ok(size) = usize::try_from(request.size) else {
        return u64::MAX - 2;
    };
    if !(1..=control_abi::VOLUME_MAX_TRANSFER_SIZE).contains(&size) {
        return u64::MAX - 2;
    }
    let mut payload = [0_u8; control_abi::VOLUME_MAX_TRANSFER_SIZE];
    if !address_space::copy_from_current_user(request.buffer_address, &mut payload[..size]) {
        return u64::MAX - 2;
    }
    match volume::commit_live(request.expected_generation, &payload[..size]) {
        Ok(generation) => generation,
        Err(volume::LiveError::Stale) => control_abi::VOLUME_RESULT_STALE_GENERATION,
        Err(_) => u64::MAX - 2,
    }
}

struct VolumeRequest {
    expected_generation: u64,
    buffer_address: u64,
    size: u64,
}

fn volume_request(address: u64) -> Option<VolumeRequest> {
    let mut request = [0_u8; control_abi::VOLUME_REQUEST_SIZE];
    if !address_space::copy_from_current_user(address, &mut request)
        || u32::from_le_bytes(request[0..4].try_into().ok()?) as usize
            != control_abi::VOLUME_REQUEST_SIZE
        || u16::from_le_bytes(request[4..6].try_into().ok()?) != control_abi::VERSION
        || request[6..8] != [0, 0]
    {
        return None;
    }
    Some(VolumeRequest {
        expected_generation: u64::from_le_bytes(request[8..16].try_into().ok()?),
        buffer_address: u64::from_le_bytes(request[16..24].try_into().ok()?),
        size: u64::from_le_bytes(request[24..32].try_into().ok()?),
    })
}

fn root_device_call(caller_domain: u64, capability: u64, frame: *const UserTrapFrame) -> u64 {
    let Some(frame) = (unsafe { frame.as_ref() }) else {
        return u64::MAX - 2;
    };
    if !domain::handle_permits(
        caller_domain,
        capability,
        control_abi::ROOT_OBJECT,
        Rights::READ,
    ) || !initramfs::published_root_is_live()
    {
        return u64::MAX - 2;
    }
    let command = frame.first_argument();
    let argument = frame.second_argument();
    match command {
        control_abi::ROOT_PROBE if argument == 0 => control_abi::ROOT_DESCRIPTOR_WORD,
        control_abi::ROOT_LOOKUP => root_lookup(argument),
        control_abi::ROOT_READ => root_read(argument),
        control_abi::ROOT_READ_DIRECTORY => root_read_directory(argument),
        _ => u64::MAX - 2,
    }
}

fn root_lookup(address: u64) -> u64 {
    let mut request = [0_u8; control_abi::ROOT_LOOKUP_REQUEST_SIZE];
    if !address_space::copy_from_current_user(address, &mut request)
        || !root_request_header(&request, control_abi::ROOT_LOOKUP_REQUEST_SIZE)
    {
        return u64::MAX - 2;
    }
    let path_address = root_request_word(&request, 8);
    let path_size = root_request_word(&request, 16);
    let output_address = root_request_word(&request, 24);
    let output_size = root_request_word(&request, 32);
    let Some(path) = copy_root_path(path_address, path_size) else {
        return u64::MAX - 2;
    };
    if output_size != control_abi::ROOT_LOOKUP_DESCRIPTOR_SIZE as u64 {
        return u64::MAX - 2;
    }
    let Some((mode, data_size)) = initramfs::published_lookup(path.as_slice()) else {
        return u64::MAX - 2;
    };
    let Some(descriptor) = control_abi::root_lookup_descriptor(mode, data_size) else {
        return u64::MAX - 2;
    };
    if address_space::copy_to_current_user(output_address, &descriptor) {
        descriptor.len() as u64
    } else {
        u64::MAX - 2
    }
}

fn root_read(address: u64) -> u64 {
    let mut request = [0_u8; control_abi::ROOT_READ_REQUEST_SIZE];
    if !address_space::copy_from_current_user(address, &mut request)
        || !root_request_header(&request, control_abi::ROOT_READ_REQUEST_SIZE)
    {
        return u64::MAX - 2;
    }
    let path_address = root_request_word(&request, 8);
    let path_size = root_request_word(&request, 16);
    let offset = root_request_word(&request, 24);
    let output_address = root_request_word(&request, 32);
    let output_size = root_request_word(&request, 40);
    let (Some(path), Ok(offset), Ok(output_size)) = (
        copy_root_path(path_address, path_size),
        usize::try_from(offset),
        usize::try_from(output_size),
    ) else {
        return u64::MAX - 2;
    };
    if !(1..=control_abi::ROOT_MAX_READ_SIZE).contains(&output_size) {
        return u64::MAX - 2;
    }
    let mut output = [0_u8; control_abi::ROOT_MAX_READ_SIZE];
    let Some(size) = initramfs::published_read(path.as_slice(), offset, &mut output[..output_size])
    else {
        return u64::MAX - 2;
    };
    if size == 0 || address_space::copy_to_current_user(output_address, &output[..size]) {
        size as u64
    } else {
        u64::MAX - 2
    }
}

fn root_read_directory(address: u64) -> u64 {
    let mut request = [0_u8; control_abi::ROOT_DIRECTORY_REQUEST_SIZE];
    if !address_space::copy_from_current_user(address, &mut request)
        || !root_request_header(&request, control_abi::ROOT_DIRECTORY_REQUEST_SIZE)
    {
        return u64::MAX - 2;
    }
    let directory_address = root_request_word(&request, 8);
    let directory_size = root_request_word(&request, 16);
    let cursor = root_request_word(&request, 24);
    let output_address = root_request_word(&request, 32);
    let output_capacity = root_request_word(&request, 40);
    let directory = if directory_size == 0 {
        if directory_address != 0 {
            return u64::MAX - 2;
        }
        None
    } else {
        let Some(path) = copy_root_path(directory_address, directory_size) else {
            return u64::MAX - 2;
        };
        Some(path)
    };
    let (Ok(cursor), Ok(output_capacity)) =
        (usize::try_from(cursor), usize::try_from(output_capacity))
    else {
        return u64::MAX - 2;
    };
    if !(control_abi::ROOT_DIRECTORY_RECORD_HEADER_SIZE
        ..=control_abi::ROOT_MAX_DIRECTORY_RECORD_SIZE)
        .contains(&output_capacity)
    {
        return u64::MAX - 2;
    }
    let directory = directory.as_ref().map_or(&[][..], RootPath::as_slice);
    let Some(entry) = initramfs::published_next_child(directory, cursor) else {
        return u64::MAX - 2;
    };
    let Some(entry) = entry else {
        return 0;
    };
    let Some(record_size) =
        control_abi::ROOT_DIRECTORY_RECORD_HEADER_SIZE.checked_add(entry.name.len())
    else {
        return u64::MAX - 2;
    };
    if record_size > output_capacity {
        return u64::MAX - 2;
    }
    let mut output = [0_u8; control_abi::ROOT_MAX_DIRECTORY_RECORD_SIZE];
    output[0..4].copy_from_slice(&(record_size as u32).to_le_bytes());
    output[4..6].copy_from_slice(&control_abi::VERSION.to_le_bytes());
    output[8..16].copy_from_slice(&(entry.next_cursor as u64).to_le_bytes());
    output[16..20].copy_from_slice(&entry.mode.to_le_bytes());
    output[20..24].copy_from_slice(&(entry.name.len() as u32).to_le_bytes());
    output[24..32].copy_from_slice(&(entry.data_size as u64).to_le_bytes());
    output[32..record_size].copy_from_slice(entry.name);
    if address_space::copy_to_current_user(output_address, &output[..record_size]) {
        record_size as u64
    } else {
        u64::MAX - 2
    }
}

struct RootPath {
    bytes: [u8; control_abi::ROOT_MAX_PATH_SIZE],
    size: usize,
}

impl RootPath {
    fn as_slice(&self) -> &[u8] {
        &self.bytes[..self.size]
    }
}

fn copy_root_path(address: u64, size: u64) -> Option<RootPath> {
    let size = usize::try_from(size).ok()?;
    if !(1..=control_abi::ROOT_MAX_PATH_SIZE).contains(&size) {
        return None;
    }
    let mut path = RootPath {
        bytes: [0; control_abi::ROOT_MAX_PATH_SIZE],
        size,
    };
    address_space::copy_from_current_user(address, &mut path.bytes[..size]).then_some(path)
}

fn root_request_header(request: &[u8], expected_size: usize) -> bool {
    u32::from_le_bytes(request[0..4].try_into().unwrap()) as usize == expected_size
        && u16::from_le_bytes(request[4..6].try_into().unwrap()) == control_abi::VERSION
        && request[6..8] == [0, 0]
}

fn root_request_word(request: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(request[offset..offset + 8].try_into().unwrap())
}

fn network_device_call(caller_domain: u64, capability: u64, frame: *mut UserTrapFrame) -> u64 {
    let Some(frame) = (unsafe { frame.as_mut() }) else {
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
        control_abi::NETWORK_DEVICE_ACTIVATE_EMPTY_QUEUES
            if argument == 0
                && domain::handle_permits(
                    caller_domain,
                    capability,
                    control_abi::NETWORK_DEVICE_OBJECT,
                    Rights::WRITE,
                )
                && address_space::activate_auxiliary_network_empty_queues(caller_domain) =>
        {
            0
        }
        control_abi::NETWORK_DEVICE_PING
            if argument != 0
                && argument >> 32 == 0
                && network::link_is_up()
                && network::ipv4_configuration() as u32 == argument as u32
                && domain::handle_permits(
                    caller_domain,
                    capability,
                    control_abi::NETWORK_DEVICE_OBJECT,
                    Rights::READ,
                ) =>
        {
            NETWORK_PING_RESULT.store(1, Ordering::Release);
            0
        }
        control_abi::NETWORK_DEVICE_PING
            if argument != 0
                && argument >> 32 == 0
                && network::link_is_up()
                && network::ipv4_configuration() != 0
                && network::ipv4_gateway() != 0
                && domain::handle_permits(
                    caller_domain,
                    capability,
                    control_abi::NETWORK_DEVICE_OBJECT,
                    Rights::READ,
                )
                && NETWORK_PING_REQUESTED
                    .compare_exchange(0, argument, Ordering::AcqRel, Ordering::Acquire)
                    .is_ok() =>
        {
            NETWORK_PING_RESULT.store(0, Ordering::Release);
            if !domain::renew_auxiliary_call_epoch(caller_domain)
                || !scheduler::suspend_container_lifecycle(
                    frame,
                    USER_CODE_SELECTOR,
                    USER_DATA_SELECTOR,
                    0,
                    active_fs_base(),
                )
            {
                NETWORK_PING_REQUESTED.store(0, Ordering::Release);
                return u64::MAX - 2;
            }
            u64::MAX
        }
        control_abi::NETWORK_DEVICE_QUERY_PING
            if argument == 0
                && domain::handle_permits(
                    caller_domain,
                    capability,
                    control_abi::NETWORK_DEVICE_OBJECT,
                    Rights::READ,
                ) =>
        {
            u64::from(NETWORK_PING_RESULT.swap(0, Ordering::AcqRel))
        }
        ginux_native_abi::NATIVE_NETWORK_DEVICE_DHCP
            if argument == 0
                && domain::handle_permits(
                    caller_domain,
                    capability,
                    control_abi::NETWORK_DEVICE_OBJECT,
                    Rights::WRITE,
                )
                && NETWORK_DHCP_REQUESTED
                    .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                    .is_ok() =>
        {
            NETWORK_DHCP_RESULT.store(0, Ordering::Release);
            if !domain::renew_auxiliary_call_epoch(caller_domain)
                || !scheduler::suspend_container_lifecycle(
                    frame,
                    USER_CODE_SELECTOR,
                    USER_DATA_SELECTOR,
                    0,
                    active_fs_base(),
                )
            {
                NETWORK_DHCP_REQUESTED.store(false, Ordering::Release);
                return u64::MAX - 2;
            }
            u64::MAX
        }
        ginux_native_abi::NATIVE_NETWORK_DEVICE_QUERY_DHCP
            if argument == 0
                && domain::handle_permits(
                    caller_domain,
                    capability,
                    control_abi::NETWORK_DEVICE_OBJECT,
                    Rights::READ,
                ) =>
        {
            u64::from(NETWORK_DHCP_RESULT.swap(0, Ordering::AcqRel))
        }
        control_abi::NETWORK_DEVICE_QUERY_PING_TARGET
            if argument == 0
                && domain::handle_permits(
                    caller_domain,
                    capability,
                    control_abi::NETWORK_DEVICE_OBJECT,
                    Rights::READ,
                ) =>
        {
            NETWORK_PING_ACTIVE.load(Ordering::Acquire)
        }
        control_abi::NETWORK_DEVICE_SET_DNS
            if domain::handle_permits(
                caller_domain,
                capability,
                control_abi::NETWORK_DEVICE_OBJECT,
                Rights::WRITE,
            ) =>
        {
            network_result(network::set_dns(argument))
        }
        control_abi::NETWORK_DEVICE_QUERY_DNS
            if argument == 0
                && domain::handle_permits(
                    caller_domain,
                    capability,
                    control_abi::NETWORK_DEVICE_OBJECT,
                    Rights::READ,
                ) =>
        {
            network::dns()
        }
        _ => u64::MAX - 2,
    }
}

pub(crate) fn take_network_ping_request() -> Option<u32> {
    let target = NETWORK_PING_REQUESTED.swap(0, Ordering::AcqRel);
    NETWORK_PING_ACTIVE.store(target, Ordering::Release);
    u32::try_from(target).ok().filter(|target| *target != 0)
}

pub(crate) fn complete_network_ping(success: bool) {
    NETWORK_PING_ACTIVE.store(0, Ordering::Release);
    NETWORK_PING_RESULT.store(if success { 1 } else { 2 }, Ordering::Release);
}

pub(crate) fn take_network_dhcp_request() -> bool {
    NETWORK_DHCP_REQUESTED.swap(false, Ordering::AcqRel)
}

pub(crate) fn complete_network_dhcp(success: bool) {
    NETWORK_DHCP_RESULT.store(if success { 1 } else { 2 }, Ordering::Release);
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
        control_abi::MANAGEMENT_DRAIN_AUXILIARY
            if request.object == 0
                && request.rights == 0
                && domain::request_auxiliary_drain(caller_domain, capability, request.source) =>
        {
            0
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
        active_fs_base(),
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

fn console_read_line(domain_id: u64, capability: u64, frame: *const UserTrapFrame) -> u64 {
    let Some(frame) = (unsafe { frame.as_ref() }) else {
        return u64::MAX - 2;
    };
    if !domain::handle_permits(domain_id, capability, 2, Rights::READ) {
        return u64::MAX - 2;
    }
    let Ok(capacity) = usize::try_from(frame.second_argument()) else {
        return u64::MAX - 2;
    };
    if !(1..=control_abi::CONSOLE_MAX_LINE_BYTES).contains(&capacity) {
        return u64::MAX - 2;
    }
    let mut line = [0_u8; control_abi::CONSOLE_MAX_LINE_BYTES];
    let mut length = 0;
    loop {
        match crate::serial_read_byte_blocking() {
            b'\r' | b'\n' => {
                crate::serial_write_bytes(b"\r\n");
                break;
            }
            0x08 | 0x7f if length != 0 => {
                length -= 1;
                crate::serial_write_bytes(b"\x08 \x08");
            }
            byte @ 0x20..=0x7e if length < capacity => {
                line[length] = byte;
                length += 1;
                crate::serial_write_bytes(&[byte]);
            }
            _ => {}
        }
    }
    if length != 0 && !address_space::copy_to_current_user(frame.first_argument(), &line[..length])
        || !domain::renew_auxiliary_call_epoch(domain_id)
    {
        return u64::MAX - 2;
    }
    length as u64
}

fn console_read_byte(domain_id: u64, capability: u64, frame: *const UserTrapFrame) -> u64 {
    let Some(frame) = (unsafe { frame.as_ref() }) else {
        return u64::MAX - 2;
    };
    if !domain::handle_permits(domain_id, capability, 2, Rights::READ)
        || frame.first_argument() != 0
        || frame.second_argument() != 0
    {
        return u64::MAX - 2;
    }
    let byte = crate::serial_read_byte_blocking();
    if !domain::renew_auxiliary_call_epoch(domain_id) {
        return u64::MAX - 2;
    }
    u64::from(byte)
}

fn host_inspect(domain_id: u64, capability: u64, frame: *const UserTrapFrame) -> u64 {
    let Some(frame) = (unsafe { frame.as_ref() }) else {
        return u64::MAX - 2;
    };
    if !domain::handle_permits(domain_id, capability, 1, Rights::READ)
        || frame.second_argument() != 0
    {
        return u64::MAX - 2;
    }
    let mut request = [0_u8; control_abi::HOST_INSPECT_REQUEST_SIZE];
    if !address_space::copy_from_current_user(frame.first_argument(), &mut request)
        || u32::from_le_bytes(request[0..4].try_into().unwrap()) as usize != request.len()
        || u16::from_le_bytes(request[4..6].try_into().unwrap())
            != control_abi::HOST_INSPECT_VERSION
        || u64::from_le_bytes(request[24..32].try_into().unwrap())
            != control_abi::HOST_INSPECT_RECORD_SIZE as u64
    {
        return u64::MAX - 2;
    }
    let command = u16::from_le_bytes(request[6..8].try_into().unwrap());
    let selector = u64::from_le_bytes(request[8..16].try_into().unwrap());
    let output_address = u64::from_le_bytes(request[16..24].try_into().unwrap());
    let record = match command {
        control_abi::HOST_PROC_ENUMERATE => {
            let Ok(cursor) = usize::try_from(selector) else {
                return u64::MAX - 2;
            };
            let container_cursor_base = domain::HOST_DOMAIN_CURSOR_END as usize + 1;
            let container_cursor_end = container_cursor_base + control_abi::CONTAINER_LIMIT;
            if cursor == container_cursor_end {
                return if domain::renew_auxiliary_call_epoch(domain_id) {
                    0
                } else {
                    u64::MAX - 2
                };
            }
            if cursor <= domain::HOST_DOMAIN_CURSOR_END as usize {
                let Some(record) = domain::inspect_live_domain(cursor) else {
                    return u64::MAX - 2;
                };
                if let Some(record) = record {
                    record
                } else if let Some(slot) = next_running_container_slot(0) {
                    domain::InspectionRecord {
                        pid: 100 + slot as u64,
                        parent_pid: 0,
                        kind: control_abi::HOST_PROCESS_KIND_CONTAINER_INIT,
                        state: control_abi::HOST_INSPECT_STATE_READY,
                        active_threads: 1,
                        next_cursor: (container_cursor_base + slot + 1) as u64,
                    }
                } else {
                    return if domain::renew_auxiliary_call_epoch(domain_id) {
                        0
                    } else {
                        u64::MAX - 2
                    };
                }
            } else {
                let Some(start) = cursor.checked_sub(container_cursor_base) else {
                    return u64::MAX - 2;
                };
                let Some(slot) = next_running_container_slot(start) else {
                    return if domain::renew_auxiliary_call_epoch(domain_id) {
                        0
                    } else {
                        u64::MAX - 2
                    };
                };
                domain::InspectionRecord {
                    pid: 100 + slot as u64,
                    parent_pid: 0,
                    kind: control_abi::HOST_PROCESS_KIND_CONTAINER_INIT,
                    state: control_abi::HOST_INSPECT_STATE_READY,
                    active_threads: 1,
                    next_cursor: (container_cursor_base + slot + 1) as u64,
                }
            }
        }
        control_abi::HOST_PROC_STATUS => {
            if let Some(slot) = selector
                .checked_sub(100)
                .and_then(|slot| usize::try_from(slot).ok())
                .filter(|slot| {
                    *slot < control_abi::CONTAINER_LIMIT
                        && container_state(*slot) == control_abi::CONTAINER_STATE_RUNNING
                })
            {
                domain::InspectionRecord {
                    pid: 100 + slot as u64,
                    parent_pid: 0,
                    kind: control_abi::HOST_PROCESS_KIND_CONTAINER_INIT,
                    state: control_abi::HOST_INSPECT_STATE_READY,
                    active_threads: 1,
                    next_cursor: 0,
                }
            } else {
                let Some(record) = domain::inspect_host_process(selector) else {
                    return u64::MAX - 2;
                };
                record
            }
        }
        _ => return u64::MAX - 2,
    };
    let mut output = [0_u8; control_abi::HOST_INSPECT_RECORD_SIZE];
    output[0..4].copy_from_slice(&(control_abi::HOST_INSPECT_RECORD_SIZE as u32).to_le_bytes());
    output[4..6].copy_from_slice(&control_abi::HOST_INSPECT_VERSION.to_le_bytes());
    output[6..8].copy_from_slice(&record.kind.to_le_bytes());
    output[8..10].copy_from_slice(&record.state.to_le_bytes());
    output[12..16].copy_from_slice(&record.active_threads.to_le_bytes());
    output[16..24].copy_from_slice(&record.pid.to_le_bytes());
    output[24..32].copy_from_slice(&record.parent_pid.to_le_bytes());
    output[32..40].copy_from_slice(&record.next_cursor.to_le_bytes());
    if address_space::copy_to_current_user(output_address, &output)
        && domain::renew_auxiliary_call_epoch(domain_id)
    {
        output.len() as u64
    } else {
        u64::MAX - 2
    }
}

pub(crate) fn publish_linux_abi_test_results() -> bool {
    const VERIFIED_LINUX_PROFILE: u64 = (1 << 16) - 1;

    if control_abi::LINUX_ABI_TEST_COUNT > u64::BITS as usize {
        return false;
    }
    LINUX_ABI_TEST_RESULTS
        .compare_exchange(
            0,
            VERIFIED_LINUX_PROFILE,
            Ordering::Release,
            Ordering::Relaxed,
        )
        .is_ok()
}

fn linux_abi_test_results(domain_id: u64, capability: u64, frame: *const UserTrapFrame) -> u64 {
    let Some(frame) = (unsafe { frame.as_ref() }) else {
        return u64::MAX - 2;
    };
    let results = LINUX_ABI_TEST_RESULTS.load(Ordering::Acquire);
    if results == 0
        || !domain::handle_permits(domain_id, capability, 1, Rights::READ)
        || frame.first_argument() != 0
        || frame.second_argument() != 0
        || !domain::renew_auxiliary_call_epoch(domain_id)
    {
        u64::MAX - 2
    } else {
        results
    }
}

fn container_control(domain_id: u64, capability: u64, frame: *mut UserTrapFrame) -> u64 {
    let Some(frame) = (unsafe { frame.as_mut() }) else {
        return u64::MAX - 2;
    };
    if !domain::handle_permits(domain_id, capability, 1, Rights::MANAGE) {
        return u64::MAX - 2;
    }
    let operation = frame.first_argument();
    if operation == control_abi::CONTAINER_HOST_SHUTDOWN && frame.second_argument() == 0 {
        CONTAINER_HOST_SHUTDOWN_REQUESTED.store(true, Ordering::SeqCst);
        return active_container_slot()
            .map(|slot| u64::from(CONTAINER_CONTROL_STATES[slot].load(Ordering::SeqCst)))
            .unwrap_or(0);
    }
    let Some(mut request) = read_container_request(frame.second_argument()) else {
        return u64::MAX - 2;
    };
    let name = container_request_name(&request);
    let requested_slot = container_slot_by_name(name);
    let result = match operation {
        control_abi::CONTAINER_QUERY if name.is_empty() => {
            let Ok(cursor) = usize::try_from(read_u64_at(&request, 24)) else {
                return u64::MAX - 2;
            };
            let Some(slot) = next_container_slot(cursor) else {
                return if domain::renew_auxiliary_call_epoch(domain_id) {
                    0
                } else {
                    u64::MAX - 2
                };
            };
            write_container_request(&mut request, slot, next_container_cursor(slot + 1));
            if !address_space::copy_to_current_user(frame.second_argument(), &request) {
                return u64::MAX - 2;
            }
            u64::from(CONTAINER_CONTROL_STATES[slot].load(Ordering::SeqCst))
        }
        control_abi::CONTAINER_QUERY if requested_slot.is_some() => {
            let slot = requested_slot.unwrap_or(0);
            write_container_request(&mut request, slot, control_abi::CONTAINER_CURSOR_END);
            if !address_space::copy_to_current_user(frame.second_argument(), &request) {
                return u64::MAX - 2;
            }
            u64::from(CONTAINER_CONTROL_STATES[slot].load(Ordering::SeqCst))
        }
        control_abi::CONTAINER_CREATE if requested_slot.is_none() && valid_container_name(name) => {
            let Some(slot) = free_container_slot() else {
                return u64::MAX - 2;
            };
            let Some(container_domain) = domain::create_container_domain(domain_id, slot, 1, 128)
            else {
                return u64::MAX - 2;
            };
            CONTAINER_DOMAIN_IDS[slot].store(container_domain, Ordering::SeqCst);
            CONTAINER_CPU_COUNTS[slot].store(1, Ordering::SeqCst);
            CONTAINER_MEMORY_MIB[slot].store(128, Ordering::SeqCst);
            store_container_name(slot, name);
            store_container_value(
                b"/sbin/init",
                &CONTAINER_INITS[slot],
                &CONTAINER_INIT_SIZES[slot],
            );
            CONTAINER_CONFIG_FLAGS[slot].store(
                control_abi::CONTAINER_CONFIG_INIT
                    | control_abi::CONTAINER_CONFIG_CPU
                    | control_abi::CONTAINER_CONFIG_MEMORY,
                Ordering::SeqCst,
            );
            CONTAINER_CONTROL_STATES[slot]
                .store(control_abi::CONTAINER_STATE_CREATED as u8, Ordering::SeqCst);
            control_abi::CONTAINER_STATE_CREATED
        }
        control_abi::CONTAINER_SET_INIT
            if configurable_container(requested_slot)
                && valid_container_init(container_request_init(&request)) =>
        {
            let slot = requested_slot.unwrap_or(0);
            store_container_value(
                container_request_init(&request),
                &CONTAINER_INITS[slot],
                &CONTAINER_INIT_SIZES[slot],
            );
            update_container_configuration(slot, control_abi::CONTAINER_CONFIG_INIT)
        }
        control_abi::CONTAINER_SET_CPU
            if configurable_container(requested_slot) && read_u16_at(&request, 70) != 0 =>
        {
            let slot = requested_slot.unwrap_or(0);
            let cpu_count = read_u16_at(&request, 70);
            let memory_mib = CONTAINER_MEMORY_MIB[slot].load(Ordering::Acquire);
            if !domain::configure_container_domain_limits(
                CONTAINER_DOMAIN_IDS[slot].load(Ordering::Acquire),
                cpu_count,
                memory_mib,
            ) {
                return u64::MAX - 2;
            }
            CONTAINER_CPU_COUNTS[slot].store(cpu_count, Ordering::Release);
            update_container_configuration(slot, control_abi::CONTAINER_CONFIG_CPU)
        }
        control_abi::CONTAINER_SET_MEMORY_MIB
            if configurable_container(requested_slot) && read_u64_at(&request, 200) != 0 =>
        {
            let slot = requested_slot.unwrap_or(0);
            let memory_mib = read_u64_at(&request, 200);
            let cpu_count = CONTAINER_CPU_COUNTS[slot].load(Ordering::Acquire);
            if !domain::configure_container_domain_limits(
                CONTAINER_DOMAIN_IDS[slot].load(Ordering::Acquire),
                cpu_count,
                memory_mib,
            ) {
                return u64::MAX - 2;
            }
            CONTAINER_MEMORY_MIB[slot].store(memory_mib, Ordering::Release);
            update_container_configuration(slot, control_abi::CONTAINER_CONFIG_MEMORY)
        }
        control_abi::CONTAINER_SET_AUTOSTART
            if configurable_container(requested_slot) && read_u64_at(&request, 24) <= 1 =>
        {
            update_container_autostart(requested_slot.unwrap_or(0), read_u64_at(&request, 24) == 1)
        }
        control_abi::CONTAINER_SET_IMAGE
            if configurable_container(requested_slot)
                && valid_container_image(container_request_image(&request)) =>
        {
            let slot = requested_slot.unwrap_or(0);
            let mut canonical = [0_u8; control_abi::CONTAINER_MAX_IMAGE_SIZE];
            let canonical_size =
                canonical_container_image(container_request_image(&request), &mut canonical)
                    .unwrap_or(0);
            let canonical = &canonical[..canonical_size];
            store_container_value(
                canonical,
                &CONTAINER_IMAGES[slot],
                &CONTAINER_IMAGE_SIZES[slot],
            );
            if crate::oci_image::is_installed(slot) && !crate::image_store::uninstall_rootfs(slot) {
                return u64::MAX - 2;
            }
            update_container_configuration(slot, control_abi::CONTAINER_CONFIG_IMAGE)
        }
        control_abi::CONTAINER_STOP
            if requested_slot.is_some()
                && container_state(requested_slot.unwrap_or(0))
                    == control_abi::CONTAINER_STATE_RUNNING
                && (active_container_slot() == requested_slot
                    || address_space::oci_runtime_is_suspended(requested_slot.unwrap_or(0))) =>
        {
            let slot = requested_slot.unwrap_or(0);
            if address_space::oci_runtime_is_suspended(slot) {
                CONTAINER_SELECTED_SLOT.store(slot as u8, Ordering::Release);
                CONTAINER_STOP_REQUESTED[slot].store(true, Ordering::Release);
                CONTAINER_RESTART_REQUESTED.store(true, Ordering::Release);
            } else {
                CONTAINER_CONTROL_STATES[slot]
                    .store(control_abi::CONTAINER_STATE_STOPPED as u8, Ordering::SeqCst);
            }
            control_abi::CONTAINER_STATE_STOPPED
        }
        control_abi::CONTAINER_RESTART | control_abi::CONTAINER_START_ATTACHED
            if requested_slot.is_some()
                && active_container_slot().is_none_or(|slot| Some(slot) == requested_slot)
                && CONTAINER_CONFIG_FLAGS[requested_slot.unwrap_or(0)].load(Ordering::SeqCst)
                    & control_abi::CONTAINER_CONFIG_COMPLETE
                    == control_abi::CONTAINER_CONFIG_COMPLETE
                && matches!(
                    container_state(requested_slot.unwrap_or(0)),
                    control_abi::CONTAINER_STATE_CONFIGURED
                        | control_abi::CONTAINER_STATE_RUNNING
                        | control_abi::CONTAINER_STATE_STOPPED
                        | control_abi::CONTAINER_STATE_FAILED
                ) =>
        {
            let slot = requested_slot.unwrap_or(0);
            CONTAINER_SELECTED_SLOT.store(slot as u8, Ordering::SeqCst);
            reset_container_console(slot, operation == control_abi::CONTAINER_START_ATTACHED);
            if container_image_is_embedded(slot) || crate::oci_image::is_installed(slot) {
                CONTAINER_RESTART_REQUESTED.store(true, Ordering::SeqCst);
            } else {
                CONTAINER_PULL_REQUESTED.store(true, Ordering::SeqCst);
            }
            control_abi::CONTAINER_STATE_STOPPED
        }
        control_abi::CONTAINER_ATTACH
            if requested_slot.is_some()
                && container_state(requested_slot.unwrap_or(0))
                    == control_abi::CONTAINER_STATE_RUNNING
                && !CONTAINER_CONSOLE_ATTACHED[requested_slot.unwrap_or(0)]
                    .swap(true, Ordering::AcqRel) =>
        {
            let slot = requested_slot.unwrap_or(0);
            CONTAINER_SELECTED_SLOT.store(slot as u8, Ordering::Release);
            CONTAINER_RESTART_REQUESTED.store(true, Ordering::Release);
            redraw_container_console(slot);
            control_abi::CONTAINER_STATE_RUNNING
        }
        control_abi::CONTAINER_INJECT_FAILURE
            if requested_slot.is_some()
                && active_container_slot() == requested_slot
                && container_state(requested_slot.unwrap_or(0))
                    == control_abi::CONTAINER_STATE_RUNNING =>
        {
            CONTAINER_CONTROL_STATES[requested_slot.unwrap_or(0)]
                .store(control_abi::CONTAINER_STATE_FAILED as u8, Ordering::SeqCst);
            control_abi::CONTAINER_STATE_FAILED
        }
        _ => u64::MAX - 2,
    };
    if result != u64::MAX - 2 {
        if !domain::renew_auxiliary_call_epoch(domain_id) {
            return u64::MAX - 2;
        }
        if matches!(
            operation,
            control_abi::CONTAINER_STOP
                | control_abi::CONTAINER_RESTART
                | control_abi::CONTAINER_START_ATTACHED
                | control_abi::CONTAINER_ATTACH
                | control_abi::CONTAINER_INJECT_FAILURE
        ) {
            if !scheduler::suspend_container_lifecycle(
                frame,
                USER_CODE_SELECTOR,
                USER_DATA_SELECTOR,
                result,
                active_fs_base(),
            ) {
                return u64::MAX - 2;
            }
            return u64::MAX;
        }
    }
    result
}

fn read_container_request(address: u64) -> Option<[u8; control_abi::CONTAINER_REQUEST_SIZE]> {
    let mut request = [0_u8; control_abi::CONTAINER_REQUEST_SIZE];
    if !address_space::copy_from_current_user(address, &mut request)
        || read_u32_at(&request, 0) != control_abi::CONTAINER_REQUEST_SIZE as u32
        || read_u16_at(&request, 4) != control_abi::CONTAINER_REQUEST_VERSION
        || usize::from(read_u16_at(&request, 6)) > control_abi::CONTAINER_MAX_NAME_SIZE
        || usize::from(read_u16_at(&request, 32)) > control_abi::CONTAINER_MAX_INIT_SIZE
        || usize::from(read_u16_at(&request, 68)) > control_abi::CONTAINER_MAX_IMAGE_SIZE
    {
        return None;
    }
    Some(request)
}

fn container_request_name(request: &[u8; control_abi::CONTAINER_REQUEST_SIZE]) -> &[u8] {
    let size = usize::from(read_u16_at(request, 6));
    request.get(8..8 + size).unwrap_or_default()
}

fn container_request_init(request: &[u8; control_abi::CONTAINER_REQUEST_SIZE]) -> &[u8] {
    let size = usize::from(read_u16_at(request, 32));
    request.get(36..36 + size).unwrap_or_default()
}

fn container_request_image(request: &[u8; control_abi::CONTAINER_REQUEST_SIZE]) -> &[u8] {
    let size = usize::from(read_u16_at(request, 68));
    request.get(72..72 + size).unwrap_or_default()
}

fn valid_container_name(name: &[u8]) -> bool {
    !name.is_empty()
        && name.len() <= control_abi::CONTAINER_MAX_NAME_SIZE
        && name
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

fn valid_container_init(init: &[u8]) -> bool {
    !init.is_empty()
        && init.len() <= control_abi::CONTAINER_MAX_INIT_SIZE
        && init[0] == b'/'
        && init.iter().all(u8::is_ascii_graphic)
}

fn valid_container_image(image: &[u8]) -> bool {
    let mut canonical = [0_u8; control_abi::CONTAINER_MAX_IMAGE_SIZE];
    canonical_container_image(image, &mut canonical).is_some()
}

fn canonical_container_image(image: &[u8], output: &mut [u8]) -> Option<usize> {
    if image == b"embedded://minimal-system-v0" {
        output.get_mut(..image.len())?.copy_from_slice(image);
        return Some(image.len());
    }
    ginux_oci::ImageReference::parse(image)
        .ok()?
        .write_canonical(output)
        .ok()
}

fn container_slot_by_name(name: &[u8]) -> Option<usize> {
    (!name.is_empty()).then_some(())?;
    (0..control_abi::CONTAINER_LIMIT).find(|slot| {
        let size = usize::from(CONTAINER_NAME_SIZES[*slot].load(Ordering::SeqCst));
        size == name.len()
            && CONTAINER_NAMES[*slot][..size]
                .iter()
                .zip(name)
                .all(|(stored, supplied)| stored.load(Ordering::SeqCst) == *supplied)
    })
}

fn free_container_slot() -> Option<usize> {
    (0..control_abi::CONTAINER_LIMIT)
        .find(|slot| CONTAINER_CONTROL_STATES[*slot].load(Ordering::SeqCst) == 0)
}

fn next_container_slot(cursor: usize) -> Option<usize> {
    (cursor..control_abi::CONTAINER_LIMIT)
        .find(|slot| CONTAINER_CONTROL_STATES[*slot].load(Ordering::SeqCst) != 0)
}

fn next_running_container_slot(cursor: usize) -> Option<usize> {
    (cursor..control_abi::CONTAINER_LIMIT)
        .find(|slot| container_state(*slot) == control_abi::CONTAINER_STATE_RUNNING)
}

fn next_container_cursor(cursor: usize) -> u64 {
    next_container_slot(cursor)
        .map(|slot| slot as u64)
        .unwrap_or(control_abi::CONTAINER_CURSOR_END)
}

fn selected_container_slot() -> Option<usize> {
    let slot = usize::from(CONTAINER_SELECTED_SLOT.load(Ordering::SeqCst));
    (slot < control_abi::CONTAINER_LIMIT).then_some(slot)
}

fn active_container_slot() -> Option<usize> {
    let slot = usize::from(CONTAINER_ACTIVE_SLOT.load(Ordering::SeqCst));
    (slot < control_abi::CONTAINER_LIMIT).then_some(slot)
}

fn selected_or_active_container_slot() -> Option<usize> {
    selected_container_slot().or_else(active_container_slot)
}

fn container_state(slot: usize) -> u64 {
    u64::from(CONTAINER_CONTROL_STATES[slot].load(Ordering::SeqCst))
}

fn container_image_is_embedded(slot: usize) -> bool {
    let size = usize::from(CONTAINER_IMAGE_SIZES[slot].load(Ordering::SeqCst));
    let embedded = b"embedded://minimal-system-v0";
    size == embedded.len()
        && CONTAINER_IMAGES[slot][..size]
            .iter()
            .zip(embedded)
            .all(|(stored, expected)| stored.load(Ordering::SeqCst) == *expected)
}

pub(crate) fn configured_container_uses_installed_oci_image() -> bool {
    let Some(slot) = selected_container_slot() else {
        return false;
    };
    !container_image_is_embedded(slot) && crate::oci_image::is_installed(slot)
}

pub(crate) fn container_uses_installed_oci_image(slot: usize) -> bool {
    slot < control_abi::CONTAINER_LIMIT
        && !container_image_is_embedded(slot)
        && crate::oci_image::is_installed(slot)
}

pub(crate) fn configured_container_slot() -> Option<usize> {
    selected_container_slot()
}

pub(crate) fn current_container_slot() -> Option<usize> {
    active_container_slot()
}

pub(crate) fn container_domain_id(slot: usize) -> Option<u64> {
    let domain = CONTAINER_DOMAIN_IDS.get(slot)?.load(Ordering::Acquire);
    (domain != 0
        && domain::container_domain_for_slot(slot) == Some(domain)
        && domain::container_domain_limits(domain).is_some())
    .then_some(domain)
}

pub(crate) fn configured_container_init(output: &mut [u8]) -> Option<usize> {
    let slot = selected_container_slot()?;
    let size = usize::from(CONTAINER_INIT_SIZES[slot].load(Ordering::SeqCst));
    let output = output.get_mut(..size)?;
    for (destination, source) in output.iter_mut().zip(&CONTAINER_INITS[slot][..size]) {
        *destination = source.load(Ordering::SeqCst);
    }
    Some(size)
}

pub(crate) fn configured_container_image(output: &mut [u8]) -> Option<usize> {
    let slot = selected_container_slot()?;
    let size = usize::from(CONTAINER_IMAGE_SIZES[slot].load(Ordering::SeqCst));
    let output = output.get_mut(..size)?;
    for (destination, source) in output.iter_mut().zip(&CONTAINER_IMAGES[slot][..size]) {
        *destination = source.load(Ordering::SeqCst);
    }
    Some(size)
}

pub(crate) fn take_container_pull_request() -> bool {
    CONTAINER_PULL_REQUESTED.swap(false, Ordering::AcqRel)
}

pub(crate) fn complete_container_pull(success: bool) {
    let Some(slot) = selected_container_slot() else {
        return;
    };
    if success {
        CONTAINER_RESTART_REQUESTED.store(true, Ordering::Release);
    } else {
        CONTAINER_CONTROL_STATES[slot]
            .store(control_abi::CONTAINER_STATE_FAILED as u8, Ordering::Release);
        CONTAINER_SELECTED_SLOT.store(CONTAINER_SLOT_NONE, Ordering::Release);
    }
}

fn store_container_name(slot: usize, name: &[u8]) {
    for (index, byte) in CONTAINER_NAMES[slot].iter().enumerate() {
        byte.store(name.get(index).copied().unwrap_or(0), Ordering::SeqCst);
    }
    CONTAINER_NAME_SIZES[slot].store(name.len() as u8, Ordering::SeqCst);
}

fn store_container_value<const N: usize>(value: &[u8], output: &[AtomicU8; N], size: &AtomicU8) {
    for (index, byte) in output.iter().enumerate() {
        byte.store(value.get(index).copied().unwrap_or(0), Ordering::SeqCst);
    }
    size.store(value.len() as u8, Ordering::SeqCst);
}

fn configurable_container(slot: Option<usize>) -> bool {
    slot.is_some_and(|slot| {
        matches!(
            container_state(slot),
            control_abi::CONTAINER_STATE_CREATED
                | control_abi::CONTAINER_STATE_CONFIGURED
                | control_abi::CONTAINER_STATE_STOPPED
                | control_abi::CONTAINER_STATE_FAILED
        )
    })
}

fn update_container_configuration(slot: usize, flag: u16) -> u64 {
    let flags = CONTAINER_CONFIG_FLAGS[slot].fetch_or(flag, Ordering::SeqCst) | flag;
    let state = if flags & control_abi::CONTAINER_CONFIG_COMPLETE
        == control_abi::CONTAINER_CONFIG_COMPLETE
    {
        control_abi::CONTAINER_STATE_CONFIGURED
    } else {
        control_abi::CONTAINER_STATE_CREATED
    };
    CONTAINER_CONTROL_STATES[slot].store(state as u8, Ordering::SeqCst);
    state
}

fn update_container_autostart(slot: usize, enabled: bool) -> u64 {
    if enabled {
        CONTAINER_CONFIG_FLAGS[slot]
            .fetch_or(control_abi::CONTAINER_CONFIG_AUTOSTART, Ordering::SeqCst);
    } else {
        CONTAINER_CONFIG_FLAGS[slot]
            .fetch_and(!control_abi::CONTAINER_CONFIG_AUTOSTART, Ordering::SeqCst);
    }
    update_container_configuration(slot, 0)
}

fn write_container_request(
    request: &mut [u8; control_abi::CONTAINER_REQUEST_SIZE],
    slot: usize,
    next_cursor: u64,
) {
    request.fill(0);
    request[0..4].copy_from_slice(&(control_abi::CONTAINER_REQUEST_SIZE as u32).to_le_bytes());
    let size = usize::from(CONTAINER_NAME_SIZES[slot].load(Ordering::SeqCst));
    request[6..8].copy_from_slice(&(size as u16).to_le_bytes());
    for (output, stored) in request[8..8 + size]
        .iter_mut()
        .zip(&CONTAINER_NAMES[slot][..size])
    {
        *output = stored.load(Ordering::SeqCst);
    }
    request[4..6].copy_from_slice(&control_abi::CONTAINER_REQUEST_VERSION.to_le_bytes());
    request[24..32].copy_from_slice(&next_cursor.to_le_bytes());
    let init_size = usize::from(CONTAINER_INIT_SIZES[slot].load(Ordering::SeqCst));
    request[32..34].copy_from_slice(&(init_size as u16).to_le_bytes());
    request[34..36].copy_from_slice(
        &CONTAINER_CONFIG_FLAGS[slot]
            .load(Ordering::SeqCst)
            .to_le_bytes(),
    );
    for (output, stored) in request[36..36 + init_size]
        .iter_mut()
        .zip(&CONTAINER_INITS[slot][..init_size])
    {
        *output = stored.load(Ordering::SeqCst);
    }
    let image_size = usize::from(CONTAINER_IMAGE_SIZES[slot].load(Ordering::SeqCst));
    request[68..70].copy_from_slice(&(image_size as u16).to_le_bytes());
    request[70..72].copy_from_slice(
        &CONTAINER_CPU_COUNTS[slot]
            .load(Ordering::Acquire)
            .to_le_bytes(),
    );
    for (output, stored) in request[72..72 + image_size]
        .iter_mut()
        .zip(&CONTAINER_IMAGES[slot][..image_size])
    {
        *output = stored.load(Ordering::SeqCst);
    }
    request[200..208].copy_from_slice(
        &CONTAINER_MEMORY_MIB[slot]
            .load(Ordering::Acquire)
            .to_le_bytes(),
    );
}

fn read_u16_at(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([bytes[offset], bytes[offset + 1]])
}

fn read_u32_at(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

fn read_u64_at(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap())
}

fn write_abi_descriptor(caller_domain: u64, frame: *const UserTrapFrame) -> u64 {
    let mut descriptor = control_abi::descriptor();
    let Some(frame) = (unsafe { frame.as_ref() }) else {
        return u64::MAX - 2;
    };
    if CONTROL_DESCRIPTOR_OVERRIDE_DOMAIN
        .compare_exchange(caller_domain, 0, Ordering::AcqRel, Ordering::Acquire)
        .is_ok()
    {
        descriptor[12] = 1;
    }
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
    let action = scheduler::yield_runtime(
        frame,
        USER_CODE_SELECTOR,
        USER_DATA_SELECTOR,
        active_fs_base(),
    );
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
    if failure_policy == Some(domain::ServiceFailurePolicy::StopOnly) {
        if !crate::packet::retire_egress_producer_if_bound(caller_domain) {
            serial_write("ginux: auxiliary exit egress retirement failed\r\n");
            return u64::MAX;
        }
        if crate::packet::egress_is_empty() {
            if let Some((waiting_domain, sequence)) = scheduler::blocked_egress_waiter() {
                if domain::take_blocked_auxiliary_drain_request(waiting_domain)
                    && !scheduler::wake_egress_waiter_drained(caller_domain, sequence)
                {
                    serial_write("ginux: auxiliary exit drain wake failed\r\n");
                    return u64::MAX;
                }
            }
        }
    }
    let action = scheduler::exit_runtime(frame, USER_CODE_SELECTOR, USER_DATA_SELECTOR, close_wait);
    if matches!(action, RuntimeAction::Invalid) {
        serial_write("ginux: runtime exit scheduling failed\r\n");
        return u64::MAX;
    }
    if !domain::detach_thread(caller_domain) {
        serial_write("ginux: runtime exit thread detach failed\r\n");
        return u64::MAX;
    }
    if service_exit
        && (!ipc::retire_service(caller_domain) || !domain::retire_clean_service(caller_domain))
    {
        return u64::MAX;
    }
    if failure_policy == Some(domain::ServiceFailurePolicy::StopOnly) {
        if !domain::begin_auxiliary_runtime_exit(caller_domain, auxiliary_exit_status.unwrap_or(0))
        {
            serial_write("ginux: auxiliary exit transition failed\r\n");
            return u64::MAX;
        }
        if !address_space::quiesce_auxiliary_device(caller_domain) {
            serial_write("ginux: auxiliary exit quiesce failed status: ");
            serial_write_u64(u64::from(auxiliary_exit_status.unwrap_or(0)));
            serial_write("\r\n");
            return u64::MAX;
        }
        if !address_space::activate_kernel_root_for_auxiliary(caller_domain) {
            serial_write("ginux: auxiliary exit kernel-root restoration failed\r\n");
            return u64::MAX;
        }
        if !address_space::release_auxiliary_address_space(caller_domain) {
            serial_write("ginux: auxiliary exit address-space release failed\r\n");
            return u64::MAX;
        }
        if !domain::retire_stopped_auxiliary_service(caller_domain) {
            serial_write("ginux: auxiliary exit retirement failed\r\n");
            return u64::MAX;
        }
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
    if fault_domain.is_some() && failure_policy.is_none() {
        serial_write("ginux: unexpected control fault vector/rip: ");
        serial_write_u64(fault_metadata.vector);
        serial_write(" ");
        serial_write_u64(fault_metadata.instruction);
        serial_write("\r\n");
    }
    if fault_domain.is_none() && ELF_PROBE_ACTIVE.load(Ordering::SeqCst) {
        serial_write("ginux: Linux user fault vector/error/address/rip:");
        serial_write_u64(fault_metadata.vector);
        serial_write(" ");
        serial_write_u64(fault_metadata.error);
        serial_write(" ");
        serial_write_u64(fault_metadata.address);
        serial_write(" ");
        serial_write_u64(fault_metadata.instruction);
        serial_write("\r\n");
        if deliver_linux_sigsegv(frame, fault_metadata) {
            return 0;
        }
        abort_linux_signal();
        ELF_PROBE_ACTIVE.store(false, Ordering::SeqCst);
        return u64::MAX;
    }
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
    if failure_policy == Some(domain::ServiceFailurePolicy::StopOnly) {
        serial_write("ginux: auxiliary user fault vector/error/address/rip: ");
        for value in [
            fault_metadata.vector,
            fault_metadata.error,
            fault_metadata.address,
            fault_metadata.instruction,
        ] {
            serial_write_u64(value);
            serial_write(" ");
        }
        serial_write("\r\n");
        if !domain::begin_auxiliary_runtime_fault(fault_domain, fault_metadata) {
            serial_write("ginux: auxiliary fault transition failed\r\n");
            return u64::MAX;
        }
        if !crate::packet::retire_egress_producer_if_bound(fault_domain) {
            serial_write("ginux: auxiliary fault egress retirement failed\r\n");
            return u64::MAX;
        }
        if !address_space::quiesce_faulted_auxiliary_device(fault_domain) {
            serial_write("ginux: auxiliary device quiesce failed vector/error/address/rip:");
            serial_write_u64(fault_metadata.vector);
            serial_write(" ");
            serial_write_u64(fault_metadata.error);
            serial_write(" ");
            serial_write_u64(fault_metadata.address);
            serial_write(" ");
            serial_write_u64(fault_metadata.instruction);
            serial_write("\r\n");
            return u64::MAX;
        }
        if !address_space::activate_kernel_root_for_auxiliary(fault_domain) {
            serial_write("ginux: auxiliary kernel-root restoration failed\r\n");
            return u64::MAX;
        }
        if !address_space::release_auxiliary_address_space(fault_domain) {
            serial_write("ginux: auxiliary address-space release failed\r\n");
            return u64::MAX;
        }
        if !domain::retire_stopped_auxiliary_service(fault_domain) {
            serial_write("ginux: auxiliary retirement failed\r\n");
            return u64::MAX;
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
            fs_base,
        } if activate_user_context(address_space_root, fs_base) => result,
        RuntimeAction::Resume { .. } => u64::MAX,
        RuntimeAction::Complete | RuntimeAction::Invalid => u64::MAX,
    }
}

pub(crate) fn active_fs_base() -> u64 {
    unsafe { read_msr(FS_BASE_MSR) }
}

pub(crate) fn verify_user_fs_base_installation() -> bool {
    let saved = active_fs_base();
    let first_installed = install_user_fs_base(0x1110);
    let second_installed = install_user_fs_base(0x2220);
    unsafe { write_msr(FS_BASE_MSR, saved) };
    first_installed && second_installed && active_fs_base() == saved
}

fn install_current_scheduler_fs_base() -> bool {
    scheduler::current_fs_base().is_some_and(install_user_fs_base)
}

fn install_user_fs_base(fs_base: u64) -> bool {
    if fs_base >= 1 << 47 {
        return false;
    }
    unsafe { write_msr(FS_BASE_MSR, fs_base) };
    active_fs_base() == fs_base
}

pub(crate) fn activate_user_context(root: u64, fs_base: u64) -> bool {
    fs_base < 1 << 47 && activate_user_root(root) && install_user_fs_base(fs_base)
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
    let Some(frames) = paging::validated_user_root_frames(root) else {
        return false;
    };
    if !frames.context_matches(instruction, stack)
        || !frames.frames_owned_by(owner)
        || frames
            .dma()
            .is_some_and(|dma| !memory::frame_addresses_owned_by(owner, &[dma]))
    {
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
