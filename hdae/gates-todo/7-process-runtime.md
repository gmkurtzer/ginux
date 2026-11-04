# Process runtime

## Source

Execution-kernel and dynamic-userspace foundations required for systemd.

## Goal

Provide the bounded Linux process, thread, synchronization, signal, time,
credential, and event behavior required to start systemd and its approved
early helper fixtures.

## Approved scope

- Implement the tested `fork`/`clone`/`exec`/`exit`/`wait` lifecycle profile.
- Implement virtual PID, process-group, session, parent, child, and thread
  identity required by fixtures.
- Implement the tested signal actions, masks, delivery, return, and termination
  behavior.
- Implement futex, clocks, timers, polling, epoll, eventfd, and related blocking
  behavior required by the selected systemd probes.
- Implement virtual credentials, groups, capabilities, resource limits, and
  privilege transitions required by the fixtures without granting host
  authority.
- Add cleanup and generation-safe identity tests for repeated lifecycle use.

## Non-goals

- Complete Linux scheduling, ptrace, job control, namespaces, audit, or every
  process and event syscall.
- Mapping virtual credentials or capabilities to equivalent host privilege.
- Performance optimization beyond preventing pathological test behavior.

## Completed work

- [x] Implement the bounded Ginus process table with virtual PID, TID, parent,
  process-group, session, fork, clone-thread, exec epoch, exit, wait, and reap
  behavior.
- [x] Add generation-safe internal task handles, monotonic virtual PID
  allocation, deterministic exhaustion, invalid-child, stale-handle, shutdown,
  and cleanup behavior.
- [x] Implement virtual credentials, supplementary groups, capability dropping,
  masked/pending signal delivery, bounded futex waits/wakes, and eventfd
  counters.
- [x] Implement virtual signal dispositions, exec reset semantics, monotonic and
  realtime clocks, one-shot/periodic timers, and bounded epoll-style readiness.
- [x] Add the Rocky QEMU process-runtime fixture with 256 repeated virtual
  fork/clone/exec/wait cycles, signal interruption/return, virtual privilege
  drop, synchronization/events/time, stale-identity rejection, and teardown.
- [x] Connect native `fork`, `getpid`, `getppid`, exit, and `wait4` to the
  process model with child auto-attachment and host/virtual PID translation.
- [x] Add 32 native fork/wait cycles with two concurrent children per round,
  interrupted-wait restart coverage, exit-status accounting, and zero-task
  cleanup.
- [x] Add Gate 7 to the offline aggregate suite and provide the nine-check
  human demonstration.

## Remaining work

- [ ] Human review and acceptance of the Gate 7 demonstration and evidence.

## Acceptance criteria

- Approved fixtures repeatedly create, execute, wait for, and reap processes
  and threads without stale identity reuse or resource leakage.
- Signal, futex, timer, and event fixtures observe the documented Linux-style
  results, interruption, timeout, and cleanup behavior.
- Virtual root and capabilities affect only Ginus policy and never confer host
  credentials or capabilities.
- Invalid PIDs, TIDs, selectors, pointers, flags, and lifecycle races fail
  deterministically without crossing domain boundaries or crashing Ginus.
- Blocking operations can be interrupted and domain shutdown can terminate all
  managed work within the documented test timeout.
- Earlier VFS, device, runtime, and interception tests remain passing.
- `make demo-gate-7` lets a human observe managed process creation, signaling,
  waiting, synchronization, and complete lifecycle cleanup.

## Verification plan

- `make test-process` -> lifecycle, identity, signal, futex, time, event,
  credential, race, exhaustion, and cleanup fixtures pass.
- `make test-runtime` -> approved static and dynamic fixtures remain passing.
- Repeated lifecycle stress procedure -> completes the documented iteration
  count with zero leaked managed tasks or backend resources.
- `make test` with network disabled -> exits zero.
- `make demo-gate-7` -> performs the documented process and signal lifecycle,
  verifies cleanup, and exits zero.

## Evidence

- `cargo test -p ginus-process` -> seven lifecycle, identity, credentials,
  signal, futex, eventfd, exhaustion, invalid-selector, and cleanup tests
  passed, including deterministic virtual clocks, periodic timers, and event
  readiness.
- `cargo clippy -p ginus-process --all-targets -- -D warnings` -> passed.
- `make test-process` -> passed the same deterministic component suite.
- Expanded `make test-process` -> passed seven component tests and the Rocky
  Linux 10 QEMU fixture: 256 lifecycle cycles, virtual PID 1, stale-ID denial,
  signal handler/return, interrupted wait, virtual UID/GID drop without host
  change, futex/eventfd/epoll/timer behavior, and zero-task teardown.
- Final `make test-process` -> passed the model fixture plus a native ptrace
  fixture with virtual PID 1, 32 fork/wait cycles, two concurrent children per
  round, child auto-attachment, monotonic PID translation through fork and
  wait, 32 exit-status-23 observations, interrupted/restarted wait argument
  restoration, and zero-task cleanup.
- `make test-runtime` -> passed the earlier static and dynamic execution
  fixtures in Rocky QEMU.
- `make test-native-vfs` -> passed balanced Rocky Bash, sensitive-operation
  denial, supervisor-loss `PTRACE_O_EXITKILL`, unexpected-stop cleanup, and
  immutable-image checks.
- `make demo-gate-7` -> passed all nine named human-visible model and native
  lifecycle checks and reported its non-claims.
- Aggregate `make test` -> passed formatting, strict lint, 38 component unit
  tests, prior interception/runtime/config/domain/image/device regressions,
  and the Gate 7 Rocky QEMU process fixture with no network acquisition.

## Decisions and limitations

- Behavior is implemented to explicit fixtures and documented semantics, not a
  blanket Linux process-conformance claim.
- Unsupported privilege transitions must fail explicitly and remain virtual.
- The native fork return is translated after `PTRACE_EVENT_FORK` through a
  temporary, restored post-syscall software breakpoint. This is bounded Gate 7
  proof machinery, not yet a production mechanism for arbitrary multithreaded
  code sharing the same instruction mapping.
- Native signal-frame construction and native clone-thread semantics remain
  limited to the approved fixture surface; the model covers signal return and
  clone-thread identity, and Gate 9 will expose additional systemd requirements.
- Stress exposed and fixed three fail-closed boundary defects: raw
  `PTRACE_PEEKTEXT` result handling, the auto-attached child's initial trace
  phase, and restoration of virtual wait selectors across kernel restart.

## Delivery strategy

- Working branch: `hdae/process-runtime`
- Intended base: `main`
- Pull request: https://github.com/gmkurtzer/usshim/pull/7; dedicated branch
  push and PR creation authorized on 2026-11-03; never merge or close without
  explicit authorization
- Planned commits: process and thread lifecycle; signals, synchronization, and
  time; credentials and event interfaces; stress and cleanup verification
- Actual commits: `440c98f` added the process-runtime foundation; `b385ed6`
  added virtual time, signal-action, and event behavior; `0801d80` added the
  QEMU model fixture; `6780e35` added native multi-tracee supervision, stress,
  aggregate coverage, and the human demonstration; the final evidence
  checkpoint is recorded by the commit containing this gate update

## Review policy and outcome

Human checkpoint. Gate 7 is technically passed and remains open pending
explicit human review. Process identity and privilege behavior are security
boundaries and require acceptance before virtual kernel views build on them.
