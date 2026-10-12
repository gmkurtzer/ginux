# Ginux

Ginux is an immutable, capability-secure, container-native operating system for AI infrastructure. The native host is intentionally small: an immutable kernel boots one constrained control-plane domain, which creates isolated services and containers through stable interfaces.

The first M6 storage primitive is a canonical two-sector persistent-volume
record. It preserves the older generation while preparing and flushing an
inactive slot, writes the commit marker last, and accepts the new generation
only after the final flush. Every QEMU boot proves recovery at all five commit
stages, rejects partial commit markers and corrupt records, falls back to the
older valid slot, and fails closed on generation exhaustion or ambiguity. See
[Persistent Volume Record v0](specs/kernel/persistent-volume-v0.md).
Control ABI v4 operation 18 exposes the live reference volume through signed
object 6. A write-only ring-3 service commits generation one and is then
faulted; its authority and address space are retired while the volume remains.
A fresh read-only service recovers the exact payload, cannot mutate it, and
also loses its attenuated handle at exit. This proves service-restart
persistence and authority separation; that live service path still uses its
in-memory reference backend. See
[Persistent Volume Service v0](specs/kernel/volume-service-v0.md).
The next M6 layer attaches a deterministic 1 MiB modern virtio-blk function
and performs bounded, read-only PCI configuration discovery. Every boot
validates its common, notification, ISR, and device capabilities and rejects
malformed copied configurations without enabling MMIO, bus mastering, DMA, or
queues. Discovery also requires one disabled MSI-X capability, validates its
vector count and memory-BAR-backed table and pending-bit ranges, and rejects
malformed interrupt metadata without mapping or programming it. See
[PCI Virtio Block Discovery v0](specs/kernel/pci-virtio-block-v0.md).
The validated MSI-X table and pending-bit array are now placed in a separate,
bounded supervisor-only uncached mapping. Their low-memory cacheable aliases
are removed before page-table activation, and the complete sealed MMIO table
is verified on every boot. The kernel atomically rejects invalid programming,
then writes one exact local-APIC message entry while keeping both that entry
masked and MSI-X disabled. See
[Virtio MSI-X Mapping v0](specs/kernel/virtio-msix-mapping-v0.md).
The kernel now temporarily routes table entry zero to local-APIC vector 33,
binds queue zero to that entry, and accepts a blank-sector read only after
exactly one MSI-X delivery plus complete used-ring and payload validation.
Teardown masks and disables MSI-X, revokes bus mastering, resets the device,
verifies the APIC vector is idle, and removes the temporary IDT gate. See
[Virtio MSI-X Interrupt v0](specs/kernel/virtio-msix-interrupt-v0.md).
The validated configuration regions are now mapped through a bounded,
uncached, supervisor-only higher-half window. The kernel forces PCI bus mastering off,
resets the device, accepts only modern virtio plus flush support, confirms one
or more queues, and returns the device to reset without setting `DRIVER_OK` or
providing DMA memory. See
[Virtio Block Initialization v0](specs/kernel/virtio-block-init-v0.md).
The real device path uses one generation-owned 4 KiB DMA lease for a bounded
split ring. It first reads reserved blank sector two, validating capacity,
completion length, status, and buffer guard bytes. The same queue then supports
bounded sector reads, writes, and flushes and binds sectors zero and one to the
persistent-volume record. The automated gate creates a fresh disk, initializes
generation one, boots a second VM against the same image, and recovers the exact
record without rewriting it. A third boot corrupts the committed slot and
proves nonempty invalid media is rejected without modification after safe
device and DMA teardown. Teardown revokes bus mastering, resets the device, and
scrubs the lease before release. This trusted-QEMU bootstrap proof has no IOMMU
confinement and does not yet place the device path in an isolated service. See
[Virtio-backed Persistent Volume v0](specs/kernel/virtio-volume-v0.md) and
[Virtio Block Read v0](specs/kernel/virtio-block-read-v0.md).
The first device-isolation step delegates manifest object 7 with read/write
authority, then attenuates it into separate stop-only ring-3 domains. A
read-only observer sees reset status and faults on mutation. A control service
with a write-only capability uses its necessarily read/write x86 leaf to
require `FLUSH` and `VERSION_1`, publish exactly those features, and complete
`FEATURES_OK`, select queue zero, and set its bounded size to eight before
deliberately faulting. The control domain owns one additional RW/NX DMA page,
initializes it in ring 3, and publishes its exact base, `+0x100`, and `+0x200`
offsets as the descriptor, available, and used-ring addresses. After the kernel
arms PCI bus mastering, the service enables queue zero and sets `DRIVER_OK`, but
first constructs an exact three-descriptor sector-two read chain. It publishes
descriptor head zero by advancing the available index to one. The kernel maps
only the distinct 4 KiB page containing queue zero's validated two-byte
notification register and binds that page to the teardown record. Ring 3
binds queue zero to MSI-X entry zero and notifies it. Control ABI v4 operation
19 verifies the write-only object-7 capability, active queue, exact DMA lease,
domain generation, and request generation before blocking the service. The
kernel unmasks its private vector 33, wakes only that saved generation, and
remasks delivery before returning to ring 3. The proof rejects a stale request
and a duplicate wait, then validates used ID zero, length 513, successful status,
and the blank 512-byte sector-two payload. The kernel then advances the exact
expected request generation, clears only the consumed per-request interrupt
state, and the same service reuses descriptor head zero for a second
notify/block/wake cycle. It then writes a fixed pattern to reserved sector
three, completes a negotiated virtio flush, poisons its DMA buffer, and reads
the sector back through generations three through five. The first automated VM
boot requires sector three to be fresh; the second VM boot against the same
image requires the isolated service to recover the previously flushed marker.
On its deliberate fault, the
registered device hook masks and disables MSI-X, validates the active binding, disables bus
mastering, resets the device, retires the temporary interrupt gate, and only then validates every byte of the now-stable
completed DMA layout before the page is scrubbed and released;
release is rejected
unless quiescence completed. Each service loses its capability, mapping, and
owned frames. A calibrated 100 ms APIC one-shot wakes a request whose device
interrupt never arrives; successful MSI-X delivery cancels it. This is the
isolated durability primitive, not yet the two-slot volume transaction. A general
multi-request ring, concurrent requests, and IOMMU confinement remain
unimplemented. See
[Block-device Control Grant v0](specs/kernel/device-grant-v0.md) and
[Isolated DMA Lease v0](specs/kernel/device-dma-lease-v0.md), plus
[Isolated Virtio Interrupt Wait v0](specs/kernel/isolated-virtio-interrupt-v0.md)
and [Isolated Virtio Durability v0](specs/kernel/isolated-virtio-durability-v0.md).

The first system-container storage prerequisite is a deterministic, bounded
ASCII `newc` initramfs independently bound by size and SHA-256 in the signed
boot manifest. The kernel validates its measurement, normalized metadata, safe relative
paths, uniqueness, alignment, an executable `init`, and an exact trailer while
rejecting malformed variants during every QEMU boot. The generated archive now
contains the build-produced Linux main, interpreter, two DSOs, two static
exec targets, and a separately linked `/sbin/init` candidate. The validated
`profiles/minimal-system-v0/profile.tsv` matrix is embedded into the signed
root and records ready, partial, and missing requirements without treating
partial behavior as compatibility. The kernel resolves the candidate only
from the authenticated index and validates its two-page RX/one-page RW ELF
plan. A dedicated measured exec workload launches it as PID 1, verifies its
identity, opens `/dev/console`, emits `GINUX_SYSTEM_INIT_V0`, and remains in an
untimed supervisor wait until the native control plane requests clean shutdown
while final process teardown closes all inherited descriptors. It enumerates
the immutable bounded `/dev`, proves `/dev/null` discard/EOF behavior,
`/dev/zero` zero-fill behavior, and the `/dev/tty` terminal alias before
publishing `DEVFS_READY`. Before
readiness it enumerates `/proc` and `/proc/1`, then reads a bounded read-only
`/proc/1/status` view containing its exact name, running state, PID 1, and
parent PID 0. Its forked PID 2 appears dynamically with a zombie status and
parent PID 1, then disappears from `/proc` after `wait4` reaps it. It also
proves `/tmp` starts empty, creates the generation-bound
64-byte `ginux-ready` file, enumerates, writes, rereads, unlinks, and confirms
removal; unlink or final teardown scrubs the backing store. The kernel installs
the measured root as the sole
`ro,nosuid,nodev` mount in a generation-bound Linux mount namespace; PID 1
enumerates and reads `/proc/1/mountinfo` to verify that exact state. A separate
generation-bound user namespace maps container UID/GID 0 to outside ID 100000,
denies supplementary-group mutation, carries no Linux or native capabilities,
and is inherited by the fork child. PID 1 validates its read-only `uid_map`,
`gid_map`, and `setgroups` proc records. PID 1 also
proves identity, root, mount, kernel, namespace, raw-I/O, and BPF management
requests fail with `EPERM`, while an unknown syscall remains `ENOSYS` and its
credentials and immutable root remain unchanged. PID 1 installs a caught
`SIGUSR1`, blocks and queues it, observes it through `rt_sigpending`, consumes
it with `rt_sigtimedwait`, restores the inherited action, and separately proves
an ignored `SIGUSR2` is discarded without disturbing inherited signal state.
PID 1 then
invokes the public Linux `fork` ABI,
resumes an independently owned PID 2 under its private CR3, and reaps its
`exit(42)` status through `wait4` before publishing readiness. The kernel
copies the root into a zeroed, contiguous 32-frame allocation,
verifies source independence and the complete zero tail, and builds a bounded
pathname index whose 32-bit offsets resolve only inside the owned image. A
final indexed copy remains kernel-owned through
shutdown and is published to the control plane as read-only capability object
5. The kernel plans the interpreted main/interpreter/two-DSO graph directly
from those authenticated measured-root bytes, binding canonical names,
SONAMEs, dependency order, digests, and bounded non-overlapping load windows
without requiring equality to kernel-embedded ELF artifacts. Substituted,
reordered, and truncated graph members are rejected. Control ABI v4 provides bounded probe, exact-path metadata lookup, file
reads, and cursor-based direct-child enumeration; the ring-3 proof reads `init`
and enumerates `dev` while rejecting the wrong capability,
traversal, and an invalid output range. The Linux syscall profile now resolves
`/etc/hostname`, `/etc/os-release`, and `/etc/motd` from that same measured, retained index.
Linux file descriptors carry a bounded one-based index selector rather than a
kernel-specialized content identity, so the generic `os-release` file uses the
same read and metadata path as the executable graph. The ring-3 interpreter
validates exact contents and metadata and proves writable access is denied. It
also opens `/etc`, enumerates all three measured children, and verifies directory
metadata, EOF, cursor rewind, and atomic undersized-buffer rejection. The Linux `/` view enumerates every
direct measured child, including an explicit `/sbin`; the fixed 256-byte
staging buffer returns `sbin` on a second cursor page. Measured directory descriptors also carry index
selectors: `proc`, `sys`, `tmp`, and the otherwise unlisted `var` node open
without per-directory kernel identities; `dev`
retains its synthetic device entries. The measured `proc` node is populated by
the bounded synthetic PID 1 directory plus status, root-mount, and user-namespace records;
it is not yet a general procfs. The measured `tmp` node anchors a separate
generation-safe bounded tmpfs whose one admitted `ginux-ready` file is created,
enumerated, used, unlinked, and scrubbed at runtime; it is not a general tmpfs. A
bounded index-driven contract resolves any direct measured regular child below
an open measured directory; `motd` proves the syscall layer needs no
per-filename case. Normalized multi-component paths are bounded and `lib` is
opened relative to `/var`, while `../lib` is rejected before index lookup.
The publication index now admits 64 nodes and is exercised beyond its original
16-node limit. Open, metadata, and access checks share that lookup. The current native
`init` remains a format marker, not PID 1, but the Linux-visible `/init`,
and `/lib64/*` paths map to internal measured-root entries, while
`/bin/signal-probe` and `/bin/exec-second` are direct measured-root nodes. The
first is a dedicated two-page RX signal-runtime profile, leaving the full
five-page primary workload unchanged; after its caught and pending-signal
checks, it proves default `SIGPIPE` process termination and resource cleanup.
The complete dynamic
main/interpreter/two-DSO graph executes from those retained bytes. A bounded
single-threaded `execve` proof atomically replaces two independent static workloads and
the running interpreted `/init` graph with the selected measured executable, rebuilds up to eight bounded argument and eight environment entries
plus a deterministic Linux auxiliary vector in a 512-byte initial-stack image,
and validates every auxiliary pair in ring 3 for both static and interpreted
replacement. It allocates and materializes up to eight
measured ELF pages only during the transaction. It derives entry and W^X mappings from that
runtime selection instead of a boot-predeclared target template, replaces the
runtime-owned active-image record, removes the old image pages, scrubs and
releases their exact generation-tagged leases as part of the mapping commit,
and applies close-on-exec and
signal/alternate-stack transition rules, installs the replacement process
name, and exits without reviving the old program. Stale-generation retirement
and a second release are rejected without mutation, while RELRO permission
changes remain synchronized with the active-image record. Generation-tagged replacement
frames are scrubbed and released after the process root becomes inactive, with
the prior live-frame count restored. Every successful exec materializes its
bounded initial stack in a fresh zeroed RW/NX page, replaces the exact old
stack lease in the rollback-capable image transaction, and reclaims the fresh
lease after the process root is inactive. The execution contract binds every active
page-table hierarchy frame to its exact allocator generation, verifies that
the live CR3 and all admitted Linux ranges traverse only that owned hierarchy,
and rejects a stale root lease without mutation. Every successful exec clones
the complete hierarchy into fresh generation-bound frames—five for the static
profile and six for the interpreted profile—applies and verifies the replacement
only in that inactive clone, switches CR3 after verification, and then scrubs
and retires the exact old hierarchy. The fresh tables are reclaimed after the
process root becomes inactive. Every successful exec also
replaces the bounded heap with a fresh zeroed RW/NX lease and resets the process
break to the fixed heap base. The old heap is scrubbed at commit and the replacement
lease is reclaimed after the process root becomes inactive. Scratch `mmap`
pages have no inactive reserve leases: each successful request allocates fresh
generation-bound frames, and complete or partial unmap scrubs and releases the
exact selected leases. Exec requires the scratch registry and its lease slots
to be empty.
Interpreted replacement installs the measured
interpreter first, lets it remap the authenticated main and both DSOs, reapplies
RELRO, pre-materializes fresh primary and sibling TLS leases from the complete
authenticated templates in the same transaction, restores FS to the fresh
primary mapping, replaces the guarded sibling stack with a fresh zeroed RW/NX
lease while preserving both guard pages, and retires the complete old
interpreter/main/DSO graph. The archive
is not yet a general mount or a system-container image.

The initial executable milestone is an x86-64 unified EFI image tested under QEMU. Its boot component passes a development-signed authority manifest and measured control-plane bundle, exits firmware boot services, and transfers control to a separate `no_std` kernel. The kernel establishes protected CPU and memory state, seals manifest-derived capabilities into one native domain, and runs the measured W^X control-plane payload at ring 3 through a capability-checked call gate. Calibrated APIC one-shots preempt the control context to run an isolated service and its replacement generation, while bounded IPC carries the request and reply. The control plane can also create, bounded-load, stage, cancel, and retire stop-only auxiliary workloads without placing third-party code in the kernel. A separate isolated service probes and configures link, IPv4, default-gateway, and one static-route record through the first manifest-authorized network-device contract in control ABI v2; a monotonic generation makes effective configuration changes observable and rejects stale whole-state applies. Distinct read-only and write-only services prove that observation and mutation authority can be delegated independently, and a faulted network service is retired without losing mediated state or panicking the kernel. Packet I/O and a general route table remain deliberately unimplemented. On Apple Silicon development hosts it uses QEMU TCG emulation. The same Q35/OVMF profile will use KVM acceleration on x86-64 Linux.

Native scheduler contexts retain a lower-canonical per-thread FS base alongside
registers and the address-space root. Yield, blocking, and timer preemption
capture it; every initial entry and resume programs and reads back the selected
value before ring 3. Native bootstrap payloads currently use zero, while the
Linux TLS contexts use nonzero FS bases outside this scheduler path.

M4 begins with a strict static x86-64 ELF64 parser profile. It scans up to nine
program headers in any order, normalizes executable, optional read-only, and
writable loads,
and accepts validated optional `PT_PHDR`, `PT_GNU_RELRO`, non-executable
`PT_GNU_STACK`, bounded `PT_INTERP`, `PT_TLS`, and one-, two-, three-, four-,
five-, or six-entry
relocation `PT_DYNAMIC` declarations. Three-relocation images may place the eager
`R_X86_64_JUMP_SLOT` record in a separate one-entry `DT_JMPREL` table described
by `DT_PLTRELSZ` and `DT_PLTREL`; the six-relocation runtime main uses two
entries in that split table. The symbol profile validates a two- or three-entry
symbol-table prefix, bounded string table, bounded SysV `DT_HASH` or
one-bucket/one-word-bloom `DT_GNU_HASH`, and up to two global function symbols
used by paired `R_X86_64_GLOB_DAT` and `R_X86_64_JUMP_SLOT` records. The parser admits an ordered list of at most two distinct
`DT_NEEDED` names and rejects overflow or duplicates. A separate LLD-produced
metadata fixture proves both names and the second DSO's `DT_SONAME`. Independent
authority records bind both ordered names to their SHA-256 digests and canonical
load biases, with pairwise overlap rejection. Both dependencies are mapped and
self-relocated by the userspace interpreter. The main image's two undefined
symbols are matched by complete name against one provider in each authenticated
`ET_DYN` dependency.
Images whose expanded
program-header table covers the entry point are rejected. A bounded proof maps
a resolved `ET_EXEC` main image, relocated `ET_DYN` interpreter, and two
relocated dependencies into separate W^X regions, builds the dynamic startup
stack, and enters the interpreter first.
The linker-produced main also carries an authenticated read-only `PT_TLS` view
of its writable load: eight initialized bytes, sixteen total bytes, and
eight-byte alignment. The parser caps both TLS size and alignment at 64 bytes,
requires matching file and virtual displacement inside the writable load, and
rejects malformed or duplicate declarations. Planning preserves all three
templates and applies the main and dependency biases. The interpreted execution
proof allocates a separate owned RW/NX page and constructs a deterministic
three-module static layout: the auxiliary DSO occupies `TP-48..TP-32`, the
primary DSO occupies `TP-32..TP-16`, the main occupies `TP-16..TP`, and all
initialized `.tdata` regions retain zeroed `.tbss` tails. A minimal 16-byte TCB
contains its own address. FS is activated before the interpreter,
and the main preinitializer refuses to advance unless its TLS template, zero
tail, and self pointer are correct. Both DSOs' direct initializers also refuse
to advance unless their initialized and zero-filled regions are correct.
The main accesses name the hidden TLS symbols
through `R_X86_64_TPOFF32`; LLD resolves both offsets without adding a dynamic
relocation class, proving the toolchain layout matches the loader. Teardown
scrubs and releases the TLS frame and restores the supervisor's prior FS base.
The kernel admits only the live static-block-plus-TCB extent to syscall copy
validation; the interpreter compares the initialized `TLS_OK!` value directly
through FS-relative storage before invoking the preinitializer, while one-byte
underflow and overflow ranges are rejected internally.
Each DSO names its hidden TLS template through one toolchain-emitted
`R_X86_64_GOTTPOFF` object reference. LLD converts that reference into a
bounded `R_X86_64_TPOFF64` GOT relocation and marks the object `DF_STATIC_TLS`.
The userspace interpreter validates both exact relocation shapes and installs
`-32` for the primary DSO and `-48` for the auxiliary DSO; each initializer
loads its value from its GOT before using it as an FS-relative offset. Malformed
types, symbol indexes, addends, alignments, and targets are rejected. TLS
modules beyond the bounded graph, dynamic TLS, and per-thread allocation
remain unimplemented.
A reusable bounded planner computes each aligned block, signed thread-pointer
offset, live extent, and TCB fit from the authenticated templates. It rejects
zero or non-power-of-two alignments, undersized declarations, misaligned thread
pointers, arithmetic underflow, and page overflow; the execution path consumes
that plan rather than maintaining separate layout arithmetic.
The same reusable materializer revalidates the plan before mutation, clears the
whole owned page, copies every `.tdata` template, preserves zero-filled tails
and padding, and writes the self-referential TCB. A substituted plan is rejected
without changing the destination page.
The interpreted address space also allocates a second independently owned
RW/NX TLS page from that same authenticated plan. Its identical static blocks
have a distinct self-referential TCB and thread pointer. The active syscall-copy
contract admits only the initial thread's live TLS extent and rejects the
sibling extent even while both mappings exist. Teardown verifies the sibling
page is scrubbed before releasing both frame leases and restoring the allocator
count. The bounded `clone(2)` profile consumes this preallocated sibling; it
does not yet allocate arbitrary threads, and code in one process continues to
share that process's address space.
FS base and the syscall-copy TLS extent are represented together as a checked
execution context. Before ring-3 entry, the kernel installs the sibling context,
reads back its FS base, validates its self pointer, proves copy admission moved
from the initial extent to the sibling extent, and then restores and revalidates
the initial context. Bootstrap is single-core with interrupts disabled during
this transition; scheduler-driven Linux TLS switching remains future work.
A two-slot Linux thread table now binds monotonically assigned TIDs and
generation-tagged internal handles to those checked TLS contexts. The initial
thread runs as TID 1; the boot proof reserves TID 2 for the sibling context,
switches to it and back, rejects a third live reservation, retires it, reuses
the slot as TID 3 with a new generation, and rejects the stale handle. The
ring-3 `gettid`, `set_tid_address`, and robust-list lookup paths resolve the
current table entry rather than embedding TID 1. Process exit retires the
initial entry and teardown requires the complete table to be empty before
resetting the process-local TID sequence.
The sibling slot also owns a separate, zeroed, RW/NX page between two unmapped
guard pages. Its exact bounds follow the slot across TID 2 retirement and TID 3
reuse. The current-thread syscall-copy contract rejects that whole page. Thread
retirement scrubs the page while running on the kernel stack, and teardown
verifies it remains zero before releasing its frame. This establishes
bounded stack ownership and cleanup, not memory isolation within the shared
process address space.
An internal clone constructor now accepts exactly the bounded shared-VM,
shared-files, shared-signal-handler, thread-group, SysV-semaphore, TLS,
parent-TID, child-set-TID, and child-clear-TID flag set. It requires the reserved stack top,
the sibling TLS pointer, aligned TID words inside the parent's writable stack,
and a safe return RIP/RFLAGS pair. It records a child context that resumes after
the eventual syscall with `rax = 0`, while the parent result is the child's TID.
Malformed flags and stack bounds fail without changing the slot, and retirement
clears the saved context before generation reuse. An internal publication proof
then exercises the current-thread copy boundary: denial rolls the entire child
execution state back to empty, while success writes the child TID to distinct
parent- and child-TID words only after the complete context exists. The proof restores the
initial stack word and retires the unpublished child. The ring-3 path exposes
the same transaction as syscall 56 for one fixed sibling context.
The Linux `SYSCALL` entry now captures and restores `rbx`, `rbp`, and
`r12`-`r15` alongside all six syscall argument registers. Existing argument
indices remain unchanged for dispatch. This makes the complete preserved user
register set available on the kernel stack when `sched_yield` or blocking
`FUTEX_WAIT` switches between the bounded parent and child.
The assembly layout is now represented by one `repr(C)` syscall-frame type
rather than an untyped word slice. Clone construction copies all twelve saved
registers into generation-bound per-thread storage alongside RIP, RSP, RFLAGS,
RAX, and FS base. A checked selector reconstructs the complete return context
only for the matching live handle; retired handles and newly reused slots have
no selectable context. The live yield, futex-wait, and child-exit paths use that
selection to redirect `SYSRETQ`.
A checked installer now validates the selected child first, switches the
generation-tagged running state and FS/copy context, and then fills an exact
return frame. Because all fallible work precedes the frame writes, a stale
handle leaves every output word and the running parent unchanged. The boot
proof installs the child, observes that copy admission moved to its TLS, and
restores the parent with the reverse admission decision. The live thread paths
use the same installer for the assembly-owned return frame.
The real ring-3 `sched_yield` path now round-trips through that typed live
frame. With interrupts disabled, the handler validates and reinstalls the
current TLS context, writes all twelve registers plus RIP/RSP/RFLAGS into the
assembly-owned return state, reads the result back, and resumes successfully
through `SYSRETQ`.
The interpreted runtime now calls bounded x86-64 `clone(2)` with exactly
`CLONE_VM|CLONE_FS|CLONE_FILES|CLONE_SIGHAND|CLONE_THREAD|CLONE_SYSVSEM|CLONE_SETTLS|CLONE_PARENT_SETTID|CLONE_CHILD_CLEARTID|CLONE_CHILD_SETTID`.
The call accepts only the authenticated sibling TLS pointer, guarded stack top,
and distinct aligned parent- and child-TID words in the main writable mapping.
Both words receive the monotonically allocated child TID before either thread
returns to ring 3; the parent receives that same value, while the child resumes
after the syscall with `rax = 0`. Each child immediately requires its live
`gettid()` result to match the published child-TID word and requires `FS:[0]`
to contain its own sibling TLS/TCB pointer. Signal masks are also thread-owned:
each clone snapshots the parent's current mask, context switches save and
restore the active thread's mask with its scheduler slot, and retirement clears
that state. Each measured child proves it inherited blocked `SIGUSR1`, unblocks
it independently, and leaves the parent's blocked mask unchanged. The parent first yields to the
child; the child yields back, and the resumed parent yields to the same child
again. While that child is ready, a second live `clone` attempt returns
`EAGAIN` without replacing it. All three cooperative switches return zero in their resumed contexts.
The child then blocks in `FUTEX_WAIT` on the parent-TID word and restores the parent. An
explicit `FUTEX_WAKE` makes the child ready. The parent then waits on the
child-TID word, selecting the child again; its first wait resumes with zero.
The child calls `exit(42)`, clears only its child-TID word, wakes the matching
parent, leaves the parent-TID word intact, retires itself, and restores the
parent's registers, FS base, and copy ownership. Both resumed waits return
zero. The complete handshake then runs a second time through the reused slot;
the kernel requires the guarded stack to be fully scrubbed first and the new
child TID to exceed the first. Before the first successful creation, a live
syscall attempt proves that malformed flags return `EINVAL`, retires its
provisional slot, and permits the valid clone to follow. After both children
retire and the main RELRO page is sealed, a parent-TID write into that page
returns `EFAULT`; final teardown proves the failed transaction leaked no slot.
Occupied capacity returns `EAGAIN` without mutating the live slot. This remains a fixed
two-slot, two-sequential-child proof without arbitrary allocation, more than
one live child, asynchronous preemption, multiple waiters, requeue
operations, or a general scheduler.
A separate four-slot process-lifecycle table now establishes the ownership
model required before process-style creation is exposed. Internal identities
bind slot, generation, and monotonic PID; every child has a live recorded
parent. Creation reserves an invisible `Preparing` record, and only a validated
distinct generation-checked resource bundle can publish it as running; each
bundle contains four through six page-table leases and one through 41
private-page mapping records. Every record binds its virtual address, frame
lease, and W^X permissions, and publication requires an exact match with every
user leaf in the complete hierarchy while rejecting overlap in any frame.
Publication also requires a saved userspace context whose RIP resolves inside
an executable recorded page, whose RSP resolves to the top of a writable page,
and whose flags and FS base are safe. Exit first moves the complete bundle to a non-runnable `Exiting` state, and zombie
publication is denied until one prevalidated operation scrubs and releases
every lease. Only the live parent can reap it. The QEMU proof uses three real
six-frame address spaces, rejects stale leases, partial aliases, substituted
address or permission metadata, and invalid execution contexts, aborts one reservation, creates
and reaps two children through one reused slot, retires PID 1, and restores the
allocator count. The live PID 1 `fork` and `wait4` path now uses this table;
descriptor, credential, signal, mount, and namespace inheritance remain
bounded to the existing process-global proof state rather than independently
modeled per process.
A separate eager-copy primitive validates generation-bound ownership and
pairwise separation before copying two complete private pages. Its proof
rejects stale and aliased destinations without partial writes, demonstrates
parent/child independence, scrubs both copies, and restores the allocator's
live-frame count. A companion transaction walks every user leaf in a bounded
four-to-six-frame hierarchy, rejects incomplete inventories, copies and
rewires the page tables, and replaces every parent leaf with its corresponding
private child frame while preserving W^X. The QEMU proof validates two
independent roots and reclaims all twelve temporary frames. The process proof
now uses that operation to copy PID 1's two private pages, construct and
revalidate the child's mapping inventory, and atomically publish the cloned
bundle with a parent-derived execution context whose fork return value is zero;
child mutation leaves the parent unchanged. PID 1 and that child are then
admitted to the real native scheduler, selected under their exact roots, and
yielded in both directions before their generation-tagged bindings are retired.
Each root inherits the supervisor-only identity and higher-half kernel entries;
the selected child CR3 is loaded, verified through the direct map, and restored
in kernel mode. A bounded child then enters ring 3 under that CR3, requires its
fork return in `RAX` to be zero, writes through its recorded FS base without
changing the parent page, and returns through the user gate with FS and CR3
restored. The entry stub installs all fifteen general registers, and the user
gate captures and compares the complete frame before resuming ring 3. The child
then issues `exit(42)`; the gate captures that status, returns to the kernel,
and the same status drives resource teardown, zombie publication, and the
parent's wait result. The live Linux exit paths now take an exact address-space
snapshot before teardown. It combines the active image, primary stack, heap,
primary and sibling TLS, sibling stack, and bounded `mmap` registries into at
most 41 generation-checked mapping records, then requires a complete page-table
walk to find exactly those user leaves with the recorded W^X permissions. A
missing mapping, altered permission, stale root generation, or nonempty
detached hierarchy frame is rejected. The kernel also converts the live syscall frame into the process
model's complete 15-register context and admits it with the snapshot as PID 1.
This includes Linux's architectural `RCX` and `R11` syscall clobbers and permits
a lower-canonical FS base without requiring that it currently be mapped. The
record is installed in non-stack kernel storage before userspace starts and
refreshed from the exact live snapshot and return context at every PID 1
syscall boundary. Terminal cleanup requires at least two successful refreshes,
one parentless running record, and no child, scheduler binding, or retiring
bundle. After each terminal ring-3 return, while the retained PID 1 resources
are still live, the kernel reserves a child, allocates and eagerly copies every
private page, clones the complete page-table hierarchy, derives a zero-return
child context, and atomically publishes the result. It compares every copied
page byte, requires complete parent/child frame separation, then tears the
child down, reaps status 42, and proves the allocator count and PID 1 record
were restored. One live child also has a bounded private trampoline installed
at its saved RIP, is selected through the native process scheduler, enters ring
3 under its cloned CR3 with the complete fork context and FS base, and drives
zombie publication with its captured `exit(42)`. The parent code remains
unchanged. The minimal system workload now consumes the same machinery through
Linux syscall 57 (`fork`) and syscall 61 (`wait4`). The child resumes its real
post-fork code with `RAX = 0`, observes PID 2 and parent PID 1, and exits 42.
PID 1 resumes with the child PID, verifies invalid waits do not consume the
zombie, receives status `42 << 8`, and proves a repeated reap returns
`-ECHILD`. The profile therefore records `process.fork-wait` ready while
retaining explicit limits on concurrency and inherited process state. The
exact contract and limits are recorded in
[Bounded Linux Process Lifecycle v0](specs/kernel/linux-process-v0.md).
Each thread slot also owns its clear-child-TID pointer and robust-list head.
The lifecycle proof assigns different sentinel registrations to TIDs 1 and 2,
switches between them, and proves neither becomes visible through the other.
Retiring TID 2 clears both values before its slot is reused, and the replacement
starts empty. The real TID 1 syscall registrations are resolved through that
slot; exit zeroes the exact registered child-TID word, records and clears the
exact robust-list head, then retires the thread. Post-exit verification requires
both per-thread fields to be zero in every slot.
A compact userspace interpreter assembled from its own source derives its load
bias, reads each admitted `DT_RELA` record, applies its own load-biased relative
relocation plus the main image's relative and symbol-based relocations. The
interpreter independently requires the exact primary and split relocation
sizes, 24-byte `RELA` entries, counted-relative totals, and `DT_RELA` PLT kind
for the main and both dependencies before applying those records. The
symbol relocations resolve to the authenticated dependencies, and the interpreter
also applies both dependencies' own relative and self-symbol relocations. Each
DSO and the main expose one authenticated `DT_INIT` function and one eight-byte
`DT_INIT_ARRAY`. The main additionally exposes one eight-byte
`DT_PREINIT_ARRAY`. The interpreter independently scans the main's dynamic
table for the exact relocation size and entry size, two-entry relative count,
and complete preinit tag pair. It binds that address to the first relative
relocation target, then relocates and calls the preinitializer
before loading either dependency, calls each dependency's direct init function
and constructor in order, resolves the main, and then calls the main's direct
init function and constructor before transferring control to its entry.
Before applying any init-array relative relocation, the interpreter also
requires the runtime `DT_INIT_ARRAY` address to equal that relocation target
for both dependencies and the main.
Each exported function refuses to succeed before its dependency constructor
runs; each constructor requires its image's direct init phase, and the main
constructor calls both exports before recording initialization,
and the main entry refuses to continue without that record. All three
constructors receive the original `argc`, `argv`, and `envp` context and require
the bounded `/init` and `GINUX=1` vectors before recording initialization. The main workload calls through the
resolved jump slots; the loader validates all six dependency relocations and
both dependency calls return checked values before the full
ring-3 probe completes, while the kernel verifies the interpreter's relocated
target. The kernel then restores its root and scrubs every owned frame.
The build rejects unresolved ELF relocation records in all three raw embedded
text payloads before extracting their bytes.
The executable QEMU path now goes beyond the synthetic metadata fixtures: LLD
produces an `ET_DYN` interpreter, PIE main, `libginux.so`, and
`libginux-aux.so`, including
program headers, dynamic tables, GOT/PLT, SysV/GNU hashes, `DT_NEEDED`, and
`DT_SONAME`. The kernel authenticates all four linker outputs, applies fixed
load biases, accepts the dependencies' intentionally unmapped ELF headers, and
executes the complete probe through the linker-generated jump slot.
The authenticated interpreter occupies a bounded two-page RX extent plus one
RW/NX page; its metadata/startup core remains on the first executable page and
its runtime-file discovery path begins on the second. Before relocating itself,
the linked interpreter validates its mapped ELF
header and bounded program-header table and derives its sole `PT_DYNAMIC`
address and entry count; it no longer embeds its linker layout. After applying
that relocation, the interpreter bounds and scans its initial stack,
requires `AT_PHDR`, `AT_PHNUM`, `AT_BASE`, and `AT_ENTRY`, and verifies that
`AT_BASE` identifies its actual load page. It derives the main load bias from
`AT_PHDR`, the authenticated file's `e_phoff`, and matching `e_phnum`. It then
invokes the Linux syscall gate from its own RX range and opens `/init`. After inspecting it through a temporary
read-only window, the interpreter reopens it, bounds its 56-byte program-header
table to nine entries in the two-page window, and derives page-aligned mmap
requests from each readable `PT_LOAD`. It installs the authenticated header,
four RX, and two RW/NX pages through exact `MAP_FIXED_NOREPLACE` contracts. It
then captures the main image's two ordered `DT_NEEDED` strings, constructs each
`/lib64/<needed-name>` on its bounded stack, inspects those derived dependency
paths, walks both files' `PT_LOAD` entries the same way, and installs
RX pages at `0x618000..0x61a000`, an R/NX page at `0x61a000`, and an RW/NX page
at `0x61b000` for the primary DSO. The auxiliary DSO receives RX at `0x638000`
and RW/NX at `0x63b000`. The kernel admits a
page-aligned dependency bias only within `0x610000..0x620000`, binds the first
valid request, and requires every later dependency page to preserve the
authenticated relative layout at that bias. A bounded eight-entry contract
table is derived from the dependency's authenticated page plan; the current
fixture consumes four entries and exercises every admitted permission class.
Kernel execution state now provides two isolated dependency slots, each with
its own base window, selected bias, eight-entry contract table, mapping mask,
generation leases, and cleanup validation. The running interpreter populates
both slots, executes the main's one-entry preinit array, applies all three
admitted relocations in each DSO, executes each
dependency's `DT_INIT` function and one-entry init array, resolves the main, and
executes its `DT_INIT` function and one-entry init array before transferring to
the main entry.
Wrong offsets, duplicate
placement, a mixed bias, and W+X are rejected before it applies
dependency and main relocations. Both dynamic-table addresses and entry counts
come from bounded `PT_DYNAMIC` headers, and final transfer uses `AT_ENTRY`
rather than an embedded main entry address. The kernel installs only the interpreter for
this stage; it authenticates and plans the main and both dependencies and reserves
only the address-space infrastructure and interpreter frames initially. Main
and dependency payload frames are allocated only when their authenticated mmap
requests arrive. Each successful authenticated mapping atomically joins the
process's bounded active-image record alongside the initially installed
interpreter pages. Each entry binds its address, exact frame generation, and
W^X permissions. Teardown requires that record to contain the complete
interpreter, main, and two-DSO page set with no duplicates or trailing state.
All three userspace-loaded images' content and leaves are installed by userspace,
and every executable-page lease is scrubbed and released after the process
root is deactivated.
Linker-private metadata remains mapped but outside the probe's bounded
syscall-copy data extent.
Interpreter and dependency selection bind the exact `PT_INTERP` path or
`DT_NEEDED` name and SHA-256 digest before parsing or mapping. The dependency's
own `DT_SONAME` must equal both the requested name and the authority record. A
production authority source and general dynamic linking remain unimplemented.
The admitted linker tag subset also validates `DT_PLTGOT`, zero `DT_DEBUG`, one
counted relative relocation, and `DF_1_PIE`. Malformed dynamic tables,
duplicate tags, unsupported relocations, and relocation targets outside writable memory
are rejected before mapping.
The static planner also rejects any image carrying dynamic relocation metadata,
so unresolved code cannot bypass interpreter-first startup.
The execution-oriented main plan keeps its six-header table in a distinct
read-only load so its `0x400100` entry remains intact.
The profile is validated during boot,
plans and installs up to two RX/RW pages per segment with independently
zero-filled BSS, including ELF and program headers in the first load page. The
runtime proof owns every mapped frame, preserves per-page W^X permissions, and
rejects larger plans. The planner materializes a conventional third read-only
load and non-page-base segments. The mapper owns its frame and a separate leaf
table, activates the root, and executes the toolchain-linked image through the
full syscall workload before tearing every frame down. The synthetic two-load
proof also activates the collision-checked page-table root and
executes a non-page-base verification entry at ring 3. This
entry receives a distinct RW/NX stack containing `argc`, `argv[0]`, `envp[0]`,
and bounded Linux `AT_PHDR`/`AT_PHENT`/`AT_PHNUM`/`AT_PAGESZ`, `AT_CLKTCK`,
`AT_PLATFORM`, conservative `AT_HWCAP`/`AT_HWCAP2`, static-loader,
credential, `AT_SECURE`, `AT_EXECFN`, and `AT_ENTRY` metadata. It validates its
argument, environment, executable-name, secure-mode, and entry metadata from
ring 3 before terminating through the architectural x86-64 `SYSCALL` instruction
after checking that an unsupported call returns `-ENOSYS`, performing bounded
console `write` copy-in and `uname` copy-out operations, and validating a
`getpid()` result returned through `SYSRETQ`. Console writes accept 1 through
256 validated bytes on descriptor 1 and preserve exact byte content. Bounded
`writev` atomically validates and assembles up to four vectors under the same
aggregate limit before emitting any bytes. Bounded `poll` atomically reports
immediate console and device readiness across four descriptors. With one to
four descriptors and a runnable peer, it also blocks in the scheduler for a
signed millisecond timeout or indefinitely, wakes after relevant eventfd,
pipe, close, or timerfd state changes, and returns zero after an APIC-driven
timeout. A zero-descriptor `poll` or `ppoll` ignores the table pointer and uses
the same timeout path as a sleep; the measured graph proves positive timeouts
for both forms. This is a fixed two-thread proof, not a general wait-queue
facility; blocking without a runnable peer returns `ENOSYS`.
A caught signal unblocked by the ordinary thread mask can interrupt `poll` with
`EINTR` without changing the output table.
`ppoll` uses the same scheduler-backed descriptor path with a checked
nanosecond timespec or an infinite null timeout. Its optional eight-byte signal
mask is installed only for the blocked interval, with `SIGKILL` and `SIGSTOP`
removed, and the calling thread's prior mask is restored on readiness, timeout,
rollback, or signal interruption. A sibling may target the initial thread with
`tgkill` while it is blocked with a caught standard signal unblocked by the
temporary mask. The kernel transactionally saves the sender, cancels the wait,
restores the waiter's prior mask, switches to that thread's handler, and returns
`EINTR` through `rt_sigreturn` without modifying the poll output. The measured
two-thread graph proves that handoff, an APIC-driven timeout, and restoration of
the original per-thread mask. Cross-thread delivery to other states, queuing a
signal blocked by the temporary mask, nested delivery, and syscall restart
remain outside this bounded profile. A single bounded epoll
instance can register one console or device descriptor, report level-triggered
readiness, and preserve packed 64-bit user data. With a runnable peer,
`epoll_wait` blocks for a signed
millisecond timeout or indefinitely, wakes after relevant descriptor or
registration changes, and returns zero after an APIC-driven timeout. The
registration, readiness, and data word remain live while blocked, and the
output event is revalidated before completion. This remains a fixed two-thread,
one-registration proof; blocking without a runnable peer returns `ENOSYS`. A
caught signal unblocked by the ordinary thread mask can interrupt `epoll_wait`
with `EINTR` without changing its output event.
`epoll_pwait` and `epoll_pwait2` use the same scheduler path after validating
their temporary signal mask, and the latter converts a null or checked
nanosecond timespec. The mask is installed only while blocked and restored on
readiness, timeout, rollback, or targeted signal interruption. The measured
graph drives separate `tgkill` handoffs through `poll`, `epoll_wait`, `ppoll`,
and `epoll_pwait`, verifies `EINTR` with unchanged output, and then proves an
`epoll_pwait` APIC timeout and mask restoration. Blocked signals, nested
delivery, and syscall restart remain unsupported. A bounded eventfd
counter provides a signalable source for that epoll path, including
nonblocking saturation and semaphore reads. With a runnable peer, a blocking
read on an empty eventfd now sleeps in its scheduler slot until a write makes
the counter readable; descriptor closure wakes it with `EBADF`, and its
eight-byte output is revalidated on completion. A caught standard signal that
is unblocked by the waiting thread's ordinary mask may interrupt that read:
the same transactional sender/handler handoff returns `EINTR` without changing
the output or counter. The measured graph then blocks the same read again and
completes it through a write, proving the descriptor remains live. Blocked
signals, unsupported dispositions, nested delivery, and syscall restart retain
their explicit `ENOSYS` boundary. A write that would saturate a blocking
eventfd now stores its already-copied increment in the scheduler wait. A read
that creates enough capacity wakes and atomically commits that saved value;
signal interruption returns `EINTR` without changing the counter, and close
wakes it with `EBADF`. The measured graph mutates the userspace input while the
retry sleeps and still observes the original saved increment. Blocking without
a runnable peer retains `ENOSYS`.
A bounded monotonic timerfd adds one-shot and periodic clock-driven readiness
with relative or absolute arming. An unexpired or disarmed blocking read sleeps
in its scheduler slot while a peer is runnable. Arming and disarming refresh the
stored deadline; an APIC tick completes an expired read with its accumulated
count and preempts directly back to the reader. Descriptor closure wakes it with
`EBADF`. A caught standard signal can interrupt the wait with `EINTR` without
changing its output or consuming/disarming the timer; retrying can still finish
at the original deadline. Blocked signals, nested delivery, and syscall restart
remain unsupported. One bounded pipe pair provides a 256-byte FIFO
with atomic scalar or vectored I/O, poll/epoll readiness, EOF, broken-peer
behavior, FIFO metadata, and byte-count introspection. An empty blocking scalar
or vectored read sleeps in its scheduler slot while a peer is runnable, then
revalidates and copies available bytes after a write or returns EOF after the
writer closes. A blocked `readv` retains a kernel-owned snapshot of up to four
iovecs and revalidates every snapshotted output range at completion; changing
the userspace iovec table while asleep cannot redirect the copy. An admitted
caught signal may interrupt either wait with `EINTR` without
changing the output or FIFO; retrying the same read can then complete normally.
The measured graph proves both wake paths, the scalar interruption/retry path,
and readv snapshot isolation. A saturated blocking scalar or vectored write
also sleeps while a peer is runnable. It revalidates and atomically copies its
input only after enough capacity becomes available, returns `EINTR` without
changing the FIFO when interrupted, and wakes with `EPIPE` if the reader
closes. A blocked `writev` retains a kernel-owned snapshot of up to four
iovecs, so later userspace table mutations cannot redirect its input. The
measured graph proves both write forms across interruption, capacity wake, and
reader-close wake, including writev snapshot isolation. Broken scalar and
vectored writes generate `SIGPIPE`: caught delivery resumes the write with
`EPIPE`, ignored delivery preserves `EPIPE`, and the default disposition
terminates the bounded Linux process. A bounded console ioctl reports a fixed 80-by-24 geometry, and `close` irreversibly
retires either initial console descriptor. Bounded `dup` reuses the lowest closed slot and preserves whether
the console endpoint is input or output; `dup2` provides exact-slot replacement.
`fcntl(F_GETFD/F_GETFL)` exposes the bounded descriptor and endpoint flags. It also
configures and queries an FS-relative
TLS base through bounded `arch_prctl(ARCH_SET_FS)` and restores the supervisor's
prior FS base afterward. Pointer-bearing calls are constrained to the RW
segment's declared ELF memory extent, allocated heap bytes below the current
break, the mapped user stack, or the active bounded mappings. They
cannot cross the stack's guard-page boundaries. Invalid ranges and write
arguments return Linux `-EFAULT`, `-EBADF`, or `-EINVAL`. Copy-in may also read
the declared main or interpreter RX ELF extent, while copy-out rejects both.
A bounded area walker admits a buffer only when adjacent declared areas cover
every byte with the requested permission. The pipe proof copies two bytes
across the exact linked-main RX-to-RW boundary; gaps, RELRO, guard pages, and
address overflow still fail closed.
The ring-3 workload verifies each error and continues into valid calls.
A real ring-3 page fault now enters a bounded `SIGSEGV` handler registered with
`rt_sigaction` and `SA_RESTORER`. With `SA_ONSTACK`, the kernel selects the
registered writable alternate stack, marks an ordinary registration
`SS_ONSTACK`, and admits syscall
entry from that range only for the active signal frame. It builds the 440-byte
x86-64 Linux `rt_sigframe` base layout with the restorer, `ucontext`, and
128-byte `siginfo_t` at their UAPI offsets, preserves the normal-stack red zone,
and retains the complete interrupted register state. Handler arguments always
identify the frame's information and context records. `SA_SIGINFO` populates
page-fault `SEGV_MAPERR` address/trap state and software-generation identity;
the frame advertises no floating-point state yet. The handler observes
`SS_ONSTACK`, proves
the alternate stack cannot be replaced while active, and proves a premature
`rt_sigreturn` is rejected without consuming that state, maps the missing page,
and returns through the restorer. A dedicated `IRETQ` syscall epilogue restores
all general registers—including `rcx` and `r11`—plus RIP, RSP, RFLAGS, and the
prior mask before retrying the faulting load. This covers synchronous page-fault
delivery plus `kill`, `tkill`, and `tgkill` generation of explicitly caught
standard signals 1 through 31. Ignored signals are discarded; unsupported
default dispositions and real-time generation fail explicitly instead of
claiming Linux behavior. The thread-directed paths reject zero or out-of-range
identities before distinguishing absent thread groups or threads. Blocked
standard signals coalesce independently in a pending set. `rt_sigpending`
reports the blocked subset, and `rt_sigprocmask` delivers the lowest-numbered
newly unblocked signal while retaining each signal's first source code. The
measured runtime proves `SIGUSR1` wins over an earlier-queued `SIGUSR2`, and
that a duplicate does not replace `SI_USER` with `SI_TKILL`. When both become
unblocked together, the first successful `rt_sigreturn` installs the second
frame before any original userspace instruction executes; a user-edited return
register survives through that chain. Pending selection is deferred while a
frame is active because nested delivery remains outside this profile.
`rt_sigreturn` copies
the complete frame back, requires the published selectors, context flags,
alternate-stack record, null FP-state pointer, and reserved words to remain
valid, then restores user-edited integer registers, Linux-restorable RFLAGS
bits, and `uc_sigmask` with `SIGKILL` and `SIGSTOP` removed. A malformed frame
fails without consuming the active context. Floating-point/XSAVE state,
user-edited alternate-stack restoration, other asynchronous sources, nested
delivery, and syscall restart remain future work. `SS_AUTODISARM` is supported:
delivery records the configured stack in `ucontext`, makes the handler-visible
configuration disabled, retains syscall access through the active kernel frame,
and restores the exact configuration on `rt_sigreturn`.
`SA_NODEFER` suppresses the automatic delivered-signal mask bit while retaining
the explicit action mask, and `SA_RESETHAND` atomically restores the default
disposition after committing a delivery. The measured handler observes that
mask and verifies the one-shot action is already cleared after return.
Bounded `rt_sigtimedwait` synchronously consumes the lowest-numbered matching
pending standard signal. It supports an immediate zero-time poll, optional
128-byte `siginfo_t` output, and immediate consumption when a signal is already
pending even if the supplied timeout is nonzero or null. A null-timeout wait
with no pending match can also suspend one of the two bounded threads while its
peer is runnable. A matching generated standard signal writes the optional
information record, installs the signal number as the suspended syscall's
return value, and makes that thread runnable without invoking its disposition.
A caught, unblocked standard signal outside the requested set may instead
interrupt a sibling's wait through `tgkill`. The kernel transactionally saves
the sender, cancels the typed wait, switches to the waiter's handler, and
returns `EINTR` through `rt_sigreturn` without consuming a requested signal or
modifying the optional information record. The measured two-thread graph proves
that handoff with `SIGUSR2`, then proves that a later matching `SIGUSR1` still
completes the normal synchronous-wait path.
A positive timeout stores a checked monotonic deadline in that same wait record;
the periodic local-APIC tick makes the waiter runnable with `EAGAIN` after the
deadline, independently of Linux syscall entry, saves the interrupted peer's
complete integer context, and dispatches the waiter immediately. The measured workload proves both timeout
and signal handoffs from a futex-blocked child to its signal-waiting parent and
requires all typed wait state to be reclaimed at exit. Empty zero-time polls
also return `EAGAIN`. Nonmatching signals that are blocked by the target's
current mask, unsupported dispositions, nested delivery, syscall restart, and
waits without another runnable thread remain outside the profile and return
`ENOSYS`. Input, timeout, and output failures leave pending and thread state
unchanged.
The bounded process reports UID, effective UID, GID, and effective GID zero as
container-local identity only; these values grant no native kernel capability.
Real/effective/saved UID and GID triples are also zero and are copied only
after all three output pointers pass writable-VMA validation.
Its initial single thread reports PID/TID/process-group/session ID 1 and parent
PID 0, matching an initial process namespace. A bounded
`set_tid_address` registration returns TID 1 and clears the registered word as
part of exit.
Bounded `set_robust_list` and `get_robust_list` calls retain and report one
24-byte thread robust-list head, reject foreign process IDs and invalid ranges,
and discard the registration during exit. A bounded matching futex wait may
block either of the two threads only while its peer is ready. Explicit wake
restores the blocked child; child exit clears its registered word and restores
the blocked parent. An optional relative timeout uses the same typed monotonic
deadline state as signal waits: zero returns `ETIMEDOUT` immediately, while a
positive duration wakes at the next Linux syscall or scheduling boundary after
expiry. The same periodic local-APIC tick can instead mark the timed waiter ready
and dispatch it directly from interrupt context. Mismatches return `EAGAIN`, and a matching wait without another runnable
thread returns `ENOSYS`. General futex queues and robust futex recovery are not
implemented.
The measured minimal-system PID 1 additionally exercises the public private
operation encodings before readiness. `FUTEX_WAIT_PRIVATE` must return
`EAGAIN` for a mismatched aligned word without changing it, and
`FUTEX_WAKE_PRIVATE` must return zero when no waiter matches. It then emits
`FUTEX_READY`. Combined with the dedicated two-thread runtime above, this closes
the bounded `futex.private` profile row without claiming general Linux futex
compatibility.
The bounded M5 system image keeps PID 1 and its console descriptor live after
the readiness line. PID 1 blocks in `rt_sigtimedwait` for `SIGTERM`; the native
control path verifies the saved generation-bound process and thread state,
wakes it with kernel-origin signal information, and resumes the same context.
PID 1 validates the event, emits `SUPERVISOR_SHUTDOWN` through the retained
console, restores its signal mask, closes the descriptor, and exits. This is a
deterministic persistent-supervisor proof, not yet an interactive console or a
general service-restart policy. Static exec now clears the replaced image's FS
base before entering an image without TLS, so the saved supervisor context
cannot retain a stale thread pointer.
Bounded `mprotect` can activate a complete `PROT_NONE` scratch range as fresh
zeroed read-only/NX or RW/NX pages, or irreversibly seal the page named by the
parsed `PT_GNU_RELRO` range or any complete RW scratch range as read-only/NX.
Changing a subrange rebuilds the bounded registry around it so the other pages
retain independent protection. The page-table permissions and syscall copy
direction change together: reads continue after sealing, writes fail, and
executable permissions are rejected. Read-only pages cannot regain write
permission, and active pages cannot transition back to `PROT_NONE` yet.
An exact idempotent protection request succeeds only after revalidating every
selected lease and page-table leaf.
One separately owned, zero-filled RW/NX page backs a bounded `brk` heap. The
ring-3 workload grows and shrinks the break, writes through the mapping, and
confirms that an out-of-range request leaves the current break unchanged.
A four-page fixed `mmap` registry can install page-aligned private areas of one
through four pages in its reserved virtual window. Physical frames are allocated
only when a mapping is admitted. Every nonzero scratch length is rounded up to
a page consistently across mapping, protection, residency, discard, and unmap;
a rounded request requiring more than four pages is rejected. Anonymous areas
may be read-only/NX, RW/NX, or virtual-only `PROT_NONE` reservations. Reservations own no
physical frame, keep their page-table leaves absent, deny syscall copies, and
remain occupied for placement purposes until activation or unmap. RW/NX areas are
removed from the hardware page table by exact `munmap`, scrubbed, and proven
zero on reuse. Bounded `MADV_DONTNEED` also zeros an anonymous area in place
without widening a sealed read-only area, while `mincore` reports mapped pages
resident and `PROT_NONE` reservations nonresident. Any contiguous part of the window can
instead expose a private read-only/NX copy of an immutable runtime ELF file.
File offsets must
be page aligned, partial final pages are zero-filled, syscall copy-out cannot
modify the mapping, and unmap scrubs every payload frame. The runtime proof
keeps independent areas live simultaneously, queries and removes them without
disturbing their neighbors, and preserves access and immutable-file attributes
across splits. A four-page proof seals the middle pair, punches two holes,
reuses the lowest hole, rejects a two-page request when free pages are
fragmented, and finally unmaps the complete sparse window. `mprotect`,
`madvise`, `mincore`, and `munmap` walk adjacent records, so range behavior no
longer depends on their original allocation boundaries. Anonymous
`MAP_FIXED_NOREPLACE` can select an exact free subrange inside the window;
overlap returns `EEXIST` without changing either the existing mappings or frame
ownership. A reservation also counts as overlap. Anonymous `MAP_FIXED` can
atomically fill holes, reservations, or replace a selected
subrange after all old and new frame leases and page-table leaves pass one
preflight. Replacement publishes fresh zeroed read-only/NX or RW/NX pages before scrubbing and
retiring the displaced generation leases. A `PROT_NONE` fixed replacement
instead clears the selected leaves, retires their leases, and leaves an
occupied virtual-only reservation that can later be activated.
Every occupied page selected by unmap validates generation ownership, removes
the hardware leaf, scrubs the released frame, and returns its lease to the
allocator; process exit requires the complete registry and all four lease
slots to be empty.
Placement outside the bounded window, fixed shared or file replacement
mappings, writable mappings outside the exact authenticated main/dependency
data pages, and arbitrary filesystem-backed
mappings are not implemented.
The root view starts with `/` as its working directory. Bounded `chdir` and
`fchdir` can select an immutable indexed directory such as `/var/lib`, and
`getcwd` returns that retained path. `AT_FDCWD` and relative `chdir` resolve
beneath the current measured directory; empty, `.`, `..`, and empty path
components are rejected. Paths and outputs remain fixed-size, and no mutable
VFS is implied.
Its first procfs-compatible identity is `/proc/self/exe`, whose bounded
`readlink` result is `/init` before replacement and the selected canonical
`/bin/signal-probe` or `/bin/exec-second` path after
the bounded exec commit; rejected exec calls retain the old identity.
An immutable runtime-file view exposes the exact authenticated `/init`,
`/lib64/ld-ginux.so`, `/lib64/libginux.so`, and `/lib64/libginux-aux.so` bytes
as read-only regular files. The linked main opens the auxiliary DSO, reads and
validates its ELF magic, and closes its independent descriptor. The interpreter
also derives, inspects, and executable-maps both DSO paths.
Bounded `openat`, scalar and vectored reads, `pread64`, `lseek`, private
read-only `mmap`, metadata, and access checks operate on those files. This is
bounded immutable-root traversal, not a general VFS or general executable
loading. The kernel
authenticates and plans the fixed four-image graph; the interpreter installs
the authenticated main and both dependency segments admitted by that exact plan.
The separate static proofs resolve normalized absolute or current-directory-relative
`execve` paths through the measured root, then admit any measured regular file
that satisfies the bounded static ELF execution profile. The two
independently linked fixtures, `bin/signal-probe` and `bin/exec-second`, prove
selection is neither compiled-byte nor fixed-address identity: both use bounded
two-page text segments, while the second has an independent header and data
base. The profile
allows one header page, up to five executable pages, up to two writable pages,
and at most eight pages in total. The active mapping list is process runtime
state and binds each virtual page to its generation-tagged physical-frame lease
and W^X permissions. A successful transaction replaces it with the selected
plan, while a failure retains the old list. The profile also allows up to eight
63-byte `argv` strings, eight equivalent
`envp` strings, and one live thread. Rejected path, pointer, count, string, and
aggregate-stack cases leave the original image
running; success installs exact W^X target frames, unmaps the old image, and
retains the selected canonical measured-root identity.
The same atomic transaction is exposed through a flags-zero `execveat` profile;
each proof opens `/bin`, returns cwd to `/`, and resolves its selected basename relative
to that measured-directory descriptor while rejecting bad descriptors,
non-directory descriptors, and unsupported flags.
`AT_EMPTY_PATH` also executes the selected authenticated image through an open regular
file descriptor, rejects a directory descriptor, and retains the canonical
measured-root executable identity.
The proof also closes only `FD_CLOEXEC` descriptors, preserves an ordinary
descriptor, resets caught signals while retaining an ignored disposition,
disables the alternate signal stack, and derives the bounded process name from
the selected measured node's basename. The interpreted `/init` proof accepts
the same one-to-eight argument and zero-to-eight environment-entry kernel
contract as static replacement. Its replacement run proves
`argv = ["/init", "dynamic-exec", NULL]` and
`envp = ["GINUX=1", "GINUX_MODE=exec", NULL]` reach the main and both DSO
constructors, then installs the
authenticated interpreter pages atomically, resets the main/DSO mapping
contracts, rematerializes initial and sibling TLS pages from authenticated
templates, restores the initial FS base, and executes the replacement main
after both DSOs have been mapped and relocated. The internal measured node
`linux/init` retains the canonical Linux-visible `/init` identity through
`/proc/self/exe`. This remains one fixed graph,
not a general dynamic-linker contract.
Per-process `umask` state starts at `022`, returns its prior value, and masks
updates to the nine permission bits.
Bounded `fstat` identifies descriptors 0 and 1 as fixed character devices and
copies the exact x86-64 stat layout only after validating the complete output.
Read-only `getrlimit` and `prlimit64` queries expose the bounded stack and file
descriptor ceilings; unsupported resources, foreign PIDs, mutations, and
invalid output ranges fail without changing user memory.
Bounded `sched_getaffinity` and `getcpu` report the current bootstrap topology
as CPU 0 in NUMA node 0, with atomic validation of their output ranges.
Bounded `clock_gettime` reports monotonic and raw-monotonic elapsed time from a
private TSC epoch, using PIT calibration when CPUID has no frequency data;
`clock_getres` reports its rounded single-tick resolution. The sole process and
thread expose that same non-descheduled interval through CPU clocks and
100-Hz `times` accounting; bounded `getrusage` returns it as user time in a
fully initialized x86-64 resource-usage structure. Bounded `nanosleep` returns
immediately for zero or stores a positive relative monotonic deadline while a
peer is runnable. A periodic local-APIC interrupt marks the sleeper ready after
expiry without requiring another syscall, preserves the interrupted peer's
`RAX`, `RCX`, `R11`, remaining integer registers, RIP/RSP/RFLAGS, and FS base,
then dispatches the sleeper immediately. A caught standard signal unblocked by
the sleeper's ordinary mask may interrupt that wait with `EINTR`; a non-null
remainder pointer receives the normalized unslept relative duration before the
signal frame commits, while successful expiry leaves it untouched. The measured
graph proves the interruption and a subsequent APIC-driven relative
`clock_nanosleep` expiry. `clock_nanosleep` accepts `CLOCK_MONOTONIC` in
relative or `TIMER_ABSTIME` mode; an elapsed absolute deadline returns
immediately and ignores its remainder pointer. Realtime clocks remain
unsupported. Blocked signals, nested delivery, and syscall restart remain
unsupported. The three interval-timer classes can be
queried and idempotently disarmed; arming remains unsupported until asynchronous
signal delivery exists.
The bounded ELF parser and page-image planner also validate checked,
page-aligned load-bias plans for position-independent `ET_DYN` images. Dynamic
execution planning binds an exact `PT_INTERP` path to a non-overlapping relocated
interpreter image while preserving distinct initial-entry, `AT_ENTRY`, and
`AT_BASE` values. The bounded synthetic interpreter-first execution path is
verified, including rejection of substituted, mutated, and truncated interpreter
bytes. The dependency is independently authenticated by exact name and digest,
requires a matching internal `DT_SONAME`, is mapped W^X at a non-overlapping
bias, and is rejected if it requests another dependency. Synthetic userspace
relative relocations are executed for the interpreter, main image, and
dependency. The dependency also receives its own
eager `GLOB_DAT` and `JUMP_SLOT` self-bindings. The main image proves one
cross-object symbol resolution through `R_X86_64_GLOB_DAT` and eager
`R_X86_64_JUMP_SLOT`; the jump slot is called in ring 3 and returns a checked
value from dependency code. The userspace interpreter verifies that the
relocation's symbol index is the admitted SysV main-image or GNU dependency
hash bucket result, follows each validated symbol-name offset into its
`DT_STRTAB`, and
requires the complete NUL-terminated names to match. Multiple dependencies,
transitive graphs, search paths, alternate SONAME selection,
symbol-version processing, interposition, multi-bucket lookup, lazy binding,
and a real userspace linker remain outside the profile.
This is not yet a general Linux
process or syscall environment; see
[Linux ELF64 Profile v0](specs/kernel/linux-elf-v0.md) and
[Linux Syscall Entry v0](specs/kernel/linux-syscall-v0.md).

## Quick start

Requirements:

- Rust 1.99.0 through `rustup`
- Clang with the `x86_64-none-elf` assembly target
- QEMU with x86-64 and OVMF support
- GNU Make

Check the local environment:

```sh
make doctor
```

Build the bootable EFI filesystem:

```sh
make build
```

`make build` deterministically regenerates the development manifest, measured
control-plane bundle, measured initramfs, and Ed25519 signature from `boot/manifest-v0.conf` and
`control-plane/bootstrap.hex`; that source declares its authenticated control,
service, and restart entry labels inline. Run `make manifest` directly when
working on those inputs. The kernel build assembles `kernel/linux-probe.S` as
an x86-64 ELF object, extracts its `.text` section for the executable runtime
proof, and also links a complete static ELF with the Rust toolchain's LLD for
parser compatibility verification. The checked-in linker script fixes its
read-only headers at `0x3ff000`, a five-page RX load with entry at `0x400100`,
and RW data/BSS at `0x405000`, matching the ring-3 workload's bounded layout.
This tests ordinary
linker metadata on both macOS and Linux hosts. `make linux-probe` builds the
static artifact at `build/linux-probe.elf` and the dynamic fixtures at
`build/linux-dynamic/ld-ginux.so`,
`build/linux-dynamic/linux-linked-main.elf`, and
`build/linux-dynamic/libginux.so`. It also emits the bounded dependency-list
fixtures `linux-linked-graph-main.elf`, `libginux-aux.so`,
`linux-signal-probe.elf`, `linux-exec-second.elf`, and
`linux-system-init.elf` for inspection
and the deterministic proof root at `build/initramfs-v0.cpio` without assembling the EFI
filesystem. Override the tools with `CLANG=/path/to/clang` or
`LLD=/path/to/rust-lld`. Kernel and
proof-workload ABI constants share the machine-readable
`specs/abi/control-v4.rs` source, and the manifest builder expands its symbolic
feature and descriptor words before signing. The checked-in key is an RFC test
identity and must not be treated as a production trust root.

`make system-image` builds the pinned minimal system-container candidate and
signed development inputs from the profile, repository-built ELF artifacts,
and deterministic archive recipe. See
[Minimal System Container Profile v0](specs/container/minimal-system-v0.md).

The interpreter and dependency proof payloads are assembled from
`kernel/linux-interpreter.S` and `kernel/linux-dependency.S` under separate
bounded text-size and unresolved-relocation checks.

The result is placed at:

```text
build/esp/EFI/BOOT/BOOTX64.EFI
```

Boot it in headless QEMU:

```sh
make run
```

Run the automated boot assertion:

```sh
make test
```

The test succeeds only after the post-firmware kernel validates the UEFI memory map and emits `GINUX_KERNEL_OK` over the serial console.

Before committing a milestone, run the complete clean gate:

```sh
make gate
```

This formats the workspace, removes prior artifacts, runs lint checks, rebuilds
the EFI image, executes the QEMU boot assertions, and checks the working diff
for whitespace errors.

`make help` lists all supported development targets.

## Virtualization profiles

- **macOS on Apple Silicon:** QEMU x86-64 with TCG. UTM is optional for graphical inspection; the checked-in QEMU runner is authoritative.
- **Linux on x86-64:** QEMU x86-64 with KVM when `/dev/kvm` is available, falling back to TCG otherwise.

Override discovery when necessary:

```sh
QEMU_ACCEL=tcg QEMU_CPU=qemu64,+smep,+smap,+umip OVMF_CODE=/path/to/OVMF_CODE.fd OVMF_VARS=/path/to/OVMF_VARS.fd make run
```

The default virtual CPU is `qemu64,+smep,+smap,+umip`, matching the kernel's
fail-closed supervisor execution/access and user-mode instruction protection
requirements. `QEMU_CPU` may select a different model only if it still
advertises SMEP, SMAP, and UMIP.

See [ARCHITECTURE.md](ARCHITECTURE.md) for the full design, [ADR 0001](docs/decisions/0001-native-domains-and-authority.md) for the native domain and authority model, [Boot Handoff Contract v4](specs/boot/handoff-v4.md) for the firmware-to-kernel boundary, [Initramfs Archive Profile v0](specs/kernel/initramfs-v0.md) for the proof root format, and [Paging v0](specs/kernel/paging-v0.md) for the transitional kernel address space.
