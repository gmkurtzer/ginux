# Native Protocol Service Bootstrap ABI v5

**Status:** Shared Rust and C layouts are exercised by a measured ICMP echo
reply generation under QEMU/TCG.

Version 5 extends the v4 common prefix from 56 to 72 bytes. It retains the
two-packet ARP-then-ICMP sequence and adds:

| Offset | Size | Field |
|---:|---:|---|
| 56 | 8 | opaque egress packet capability |
| 64 | 8 | exact nonzero first egress sequence |

The feature word is exactly the v4 set plus `PACKET_EGRESS`. The ingress and
egress handles must both be nonzero and distinct. All v4 endpoint, count,
sequence, and reserved-zero checks remain mandatory.

The sealed protocol-domain capability table contains ingress object 8 `READ`
and egress object 9 `WRITE`. The ingress handle cannot publish. The egress
handle cannot consume, wait, observe the ingress queue, or call the network
device. The service receives no NIC, DMA, MMIO, interrupt, routing, lifecycle
management, or control-plane authority.

After validating the echo request, the service constructs one reply, publishes
it at the exact expected egress sequence, rejects ingress replay, and exits.
The reply remains in kernel-owned egress storage after that domain and both
capabilities retire. A fresh domain with object 9 `READ` only rejects
publication and a future sequence, consumes the exact reply, and rejects
replay.

Version 5 itself grants no device authority. A separate network bootstrap v2
consumer now connects a subsequent egress sequence to virtio-net while keeping
that separation. Later control operations 28 and 29 add egress waiting and
producer backpressure without changing this frozen bootstrap. Arbitrary packet
transmission, routing, sockets, and container endpoints remain undefined.
