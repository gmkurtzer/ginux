//! Native process-lane dispatch hooks.
//!
//! Owner: G-process lane. Next task: G03 connects native fork to managed
//! child ownership, replacing fixture-specific completion rules with ordinary
//! parent/child ownership. Until then every hook declines and the supervisor
//! keeps its existing behavior byte-identical.

/// Lane membership by syscall number. Reserved for G03+.
pub fn claims(syscall: u64) -> bool {
    let _ = syscall;
    false
}

/// Emulated result when this lane handles the entry. `None` falls through to
/// the shared balanced-contract decision. Reserved for G03+.
pub fn claim(syscall: u64, rdi: u64, rsi: u64, rdx: u64) -> Option<i64> {
    let _ = (syscall, rdi, rsi, rdx);
    None
}
