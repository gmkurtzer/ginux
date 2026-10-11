# Native Domain Metadata v0

**Status:** Sealed control-plane and delegated service domains executing at ring 3.

The kernel constructs one initial control-plane domain and one isolated service
domain in a four-slot bootstrap table retained for the kernel lifetime. A domain
identifier combines a fixed-table slot and generation so stale identifiers
cannot name a later occupant. The record has an explicit role and lifecycle
state and owns a domain-local capability table.

Initial control-plane authority is a finite list of boot grants. Each grant
becomes a root capability, after which the capability table's boot-authority
path is permanently sealed. The domain manager likewise refuses a second
control-plane construction. Later kernel operations query this same published
manager and sealed capability table; they cannot reconstruct authority from the
manifest. Handles are meaningful only in the owning domain's capability
namespace.

The current manifest grants the control plane host control, console, and one
bounded IPC channel. The manager validates that IPC root and delegates only its
read/write authority into a newly sealed service-domain table. The service gets
no host-control or console handle. Every call-gate authorization resolves the
opaque handle in the current scheduler domain, never in a global or
control-plane-default namespace. The boot gate rejects using the service handle
in the control domain and the control IPC handle in the service domain.

The domains own distinct W^X address spaces and scheduler contexts. The service
domain metadata currently remains after its sole thread faults; domain teardown,
capability revocation on death, restart, and reusable service-domain creation
remain undefined.
