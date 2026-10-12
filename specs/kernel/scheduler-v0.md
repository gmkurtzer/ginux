# Scheduler State Machine v0

**Status:** Cooperative primitives plus rearmed one-shot preemption verified.

The bootstrap scheduler owns eight fixed thread slots. A thread identifier
contains a slot and generation, and each thread belongs to one native domain.
The current lifecycle is:

`absent -> (Staged | Ready) -> Running -> (Ready | Blocked) ... -> absent`

Only the scheduler selects a `Ready` thread as `Running`. A yield is accepted
only from the current running identity, returns it to `Ready`, and selects the
next ready slot round-robin. Exit is likewise restricted to the current thread,
removes it, and selects another ready thread. Slot reuse increments generation,
so stale identifiers cannot observe or control a replacement.
An auxiliary submitted by a running control-plane context begins `Staged`.
Every scheduler transition leaves staged work ineligible; only an explicit
management start changes one exact staged context to `Ready`. This gives the
control plane a deterministic interval in which it can initialize or cancel a
submitted workload. Cancellation removes only an exact `Staged` or `Ready` domain,
reverses its thread charge, and cannot remove a running or blocked context.
A staged auxiliary may receive one bounded private-stack payload. Successful
initialization sets its saved `RSI` to the target virtual address and `RDX` to
the byte length without changing readiness or exposing the source mapping.

A blocking IPC receive atomically saves the current context, marks it
`Blocked`, and selects another ready thread. If none exists, the transition is
rolled back and the caller remains `Running`. A peer send writes its word into
the sole blocked peer's saved result register and changes it to `Ready` only
when the IPC object, endpoint direction, and expected peer generation exactly
match the saved wait key. Object, direction, or generation mismatches leave the
waiter unchanged. Waking does not grant authority or attach a new thread.

Each thread owns a C-layout saved context containing all general-purpose
registers, user RIP/RSP/RFLAGS, its page-table root, and its lower-canonical
FS base. Creation and yield
reject null or non-lower-canonical instruction/stack pointers, a missing
mandatory RFLAGS bit, a cleared interrupt-enable flag, user I/O privilege,
nested-task or virtual-8086 state, null or unaligned roots, and a noncanonical
FS base. Requiring IF at
creation and every saved-context transition prevents the kernel from admitting
a user context that cannot be preempted by maskable interrupts. Yield
atomically replaces the current thread's saved context before choosing another
thread. Trap and timer-preemption paths capture the active `IA32_FS_BASE` with
the general context. Every scheduler resume returns the selected FS base with
its page-table root; the architecture boundary validates, programs, and reads
back both before returning to ring 3. Initial control, service, and auxiliary
entries likewise install the selected scheduler context's FS base.
Every scheduler-driven address-space activation also validates the selected
context's saved RIP and RSP against the fixed executable and stack mappings in
that root. Structurally valid pointers into guards, absent pages, or a stack's
one-past-end boundary therefore fail before `IRETQ` rather than depending on a
subsequent user fault.

The QEMU boot gate covers two-thread ordering, rejected non-current yield,
single-running-thread state, exit selection, idle behavior, slot reuse, domain
ownership, stale-ID rejection, invalid-context rejection, and preservation of a
modified register context and distinct FS bases across another thread's run and
exit. The current native payloads use FS base zero; Linux scheduler integration
with the live static-TLS runtime remains a separate milestone.

The bounded process proof now converts a process-owned context into this same
scheduler representation using the exact generation-checked root recorded in
its address-space bundle. It admits PID 1 and one eagerly cloned child under a
synthetic container domain, rejects domain zero and duplicate admission,
selects both contexts through normal round-robin yield, and matches their full
register, root, stack, flags, and FS state. A process record retains the exact
scheduler thread identity and cannot begin address-space teardown until that
binding is withdrawn. Withdrawal handles an exact ready or running identity;
the proof rejects reuse of the retired child identity. While the child is
selected, its validated root is installed in CR3 and then replaced with the
exact prior root, proving the scheduler-selected address space is safe for
kernel execution. The synthetic child then enters ring 3 with its selected RIP,
RSP, RFLAGS, zero fork return, and FS base, writes its private page, and returns
through the user gate. The dedicated entry restores all fifteen general
registers; the gate captures the complete frame and requires it to equal the
recorded child context, resumes ring 3, and captures the child's subsequent
`exit(42)` trap. That captured status drives teardown, zombie publication, and
the synthetic parent's wait. The live Linux PID 1 runtime is not yet attached
to this process scheduler path.

Scheduler capacity does not grant permission to create a thread. Runtime
creation first reserves a slot from the generation-checked owning domain; exit
and fault paths release that reservation before a service domain may be
retired. The current bootstrap policy permits one attached thread per domain.

The measured ring-3 proof creates one control-plane context and one initial
IPC-only service-domain context. Each owns a distinct page-table root, executable code frame, and writable,
non-executable stack preceded by an absent guard page. `INT 0x80` saves the
hardware trap frame into the current scheduler record. Yield selects the other
context, activates its recorded CR3, and restores its registers through
`IRETQ`. The APIC one-shot performs the same transition from a full interrupt
frame, without cooperation from the running context. Exit destroys the current
slot and either resumes the remaining context or returns to the kernel. In the
current proof, a timer preempts context 1 to context 2, which receives an IPC request, sends its reply, and raises an
expected null-page write fault. The scheduler removes it, queues one replacement
service generation on the retained service address space, rearms the timer, and
switches back to context 1. The survivor consumes the reply and fault event,
then the second preemption dispatches the replacement. It negotiates the ABI
and blocks on an empty inbound IPC direction, resuming the control context.
The control context directly wakes it with request 3, yields until it receives
acknowledgement 4, then wakes it again with GET command 5 and requires the
retained value 3. A third wake sends unsupported command 6 and requires an
explicit error before it stages and cancels one
auxiliary, stages, initializes, and explicitly starts a distinct auxiliary,
then voluntarily yields. After the replacement and auxiliary finish, the
control context resumes, consumes the auxiliary's exit record, verifies budget
exhaustion, and exits.
The awakened replacement replies and exits cleanly; the admitted auxiliary validates its
private payload and exits with status 42. The boot
succeeds only after exactly two preemptions, six cooperative yields, one
contained primary-service fault, three normal exits, one restart, three
block/wake pairs, and thirteen or fourteen cross-root dispatches depending on the valid
timer interleaving.
The initial one-shot cannot select the service before the control plane has
successfully published that request. An earlier expiration is acknowledged and
rearmed without changing the current context or the exact two-preemption audit.
Before the replacement becomes runnable, restart setup may remove its exact
ready context as a rollback operation. This also reverses the domain thread
charge and restores the retained address-space owner recorded by the scheduler;
it cannot remove a running, blocked, ambiguous, or wrong-domain context.

Call budgets belong to domains rather than threads. The control domain receives
70 charged calls and each service generation receives twenty-three. Every call-gate
entry except thread exit is charged before dispatch, including denied
operations and yield. Exhaustion returns `UINT64_MAX - 3`; exit remains
available so a depleted domain is not trapped. The ring-3 proof accounts for
exactly 82 charged calls across all generations, deliberately exhausts the
control domain, and requires its next discovery call to be rejected before it
exits.

This revision uses one guarded TSS privilege stack because execution is
single-CPU and syscalls cannot nest. It stores no priority, CPU-time budget,
affinity, wait reason, timer deadline, periodic tick, or quantum policy. The
two explicitly armed one-shots are a bring-up proof rather than a complete
preemptive scheduler.
The call budget is a bring-up resource bound, not a complete
resource-accounting policy. Address-space switching currently flushes the local
TLB through CR3 reload; PCID, TLB shootdown, and multi-CPU ownership are not
defined.
