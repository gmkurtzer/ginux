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
- [x] Added a coarse rank-10 IRQ-safe serialization boundary to native and
  Linux syscall, user-fault, timer-interrupt, and virtio-interrupt entry.
- [x] Audited direct interrupt masking and documented every remaining use as
  local interrupt admission, waiting, calibration, or guard restoration rather
  than cross-CPU exclusion.
- [x] Added a fail-closed architecture check for the shared entry boundary,
  locked physical allocator, and removal of stale single-CPU assumptions.
- [x] Made contiguous-frame validation, scrubbing, and release one locked
  transaction so another CPU cannot recycle an extent while it is scrubbed.

## Remaining work

None. Gate 9 starts application processors and therefore owns observable
cross-CPU subsystem stress; Gate 8 established and verified the synchronization
contract that makes those runs safe to begin.

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
- The first coarse entry lock correctly failed the deterministic QEMU test at
  a nested auxiliary user fault with a lock-order panic. This proved that a
  syscall may retain its kernel frame while launched ring-3 work re-enters.
  Replacing that boundary with per-CPU recursive depth plus cross-CPU ticket
  exclusion passed the same complete QEMU suite and clean shutdown.
- `tools/architecture/check-synchronization.sh` fails closed if the Linux or
  native syscall, user-fault, timer, or virtio entry-owner file loses the
  shared boundary; if the allocator loses its IRQ-safe lock; or if the process
  owner regains a single-CPU serialization claim.
- `cargo fmt --all`, `make architecture-check`, and `make check` pass after the
  interrupt audit and locked scrub/release transaction.
- `make build-verification && bash tests/qemu/boot.sh` passes all lifecycle,
  signal, IPC, filesystem, networking, allocator, Linux-runtime, and clean
  shutdown checks and ends with `Boot test passed`.

## Decisions and limitations

The approved batch delegation authorizes the documented ticket-lock,
IRQ-preserving wrapper, acquire/release publication, and ascending rank model.
Any incompatible primitive or order change remains an architecture decision.
The coarse kernel-entry lock intentionally serializes shared runtime owners
until Gate 9 introduces and tests finer concurrent scheduling. Gate 8 cannot
observe cross-CPU execution without violating its explicit AP-startup non-goal;
the mandatory 1/2/4-vCPU stress matrix remains a Gate 9 criterion rather than
being claimed from single-CPU evidence.

## Delivery strategy

- Working branch: `codex/feedback-hardening`.
- Intended base: `main`.
- Pull request: the shared ten-gate pull request.
- Planned commits: primitives/assertions followed by one synchronized
  subsystem per commit with focused and full-suite evidence.
- Actual commits: `ff07cdd9` adds the synchronization contract and primitives;
  `360921a0` synchronizes physical allocation; `a0bd2fc0` adds the recursive
  cross-CPU-exclusive kernel-entry boundary; `19dba5ef` enforces the
  synchronization architecture and locks frame scrubbing.

## Review policy and outcome

Closed under the human-confirmed batch delegation after the synchronization,
lock-order, interrupt audit, static fail-closed enforcement, full subsystem
regression, and clean QEMU shutdown criteria passed. Observable cross-CPU
subsystem stress remains mandatory in Gate 9, where AP startup is in scope.
