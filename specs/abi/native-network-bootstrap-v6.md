# Native Network Service Bootstrap ABI v6

**Status:** Additive full-FIFO reliable-egress batch verified through four
interrupt-completed virtio-net transmissions under QEMU/TCG.

Version 6 preserves the complete 96-byte v5 layout and adds `EGRESS_BATCH`.
Its exact feature set is the v5 resident reliable-egress set plus that bit.
All address, capability, MAC, reserved-zero, and initial-sequence constraints
remain unchanged; the initial sequence must admit three successors.

The measured service remains in one ring-3 domain while an isolated write-only
producer fills all four egress FIFO entries. It claims, transmits, and
acknowledges each sequence in order using TX descriptor zero, one guarded DMA
buffer, one active queue, one VT-d hierarchy, and one device capability.
Available and used indices and kernel-owned interrupt requests advance from one
through four. Each acknowledgement requires the matching validated completion;
earlier completions cannot authorize a later sequence.

Successful network-device completion also renews the auxiliary service's
23-call epoch. Renewal is kernel-internal and requires the exact live auxiliary
domain, owned address space, attached thread, and active call. Denied calls,
ordinary yield, packet operations, failed waits, and timed-out or cancelled
device generations do not renew it. This bounds calls between device progress
events while allowing the resident driver to exceed its bootstrap lifetime
allowance.

The QEMU proof necessarily crosses that original lifetime bound, drains the
full FIFO, rejects premature acknowledgement, duplicate claim, interrupt
replay, and post-ack claim replay for every packet, and requires an empty queue
and claim slot afterward. Producer, service, interrupt, device, VT-d, DMA,
capability, and address-space state then retire in order.

## Limits

This profile processes exactly four fixed replies and exits. Epoch renewal is
currently tied only to successful network waits; it is not a general CPU,
bandwidth, or admission-control policy. The profile still has one producer,
one claim, one descriptor, one DMA page, and one NIC, with no batching ABI,
claim deadline, dynamic queue sizing, peer-receipt proof, or container sockets.
