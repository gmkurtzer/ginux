# Protocol IPv4 Validation v0

**Status:** A measured Rust protocol service validates a FIFO-delivered
Ethernet/IPv4/ICMP frame in ring 3 under QEMU/TCG.

The `ginux-protocol-state` `no_std` parser accepts an Ethernet II IPv4 frame
only after ARP state has established the exact peer MAC. This profile requires:

- exact local destination and learned peer source MACs;
- EtherType IPv4;
- IPv4 version 4 with a five-word header and zero DSCP/ECN;
- a total length exactly matching the delivered Ethernet payload;
- a nonzero TTL and valid IPv4 header checksum;
- neither `MF` nor a nonzero fragment offset;
- configured peer source and local destination IPv4 addresses;
- protocol number 1 and an eight-byte payload in the runtime proof.

The measured v3 generation first consumes ARP, then a separately published
42-byte IPv4/ICMP frame. It corrupts the copied IPv4 checksum locally and
requires rejection before restoring the byte, proving malformed input does not
need kernel parsing. The generation then exits with its capability and frames
fully reclaimed.

Bootstrap ABI v3 only dispatches and validates the IPv4 header. Additive ABI v4
also validates the bounded ICMP echo request. Neither accepts IPv4 options,
reassembles fragments, routes packets, transmits replies, or exposes sockets.
