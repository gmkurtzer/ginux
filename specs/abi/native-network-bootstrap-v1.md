# Native Network Service Bootstrap ABI v1

**Status:** Additive packet-handoff extension verified with the measured
ring-3 network service.

Version 1 preserves the first 72 bytes of bootstrap v0 and replaces the v0
reserved word with two words. Offset 72 is an opaque packet-producer
capability; offset 80 remains reserved and must be zero. The record is exactly
88 bytes, uses version 1, and requires feature bit six `PACKET_HANDOFF` in
addition to all v0 features through `RX_REUSE`.
The service also requires control discovery feature bit 11 `PACKET_HANDOFF`;
v0 services do not require that additive feature.

Bootstrap v0 remains frozen at 80 bytes and rejects bit six. A service selects
the layout from the version and size header before accessing fields beyond the
common prefix. The v1 packet capability is independent of the network-device
capability: the measured producer receives `WRITE` over packet object 8 and
cannot use it to consume frames or control the NIC.

## Packet request v0

Packet publication, consumption, and availability waits use an exact 32-byte
little-endian request:

| Offset | Width | Field |
|---:|---:|---|
| 0 | 4 | size `32` |
| 4 | 2 | version `0` |
| 6 | 2 | reserved zero |
| 8 | 8 | userspace buffer address |
| 16 | 4 | buffer length or capacity, `1..1514`; zero for wait |
| 20 | 4 | reserved zero |
| 24 | 8 | expected sequence; zero for publication |

Operation 21 requires packet `WRITE`, copies the complete input into the
kernel-owned four-entry FIFO, and returns its nonzero monotonic sequence.
Operation 22 requires packet `READ`, requires an exact expected sequence, and
copies the packet before removing it. A future or stale sequence returns the
stale result without modifying userspace or consuming the packet. A failed
copy also retains the packet. Publication returns busy while all four entries
are occupied. Producer retirement does not discard queued entries, and a fresh
producer generation may bind while older entries await consumption.

Operation 23 requires packet `READ`. With a zero buffer address and capacity,
it returns the expected sequence immediately when that packet is at the FIFO
head, blocks when an empty FIFO's exact next sequence is requested, and returns
the stale result for any future or consumed sequence. A completed publication
wakes the sole waiter for its exact sequence; the subsequent operation 22 is
still required to copy and remove the packet. See
[Native Network Packet Wait v0](../services/network-packet-wait-v0.md).

The current bootstrap proof publishes exactly the validated 42-byte
Ethernet/ARP reply as sequence one. The packet survives producer, NIC, VT-d,
and DMA retirement. A distinct ring-3 consumer has packet `READ` only, no
packet `WRITE`, and no network-device authority; it validates all bytes and
can consume the sequence only once.

Optional feature bit seven `PACKET_FAULT` requires bit six and is accepted only
with the otherwise complete v1 feature set. It is a test-only request for the
measured producer to execute its ordinary fatal path immediately after a
successful publication. The copied packet remains sequence two while fault
containment resets the live NIC, retires both interrupt routes and VT-d, scrubs
DMA, and revokes the producer's capabilities. A fresh read-only consumer then
receives the exact packet. No public operation can inject this feature into an
already running generation.

Optional feature bit eight `PACKET_BATCH` requires bit six and is accepted
only with the otherwise complete v1 feature set. It is a test-only request for
the measured producer to publish both validated 42-byte replies before clean
retirement. They receive sequences three and four. Two distinct read-only
ring-3 consumers drain them in FIFO order, and each consumer proves that the
already-consumed sequence is stale while a later packet remains. The kernel
also fills the queue, rejects a fifth publication as busy, drains through
wraparound, and preserves monotonic sequence allocation.

## Limits

There is one global four-entry FIFO, one bound producer generation, and a
maximum Ethernet frame size of 1514 bytes. Publication uses a bounded copy
rather than shared memory. There is no wait timeout or cancellation,
backpressure notification, production batching interface, multi-consumer
fanout, VLAN-sized frame support, or zero-copy lease. The single-CPU call gate
serializes access; SMP requires locking.
