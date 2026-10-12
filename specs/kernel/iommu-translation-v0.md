# IOMMU Translation Hierarchy v0

**Status:** Active one-device, one-page Q35 DMA confinement profile.

## Purpose

This contract confines both the bounded kernel-reference virtio-blk transaction
and each isolated native volume-service generation to one owned DMA page. It
encodes a minimal legacy-mode hierarchy for the exact Q35 source ID, installs
it with bounded register transitions, and restores the IOMMU to its disabled,
zero-root state before releasing any table or DMA frame.

## Table image

The hardware capability register must advertise the 48-bit adjusted guest
address width. The kernel allocates six independently tracked, zeroed, 4 KiB
frames owned by the kernel:

1. a 256-entry legacy root table;
2. a 256-entry legacy context table;
3. a PML4;
4. a page-directory-pointer table;
5. a page directory; and
6. a page table.

Only the discovered PCI bus root entry and device/function context entry are
present. The context uses domain ID 1, multi-level translation, and the 48-bit
address-width encoding. Four second-level indices form an identity translation
from the DMA page's device-visible address to the same owned physical page.
Intermediate and leaf entries grant read and write; no large-page entry is
used. Every other 64-bit word in all six frames must remain zero.

The encoder rejects unsupported address width, invalid device/function values,
unaligned or out-of-range physical pages, lost frame ownership, duplicate
expected entries, unexpected nonzero words, and any mismatch in the exact
hierarchy.

## Lifecycle

Before activation, a read-only probe requires translation disabled, a zero root
register, matching capabilities, and clear fault status. The kernel publishes
the verified root address, issues Set Root Table Pointer, waits for completion,
performs global context-cache and IOTLB invalidations, enables translation, and
waits for Translation Enable Status. Every wait has a fixed iteration bound;
an activation failure attempts to disable translation and clear/reload a zero
root before failing closed.

The QEMU virtio-blk function is configured with `iommu_platform=on`, and every
kernel and native driver negotiation requires `VIRTIO_F_ACCESS_PLATFORM`.
Without both conditions QEMU may use a bypass address space even while VT-d
translation is enabled.

Before the ordinary transaction, the kernel allocates a separate owned guard
frame, fills it with a sentinel, and deliberately leaves it absent from the
hierarchy. A read request retains its descriptor table, request header, status,
available ring, and used ring in the permitted page while its 512-byte payload
descriptor names the guard. QEMU currently publishes an ordinary successful
virtio completion even though VT-d drops the payload write, so queue status is
not accepted as confinement evidence. The kernel resets the device, revokes
bus mastering, and requires every guard byte to remain unchanged.

The capability-selected fault-record offset must describe exactly one bounded
128-bit record. Its pending bit, fault-information address, source ID, write
transaction type, no-PASID value, and write-permission reason must exactly
match the attempted access. The kernel clears the record through its
write-one-to-clear valid bit, requires fault status to clear, globally
invalidates the context cache and IOTLB, and requires the original root and
translation-enable state to remain active. A normal interrupt-driven read and
the persistent read/write/flush transaction must then succeed through the
permitted page, proving recovery after the rejected access.

After the device is reset and bus mastering is revoked, the kernel disables
translation and waits for acknowledgement, invalidates both caches, writes a
zero root, and reloads it. It then requires a disabled translation status,
zero root register, and clear fault status. Only afterward are the six
hierarchy frames scrubbed and released, followed by the guard and DMA pages.

For the isolated native volume service, translation ownership is registered
with the same domain resource record as its address space and device-teardown
hook. A fresh hierarchy is built from the exact generation-tagged DMA lease and
activated before PCI bus mastering is armed. Device waits fail closed if the
registered translation is no longer active.

Clean exit, control-ABI rejection, malformed-descriptor rejection, service
fault, notified-timeout cancellation, persistent recovery, and corrupt-media
rejection all converge on the registered teardown path. It must quiesce and
reset the device and revoke bus mastering while translation is still active.
Only then may address-space release disable translation, invalidate caches,
clear the root, and scrub the six table frames. The service DMA frame is
scrubbed afterward. Returned lifecycle evidence requires both table and DMA
leases to be absent from the allocator. Consecutive service generations must
use distinct generation-tagged table and DMA leases even when the allocator
reuses the same physical addresses.

## Deliberate limits

This hierarchy is logically immutable after construction but its ordinary
kernel direct-map aliases are not yet made read-only. Fault status is polled;
there is no fault interrupt or general fault-policy engine. The single
identity-mapped page, fixed domain ID, serialized reuse of one remapping unit,
QEMU-specific single fault record, and exact reason encoding are a bounded
bootstrap profile, not a general IOVA allocator, concurrent multi-device domain
model, or production hardware claim.
