# Native Protocol Service Runtime v0

**Status:** A measured freestanding Rust service consumes and validates real
Ethernet/ARP replies in an isolated ring-3 domain under QEMU/TCG, including a
bounded resident wait-loop proof across two separately published packets.

## Boundary

`ginux-protocol-service-runtime` is a `no_std`, `no_main` static PIE built for
`x86_64-unknown-none`. `make protocol-service` builds it directly; `make
manifest`, `make check`, and `make gate` include it. Manifest generation stores
the exact artifact at `native/protocol` in the signed deterministic initramfs.

The bounded native ELF loader requires three load segments, relative-only
relocations, W^X image mappings, a read-only bootstrap page, a two-page NX
stack, and guard pages. The live domain receives no DMA or MMIO mapping. Its
sealed capability table contains only one packet-object `READ` grant.

The isolated virtio-net driver first validates an interrupt-completed gateway
ARP reply and copies its 42 Ethernet bytes into the kernel FIFO. Driver
teardown then removes PCI, MSI-X, VT-d, DMA, and packet-producer authority.
Only afterward does the first protocol-service proof start with bootstrap ABI
v0 and the exact pending sequence.

## Runtime proof

The service requires control ABI v4, the structured descriptor, and packet
handoff support. Before consumption it proves its packet handle cannot publish
and cannot probe the network device. It then consumes exactly one sequence and
validates:

- the Ethernet destination is the configured local MAC;
- EtherType is ARP;
- hardware/protocol types and address widths are Ethernet/IPv4;
- the operation is reply;
- Ethernet and ARP sender MACs agree and are unicast;
- sender IPv4 is the configured peer;
- target MAC and IPv4 are the configured local endpoint.

It changes the accepted opcode locally and requires that malformed copy to be
rejected, then proves the consumed sequence cannot be replayed. Success exits
through lifecycle ABI v0. The kernel requires clean exit, exact delivery
metadata, capability revocation, root restoration, and complete frame
reclamation.

## Resident wait-loop proof

A separate generation starts before any packet is pending with bootstrap ABI
v1, the next exact sequence, and a fixed packet count of two. A write-only
ring-3 producer remains runnable while the service blocks. It publishes one
validated ARP frame, yields until the service consumes it and blocks for the
next sequence, then publishes the second frame.

The same measured protocol domain consumes both consecutive sequences and
performs the full Ethernet/ARP validation and malformed-copy rejection for
each. It then rejects replay of the second sequence and exits cleanly. The
kernel requires both waits to wake through publication, the domain to remain
live between arrivals, FIFO exhaustion to advance exactly twice, and all wait
keys, capabilities, address-space frames, and producer frames to be retired.

Ethernet/ARP parsing and one-entry neighbor state live in the reusable
`ginux-protocol-state` `no_std` crate. The first reply learns the configured
peer's MAC, the second refreshes it, and a locally constructed conflicting
mapping must be rejected without mutating the learned entry.

## Deliberate limits

Bootstrap ABI v2 removes the packet-count limit and leaves the measured service
blocked on its next exact sequence. The QEMU proof delivers two frames, confirms
the third wait is live, and uses control-plane stop authority to cancel and
fully reclaim the domain. This establishes external lifecycle ownership without
granting management authority to the protocol service.

Bootstrap ABI v3 is a separate bounded dispatch generation. It learns the peer
from ARP, then consumes an actual Ethernet/IPv4/ICMP frame, validates its fixed
IPv4 header and checksum in ring 3, and rejects a locally corrupted checksum.

Bootstrap ABI v4 additionally validates the ICMPv4 echo-request type, code,
complete-message checksum, identifier, sequence, and bounded payload profile.
The service corrupts its copied ICMP checksum and requires rejection without
gaining packet-write authority.

This remains a fixed ARP proof, not a network stack. The resident path accepts
exactly two packets under v1; v2 may continue until administratively stopped.
There is no graceful drain or state handoff protocol. There is no
Ethernet demultiplexing, ARP request generation, multi-entry or expiring
neighbor cache, IPv4 options or reassembly, ICMP replies or errors, UDP, TCP, routing,
socket API, shared-memory
packet path, or container network interface. The current FIFO and call gate
remain single-CPU and copied.
