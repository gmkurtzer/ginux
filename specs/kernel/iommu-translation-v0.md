# IOMMU Translation Hierarchy v0

**Status:** Active one-device, one-page Q35 DMA confinement profile.

## Purpose

This contract confines the bounded kernel-reference virtio-blk transaction to
one owned DMA page. It encodes a minimal legacy-mode hierarchy for the exact
Q35 source ID, installs it with bounded register transitions, and restores the
IOMMU to its disabled, zero-root state before releasing any table frame.

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

The hierarchy remains active while the bounded kernel virtio read/write/flush
transaction uses the corresponding DMA lease. The hardware root must match the
owned root frame and fault status must remain clear.

After the device is reset and bus mastering is revoked, the kernel disables
translation and waits for acknowledgement, invalidates both caches, writes a
zero root, and reloads it. It then requires a disabled translation status,
zero root register, and clear fault status. Only afterward are the six
hierarchy frames scrubbed and released, followed by the separate DMA page.

## Deliberate limits

This hierarchy is logically immutable after construction but its ordinary
kernel direct-map aliases are not yet made read-only. Fault status is polled at
lifecycle boundaries; there is no fault interrupt, recorded-fault diagnosis,
or deliberate unauthorized-DMA injection yet. The isolated native volume
service is not yet attached to this domain. The single identity-mapped page and
fixed domain ID are a bounded bootstrap profile, not a general IOVA allocator,
multi-device domain model, or production hardware claim.
