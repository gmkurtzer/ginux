//! Low-level supervision plumbing for the native supervisor.
//!
//! Owner: G-launch. Raw ptrace/wait loop helpers, host syscall shims, and the
//! supervisor console writers shared by every entry point. No workload policy
//! lives here; dispatch decisions belong to `dispatch`.

use core::arch::asm;

pub const SYS_WRITE_HOST: usize = 1;
pub const SYS_READ_HOST: usize = 0;
pub const SYS_CLOSE_HOST: usize = 3;
pub const SYS_FSTAT: usize = 5;
pub const SYS_RT_SIGPROCMASK: usize = 14;
pub const SYS_PREAD64: usize = 17;
pub const SYS_PWRITE64: usize = 18;
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
pub const SYS_OPENAT_HOST: usize = 257;
pub const SYS_SIGNALFD4: usize = 289;
pub const AT_FDCWD: usize = (-100isize) as usize;
pub const O_RDONLY: usize = 0;
pub const O_RDWR: usize = 2;
pub const CLOCK_MONOTONIC: usize = 1;
pub const CLOCK_PROCESS_CPUTIME_ID: usize = 2;

pub const PTRACE_TRACEME: usize = 0;
pub const PTRACE_PEEKTEXT: usize = 1;
pub const PTRACE_POKETEXT: usize = 4;
pub const PTRACE_CONT: usize = 7;
pub const PTRACE_PEEKDATA: usize = 2;
pub const PTRACE_KILL: usize = 8;
pub const PTRACE_GETREGS: usize = 12;
pub const PTRACE_SETREGS: usize = 13;
pub const PTRACE_SYSCALL: usize = 24;
pub const PTRACE_SYSEMU: usize = 31;
pub const PTRACE_SETOPTIONS: usize = 0x4200;
pub const PTRACE_GETEVENTMSG: usize = 0x4201;
pub const PTRACE_O_TRACESYSGOOD: usize = 1;
pub const PTRACE_O_TRACEFORK: usize = 1 << 1;
pub const PTRACE_O_TRACEEXEC: usize = 1 << 4;
pub const PTRACE_O_EXITKILL: usize = 1 << 20;
pub const PTRACE_EVENT_FORK: usize = 1;
pub const PTRACE_EVENT_EXEC: usize = 4;

pub const EPERM: i64 = 1;
pub const ESRCH: i64 = 3;
pub const EBADF: i64 = 9;
pub const ECHILD: i64 = 10;
pub const EINVAL: i64 = 22;
pub const ENOSYS: i64 = 38;
pub const VIRTUAL_PID: u64 = 4242;
pub const SIG_BLOCK: usize = 0;
pub const SIGINT: usize = 2;
pub const SIGTERM: usize = 15;
pub const SIGKILL: usize = 9;
pub const SIGCHLD: usize = 17;
pub const PR_SET_NO_NEW_PRIVS: usize = 38;
pub const DOMAIN_UID: usize = 1000;
pub const DOMAIN_GID: usize = 1000;

#[repr(C)]
pub struct UserRegs {
    pub r15: u64,
    pub r14: u64,
    pub r13: u64,
    pub r12: u64,
    pub rbp: u64,
    pub rbx: u64,
    pub r11: u64,
    pub r10: u64,
    pub r9: u64,
    pub r8: u64,
    pub rax: u64,
    pub rcx: u64,
    pub rdx: u64,
    pub rsi: u64,
    pub rdi: u64,
    pub orig_rax: u64,
    pub rip: u64,
    pub cs: u64,
    pub eflags: u64,
    pub rsp: u64,
    pub ss: u64,
    pub fs_base: u64,
    pub gs_base: u64,
    pub ds: u64,
    pub es: u64,
    pub fs: u64,
    pub gs: u64,
}

pub fn wait(pid: usize, status: &mut i32) -> isize {
    unsafe { syscall4(SYS_WAIT4, pid, status as *mut _ as usize, 0, 0) }
}

pub fn stopped(status: i32) -> bool {
    status & 0xff == 0x7f
}

pub fn ptrace(request: usize, pid: usize, addr: usize, data: usize) -> isize {
    unsafe { syscall4(SYS_PTRACE, request, pid, addr, data) }
}

pub fn read_tracee_word(pid: usize, address: usize) -> bool {
    let mut word = 0u64;
    ptrace(PTRACE_PEEKDATA, pid, address, &mut word as *mut _ as usize) >= 0
}

pub fn line(message: &[u8]) {
    write_all(message);
    write_all(b"\n");
}

pub fn write_all(bytes: &[u8]) {
    let mut offset = 0;
    while offset < bytes.len() {
        let result = unsafe {
            syscall3(
                SYS_WRITE_HOST,
                1,
                bytes.as_ptr().add(offset) as usize,
                bytes.len() - offset,
            )
        };
        if result <= 0 {
            return;
        }
        offset += result as usize;
    }
}

pub fn write_u64(mut value: u64) {
    let mut buffer = [0u8; 20];
    let mut index = buffer.len();
    if value == 0 {
        write_all(b"0");
        return;
    }
    while value != 0 {
        index -= 1;
        buffer[index] = b'0' + (value % 10) as u8;
        value /= 10;
    }
    write_all(&buffer[index..]);
}

pub fn write_i64(value: i64) {
    if value < 0 {
        write_all(b"-");
        write_u64(value.unsigned_abs());
    } else {
        write_u64(value as u64);
    }
}

pub fn exit_process(code: usize) -> ! {
    unsafe { syscall1(SYS_EXIT_HOST, code) };
    loop {
        core::hint::spin_loop();
    }
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
