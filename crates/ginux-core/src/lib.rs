#![no_std]

pub const SYS_WRITE: u64 = 1;
pub const SYS_GETPID: u64 = 39;
pub const SYS_FORK: u64 = 57;
pub const SYS_EXIT: u64 = 60;
pub const SYS_WAIT4: u64 = 61;
pub const SYS_UNAME: u64 = 63;
pub const SYS_GETUID: u64 = 102;
pub const SYS_GETGID: u64 = 104;
pub const SYS_GETEUID: u64 = 107;
pub const SYS_GETEGID: u64 = 108;
pub const SYS_GETPPID: u64 = 110;
pub const SYS_GETPGRP: u64 = 111;
pub const SYS_EXIT_GROUP: u64 = 231;

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
        63 | 165 | 166 => Action::Emulate,
        16 | 56 | 57 | 58 | 59 | 61 | 80 | 83 | 95 | 125 | 126 | 137 | 157 | 213 | 232 | 233
        | 247 | 253 | 254 | 255 | 258 | 281 | 283 | 286 | 287 | 289 | 291 | 294 | 435 | 441 => {
            Action::Mediate
        }
        _ => decide_balanced(syscall).action,
    };
    Decision { syscall, action }
}

pub const fn systemd_surface(syscall: u64, shared_network: bool) -> BalancedSurface {
    if shared_network && is_shared_network_syscall(syscall) {
        BalancedSurface::Network
    } else if matches!(
        syscall,
        56 | 57
            | 58
            | 61
            | 213
            | 232
            | 233
            | 247
            | 253
            | 254
            | 255
            | 281
            | 283
            | 286
            | 287
            | 289
            | 291
            | 294
            | 435
            | 441
    ) {
        BalancedSurface::Task
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
        13 | 14 | SYS_FORK | SYS_WAIT4 | 99 | 158 | 202 | 218 | 273 | 302 | 334 => {
            BalancedSurface::Task
        }
        1 | 20 => BalancedSurface::Console,
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

pub const fn balanced_identity_result(syscall: u64) -> Option<i64> {
    match syscall {
        SYS_GETPID => Some(4242),
        SYS_GETPPID => Some(1),
        SYS_GETPGRP => Some(4242),
        SYS_GETUID | SYS_GETGID | SYS_GETEUID | SYS_GETEGID => Some(0),
        _ => None,
    }
}

pub const fn balanced_arguments_allowed(syscall: u64, arg0: u64, arg1: u64, arg2: u64) -> bool {
    const O_ACCMODE: u64 = 3;
    const O_CREAT: u64 = 0o100;
    const O_EXCL: u64 = 0o200;
    const O_TRUNC: u64 = 0o1000;
    const O_APPEND: u64 = 0o2000;
    const O_TMPFILE: u64 = 0o20200000;
    const MUTATING_OPEN_FLAGS: u64 = O_CREAT | O_EXCL | O_TRUNC | O_APPEND | O_TMPFILE;
    const PROT_WRITE: u64 = 2;
    const PROT_EXEC: u64 = 4;
    match syscall {
        0 => arg2 <= 1024 * 1024,
        1 => matches!(arg0, 1 | 2) && arg2 <= 4096,
        20 => matches!(arg0, 1 | 2) && arg2 <= 16,
        2 => arg1 & (O_ACCMODE | MUTATING_OPEN_FLAGS) == 0,
        9 | 10 => arg2 & (PROT_WRITE | PROT_EXEC) != (PROT_WRITE | PROT_EXEC),
        17 => arg2 <= 1024 * 1024,
        257 => arg2 & (O_ACCMODE | MUTATING_OPEN_FLAGS) == 0,
        302 => arg0 == 0 && arg1 <= 15,
        318 => arg2 <= 256,
        SYS_WAIT4 => arg0 == u64::MAX || arg0 > 0,
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
    fn systemd_contract_adds_shared_network_without_changing_balanced_policy() {
        assert_eq!(decide_balanced(41).action, Action::Deny(38));
        assert_eq!(decide_systemd(41, false).action, Action::Deny(38));
        assert_eq!(decide_systemd(41, true).action, Action::Mediate);
        assert_eq!(systemd_surface(42, true), BalancedSurface::Network);
        assert_eq!(decide_systemd(59, true).action, Action::Mediate);
        assert_eq!(decide_systemd(165, true).action, Action::Emulate);
        assert_eq!(decide_systemd(166, true).action, Action::Emulate);
        assert_eq!(decide_systemd(63, true).action, Action::Emulate);
        assert_eq!(decide_systemd(291, true).action, Action::Mediate);
        assert_eq!(decide_systemd(289, true).action, Action::Mediate);
        assert_eq!(decide_systemd(294, true).action, Action::Mediate);
        assert_eq!(decide_systemd(283, true).action, Action::Mediate);
        assert_eq!(decide_systemd(56, true).action, Action::Mediate);
        assert_eq!(decide_systemd(435, true).action, Action::Mediate);
        assert_eq!(decide_systemd(247, true).action, Action::Mediate);
        assert_eq!(decide_balanced(63).action, Action::Deny(38));
        assert_eq!(decide_balanced(291).action, Action::Deny(38));
        assert_eq!(decide_balanced(289).action, Action::Deny(38));
        assert_eq!(decide_balanced(294).action, Action::Deny(38));
        assert_eq!(decide_balanced(283).action, Action::Deny(38));
        assert_eq!(decide_balanced(56).action, Action::Deny(38));
        assert_eq!(decide_balanced(435).action, Action::Deny(38));
        assert_eq!(decide_balanced(247).action, Action::Deny(38));
    }
}
