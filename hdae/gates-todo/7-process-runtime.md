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

- [ ] None; gate not started.

## Remaining work

- [ ] Implement bounded process/thread creation and lifecycle.
- [ ] Implement signals, synchronization, time, and events.
- [ ] Implement virtual identity, credentials, and capability behavior.
- [ ] Add concurrency, race, invalid-selector, exhaustion, and cleanup tests.

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

- Not run.

## Decisions and limitations

- Behavior is implemented to explicit fixtures and documented semantics, not a
  blanket Linux process-conformance claim.
- Unsupported privilege transitions must fail explicitly and remain virtual.

## Delivery strategy

- Working branch: `hdae/process-runtime`
- Intended base: `main`
- Pull request: intended; dedicated branch push and PR creation authorized on
  2026-11-03; never merge or close without explicit authorization
- Planned commits: process and thread lifecycle; signals, synchronization, and
  time; credentials and event interfaces; stress and cleanup verification
- Actual commits: pending

## Review policy and outcome

Human checkpoint. Process identity and privilege behavior are security
boundaries and require explicit review before virtual kernel views build on
them. Present `make demo-gate-7`, fresh evidence, and the pull request before
acceptance.
