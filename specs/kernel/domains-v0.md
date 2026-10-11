# Native Domain Metadata v0

**Status:** Bootstrap metadata only; no userspace execution yet.

The kernel constructs exactly one initial control-plane domain record in a
four-slot bootstrap table. A domain
identifier combines a fixed-table slot and generation so stale identifiers
cannot name a later occupant. The record has an explicit role and lifecycle
state and owns a domain-local capability table.

Initial control-plane authority is a finite list of boot grants. Each grant
becomes a root capability, after which the capability table's boot-authority
path is permanently sealed. The domain manager likewise refuses a second
control-plane construction. Handles are meaningful only in the owning domain's
capability namespace.

The current boot grants are temporary bring-up objects for host control and the
console. They are not yet sourced from a signed manifest. `Ready` means only
that kernel metadata and authority exist; no control-plane address space,
thread, executable image, syscall path, scheduling state, or resource budget
exists yet.
