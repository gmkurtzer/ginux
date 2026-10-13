# Native Protocol Service Bootstrap ABI v3

**Status:** Shared Rust and C layouts are exercised by an ARP-then-IPv4
measured protocol-service generation under QEMU/TCG.

Version 3 retains the 56-byte v1/v2 layout. It requires the exact
packet-consumer, Ethernet/ARP, wait-loop, and IPv4-dispatch feature set, with
`packet_count == 2`. The first sequence contains an ARP reply that establishes
the configured peer MAC. The second contains an Ethernet IPv4 frame from that
learned MAC and configured peer IPv4 to the configured local endpoint.

The record remains packet-read authority only. It does not grant publication,
device, DMA, MMIO, interrupt, routing, or management authority. Unknown feature
bits, any other count, reserved data, or a sequence that cannot advance twice
fail closed.

Version 3 is a bounded dispatch proof. It does not define an indefinite mixed
protocol loop, neighbor expiry, IPv4 options, reassembly, outgoing traffic,
ICMP semantics, UDP, TCP, or sockets.
