# Domain Address Spaces v0

**Status:** Hardware-verified construction primitive; not ready for workloads.

The kernel allocates a new x86-64 PML4 and subordinate page tables from the
persistent physical-frame allocator. PML4 entry zero shares the transitional
kernel identity map, and entry 256 shares its higher-half direct-map alias;
both remain supervisor-only. A separate lower-canonical PML4 region contains
one explicit user, writable 4 KiB mapping; every other entry except the shared
direct-map entry is absent.

All allocator-owned page-table and payload frames are constructed, walked,
validated, populated, cleared, and inspected through checked higher-half
direct-map pointers. Their leases, CR3 values, and page-table entries continue
to carry physical addresses. The hardware alias proof writes through the user
virtual mapping while the kernel observes the same physical frame through the
direct map, so successful QEMU completion proves both translations agree.

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

A separate Linux ELF proof builds a private lower-half hierarchy for
the conventional `0x400000` and `0x401000` segment pages. It clones the
surrounding supervisor identity leaves, replaces only those pages with owned
user RX and RW/NX frames, preserves the supervisor-only direct-map entry, and
rejects target overlap with the loaded kernel image, loads CR3, observes both
user pages through their declared virtual addresses, and performs one bounded
CPL3 entry/return on a distinct RW/NX stack page with supervisor-only neighbors
before restoring the kernel root. General Linux process execution and collision
policy remain separate milestones.

That Linux proof also reserves four initially absent leaves over
`0x40b000..0x40f000`. Its bounded anonymous mapping call rounds a nonzero
length up to one through four pages, validates the resulting RW/NX request,
and allocates exact generation-bound frames before installing any leaf. The
lowest-free placement policy and exact
`MAP_FIXED_NOREPLACE` placement share the same overlap preflight; fixed overlap
returns `EEXIST` before frame allocation. Unmap accepts a page-aligned window
address and nonzero length rounded to the selected subrange,
clears each occupied leaf, invalidates its local translation, and scrubs and
releases every selected frame before reuse. While active, mapped subranges can
be irreversibly sealed read-only after one validation pass; copy permissions
change with the hardware
leaves, and unmap accepts either state. Bounded discard validates the same owned
leaf and zeros its frame in place, preserving whichever writable state the
mapping currently has.
The matching residency query reports each requested page only while it is
active and does not walk or disclose addresses outside the fixed window. The
gate observes all four leaves absent after ring-3 exit. This is a single-core,
four-page lifecycle, not a general VMA allocator or TLB-shootdown protocol.

Anonymous `MAP_FIXED` replacement allocates and zeroes every fresh frame before
preflighting the complete selected leaf range. Each old leaf must either be
absent with no lease or match its exact generation-owned frame and current
write state. The kernel then replaces every selected PTE, invalidates the local
translations, rebuilds the bounded registry, and scrubs and releases the
displaced leases before returning. No userspace instruction runs during this
single-core transaction. File/shared replacement and cross-CPU shootdown remain
unsupported.

Completed proof address-space page-table and data frames are cleared and
returned through the allocator's generation-checked live-ownership ledger.
The boot gate also constructs both supported zero-thread auxiliary domains and
assigns each a seven-frame W^X address space under its exact domain generation.
It proves their page-table and user-frame leases are physically disjoint and
that domain discard is rejected while any frame remains live. Thread admission
remains zero until each validated root and every reachable frame match that
generation in the allocator ledger. Binding raises each stored thread limit to
one; the proof reserves one thread per domain, rejects a second in either, and
refuses to unbind either space until both reservations are released. It then
releases the first space while requiring the second registry entry and all
seven of its frames to remain intact, releases the second, and discards both
unstarted domains without emitting lifecycle events. Both stale
domain/capability pairs are rejected. No scheduler context is queued, so this
is the concurrent transactional admission path rather than executable
auxiliary workloads.
The kernel retains the complete frame leases in a fixed-capacity registry keyed
by the exact auxiliary domain generation after a successful bind. Its capacity
is the domain subsystem's shared maximum auxiliary-service count, so identity
admission and address-space retention cannot diverge. Teardown
looks up that registry entry, refuses release while a thread remains attached,
then unbinds, zeroes, and returns all seven frames before removing the entry.
The proof requires the entry to remain present after a rejected early release
and requires the registry and allocator ownership count to be empty afterward.
The userspace stop path may remove a scheduler-ready auxiliary, detach its
thread reservation, and then release the same registry entry before publishing
retirement. It cannot release a running address space.
While an auxiliary remains staged, the control plane may copy one bounded
payload into the high end of its first private writable, non-executable stack
page. The scheduler publishes only the resulting target virtual address and
length in the saved context; no control-plane mapping or physical address is
shared with the workload.
During the primary bootstrap workload, the control plane creates a fresh
auxiliary generation and submits a payload-validating program through the
bounded management loader. The scheduler stages exactly one thread against the
registered root, keeps it staged across every scheduler handoff until explicit
start, dispatches it when the control plane voluntarily yields, and handles its
exit through the normal syscall path. The surviving control context then
resumes and consumes the resulting lifecycle status before it exits.
That path restores the kernel root, clears and releases all seven frames, and
retires the exact domain generation before returning to the boot verifier.
A second live auxiliary generation executes `UD2`. The architectural exception
path removes its scheduler context, records vector 6 with a zero non-page-fault
address, restores the kernel root, releases its complete hierarchy, and emits
the stop-only fault lifecycle record without attempting a restart.
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
identity entry, entry 256 must share its supervisor-only direct-map entry,
entry one must be a present writable user hierarchy, and all other entries must
be absent. Validation follows the user hierarchy through the direct map and
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
