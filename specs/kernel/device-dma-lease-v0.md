# Isolated DMA Lease v0

**Status:** Compiled-service interrupt-driven volume I/O and fault cleanup are
verified in QEMU.

## Purpose

This contract binds one DMA frame to the exact native volume-service domain
generation. The service can use that frame for a bounded virtio split ring, but
cannot retain it or leave the device capable of accessing it after failure.

## Mapping and ownership

The native service address space owns thirteen base frames: four page-table
frames, six static-PIE image frames, two stack frames, and one read-only
bootstrap frame. An additional zeroed 4 KiB lease is mapped RW/NX at
`USER_TEST_ADDRESS + 11 * 4096`. The virtio common and notification pages
occupy the following two uncached leaves. Absent pages on both sides of the
two-page stack remain guards.

Root validation requires exact page permissions and non-aliasing across every
owned frame. The DMA physical address must be nonzero, below the development
allocator's 4 GiB ceiling, distinct from all base frames, and owned by the
same live domain generation. Every CR3 activation revalidates that ownership.
The MMIO frames are external, above the allocator ceiling, and are never
released as ordinary memory.

The kernel writes the DMA virtual and physical addresses into a read-only
bootstrap record. This is an internal development-profile contract, not yet a
stable allocation ABI.

## Ring-3 use

The service writes the descriptor table at offset `0x000`, available ring at
`0x100`, used ring at `0x200`, request header at `0x400`, sector buffer at
`0x600`, and status byte at `0x900`. It programs exactly those physical
offsets into queue zero, binds MSI-X entry zero, enables the queue, and sets
`DRIVER_OK` only after the kernel has armed bus mastering.

One descriptor head represents read, write, and flush requests. Available and
used indices advance monotonically and wrap through an eight-entry ring. The
fresh-volume path completes ten serialized operations; recovery completes two.
The kernel accepts teardown only when the final used index equals the available
index, the final entry reports descriptor head zero with the expected length,
the final status is successful, and the last request is a sector-five read.

## Teardown

The service normally reports a bounded lifecycle completion after transaction
success. Its registered hook masks and disables MSI-X, validates the active
queue-to-lease binding, revokes PCI bus mastering, resets the device, and
retires the temporary interrupt gate before inspecting or releasing DMA
memory. Address-space release fails closed unless this quiescence record
exists. Fault, cancellation, and pre-activation rejection use the same hook
without trusting a completed queue.

Before bus mastering is armed, the kernel installs a fresh six-frame VT-d
hierarchy containing only this lease. After quiescence, it disables translation,
invalidates the context and IOTLB caches, clears the hardware root, and scrubs
the hierarchy. It then validates stable DMA state, restores its own CPU root,
zeros the DMA and all base frames, releases their exact generation-tagged
leases, revokes object 7, and retires the domain. The gate reads the former DMA
page through the supervisor direct map and requires it to be entirely zero,
while the allocator must report no frames owned by the retired generation and
no live IOMMU table leases. Replacement generations must receive distinct
generation-tagged DMA and table leases.

## Deliberate limits

- Requests are serialized through one descriptor head and one DMA page.
- Page-granular common and notification mappings expose unrelated bytes within
  those pages.
- Interrupt routing remains kernel-owned; the service receives only a
  generation-bound wait operation.
- The IOMMU profile maps one identity-addressed page for one serialized device;
  it is not yet a general IOVA allocator or concurrent multi-device domain.
- The fixed layout and physical-address bootstrap field are not a stable
  general driver ABI.
