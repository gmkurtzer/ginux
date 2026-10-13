# Native Network Receive Replenishment v0

**Status:** One descriptor is safely reused for two interrupt-completed ARP
exchanges on QEMU Q35.

## Contract

Bootstrap feature `RX_REUSE` requires `ARP_PROBE` and every feature below it.
It extends the one-exchange receive contract without changing the bootstrap
record layout or operation-20 calling convention.

The first exchange uses TX request generation one and RX generation two. Before
returning from the second wait, the kernel independently validates the complete
first request, reply, rings, descriptor, and canaries. The service then poisons
the parsed portion of its receive buffer, republishes descriptor ID zero in RX
available slot one, republishes the transmit descriptor in TX available slot
one, advances both available indices from one to two, and notifies RX before TX.
It does not reset the device, queue, used rings, or interrupt routes.

The second transmit uses request generation three and the second receive uses
generation four. The kernel admits only the next exact generation and the
command implied by its parity: odd generations are TX and even generations are
RX. Stale, duplicate, skipped, or wrong-direction requests fail without
blocking. Each accepted completion must contain exactly one interrupt before
the kernel advances the generation.

The service requires the second TX and RX used indices to reach two, both
second used elements to name descriptor zero, and the poisoned receive bytes to
be replaced by another valid one-buffer gateway ARP reply. Clean exit is
accepted only after the kernel independently verifies both available entries,
both used entries and lengths, final packet identities, unchanged transmit
payload, unused zero descriptors, and intact boundary canaries.

Teardown then masks both MSI-X entries, revokes bus mastering, resets the NIC,
retires vectors 35 and 34, disables and scrubs VT-d, and scrubs the DMA page.

## Deliberate limits

This is bounded replenishment, not an unbounded packet loop. It reuses one
descriptor exactly once and serializes four waits. It does not yet support ring
wraparound, multiple outstanding descriptors, batching, arbitrary packets,
backpressure, packet IPC, or a container-facing interface.
