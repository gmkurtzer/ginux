# Capability Table v0

**Status:** In-kernel primitive with boot-time invariant checks.

A capability names one kernel object and carries an explicit rights bitset.
Handles contain a fixed-table slot and generation. A handle resolves only when
both fields match a live entry, preventing a revoked handle from becoming valid
when its slot is reused.

The temporary call-gate encoding places the 16-bit slot in bits 0-15 and the
32-bit nonzero generation in bits 16-47; bits 48-63 must be zero. Decoding never
grants authority by itself—the resulting handle is resolved only in the calling
domain's table.

There are three authority-producing operations:

1. `authorize_boot` introduces a non-empty root capability from the immutable
   boot authority envelope. It is the sole parentless operation and must become
   inaccessible after initial domain construction. Sealing the table makes
   every later root-authority insertion fail permanently.
2. `derive` creates a capability for the same object with a non-empty, strict
   subset of its parent's rights. It cannot copy equal authority, add a right,
   or change the object.
3. `accept_delegated` inserts a root into a new domain only while that table is
   unsealed. It is not directly callable from userspace. The domain manager
   first resolves a live source capability in the delegating domain and limits
   the new root to that object and rights before sealing the destination table.

Every derived entry records its parent handle. Revoking an entry recursively
removes all descendants. Authorization checks require a live generation, the
same object, and every requested right.

The current 64-entry table is deliberately fixed and local. It does not yet
define general userspace-requested transfer, object lifetime, concurrency,
resource charging, or synchronization with in-flight operations. The boot test covers rejected
equal/expanded derivation, least-rights access, recursive revocation, slot
reuse, stale-generation rejection, one kernel-mediated IPC delegation, and
cross-domain handle substitution.
