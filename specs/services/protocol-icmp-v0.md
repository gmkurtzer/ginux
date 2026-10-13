# Protocol ICMPv4 Echo Validation v0

**Status:** A measured Rust protocol service validates an actual FIFO-delivered
ICMPv4 echo request in ring 3 under QEMU/TCG.

After v4 validates Ethernet, learned ARP identity, and the fixed IPv4 header,
the `ginux-protocol-state` parser requires:

- at least the eight-byte ICMP echo header;
- type 8 and code 0;
- a valid checksum over the complete ICMP message;
- identifier `0xbeef` and sequence `1` in the measured runtime profile;
- an empty payload in the measured runtime profile.

Unit tests separately reject a checksum-correct non-echo type and a corrupted
checksum. The ring-3 service corrupts its copied checksum and requires rejection
before restoring it. It retains packet-read authority only and exits with full
capability and frame reclamation.

This milestone does not construct or transmit an echo reply. It has no ICMP
error handling, rate limiting, arbitrary payload policy, routing, socket API,
or container endpoint.
