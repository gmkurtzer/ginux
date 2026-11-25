//! Native filesystem-lane dispatch hooks.
//!
//! Gate 31 supplies the complete immutable call context and operation
//! inventory required before this lane can safely claim an operation. The
//! Gate 53 routes every filesystem-lane entry through the shared authority
//! guard before the kernel continues the already-confined operation.

use ginux_fs_policy::CallContext;

/// Lane membership comes from the independently checked operation inventory.
pub const fn claims(syscall: u64) -> bool {
    matches!(
        syscall,
        // Observation/directory-state, then safe-open. Later families are
        // added only after these routes keep their positive and negative
        // evidence. Unsupported safe-open spellings stay owned and receive
        // their inventory-defined ENOSYS from the existing core decision.
        2 | 4
            | 6
            | 21
            | 79
            | 85
            | 89
            | 137
            | 191
            | 192
            | 194
            | 195
            | 257
            | 262
            | 267
            | 332
            | 437
            | 439
    )
}

/// Result of asking the filesystem lane to own an entry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Claim {
    /// This syscall is not owned by the filesystem lane.
    Unclaimed,
    /// Continue the original syscall against its pinned kernel root/cwd/fd
    /// objects.  No supervisor pathname executor is involved.
    Continue,
    /// The lane owns the syscall but validation failed before any host effect.
    Denied(i64),
}

const EPERM: i64 = 1;

fn evaluate(context: &CallContext, confinement_permits: bool, claimed: bool) -> Claim {
    if !claimed {
        return Claim::Unclaimed;
    }
    if ginux_fs_policy::confined_kernel_route(context, confinement_permits).is_err() {
        return Claim::Denied(-EPERM);
    }
    Claim::Continue
}

/// Ask the filesystem lane to own an entry. Claimed operations return an
/// explicitly authorized confined-kernel continuation or a pre-effect denial;
/// only genuinely unclaimed operations use another inventory owner.
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
        assert_eq!(evaluate(&context, true, true), Claim::Continue);

        let mut stale = context;
        stale.authority.descriptor_generation += 1;
        assert_eq!(evaluate(&stale, true, true), Claim::Denied(-EPERM));
        assert_eq!(evaluate(&context, false, true), Claim::Denied(-EPERM));
    }

    #[test]
    fn only_unowned_operation_is_unclaimed() {
        assert_eq!(evaluate(&open_context(), true, false), Claim::Unclaimed);
    }

    #[test]
    fn observation_and_safe_open_are_the_only_enabled_families() {
        for syscall in [
            2, 4, 6, 21, 79, 85, 89, 137, 191, 192, 194, 195, 257, 262, 267, 332, 437, 439,
        ] {
            assert!(claims(syscall), "enabled syscall {syscall}");
        }
        for syscall in [76, 82, 83, 86, 264, 316, 452] {
            assert!(!claims(syscall), "later-family syscall {syscall}");
        }
    }
}
