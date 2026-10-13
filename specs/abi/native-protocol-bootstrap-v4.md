# Native Protocol Service Bootstrap ABI v4

**Status:** Shared Rust and C layouts are exercised by a measured ICMP echo
validation generation under QEMU/TCG.

Version 4 retains the 56-byte v3 layout and its two-packet ARP-then-IPv4
sequence. It adds the exact ICMP-echo feature bit. The IPv4 payload must be a
valid ICMPv4 echo request; the measured profile requires identifier `0xbeef`,
sequence `1`, and no echo data.

All v3 authority restrictions remain: the service has packet-read authority
only, cannot publish the echo reply, and receives no device, DMA, MMIO,
interrupt, routing, or management capability. Unknown bits, a count other than
two, reserved data, or invalid endpoint and sequence fields fail closed.

Version 4 validates receipt only. Reply construction, transmit delegation,
rate limiting, arbitrary echo payloads, ICMP errors, and other ICMP types are
outside this contract.
