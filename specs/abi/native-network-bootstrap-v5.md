# Native Network Service Bootstrap ABI v5

**Status:** Additive resident reliable-egress extension verified across two
interrupt-completed virtio-net transmissions under QEMU/TCG.

Version 5 preserves the complete 96-byte v4 layout and adds
`EGRESS_REUSE`. Its exact feature set is the v4 reliable blocking-egress set
plus that bit. All address, capability, MAC, reserved-zero, and initial
sequence constraints remain unchanged; the initial sequence must also admit
one successor.

The measured service remains in one ring-3 domain and consumes two consecutive
42-byte ICMPv4 replies from object 9. It reuses TX descriptor zero, its guarded
DMA buffer, the active virtio queue, the VT-d hierarchy, and the device
capability. Available and used indices advance from one to two, while the
kernel-owned interrupt request advances from generation one to generation two.

For each packet, claim leaves the queue head present, premature acknowledgement
and duplicate claim are rejected, TX completion must validate the exact ring
entry and bytes, and only then may acknowledgement remove that sequence. A
replay claim is rejected after each acknowledgement. Generation-wide denial
probes for publication, wrong-object waiting, and a future sequence run once
before the first transaction rather than consuming the service's bounded call
budget again for the second packet.

The QEMU proof starts the network service on an empty FIFO and lets an isolated
write-only producer publish both consecutive replies. Success requires two
ordered MSI-X completions, two acknowledgements, an empty FIFO and claim slot,
the second sequence as the last delivery, and ordered retirement of the
producer, service, interrupt route, device, VT-d hierarchy, DMA lease,
capabilities, and address spaces.

## Limits

This profile processes exactly two fixed replies and then exits. Bootstrap v6
separately drains all four FIFO entries using completion-bound call epochs. It
has one global egress claim, one TX descriptor, one DMA page, and one NIC. It does not
define an unbounded service loop, batching, multiple producers, claim timeout,
dynamic queue sizing, peer receipt, or container-facing sockets.
