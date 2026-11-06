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

pub const AUDIT_ARCH: u32 = 0xc000_00b7;
pub const ABI_REJECT_PROCESS: &[u8] = b"process requires the aarch64 syscall ABI";
pub const ABI_REJECT_NATIVE: &[u8] = b"native requires the aarch64 syscall ABI";
pub const ABI_REJECT_LAUNCH: &[u8] = b"launch requires the aarch64 syscall ABI";
const SYSCALL_INSN_LEN: u64 = 4;
const ENTRY_PHASE: u64 = 1;
const EXIT_PHASE: u64 = 2;
const RESTART_PHASE: u64 = 3;
const SYNTHETIC_ENTRY_RAX: u64 = (-38i64) as u64;
const UNKNOWN_SYSCALL: u64 = u64::MAX - 1;
const NT_PRSTATUS: usize = 1;
const NT_ARM_SYSTEM_CALL: usize = 0x404;
const PTRACE_GETREGSET: usize = 0x4204;
const PTRACE_SETREGSET: usize = 0x4205;
const PTRACE_GET_SYSCALL_INFO: usize = 0x420e;
const SYSCALL_INFO_ENTRY: u8 = 1;
const SYSCALL_INFO_EXIT: u8 = 2;

#[repr(C)]
struct UserPtRegs {
    regs: [u64; 31],
    sp: u64,
    pc: u64,
    pstate: u64,
}

#[repr(C)]
struct IoVec {
    base: usize,
    len: usize,
}

#[derive(Clone, Copy)]
struct SavedArgs {
    pid: u32,
    live: bool,
    nr: u64,
    args: [u64; 6],
}

const SAVED_LEN: usize = 128;
static mut SAVED_ARGS: [SavedArgs; SAVED_LEN] = [SavedArgs {
    pid: 0,
    live: false,
    nr: 0,
    args: [0; 6],
}; 128];

fn ptrace(request: usize, pid: usize, addr: usize, data: usize) -> isize {
    crate::supervisor::ptrace(request, pid, addr, data)
}

fn syscall_phase(pid: usize) -> u8 {
    let mut info = [0u64; 8];
    let read = ptrace(
        PTRACE_GET_SYSCALL_INFO,
        pid,
        core::mem::size_of_val(&info),
        info.as_mut_ptr() as usize,
    );
    if read < 0 {
        0
    } else {
        info[0] as u8
    }
}

fn saved_slot(index: usize) -> *mut SavedArgs {
    unsafe { (core::ptr::addr_of_mut!(SAVED_ARGS) as *mut SavedArgs).add(index) }
}

fn remember_entry(pid: usize, nr: u64, args: [u64; 6]) {
    let pid32 = pid as u32;
    unsafe {
        let mut free = None;
        let mut index = 0;
        while index < SAVED_LEN {
            let current = core::ptr::read(saved_slot(index));
            if current.live && current.pid == pid32 {
                core::ptr::write(
                    saved_slot(index),
                    SavedArgs {
                        pid: pid32,
                        live: true,
                        nr,
                        args,
                    },
                );
                return;
            }
            if free.is_none() && !current.live {
                free = Some(index);
            }
            index += 1;
        }
        if let Some(index) = free {
            core::ptr::write(
                saved_slot(index),
                SavedArgs {
                    pid: pid32,
                    live: true,
                    nr,
                    args,
                },
            );
        }
    }
}

fn recall_entry(pid: usize) -> Option<(u64, [u64; 6])> {
    let pid32 = pid as u32;
    unsafe {
        let mut index = 0;
        while index < SAVED_LEN {
            let current = core::ptr::read(saved_slot(index));
            if current.live && current.pid == pid32 {
                return Some((current.nr, current.args));
            }
            index += 1;
        }
    }
    None
}

fn read_raw(pid: usize) -> Option<UserPtRegs> {
    let mut raw = UserPtRegs {
        regs: [0; 31],
        sp: 0,
        pc: 0,
        pstate: 0,
    };
    let mut iovec = IoVec {
        base: &mut raw as *mut _ as usize,
        len: core::mem::size_of::<UserPtRegs>(),
    };
    if ptrace(
        PTRACE_GETREGSET,
        pid,
        NT_PRSTATUS,
        &mut iovec as *mut _ as usize,
    ) < 0
    {
        return None;
    }
    Some(raw)
}

fn read_syscallno(pid: usize, raw: &UserPtRegs) -> i64 {
    let mut value = 0i32;
    let mut iovec = IoVec {
        base: &mut value as *mut _ as usize,
        len: core::mem::size_of::<i32>(),
    };
    if ptrace(
        PTRACE_GETREGSET,
        pid,
        NT_ARM_SYSTEM_CALL,
        &mut iovec as *mut _ as usize,
    ) < 0
    {
        raw.regs[8] as i64
    } else {
        i64::from(value)
    }
}

fn internal_number(native: i64) -> u64 {
    if native < 0 {
        u64::MAX
    } else {
        ginux_core::to_x86(native as u64).unwrap_or(UNKNOWN_SYSCALL)
    }
}

fn native_number(internal: u64) -> i32 {
    if internal == u64::MAX || internal == UNKNOWN_SYSCALL {
        -1
    } else {
        match ginux_core::to_aarch64(internal) {
            Some(number) if number <= i32::MAX as u64 => number as i32,
            _ => -1,
        }
    }
}

/// Fill the logical x86_64 register file from `user_pt_regs`.
///
/// `cs` carries the stop phase for [`write_user_regs`]. `eflags` carries
/// `pstate`, which `SETREGSET` must write back unchanged. Entry stops record
/// the six arguments so an exit stop can still present them in `rdi`..`r9`
/// after `x0` has become the return value. `rax` at entry is the x86 sentinel
/// `-ENOSYS`, so a later result write is visible.
pub fn read_user_regs(pid: usize, _addr: usize, data: usize) -> isize {
    let Some(raw) = read_raw(pid) else {
        return -1;
    };
    let native = read_syscallno(pid, &raw);
    let phase = syscall_phase(pid);
    let args = [
        raw.regs[0],
        raw.regs[1],
        raw.regs[2],
        raw.regs[3],
        raw.regs[4],
        raw.regs[5],
    ];
    let (nr, arg, rax, phase_tag) = if phase == SYSCALL_INFO_EXIT {
        if let Some((nr, saved)) = recall_entry(pid) {
            (nr, saved, raw.regs[0], EXIT_PHASE)
        } else {
            (internal_number(native), args, raw.regs[0], EXIT_PHASE)
        }
    } else {
        let nr = internal_number(native);
        if phase == SYSCALL_INFO_ENTRY {
            remember_entry(pid, nr, args);
        }
        (nr, args, SYNTHETIC_ENTRY_RAX, ENTRY_PHASE)
    };
    let logical = crate::supervisor::UserRegs {
        r15: 0,
        r14: 0,
        r13: 0,
        r12: 0,
        rbp: 0,
        rbx: 0,
        r11: 0,
        r10: arg[3],
        r9: arg[5],
        r8: arg[4],
        rax,
        rcx: 0,
        rdx: arg[2],
        rsi: arg[1],
        rdi: arg[0],
        orig_rax: nr,
        rip: raw.pc,
        cs: phase_tag,
        eflags: raw.pstate,
        rsp: raw.sp,
        ss: 0,
        fs_base: 0,
        gs_base: 0,
        ds: 0,
        es: 0,
        fs: 0,
        gs: 0,
    };
    unsafe {
        core::ptr::write_unaligned(data as *mut crate::supervisor::UserRegs, logical);
    }
    0
}

pub fn write_user_regs(pid: usize, _addr: usize, data: usize) -> isize {
    let logical = unsafe { core::ptr::read_unaligned(data as *const crate::supervisor::UserRegs) };
    let Some(mut raw) = read_raw(pid) else {
        return -1;
    };
    let restart = logical.cs == RESTART_PHASE;
    let exit = logical.cs == EXIT_PHASE;
    let result_override = logical.rax != SYNTHETIC_ENTRY_RAX;
    raw.regs[0] = if restart {
        logical.rdi
    } else if exit || result_override {
        logical.rax
    } else {
        logical.rdi
    };
    raw.regs[1] = logical.rsi;
    raw.regs[2] = logical.rdx;
    raw.regs[3] = logical.r10;
    raw.regs[4] = logical.r8;
    raw.regs[5] = logical.r9;
    let syscallno = native_number(logical.orig_rax);
    if syscallno >= 0 {
        raw.regs[8] = syscallno as u64;
    }
    raw.pc = logical.rip;
    raw.sp = logical.rsp;
    if logical.eflags != 0 {
        raw.pstate = logical.eflags;
    }
    let mut iovec = IoVec {
        base: &raw as *const _ as usize,
        len: core::mem::size_of::<UserPtRegs>(),
    };
    if ptrace(
        PTRACE_SETREGSET,
        pid,
        NT_PRSTATUS,
        &mut iovec as *mut _ as usize,
    ) < 0
    {
        return -1;
    }
    let mut syscall_word = syscallno;
    let mut syscall_iovec = IoVec {
        base: &mut syscall_word as *mut _ as usize,
        len: core::mem::size_of::<i32>(),
    };
    ptrace(
        PTRACE_SETREGSET,
        pid,
        NT_ARM_SYSTEM_CALL,
        &mut syscall_iovec as *mut _ as usize,
    )
}

/// Replace one aligned instruction with `brk #0`.
pub fn breakpoint_trap_word(_word: usize) -> usize {
    0xd420_0000
}

/// Rewind `svc` and ask [`write_user_regs`] to keep argument 0 in `x0`.
pub fn prepare_syscall_restart(regs: &mut crate::supervisor::UserRegs, _syscall: u64) {
    regs.rip = regs.rip.wrapping_sub(SYSCALL_INSN_LEN);
    regs.cs = RESTART_PHASE;
}
