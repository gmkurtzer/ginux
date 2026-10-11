# Domain Address Spaces v0

**Status:** Hardware-verified construction primitive; not ready for workloads.

The kernel allocates a new x86-64 PML4 and subordinate page tables from the
persistent physical-frame allocator. PML4 entry zero shares the transitional
kernel identity map and remains supervisor-only. A separate lower-canonical
PML4 region contains one explicit user, writable 4 KiB mapping; every other
entry in that region is absent.

The QEMU gate switches CR3 to this root, verifies the user virtual address
aliases the owned physical frame, restores the kernel root, and confirms the
restoration. Page-table and data frames are distinct and remain owned under
generation-tagged leases until the proof address space is released.

This proves controlled construction and hardware activation, not process
isolation completeness. The transitional kernel mapping is NX by default;
validated executable PE pages are read-only, while firmware runtime code is
still writable. A second hardware-tested layout provides a
user-readable/executable, non-writable code page and two
user-readable/writable, non-executable stack pages, each preceded by an absent
guard page. The kernel mapping remains supervisor-only in both roots.

Completed proof address-space page-table and data frames are cleared and
returned through the allocator's generation-checked live-ownership ledger.
The boot gate also constructs a real zero-thread auxiliary domain, assigns it a
seven-frame W^X address space under that exact domain generation, and proves
that domain discard is rejected while any frame remains live. Thread admission
remains zero until the validated root and every reachable frame match that
generation in the allocator ledger. Binding raises the stored thread limit to
one; the proof reserves exactly one thread, rejects a second, and refuses to
unbind until that reservation is released. It then unbinds, clears and releases
all frames, discards the unstarted domain without emitting a lifecycle event,
and rejects its stale domain/capability pair. No scheduler context is queued,
so this is the transactional admission path rather than an executable
auxiliary workload.
The kernel retains the complete frame leases in a fixed-capacity registry keyed
by the exact auxiliary domain generation after a successful bind. Its capacity
is the domain subsystem's shared maximum auxiliary-service count, so identity
admission and address-space retention cannot diverge. Teardown
looks up that registry entry, refuses release while a thread remains attached,
then unbinds, zeroes, and returns all seven frames before removing the entry.
The proof requires the entry to remain present after a rejected early release
and requires the registry and allocator ownership count to be empty afterward.
There is no
address-space identifier or TLB shootdown protocol, and the kernel mapping
remains transitional.

## Ring-3 entry proof

The kernel copies only the manifest-measured control-plane bundle into the
read-only executable page. Its GDT includes user code/data descriptors and TSS
RSP0 names a dedicated guarded privilege stack. A DPL3 interrupt gate at vector
`0x80` accepts the version-negotiated bring-up operations documented by the
control ABI. The QEMU program enters CPL3 through
`IRETQ`, issues a ping call, and verifies the returned value after `IRETQ`
resumes the same user context. A calibrated APIC one-shot preempts context 1
and switches to context 2, whose deliberate null-page write fault is contained
by the scheduler. Context 1 then resumes at its interrupted instruction. A
rearmed one-shot later dispatches the replacement service generation and
returns to context 1 after that service exits. The final control exit
deliberately abandons the saved user frame and returns on
the suspended kernel stack. The kernel restores its original CR3. The entry
stub preserves every non-result general-purpose register.

The two proof contexts use separate PML4 roots and separate physical code and
stack frames while retaining identical virtual addresses. Before ring-3 entry,
the kernel activates each root and observes a different marker through the
same stack virtual address. Scheduler dispatch reloads CR3 on the preemption
and later cross-context transitions and reads it back before returning to
userspace.
Every scheduler-triggered CR3 load first requires the target to equal the
current scheduler record, fall below the 4 GiB bootstrap ceiling, and have a
bounded PML4 shape: entry zero must exactly share the kernel's supervisor-only
identity entry, entry one must be a present writable user hierarchy, and all
other entries must be absent. Validation follows the user hierarchy and
requires exactly one non-large PDPT entry, one non-large directory entry, and
the fixed code/guard/stack/guard/stack leaf layout with W^X permissions. All
seven page-table and payload frames must be distinct, nonzero, and below 4 GiB;
every unmodeled entry must remain absent. The hardware readback still has to
match after the load. Before activation, all seven physical addresses must also
appear in the allocator's live ledger under the scheduler's exact current
domain generation. This prevents an internally corrupted or stale scheduler
root from silently widening a domain's mappings or borrowing another domain's
frames.
The scheduler also supplies the target context's saved instruction and stack
pointers at this boundary. The instruction must fall inside the fixed
read-only executable page, while the stack pointer must name the top or an
interior byte of one of the two writable, non-executable stack pages. Guard
pages and the one-past-end address above a stack are rejected. This is a
transitional fixed-layout policy, not a general virtual-memory-area model.

The one bounded service restart retains the service address space and immutable
code mapping, but not its writable stack contents. While the failed service CR3
is still active, the exception path clears and verifies the entire stack page
before a replacement domain or context is created. The seven frame leases are
then transferred atomically from the failed domain generation to the
replacement. Completion verifies the local address-space record against the
allocator's new owner and reclaims the frames using that owner. General
mutable-region inventory, shared ownership, and fresh-address-space restart
remain future work.

The current measured payload additionally proves capability enforcement: a
console-only grant is denied for a host-control operation, while the appropriate
manifest-derived grant succeeds.

This is not yet a stable syscall ABI. It has no structured arguments, periodic
preemption policy, signal model, IPC page, or complete fault-containment policy. The `INT 0x80`
mechanism is a bring-up gate; the eventual ABI may use `SYSCALL/SYSRET` with a
safe fallback.
The first pointer-bearing operations perform bounded reads and writes against
validated lower-canonical user ranges. They reject absent, supervisor-only, or
large-page mappings at every level; writes additionally require effective
writable permission, while reads stage the result before use. Each operation
also requires active CR3 to match the current scheduler record before walking
the mapping. It revalidates the fixed hierarchy and its exact generation-tagged
allocator ownership at each copy boundary rather than relying only on the most
recent CR3 activation check. The hardware workload verifies that a copy spanning a readable
page and an absent guard page is rejected before any bytes are consumed. This is not yet
a general copyin/copyout subsystem: mappings cannot mutate concurrently in the
single-core bootstrap, and there is no exception-table recovery for a race or
hardware fault during a copy.
The current executable remains position-fixed and has no relocation policy,
but its authenticated container supplies checked lengths and distinct control,
service, and restart entry offsets resolved from inline authenticated workload
labels.
