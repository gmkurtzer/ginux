# Native Protocol Service Bootstrap ABI v1

**Status:** Shared Rust and C layouts are exercised by the measured resident
protocol service under QEMU/TCG.

Version 1 keeps the v0 record size and alignment. It replaces the final eight
reserved bytes with a bounded packet count:

| Offset | Width | Field |
|---:|---:|---|
| 0 | 8 | magic `GINUXPB0` |
| 8 | 2 | version `1` |
| 10 | 2 | size `56` |
| 12 | 4 | exact feature bits: packet consumer, Ethernet/ARP, and wait loop |
| 16 | 8 | opaque packet capability |
| 24 | 8 | nonzero first expected packet sequence |
| 32 | 8 | local 48-bit MAC in network order |
| 40 | 4 | local IPv4 address in network order |
| 44 | 4 | peer IPv4 address in network order |
| 48 | 4 | packet count, exactly `2` in v1 |
| 52 | 4 | reserved zero |

The service rejects a null record, wrong magic, version, size, or feature set,
zero capability, a sequence that cannot advance twice, invalid MAC, zero or
identical IP addresses, a packet count other than two, or a nonzero reserved
field. Unknown feature bits fail closed.

The capability remains generation-local packet-object `READ` authority. For
each consecutive sequence, the service first issues an unbounded exact-sequence
wait and then performs a separate consume. Publication wakes the waiter but
does not reserve or transfer the FIFO entry; the subsequent consume performs
the authoritative sequence check. Cancellation, domain retirement, or a peer
fault removes the wait key through the existing packet-wait lifecycle rules.

The fixed count is an executable ABI proof, not a general event-loop contract.
Version 1 does not define dynamic batching, service shutdown messages, packet
classification, neighbor-cache lifetime, transmit authority, sockets, or
container endpoints.
