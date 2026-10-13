# Native Protocol Service Runtime v0

**Status:** A measured freestanding Rust service consumes and validates one
real virtio-net Ethernet/ARP reply in an isolated ring-3 domain under QEMU/TCG.

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
Only afterward does the protocol service start with bootstrap ABI v0 and the
exact pending sequence.

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

## Deliberate limits

This is one synchronous ARP reply, not a network stack. There is no Ethernet
demultiplexing loop, ARP request generation, neighbor cache, IPv4 validation,
ICMP, UDP, TCP, fragmentation, routing, socket API, shared-memory packet path,
or container network interface. The current FIFO and call gate remain
single-CPU and copied.
