# PCI Virtio Network Discovery v0

**Status:** Deterministic Q35 discovery and firmware-authority revocation are
verified in QEMU.

## Purpose

This contract establishes the first physical network-device boundary without
starting a driver. The kernel must identify one modern virtio-net PCI function,
validate its transport metadata from a copied configuration image, and revoke
any DMA or MMIO authority left active by firmware before reclaiming boot-service
memory.

## Development profile

The authoritative QEMU machine exposes one modern-only `virtio-net-pci`
function at `00:03.0`, backed by restricted user networking with no host
forward. Platform DMA is required so a later driver must negotiate
`VIRTIO_F_ACCESS_PLATFORM`. The existing block function remains at `00:02.0`.

The bus-zero scan examines function zero only and requires exactly one matching
vendor/device identity (`1af4:1041`). Duplicate matches, an absent function, or
an invalid matching function fail closed.

## Copied configuration validation

The parser requires a type-zero header, a bounded aligned acyclic capability
chain, one common, notification, ISR, and device configuration capability, a
nonzero notification multiplier, valid memory BAR references, and exactly one
disabled MSI-X capability with bounded table and pending-bit ranges. Duplicate,
truncated, cyclic, misaligned, zero-length, or invalid-BAR capabilities are
rejected. The same parser core serves virtio block and network identities, but
the accepted device ID is explicit for each call.

Malformed-copy tests change the identity, remove the capability-list status,
misalign or cycle the chain, corrupt required capability lengths and BARs,
remove or enable MSI-X, and corrupt both MSI-X BAR indicators. These tests do
not write live device state.

## Firmware handoff

OVMF currently hands the network function to the kernel with PCI I/O, MMIO,
and bus mastering enabled. Immediately after successful copied-configuration
validation, and before physical-memory initialization or boot-service memory
reclamation, the kernel clears all three command bits through PCI configuration
space and reads them back. A later boot-stage check requires them to remain
clear.

The validated ACPI DMAR unit must also contain an exact endpoint scope for
`00:03.0`. No network BAR is mapped, no MMIO register or device feature is read,
no MSI-X entry or queue is configured, and no DMA page is delegated in this
milestone.

## Deliberate limits

- This is discovery and authority revocation, not a network driver.
- The restricted QEMU user backend is test infrastructure; the guest sends no
  traffic yet.
- Device reset semantics, feature negotiation, receive/transmit queues,
  interrupts, IOMMU translation, a native network service, and container
  networking remain later M6 work.
- Only bus zero, function zero, and the deterministic Q35 profile are covered.
