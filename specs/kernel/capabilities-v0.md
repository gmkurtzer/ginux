# Capability Table v0

**Status:** In-kernel primitive with boot-time invariant checks.

A capability names one kernel object and carries an explicit rights bitset.
Handles contain a fixed-table slot and generation. A handle resolves only when
both fields match a live entry, preventing a revoked handle from becoming valid
when its slot is reused.

There are two authority-producing operations:

1. `authorize_boot` introduces a non-empty root capability from the immutable
   boot authority envelope. It is the sole parentless operation and must become
   inaccessible after initial domain construction. Sealing the table makes
   every later root-authority insertion fail permanently.
2. `derive` creates a capability for the same object with a non-empty, strict
   subset of its parent's rights. It cannot copy equal authority, add a right,
   or change the object.

Every derived entry records its parent handle. Revoking an entry recursively
removes all descendants. Authorization checks require a live generation, the
same object, and every requested right.

The current 64-entry table is deliberately fixed and local. It does not yet
define object lifetime, cross-domain transfer, concurrency, resource charging,
or synchronization with in-flight operations. The boot test covers rejected
equal/expanded derivation, least-rights access, recursive revocation, slot
reuse, and stale-generation rejection.
