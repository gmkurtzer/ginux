# Native Network Service Transmit v0

**Status:** One confined, interrupt-completed transmit is verified on QEMU Q35.

## Contract

Bootstrap feature `TX_INTERRUPT` requires `TRANSMIT_PROBE`, which requires
`EMPTY_QUEUES`. The driver binds TX queue 1 to MSI-X table entry 1 while RX
remains unbound. Before ring-3 entry, the kernel programs that entry for local
APIC vector 35, installs the interrupt gate, and keeps the entry and global
MSI-X function masked. After capability command 12 validates and activates the
service-configured queues, the measured ring-3 driver
owns publication of one queue-1 descriptor. The descriptor addresses only its
generation-owned DMA page and contains a zeroed 12-byte modern virtio-net
header followed by this fixed 19-byte Ethernet frame:

- broadcast destination `ff:ff:ff:ff:ff:ff`;
- source `52:54:00:12:34:56`;
- experimental EtherType `0x88b5`; and
- payload `GINUX`.

The service publishes available index one with descriptor ID zero, executes a
full compiler fence, writes queue index one to the exact transmit notification
register. It then invokes native operation 20, command 1, with its opaque
network capability and request generation one. The kernel validates the exact
caller, capability, VT-d lease, queue binding, vector selection, and request,
saves the ring-3 context as blocked, unmasks MSI-X, and waits with a bounded
APIC timer. Vector 35 wakes only the matching network-device wait. Success
requires exactly one interrupt. Reusing request generation one is rejected
without blocking. A timeout or malformed completion drives the normal ring-3
driver fault path.

## Independent validation and teardown

Before accepting clean lifecycle exit, the kernel requires the live exact
queue bindings, descriptor address and length, available entry, used index and
descriptor ID, unchanged header and Ethernet bytes, intact boundary canaries,
no unexpected descriptor entries, and completion of the admitted interrupt
wait. It then masks and disables MSI-X, revokes bus mastering, resets and
disables the function, retires vector 35, retires VT-d, and scrubs the
translation and DMA frames in that order.

## Deliberate limits

This proves device consumption, not peer reception. It sends one fixed frame,
uses one serialized request generation, and has no general packet API,
batching, multi-buffer support, receive path, or container interface. Network
timeout cancellation and restart are the next contract extension.
