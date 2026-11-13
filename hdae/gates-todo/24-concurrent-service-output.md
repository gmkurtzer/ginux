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

## Remaining work
- [ ] Localize the output-loss root cause.
- [ ] Implement the narrow correction and focused regression.
- [ ] Run all verification commands and record fresh evidence.
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
- Focused deterministic service-output regression -> two concurrent services
  emit unique markers; each marker is present only in its originating instance
  console and journal.
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

## Decisions and limitations
- Gate 23 remains partial until this gate passes and its concurrent criterion
  is rerun.
- Missing output is treated as a product regression because systemd explicitly
  routes the service to `journal+console`; the fix must not substitute a test
  artifact for that operator-visible interface.

## Delivery strategy
- Working branch: `hdae/concurrent-service-output`
- Intended base: `hdae/virtual-sysinfo-systemd-compat`
- Pull request: intended as a stacked pull request; publication explicitly
  authorized by the human on 2026-11-12
- Planned commits: signed gate/control-plane commit; signed implementation and
  focused-regression commit; signed evidence and delivery reconciliation commit
- Actual commits: pending

## Review policy and outcome
Human-approved checkpoint gate. Execute and publish it, then stop for human
review. Do not merge or close either Gate 23 or Gate 24 pull request.
