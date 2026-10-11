# Scheduler State Machine v0

**Status:** Cooperative primitives plus rearmed one-shot preemption verified.

The bootstrap scheduler owns eight fixed thread slots. A thread identifier
contains a slot and generation, and each thread belongs to one native domain.
The current lifecycle is:

`absent -> Ready -> Running -> (Ready | Blocked) ... -> absent`

Only the scheduler selects a `Ready` thread as `Running`. A yield is accepted
only from the current running identity, returns it to `Ready`, and selects the
next ready slot round-robin. Exit is likewise restricted to the current thread,
removes it, and selects another ready thread. Slot reuse increments generation,
so stale identifiers cannot observe or control a replacement.

A blocking IPC receive atomically saves the current context, marks it
`Blocked`, and selects another ready thread. If none exists, the transition is
rolled back and the caller remains `Running`. A peer send writes its word into
the sole blocked peer's saved result register and changes it to `Ready` only
when the IPC object and endpoint direction exactly match the saved wait key.
Object or direction mismatches leave the waiter unchanged. Waking does not
grant authority or attach a new thread.

Each thread owns a C-layout saved context containing all general-purpose
registers, user RIP/RSP/RFLAGS, and its page-table root. Creation and yield
reject null or non-lower-canonical instruction/stack pointers, a missing
mandatory RFLAGS bit, a cleared interrupt-enable flag, user I/O privilege,
nested-task or virtual-8086 state, and null or unaligned roots. Requiring IF at
creation and every saved-context transition prevents the kernel from admitting
a user context that cannot be preempted by maskable interrupts. Yield
atomically replaces the current thread's saved context before choosing another
thread.
Every scheduler-driven address-space activation also validates the selected
context's saved RIP and RSP against the fixed executable and stack mappings in
that root. Structurally valid pointers into guards, absent pages, or a stack's
one-past-end boundary therefore fail before `IRETQ` rather than depending on a
subsequent user fault.

The QEMU boot gate covers two-thread ordering, rejected non-current yield,
single-running-thread state, exit selection, idle behavior, slot reuse, domain
ownership, stale-ID rejection, invalid-context rejection, and preservation of a
modified register context across another thread's run and exit.

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
The control context directly wakes it with a message, verifies budget
exhaustion, and exits. The awakened replacement observes the message and exits
cleanly. The boot succeeds only after exactly two preemptions, no cooperative
yields, one contained fault, two normal exits, one restart, one block/wake
pair, and five cross-root dispatches.

Call budgets belong to domains rather than threads. The control domain receives
11 charged calls and each service generation receives three. Every call-gate
entry except thread exit is charged before dispatch, including denied
operations and yield. Exhaustion returns `UINT64_MAX - 3`; exit remains
available so a depleted domain is not trapped. The ring-3 proof accounts for
exactly 16 charged calls, deliberately exhausts the control domain, and requires
its next discovery call to be rejected before it exits.

This revision uses one guarded TSS privilege stack because execution is
single-CPU and syscalls cannot nest. It stores no priority, CPU-time budget,
affinity, wait reason, timer deadline, periodic tick, or quantum policy. The
two explicitly armed one-shots are a bring-up proof rather than a complete
preemptive scheduler.
The call budget is a bring-up resource bound, not a complete
resource-accounting policy. Address-space switching currently flushes the local
TLB through CR3 reload; PCID, TLB shootdown, and multi-CPU ownership are not
defined.
