//! aarch64 host ABI for the Ginux daemon process.
//!
//! Host numbers match the generic Linux syscall table used by aarch64
//! (`include/uapi/asm-generic/unistd.h` as of Linux 6.12). The kernel has no
//! `fork` or `pause` entry, so the daemon uses `clone(SIGCHLD)` and `ppoll`.

use core::arch::{asm, global_asm};

global_asm!(
    ".global _start",
    ".type _start, %function",
    "_start:",
    "mov x0, sp",
    "bic sp, x0, #15",
    "bl rust_main",
    "brk #1",
);

pub const SYS_WRITE_HOST: usize = 64;
pub const SYS_READ_HOST: usize = 63;
pub const SYS_CLOSE_HOST: usize = 57;
pub const SYS_FSTAT: usize = 80;
pub const SYS_RT_SIGPROCMASK: usize = 135;
pub const SYS_RT_SIGACTION_HOST: usize = 134;
pub const SYS_PREAD64: usize = 67;
pub const SYS_PWRITE64: usize = 68;
pub const SYS_DUP_HOST: usize = 23;
pub const SYS_FCNTL_HOST: usize = 25;
pub const SYS_FSYNC: usize = 82;
pub const SYS_CLOCK_GETTIME: usize = 113;
pub const SYS_CHDIR: usize = 49;
pub const SYS_SETGID: usize = 144;
pub const SYS_SETUID: usize = 146;
pub const SYS_SETGROUPS: usize = 159;
pub const SYS_PRCTL: usize = 167;
pub const SYS_CHROOT: usize = 51;
pub const SYS_PAUSE: usize = 73;
pub const SYS_FORK: usize = 220;
pub const SYS_EXECVE: usize = 221;
pub const SYS_EXIT_HOST: usize = 93;
pub const SYS_WAIT4: usize = 260;
pub const SYS_KILL: usize = 129;
pub const SYS_PTRACE: usize = 117;
pub const SYS_SETRLIMIT_HOST: usize = 164;
pub const SYS_SCHED_SETAFFINITY_HOST: usize = 122;
pub const SYS_GETDENTS64_HOST: usize = 61;
pub const SYS_OPENAT_HOST: usize = 56;
pub const SYS_MKDIRAT_HOST: usize = 34;
pub const SYS_LAUNCH_MKDIRAT: usize = 34;
pub const SYS_UNLINKAT_HOST: usize = 35;
pub const SYS_SYMLINKAT_HOST: usize = 36;
pub const SYS_SIGNALFD4: usize = 74;
pub const SYS_PROCESS_VM_WRITEV: usize = 271;
pub const SYS_CLOSE_RANGE_HOST: usize = 436;

const SIGCHLD: usize = 17;

pub fn host_fork() -> isize {
    // `clone` argument order on aarch64 is flags, newsp, parent_tid, tls,
    // child_tid. A zero stack keeps the parent's stack, which is fork.
    // `SYS_FORK` is the clone number; aarch64 has no fork entry.
    unsafe { syscall5(SYS_FORK, SIGCHLD, 0, 0, 0, 0) }
}

pub fn host_pause() -> isize {
    // `SYS_PAUSE` is the ppoll number. A null poll waits until a signal.
    unsafe { syscall4(SYS_PAUSE, 0, 0, 0, 0) }
}

#[allow(dead_code)]
#[inline(always)]
pub unsafe fn syscall0(number: usize) -> isize {
    let result: isize;
    asm!(
        "svc #0",
        in("x8") number,
        lateout("x0") result,
        options(nostack)
    );
    result
}

#[inline(always)]
pub unsafe fn syscall1(number: usize, a1: usize) -> isize {
    let result: isize;
    asm!(
        "svc #0",
        in("x8") number,
        inlateout("x0") a1 => result,
        options(nostack)
    );
    result
}

#[inline(always)]
pub unsafe fn syscall2(number: usize, a1: usize, a2: usize) -> isize {
    let result: isize;
    asm!(
        "svc #0",
        in("x8") number,
        inlateout("x0") a1 => result,
        in("x1") a2,
        options(nostack)
    );
    result
}

#[inline(always)]
pub unsafe fn syscall3(number: usize, a1: usize, a2: usize, a3: usize) -> isize {
    let result: isize;
    asm!(
        "svc #0",
        in("x8") number,
        inlateout("x0") a1 => result,
        in("x1") a2,
        in("x2") a3,
        options(nostack)
    );
    result
}

#[inline(always)]
pub unsafe fn syscall4(number: usize, a1: usize, a2: usize, a3: usize, a4: usize) -> isize {
    let result: isize;
    asm!(
        "svc #0",
        in("x8") number,
        inlateout("x0") a1 => result,
        in("x1") a2,
        in("x2") a3,
        in("x3") a4,
        options(nostack)
    );
    result
}

#[inline(always)]
pub unsafe fn syscall5(
    number: usize,
    a1: usize,
    a2: usize,
    a3: usize,
    a4: usize,
    a5: usize,
) -> isize {
    let result: isize;
    asm!(
        "svc #0",
        in("x8") number,
        inlateout("x0") a1 => result,
        in("x1") a2,
        in("x2") a3,
        in("x3") a4,
        in("x4") a5,
        options(nostack)
    );
    result
}

#[inline(always)]
pub unsafe fn syscall6(
    number: usize,
    a1: usize,
    a2: usize,
    a3: usize,
    a4: usize,
    a5: usize,
    a6: usize,
) -> isize {
    let result: isize;
    asm!(
        "svc #0",
        in("x8") number,
        inlateout("x0") a1 => result,
        in("x1") a2,
        in("x2") a3,
        in("x3") a4,
        in("x4") a5,
        in("x5") a6,
        options(nostack)
    );
    result
}
