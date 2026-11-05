//! Native channel-lane dispatch hooks.
//!
//! Owner: G-files/channel adapter lane (serialized with G-files changes).
//! Next task: G14 supplies the generic opaque transport the §4 frames ride
//! on. Until then every hook declines and the supervisor keeps its existing
//! behavior byte-identical.

/// Lane membership by syscall number. Reserved for G14.
pub fn claims(syscall: u64) -> bool {
    let _ = syscall;
    false
}

/// Emulated result when this lane handles the entry. `None` falls through to
/// the shared balanced-contract decision. Reserved for G14.
pub fn claim(syscall: u64, rdi: u64, rsi: u64, rdx: u64) -> Option<i64> {
    let _ = (syscall, rdi, rsi, rdx);
    None
}
