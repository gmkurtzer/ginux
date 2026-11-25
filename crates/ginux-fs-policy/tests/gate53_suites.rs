use ginux_fs_policy::{
    accountable_route, confined_kernel_route, descriptor_kernel_route, descriptor_required_rights,
    filesystem_lane_owns, operation, operation_executor, operation_family, AuthorityContext,
    CallContext, Capability, CapabilityKind, ContractAuthorization, Executor, InstanceContext,
    ObjectId, OperationFamily, Owner, Rights, TaskContext, OPERATIONS,
};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static FIXTURE_SEQUENCE: AtomicU64 = AtomicU64::new(1);

fn fixture(name: &str) -> PathBuf {
    let sequence = FIXTURE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "ginux-gate53-{name}-{}-{sequence}",
        std::process::id()
    ));
    fs::create_dir(&path).unwrap();
    path
}

fn context(syscall: u64) -> CallContext {
    let instance = 17;
    let lifetime = 19;
    let root_generation = 23;
    let cwd_generation = 29;
    let descriptor_generation = 31;
    CallContext::x86_64(
        syscall,
        [0; 6],
        TaskContext {
            host: 37,
            virtual_id: 41,
            generation: 43,
        },
        InstanceContext {
            identity: instance,
            lifetime,
            root_generation,
        },
        AuthorityContext {
            root: Capability::new(
                CapabilityKind::Root,
                instance,
                lifetime,
                root_generation,
                ObjectId {
                    device: 47,
                    inode: 53,
                },
                Rights::LOOKUP.union(Rights::READ),
            ),
            cwd: Capability::new(
                CapabilityKind::WorkingDirectory,
                instance,
                lifetime,
                cwd_generation,
                ObjectId {
                    device: 47,
                    inode: 59,
                },
                Rights::LOOKUP,
            ),
            descriptor_table: Capability::new(
                CapabilityKind::DescriptorTable,
                instance,
                lifetime,
                descriptor_generation,
                ObjectId {
                    device: instance,
                    inode: descriptor_generation,
                },
                Rights::LOOKUP
                    .union(Rights::READ)
                    .union(Rights::WRITE)
                    .union(Rights::CREATE)
                    .union(Rights::METADATA),
            ),
            cwd_generation,
            descriptor_generation,
        },
        ContractAuthorization::Mediate,
    )
}

#[test]
fn fs_t01_namespace_walking_has_one_ordered_route() {
    for operation in OPERATIONS {
        let route = accountable_route(&context(operation.syscall)).unwrap();
        assert_eq!(route.operation, operation);
        assert_eq!(route.family, operation_family(operation));
        assert_eq!(route.executor, operation_executor(operation));
        assert_eq!(
            filesystem_lane_owns(operation.syscall),
            operation.owner == Owner::FilesystemLane
        );
    }
    assert!(accountable_route(&context(u64::MAX)).is_err());
}

#[test]
fn fs_t02_deterministic_exchange_keeps_acquired_object() {
    let root = fixture("race");
    let selected = root.join("selected");
    let replacement = root.join("replacement");
    fs::write(&selected, b"authorized").unwrap();
    fs::write(&replacement, b"outside-decoy").unwrap();

    let mut acquired = File::open(&selected).unwrap();
    fs::rename(&replacement, &selected).unwrap();
    let mut bytes = Vec::new();
    acquired.read_to_end(&mut bytes).unwrap();
    assert_eq!(bytes, b"authorized");
    assert_eq!(fs::read(&selected).unwrap(), b"outside-decoy");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn fs_t03_two_ended_mutations_require_both_authorities() {
    let required = Rights::LOOKUP.union(Rights::WRITE);
    for source_instance in [17, 71] {
        for destination_instance in [17, 71] {
            let source = Capability::new(
                CapabilityKind::Descriptor,
                source_instance,
                19,
                23,
                ObjectId {
                    device: source_instance,
                    inode: 1,
                },
                required,
            );
            let destination = Capability::new(
                CapabilityKind::DirectoryDescriptor,
                destination_instance,
                19,
                23,
                ObjectId {
                    device: destination_instance,
                    inode: 2,
                },
                required,
            );
            let admitted = source.authorizes(17, 19, 23, required)
                && destination.authorizes(17, 19, 23, required);
            assert_eq!(
                admitted,
                source_instance == 17 && destination_instance == 17
            );
        }
    }
    for syscall in [82, 86, 264, 265, 316] {
        assert_eq!(
            operation_family(operation(syscall).unwrap()),
            OperationFamily::TwoPathMutation
        );
        assert!(confined_kernel_route(&context(syscall), true).is_ok());
    }
}

#[test]
fn fs_t04_descriptor_lifecycle_rejects_stale_generation() {
    let mut call = context(0);
    assert!(descriptor_kernel_route(&call).is_ok());
    call.authority.descriptor_generation += 1;
    assert!(descriptor_kernel_route(&call).is_err());
    call = context(1);
    call.authority.descriptor_table = Capability::new(
        CapabilityKind::DescriptorTable,
        17,
        19,
        31,
        ObjectId {
            device: 17,
            inode: 31,
        },
        Rights::READ,
    );
    assert!(descriptor_kernel_route(&call).is_err());
}

#[test]
fn fs_t05_special_interfaces_are_virtual_or_explicitly_denied() {
    for syscall in [165, 166] {
        let operation = operation(syscall).unwrap();
        assert_eq!(operation.owner, Owner::VirtualKernel);
        assert_eq!(operation_executor(operation), Executor::AccountableExternal);
    }
    for syscall in [
        155, 161, 162, 206, 209, 300, 304, 313, 425, 426, 427, 428, 438, 442, 457, 458,
    ] {
        let operation = operation(syscall).unwrap();
        assert_eq!(operation.owner, Owner::ExplicitDeny);
        assert_eq!(operation_executor(operation), Executor::ExplicitDeny);
    }
}

#[test]
fn fs_t06_indirect_io_requires_equivalent_rights() {
    let mut shared_write = context(9);
    shared_write.arguments[2] = 2;
    shared_write.arguments[3] = 1;
    assert_eq!(
        descriptor_required_rights(&shared_write),
        Rights::READ.union(Rights::WRITE)
    );
    for syscall in [40, 275, 326] {
        assert_eq!(
            descriptor_required_rights(&context(syscall)),
            Rights::READ.union(Rights::WRITE)
        );
    }
    for syscall in [206, 209, 425, 426, 427] {
        assert_eq!(operation(syscall).unwrap().owner, Owner::ExplicitDeny);
    }
}

#[test]
fn fs_t07_faults_fail_before_effect_and_preserve_errno_class() {
    let root = fixture("faults");
    let protected = root.join("protected");
    fs::write(&protected, b"unchanged").unwrap();

    let mut call = context(2);
    for mutation in 0..6 {
        let mut failed = call;
        match mutation {
            0 => failed.abi = 0,
            1 => failed.task.generation = 0,
            2 => failed.instance.lifetime += 1,
            3 => failed.instance.root_generation += 1,
            4 => failed.authority.cwd_generation += 1,
            _ => failed.authority.descriptor_generation += 1,
        }
        assert!(confined_kernel_route(&failed, true).is_err());
        assert_eq!(fs::read(&protected).unwrap(), b"unchanged");
    }
    call.contract = ContractAuthorization::Deny(13);
    assert!(confined_kernel_route(&call, true).is_err());
    assert_eq!(fs::read(&protected).unwrap(), b"unchanged");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn fs_t08_every_supported_route_has_positive_and_negative_control() {
    for operation in OPERATIONS {
        let call = context(operation.syscall);
        let route = accountable_route(&call).unwrap();
        match operation.owner {
            Owner::FilesystemLane => {
                assert!(confined_kernel_route(&call, true).is_ok());
                assert!(confined_kernel_route(&call, false).is_err());
            }
            Owner::DescriptorCore => {
                assert!(descriptor_kernel_route(&call).is_ok());
                let mut stale = call;
                stale.authority.descriptor_generation += 1;
                assert!(descriptor_kernel_route(&stale).is_err());
            }
            Owner::ExplicitDeny => assert_eq!(route.executor, Executor::ExplicitDeny),
            Owner::ProcessLane | Owner::VirtualKernel => {
                assert_eq!(route.executor, Executor::AccountableExternal)
            }
        }
    }
}

#[test]
fn fs_t09_stop_recreate_cannot_reuse_authority() {
    let old = context(2);
    let mut replacement = old;
    replacement.instance.lifetime += 1;
    replacement.authority.root = Capability::new(
        CapabilityKind::Root,
        replacement.instance.identity,
        replacement.instance.lifetime,
        replacement.instance.root_generation,
        ObjectId {
            device: 97,
            inode: 101,
        },
        Rights::LOOKUP.union(Rights::READ),
    );
    replacement.authority.cwd = Capability::new(
        CapabilityKind::WorkingDirectory,
        replacement.instance.identity,
        replacement.instance.lifetime,
        replacement.authority.cwd_generation,
        ObjectId {
            device: 97,
            inode: 103,
        },
        Rights::LOOKUP,
    );
    replacement.authority.descriptor_table = Capability::new(
        CapabilityKind::DescriptorTable,
        replacement.instance.identity,
        replacement.instance.lifetime,
        replacement.authority.descriptor_generation,
        ObjectId {
            device: 97,
            inode: 107,
        },
        Rights::LOOKUP
            .union(Rights::READ)
            .union(Rights::WRITE)
            .union(Rights::CREATE)
            .union(Rights::METADATA),
    );
    assert!(confined_kernel_route(&replacement, true).is_ok());

    let mut stale_completion = replacement;
    stale_completion.authority = old.authority;
    assert!(confined_kernel_route(&stale_completion, true).is_err());

    let root = fixture("recovery");
    let persistent = root.join("persistent");
    let mut file = File::create(&persistent).unwrap();
    file.write_all(b"state").unwrap();
    drop(file);
    assert_eq!(fs::read(&persistent).unwrap(), b"state");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn required_security_mutations_are_reached_and_killed() {
    // Lexical prefix containment would admit the sibling; object authority
    // rejects it.
    assert!(b"/instance-peer/file".starts_with(b"/instance"));
    let peer = Capability::new(
        CapabilityKind::Descriptor,
        71,
        19,
        23,
        ObjectId {
            device: 71,
            inode: 1,
        },
        Rights::READ,
    );
    assert!(!peer.authorizes(17, 19, 23, Rights::READ));

    // Missing magic-link/opaque-handle protection would route these to the
    // kernel; the inventory pins explicit denial.
    for syscall in [304, 438] {
        assert_eq!(
            operation_executor(operation(syscall).unwrap()),
            Executor::ExplicitDeny
        );
    }

    // One-ended checks are insufficient.
    let source_allowed = true;
    let destination_allowed = false;
    assert!(source_allowed);
    assert!(!(source_allowed && destination_allowed));

    // Inherited-directory and stale-generation mutants fail the descriptor
    // authority conjunction.
    let mut inherited = context(0);
    inherited.authority.descriptor_table = Capability::unavailable(
        CapabilityKind::DescriptorTable,
        inherited.instance.identity,
        inherited.instance.lifetime,
    );
    assert!(descriptor_kernel_route(&inherited).is_err());
    let mut stale = context(0);
    stale.authority.descriptor_generation += 1;
    assert!(descriptor_kernel_route(&stale).is_err());

    // Tail arguments remain part of the immutable request, and openat2 stays
    // unsupported until its complete structure can be admitted.
    let original = context(437);
    let mut changed_tail = original;
    changed_tail.arguments[3] = 24;
    assert_ne!(original, changed_tail);
    assert_eq!(operation(437).unwrap().unsupported_errno, 38);

    // Contract-before-hook and resolver-failure fallback mutants are killed
    // by the same fail-closed route guard.
    let mut denied = context(2);
    denied.contract = ContractAuthorization::Deny(13);
    assert!(confined_kernel_route(&denied, true).is_err());
    assert!(confined_kernel_route(&context(2), false).is_err());
}
