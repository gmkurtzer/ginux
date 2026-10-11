# Transitional x86-64 Paging v0

**Status:** Experimental; replaced before enabling untrusted workloads.

## Purpose

Remove the kernel's dependency on firmware-owned page tables while preserving the simple physical-address view needed for early initialization.

## Initial mapping

The kernel constructs and activates its own four-level x86-64 page tables:

- PML4 entry 0 addresses one page-directory-pointer table.
- The first four pointer-table entries address four page directories.
- Each page directory contains 512 identity-mapped 2 MiB pages.
- The resulting transitional address space identity maps physical addresses from 0 through 4 GiB.

Before writing CR3, the kernel verifies that its current code, stack, boot information, UEFI memory map, shutdown runtime-service entry, and page-table objects all fall within the mapped range. The kernel reads CR3 back and verifies that it identifies the expected PML4.

Interrupts remain disabled during and after the transition. They cannot be enabled until Ginux installs its own GDT, IDT, exception handlers, and interrupt-controller policy.

## Security limitations

This map is intentionally transitional:

- The entire mapped range is writable and executable.
- Physical address zero is mapped.
- There are no guard pages around the kernel stack.
- Kernel code, read-only data, mutable data, MMIO, and firmware regions are not separated by permissions.
- Addresses above 4 GiB are unavailable.
- The transition relies on the initial x86-64 UEFI environment using identity-addressable physical pointers before a virtual-address-map transition.

This address space must never host untrusted code. Later paging revisions must provide W^X permissions, an unmapped null page, stack guards, explicit MMIO mappings, direct-map bounds, kernel/user separation, and controlled address-space construction for domains.
