# Virtio Network Queue MMIO v0

**Status:** Exact inactive mappings are verified on QEMU Q35.

## Purpose

This contract maps the final transport surfaces needed to activate the first
virtio-net queues while granting no device authority yet.

## Notification mapping

The complete discovered notification capability is mapped into one distinct
higher-half slot. It is supervisor-only, writable, uncached, and
non-executable. The receive and transmit notification addresses previously
derived from queue offsets must each resolve to an aligned 16-bit register in
this mapping and must remain distinct.

Merely mapping the page does not notify a queue. No notification register is
read or written in this milestone.

## MSI-X mapping

The exact span covering the network function's four-entry MSI-X table and
pending-bit array is mapped in a separate supervisor-only, writable, uncached,
non-executable slot. Both discovered subranges must resolve completely inside
that mapping. Its low identity alias is removed before CR3 activation.

The PCI MSI-X enable bit remains clear. No table entry is programmed, no
vector is installed, and the function remains reset with PCI I/O decoding,
memory decoding, and bus mastering disabled.

## Sealed table invariant

The transitional MMIO page table now contains exactly seven mutually disjoint
spans:

1. virtio-blk transport;
2. virtio-blk MSI-X;
3. VT-d registers;
4. virtio-net common configuration;
5. virtio-net device configuration;
6. virtio-net notification capability; and
7. virtio-net MSI-X state.

Every unused entry remains non-present. The device-configuration span is the
only read-only mapping; the other six are writable. Every mapping is
supervisor-only, cache-disabled, and NX.

## Deliberate limits

- Mapping is not activation: queues, VT-d translation, MSI-X, and bus
  mastering remain disabled.
- The next milestone must activate VT-d before writing queue addresses and
  must enable bus mastering only after all queue state is validated.
- Dynamic MMIO grant/revocation replaces these static bootstrap mappings in
  the isolated native-service path.
