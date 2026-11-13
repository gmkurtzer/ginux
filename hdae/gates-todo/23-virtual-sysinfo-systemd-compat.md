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

## Remaining work
- [ ] Implement configured virtual total memory in both supervisor paths.
- [ ] Add focused regression coverage for virtual memory and private counters.
- [ ] Run all verification commands and record fresh evidence.
- [ ] Commit the verified implementation and stop for human checkpoint review.

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

## Decisions and limitations
- The configured virtual memory value is already the authoritative
  instance-visible total used by `/proc/meminfo`; reusing it keeps the two
  virtual interfaces consistent without disclosing host state.
- This value is descriptive virtual topology, not quota enforcement. Resource
  enforcement remains outside this corrective gate.
- Accepted Gate 22 remains locked; this file owns the compatibility correction.

## Delivery strategy
- Working branch: `hdae/virtual-sysinfo-systemd-compat`
- Intended base: `main`
- Pull request: intended, publication requires separate current authorization
- Planned commits: signed gate/control-plane commit; signed implementation and
  focused-regression commit; signed evidence reconciliation commit if needed
- Actual commits: pending

## Review policy and outcome
Human-approved checkpoint gate. The agent may implement and verify it, then
must stop for human review; technical passage does not close the gate.
