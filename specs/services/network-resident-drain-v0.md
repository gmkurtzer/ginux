# Resident Network Service Drain v0

**Status:** Control-plane-authorized graceful drain verified through real
virtio-net, MSI-X, and VT-d teardown under QEMU/TCG.

A caller holding host-control object 1 `MANAGE` authority submits management
action 8 with the exact resident network-service generation in `source` and
zero `object` and `rights`. The target must be a ready, running stop-only
auxiliary with packet-egress object 9 `READ`. Wrong callers, wrong
capabilities, wrong generations, non-network targets, and duplicate requests
fail without changing lifecycle state.

The request is a graceful boundary, not an immediate stop. The network service
continues claiming, transmitting, and acknowledging every egress packet
already accepted by the bounded FIFO. Its write-only producer waits until the
queue is empty and then exits, closing future publication. If the network
service is blocked on the next exact sequence, producer retirement wakes that
specific object-9 waiter with `PACKET_RESULT_DRAINED` (`UINT64_MAX - 6`). The
request is consumed once. No packet contents, claim, or publication authority
are granted by that result.

The measured service treats `PACKET_RESULT_DRAINED` as successful completion
and exits normally. It does not overwrite the last completed DMA payload while
checking the empty queue. Normal auxiliary exit validates all four TX and
interrupt generations and acknowledgements, then masks interrupts, revokes bus
mastering, resets the NIC, retires VT-d, scrubs DMA and address-space frames,
revokes capabilities, and publishes the normal `EXITED` lifecycle event.

The executable proof fills the four-entry FIFO and requests drain before
dispatch. All four replies are preserved and transmitted in order. The proof
also requires the producer and driver to retire, the lifecycle FIFO to contain
their ordered normal-exit records, and no wait key, interrupt route,
translation, DMA lease, capability, domain, or owned frame to remain.

## Limits

This contract drains one globally bound egress producer and one resident
network-service generation. It does not yet define concurrent producers,
ingress drain, protocol-state transfer, overlapping driver generations,
deadlines, forced fallback after a stuck device, or automatic replacement.
