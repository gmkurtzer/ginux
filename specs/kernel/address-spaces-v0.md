# Domain Address Spaces v0

**Status:** Hardware-verified construction primitive; not ready for workloads.

The kernel allocates a new x86-64 PML4 and subordinate page tables from the
persistent physical-frame allocator. PML4 entry zero shares the transitional
kernel identity map and remains supervisor-only. A separate lower-canonical
PML4 region contains one explicit user, writable 4 KiB mapping; every other
entry in that region is absent.

The QEMU gate switches CR3 to this root, verifies the user virtual address
aliases the owned physical frame, restores the kernel root, and confirms the
restoration. Page-table and data frames are distinct and remain owned for the
kernel lifetime.

This proves controlled construction and hardware activation, not process
isolation completeness. The transitional kernel mapping is still broadly
writable/executable. A second hardware-tested layout enables `EFER.NXE` and
provides a user-readable/executable, non-writable code page, an absent guard
page, and a user-readable/writable, non-executable stack page. The kernel mapping
remains supervisor-only in both roots.

Page-table frames are not reclaimed, there is no address-space identifier or
TLB shootdown protocol, and no ring-3 code runs yet. Separate IPC and syscall
pages plus an entry/exit path are required before launching the control plane.
