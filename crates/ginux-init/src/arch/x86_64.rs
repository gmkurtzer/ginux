//! x86_64 host ABI for the Ginux daemon process.
//!
//! Host numbers match `asm/unistd_64.h`. `SYS_LAUNCH_MKDIRAT` stays 259
//! (`mknodat`) because that is the number the launch path already issues
//! with a three-argument call.

use core::arch::{asm, global_asm};

global_asm!(
    ".global _start",
    ".type _start,@function",
    "_start:",
    "xor %rbp, %rbp",
    "mov %rsp, %rdi",
    "andq $-16, %rsp",
    "call rust_main",
    "ud2",
    options(att_syntax)
);

pub const SYS_WRITE_HOST: usize = 1;
pub const SYS_READ_HOST: usize = 0;
pub const SYS_CLOSE_HOST: usize = 3;
pub const SYS_FSTAT: usize = 5;
pub const SYS_RT_SIGPROCMASK: usize = 14;
pub const SYS_RT_SIGACTION_HOST: usize = 13;
pub const SYS_PREAD64: usize = 17;
pub const SYS_PWRITE64: usize = 18;
pub const SYS_DUP_HOST: usize = 32;
pub const SYS_FCNTL_HOST: usize = 72;
pub const SYS_FSYNC: usize = 74;
pub const SYS_CLOCK_GETTIME: usize = 228;
pub const SYS_CHDIR: usize = 80;
pub const SYS_SETGID: usize = 106;
pub const SYS_SETUID: usize = 105;
pub const SYS_SETGROUPS: usize = 116;
pub const SYS_PRCTL: usize = 157;
pub const SYS_CHROOT: usize = 161;
pub const SYS_PAUSE: usize = 34;
pub const SYS_FORK: usize = 57;
pub const SYS_EXECVE: usize = 59;
pub const SYS_EXIT_HOST: usize = 60;
pub const SYS_WAIT4: usize = 61;
pub const SYS_KILL: usize = 62;
pub const SYS_PTRACE: usize = 101;
pub const SYS_SETRLIMIT_HOST: usize = 160;
pub const SYS_SCHED_SETAFFINITY_HOST: usize = 203;
pub const SYS_GETDENTS64_HOST: usize = 217;
pub const SYS_OPENAT_HOST: usize = 257;
pub const SYS_MKDIRAT_HOST: usize = 258;
pub const SYS_LAUNCH_MKDIRAT: usize = 259;
pub const SYS_UNLINKAT_HOST: usize = 263;
pub const SYS_SYMLINKAT_HOST: usize = 266;
pub const SYS_SIGNALFD4: usize = 289;
pub const SYS_PROCESS_VM_WRITEV: usize = 311;
pub const SYS_CLOSE_RANGE_HOST: usize = 436;

pub fn host_fork() -> isize {
    unsafe { syscall0(SYS_FORK) }
}

pub fn host_pause() -> isize {
    unsafe { syscall0(SYS_PAUSE) }
}

#[inline(always)]
pub unsafe fn syscall0(number: usize) -> isize {
    let result: isize;
    asm!(
        "syscall",
        inlateout("rax") number as isize => result,
        lateout("rcx") _,
        lateout("r11") _,
        options(nostack)
    );
    result
}

#[inline(always)]
pub unsafe fn syscall1(number: usize, a1: usize) -> isize {
    let result: isize;
    asm!(
        "syscall",
        inlateout("rax") number as isize => result,
        in("rdi") a1,
        lateout("rcx") _,
        lateout("r11") _,
        options(nostack)
    );
    result
}

#[inline(always)]
pub unsafe fn syscall2(number: usize, a1: usize, a2: usize) -> isize {
    let result: isize;
    asm!(
        "syscall",
        inlateout("rax") number as isize => result,
        in("rdi") a1,
        in("rsi") a2,
        lateout("rcx") _,
        lateout("r11") _,
        options(nostack)
    );
    result
}

#[inline(always)]
pub unsafe fn syscall3(number: usize, a1: usize, a2: usize, a3: usize) -> isize {
    let result: isize;
    asm!(
        "syscall",
        inlateout("rax") number as isize => result,
        in("rdi") a1,
        in("rsi") a2,
        in("rdx") a3,
        lateout("rcx") _,
        lateout("r11") _,
        options(nostack)
    );
    result
}

#[inline(always)]
pub unsafe fn syscall4(number: usize, a1: usize, a2: usize, a3: usize, a4: usize) -> isize {
    let result: isize;
    asm!(
        "syscall",
        inlateout("rax") number as isize => result,
        in("rdi") a1,
        in("rsi") a2,
        in("rdx") a3,
        in("r10") a4,
        lateout("rcx") _,
        lateout("r11") _,
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
        "syscall",
        inlateout("rax") number as isize => result,
        in("rdi") a1,
        in("rsi") a2,
        in("rdx") a3,
        in("r10") a4,
        in("r8") a5,
        lateout("rcx") _,
        lateout("r11") _,
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
        "syscall",
        inlateout("rax") number as isize => result,
        in("rdi") a1,
        in("rsi") a2,
        in("rdx") a3,
        in("r10") a4,
        in("r8") a5,
        in("r9") a6,
        lateout("rcx") _,
        lateout("r11") _,
        options(nostack)
    );
    result
}
