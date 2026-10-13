# Native Network Service Receive v0

**Status:** One confined, interrupt-completed ARP exchange is verified on QEMU
Q35.

## Contract

Bootstrap feature `ARP_PROBE` requires `TX_INTERRUPT`, `TRANSMIT_PROBE`, and
`EMPTY_QUEUES`. The measured ring-3 driver binds receive queue 0 to MSI-X table
entry 0 and transmit queue 1 to entry 1. The kernel programs local-APIC vectors
34 and 35, installs both interrupt gates, and keeps both entries and the global
MSI-X function masked until a generation-bound wait is admitted.

The service publishes one device-write-only receive descriptor covering 1535
bytes in its sole DMA page. Canary bytes immediately precede and follow that
range. It then publishes a transmit descriptor containing a zeroed 12-byte
modern virtio-net header and a padded Ethernet/ARP request for QEMU's fixed
`10.0.2.2` gateway from guest address `10.0.2.15`. The receive queue is notified
before the transmit queue so a reply always has an available buffer.

Native operation 20 serializes the two completions. `TX_NOTIFIED` command 1,
request generation one, unmasks vector 35 and completes only after exactly one
transmit interrupt. The kernel then advances the admitted device generation to
two. `RX_POSTED` command 2, request generation two, unmasks vector 34 and
completes only after exactly one receive interrupt. A duplicate receive wait is
rejected without blocking.

A notified RX timeout is cancellation, not an empty receive. The kernel keeps
RX entry 0 masked until the calibrated device-wait timer expires, then masks
and disables MSI-X, revokes bus mastering, resets the NIC, and retires both
routes before returning `CANCELLED`. The service exits `RESTART_REQUIRED`; it
cannot reuse either queue, its DMA page, or its request generations. A fresh
domain with newly delegated authority, VT-d tables, and DMA must reinitialize
the reset device. The QEMU gate requires that replacement to complete and
validate another ARP exchange.

The service accepts only one-buffer modern virtio-net input with a zeroed
header, `num_buffers` equal to one, one used-ring element with descriptor ID
zero, and a bounded length. It then validates Ethernet destination/source and
EtherType plus the complete ARP reply hardware type, protocol type, widths,
operation, sender identity, and target identity. The expected identities are
the pinned guest MAC, QEMU gateway MAC, guest IPv4 address, and gateway IPv4
address.

## Independent validation and teardown

The kernel does not trust the service's clean exit. It independently requires
the exact RX and TX queue bindings, descriptor addresses, flags and lengths,
available and used entries, unchanged padded ARP request, bounded ARP reply,
virtio header, endpoint identities, unused zero descriptors, both admitted
interrupt completions, and all three canaries.

Teardown masks both entries and the MSI-X function, disables MSI-X, revokes bus
mastering, resets and disables the NIC, retires vectors 35 and 34, disables and
scrubs the exact VT-d hierarchy, and finally scrubs the service DMA and address
space. No interrupt route or device-visible page survives the generation.

## Deliberate limits

This proves one serialized receive buffer and a deterministic peer response.
It does not provide a packet IPC interface, arbitrary Ethernet traffic, receive
buffer replenishment, batching, multiple in-flight requests, IPv4 packet
processing, DHCP, sockets, routing, a system-container interface, or production
network configuration. Malformed-reply recovery remains a separate milestone.
