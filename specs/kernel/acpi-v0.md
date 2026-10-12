# ACPI Platform Discovery v0

**Status:** Read-only discovery through the higher-half direct map.

Starting from the validated UEFI RSDP, the kernel prefers the XSDT when
available and otherwise uses the RSDT. It bounds and checksums the root table,
then locates and validates exactly one Multiple APIC Description Table (MADT)
and one DMA Remapping Table (DMAR).

The MADT parser records:

- enabled or online-capable local-APIC and x2APIC processor entries;
- the effective local-APIC physical address, including an address override;
- the number of I/O APICs; and
- the first I/O APIC's physical address and global-interrupt base.

All physical ranges must fit inside the current 4 GiB direct-map aperture.
The pre-paging boot path validates the RSDP through its UEFI identity pointer;
after the kernel activates its own CR3, discovery translates the RSDP and every
SDT byte range to a checked supervisor-only higher-half pointer. Tables are
limited to 1 MiB, root entry counts are bounded, every variable MADT entry must
make forward progress and remain inside the table, and all arithmetic is
checked. Unknown entry types are skipped by their validated length.
In xAPIC mode, the MADT local-APIC address must agree with the enabled base in
`IA32_APIC_BASE` before the kernel accesses APIC MMIO.

The Q35 profile includes QEMU's Intel IOMMU with interrupt remapping disabled.
The DMAR parser requires a 32- through 64-bit host address width, reserved-zero
fixed fields, and at least one segment-zero DMA-remapping hardware unit. Every
remapping structure has a bounded nonzero length. Each hardware-unit record
has known flags, a reserved-zero byte, a nonzero page-aligned register base
inside the 4 GiB direct-map aperture, and bounded device scopes whose PCI paths
are validated device/function pairs. Scope types, reserved fields, PCI
enumeration identifiers, path components, duplicates, and the fixed endpoint
capacity are checked. The kernel records the number of units, address width,
first register base, whether the first unit uses the include-all flag, and up to
64 directly addressed PCI endpoint source IDs from that unit.
Unknown remapping-structure types are skipped only after their length is
validated. Synthetic zero-length, truncated, zero-base, unknown-flag, trailing,
and malformed-scope records must all be rejected during every boot.

The discovered first-unit register page is consumed under the separate
[IOMMU Register Access v0](iommu-v0.md) contract. This ACPI parser does not
retain ACPI-reclaimable pages, enumerate interrupt-source overrides, initialize
secondary processors, access an I/O APIC, or execute AML. It also does not
resolve a nested bridge path. The validated Q35 endpoint scope and register base
feed the separate bounded translation hierarchy; broader topology support
remains future work.
