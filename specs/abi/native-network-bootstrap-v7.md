# Native Network Service Bootstrap ABI v7

**Status:** Additive resident reliable-egress loop with both control-plane stop
and graceful drain verified with ordered virtio-net teardown under QEMU/TCG.

Version 7 preserves the complete 96-byte v6 layout and adds
`EGRESS_RESIDENT`. Its exact feature set is the v6 full-FIFO reliable-egress
set plus that bit. All address, capability, MAC, reserved-zero, and initial
sequence constraints remain unchanged; the initial sequence must admit four
successors.

The measured ring-3 network service treats a zero transmission count as an
unbounded service loop. It waits for the exact next object-9 sequence, claims
the queue head, copies it into its confined DMA page, transmits through the
resident virtio-net queue, waits for the matching kernel-owned MSI-X request,
and acknowledges only after kernel validation. It advances to the next exact
sequence and repeats without replacing its domain, capability, queue, DMA
lease, VT-d hierarchy, or interrupt route.

The executable proof fills all four FIFO entries. Its write-only producer uses
additive control operation 30, `EGRESS_DRAIN_WAIT`, to block until successful
acknowledgements empty the queue. That wait neither reads packet contents nor
reserves a slot. The fourth acknowledgement wakes only that producer; the
service then blocks indefinitely on sequence five, the producer exits, and
control returns with the service and its exact wait key still live.

The persistent control plane stops the blocked service through its existing
`MANAGE` authority. Stop first cancels the scheduler wait, then masks and
retires the interrupt routes, revokes bus mastering, resets and disables the
NIC, retires VT-d, scrubs DMA and address-space frames, revokes capabilities,
and publishes one `STOPPED` lifecycle event. The gate requires the device to
have been active when this ordered quiescence began and requires no packet
claim, wait key, interrupt route, translation, DMA lease, capability, domain,
or owned frame to remain afterward.

The same v7 service also supports the separate graceful-drain lifecycle. A
host-management request marks its exact generation for drain. It preserves
and acknowledges every accepted FIFO entry, waits for the producer to close,
receives `PACKET_RESULT_DRAINED` instead of a nonexistent next sequence, and
exits normally through the same ordered device teardown. See
[Resident Network Service Drain v0](../services/network-resident-drain-v0.md).

## Limits

The service loop has no state handoff, dynamic queue size, claim deadline,
watchdog, or automatic restart policy. A separate proof
performs explicit clean replacement with stable logical identity and fresh
device authority, but does not overlap generations. Operation 30 is
single-waiter and applies only to the currently bound object-9 write-only
producer. The proof transmits four fixed ICMP replies before external stop;
container sockets, routing, UDP/TCP, multi-queue operation, and peer-receipt
proof remain separate milestones.
