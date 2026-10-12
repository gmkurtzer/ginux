# Transitional x86-64 Paging v0

**Status:** Experimental; replaced before enabling untrusted workloads.

## Purpose

Remove the kernel's dependency on firmware-owned page tables while preserving the simple physical-address view needed for early initialization.

## Initial mapping

The kernel constructs and activates its own four-level x86-64 page tables:

- PML4 entry 0 addresses one page-directory-pointer table.
- PML4 entry 256 addresses a separate page-directory-pointer table, providing
  a higher-half mapping beginning at `0xffff800000000000`.
- The first four pointer-table entries address four page directories.
- Most page-directory entries identity-map 2 MiB pages.
- Regions containing protected pages or executable UEFI descriptors are split
  into 4 KiB mappings.
- The resulting transitional address space identity maps physical addresses from 0 through 4 GiB.

The first four entries of the higher-half pointer table cover that same 4 GiB
physical range and are supervisor-only. They deliberately share the identity
map's page-directory hierarchy rather than using separately permissive leaves. Consequently, every
read-only, non-present, and NX decision—including guard pages, kernel image
permissions, and sealed page-table pages—is identical through both aliases.
The final higher-half pointer-table entry owns one bounded MMIO directory and
table. It maps only the contiguous span covering the four validated virtio
configuration regions as
supervisor-only, writable, NX, cache-disabled memory; it has no low identity
alias for the device's above-4-GiB physical BAR.
Every derived domain root copies both supervisor-only kernel PML4 entries, and
root validation rejects a missing, altered, or user-accessible direct-map entry.
The bounded Linux process proof applies the same rule to its private roots:
PML4 entries 0 and 256 are copied only from the active kernel root after their
presence and supervisor status are checked. A scheduler-selected child root is
then loaded into CR3 in kernel mode, verified through the inherited direct map,
and replaced with the exact prior root. Generation-stale roots and mismatched
kernel entries fail before CR3 is changed.
The kernel verifies both root entries and the first four matching pointer-table
entries after CR3 activation, excluding only the hardware-maintained Accessed
bit, and reads the PML4 page through both addresses.

Physical-to-direct-map conversion is checked as a half-open byte range. Empty,
overflowing, and beyond-4-GiB ranges are rejected. The physical-frame allocator
uses this conversion to zero every newly leased page, and its fresh/recycled
frame verifier reads and dirties allocator-owned pages only through the
higher-half alias. Page-table entries and CR3 continue to contain physical
addresses.

Physical page zero and the pages immediately below and above the bootstrap,
privilege-transition, and double-fault stacks are deliberately not present.
The stack allocations and both boundaries are page-aligned, and the page-table
builder reads back every protected PTE after loading CR3.

Before constructing entries with the NX bit, the kernel requires CPUID NX
support, enables `EFER.NXE`, and reads it back. Every 2 MiB identity mapping and
every newly split 4 KiB mapping starts non-executable. Only pages covered by a
validated executable PE section or the final UEFI map's
`EfiRuntimeServicesCode` descriptors have NX cleared. PE executable pages also
have `WRITABLE` cleared before CR3 activation. Every page in the loaded PE
image is split: only pages covered by a validated writable section retain
`WRITABLE`; image headers, read-only data, and section-alignment gaps are
read-only and NX. Static page-table pages are a stricter exception inside
writable image storage and are sealed read-only after construction. Loader
data outside the image, stacks, conventional memory, ACPI, MMIO, and
boot-service remnants remain non-executable. The kernel walks the full active
identity hierarchy after activation and again after the ring-3 workload,
requiring every executable leaf to have a PE/runtime-code justification, every
PE image leaf to match its effective write policy, and every unsplit region to
remain NX.

The signed manifest and measured control-plane bundle receive a direct range
check after activation and after the complete ring-3 workload. Every page
covering either byte range must be present, read-only, and non-executable; this
ties boot-policy immutability to hardware mappings rather than Rust constness.

Before writing CR3, the kernel verifies that its current code, stack, boot information, UEFI memory map, shutdown runtime-service entry, and page-table objects all fall within the mapped range. The kernel reads CR3 back and verifies that it identifies the expected PML4.

Immediately after activating the kernel root, the kernel sets and reads back
`CR0.WP`. Supervisor-mode writes must therefore respect read-only PTEs in both
the kernel root and every domain root derived from it. This is a prerequisite
for immutable kernel text and prevents privileged code from bypassing the
read-only user-code mapping through its user virtual address.

Before activation, every identity-map region containing the statically
allocated PML4, two PDPTs, four page directories, eight fine page tables, or
the MMIO directory and table is split into 4 KiB mappings. The kernel clears
`WRITABLE` on all seventeen table pages,
loads CR3, enables `CR0.WP`, and reads every leaf PTE back. It repeats that
readback after the complete ring-3 scheduling and fault/restart workload. The
active mapping hierarchy is therefore not writable through its identity alias;
dynamically allocated domain page tables remain writable while their address
spaces exist.

Interrupts remain disabled during and after the transition. They cannot be enabled until Ginux installs its own GDT, IDT, exception handlers, and interrupt-controller policy.

## Security limitations

This map is intentionally transitional:

- PE executable sections are read-only, but UEFI runtime-service code remains
  writable because the kernel does not own its implementation contract.
- Memory outside the loaded PE image is not yet divided into general
  kernel-owned read-only data, mutable data, MMIO, and firmware mapping classes.
- Most identity-map aliases remain writable, so this revision does not yet make
  kernel text or user-code physical frames immutable through every alias.
- General physical addresses above 4 GiB are unavailable; only the one bounded
  virtio MMIO window is explicitly mapped.
- Kernel static paging objects and the pre-transition boot-validation path still
  use identity addresses. Post-transition ACPI reads, xAPIC MMIO,
  allocator-owned frame scrubbing, and all dynamic domain page-table and
  payload-frame operations use the higher-half alias.
- The low identity mapping cannot be replaced by conventional Linux lower-half
  mappings until those kernel accesses migrate to the direct map.
- The transition relies on the initial x86-64 UEFI environment using identity-addressable physical pointers before a virtual-address-map transition.

Untrusted code executes only in domain roots where the identity, direct, and
MMIO mappings are supervisor-only. Later paging revisions must replace the
firmware shutdown dependency, generalize explicit MMIO ownership and unmapping,
and define permissions for dynamically loaded kernel modules.
