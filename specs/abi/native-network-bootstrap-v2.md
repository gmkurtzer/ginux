# Native Network Service Bootstrap ABI v2

**Status:** Additive protocol-egress transmit extension verified with the
measured ring-3 network service under QEMU/TCG.

Version 2 preserves the first 72 bytes of bootstrap v0 and extends the record
to exactly 96 bytes:

| Offset | Width | Field |
|---:|---:|---|
| 72 | 8 | opaque egress-consumer capability |
| 80 | 8 | exact expected egress sequence |
| 88 | 8 | reserved zero |

The feature word is exactly `VIRTIO_NETWORK | EMPTY_QUEUES |
TRANSMIT_PROBE | TX_INTERRUPT | PACKET_EGRESS`. The device and egress handles
must be nonzero and distinct, and the expected sequence must be nonzero and
not `UINT64_MAX`. The service selects this layout from the version and size
header before reading the extension.

The sealed network-domain table contains network-device object 4 `READ|WRITE`
and packet-egress object 9 `READ`. The egress handle cannot publish, access
ingress object 8, or control the NIC. The device handle cannot access either
packet queue. Operation 22 copies only the exact requested egress sequence into
the service's owned DMA page; a future sequence returns `STALE` without
modifying the destination, and replay fails after destructive delivery.

Bootstrap v0 and v1 remain frozen. The shared `ginux-native-abi` Rust layout,
C header, and independent C11 consumer freeze the v2 size and extension field
offsets.

## Limits

The proof consumes and transmits one fixed 42-byte ICMPv4 echo reply. It does
not define egress waiting, producer backpressure, batching, arbitrary packet
policy, routing, sockets, or a container endpoint.
