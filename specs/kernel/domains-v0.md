# Native Domain Metadata v0

**Status:** Sealed bootstrap control-plane domain executing at ring 3.

The kernel constructs exactly one initial control-plane domain record in a
four-slot bootstrap table retained for the kernel lifetime. A domain
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

The current grants are signed manifest records for host control, console, and
one bounded IPC channel. The control-plane domain owns the W^X address space
and two cooperatively scheduled proof threads that exercise those opaque
handles through the call gate. Domain resource budgets, teardown, restart, and
creation of subordinate service domains remain undefined.
