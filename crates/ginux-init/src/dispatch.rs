//! Shared native-syscall dispatch wiring.
//!
//! Single owner for entry classification and the contract decision used by
//! the supervision loops. Lane hooks live in disjoint files
//! (`process_hooks`, `fs_hooks`, `channel_hooks`) and currently all decline,
//! so every entry falls through to the existing balanced-contract decision
//! with byte-identical behavior.

use ginux_core::{balanced_arguments_allowed, decide_balanced, BalancedSurface, Decision};

use crate::supervisor::UserRegs;

/// Lane a syscall number is routed to once its owning lane implements it.
pub enum Lane {
    Process,
    Filesystem,
    Channel,
    Contract,
}

/// Classify an entry by number for future lane wiring. All lanes decline
/// today, so this always returns `Lane::Contract`.
pub fn classify(syscall: u64) -> Lane {
    if crate::process_hooks::claims(syscall) {
        Lane::Process
    } else if crate::fs_hooks::claims(syscall) {
        Lane::Filesystem
    } else if crate::channel_hooks::claims(syscall) {
        Lane::Channel
    } else {
        Lane::Contract
    }
}

/// Outcome of dispatching one native entry.
pub enum NativeDecision {
    /// A lane hook claimed the entry; emulate it with this result.
    Hooked { lane: Lane, result: i64 },
    /// No hook claimed the entry; apply the shared contract decision.
    Contract(Decision),
}

/// Dispatch one native supervision entry through lane hooks first, then the
/// existing balanced-contract decision. `classify` is the single routing
/// table: each lane arm gives its hook the first chance, and anything no
/// hook claims falls through to `decide_balanced` byte-identically.
pub fn decide_native(syscall: u64, rdi: u64, rsi: u64, rdx: u64) -> NativeDecision {
    match classify(syscall) {
        Lane::Process => match crate::process_hooks::claim(syscall, rdi, rsi, rdx) {
            Some(result) => NativeDecision::Hooked {
                lane: Lane::Process,
                result,
            },
            None => NativeDecision::Contract(decide_balanced(syscall)),
        },
        Lane::Filesystem => match crate::fs_hooks::claim(syscall, rdi, rsi, rdx) {
            Some(result) => NativeDecision::Hooked {
                lane: Lane::Filesystem,
                result,
            },
            None => NativeDecision::Contract(decide_balanced(syscall)),
        },
        Lane::Channel => match crate::channel_hooks::claim(syscall, rdi, rsi, rdx) {
            Some(result) => NativeDecision::Hooked {
                lane: Lane::Channel,
                result,
            },
            None => NativeDecision::Contract(decide_balanced(syscall)),
        },
        Lane::Contract => NativeDecision::Contract(decide_balanced(syscall)),
    }
}

/// Dispatch one process-supervision entry through the process-lane hook.
/// `None` keeps the existing entry match in the supervision loop.
pub fn process_entry(syscall: u64, rdi: u64, rsi: u64, rdx: u64) -> Option<i64> {
    crate::process_hooks::claim(syscall, rdi, rsi, rdx)
}

pub fn native_arguments_allowed(syscall: u64, registers: &UserRegs) -> bool {
    balanced_arguments_allowed(syscall, registers.rdi, registers.rsi, registers.rdx)
}

pub fn balanced_surface_name(surface: BalancedSurface) -> &'static [u8] {
    match surface {
        BalancedSurface::ImageRead => b"immutable-image result=pending",
        BalancedSurface::Memory => b"memory result=pending",
        BalancedSurface::Task => b"task result=pending",
        BalancedSurface::Console => b"bootstrap-console result=pending",
        BalancedSurface::Random => b"random result=pending",
        BalancedSurface::VirtualIdentity | BalancedSurface::Lifecycle | BalancedSurface::Denied => {
            b"invalid result=denied"
        }
    }
}
