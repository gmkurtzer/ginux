# ACPI Platform Discovery v0

**Status:** Read-only discovery under the transitional identity map.

Starting from the validated UEFI RSDP, the kernel prefers the XSDT when
available and otherwise uses the RSDT. It bounds and checksums the root table,
then locates and validates the Multiple APIC Description Table (MADT).

The MADT parser records:

- enabled or online-capable local-APIC and x2APIC processor entries;
- the effective local-APIC physical address, including an address override;
- the number of I/O APICs; and
- the first I/O APIC's physical address and global-interrupt base.

All physical ranges must fit inside the current 4 GiB identity map. Tables are
limited to 1 MiB, root entry counts are bounded, every variable MADT entry must
make forward progress and remain inside the table, and all arithmetic is
checked. Unknown entry types are skipped by their validated length.
In xAPIC mode, the MADT local-APIC address must agree with the enabled base in
`IA32_APIC_BASE` before the kernel accesses APIC MMIO.

This revision does not retain ACPI-reclaimable pages, enumerate interrupt-source
overrides, initialize secondary processors, access an I/O APIC, or execute AML.
