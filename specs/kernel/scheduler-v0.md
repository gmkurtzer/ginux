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

The QEMU boot gate covers two-thread ordering, rejected non-current yield,
single-running-thread state, exit selection, idle behavior, slot reuse, domain
ownership, and stale-ID rejection.

This revision stores no CPU register context, kernel/user stacks, address-space
root, priority, budget, affinity, wait reason, or timer deadline. It does not
preempt or context-switch the ring-3 proof program. Those fields and operations
must be added without weakening the lifecycle invariants defined here.
