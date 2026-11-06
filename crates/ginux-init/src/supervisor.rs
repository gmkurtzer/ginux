//! Low-level supervision plumbing for the native supervisor.
//!
//! Owner: G-launch. Raw ptrace/wait loop helpers, host syscall shims, and the
//! supervisor console writers shared by every entry point. No workload policy
//! lives here; dispatch decisions belong to `dispatch`.

use core::sync::atomic::{AtomicU8, Ordering};

pub use crate::arch::{
    breakpoint_trap_word, host_fork, host_pause, prepare_syscall_restart, read_user_regs, syscall1,
    syscall2, syscall3, syscall4, syscall5, syscall6, write_user_regs, ABI_REJECT_LAUNCH,
    ABI_REJECT_NATIVE, ABI_REJECT_PROCESS, AUDIT_ARCH, SYS_CHDIR, SYS_CHROOT, SYS_CLOCK_GETTIME,
    SYS_CLOSE_HOST, SYS_CLOSE_RANGE_HOST, SYS_DUP_HOST, SYS_EXECVE, SYS_EXIT_HOST, SYS_FCNTL_HOST,
    SYS_FSTAT, SYS_FSYNC, SYS_GETDENTS64_HOST, SYS_KILL, SYS_LAUNCH_MKDIRAT, SYS_MKDIRAT_HOST,
    SYS_OPENAT_HOST, SYS_PRCTL, SYS_PREAD64, SYS_PROCESS_VM_WRITEV, SYS_PTRACE, SYS_PWRITE64,
    SYS_READ_HOST, SYS_RT_SIGACTION_HOST, SYS_RT_SIGPROCMASK, SYS_SCHED_SETAFFINITY_HOST,
    SYS_SETGID, SYS_SETGROUPS, SYS_SETRLIMIT_HOST, SYS_SETUID, SYS_SIGNALFD4, SYS_SYMLINKAT_HOST,
    SYS_UNLINKAT_HOST, SYS_WAIT4, SYS_WRITE_HOST,
};
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
#[allow(dead_code)]
pub const PTRACE_GETREGS: usize = 12;
#[allow(dead_code)]
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
pub const EIO: i64 = 5;
pub const EFAULT: i64 = 14;
pub const EACCES: i64 = 13;
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

/// Whether `process_vm_writev` works for this supervisor: unknown until the
/// first answer, then cached. Kernels built without
/// `CONFIG_CROSS_MEMORY_ATTACH` return `ENOSYS`; a custom seccomp profile can
/// return `EPERM`.
static VM_WRITE: AtomicU8 = AtomicU8::new(VM_WRITE_UNKNOWN);
const VM_WRITE_UNKNOWN: u8 = 0;
const VM_WRITE_AVAILABLE: u8 = 1;
const VM_WRITE_DENIED: u8 = 2;

/// Copy `bytes` into tracee memory at `address` the way the kernel's own copy
/// to user memory would: a read-only or unmapped range fails with `-EFAULT`,
/// and the bytes before the first unwritable one are written. This uses
/// `process_vm_writev`, which honors the tracee's page protections.
///
/// If that call is unavailable (`ENOSYS`, `EPERM` or `EACCES` on its first
/// use), every later copy falls back to [`poke_tracee_bytes`] with reduced
/// fidelity: an unmapped range still fails with `-EFAULT`, but read-only
/// memory is written, because ptrace writes ignore page protections.
/// Returns the negative errno on failure.
pub fn write_tracee_bytes(pid: usize, address: usize, bytes: &[u8]) -> Result<(), i64> {
    let state = VM_WRITE.load(Ordering::Relaxed);
    if state != VM_WRITE_DENIED {
        let local = [bytes.as_ptr() as usize, bytes.len()];
        let remote = [address, bytes.len()];
        let written = unsafe {
            syscall6(
                SYS_PROCESS_VM_WRITEV,
                pid,
                local.as_ptr() as usize,
                1,
                remote.as_ptr() as usize,
                1,
                0,
            )
        } as i64;
        let unavailable = matches!(-written, ENOSYS | EPERM | EACCES);
        if !unavailable || state == VM_WRITE_AVAILABLE {
            if written >= 0 {
                VM_WRITE.store(VM_WRITE_AVAILABLE, Ordering::Relaxed);
            }
            return if written == bytes.len() as i64 {
                Ok(())
            } else if written < 0 {
                Err(written)
            } else {
                Err(-EFAULT)
            };
        }
        VM_WRITE.store(VM_WRITE_DENIED, Ordering::Relaxed);
        write_all(b"GINUX_SUPERVISOR_AUDIT: tracee-write=ptrace-fallback process_vm_writev=");
        write_i64(written);
        write_all(b"\n");
    }
    poke_tracee_bytes(pid, address, bytes).map_err(fault_errno)
}

/// Map a failed ptrace access to the errno a user copy would report: an
/// unmapped address (`EIO` or `EFAULT`) is `-EFAULT`; anything else, such as
/// `ESRCH` for a tracee that is gone, stays as it is.
pub fn fault_errno(error: i64) -> i64 {
    if error == -EIO || error == -EFAULT {
        -EFAULT
    } else {
        error
    }
}

/// Merge `bytes` into the tracee's aligned words with `PTRACE_PEEKDATA` and
/// `PTRACE_POKEDATA`, preserving every neighboring byte. Ptrace writes ignore
/// page protections. Returns the raw negative errno of the first failed
/// access; the words before it are written.
pub fn poke_tracee_bytes(pid: usize, address: usize, bytes: &[u8]) -> Result<(), i64> {
    let mut index = 0;
    while index < bytes.len() {
        let current = address.checked_add(index).ok_or(-EFAULT)?;
        let aligned = current & !0x7;
        let mut word = 0usize;
        let result = ptrace(PTRACE_PEEKDATA, pid, aligned, &mut word as *mut _ as usize);
        if result < 0 {
            return Err(result as i64);
        }
        let mut lanes = word.to_le_bytes();
        let mut lane = current - aligned;
        while lane < 8 && index < bytes.len() {
            lanes[lane] = bytes[index];
            lane += 1;
            index += 1;
        }
        let result = ptrace(PTRACE_POKETEXT, pid, aligned, usize::from_le_bytes(lanes));
        if result < 0 {
            return Err(result as i64);
        }
    }
    Ok(())
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
