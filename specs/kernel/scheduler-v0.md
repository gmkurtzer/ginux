# Scheduler State Machine v0

**Status:** Cooperative two-context switching verified in hardware.

The bootstrap scheduler owns eight fixed thread slots. A thread identifier
contains a slot and generation, and each thread belongs to one native domain.
The current lifecycle is:

`absent -> Ready -> Running -> Ready ... -> absent`

Only the scheduler selects a `Ready` thread as `Running`. A yield is accepted
only from the current running identity, returns it to `Ready`, and selects the
next ready slot round-robin. Exit is likewise restricted to the current thread,
removes it, and selects another ready thread. Slot reuse increments generation,
so stale identifiers cannot observe or control a replacement.

Each thread owns a C-layout saved context containing all general-purpose
registers, user RIP/RSP/RFLAGS, and its page-table root. Creation and yield
reject null or non-lower-canonical instruction/stack pointers, a missing
mandatory RFLAGS bit, and null or unaligned roots. Yield atomically replaces
the current thread's saved context before choosing another thread.

The QEMU boot gate covers two-thread ordering, rejected non-current yield,
single-running-thread state, exit selection, idle behavior, slot reuse, domain
ownership, stale-ID rejection, invalid-context rejection, and preservation of a
modified register context across another thread's run and exit.

The measured ring-3 proof creates one control-plane context and one initial
IPC-only service-domain context. Each owns a distinct page-table root, executable code frame, and writable,
non-executable stack preceded by an absent guard page. `INT 0x80` saves the
hardware trap frame into the current scheduler record. Yield selects the other
context, activates its recorded CR3, and restores its registers through
`IRETQ`; exit destroys the current slot and either resumes the remaining
context or returns to the kernel. In the current proof, context 1 yields to
context 2, which receives an IPC request, sends its reply, and raises an
expected invalid-opcode fault. The scheduler removes it, queues one replacement
service generation on the retained service address space, and switches back to
context 1. The survivor consumes the reply and exits; the replacement then
negotiates the ABI and exits cleanly. The boot succeeds only after exactly one
yield, one contained fault, two normal exits, one restart, and three cross-root
dispatches.

Each created thread also receives a fixed ten-call bootstrap budget. Every
call-gate entry is charged before dispatch, including denied operations, yield,
and exit. Exhaustion rejects further dispatch. The ring-3 proof accounts for
exactly fifteen calls across its three contexts, demonstrating that scheduling
does not merge their budgets.

This revision uses one guarded TSS privilege stack because execution is
single-CPU and syscalls cannot nest. It stores no priority, CPU-time budget,
affinity, wait reason, or timer deadline and provides
no preemption. The syscall counter is a bring-up resource bound, not a complete
resource-accounting policy. Address-space switching currently flushes the local
TLB through CR3 reload; PCID, TLB shootdown, and multi-CPU ownership are not
defined.
