# IOMMU Register Probe v0

**Status:** Read-only Q35 Intel IOMMU capability discovery.

## Boundary

The initial Q35 profile exposes one Intel DMA-remapping hardware unit at the
fixed physical base `0xfed90000`. ACPI DMAR discovery must report that exact
base before the kernel uses the mapping. This target-profile constant keeps the
register page available during the early page-table transition; it is not a
general hardware-discovery mechanism.

The kernel maps exactly one 4 KiB register page in its bounded higher-half MMIO
window. The mapping is supervisor-only, read-only, uncached, and non-executable.
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
profile. The probe performs no MMIO writes, and the automated QEMU test requires
translation to remain disabled afterward.

After PCI discovery, the kernel resolves the discovered virtio-blk bus, device,
and function against the first unit's validated DMAR scopes. The Q35 profile
must contain an exact, single-path PCI endpoint scope for that source ID because
the unit does not set `INCLUDE_PCI_ALL`. Synthetic checks reject duplicate
endpoint scopes, nonzero reserved or PCI enumeration fields, unknown scope
types, and out-of-range device/function components. This proves which remapping
unit must eventually own the device; it does not turn translation on.

## Deliberate limits

This contract does not resolve multi-hop bridge scopes, allocate or install
root, context, or second-level page tables, issue invalidations, enable
translation, route or handle IOMMU faults, or confine device DMA. Bus-mastering devices can
therefore still address guest physical memory outside their software DMA lease.
The mapping will become writable only when a later contract defines the exact
register transitions, rollback behavior, and teardown invariants needed to
activate translation safely.
