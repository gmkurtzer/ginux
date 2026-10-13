# Protocol ICMPv4 Echo Reply v0

**Status:** A measured Rust protocol service constructs and delegates one
ICMPv4 echo reply between isolated ring-3 domains under QEMU/TCG.

The memory-safe `ginux-protocol-state` builder first applies the complete v4
Ethernet, IPv4, and ICMP echo-request validation without mutation. On success
it swaps Ethernet and IPv4 endpoints, changes ICMP type 8 to type 0, preserves
code, identifier, sequence, and payload, and recomputes both IPv4 and ICMP
checksums. Unit tests require the exact 42-byte proof reply and prove a bad
request remains byte-for-byte unchanged.

The protocol service has ingress `READ` and independent egress `WRITE`; it has
neither reverse direction. Egress publication copies the complete reply into a
separate four-entry kernel mailbox. The proof then retires the protocol domain
and starts a fresh egress consumer with only egress `READ`. That consumer
rejects publication and a future sequence without modifying its output,
receives the exact reply, and proves destructive delivery by rejecting replay.
Capability checks also deny ingress and network-device authority.

An additive network bootstrap v2 proof now gives a fresh measured network
service egress `READ` plus independent NIC authority. It consumes a second
reply into confined DMA and transmits it through virtio-net without giving the
protocol service device authority. See
[Native Network Protocol Egress Transmit v0](network-egress-transmit-v0.md).
Egress blocking/wakeup, arbitrary payload policy, rate limiting, routing,
sockets, and container endpoints remain later milestones.
