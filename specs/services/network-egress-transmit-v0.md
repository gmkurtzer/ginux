# Native Network Protocol Egress Transmit v0

**Status:** One protocol-generated ICMPv4 echo reply is consumed and
transmitted by a confined network service under QEMU/TCG.

The measured protocol service receives ingress `READ` and egress `WRITE`,
constructs the exact checksummed 42-byte Ethernet/IPv4/ICMP reply, publishes
the next egress sequence, and retires without device authority. The copied
packet remains in kernel-owned object 9.

A fresh measured network-service generation receives network bootstrap ABI v2,
network-device `READ|WRITE`, and egress `READ`. Before consuming the expected
sequence, it proves that the egress handle cannot publish and that a future
sequence returns `STALE` without changing the destination. It then consumes
the exact sequence into its generation-owned DMA page and proves replay is
rejected.

The service prepends a zeroed 12-byte modern virtio-net header, publishes one
queue-1 descriptor and available entry, notifies queue 1, and blocks on native
network wait request one. Kernel-owned MSI-X vector 35 wakes only that domain
and request. Before accepting clean exit, the kernel independently validates
the descriptor address and 54-byte length, header, exact reply bytes, used
entry, untouched descriptors, canaries, and completed interrupt generation.

Teardown masks and disables MSI-X, revokes bus mastering, resets the device,
retires vector 35, disables and scrubs the exact VT-d translation, scrubs DMA,
revokes both capabilities, and retires the service domain. The egress queue is
empty and its next sequence has advanced exactly once.

## Limits

This proves the complete isolated protocol-to-device ownership path and device
consumption. It does not prove peer reception. The source request is a
deterministic copied proof frame rather than a live NIC receive, and there is
no egress wait, batching, routing, socket layer, or container interface.

This v2 proof destructively consumes before transmission. Bootstrap v3 adds
claim/ack delivery so a driver fault before acknowledgement preserves the
packet for a replacement. See
[Reliable Protocol Egress v0](network-reliable-egress-v0.md).
