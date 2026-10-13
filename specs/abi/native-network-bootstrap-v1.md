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

Packet publication and consumption use an exact 32-byte little-endian request:

| Offset | Width | Field |
|---:|---:|---|
| 0 | 4 | size `32` |
| 4 | 2 | version `0` |
| 6 | 2 | reserved zero |
| 8 | 8 | userspace buffer address |
| 16 | 4 | buffer length or capacity, `1..1514` |
| 20 | 4 | reserved zero |
| 24 | 8 | expected sequence; zero for publication |

Operation 21 requires packet `WRITE`, copies the complete input into the
kernel-owned one-slot mailbox, and returns its nonzero monotonic sequence.
Operation 22 requires packet `READ`, requires an exact expected sequence, and
copies the packet before removing it. A future or stale sequence returns the
stale result without modifying userspace or consuming the packet. A failed
copy also retains the packet. Publication fails while the slot is occupied.

The current bootstrap proof publishes exactly the validated 42-byte
Ethernet/ARP reply as sequence one. The packet survives producer, NIC, VT-d,
and DMA retirement. A distinct ring-3 consumer has packet `READ` only, no
packet `WRITE`, and no network-device authority; it validates all bytes and
can consume the sequence only once.

## Limits

There is one global slot, one producer generation, and a maximum Ethernet
frame size of 1514 bytes. Publication uses a bounded copy rather than shared
memory. There is no blocking receive, backpressure notification, batching,
multi-consumer fanout, VLAN-sized frame support, or zero-copy lease. The
single-CPU call gate serializes access; SMP requires locking.
