//! Syscall validation for native process supervision.
//!
//! The `process-native` loop runs supported fork, clone, vfork, and exec
//! calls under ptrace, then records their outcomes in `ProcessTable`.
//! Unsupported clone flags and execveat forms return ENOSYS before execution.
//! Clone3 is unsupported.
//!
//! The loop also handles virtual identities, blocking/nonblocking child
//! waits, termination signals, and descriptor inheritance. Pipes and stdio
//! aliases are mirrored in `PipeSpace`; ordinary file opens are unsupported.
//! Signal handlers, job control, and PI futex operations are unsupported.
//! These hooks do not enable process creation in configured `launch`.

use crate::supervisor::{ENOSYS, SYS_FORK};

/// Raw clone entry: what glibc `fork()` and the `posix_spawn` fallback emit.
pub const SYS_CLONE: u64 = 56;
/// Raw vfork entry: classic process-creation spelling, parent-blocked.
pub const SYS_VFORK: u64 = 58;
/// Raw execve entry: replaces the calling task's image, same pid.
pub const SYS_EXECVE: u64 = 59;
/// Raw execveat entry: dirfd-relative executable replacement.
pub const SYS_EXECVEAT: u64 = 322;

/// `AT_FDCWD` dirfd: resolve the path against the working directory.
pub const AT_FDCWD: u64 = (-100i64) as u64;

/// Share the descriptor table when cloning.
pub const CLONE_FILES: u64 = 0x0000_0400;
pub const CLONE_PARENT_SETTID: u64 = 0x0010_0000;
pub const CLONE_CHILD_SETTID: u64 = 0x0100_0000;
/// `CLONE_THREAD` flag (bit 16): create a thread-group member.
pub const CLONE_THREAD: u64 = 0x0001_0000;
/// `CLONE_VM` flag (bit 8): child shares the parent's address space.
pub const CLONE_VM: u64 = 0x0000_0100;
/// `CLONE_VFORK` flag (bit 14): parent stays blocked until the child
/// releases shared memory by exiting or executing.
pub const CLONE_VFORK: u64 = 0x0000_4000;
/// `CLONE_SIGHAND` flag (bit 11): child shares the parent's signal handlers.
/// Required alongside `CLONE_VM` for every supported thread shape.
pub const CLONE_SIGHAND: u64 = 0x0000_0800;
/// `CLONE_PARENT` flag (bit 15): child takes the caller's parent, which
/// would break managed ownership; thread shapes carrying it are denied.
pub const CLONE_PARENT: u64 = 0x0000_8000;
/// Host-namespace creation flags: a thread that also unshares host state has
/// no managed representation; thread shapes carrying any of them are denied.
pub const CLONE_NEWNS: u64 = 0x0002_0000;
pub const CLONE_NEWCGROUP: u64 = 0x0200_0000;
pub const CLONE_NEWUTS: u64 = 0x0400_0000;
pub const CLONE_NEWIPC: u64 = 0x0800_0000;
pub const CLONE_NEWUSER: u64 = 0x1000_0000;
pub const CLONE_NEWPID: u64 = 0x2000_0000;
pub const CLONE_NEWNET: u64 = 0x4000_0000;
pub const CLONE_NEWTIME: u64 = 0x0000_0080;
/// `CLONE_UNTRACED` flag (bit 23): an untraced child would escape
/// supervision; thread shapes carrying it are denied.
pub const CLONE_UNTRACED: u64 = 0x0080_0000;
/// Low byte of the clone flags: the child exit signal. Threads never carry
/// one; any nonzero value in a thread shape is denied.
pub const CLONE_EXIT_SIGNAL_MASK: u64 = 0x0000_00ff;

/// Raw `read` entry: gated in the supervision loop to stdio and tracked
/// pipe ends; anything else fails closed with `-EBADF`.
pub const SYS_READ: u64 = 0;
/// Raw `close` entry: always executes on the host; the loop drops the
/// mirrored entry when the host reports success.
pub const SYS_CLOSE: u64 = 3;
/// Raw `pipe` entry: creates a connected pair, both ends owned by the caller.
pub const SYS_PIPE: u64 = 22;
/// Raw `dup` entry: lowest-free alias with `FD_CLOEXEC` cleared.
pub const SYS_DUP: u64 = 32;
/// Raw `dup2` entry: alias at an explicit number with `FD_CLOEXEC` cleared.
pub const SYS_DUP2: u64 = 33;
/// Raw `fcntl` entry: only the duplication and flag commands below run.
pub const SYS_FCNTL: u64 = 72;
/// Raw `dup3` entry: alias at an explicit number with `O_CLOEXEC` choice.
pub const SYS_DUP3: u64 = 292;
/// Raw `pipe2` entry: pipe pair with atomic flags.
pub const SYS_PIPE2: u64 = 293;

/// `O_CLOEXEC` creation flag: the end dies with executable replacement.
pub const O_CLOEXEC: u64 = 0o2000000;
/// `O_NONBLOCK` creation flag: the only other flag a pipe pair accepts.
pub const O_NONBLOCK: u64 = 0o4000;
/// `O_DIRECT` creation flag: packetized pipe mode the mirror does not
/// represent; explicitly denied on pipes.
pub const O_DIRECT: u64 = 0o40000;
/// The only `pipe2` flags the mirror represents.
pub const PIPE2_ALLOWED_FLAGS: u64 = O_CLOEXEC | O_NONBLOCK;
/// `fcntl` duplication/flag commands the mirror represents.
pub const F_DUPFD: u64 = 0;
pub const F_GETFD: u64 = 1;
pub const F_SETFD: u64 = 2;
pub const F_DUPFD_CLOEXEC: u64 = 1030;
/// `FD_CLOEXEC` flag bit for `F_GETFD`/`F_SETFD`.
pub const FD_CLOEXEC: u64 = 1;

/// Whether a `pipe2` flag word names a representable pair: the allowed
/// subset only. `O_DIRECT` and every other bit fail explicitly with
/// `-EINVAL` before the host runs.
pub fn pipe2_flags_valid(flags: u64) -> bool {
    flags & !PIPE2_ALLOWED_FLAGS == 0 && flags & O_DIRECT == 0
}

/// Whether a `dup3` flag word names a representable alias: empty or
/// `O_CLOEXEC` only. Anything else fails explicitly with `-EINVAL`.
pub fn dup3_flags_valid(flags: u64) -> bool {
    flags == 0 || flags == O_CLOEXEC
}

/// Whether an `fcntl` command belongs to the mediated duplication/flag
/// subset. Anything else (file status flags, locks, leases, signals,
/// pipe sizing) keeps the existing explicit `-ENOSYS` denial.
pub fn fcntl_supported(command: u64) -> bool {
    command == F_DUPFD || command == F_GETFD || command == F_SETFD || command == F_DUPFD_CLOEXEC
}

/// Raw `arch_prctl` entry: installs or reads the calling thread's TLS base.
/// Thread-local register state only; mediated for every subcommand.
pub const SYS_ARCH_PRCTL: u64 = 158;
/// Raw `gettid` entry: translated to the virtual tid like `getpid`.
pub const SYS_GETTID: u64 = 186;
/// Raw `futex` entry: wait/wake on the thread group's own shared memory.
/// The host kernel performs the blocking and wakeup, so join/mutex observe
/// real results. PI operations are denied because they expose host tids.
pub const SYS_FUTEX: u64 = 202;
/// Raw `set_tid_address` entry: per-thread exit-clear registration. The
/// returned tid is translated to the virtual tid at the exit stop.
pub const SYS_SET_TID_ADDRESS: u64 = 218;
/// Raw `set_robust_list` entry: per-thread robust-mutex registration in
/// tracee memory; mediated.
pub const SYS_SET_ROBUST_LIST: u64 = 273;
/// Raw `get_robust_list` entry: reads back the calling thread's own
/// registration; only pid 0 or its own virtual tid is accepted.
pub const SYS_GET_ROBUST_LIST: u64 = 274;
/// Raw `rseq` entry: per-thread restartable-sequence registration in tracee
/// memory; mediated.
pub const SYS_RSEQ: u64 = 334;

/// Ptrace fork-event stops delivered for the three mediated entries.
pub const PTRACE_EVENT_VFORK: usize = 2;
pub const PTRACE_EVENT_CLONE: usize = 3;

/// Ptrace options requesting the vfork/clone event stops alongside fork.
/// Each option is the event number as a bit, per `linux/ptrace.h`.
pub const PTRACE_O_TRACECLONE: usize = 1 << PTRACE_EVENT_CLONE;
pub const PTRACE_O_TRACEVFORK: usize = 1 << PTRACE_EVENT_VFORK;

/// Lane membership by syscall number: the three process-creation entries
/// plus the two executable-replacement entries belong to the process lane.
/// Other entries are handled directly by the supervision loop.
pub const fn claims(syscall: u64) -> bool {
    syscall == SYS_FORK as u64
        || syscall == SYS_CLONE
        || syscall == SYS_VFORK
        || syscall == SYS_EXECVE
        || syscall == SYS_EXECVEAT
}

/// Whether a `CLONE_THREAD` flag word names the one supported thread shape:
/// a thread-group member sharing memory and signal handlers (`CLONE_VM` +
/// `CLONE_SIGHAND`, the glibc `pthread_create` basis), without `CLONE_VFORK`
/// (parent blocking is meaningless inside a thread group), without an exit
/// signal, without reparenting (`CLONE_PARENT`), without host-namespace
/// creation, and without `CLONE_UNTRACED` (which would evade supervision).
/// The remaining sharing flags (`CLONE_FS`, `CLONE_FILES`), `CLONE_SYSVSEM`,
/// and the `CLONE_SETTLS` / `CLONE_PARENT_SETTID` / `CLONE_CHILD_CLEARTID`
/// tid plumbing are accepted in any combination: they stay inside the thread
/// group's own memory. Anything else with `CLONE_THREAD` fails explicitly
/// with `-ENOSYS` and never executes on the host.
pub fn thread_supported(flags: u64) -> bool {
    const NAMESPACES: u64 = CLONE_NEWNS
        | CLONE_NEWCGROUP
        | CLONE_NEWUTS
        | CLONE_NEWIPC
        | CLONE_NEWUSER
        | CLONE_NEWPID
        | CLONE_NEWNET
        | CLONE_NEWTIME;
    flags & CLONE_THREAD != 0
        && flags & (CLONE_VM | CLONE_SIGHAND) == (CLONE_VM | CLONE_SIGHAND)
        && flags & (CLONE_VFORK | CLONE_PARENT | CLONE_UNTRACED | NAMESPACES) == 0
        && flags & CLONE_EXIT_SIGNAL_MASK == 0
}

/// Reject unsupported clone flags and concurrent address-space sharing
/// outside a thread group. `None` lets the supervision loop handle the call;
/// it is not permission for other callers to execute a host syscall.
pub fn claim(syscall: u64, rdi: u64, _rsi: u64, _rdx: u64) -> Option<i64> {
    if syscall == SYS_CLONE {
        // Accept only the flags represented by managed process ownership.
        // CLONE_UNTRACED and CLONE_PARENT must be rejected for processes too.
        const ALLOWED: u64 = CLONE_VM
            | 0x200
            | CLONE_FILES
            | CLONE_SIGHAND
            | CLONE_VFORK
            | CLONE_THREAD
            | 0x40000
            | 0x80000
            | CLONE_PARENT_SETTID
            | 0x200000
            | CLONE_CHILD_SETTID
            | 0xff;
        if rdi & !ALLOWED != 0 || rdi & CLONE_UNTRACED != 0 {
            return Some(-ENOSYS);
        }
        if (rdi & CLONE_THREAD) != 0 {
            return if thread_supported(rdi) {
                None
            } else {
                Some(-ENOSYS)
            };
        }
        if rdi & CLONE_EXIT_SIGNAL_MASK != 17 {
            return Some(-ENOSYS);
        }
        if (rdi & (CLONE_VM | CLONE_VFORK)) == CLONE_VM {
            return Some(-ENOSYS);
        }
    }
    None
}

/// Apply the legacy clone ownership policy to clone3's split flags and exit
/// signal while preserving clone3-only features already mediated elsewhere.
pub fn clone3_supported(flags: u64, exit_signal: u64) -> bool {
    const CLONE_PIDFD: u64 = 0x0000_1000;
    const CLONE_CLEAR_SIGHAND: u64 = 0x1_0000_0000;
    const CLONE_INTO_CGROUP: u64 = 0x2_0000_0000;
    if flags & CLONE_CLEAR_SIGHAND != 0 && flags & CLONE_SIGHAND != 0 {
        return false;
    }
    let legacy = flags & !(CLONE_PIDFD | CLONE_CLEAR_SIGHAND | CLONE_INTO_CGROUP);
    exit_signal <= u8::MAX as u64 && claim(SYS_CLONE, legacy | exit_signal, 0, 0).is_none()
}

/// Whether an executable-replacement entry executes under supervision.
/// Plain `execve` and `execveat(AT_FDCWD, path, argv, envp, 0)` are mediated:
/// the host kernel performs shebang and loader transitions with the tracee
/// still attached. Any other `execveat` shape (non-CWD dirfd, or nonzero
/// flags such as `AT_EMPTY_PATH`) is unsupported and never executes on the host.
pub fn exec_mediated(syscall: u64, dirfd: u64, flags: u64) -> bool {
    if syscall == SYS_EXECVE {
        return true;
    }
    if syscall == SYS_EXECVEAT {
        return dirfd as i32 == AT_FDCWD as i32 && flags == 0;
    }
    false
}
