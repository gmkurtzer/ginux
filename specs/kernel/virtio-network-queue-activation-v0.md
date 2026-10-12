# Virtio Network Empty Queue Activation v0

**Status:** Confined activation and teardown are verified on QEMU Q35.

## Purpose

This contract crosses the first virtio-net authority boundary without moving a
packet. It proves that empty receive and transmit queues can become live only
after their DMA confinement exists, then be quiesced in the reverse order.

## Activation order

1. Allocate and validate the zeroed one-page DMA layout.
2. Build and activate the exact VT-d hierarchy for source `00:03.0`.
3. Enable PCI memory decoding while keeping bus mastering off.
4. Negotiate the exact required features again.
5. Configure queue 0 and queue 1 with eight entries and exact descriptor,
   available-ring, and used-ring addresses inside the permitted page.
6. Read back both queue configurations and enable bits.
7. Set and verify `DRIVER_OK`.
8. Enable PCI bus mastering last.

Both available indices remain zero. No descriptor is published, notification
register is touched, MSI-X entry is programmed, or packet is transferred. The
same ordering is now independently exercised by the measured native network
service: userspace negotiates and binds the queues, while a capability call
keeps final validation and bus-master authorization in the kernel.
An internal failed-return injection after successful authorization also proves
that an actual ring-3 driver fault quiesces the live device before translation
and memory retirement; no test-only bit is exposed in the bootstrap ABI.

## Teardown order

The kernel first clears and verifies bus mastering while memory decoding still
permits a device reset. It then resets the virtio function, revokes all PCI
decoding, disables and clears VT-d translation, scrubs the six table frames,
and finally scrubs and releases the DMA page. No lease may remain live.

## Deliberate limits

- This proof does not publish an RX buffer or TX descriptor.
- Interrupt programming, notification, completion validation, packet parsing,
  remain subsequent milestones.
- Queue depth eight is the bootstrap profile; the device-advertised maximum is
  retained for later tuning.
