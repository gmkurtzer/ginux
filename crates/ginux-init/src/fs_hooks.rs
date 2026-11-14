//! Native filesystem-lane dispatch hooks.
//!
//! Gate 31 supplies the complete immutable call context and operation
//! inventory required before this lane can safely claim an operation. The
//! lane still declines every call; operation-family gates enable it later.

use ginux_fs_policy::CallContext;

/// Lane membership by syscall number. Reserved for G06+.
pub const fn claims(syscall: u64) -> bool {
    let _ = syscall;
    false
}

/// Emulated result when this lane handles the entry. `None` falls through to
/// the shared balanced-contract decision. Reserved for G06+.
pub fn claim(context: &CallContext, confinement_permits: bool) -> Option<i64> {
    // Do not let a partial context become authority. This remains deliberately
    // unreachable while `claims` is false, but is a fail-closed interlock for
    // the first operation-family gate.
    if !ginux_fs_policy::filesystem_hook_authorized(context, confinement_permits)
        || !claims(context.syscall)
    {
        return None;
    }
    None
}
