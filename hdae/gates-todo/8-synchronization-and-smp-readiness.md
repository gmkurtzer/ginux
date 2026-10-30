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

- [x] Specified ticket-spinlock, IRQ-safe locking, atomic publication, memory
  ordering, interrupt-context, and global lock-order rules.
- [x] Added ranked `SpinLock<T>` and `IrqSpinLock<T>` primitives with
  fail-closed order checking and an in-kernel boot contract test.
- [x] Replaced the physical frame allocator's single-CPU `UnsafeCell` access
  with a rank-20 IRQ-safe lock across allocation, ownership, transfer,
  accounting, scrubbing, and release paths.

## Remaining work

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

- [`docs/feedback-hardening/SYNCHRONIZATION.md`](../../docs/feedback-hardening/SYNCHRONIZATION.md)
  defines primitive selection, memory ordering, interrupt rules, and the
  global rank order.
- `cargo fmt --all`, `make architecture-check`, and `make check` pass with the
  four new synchronization globals explicitly classified.
- `make build-verification && bash tests/qemu/boot.sh` reports
  `ginux: ranked synchronization primitives ready`, completes all kernel and
  Linux runtime checks, shuts down cleanly, and ends with `Boot test passed`.
- The physical allocator migration passed formatting, architecture and global
  ownership checks, `make check`, and the complete deterministic QEMU suite;
  frame reservation, zeroing, ownership, stale-lease, and domain-budget
  invariants all remained successful.

## Decisions and limitations

The approved batch delegation authorizes the documented ticket-lock,
IRQ-preserving wrapper, acquire/release publication, and ascending rank model.
Any incompatible primitive or order change remains an architecture decision.

## Delivery strategy

- Working branch: `codex/feedback-hardening`.
- Intended base: `main`.
- Pull request: the shared ten-gate pull request.
- Planned commits: primitives/assertions followed by one synchronized
  subsystem per commit with focused and full-suite evidence.
- Actual commits: `ff07cdd9` adds the synchronization contract and primitives;
  physical allocator migration pending commit.

## Review policy and outcome

Delegated closure authorized after synchronization, lock-order, interrupt, and
stress criteria pass. Outcome: pending.
