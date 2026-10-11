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
   The destination retains the local root, generation-tagged parent domain,
   and source handle as delegation provenance. Provenance is attached to each
   delegated root rather than to the destination domain as a whole, so future
   roots from different sources cannot borrow one another's authority chain.

Every derived entry records its parent handle. Revoking an entry recursively
removes all descendants. Authorization checks require a live generation, the
same object, and every requested right. For delegated domain roots, each check
also walks the bounded parent-domain chain and revalidates the original object
and requested rights. Revoking or retiring any parent therefore invalidates the
child immediately even if the child's local table entry remains allocated.
Locally derived child handles first resolve to their delegated root and then
follow that root's specific cross-domain chain. An authorization chain longer
than the fixed domain table is rejected.

The current 64-entry table is deliberately fixed and local. It does not yet
define general userspace-requested transfer, object lifetime, concurrent table
mutation, or cancellation of in-flight operations. Charged call-gate dispatch
holds a bounded generation-specific operation reference, and retirement
requires that count to reach zero before removing the complete table. The boot
test exhaustively checks all parent, derived-child, and requested-rights
combinations in the four-bit rights lattice. A derivation succeeds exactly for
a nonempty strict subset, authorization succeeds exactly for a request subset
of the derived rights, and the same handle never authorizes a different object.
It also covers rejected
equal/expanded derivation, least-rights access, recursive revocation, slot
reuse, stale-generation rejection, one kernel-mediated IPC delegation, and
cross-domain handle substitution. A separate manager invariant leaves a child
handle locally live, revokes its control-plane source, and requires the full
authorization check to fail. The same invariant derives a read-only child from
the delegated root, verifies attenuation, and requires parent revocation to
invalidate both local handles.
