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
provides a user-readable/executable, non-writable code page and two
user-readable/writable, non-executable stack pages, each preceded by an absent
guard page. The kernel mapping remains supervisor-only in both roots.

Page-table frames are not reclaimed, there is no address-space identifier or
TLB shootdown protocol, and the kernel mapping remains transitional.

## Ring-3 entry proof

The kernel copies only the manifest-measured control-plane bundle into the
read-only executable page. Its GDT includes user code/data descriptors and TSS
RSP0 names a dedicated guarded privilege stack. A DPL3 interrupt gate at vector
`0x80` accepts five bring-up call numbers. The QEMU program enters CPL3 through
`IRETQ`, issues a ping call, and verifies the returned value after `IRETQ`
resumes the same user context. Context 1 yields to context 2, whose deliberate
invalid-opcode fault is contained by the scheduler. Context 1 then resumes and
exits. The final exit deliberately abandons the saved user frame and returns on
the suspended kernel stack. The kernel restores its original CR3. The entry
stub preserves every non-result general-purpose register.

The two proof contexts use separate PML4 roots and separate physical code and
stack frames while retaining identical virtual addresses. Before ring-3 entry,
the kernel activates each root and observes a different marker through the
same stack virtual address. Scheduler dispatch then reloads CR3 on both
cross-context transitions and reads it back before returning to userspace.

The current measured payload additionally proves capability enforcement: a
console-only grant is denied for a host-control operation, while the appropriate
manifest-derived grant succeeds.

This is not yet a stable syscall ABI. It has no structured arguments,
preemption, signal model, IPC page, or complete fault-containment policy. The `INT 0x80`
mechanism is a bring-up gate; the eventual ABI may use `SYSCALL/SYSRET` with a
safe fallback.
The current bundle is raw position-fixed machine code; a bounded executable
container format and relocation policy are still required.
