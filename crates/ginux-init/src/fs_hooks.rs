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

/// Result of asking the filesystem lane to own an entry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Claim {
    /// This syscall is not owned by the filesystem lane.
    Unclaimed,
    /// The lane owns the syscall but validation failed before any host effect.
    Denied(i64),
}

const EPERM: i64 = 1;
const ENOSYS: i64 = 38;

fn evaluate(context: &CallContext, confinement_permits: bool, claimed: bool) -> Claim {
    if !claimed {
        return Claim::Unclaimed;
    }
    if !ginux_fs_policy::filesystem_hook_authorized(context, confinement_permits) {
        return Claim::Denied(-EPERM);
    }
    // A future operation-family gate must replace this explicit denial with
    // its handler. Ownership can never fall through to a legacy executor.
    Claim::Denied(-ENOSYS)
}

/// Ask the filesystem lane to own an entry. Claimed operations always return
/// either a handled result or an explicit denial; only genuinely unclaimed
/// operations may fall through to the shared contract executor.
pub fn claim(context: &CallContext, confinement_permits: bool) -> Claim {
    evaluate(context, confinement_permits, claims(context.syscall))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ginux_fs_policy::{
        AuthorityContext, Capability, CapabilityKind, ContractAuthorization, InstanceContext,
        ObjectId, Rights, TaskContext,
    };

    fn open_context() -> CallContext {
        let root = Capability::new(
            CapabilityKind::Root,
            2,
            3,
            5,
            ObjectId {
                device: 7,
                inode: 11,
            },
            Rights::LOOKUP,
        );
        CallContext::x86_64(
            2,
            [0; 6],
            TaskContext {
                host: 13,
                virtual_id: 17,
                generation: 19,
            },
            InstanceContext {
                identity: 2,
                lifetime: 3,
                root_generation: 5,
            },
            AuthorityContext {
                root,
                cwd: Capability {
                    kind: CapabilityKind::WorkingDirectory,
                    ..root
                },
                descriptor_table: Capability {
                    kind: CapabilityKind::DescriptorTable,
                    ..root
                },
                cwd_generation: 5,
                descriptor_generation: 5,
            },
            ContractAuthorization::Mediate,
        )
    }

    #[test]
    fn claimed_operation_never_falls_through() {
        let context = open_context();
        assert_eq!(evaluate(&context, true, true), Claim::Denied(-ENOSYS));

        let mut stale = context;
        stale.authority.descriptor_generation += 1;
        assert_eq!(evaluate(&stale, true, true), Claim::Denied(-EPERM));
        assert_eq!(evaluate(&context, false, true), Claim::Denied(-EPERM));
    }

    #[test]
    fn only_unowned_operation_is_unclaimed() {
        assert_eq!(evaluate(&open_context(), true, false), Claim::Unclaimed);
    }
}
