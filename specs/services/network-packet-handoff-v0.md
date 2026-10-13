# Native Network Packet Handoff v0

**Status:** One copied receive packet is transferred between independently
authorized ring-3 domains under QEMU/TCG.

The measured network service completes two interrupt-driven ARP exchanges. It
validates the second virtio header and Ethernet/ARP reply, then invokes packet
publication with only the 42 Ethernet bytes. The call gate validates the
producer's packet-object write capability and copies from its current user
mapping into a bounded kernel mailbox. No DMA address, DMA page, MMIO mapping,
MSI-X route, IOMMU table, or network-device capability is transferred.

The producer first proves its write capability cannot consume. Normal service
teardown then resets the NIC, retires interrupt routes and translation, scrubs
DMA, and revokes both producer capabilities without deleting the copied packet.

A fresh auxiliary consumer receives only packet-object read authority. It has
neither packet write nor network-device read authority. In ring 3 it proves a
future sequence is rejected without changing its output, consumes sequence one,
compares all 42 bytes with the deterministic gateway reply, and proves replay
returns empty. Its address space and capability are then retired normally.

This milestone establishes the protected boundary between a disposable device
driver and a non-device packet consumer. It does not yet provide a persistent
control-plane event loop or a container network endpoint. See
[Native Network Service Bootstrap ABI v1](../abi/native-network-bootstrap-v1.md).

A second generation repeats publication as sequence two and then takes its
compiled fatal `HLT` path while queues, MSI-X, bus mastering, and translation
are live. The kernel records the ring-3 fault, quiesces the device before
retiring VT-d and DMA, revokes both producer capabilities, and deliberately
retains the copied packet. Another fresh read-only consumer validates and
consumes sequence two. This proves packet lifetime is independent of both clean
producer exit and producer failure.

A bounded-batch generation publishes both validated replies as sequences three
and four before clean driver retirement. Two fresh read-only domains consume
them in order after all device authority is gone. The kernel's four-entry FIFO
invariant also proves full-queue backpressure, retained entries across producer
replacement, wraparound reuse, and monotonic sequence allocation. This is a
bounded copied queue, not yet a blocking or zero-copy container endpoint.
