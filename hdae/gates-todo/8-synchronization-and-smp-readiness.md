# Gate 8: Synchronization and SMP Readiness

## Goal

Give all shared state concurrency semantics that remain correct across process,
interrupt, and multi-CPU execution.

## Approved scope

- Audit atomics, spinlocks, IRQ-safe locks, memory ordering, and lock order.
- Make allocators, scheduling, process tables, sockets, filesystems, domains,
  and device state SMP-safe.
- Remove reliance on local interrupt masking as global exclusion.
- Add model and randomized concurrency tests where practical.

## Non-goals

- Starting application processors; that belongs to Gate 9.
- CPU hotplug.

## Completed work

None. Depends on Gate 7.

## Remaining work

- Specify primitive selection and global lock order.
- Audit interrupt-context acquisition and memory ordering.
- Implement and stress subsystem synchronization incrementally.

## Acceptance criteria

- Every shared mutable object uses an appropriate documented primitive.
- Interrupt handlers cannot deadlock on locks held by interrupted code on the
  same CPU.
- Verification builds fail closed on lock-order and ownership violations.
- Local interrupt masking is used only for local interrupt semantics, never as
  the sole protection for cross-CPU shared state.
- Concurrent lifecycle, signal, IPC, filesystem, network, and allocator stress
  completes without corruption, deadlock, or leaked ownership.

## Evidence

Synchronization map, lock-order document, model/stress results, interrupt
audit, and full regressions.

## Decisions and limitations

Primitive and memory-order choices require review before broad adoption.

## Delivery strategy

- Working branch: `codex/feedback-hardening`.
- Intended base: `main`.
- Pull request: the shared ten-gate pull request.
- Planned commits: primitives/assertions followed by one synchronized
  subsystem per commit with focused and full-suite evidence.
- Actual commits: pending.

## Review policy and outcome

Checkpoint required. Stop for synchronization review before another CPU is
enabled. Outcome: pending.
