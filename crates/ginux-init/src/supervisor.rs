//! Low-level supervision plumbing for the native supervisor.
//!
//! Owner: G-launch. Raw ptrace/wait loop helpers, host syscall shims, and the
//! supervisor console writers shared by every entry point. No workload policy
//! lives here; dispatch decisions belong to `dispatch`.

use core::arch::asm;
use core::sync::atomic::{AtomicU8, Ordering};

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
pub const SYS_GETUID: usize = 102;
pub const SYS_GETGID: usize = 104;
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
pub const SYS_MKDIRAT_HOST: usize = 258;
pub const SYS_SIGNALFD4: usize = 289;
pub const SYS_PROCESS_VM_WRITEV: usize = 311;
pub const SYS_SECCOMP: usize = 317;
pub const SYS_PIDFD_OPEN: usize = 434;
pub const SYS_PIDFD_GETFD: usize = 438;
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
pub const PTRACE_O_TRACESECCOMP: usize = 1 << 7;
pub const PTRACE_O_EXITKILL: usize = 1 << 20;
pub const PTRACE_EVENT_FORK: usize = 1;
pub const PTRACE_EVENT_EXEC: usize = 4;
pub const PTRACE_EVENT_SECCOMP: usize = 7;

pub const EPERM: i64 = 1;
pub const ESRCH: i64 = 3;
pub const EBADF: i64 = 9;
pub const ECHILD: i64 = 10;
pub const EIO: i64 = 5;
pub const EFAULT: i64 = 14;
pub const EACCES: i64 = 13;
pub const EINVAL: i64 = 22;
pub const EMFILE: i64 = 24;
pub const ENOSYS: i64 = 38;
pub const VIRTUAL_PID: u64 = 4242;
pub const SIG_BLOCK: usize = 0;
pub const SIGINT: usize = 2;
pub const SIGTERM: usize = 15;
pub const SIGKILL: usize = 9;
pub const SIGCONT: usize = 18;
pub const SIGSTOP: usize = 19;
pub const SIGTSTP: usize = 20;
pub const SIGCHLD: usize = 17;
pub const PR_SET_NO_NEW_PRIVS: usize = 38;
pub const SECCOMP_RET_ALLOW: u32 = 0x7fff_0000;
pub const SECCOMP_RET_TRACE: u32 = 0x7ff0_0000;
pub const SECCOMP_RET_USER_NOTIF: u32 = 0x7fc0_0000;
pub const SECCOMP_SET_MODE_FILTER: usize = 1;
pub const SECCOMP_FILTER_FLAG_NEW_LISTENER: usize = 1 << 3;
pub const HOST_NOBODY_UID: usize = 65_534;
pub const HOST_NOBODY_GID: usize = 65_534;

pub struct ManagedHostIdentity {
    pub uid: usize,
    pub gid: usize,
    pub drop_root: bool,
}

#[repr(C)]
pub struct SockFilter {
    code: u16,
    jt: u8,
    jf: u8,
    k: u32,
}

#[repr(C)]
pub struct SockFprog {
    len: u16,
    filter: *const SockFilter,
}

/// Install the compiled instance dispatch filter. Paranoid mode routes the
/// direct-safe yield operation through Ginux user notification and therefore
/// requests a notification listener. Selective mode executes that independently
/// cataloged and contract-approved operation directly and does not create an
/// unused listener; all other calls still enter Ginux through ptrace seccomp
/// events.
pub fn install_trace_filter(direct_yield: bool) -> isize {
    const BPF_LD_W_ABS: u16 = 0x20;
    const BPF_JMP_JEQ_K: u16 = 0x15;
    const BPF_RET_K: u16 = 0x06;
    const AUDIT_ARCH_X86_64: u32 = 0xc000_003e;
    const INVALID_ARCH_DATA: u32 = 0xffff;
    let yield_action = if direct_yield {
        SECCOMP_RET_ALLOW
    } else {
        SECCOMP_RET_USER_NOTIF
    };
    let paranoid_filter = [
        SockFilter {
            code: BPF_LD_W_ABS,
            jt: 0,
            jf: 0,
            k: 4,
        },
        SockFilter {
            code: BPF_JMP_JEQ_K,
            jt: 1,
            jf: 0,
            k: AUDIT_ARCH_X86_64,
        },
        SockFilter {
            code: BPF_RET_K,
            jt: 0,
            jf: 0,
            k: SECCOMP_RET_TRACE | INVALID_ARCH_DATA,
        },
        SockFilter {
            code: BPF_LD_W_ABS,
            jt: 0,
            jf: 0,
            k: 0,
        },
        SockFilter {
            code: BPF_JMP_JEQ_K,
            jt: 0,
            jf: 1,
            k: 24,
        },
        SockFilter {
            code: BPF_RET_K,
            jt: 0,
            jf: 0,
            k: yield_action,
        },
        SockFilter {
            code: BPF_RET_K,
            jt: 0,
            jf: 0,
            k: SECCOMP_RET_TRACE,
        },
    ];
    let program = SockFprog {
        len: paranoid_filter.len() as u16,
        filter: paranoid_filter.as_ptr(),
    };
    let flags = if direct_yield {
        0
    } else {
        SECCOMP_FILTER_FLAG_NEW_LISTENER
    };
    unsafe {
        syscall3(
            SYS_SECCOMP,
            SECCOMP_SET_MODE_FILTER,
            flags,
            &program as *const _ as usize,
        )
    }
}

/// Select the real host identity for managed code. Virtual credentials are
/// maintained separately by Ginux and never derive authority from this value.
pub fn managed_host_identity() -> ManagedHostIdentity {
    let uid = unsafe { syscall0(SYS_GETUID) } as usize;
    let gid = unsafe { syscall0(SYS_GETGID) } as usize;
    if uid == 0 {
        ManagedHostIdentity {
            uid: HOST_NOBODY_UID,
            gid: HOST_NOBODY_GID,
            drop_root: true,
        }
    } else {
        ManagedHostIdentity {
            uid,
            gid,
            drop_root: false,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
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

/// Deterministic x86-64 `struct sysinfo`: no host counters are exposed and
/// `mem_unit = 1` keeps libc's page calculations well-defined.
pub fn virtual_sysinfo() -> [u8; 112] {
    let mut info = [0u8; 112];
    info[104..108].copy_from_slice(&1u32.to_ne_bytes());
    info
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

/// Why a chrooted workload cannot start with its standard streams, if it
/// cannot: it keeps descriptors 0 to 2, and a directory there would anchor
/// openat, fchdir and the other *at calls outside its root. A closed stream
/// anchors nothing; one that cannot be examined is refused.
pub fn standard_stream_refusal() -> Option<&'static [u8]> {
    const S_IFMT: u64 = 0o170000;
    const S_IFDIR: u64 = 0o040000;
    const REFUSALS: [[&[u8]; 2]; 3] = [
        [
            b"standard input is a directory",
            b"standard input cannot be checked",
        ],
        [
            b"standard output is a directory",
            b"standard output cannot be checked",
        ],
        [
            b"standard error is a directory",
            b"standard error cannot be checked",
        ],
    ];
    (0..3).find_map(|fd| {
        // x86_64 struct stat: st_mode is the u32 at byte offset 24.
        let mut stat = [0u64; 18];
        match unsafe { syscall2(SYS_FSTAT, fd, stat.as_mut_ptr() as usize) } {
            0 if stat[3] & S_IFMT == S_IFDIR => Some(REFUSALS[fd][0]),
            0 => None,
            error if error == -(EBADF as isize) => None,
            _ => Some(REFUSALS[fd][1]),
        }
    })
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
