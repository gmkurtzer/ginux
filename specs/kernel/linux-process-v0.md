# Bounded Linux Process Lifecycle v0

**Status:** Internal lifecycle contract verified during x86-64 QEMU boot.

This contract introduces the kernel-owned process identity and lifecycle model
needed before Ginux can expose process-style Linux `clone`/`fork` and `wait4`.
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
four through six hierarchy leases and one through sixteen private-page mapping
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
at the page boundary. A nonzero FS base must fall inside a recorded page.

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
identities and proves the retired child identity cannot be used again. This is
scheduler-state integration, not ring-3 dispatch of the synthetic child.

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
an invalid stack position, privileged flags, or an unrecorded FS base. It then
prepares PID 3 and uses the eager transaction to copy both parent pages, clone
the hierarchy, validate the child inventory, derive `RAX = 0`, and publish the
address space and context together. The proof compares both ends of the copied
code page, mutates the child while requiring the parent to remain unchanged,
and requires the parent's original return register to remain intact. It also
proves both premature detach and zombie publication are denied, releases one
lease, proves the five survivors still block zombie publication, then scrubs
and releases the rest, publishes status 42, and reaps it through PID 1.
Finally, it reuses the same slot for PID 4, rejects the already released PID 3
bundle, exercises normal whole-bundle teardown with status 7, retires PID 1's
bundle, and requires the allocator count to return to its starting value.

## Private page copy primitive

The v0 fork preparation path also has a bounded eager-copy primitive for one
through sixteen private process pages. Before copying any byte, it validates
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

This now supplies the ownership, publication, and scheduler-staging operations
needed by a future eager fork. The process bundle retains and cross-checks its
virtual address, frame, and permission inventory, the synthetic hierarchy clone
is published through the process table, and its saved context is selected by
the native scheduler under the child root. It does not yet snapshot the running
Linux workload's mapping registry, activate the child CR3 for ring-3 execution,
or expose the operation through the Linux ABI.

## Deliberate limits

This is an internal ownership contract, not Linux process compatibility yet.
It is not connected to the syscall dispatcher and does not implement:

- construction of a complete live Linux child address space or copy-on-write;
- process-style `clone`, `fork`, `vfork`, `wait4`, or `waitid`;
- descriptor, credential, signal, mount, or namespace inheritance;
- concurrent process scheduling or process-directed signal delivery;
- orphan reparenting, subreapers, process groups, or sessions;
- resource accounting, limits, PID namespaces, or procfs generation from this
  table; or
- general capacity or PID/generation rollover recovery.

The minimal system profile therefore continues to report
`runtime process.fork-wait missing`. A later milestone may change that row only
after PID 1 creates, runs, observes, and reaps an independently owned ring-3
child through the Linux ABI.
