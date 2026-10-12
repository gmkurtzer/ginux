# Virtio Network Transmit v0

**Status:** One confined transmit completion is verified on QEMU Q35.

## Purpose

This contract performs the first packet movement through the native
virtio-net path. It proves that one bounded transmit descriptor can be
published, notified, completed through MSI-X, and retired while the device can
DMA only to one generation-owned page.

## Transaction

The kernel requires the function to begin reset with all PCI command bits and
MSI-X disabled. The exact VT-d translation for source `00:03.0` and the one
network DMA page is active before device configuration begins.

Queue 1 is configured with eight entries and MSI-X table entry 1. Descriptor
zero is device-readable and covers exactly 29 bytes in the TX buffer: a
zeroed 10-byte virtio-net header followed by this fixed 19-byte Ethernet probe:

- broadcast destination `ff:ff:ff:ff:ff:ff`;
- pinned source `52:54:00:12:34:56`;
- experimental EtherType `0x88b5`; and
- payload `GINUX`.

Distinct canaries bound the byte immediately before and after the descriptor
range. The available ring publishes only descriptor zero. After the queue is
enabled and `DRIVER_OK` is verified, PCI bus mastering is enabled last. The
kernel installs local-APIC vector 35 while MSI-X is function-masked, unmasks
only TX entry 1, enables MSI-X, and finally notifies queue 1.

## Completion and teardown

Success requires exactly one interrupt, used index one, used descriptor ID
zero, no device failure or reset-needed status, unchanged header and Ethernet
bytes, and both intact canaries.

The kernel then function-masks MSI-X, masks TX entry 1, disables MSI-X, clears
bus mastering, resets the device, revokes PCI decoding, verifies vector 35 is
idle, and removes its temporary IDT gate. Only afterward may it disable and
retire VT-d, scrub the translation tables, and scrub and release the DMA page.
Every failure after gate installation follows the same teardown path.

## Deliberate limits

- A used-ring completion proves that the emulated device consumed the
  descriptor. It does not prove that a peer received the frame.
- This bootstrap probe is intentionally shorter than a normal padded Ethernet
  frame and is not a general Ethernet interface.
- No receive buffer, protocol parser, retry, timeout injection, or concurrent
  request is implemented.
- The interrupt route and queue remain kernel-owned. Delegation to a measured,
  restartable native network service is a later M6 milestone.
