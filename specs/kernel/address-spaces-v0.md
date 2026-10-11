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
TLB shootdown protocol, and the kernel mapping remains transitional.

## Ring-3 entry proof

The kernel's GDT includes user code/data descriptors and TSS RSP0 names the
bootstrap kernel stack. A DPL3 interrupt gate at vector `0x80` accepts two
bring-up call numbers. The QEMU program enters CPL3 through `IRETQ`, issues a
ping call, and verifies the returned value after `IRETQ` resumes the same user
context. It then issues an exit call, which deliberately abandons the saved
user frame and returns on a kernel-owned stack. The kernel restores its original
CR3. The entry stub preserves every non-result general-purpose register.

This is not yet a stable syscall ABI or scheduler. It has no arguments,
preemption, signal model, IPC page, or fault-containment policy. The `INT 0x80`
mechanism is a bring-up gate; the eventual ABI may use `SYSCALL/SYSRET` with a
safe fallback.
