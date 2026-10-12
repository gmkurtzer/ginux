# Inactive IOMMU Translation Hierarchy v0

**Status:** Constructed and verified in memory; never installed in hardware.

## Purpose

This contract is the first table-construction step between read-only VT-d
discovery and active DMA confinement. It proves that Ginux can encode a minimal
legacy-mode hierarchy for the exact Q35 virtio-blk source ID and one owned DMA
page without changing any IOMMU register.

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

The hierarchy remains allocated while the bounded kernel virtio transaction
uses the corresponding DMA lease. A second read-only register probe requires
the hardware translation-enable status and root-table register to remain zero,
proving that table construction did not publish the hierarchy.

After the device is reset and bus mastering is revoked, the six hierarchy
frames are scrubbed and their leases released. Only then is the separate DMA
page scrubbed and released.

## Deliberate limits

This hierarchy is logically immutable after construction but its ordinary
kernel direct-map aliases are not yet made read-only. It is not written to the
VT-d root-table register, no context or IOTLB invalidation is issued,
translation is not enabled, faults are not handled, and DMA is not confined.
The single identity-mapped page and fixed domain ID are a bounded bootstrap
profile, not a general IOVA allocator or multi-device domain model.
