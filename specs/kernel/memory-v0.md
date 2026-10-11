# Initial Physical-Memory Classification v0

**Status:** Experimental reserving/recycling allocator enabled below 4 GiB.

After `ExitBootServices()`, the kernel walks the final UEFI memory map using the descriptor stride supplied by firmware. It does not assume that the stride equals the size of the descriptor prefix understood by the kernel.

The initial classification is deliberately conservative:

| UEFI memory type | Initial classification |
|---|---|
| `EfiConventionalMemory` | Available for a future physical-frame allocator |
| `EfiBootServicesCode`, `EfiBootServicesData` | Reclaimable, but not allocated yet |
| `EfiLoaderCode`, `EfiLoaderData` | Reserved because they may contain the unified image, boot information, memory map, or kernel stack |
| All other types | Reserved until explicitly modeled |

The kernel validates that:

- The map pointer and map length are nonzero.
- The descriptor stride is large enough for the required UEFI descriptor prefix.
- The total map length is an exact multiple of the supplied stride.
- Page totals use saturating arithmetic while parsing untrusted firmware data.

The initial allocator sorts and merges conventional ranges, rejects range
arithmetic overflow, excludes the first MiB, and limits allocations to the
currently mapped physical ceiling. Multi-frame reservations stage allocator
state and commit only when every requested frame is available. Failed or
zero-length reservations leave range cursors and accounting unchanged.
Address-space construction reserves its complete five- or seven-frame budget
before writing any page table.

Atomic release rejects zero-length, misaligned, duplicate, already-free, or
over-capacity batches. A bounded live-allocation ledger additionally requires
every submitted generation-tagged frame lease to be currently allocated; a
caller cannot turn an aligned but never-owned address into reusable memory.
Reallocation increments the lease generation, so an old owner cannot release
a newer allocation that reused the same address. Every live lease also records
either the kernel or an exact generation-tagged domain owner. Release requires
both the current lease and current owner; knowing a valid lease from another
domain is insufficient. Validation and ledger removal are staged atomically
before accepted frames enter the bounded
128-frame reuse pool. Completed proof address spaces are zeroed and released
only after the kernel CR3 is active again. The QEMU gate verifies rollback,
forged-release rejection, duplicate-release rejection, stale-generation
rejection after address reuse, and reuse across discontiguous ranges, then
reserves, writes, clears, and releases two real frames. Boot-service-memory
reclamation is not yet supported.

The allocator initializes exactly once and remains live for the kernel lifetime.
Reinitialization is rejected so no subsystem can reconstruct the UEFI inventory
and allocate frames that already have owners. Access is serialized only by the
single-processor, interrupts-disabled bootstrap state; locking or per-CPU
allocation is required before SMP startup.
Frame leases are kernel-internal values containing a physical address and
generation; the allocator separately retains their owners. A bounded transfer
operation changes every live lease owned by one non-kernel domain to another
non-kernel domain or changes nothing. The service restart path uses it to move
all seven retained address-space leases from the failed generation to its
replacement. Its local address-space record adopts the new owner only after
verifying all seven ledger entries, and final release requires that owner. The
live ledger is currently limited to 128 simultaneous allocations and fails
closed at capacity. Each non-kernel owner is additionally limited to 16 live
frames in this bootstrap revision. Multi-frame reservation stages the complete
operation, so exceeding the owner budget consumes neither frames nor budget.
Whole-owner transfer likewise checks the destination's existing count before
changing any entry. The invariant test fills one owner, rejects its next frame,
rejects transfer into an owner that would exceed the limit, and then completes
the same transfer into an empty owner. The fixed number is a bring-up policy,
not yet a configurable quota hierarchy; partial resource transfer is also
undefined.

Loader regions containing the unified image, handoff map, page tables, and kernel stacks are never admitted because only `EfiConventionalMemory` ranges enter the allocator.
