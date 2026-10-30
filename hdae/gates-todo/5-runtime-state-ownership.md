# Gate 5: Runtime State Ownership

## Goal

Move process, thread, signal, syscall scratch, scheduling, and domain runtime
state out of ambient globals and into explicitly owned lifecycle objects.

## Approved scope

- Per-CPU entry and current-task state not already completed by Gate 2.
- Per-domain, per-process, and per-thread runtime state.
- Signal queues/delivery, syscall scratch, process tables, and scheduler state.
- Construction, reset, exit, teardown, and reuse tests.

## Non-goals

- Device, filesystem, network, storage, and controller-service state; those
  belong to Gate 6.
- SMP activation or broad synchronization implementation.

## Completed work

None. Depends on Gate 4.

## Remaining work

- Migrate each runtime ownership group defined by Gate 4.
- Remove obsolete singleton scratch paths and raw mutable statics.
- Add repeated lifecycle and cross-domain contamination tests.

## Acceptance criteria

- Runtime state is reachable through its explicit owner rather than ambient
  global lookup or singleton scratch storage.
- Process exit, thread exit, domain teardown, restart, and identifier reuse
  reclaim or reset all owned state.
- Concurrent domain/process test models show no cross-owner contamination.
- No runtime writable global remains outside the Gate 4 allowlist.
- Existing ABI, Rocky, domain, and performance tests do not materially regress.

## Evidence

Before/after inventory, ownership API review, lifecycle stress results,
cross-domain negative tests, and full regression transcript.

## Decisions and limitations

Shared objects may remain only when Gate 4 explicitly classified them and Gate
8 will supply their required synchronization.

## Delivery strategy

- Working branch: `codex/feedback-hardening`.
- Intended base: `main`.
- Pull request: the shared ten-gate pull request.
- Planned commits: per-CPU/task state, process/thread/signal state, domain state,
  scheduler/process tables, and lifecycle evidence.
- Actual commits: pending.

## Review policy and outcome

Delegated closure authorized after runtime ownership, lifecycle, isolation, and
regression criteria all pass. Outcome: pending.
