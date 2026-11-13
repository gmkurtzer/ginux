# Systemd-compatible virtual sysinfo memory

## Source
Full post-integration verification on `main` after accepted Gate 22. Rocky
systemd aborts before login because Gate 22 reports zero physical memory from
`sysinfo`, which violates systemd's positive-memory requirement.

## Goal
Allow unmodified Rocky systemd to boot while keeping `sysinfo` deterministic,
instance-scoped, and free of host memory or topology disclosure.

## Approved scope
- Report `sysinfo.totalram` from the instance's configured
  `[resources].memory_mib`, with `mem_unit = 1`.
- Keep uptime, load, free RAM, shared RAM, buffer RAM, swap, high memory, and
  process-count fields zero rather than copying host values.
- Apply the same virtual structure to configured launch and native/systemd
  supervision paths.
- Retain `-EFAULT` for an unwritable result pointer.
- Add focused launch/native regression assertions and rerun the aggregate,
  Rocky systemd, and concurrent-instance suites.

## Non-goals
- Enforce the configured memory value as a host or instance memory quota.
- Expose host memory availability, load, uptime, process counts, or topology.
- Change `/proc/meminfo`, resource enforcement, or any other virtual kernel
  interface.

## Completed work
- [x] Reproduce and diagnose the Rocky systemd failure on current `main`.
- [x] Align the corrective gate and dedicated delivery branch with the human.
- [x] Implement configured virtual total memory in both supervisor paths.
- [x] Add focused regression coverage for virtual memory and private counters.
- [x] Pass static, aggregate, and complete Rocky systemd verification.
- [x] Commit the gate and implementation with sign-off.
- [x] Resolve the separately discovered concurrent service-output regression
  through Gate 24 and pass `make test-multi-instance` unchanged.

## Remaining work
- [ ] Push the branch and open its pull request to `main`.
- [ ] Complete human checkpoint review after every criterion passes.

## Acceptance criteria
- Successful x86-64 `sysinfo` reports `totalram = memory_mib * 1024 * 1024`
  and `mem_unit = 1` in both launch and native supervision.
- Every other host-derived `sysinfo` counter remains zero.
- A null or unwritable result pointer returns `-EFAULT` without host disclosure.
- Rocky systemd reaches the existing login/readiness markers.
- Aggregate and concurrent-instance regressions pass unchanged.

## Verification plan
- `make check` -> formatting, build, clippy, documentation, and static checks
  pass.
- `make test` -> aggregate unit and QEMU integration regressions pass.
- `make test-systemd` -> the complete Rocky systemd matrix reaches its existing
  readiness markers and controlled teardown.
- `make test-multi-instance` -> concurrent instances remain isolated and pass
  their existing lifecycle checks.

## Evidence
- Pre-fix `make test-systemd` -> failed before login with systemd exit 71 and
  `Assertion 'sc > 0' failed ... physical_memory()` because `sysinfo.totalram`
  was zero.
- Pre-fix `make doctor build check test` -> passed, localizing the regression
  to the full systemd acceptance path.
- `make check` -> passed after implementation.
- Isolated permitted `make test` with a dedicated QEMU host port and overlay ->
  passed all unit, image, lifecycle, and QEMU integration checks, including the
  updated launch/native sysinfo fixture.
- Isolated permitted `make test-systemd` -> passed cold, warm, repeat, offline,
  DNF-failure-integrity, configured-startup-failure, and shutdown checks. The
  pre-fix systemd physical-memory assertion did not recur.
- `make test-multi-instance` -> failed waiting for the Gate 17 checker marker
  even though both Rocky instances reached login. A materially different rerun
  on a fresh host overlay and port reproduced the missing marker.
- Read-only preserved-host diagnosis -> both instance-private checker artifacts
  existed and the oneshot service exited successfully, but its expected stdout
  marker was absent from both the instance journal and virtual console. This is
  a separate concurrent service-output regression, not a sysinfo failure.
- Post-Gate-24 `make test-multi-instance` -> passed the complete unchanged Gate
  17 concurrent-instance acceptance matrix.
- Post-Gate-24 `make test-systemd` -> passed cold, warm, repeat, offline,
  DNF-failure-integrity, configured-startup-failure, and shutdown checks.
- Post-Gate-24 `make test` -> passed the complete aggregate suite.

## Decisions and limitations
- The configured virtual memory value is already the authoritative
  instance-visible total used by `/proc/meminfo`; reusing it keeps the two
  virtual interfaces consistent without disclosing host state.
- This value is descriptive virtual topology, not quota enforcement. Resource
  enforcement remains outside this corrective gate.
- Accepted Gate 22 remains locked; this file owns the compatibility correction.
- Gate 24 owns the separate service-output correction and is stacked on this
  branch; its passing evidence completes Gate 23's final blocked criterion.

## Delivery strategy
- Working branch: `hdae/virtual-sysinfo-systemd-compat`
- Intended base: `main`
- Pull request: intended; publication authorized by the human on 2026-11-12
- Planned commits: signed gate/control-plane commit; signed implementation and
  focused-regression commit; signed evidence reconciliation commit if needed
- Actual commits: `db45403e` (gate/control plane), `72ad3373`
  (implementation and focused regressions), `b9c2aa0b` (initial verification
  checkpoint), final evidence reconciliation pending

## Review policy and outcome
Technically passed checkpoint gate. The sysinfo correction passes focused,
aggregate, full systemd, and concurrent-instance evidence after the separately
approved Gate 24 correction. Publish it and stop for human review; technical
passage does not close Gate 23.
