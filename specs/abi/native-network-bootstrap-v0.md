# Native Network Service Bootstrap ABI v0

**Status:** The shared Rust definition, independent C11 layout consumer, kernel
delivery, and measured ring-3 service validation are verified.

## Entry contract

The kernel will enter the authenticated network-service PIE at ring 3 with
`RBX` containing the user virtual address of a read-only, NX
`ginux_native_network_boot_v0` record. The record is little-endian, C-aligned,
exactly 80 bytes, and uses only fixed-width integers:

| Offset | Width | Field |
|---:|---:|---|
| 0 | 8 | magic `0x30424e58554e4947` (`GINUXNB0` in memory) |
| 8 | 2 | version `0` |
| 10 | 2 | structure size `80` |
| 12 | 4 | exact feature bitmap |
| 16 | 8 | service virtual address of virtio common configuration |
| 24 | 8 | service virtual address of the receive notification register |
| 32 | 8 | service virtual address of the transmit notification register |
| 40 | 8 | service virtual address of the owned DMA window |
| 48 | 8 | device-visible address of that DMA window |
| 56 | 8 | opaque capability for this network-device generation |
| 64 | 8 | MAC address in the low 48 bits; high 16 bits zero |
| 72 | 8 | reserved; must be zero |

Feature bit zero is `VIRTIO_NETWORK`; v0 accepts exactly that bit. Every
address, capability, and MAC field must be nonzero, and the two notification
addresses must be distinct. A wrong magic, version, size, feature bitmap,
nonzero reserved field, or nonzero MAC high bits is rejected before device
activation.

The two notification registers may share one mapped MMIO page, but each
address identifies only its exact 16-bit queue notification location. The DMA
device address is an I/O virtual address authorized only for the receiving
domain generation. The capability is opaque and resolves only in that same
generation's sealed capability table. The record exposes no PCI configuration,
MSI-X table, IDT, APIC, IOMMU-table, or kernel pointer.

## Compatibility and limits

`ginux-native-abi` is the canonical `no_std` Rust definition.
`native-network-bootstrap-v0.h` is the C binding, and `make abi-conformance`
freezes every field offset and the total size through an independent
freestanding C11 consumer.

ABI v0 is exact-version only. The first delivery proof maps the common and
notification pages and allocates one DMA page, but keeps PCI memory decoding,
bus mastering, MSI-X, and VT-d disabled. The service exercises only its
read-only logical network capability and exits; teardown revokes the capability
and scrubs every owned frame. A separate generation receives a nonzero reserved
field, faults before device activation, and undergoes the same capability,
address-space, and DMA retirement. Device activation, interrupts, and
packet-queue contents require separate generation-bound kernel checks and QEMU
proofs.

A third generation receives a fresh six-frame VT-d hierarchy that encodes only
the network source identity and its generation-owned DMA page. Translation is
activated before ring-3 entry while PCI decoding, bus mastering, and MSI-X stay
disabled. Clean lifecycle exit disables translation, invalidates the hardware
caches, scrubs the translation tables, and only then scrubs and releases the
DMA page. The service still performs no packet I/O in this proof.
