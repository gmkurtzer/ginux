#![no_std]

pub const SYS_WRITE: u64 = 1;
pub const SYS_POLL: u64 = 7;
pub const MAX_BOOTSTRAP_POLL_FDS: u64 = 16;
pub const MAX_BOOTSTRAP_POLL_TIMEOUT_MS: u64 = 300_000;
pub const SYS_GETPID: u64 = 39;
pub const SYS_FORK: u64 = 57;
pub const SYS_EXIT: u64 = 60;
pub const SYS_WAIT4: u64 = 61;
pub const SYS_UNAME: u64 = 63;
pub const SYS_GETUID: u64 = 102;
pub const SYS_GETGID: u64 = 104;
pub const SYS_GETEUID: u64 = 107;
pub const SYS_GETEGID: u64 = 108;
pub const SYS_SETUID: u64 = 105;
pub const SYS_SETGID: u64 = 106;
pub const SYS_SETREUID: u64 = 113;
pub const SYS_SETREGID: u64 = 114;
pub const SYS_SETGROUPS: u64 = 116;
pub const SYS_SETRESUID: u64 = 117;
pub const SYS_SETRESGID: u64 = 119;
pub const SYS_GETPPID: u64 = 110;
pub const SYS_GETPGRP: u64 = 111;
pub const SYS_FUTEX: u64 = 202;
pub const SYS_SET_TID_ADDRESS: u64 = 218;
pub const SYS_GETPRIORITY: u64 = 140;
pub const SYS_SETPRIORITY: u64 = 141;
pub const SYS_REBOOT: u64 = 169;
pub const SYS_GETTID: u64 = 186;
pub const SYS_EXIT_GROUP: u64 = 231;
pub const SYS_UNLINKAT: u64 = 263;
pub const SYS_RENAMEAT: u64 = 264;
pub const SYS_LINKAT: u64 = 265;
pub const SYS_SYMLINKAT: u64 = 266;
pub const SYS_READLINKAT: u64 = 267;
pub const SYS_MEMFD_CREATE: u64 = 319;
pub const SYS_CLOSE_RANGE: u64 = 436;
pub const SYS_PIDFD_OPEN: u64 = 434;
pub const SYS_PIDFD_SEND_SIGNAL: u64 = 424;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Action {
    Emulate,
    Mediate,
    Deny(i32),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Decision {
    pub syscall: u64,
    pub action: Action,
}

pub const fn decide(syscall: u64) -> Decision {
    let action = match syscall {
        SYS_GETPID | SYS_EXIT => Action::Emulate,
        SYS_WRITE => Action::Mediate,
        SYS_UNAME => Action::Deny(1),
        _ => Action::Deny(38),
    };
    Decision { syscall, action }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BalancedSurface {
    ImageRead,
    InstanceWrite,
    Memory,
    Task,
    Console,
    Random,
    Network,
    VirtualIdentity,
    Lifecycle,
    Denied,
}

pub const fn is_shared_network_syscall(syscall: u64) -> bool {
    matches!(syscall, 41..=55 | 288 | 299 | 307)
}

pub const fn decide_systemd(syscall: u64, shared_network: bool) -> Decision {
    if shared_network && is_shared_network_syscall(syscall) {
        return Decision {
            syscall,
            action: Action::Mediate,
        };
    }
    let action = match syscall {
        63 | 92 | 93 | 94 | 109 | 112 | 153 | 165 | 166 | 260 | SYS_GETPRIORITY
        | SYS_SETPRIORITY | SYS_GETTID | SYS_SETUID | SYS_SETGID | SYS_SETREUID | SYS_SETREGID
        | SYS_SETGROUPS | SYS_SETRESUID | SYS_SETRESGID | SYS_REBOOT => Action::Emulate,
        7
        | 15
        | 16
        | 18
        | 24
        | 25
        | 32
        | 33
        | 56
        | 57
        | 58
        | 59
        | 61
        | 62
        | 73
        | 74
        | 75
        | 76
        | 77
        | 79
        | 80
        | 81
        | 82
        | 83
        | 84
        | 87
        | 88
        | 90
        | 91
        | 95
        | 125
        | 126
        | 137
        | 138
        | 157
        | 190
        | 193
        | 128
        | 133
        | 135
        | 196
        | 199
        | 213
        | 228
        | 230
        | 232
        | 233
        | 247
        | 253
        | 254
        | 255
        | 258
        | 259
        | SYS_UNLINKAT
        | SYS_RENAMEAT
        | SYS_LINKAT
        | SYS_SYMLINKAT
        | SYS_READLINKAT
        | 270
        | 271
        | 280
        | 281
        | 283
        | 285
        | 286
        | 287
        | 289
        | 290
        | 291
        | 292
        | 293
        | 294
        | 306
        | 317
        | SYS_MEMFD_CREATE
        | SYS_PIDFD_SEND_SIGNAL
        | SYS_PIDFD_OPEN
        | 435
        | SYS_CLOSE_RANGE
        | 441
        | 452 => Action::Mediate,
        _ => decide_balanced(syscall).action,
    };
    Decision { syscall, action }
}

pub const fn systemd_surface(syscall: u64, shared_network: bool) -> BalancedSurface {
    if shared_network && is_shared_network_syscall(syscall) {
        BalancedSurface::Network
    } else if syscall == SYS_REBOOT {
        BalancedSurface::Lifecycle
    } else if matches!(syscall, SYS_GETPRIORITY | SYS_SETPRIORITY) {
        BalancedSurface::VirtualIdentity
    } else if matches!(
        syscall,
        18 | 73
            | 74
            | 75
            | 76
            | 77
            | 82
            | 84
            | 87
            | 88
            | 90
            | 91
            | 92
            | 93
            | 94
            | 133
            | 190
            | 196
            | 199
            | 259
            | 260
            | 280
            | 285
            | 306
            | SYS_UNLINKAT
            | SYS_RENAMEAT
            | SYS_LINKAT
            | SYS_SYMLINKAT
    ) {
        BalancedSurface::InstanceWrite
    } else if syscall == 153 {
        BalancedSurface::Console
    } else if matches!(syscall, 25 | SYS_MEMFD_CREATE) {
        BalancedSurface::Memory
    } else if matches!(
        syscall,
        SYS_SETUID
            | SYS_SETGID
            | SYS_SETREUID
            | SYS_SETREGID
            | SYS_SETGROUPS
            | SYS_SETRESUID
            | SYS_SETRESGID
    ) {
        BalancedSurface::VirtualIdentity
    } else if matches!(syscall, 193 | SYS_READLINKAT) {
        BalancedSurface::ImageRead
    } else if matches!(
        syscall,
        7 | 15
            | 24
            | 32
            | 33
            | 56
            | 57
            | 58
            | 61
            | 62
            | 79
            | 81
            | 109
            | 112
            | 128
            | 135
            | 213
            | 228
            | 230
            | 232
            | 233
            | 247
            | 253
            | 254
            | 255
            | 270
            | 271
            | 281
            | 283
            | 286
            | 287
            | 289
            | 290
            | 291
            | 292
            | 293
            | 294
            | 317
            | SYS_PIDFD_SEND_SIGNAL
            | SYS_PIDFD_OPEN
            | 435
            | SYS_CLOSE_RANGE
            | 441
            | 452
    ) {
        if syscall == 452 {
            BalancedSurface::InstanceWrite
        } else {
            BalancedSurface::Task
        }
    } else {
        balanced_surface(syscall)
    }
}

pub const fn balanced_surface(syscall: u64) -> BalancedSurface {
    match syscall {
        0 | 2 | 3 | 4 | 5 | 8 | 17 | 21 | 72 | 89 | 217 | 257 | 262 | 332 | 439 => {
            BalancedSurface::ImageRead
        }
        9 | 10 | 11 | 12 | 28 => BalancedSurface::Memory,
        13 | 14 | 15 | 24 | SYS_FORK | SYS_WAIT4 | 98 | 99 | 100 | 158 | 202 | 218 | 273 | 302
        | 334 => BalancedSurface::Task,
        1 | SYS_POLL | 20 => BalancedSurface::Console,
        318 => BalancedSurface::Random,
        SYS_GETPID | SYS_GETUID | SYS_GETGID | SYS_GETEUID | SYS_GETEGID | SYS_GETPPID
        | SYS_GETPGRP => BalancedSurface::VirtualIdentity,
        SYS_EXIT | SYS_EXIT_GROUP => BalancedSurface::Lifecycle,
        _ => BalancedSurface::Denied,
    }
}

pub const fn decide_balanced(syscall: u64) -> Decision {
    let action = match syscall {
        _ if matches!(
            balanced_surface(syscall),
            BalancedSurface::VirtualIdentity | BalancedSurface::Lifecycle
        ) =>
        {
            Action::Emulate
        }
        _ if matches!(balanced_surface(syscall), BalancedSurface::Denied) => Action::Deny(38),
        _ => Action::Mediate,
    };
    Decision { syscall, action }
}

/// The virtual pid of the single task in the balanced contract. The task
/// has no threads (thread creation is denied), so it is also its tid.
pub const BALANCED_VIRTUAL_PID: i64 = 4242;

pub const fn balanced_identity_result(syscall: u64) -> Option<i64> {
    match syscall {
        SYS_GETPID => Some(BALANCED_VIRTUAL_PID),
        SYS_GETPPID => Some(1),
        SYS_GETPGRP => Some(BALANCED_VIRTUAL_PID),
        SYS_GETUID | SYS_GETGID | SYS_GETEUID | SYS_GETEGID => Some(0),
        _ => None,
    }
}

/// Result reported for a mediated call whose host result is the caller's
/// host tid. `set_tid_address` executes on the host for its clear-on-exit
/// registration, but its return value is the caller's tid, so the task
/// sees its virtual tid instead. `None` keeps the host result.
pub const fn balanced_mediated_result(syscall: u64) -> Option<i64> {
    match syscall {
        SYS_SET_TID_ADDRESS => Some(BALANCED_VIRTUAL_PID),
        _ => None,
    }
}

/// Whether a futex operation word selects a priority-inheritance operation.
/// PI futexes keep the owner's tid in the futex word: `FUTEX_LOCK_PI`,
/// `FUTEX_LOCK_PI2` and `FUTEX_TRYLOCK_PI` store the caller's host tid in
/// workload memory, and `FUTEX_UNLOCK_PI` and the requeue-PI operations
/// compare the word against it. The command is classified by its low 7 bits
/// alone, so any combination of flag bits above it is covered:
/// `FUTEX_PRIVATE_FLAG` and `FUTEX_CLOCK_REALTIME`, and the additional flag
/// bits newer kernels accept.
pub const fn futex_priority_inheritance(op: u64) -> bool {
    const FUTEX_COMMAND: u64 = 0x7f;
    matches!(op & FUTEX_COMMAND, 6..=8 | 11..=13)
}

pub const fn balanced_arguments_allowed(syscall: u64, arg0: u64, arg1: u64, arg2: u64) -> bool {
    const O_ACCMODE: u64 = 3;
    const O_CREAT: u64 = 0o100;
    const O_EXCL: u64 = 0o200;
    const O_TRUNC: u64 = 0o1000;
    const O_APPEND: u64 = 0o2000;
    // O_TMPFILE is __O_TMPFILE | O_DIRECTORY. Only the __O_TMPFILE bit marks
    // a mutating open; masking the composite also rejected a plain
    // O_DIRECTORY, which glibc opendir() always passes.
    const O_TMPFILE_BIT: u64 = 0o20000000;
    const MUTATING_OPEN_FLAGS: u64 = O_CREAT | O_EXCL | O_TRUNC | O_APPEND | O_TMPFILE_BIT;
    const PROT_WRITE: u64 = 2;
    const PROT_EXEC: u64 = 4;
    match syscall {
        0 => arg2 <= 1024 * 1024,
        // The native supervisor additionally validates each descriptor in
        // tracee memory against the assigned bootstrap console descriptors.
        SYS_POLL => arg1 <= MAX_BOOTSTRAP_POLL_FDS && arg2 <= MAX_BOOTSTRAP_POLL_TIMEOUT_MS,
        1 => matches!(arg0, 1 | 2) && arg2 <= 4096,
        20 => matches!(arg0, 1 | 2) && arg2 <= 16,
        2 => arg1 & (O_ACCMODE | MUTATING_OPEN_FLAGS) == 0,
        9 | 10 => arg2 & (PROT_WRITE | PROT_EXEC) != (PROT_WRITE | PROT_EXEC),
        17 => arg2 <= 1024 * 1024,
        257 => arg2 & (O_ACCMODE | MUTATING_OPEN_FLAGS) == 0,
        302 => arg0 == 0 && arg1 <= 15,
        318 => arg2 <= 256,
        SYS_WAIT4 => arg0 == u64::MAX || arg0 > 0,
        SYS_FUTEX => !futex_priority_inheritance(arg1),
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emulates_virtual_identity() {
        assert_eq!(decide(SYS_GETPID).action, Action::Emulate);
    }

    #[test]
    fn mediates_output_instead_of_continuing_it() {
        assert_eq!(decide(SYS_WRITE).action, Action::Mediate);
    }

    #[test]
    fn denies_selected_and_unknown_syscalls() {
        assert_eq!(decide(SYS_UNAME).action, Action::Deny(1));
        assert_eq!(decide(0xffff).action, Action::Deny(38));
    }

    #[test]
    fn balanced_contract_is_classified_and_fail_closed() {
        assert_eq!(balanced_surface(257), BalancedSurface::ImageRead);
        assert_eq!(balanced_surface(9), BalancedSurface::Memory);
        assert_eq!(balanced_surface(1), BalancedSurface::Console);
        assert_eq!(balanced_surface(SYS_FORK), BalancedSurface::Task);
        assert_eq!(balanced_surface(SYS_WAIT4), BalancedSurface::Task);
        // rt_sigaction and rt_sigprocmask without rt_sigreturn would let a
        // handler run but never return.
        assert_eq!(balanced_surface(15), BalancedSurface::Task);
        assert_eq!(decide_balanced(15).action, Action::Mediate);
        assert_eq!(decide_balanced(0).action, Action::Mediate);
        assert_eq!(decide_balanced(SYS_GETPID).action, Action::Emulate);
        assert_eq!(balanced_identity_result(SYS_GETUID), Some(0));
        assert_eq!(decide_balanced(16).action, Action::Deny(38));
        assert_eq!(decide_balanced(41).action, Action::Deny(38));
        assert_eq!(decide_balanced(999).action, Action::Deny(38));
    }

    #[test]
    fn balanced_contract_rejects_sensitive_and_mutating_operations() {
        assert!(balanced_arguments_allowed(257, 0, 0, 0));
        assert!(!balanced_arguments_allowed(257, 0, 0, 0o100));
        assert!(!balanced_arguments_allowed(257, 0, 0, 1));
        assert!(!balanced_arguments_allowed(10, 0, 0, 6));
        assert_eq!(balanced_surface(16), BalancedSurface::Denied);
        assert_eq!(balanced_surface(41), BalancedSurface::Denied);
        assert_eq!(balanced_surface(105), BalancedSurface::Denied);
    }

    #[test]
    fn balanced_contract_reports_the_virtual_tid() {
        // set_tid_address stays mediated for its clear-on-exit registration,
        // but reports the same virtual id as getpid instead of the host tid.
        assert_eq!(decide_balanced(SYS_SET_TID_ADDRESS).action, Action::Mediate);
        assert!(balanced_arguments_allowed(
            SYS_SET_TID_ADDRESS,
            0x1000,
            0,
            0
        ));
        assert_eq!(
            balanced_mediated_result(SYS_SET_TID_ADDRESS),
            balanced_identity_result(SYS_GETPID)
        );
        for syscall in [0, 1, 9, SYS_FUTEX, SYS_WAIT4, 273, 334] {
            assert_eq!(balanced_mediated_result(syscall), None);
        }
    }

    #[test]
    fn balanced_contract_rejects_priority_inheritance_futexes() {
        const PRIVATE: u64 = 128;
        const CLOCK_REALTIME: u64 = 256;
        // LOCK_PI, UNLOCK_PI, TRYLOCK_PI, WAIT_REQUEUE_PI, CMP_REQUEUE_PI,
        // LOCK_PI2 keep the owner's host tid in the futex word.
        for command in [6, 7, 8, 11, 12, 13] {
            for op in [
                command,
                command | PRIVATE,
                command | CLOCK_REALTIME,
                command | PRIVATE | CLOCK_REALTIME,
                command | PRIVATE | 512,
                command | PRIVATE | 1024,
                command | 1 << 20,
                // Register bits above the 32-bit operation word.
                command | 0xffff_ffff_0000_0000,
            ] {
                assert!(futex_priority_inheritance(op), "{op:#x}");
                assert!(!balanced_arguments_allowed(SYS_FUTEX, 0x1000, op, 0));
            }
        }
        // WAIT, WAKE, FD, REQUEUE, CMP_REQUEUE, WAKE_OP, WAIT_BITSET,
        // WAKE_BITSET and unknown commands stay mediated.
        for command in [0, 1, 2, 3, 4, 5, 9, 10, 14, 0x46] {
            for op in [
                command,
                command | PRIVATE,
                command | PRIVATE | CLOCK_REALTIME,
            ] {
                assert!(!futex_priority_inheritance(op), "{op:#x}");
                assert!(balanced_arguments_allowed(SYS_FUTEX, 0x1000, op, 0));
            }
        }
    }

    #[test]
    fn balanced_contract_allows_read_only_directory_opens() {
        const AT_FDCWD: u64 = -100i64 as u64;
        const O_WRONLY: u64 = 1;
        const O_RDWR: u64 = 2;
        const O_CREAT: u64 = 0o100;
        const O_TRUNC: u64 = 0o1000;
        const O_APPEND: u64 = 0o2000;
        const O_NONBLOCK: u64 = 0o4000;
        const O_DIRECTORY: u64 = 0o200000;
        const O_CLOEXEC: u64 = 0o2000000;
        const O_TMPFILE: u64 = 0o20200000;
        // glibc opendir(): openat(AT_FDCWD, path, O_RDONLY|O_NONBLOCK|O_CLOEXEC|O_DIRECTORY)
        let opendir = O_NONBLOCK | O_CLOEXEC | O_DIRECTORY;
        assert!(balanced_arguments_allowed(257, AT_FDCWD, 0x1000, opendir));
        assert!(balanced_arguments_allowed(2, 0x1000, O_DIRECTORY, 0));
        // O_DIRECTORY does not unlock anything mutating, and O_TMPFILE stays
        // denied with or without its O_DIRECTORY bit.
        for flags in [
            O_DIRECTORY | O_WRONLY,
            O_DIRECTORY | O_RDWR,
            O_DIRECTORY | O_CREAT,
            O_DIRECTORY | O_TRUNC,
            O_DIRECTORY | O_APPEND,
            O_TMPFILE,
            O_TMPFILE & !O_DIRECTORY,
        ] {
            assert!(!balanced_arguments_allowed(257, AT_FDCWD, 0x1000, flags));
            assert!(!balanced_arguments_allowed(2, 0x1000, flags, 0));
        }
    }

    #[test]
    fn systemd_contract_adds_shared_network_without_changing_balanced_policy() {
        assert_eq!(decide_balanced(41).action, Action::Deny(38));
        assert_eq!(decide_systemd(41, false).action, Action::Deny(38));
        assert_eq!(decide_systemd(41, true).action, Action::Mediate);
        assert_eq!(systemd_surface(42, true), BalancedSurface::Network);
        assert_eq!(decide_systemd(59, true).action, Action::Mediate);
        assert_eq!(
            decide_systemd(SYS_GETPRIORITY, true).action,
            Action::Emulate
        );
        assert_eq!(
            systemd_surface(SYS_GETPRIORITY, true),
            BalancedSurface::VirtualIdentity
        );
        assert_eq!(
            decide_systemd(SYS_SETPRIORITY, true).action,
            Action::Emulate
        );
        assert_eq!(
            systemd_surface(SYS_SETPRIORITY, true),
            BalancedSurface::VirtualIdentity
        );
        assert_eq!(decide_systemd(165, true).action, Action::Emulate);
        assert_eq!(decide_systemd(166, true).action, Action::Emulate);
        assert_eq!(decide_systemd(63, true).action, Action::Emulate);
        assert_eq!(decide_systemd(291, true).action, Action::Mediate);
        assert_eq!(decide_systemd(289, true).action, Action::Mediate);
        assert_eq!(decide_systemd(294, true).action, Action::Mediate);
        assert_eq!(decide_systemd(283, true).action, Action::Mediate);
        assert_eq!(decide_systemd(56, true).action, Action::Mediate);
        assert_eq!(decide_systemd(62, true).action, Action::Mediate);
        assert_eq!(systemd_surface(62, true), BalancedSurface::Task);
        assert_eq!(decide_systemd(112, true).action, Action::Emulate);
        assert_eq!(systemd_surface(112, true), BalancedSurface::Task);
        assert_eq!(decide_systemd(109, true).action, Action::Emulate);
        assert_eq!(systemd_surface(109, true), BalancedSurface::Task);
        assert_eq!(decide_systemd(93, true).action, Action::Emulate);
        assert_eq!(systemd_surface(93, true), BalancedSurface::InstanceWrite);
        assert_eq!(decide_systemd(153, true).action, Action::Emulate);
        assert_eq!(systemd_surface(153, true), BalancedSurface::Console);
        assert_eq!(decide_systemd(SYS_REBOOT, true).action, Action::Emulate);
        assert_eq!(
            systemd_surface(SYS_REBOOT, true),
            BalancedSurface::Lifecycle
        );
        assert_eq!(decide_balanced(SYS_REBOOT).action, Action::Deny(38));
        assert_eq!(decide_systemd(91, true).action, Action::Mediate);
        assert_eq!(systemd_surface(91, true), BalancedSurface::InstanceWrite);
        assert_eq!(decide_systemd(270, true).action, Action::Mediate);
        assert_eq!(systemd_surface(270, true), BalancedSurface::Task);
        assert_eq!(decide_systemd(290, true).action, Action::Mediate);
        assert_eq!(systemd_surface(290, true), BalancedSurface::Task);
        assert_eq!(decide_systemd(7, true).action, Action::Mediate);
        assert_eq!(systemd_surface(7, true), BalancedSurface::Task);
        assert_eq!(decide_systemd(100, true).action, Action::Mediate);
        assert_eq!(systemd_surface(100, true), BalancedSurface::Task);
        assert_eq!(decide_systemd(98, true).action, Action::Mediate);
        assert_eq!(systemd_surface(98, true), BalancedSurface::Task);
        assert_eq!(decide_systemd(228, true).action, Action::Mediate);
        assert_eq!(systemd_surface(228, true), BalancedSurface::Task);
        assert_eq!(decide_systemd(271, true).action, Action::Mediate);
        assert_eq!(systemd_surface(271, true), BalancedSurface::Task);
        assert_eq!(decide_systemd(292, true).action, Action::Mediate);
        assert_eq!(systemd_surface(292, true), BalancedSurface::Task);
        assert_eq!(decide_systemd(293, true).action, Action::Mediate);
        assert_eq!(systemd_surface(293, true), BalancedSurface::Task);
        assert_eq!(decide_systemd(32, true).action, Action::Mediate);
        assert_eq!(systemd_surface(32, true), BalancedSurface::Task);
        assert_eq!(decide_systemd(33, true).action, Action::Mediate);
        assert_eq!(systemd_surface(33, true), BalancedSurface::Task);
        assert_eq!(decide_systemd(77, true).action, Action::Mediate);
        assert_eq!(systemd_surface(77, true), BalancedSurface::InstanceWrite);
        assert_eq!(systemd_surface(18, true), BalancedSurface::InstanceWrite);
        assert_eq!(systemd_surface(73, true), BalancedSurface::InstanceWrite);
        assert_eq!(systemd_surface(74, true), BalancedSurface::InstanceWrite);
        assert_eq!(systemd_surface(75, true), BalancedSurface::InstanceWrite);
        assert_eq!(systemd_surface(76, true), BalancedSurface::InstanceWrite);
        assert_eq!(systemd_surface(90, true), BalancedSurface::InstanceWrite);
        assert_eq!(systemd_surface(92, true), BalancedSurface::InstanceWrite);
        assert_eq!(systemd_surface(93, true), BalancedSurface::InstanceWrite);
        assert_eq!(systemd_surface(94, true), BalancedSurface::InstanceWrite);
        assert_eq!(systemd_surface(260, true), BalancedSurface::InstanceWrite);
        assert_eq!(systemd_surface(280, true), BalancedSurface::InstanceWrite);
        assert_eq!(systemd_surface(306, true), BalancedSurface::InstanceWrite);
        assert_eq!(systemd_surface(79, true), BalancedSurface::Task);
        assert_eq!(systemd_surface(81, true), BalancedSurface::Task);
        assert_eq!(systemd_surface(190, true), BalancedSurface::InstanceWrite);
        assert_eq!(systemd_surface(196, true), BalancedSurface::InstanceWrite);
        assert_eq!(systemd_surface(199, true), BalancedSurface::InstanceWrite);
        assert_eq!(decide_systemd(25, true).action, Action::Mediate);
        assert_eq!(systemd_surface(25, true), BalancedSurface::Memory);
        assert_eq!(systemd_surface(82, true), BalancedSurface::InstanceWrite);
        assert_eq!(systemd_surface(87, true), BalancedSurface::InstanceWrite);
        assert_eq!(systemd_surface(84, true), BalancedSurface::InstanceWrite);
        assert_eq!(systemd_surface(193, true), BalancedSurface::ImageRead);
        assert_eq!(
            systemd_surface(SYS_SETUID, true),
            BalancedSurface::VirtualIdentity
        );
        assert_eq!(
            systemd_surface(SYS_SETGID, true),
            BalancedSurface::VirtualIdentity
        );
        assert_eq!(
            systemd_surface(SYS_SETGROUPS, true),
            BalancedSurface::VirtualIdentity
        );
        assert_eq!(
            systemd_surface(SYS_SETREUID, true),
            BalancedSurface::VirtualIdentity
        );
        assert_eq!(
            systemd_surface(SYS_SETREGID, true),
            BalancedSurface::VirtualIdentity
        );
        assert_eq!(
            systemd_surface(SYS_CLOSE_RANGE, true),
            BalancedSurface::Task
        );
        assert_eq!(decide_systemd(435, true).action, Action::Mediate);
        assert_eq!(decide_systemd(SYS_PIDFD_OPEN, true).action, Action::Mediate);
        assert_eq!(systemd_surface(SYS_PIDFD_OPEN, true), BalancedSurface::Task);
        assert_eq!(
            decide_systemd(SYS_PIDFD_SEND_SIGNAL, true).action,
            Action::Mediate
        );
        assert_eq!(decide_systemd(452, true).action, Action::Mediate);
        assert_eq!(systemd_surface(452, true), BalancedSurface::InstanceWrite);
        assert_eq!(decide_systemd(247, true).action, Action::Mediate);
        assert_eq!(decide_systemd(SYS_READLINKAT, true).action, Action::Mediate);
        assert_eq!(decide_systemd(88, true).action, Action::Mediate);
        assert_eq!(decide_systemd(SYS_UNLINKAT, true).action, Action::Mediate);
        assert_eq!(decide_systemd(SYS_RENAMEAT, true).action, Action::Mediate);
        assert_eq!(decide_systemd(SYS_LINKAT, true).action, Action::Mediate);
        assert_eq!(decide_systemd(SYS_SYMLINKAT, true).action, Action::Mediate);
        assert_eq!(
            decide_systemd(SYS_MEMFD_CREATE, true).action,
            Action::Mediate
        );
        assert_eq!(decide_systemd(SYS_GETTID, true).action, Action::Emulate);
        assert_eq!(
            systemd_surface(SYS_READLINKAT, true),
            BalancedSurface::ImageRead
        );
        assert_eq!(systemd_surface(88, true), BalancedSurface::InstanceWrite);
        assert_eq!(
            systemd_surface(SYS_UNLINKAT, true),
            BalancedSurface::InstanceWrite
        );
        assert_eq!(
            systemd_surface(SYS_RENAMEAT, true),
            BalancedSurface::InstanceWrite
        );
        assert_eq!(
            systemd_surface(SYS_LINKAT, true),
            BalancedSurface::InstanceWrite
        );
        assert_eq!(
            systemd_surface(SYS_SYMLINKAT, true),
            BalancedSurface::InstanceWrite
        );
        assert_eq!(decide_balanced(SYS_READLINKAT).action, Action::Deny(38));
        assert_eq!(decide_balanced(88).action, Action::Deny(38));
        assert_eq!(decide_balanced(SYS_UNLINKAT).action, Action::Deny(38));
        assert_eq!(decide_balanced(SYS_RENAMEAT).action, Action::Deny(38));
        assert_eq!(decide_balanced(SYS_LINKAT).action, Action::Deny(38));
        assert_eq!(decide_balanced(SYS_SYMLINKAT).action, Action::Deny(38));
        assert_eq!(decide_balanced(SYS_MEMFD_CREATE).action, Action::Deny(38));
        assert_eq!(decide_balanced(SYS_GETTID).action, Action::Deny(38));
        assert_eq!(decide_balanced(63).action, Action::Deny(38));
        assert_eq!(decide_balanced(291).action, Action::Deny(38));
        assert_eq!(decide_balanced(289).action, Action::Deny(38));
        assert_eq!(decide_balanced(294).action, Action::Deny(38));
        assert_eq!(decide_balanced(283).action, Action::Deny(38));
        assert_eq!(decide_balanced(56).action, Action::Deny(38));
        assert_eq!(decide_balanced(62).action, Action::Deny(38));
        assert_eq!(decide_balanced(435).action, Action::Deny(38));
        assert_eq!(decide_balanced(247).action, Action::Deny(38));
    }
}
