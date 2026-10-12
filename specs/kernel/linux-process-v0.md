# Bounded Linux Process Lifecycle v0

**Status:** Bounded `fork`/`wait4` lifecycle verified during x86-64 QEMU boot.

This contract introduces the kernel-owned process identity and lifecycle model
used by the first process-style Linux `fork` and `wait4` implementation.
It is deliberately separate from the existing bounded Linux thread table:
threads share one process address space, while every running process record
owns a distinct allocator-issued, generation-checked address-space resource
bundle.

## Identity and capacity

The v0 table contains four slots. A process is identified internally by the
tuple `(slot, generation, pid)` rather than by PID alone. Slot generations
increase on reuse and PIDs increase monotonically, so a handle retained after
reaping cannot select a replacement process.

The first record is PID 1 and has no parent. PID 1 can be created only once.
Every later record names a live parent. Child creation first reserves a
`Preparing` record with no address-space bundle. That record cannot exit, become
a zombie, or be observed through wait. Commit requires a bundle containing
four through six hierarchy leases and one through 41 private-page mapping
records. Each mapping binds one aligned lower-half virtual address, its
generation-checked frame lease, and writable/executable permission. All leases
must remain live under the kernel bootstrap owner and pairwise distinct. A
complete four-level walk must find every recorded user leaf exactly once, find
no unrecorded user leaf, and match every frame and W^X permission. No lease may
overlap any running or exiting process bundle. Successful commit atomically
attaches the entire bundle and changes the record to `Running`; abort removes
the empty reservation.
Both commit and abort require the exact generation-checked handle, so neither
can affect a slot that has since been reused. A failed transaction may consume
an internal PID, but never publishes that PID as a running process.

Reservation fails without partial publication when the table is full, the
parent is absent or not running, or an identity counter is exhausted. Commit
fails without changing the reservation when any lease is stale, duplicated, or
physically attached to another process, or when the mapping inventory differs
from the supplied page tables.

The eager child-publication transaction obtains the exact parent bundle from
the prepared record rather than accepting a second caller-provided parent
inventory. Before its first write it requires distinct, live child hierarchy
and payload leases that overlap no running or exiting process. It copies every
recorded private page, clones and rewires the complete hierarchy, substitutes
the child frames while preserving the parent mapping permissions, reconstructs
the child inventory, and validates the resulting hierarchy. Only then does it
publish the child as `Running`. A failure after resource admission scrubs and
releases the complete child allocation and removes the invisible reservation.
A rejection before exclusive admission leaves the caller's proposed leases
untouched while still removing the reservation; in particular, an overlapping
input can never cause the live owner's frames to be scrubbed.

## Execution context publication

Every `Running` process owns one saved userspace execution context together
with its address-space bundle. Publication requires lower-canonical nonzero RIP
and RSP, interrupt-enabled reserved-safe RFLAGS with no I/O privilege, nested
task, or virtual-8086 bits, and a lower-canonical FS base. RIP must fall inside
an executable recorded page. RSP must fall above the base and no later than the
end of a writable recorded page, permitting the ordinary initial stack pointer
at the page boundary. FS base must be lower-canonical but need not be mapped;
Linux permits an unmapped thread pointer, and dereferencing it remains subject
to the ordinary page tables.

Eager child publication copies the recorded parent's complete general-register,
RIP, RSP, RFLAGS, and FS state rather than accepting caller-substituted state.
It sets the child's `RAX` to zero for Linux fork-return semantics and attaches
the context in the same transition that publishes the cloned address space.
The parent context remains unchanged. Beginning exit removes the context before
the process can become a zombie.

## Native scheduler admission

A running process may be admitted to the existing native scheduler using only
its recorded address-space root and saved context. Admission first revalidates
the complete bundle and context, rejects domain zero and a duplicate binding,
then records the scheduler's generation-tagged thread identity in the process
record. If record attachment fails, the newly created scheduler entry is
retired. Process exit is refused until the exact scheduler binding has been
withdrawn, so a runnable context cannot retain a root after teardown begins.

The boot proof admits PID 1 and its eagerly cloned child into the real native
scheduler state machine under one synthetic container domain. It selects PID 1,
yields to the child while matching the child's root, all saved registers, zero
fork return, stack, flags, and FS base, then yields back to the unchanged parent
context. It withdraws the ready child and running parent by their exact thread
identities and proves the retired child identity cannot be used again.

Before publication, each synthetic process root inherits exact supervisor-only
copies of PML4 entries 0 and 256 from the active kernel root, preserving the
transitional identity mapping and higher-half direct map without adding user
authority. While the scheduler has selected the child, the boot proof validates
those entries, loads the child's generation-checked root into CR3, confirms the
root remains reachable through the direct map, and restores the prior CR3. A
stale root is rejected before activation, and the switch refuses to run with
maskable interrupts enabled.

The selected child subsequently enters ring 3 at its recorded executable RIP,
with its recorded stack, RFLAGS, FS base, private CR3, and Linux fork return
value zero. Its bounded payload traps on nonzero `RAX`, writes `0x5a` through
`FS:[0]`, and returns through the existing user interrupt gate; `UD2` is the
fail-closed fallthrough. After return, the verifier requires the child stack/TLS
page to contain the marker, the corresponding parent page to remain zero, and
both the prior FS base and kernel CR3 to be restored. The dedicated entry stub
loads all fifteen saved general registers from a fixed kernel-resident array.
The user gate captures `RAX`, `RBX`, `RCX`, `RDX`, `RSI`, `RDI`, `RBP`, and
`R8` through `R15` from the interrupt frame and requires every value to match
the recorded child context before returning zero to ring 3. The child then
loads Linux syscall number 60 and status 42 and traps again. Only that ordered
second trap completes execution: the gate checks the syscall number and
bounded eight-bit status, captures the status, and abandons the user frame to
return to the kernel verifier. A missing, reordered, malformed, or wider exit
status cannot complete the probe.

## Exit and wait ordering

A running process cannot become a zombie while it still owns address-space
resources. Teardown requires the exact complete bundle and moves the record to
`Exiting`, where it is neither runnable nor waitable. Zombie publication
remains denied while the allocator tracks any lease in that bundle. Only after
one prevalidated operation scrubs and releases the whole hierarchy and payload
set can it publish the exit status. A mismatched or stale bundle or process
handle changes no state.

Only the live recorded parent can reap a zombie, and only by the child's PID.
Waiting for a running child or an unrelated process has no effect. Reaping
removes the complete record and makes its slot reusable; the next occupant has
a new generation and PID.

The boot verifier allocates three real six-frame address spaces, each with an
RX code page and RW/NX stack/TLS page, constructs
PID 1, and reserves PID 2. It rejects a stale hierarchy lease, a completely
aliased bundle, a unique-root bundle that aliases PID 1's payload page, and
inventories with substituted virtual addresses or permissions. It proves the
unpublished child cannot exit or be reaped, rejects an eager-clone request that
reuses PID 1's exact frames without releasing them, removes the reservation,
and rejects its stale handle. It rejects contexts with a non-executable RIP,
an invalid stack position, privileged flags, or a noncanonical FS base. It then
prepares PID 3 and uses the eager transaction to copy both parent pages, clone
the hierarchy, validate the child inventory, derive `RAX = 0`, and publish the
address space and context together. The proof compares both ends of the copied
code page, mutates the child while requiring the parent to remain unchanged,
and requires the parent's original return register to remain intact. It also
proves both premature detach and zombie publication are denied, releases one
lease, proves the five survivors still block zombie publication, then scrubs
and releases the rest, publishes the status 42 captured from the child's
ring-3 `exit` trap, and reaps that exact status through PID 1.
Finally, it reuses the same slot for PID 4, rejects the already released PID 3
bundle, exercises normal whole-bundle teardown with status 7, retires PID 1's
bundle, and requires the allocator count to return to its starting value.

## Private page copy primitive

The v0 fork preparation path also has a bounded eager-copy primitive for one
through 41 private process pages. Before copying any byte, it validates
every source and destination frame lease, kernel ownership, direct-map extent,
equal nonzero counts, and pairwise separation. A stale lease or any aliased
frame rejects the complete operation without modifying the zeroed destination.
Its standalone verifier transfers two complete pages, mutates the child copy,
proves the parent page remains unchanged, scrubs and releases both sides, and
requires the kernel's live-frame count to return to its starting value.

The companion hierarchy-clone transaction accepts four through six
generation-checked parent and child page-table frames. Before writing the child
hierarchy, a bounded four-level walk requires the supplied parent mapping list
to cover every present user leaf exactly once, rejects user large pages and
untracked or multiply referenced hierarchy frames, and requires every child
page to be distinct from all parent and hierarchy frames. It then copies every
table, rewires all hierarchy references to the child leases, and replaces each
parent leaf with the corresponding private child page while preserving its W^X
permissions. Incomplete mapping inventories, stale hierarchy leases, and
aliased child pages fail before modifying the destination hierarchy.

The boot proof constructs a four-level, two-page parent hierarchy, copies its
RX and RW/NX pages, clones and rewires the child hierarchy, and verifies that
both roots retain their own mappings and contents. It then scrubs and releases
all twelve frames and requires the allocator count to return to its starting
value.

## Live Linux address-space snapshot

Before normal `exit`, `exit_group`, or default-signal termination can dismantle
the running Linux address space, the kernel constructs one exact ownership
snapshot. The snapshot contains five or six generation-checked page-table
leases and up to 41 user mappings: at most 32 active image pages, the primary
stack, heap, primary and sibling TLS pages, sibling stack, and four bounded
`mmap` pages. Each populated source registry must contribute its exact virtual
address, frame generation, and W^X state. Duplicate addresses or frames are
rejected while the snapshot is assembled.

A complete four-level walk then requires every effective user leaf to appear
exactly once in the snapshot and every snapshot mapping to resolve to the same
live frame and permissions. Page-table leases reached through supervisor-only
intermediate entries remain part of the ownership walk. An owned hierarchy
lease that is no longer reachable after an exec transition is accepted only
when every entry in that frame is zero; a detached frame containing stale page
table state fails validation.

The boot workload exercises this contract for the static, dynamic,
minimal-system, and interpreted exec paths. It also makes an otherwise valid
snapshot incomplete, changes one permission, and substitutes a stale root
generation, and supplies an out-of-range mapping count, requiring all four
variants to fail. The public `fork` path uses this snapshot as the source
inventory for the independently owned child record.

The exit and default-signal paths also convert the actual syscall frame into
the process model's 15-register order. `RAX` carries the selected syscall
result while `RCX` and `R11` carry the architectural return RIP and flags; the
remaining registers, RSP, and FS base come from the same live entry context.
The model rejects a zero RIP and then admits the complete snapshot and context
as a running, parentless PID 1 record. This proves the live state has the exact
shape required by the process model without retaining a large process table on
the bounded syscall stack.

The live PID 1 record is installed into a statically allocated process table
before ring-3 entry. Every subsequent PID 1 syscall boundary rebuilds the exact
snapshot, validates the return context, and atomically replaces that record
before dispatch. This permits an `exec` or mapping operation to make its prior
record stale only until the next kernel entry, when no process-table consumer
can run in between. Child-thread syscalls do not overwrite the process leader's
record. Terminal cleanup requires at least two successful PID 1 refreshes and
refuses to clear a table containing a child, scheduler binding, retiring
bundle, or non-running init record. The table is initialized field-by-field in
static storage so neither construction nor replacement places the four-slot
table on the bounded kernel stack.

This supplies the ownership, publication, and scheduler-staging operations
used by the bounded eager fork. The process bundle retains and cross-checks its
virtual address, frame, and permission inventory, the synthetic hierarchy clone
is published through the process table, and its saved context is selected by
the native scheduler under the child root. The child executes a bounded ring-3
context-and-exit proof under that CR3, and its captured exit status drives the
existing resource teardown, zombie publication, and parent wait.

The live workload now consumes the same operations after each terminal ring-3
return and before releasing PID 1's address space. It reserves a child identity,
allocates the maximum bounded hierarchy and private-page capacity, passes only
the exact live prefixes to the clone transaction, and releases the unused
capacity. Publication eagerly copies every live private page, clones and
revalidates the complete hierarchy, and derives the child's complete context
with `RAX = 0`. The verifier requires identical mapping metadata and bytes with
no shared frame, transitions the child through exit and zombie state, reaps
status 42 from PID 1, and requires both the original parent record and allocator
count to be unchanged afterward. Running outside the syscall-dispatch frame is
required by the bounded bootstrap-verification stack contract; no user mapping
is retired between the terminal syscall snapshot and this transaction.

The first such child is admitted beside PID 1 to a serialized non-stack process
scheduler workspace. Only its private executable copy is replaced at the saved
RIP with a bounded context-check and `exit(42)` trampoline. The scheduler
selects the child, its cloned root becomes active, ring 3 observes the complete
fork-derived register frame with `RAX = 0` and the recorded FS base, and the
captured exit status drives teardown, zombie publication, and parent-only reap.
The parent is selected again before both scheduler bindings are retired, its
corresponding code bytes remain unchanged, and the temporary scheduler must be
empty before reuse. Later terminal workloads repeat clone/publication/reap
without redundantly executing the probe.

## Public `fork` and `wait4` path

Linux syscall 57 is accepted only from PID 1's leader and only when no fork is
already pending. The syscall records PID 1's exact post-syscall RIP, RSP,
RFLAGS, FS base, and complete general-register frame, then returns to the
kernel bootstrap stack. Allocation and page-table cloning never run on the
bounded syscall stack. The transaction eagerly copies every recorded private
page; copy-on-write is not claimed.

PID 1 and the committed PID 2 are admitted to the native process scheduler.
PID 2 is selected, its generation-checked root is activated, and it resumes the
actual instruction following `fork` with `RAX = 0` and the saved flags and FS
base. The process identity is separate from the existing thread identity: the
bounded child remains the sole thread leader but observes `getpid() == 2` and
`getppid() == 1`. Its `exit(42)` returns to the bootstrap stack without
retiring resources under the active child CR3. After the parent root is
restored, the scheduler selects PID 1, the child bundle is scrubbed and
released, and PID 2 becomes a zombie. PID 1 resumes with `RAX = 2`.

Linux syscall 61 accepts only a positive exact child PID, options zero, and a
null rusage pointer. A non-null status pointer must cover a complete writable
four-byte range before the zombie is consumed. Successful wait copies the
Linux exit encoding `status << 8`, returns the child PID, and removes the
generation-tagged child record. Unsupported options return `-EINVAL`, an
invalid status range returns `-EFAULT`, and an absent, running, unrelated, or
already reaped child returns `-ECHILD`. The measured PID 1 proves both error
paths leave the zombie waitable, reaps status 42, and proves a second reap is
rejected.

Failure before child publication aborts the invisible reservation. Failure
after publication but before dispatch withdraws any scheduler bindings,
releases the scheduler workspace, scrubs the child bundle, removes its record,
and restores the parent's pre-fork return value. Successful completion requires
an empty scheduler workspace and leaves only the unchanged running PID 1
record.

## Deliberate limits

This is a bounded compatibility slice rather than general Linux process
management. It does not implement:

- copy-on-write, concurrent parent/child execution, or more than one public
  child lifecycle;
- process-style `clone`, `vfork`, `waitid`, wait groups, `WNOHANG`, or rusage;
- independently modeled descriptor, credential, signal, mount, or namespace
  inheritance;
- concurrent process scheduling or process-directed signal delivery;
- orphan reparenting, subreapers, process groups, or sessions;
- resource accounting, limits, PID namespaces, or procfs generation from this
  table; or
- general capacity or PID/generation rollover recovery.

The minimal system profile reports `runtime process.fork-wait ready` only for
this exact PID 1 → PID 2 → exit 42 → `wait4` contract.
