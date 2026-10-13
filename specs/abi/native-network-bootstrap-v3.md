# Native Network Service Bootstrap ABI v3

**Status:** Additive reliable-egress extension verified across a real network
driver fault and replacement generation under QEMU/TCG.

Version 3 preserves the complete 96-byte v2 layout and gives its extension a
two-phase delivery contract. Its exact base feature set is v2 plus
`RELIABLE_EGRESS`; optional test-only `EGRESS_FAULT` requests the measured
driver's ordinary fatal path immediately after a successful claim. All v2
address, capability, sequence, MAC, and reserved-zero constraints remain.

Operations 26 and 27 use the existing 32-byte packet request:

- `EGRESS_CLAIM` requires object 9 `READ`, an output address and capacity, and
  the exact head sequence. It copies the packet and binds a single claim to the
  caller generation without removing the packet.
- `EGRESS_ACK` requires the same handle and exact sequence with zero address
  and capacity. The kernel accepts it only after that caller's TX request one
  has completed and the exact virtio descriptor, reply bytes, used entry, and
  guards validate. Success removes and scrubs the packet.

Another claim, destructive consume, wrong-generation acknowledgement,
premature acknowledgement, and replay fail without changing queue state. A
domain's clean exit, control-plane stop, or fault releases its claim but does
not remove the packet. The replacement must receive a fresh device capability,
DMA page, VT-d hierarchy, interrupt route, and egress handle, but claims the
same packet sequence.

Bootstrap v2 remains the frozen destructive-consume proof. Version 3 is the
required contract for restart-safe device delivery.

## Limits

There is one global in-flight egress claim. The proof handles one fixed ICMPv4
reply and one TX descriptor. It does not define claim deadlines, concurrent
consumers, egress waiting/backpressure, batching, or peer receipt.
