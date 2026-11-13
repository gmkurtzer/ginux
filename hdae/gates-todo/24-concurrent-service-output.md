# Concurrent service output delivery

## Source
Discovered while verifying Gate 23 with `make test-multi-instance`. Two Rocky
systemd instances reach login and their Gate 17 oneshot checkers complete, but
the checkers' `journal+console` stdout markers reach neither the instance
journal nor the virtual console, causing deterministic acceptance timeout.

## Goal
Preserve each managed service's standard output through concurrent native
instance supervision so systemd journal and virtual-console consumers receive
complete, instance-isolated output.

## Approved scope
- Reproduce and identify where successful managed service stdout is lost during
  two concurrent Rocky systemd instance boots.
- Correct the narrow runtime, descriptor, or console-delivery defect rather
  than weakening the Gate 17 assertions or replacing output evidence with a
  host-side shortcut.
- Add focused regression evidence that uniquely marked service output reaches
  the correct instance and never its peer.
- Rerun Gate 23's concurrent-instance criterion and relevant aggregate/systemd
  regressions.

## Non-goals
- Change instance contracts, shared-network semantics, systemd service
  definitions, or console authentication behavior.
- Relax output, isolation, lifecycle, or timeout assertions merely to make the
  test pass.
- Change the Gate 23 virtual `sysinfo` semantics.

## Completed work
- [x] Reproduce the failure on the retained Gate 17 host and a fresh host
  overlay with a different port.
- [x] Confirm both checkers create private artifacts and exit successfully
  while their stdout markers are absent from journal and console.
- [x] Obtain human approval to create, execute, and publish this corrective
  gate for review.
- [x] Localize the regression to immediate task-record removal after a
  `waitid` reap in merged pull request 247.
- [x] Retain a bounded, pidfd-pinned virtual credential tombstone after task
  and process slots are reclaimed.
- [x] Add deterministic credential-after-reap coverage and pass the unchanged
  concurrent service-output acceptance test.
- [x] Pass static, aggregate, and complete Rocky systemd verification.

## Remaining work
- [ ] Push the gate branch and open its stacked pull request.
- [ ] Stop for human checkpoint review without merging.

## Acceptance criteria
- A successful service's uniquely marked stdout reaches its own instance
  console and systemd journal during concurrent instance execution.
- Neither instance receives its peer's marker or host-only output.
- `make test-multi-instance` passes all existing isolation, contract, overlay,
  virtual-state, console, network, lifecycle, and recovery assertions.
- Gate 23's full Rocky systemd behavior and aggregate regressions remain green.

## Verification plan
- Focused deterministic native-supervision regression -> a queued UNIX message
  received after `waitid` reaps its sender retains the sender's virtual PID,
  UID, and GID.
- `make check` -> formatting, build, clippy, documentation, and static checks
  pass.
- `make test-multi-instance` -> complete Gate 17 concurrent acceptance passes.
- `make test-systemd` -> complete Gate 23 Rocky systemd matrix passes.
- `make test` -> aggregate unit and QEMU integration regressions pass.

## Evidence
- Pre-fix `make test-multi-instance` -> timed out waiting for
  `GINUX_GATE17_INSTANCE` after both instances reached login.
- Fresh-overlay equivalent run -> reproduced the same missing marker.
- Preserved-host inspection -> private checker artifacts exist; systemd reports
  successful oneshot completion; marker is absent from both journal and
  console.
- Commit bisect with the Gate 23 sysinfo correction applied to every candidate
  -> `d23c0cff` and earlier pass Gate 17; `e5be0578` (merge of pull request
  247) introduces the deterministic missing-output timeout.
- Descriptor and syscall tracing -> each checker successfully writes its
  unique marker to its per-instance journald UNIX socket; the later receiver
  loses the sender mapping only after systemd reaps the short-lived process.
- Focused native-supervision suite -> all existing cases pass, including the
  new check that reaps a sender before `recvmsg` and still receives the correct
  virtual PID, UID, and GID.
- `make check` -> passed.
- `make test-multi-instance` -> passed the complete unchanged Gate 17 matrix;
  both unique service markers reached their own journal and console, neither
  reached its peer, and all isolation, contract, network, restart, and teardown
  assertions passed.
- `make test-systemd` -> passed cold, warm, repeat, offline, DNF-failure
  integrity, configured-startup-failure, and controlled-shutdown checks.
- `make test` -> passed static checks, unit tests, image and host lifecycle
  checks, and every aggregate QEMU integration.

## Decisions and limitations
- Gate 23 remains partial until this gate passes and its concurrent criterion
  is rerun.
- Missing output is treated as a product regression because systemd explicitly
  routes the service to `journal+console`; the fix must not substitute a test
  artifact for that operator-visible interface.
- Reclaimed tasks move only their immutable virtual credential mapping into a
  128-entry bounded cache. Each retained pidfd pins the exact host identity;
  cache eviction closes the pin before that mapping is discarded, preventing
  host-PID reuse from being translated as an older virtual sender.

## Delivery strategy
- Working branch: `hdae/concurrent-service-output`
- Intended base: `hdae/virtual-sysinfo-systemd-compat`
- Pull request: intended as a stacked pull request; publication explicitly
  authorized by the human on 2026-11-12
- Planned commits: signed gate/control-plane commit; signed implementation and
  focused-regression commit; signed evidence and delivery reconciliation commit
- Actual commits: `b39f0324` (gate/control plane), `f3b7ed6a`
  (implementation and focused regression), `5309eff3` (passing evidence),
  delivery reconciliation pending

## Review policy and outcome
Technically passed checkpoint gate. Publish it, then stop for human review. Do
not merge or close either Gate 23 or Gate 24 pull request.
