# Initial Physical-Memory Classification v0

**Status:** Experimental reserving/recycling allocator enabled below 4 GiB,
including reclaimable boot-service memory after `ExitBootServices()`.

After `ExitBootServices()`, the kernel walks the final UEFI memory map using the descriptor stride supplied by firmware. It does not assume that the stride equals the size of the descriptor prefix understood by the kernel.

The initial classification is deliberately conservative:

| UEFI memory type | Initial classification |
|---|---|
| `EfiConventionalMemory` | Available to the physical-frame allocator |
| `EfiBootServicesCode`, `EfiBootServicesData` | Available to the physical-frame allocator after `ExitBootServices()` |
| `EfiLoaderCode`, `EfiLoaderData` | Reserved because they may contain the unified image, boot information, memory map, or kernel stack |
| All other types | Reserved until explicitly modeled |

The kernel validates that:

- The map pointer and map length are nonzero.
- The descriptor stride is large enough for the required UEFI descriptor prefix.
- The total map length is an exact multiple of the supplied stride.
- Page totals use saturating arithmetic while parsing untrusted firmware data.

The initial allocator admits conventional and boot-service ranges, sorts and
merges them, rejects range arithmetic overflow, excludes the first MiB, and
limits allocations to the currently mapped physical ceiling. Its bounded
pre-merge inventory accepts at most 128 eligible descriptors and fails closed
if firmware supplies more. It records the exact number of admitted
boot-service frames after low-memory and ceiling clipping. The QEMU gate
requires that count to be nonzero on its reference firmware. Multi-frame reservations stage allocator
state and commit only when every requested frame is available. Failed or
zero-length reservations leave range cursors and accounting unchanged.
The kernel may also request a fixed-size physically contiguous batch. That
operation selects one fresh admitted range large enough for the complete batch,
or first reuses one complete contiguous run already in the recycle pool. It
does not assemble unrelated recycled fragments. The operation records every
lease atomically. A
failed contiguous request leaves all ranges and live accounting unchanged;
successful reservations retain the ordinary exact-owner and release rules.
The invariant proof skips an undersized first range, obtains three consecutive
frames from the next range, then returns to reserve the untouched two-frame
range.
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
128-frame reuse pool. The global allocation boundary clears every byte of every
fresh or recycled frame before returning its lease, independent of caller
behavior. Completed proof address spaces are additionally zeroed before release
after the kernel CR3 is active again, reducing how long retired contents remain
resident. The QEMU gate verifies rollback,
forged-release rejection, duplicate-release rejection, stale-generation
rejection after address reuse, and reuse across discontiguous ranges, then
reserves two real frames, verifies their complete zero state, dirties both ends
of both pages, releases and reallocates them, and verifies all bytes are zero
before the new leases become usable. Boot-service frames use this same
zero-before-allocation boundary, so stale firmware contents are not exposed to
their first kernel or domain owner. Frame contents are zeroed and the recycle
proof is performed through a bounds-checked supervisor-only higher-half direct
map; lease values and page-table entries remain physical addresses.

The exec commit path can submit a bounded runtime-sized slice of exact leases
for scrub-and-release. It validates the complete batch, including direct-map
coverage, ownership, generation, uniqueness, and recycle capacity, before
zeroing any payload. Under the current serialized allocator contract, the
validated batch is then zeroed and released as one commit. A stale lease or a
second release attempt fails before memory is changed.

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
CR3 activation boundary independently resolves the complete validated page-table
hierarchy to seven physical addresses and requires each address to be live under
the scheduler's current domain generation. Ownership transfer therefore must
complete before a replacement can execute. The
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

The allocator exposes an internal exact-owner live-frame count for lifecycle
auditing. After the ring-3 proof transfers the failed service's seven frames to
its replacement and releases both address spaces, the boot gate requires zero
live frames for the control domain, the failed service generation, and the
replacement generation. A retired generation therefore cannot pass the proof
while silently retaining an allocator lease. This is a bounded bootstrap audit,
not yet a general resource destructor for every future kernel object. Service
retirement additionally places the exact generation in the domain manager's
single retained-frame quarantine until the seven leases are adopted by a
replacement or released. Allocator ownership itself never changes to an
anonymous quarantine identity, preserving generation checks throughout the
handoff.

Loader regions remain excluded even after boot-service reclamation. They may
contain the unified image, handoff map, transitional page tables, or kernel
stacks and require explicit lifetime tracking before they can be reclaimed.
