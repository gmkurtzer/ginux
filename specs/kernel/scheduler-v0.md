# Scheduler State Machine v0

**Status:** Verified metadata state machine; no assembly context switching yet.

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

This revision does not yet own per-thread kernel stacks and stores no priority,
budget, affinity, wait reason, or timer deadline. It does not preempt or switch
the ring-3 proof program; assembly save/restore must consume this context without
weakening the lifecycle invariants defined here.
