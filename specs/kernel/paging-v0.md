# Transitional x86-64 Paging v0

**Status:** Experimental; replaced before enabling untrusted workloads.

## Purpose

Remove the kernel's dependency on firmware-owned page tables while preserving the simple physical-address view needed for early initialization.

## Initial mapping

The kernel constructs and activates its own four-level x86-64 page tables:

- PML4 entry 0 addresses one page-directory-pointer table.
- The first four pointer-table entries address four page directories.
- Most page-directory entries identity-map 2 MiB pages.
- Regions containing protected pages are split into 4 KiB mappings.
- The resulting transitional address space identity maps physical addresses from 0 through 4 GiB.

Physical page zero and the pages immediately below and above the bootstrap,
privilege-transition, and double-fault stacks are deliberately not present.
The stack allocations and both boundaries are page-aligned, and the page-table
builder reads back every protected PTE after loading CR3.

Before writing CR3, the kernel verifies that its current code, stack, boot information, UEFI memory map, shutdown runtime-service entry, and page-table objects all fall within the mapped range. The kernel reads CR3 back and verifies that it identifies the expected PML4.

Immediately after activating the kernel root, the kernel sets and reads back
`CR0.WP`. Supervisor-mode writes must therefore respect read-only PTEs in both
the kernel root and every domain root derived from it. This is a prerequisite
for immutable kernel text and prevents privileged code from bypassing the
read-only user-code mapping through its user virtual address.

Before activation, every identity-map region containing the statically
allocated PML4, PDPT, four page directories, or eight fine page tables is split
into 4 KiB mappings. The kernel clears `WRITABLE` on all fourteen table pages,
loads CR3, enables `CR0.WP`, and reads every leaf PTE back. It repeats that
readback after the complete ring-3 scheduling and fault/restart workload. The
active mapping hierarchy is therefore not writable through its identity alias;
dynamically allocated domain page tables remain writable while their address
spaces exist.

Interrupts remain disabled during and after the transition. They cannot be enabled until Ginux installs its own GDT, IDT, exception handlers, and interrupt-controller policy.

## Security limitations

This map is intentionally transitional:

- The entire mapped range is writable and executable.
- Kernel code, read-only data, mutable data, MMIO, and firmware regions are not separated by permissions.
- Most identity-map aliases remain writable, so this revision does not yet make
  kernel text or user-code physical frames immutable through every alias.
- Addresses above 4 GiB are unavailable.
- The transition relies on the initial x86-64 UEFI environment using identity-addressable physical pointers before a virtual-address-map transition.

This address space must never host untrusted code. Later paging revisions must provide W^X permissions, an unmapped null page, stack guards, explicit MMIO mappings, direct-map bounds, kernel/user separation, and controlled address-space construction for domains.
