# Native Network Service Transmit v0

**Status:** One confined, completion-polled transmit is verified on QEMU Q35.

## Contract

Bootstrap feature `TRANSMIT_PROBE` requires `EMPTY_QUEUES`. After capability
command 12 activates the service-configured queues, the measured ring-3 driver
owns publication of one queue-1 descriptor. The descriptor addresses only its
generation-owned DMA page and contains a zeroed 12-byte modern virtio-net
header followed by this fixed 19-byte Ethernet frame:

- broadcast destination `ff:ff:ff:ff:ff:ff`;
- source `52:54:00:12:34:56`;
- experimental EtherType `0x88b5`; and
- payload `GINUX`.

The service publishes available index one with descriptor ID zero, executes a
full compiler fence, writes queue index one to the exact transmit notification
register, and polls the used index with a fixed upper bound. MSI-X remains
disabled. A timeout or malformed completion drives the normal ring-3 driver
fault path.

## Independent validation and teardown

Before accepting clean lifecycle exit, the kernel requires the live exact
queue bindings, descriptor address and length, available entry, used index and
descriptor ID, unchanged header and Ethernet bytes, intact boundary canaries,
and no unexpected descriptor entries. It then revokes bus mastering, resets
and disables the function, retires VT-d, and scrubs the translation and DMA
frames in that order.

## Deliberate limits

This proves device consumption, not peer reception. It sends one fixed frame,
uses bounded polling instead of interrupt delivery, and has no general packet
API, batching, multi-buffer support, receive path, or container interface.
