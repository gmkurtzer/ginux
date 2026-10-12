# Virtio MSI-X Mapping v0

**Status:** Kernel-only M6 mapping profile.

## Purpose

This contract makes the previously discovered virtio-blk MSI-X table and
pending-bit array safely addressable by the kernel. It does not enable MSI-X,
program a vector, or deliver an interrupt.

## Mapping contract

The kernel derives one bounded span containing the validated MSI-X table and
pending-bit array. During the initial page-table transaction it places that
span at the opposite end of the existing supervisor-only MMIO window from the
virtio configuration mapping. Both mappings are:

- page-granular, writable, non-executable, supervisor-only, and cache-disabled;
- disjoint in physical and virtual address space;
- collectively verified against every entry of the sealed MMIO page table;
- bounded so neither mapping can translate an address from the other; and
- installed before CR3 activation and static page-table write sealing.

QEMU places the MSI-X BAR below 4 GiB. The bootstrap identity and higher-half
direct maps would otherwise create cacheable aliases for that page. The kernel
splits the corresponding 2 MiB mapping and removes every 4 KiB MSI-X page from
both aliases before activating the page tables. Each boot verifies that the
aliases are absent and that the uncached mapping resolves the complete table
and pending-bit ranges.

## Authority and limitations

The mapping is retained only by the kernel. It is not inserted into a service
address space or represented by a userspace capability. This milestone does
not inspect or modify table entries, set the MSI-X enable bit, allocate an IDT
vector, program the local APIC destination, handle an interrupt, or replace
virtio completion polling. Those operations require a separate routing and
delivery contract.

BAR aperture sizes are still trusted from the pinned QEMU machine profile;
the kernel does not destructively probe them during bootstrap.
