# Native Protocol Service Bootstrap ABI v0

**Status:** Shared Rust and C layouts are exercised by the measured protocol
service under QEMU/TCG.

The protocol service enters with `RBX` pointing to one read-only 56-byte record:

| Offset | Width | Field |
|---:|---:|---|
| 0 | 8 | magic `GINUXPB0` |
| 8 | 2 | version `0` |
| 10 | 2 | size `56` |
| 12 | 4 | exact feature bits: packet consumer and Ethernet/ARP |
| 16 | 8 | opaque packet capability |
| 24 | 8 | nonzero expected packet sequence |
| 32 | 8 | local 48-bit MAC in network order |
| 40 | 4 | local IPv4 address in network order |
| 44 | 4 | peer IPv4 address in network order |
| 48 | 8 | reserved zero |

The service rejects a null record, wrong magic, version, size, feature set,
zero capability, zero or exhausted sequence, invalid MAC, zero or identical IP
addresses, or any reserved bit. Unknown feature bits fail closed in v0.

The capability is generation-local and conveys packet-object `READ` only. It
is not a device handle, DMA address, MMIO mapping, or transferable authority.
The expected sequence binds this invocation to exactly one FIFO head entry.

The canonical Rust definition lives in `ginux-native-abi`; the independent C11
layout is `native-protocol-bootstrap-v0.h`. Both freeze the size, alignment, and
field offsets. All multibyte scalar fields use native little-endian storage;
MAC and IPv4 values represent network-order integers so conversion to wire
bytes is explicit.

This version bootstraps one deterministic ARP validation request. Bootstrap
ABI v1 retains the 56-byte layout and adds a bounded two-packet wait loop. v0
does not define interface configuration, routing, neighbor-cache lifetime,
asynchronous packet loops, transmit authority, sockets, or container endpoints.
