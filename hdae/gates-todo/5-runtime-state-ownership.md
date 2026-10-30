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

- [x] Added one lifecycle-owner primitive that couples storage, publication,
  and reset state without allocating large runtime objects on the kernel stack.
- [x] Consolidated domain management, IPC, scheduler runtime, and reusable
  process-scheduler workspace state from eight ambient globals into four
  explicit owner roots.
- [x] Added explicit teardown for the reusable process-scheduler workspace.
- [x] Extended writable-global discovery to recognize generic custom
  `UnsafeCell` wrappers rather than letting owner roots escape the inventory.
- [x] Consolidated 13 process-table, address-space, fork scratch, diagnostic,
  and lifecycle globals into one explicit process-runtime owner while
  retaining the existing per-domain instance banks and generation resets.
- [x] Consolidated 14 socket platform, runtime, slot, Unix-socket, and packet
  buffer globals under one explicit domain socket-runtime owner while keeping
  smoltcp's self-referential buffers at stable addresses.

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

- `make architecture-check` -> passed; the enforced inventory decreased from
  620 to 616 globals, with 474 Gate 5 migrations remaining and all four new
  owner roots explicitly allowlisted.
- `make check` -> passed after Clippy rejected and caused removal of an API
  returning a mutable reference directly from an immutable owner reference.
- `make build-verification && bash tests/qemu/boot.sh` -> passed the full
  verification boot, reboot, corruption, lifecycle, and cleanup sequence.
- After process-runtime consolidation, `make check`, `make test-linux-abi`
  (23 interfaces and 375 syscalls), and the full verification QEMU sequence
  all passed; the inventory decreased from 616 to 604 globals with 461 Gate 5
  migrations remaining.
- After socket-runtime consolidation, the same three test layers passed; the
  inventory decreased from 604 to 591 globals with 447 Gate 5 migrations
  remaining.
- Direct `bash tests/qemu/boot.sh` after `make check` was an invalid first
  attempt because the normal image lacks verification-only markers; no QEMU
  process was left behind, and the test was rerun with its required image.

## Decisions and limitations

Shared objects may remain only when Gate 4 explicitly classified them and Gate
8 will supply their required synchronization.

## Delivery strategy

- Working branch: `codex/feedback-hardening`.
- Intended base: `main`.
- Pull request: the shared ten-gate pull request.
- Planned commits: per-CPU/task state, process/thread/signal state, domain state,
  scheduler/process tables, and lifecycle evidence.
- Actual commits: `900d0d56` added explicit shared runtime owner roots;
  `44a18062` consolidated process runtime ownership; socket-runtime
  consolidation pending commit; remaining runtime migrations pending.

## Review policy and outcome

Delegated closure authorized after runtime ownership, lifecycle, isolation, and
regression criteria all pass. Outcome: pending.
