# Bounded Linux Private Futex Contract v0

**Status:** Ready for `minimal-system-v0`; deliberately not a general futex
implementation.

The Linux syscall ABI accepts `FUTEX_WAIT`, `FUTEX_WAIT_PRIVATE`, `FUTEX_WAKE`,
and `FUTEX_WAKE_PRIVATE`. The futex word must be four-byte aligned and readable.
A wait compares the complete 32-bit word before changing scheduler state and
returns `-EAGAIN` when it differs from the expected value. Bad addresses return
`-EFAULT`; bad alignment, operation, or timeout fields return `-EINVAL`.

A matching wait is admitted only in the bounded two-thread runtime while the
peer is ready. The kernel saves the caller's complete return context in its
generation-tagged scheduler slot, records the exact user address, blocks the
caller, and selects the peer. Wake scans the bounded thread table for that exact
address and makes up to the requested number of matches ready. A matching wait
with no runnable peer returns `-ENOSYS` instead of deadlocking the domain.

A null timeout waits indefinitely within that graph. A zero relative timeout
returns `-ETIMEDOUT`; a positive relative timeout becomes a checked monotonic
deadline. A Linux scheduling boundary or the periodic local-APIC tick clears an
expired wait and resumes it with `-ETIMEDOUT`.

Child exit copies zero to its registered clear-child-TID address before waking
one exact futex waiter. It then clears both the clear-child-TID and robust-list
registrations before retiring the generation-tagged thread slot. The robust
list is not traversed and no owner-death recovery is attempted.

The measured minimal-system PID 1 proves the public private-operation ABI with
an aligned word initialized to one. `FUTEX_WAIT_PRIVATE` with expected value
zero must return `-EAGAIN` without changing the word, and
`FUTEX_WAKE_PRIVATE` with limit one must return zero when no waiter exists. It
then publishes exactly one `FUTEX_READY` line.

The separate measured two-thread execution graph supplies the stateful proof:
four waits block, two explicit wakes occur, one positive timeout expires, and
one child-exit clear-child-TID wake occurs. Final teardown requires every wait
record and thread slot to be reclaimed.

This version excludes multiple simultaneous waiters, general queue ordering,
cross-process/shared keys, bitset operations, requeueing, priority-inheritance
futexes, restartable waits, and robust-futex recovery. Unsupported operations
fail with `-EINVAL`; behavior outside the admitted two-thread graph is not a
compatibility claim.
