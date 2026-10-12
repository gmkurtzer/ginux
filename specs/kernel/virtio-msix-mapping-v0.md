# Virtio MSI-X Mapping v0

**Status:** Kernel-only M6 masked-programming profile.

## Purpose

This contract makes the previously discovered virtio-blk MSI-X table and
pending-bit array safely addressable by the kernel, then programs one exact
masked message entry while MSI-X remains disabled. It does not deliver an
interrupt.

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

## Masked programming contract

The kernel reserves vector 33 for the first device proof and programs table
entry zero for fixed delivery to the discovered bootstrap local-APIC ID. The
transaction:

1. rejects an out-of-range table index, an exception vector, and the userspace
   syscall vector without changing any entry byte;
2. confirms the function-level MSI-X enable bit is clear;
3. masks the selected entry before changing its message fields;
4. writes the architectural `0xfee00000 | APIC_ID << 12` message address, zero
   high address, and vector 33 data; and
5. reads back the complete entry and reconfirms function-level MSI-X remains
   disabled.

The entry stays masked after the proof. Invalid requests are validated before
the first MMIO write.

## Authority and limitations

The mapping is retained only by the kernel. It is not inserted into a service
address space or represented by a userspace capability. This milestone does
not install an IDT gate, bind the virtio queue to the entry, clear the entry
mask, set the MSI-X enable bit, handle an interrupt, or replace virtio
completion polling. Those operations require a separate routing and delivery
contract; the kernel-owned bootstrap implementation is specified by
[Virtio MSI-X Interrupt v0](virtio-msix-interrupt-v0.md).

BAR aperture sizes are still trusted from the pinned QEMU machine profile;
the kernel does not destructively probe them during bootstrap.
