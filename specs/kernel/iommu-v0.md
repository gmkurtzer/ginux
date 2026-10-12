# IOMMU Register Access v0

**Status:** Bounded Q35 Intel IOMMU capability discovery and control.

## Boundary

The initial Q35 profile exposes one Intel DMA-remapping hardware unit at the
fixed physical base `0xfed90000`. ACPI DMAR discovery must report that exact
base before the kernel uses the mapping. This target-profile constant keeps the
register page available during the early page-table transition; it is not a
general hardware-discovery mechanism.

The kernel maps exactly one 4 KiB register page in its bounded higher-half MMIO
window. The mapping is supervisor-only, writable, uncached, and non-executable.
Its low cacheable identity/direct-map alias is removed, and the sealed MMIO page
table must contain exactly the virtio configuration mapping, the MSI-X mapping,
and this IOMMU mapping.

## Probe

The probe performs volatile reads of only these Intel VT-d registers:

- version;
- capability;
- extended capability;
- global status; and
- root-table address.

It requires a nonzero major version, a nonzero capability value, at least one
supported adjusted guest-address width, translation disabled in global status,
and a zero root-table address. Failure is fatal for this exact development
profile. The initial probe performs no MMIO writes and requires translation to
remain disabled before table construction. The same probe is repeated after
teardown and requires both translation and the root-table register to be clear.

After PCI discovery, the kernel resolves the discovered virtio-blk bus, device,
and function against the first unit's validated DMAR scopes. The Q35 profile
must contain an exact, single-path PCI endpoint scope for that source ID because
the unit does not set `INCLUDE_PCI_ALL`. Synthetic checks reject duplicate
endpoint scopes, nonzero reserved or PCI enumeration fields, unknown scope
types, and out-of-range device/function components. This proves which remapping
unit owns the device.

The [IOMMU Translation Hierarchy v0](iommu-translation-v0.md) contract is the
only current writer. It limits writes to the root-table, global-command,
context-command, capability-selected IOTLB invalidation, and one
capability-selected fault-record register. Fault retirement uses only the
record's write-one-to-clear valid bit. All status waits are bounded, and the
contract restores the initial root/translation state.

## Deliberate limits

This contract does not resolve multi-hop bridge scopes, provide page-granular
CPU write sealing for the register page, route an IOMMU fault interrupt, or
support another IOMMU implementation. Register authority remains kernel-only.
