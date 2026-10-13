# Resident Network Service Replacement v0

**Status:** A stopped resident virtio-net service is replaced by a fresh
generation with stable identity and fresh DMA authority under QEMU/TCG.

The control plane first starts bootstrap-v7 network service generation A. A
write-only producer fills the four-entry egress FIFO, generation A transmits
and acknowledges all four packets, and then blocks on the exact next sequence.
Control-plane stop cancels that wait and performs complete ordered device,
interrupt, VT-d, DMA, capability, address-space, and domain retirement.

Generation B is created through the auxiliary restart path with generation
A's logical service identity. It receives freshly derived network-device and
egress capabilities, a distinct domain generation, a newly allocated DMA page,
and six new VT-d table leases. No frame lease from generation A may reappear in
generation B. The NIC begins reset and DMA-disabled, so B must negotiate and
activate it from scratch.

A second write-only producer publishes the next four monotonic egress
sequences. Generation B transmits and acknowledges them, advances to the exact
following sequence, blocks, and is stopped through the same ordered path. The
gate requires both IOMMU evidence records to be retired, the two domain IDs to
differ, the logical service identity to match, all eight sequences to be
consumed in order, and no queue claim or packet to remain.

## Limits

Replacement is an explicit control-plane action after a clean external stop.
There is no automatic restart policy, rolling overlap, state transfer, packet
claim handoff, or health-based restart decision. The egress FIFO provides the
continuity boundary; device queue and DMA state are deliberately rebuilt.
