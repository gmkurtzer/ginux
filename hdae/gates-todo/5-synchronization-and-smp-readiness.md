# Gate 5: Synchronization and SMP Readiness

## Goal

Give all shared state concurrency semantics that remain correct across process,
interrupt, and multi-CPU execution.

## Approved scope

- Audit atomics, spin locks, IRQ-safe locks, memory ordering, and lock order.
- Make allocators, scheduling, process tables, sockets, filesystems, domains,
  and device state SMP-safe.
- Remove reliance on local interrupt masking as global exclusion.
- Add model and randomized concurrency tests where practical.

## Non-goals

- Starting application processors.
- CPU hotplug.

## Completed work

None. Depends on Gate 4.

## Remaining work

- Specify primitive selection and global lock order.
- Audit interrupt-context acquisition.
- Implement and stress subsystem synchronization incrementally.

## Acceptance criteria

- Every shared mutable object uses an appropriate documented primitive.
- Interrupt handlers cannot deadlock on locks held by interrupted code on the
  same CPU.
- Verification builds fail closed on lock-order and ownership violations.
- Concurrent lifecycle, signal, IPC, filesystem, and allocator stress
  completes without corruption, deadlock, or leaked ownership.

## Evidence

Synchronization map, lock-order document, model/stress results, interrupt
audit, and regressions.

## Decisions and limitations

Primitive and memory-order choices require review before broad adoption.

## Delivery strategy

Planned commits: primitives/assertions, then one synchronized subsystem per
commit with focused and full-suite evidence.

## Review policy and outcome

Checkpoint required. Stop for synchronization review before another CPU is
enabled. Outcome: pending.
