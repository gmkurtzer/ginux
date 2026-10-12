# Virtio Network Initialization v0

**Status:** Bounded no-DMA initialization is verified on QEMU Q35.

## Purpose

This contract advances the discovered virtio-net function far enough to prove
its modern transport, platform-DMA requirement, stable MAC feature, and queue
inventory. It must not configure a queue, enable bus mastering, delegate DMA,
or send or receive a packet.

## Mapping

After copied PCI configuration validation and firmware-authority revocation,
the kernel maps the 4 KiB common-configuration region and the separate 4 KiB
device-configuration region into its bounded higher-half MMIO window. Both are
supervisor-only, uncached, and non-executable; the common page is writable and
the device page is read-only. Their physical ranges and virtual slots must be
disjoint from each other and from the virtio-blk, block MSI-X, and VT-d
mappings, and the complete MMIO page table must contain no other present entry.
No cacheable identity alias may remain.

The notification, ISR, MSI-X table, and pending-bit regions are not mapped by
this milestone.

## Initialization

The PCI function must begin with I/O decoding, MMIO decoding, and bus mastering
disabled. The kernel temporarily enables MMIO decoding only, resets the device,
sets `ACKNOWLEDGE` and `DRIVER`, and reads both 32-bit feature words.

The accepted bitmap is exactly:

- `VIRTIO_F_VERSION_1` (bit 32);
- `VIRTIO_F_ACCESS_PLATFORM` (bit 33); and
- `VIRTIO_NET_F_MAC` (bit 5).

Missing any required bit fails closed. After writing both driver-feature words,
the kernel sets and reads back `FEATURES_OK`. The common configuration must
report at least two queues for receive and transmit; the deterministic QEMU
profile currently reports three.

Because `VIRTIO_NET_F_MAC` is accepted, the kernel reads the six-byte MAC from
the device-configuration page between matching common-configuration generation
reads. It retries a changing snapshot and rejects the zero, broadcast, and
multicast addresses. The QEMU profile pins `52:54:00:12:34:56` so the automated
gate verifies the exact value on every boot.

No queue is selected or configured and `DRIVER_OK` is never set. The kernel
then resets the device again, clears all PCI decoding and bus-master bits, and
requires both zero device status and zero authority bits.

## Rejection coverage

Boot-time checks remove each required feature independently and require
selection failure. A zero feature set is also rejected. The accepted bitmap
must contain no optional feature beyond the three listed above.

## Deliberate limits

- Queue geometry, notification offsets, MSI-X routing, DMA rings, VT-d tables,
  packet validation, and service delegation remain later M6 milestones.
- This is a serialized bootstrap proof for the deterministic Q35 device, not a
  general virtio transport implementation.
