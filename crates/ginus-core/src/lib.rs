#![no_std]

pub const SYS_WRITE: u64 = 1;
pub const SYS_GETPID: u64 = 39;
pub const SYS_EXIT: u64 = 60;
pub const SYS_UNAME: u64 = 63;
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

pub const fn decide_native_bootstrap(syscall: u64) -> Decision {
    let action = match syscall {
        SYS_GETPID => Action::Emulate,
        SYS_EXIT | SYS_EXIT_GROUP => Action::Emulate,
        0 | 1 | 2 | 3 | 4 | 5 | 8 | 9 | 10 | 11 | 12 | 13 | 14 | 17 | 20 | 21 | 28 | 72 | 89
        | 99 | 102 | 104 | 107 | 108 | 158 | 202 | 217 | 218 | 257 | 262 | 273 | 302 | 318
        | 332 | 334 | 439 => Action::Mediate,
        _ => Action::Deny(38),
    };
    Decision { syscall, action }
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
    fn native_bootstrap_contract_is_explicit_and_fail_closed() {
        assert_eq!(decide_native_bootstrap(0).action, Action::Mediate);
        assert_eq!(decide_native_bootstrap(257).action, Action::Mediate);
        assert_eq!(decide_native_bootstrap(SYS_GETPID).action, Action::Emulate);
        assert_eq!(decide_native_bootstrap(41).action, Action::Deny(38));
        assert_eq!(decide_native_bootstrap(999).action, Action::Deny(38));
    }
}
