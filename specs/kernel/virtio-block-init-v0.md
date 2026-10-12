# Virtio Block Initialization v0

**Status:** Bounded M6 initialization profile.

## Purpose

This contract advances the discovered modern virtio block function to a
verified feature-negotiation boundary without granting DMA authority or
activating a queue.

## MMIO mapping

Before the kernel seals its static page tables, it maps the smallest contiguous
span covering the validated common, notification, ISR, and device regions into
a fixed supervisor-only higher-half window. Initialization itself accesses
only the common region. The mapping:
The mapping:

- accepts a nonempty range above the 4 GiB transitional direct map;
- rounds only to the enclosing 4 KiB pages;
- is limited to 512 pages;
- is writable, non-executable, supervisor-only, and cache-disabled;
- uses a direct-map hierarchy distinct from the low identity hierarchy; and
- is inherited by domain roots only as part of their unchanged supervisor
  kernel entry.

The kernel reads the complete hierarchy back before MMIO access and again
after the ring-3 workload. All page-table pages, including the new direct-map
and MMIO tables, remain read-only through the low identity map.

## Initialization sequence

The bootstrap processor performs the following bounded sequence while no
service domain can concurrently access PCI configuration or the device:

1. Set PCI memory-space decode and clear PCI bus mastering.
2. Read back the command register and require bus mastering to be clear.
3. Write device status zero and poll at most 100,000 times for reset.
4. Set `ACKNOWLEDGE`, then `DRIVER`.
5. Read both 32-bit device-feature words.
6. Require and select only `VIRTIO_F_VERSION_1` and `VIRTIO_BLK_F_FLUSH`.
7. Publish both driver-feature words and set `FEATURES_OK`.
8. Require the device to retain `FEATURES_OK` and expose at least one queue.
9. Reset the device again and require status zero.
10. Re-read the PCI command register and require bus mastering to remain clear.

Copied feature-set tests reject a device missing either required feature and
prove that no unrequested offered feature enters the accepted set.

## Authority and limitations

No virtqueue address is written, `DRIVER_OK` is never set, PCI bus mastering
remains disabled, and no interrupt or DMA operation is possible. The device is
returned to reset before general kernel initialization continues. No sector is
read or written and this contract adds no persistence claim.

The separate [Virtio Block Read v0](virtio-block-read-v0.md) contract allocates
generation-owned queue memory, validates queue geometry, and proves one bounded
read request under the bounded
[IOMMU Translation Hierarchy v0](iommu-translation-v0.md).
