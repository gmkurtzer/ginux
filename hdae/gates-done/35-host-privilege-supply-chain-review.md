# Host privilege, packaging, and supply-chain review

## Source

Defensive security-review initiative; depends on Gate 32 and may incorporate
resolved findings from Gates 33 and 34.

## Goal

Verify least-privilege host execution and trustworthy acquisition, installation,
upgrade, persistence, recovery, and removal behavior.

## Approved scope

- Review UID/GID transitions, `NoNewPrivs`, helpers, sudo boundaries, inherited
  descriptors, environment handling, RPM scripts, tmpfiles, and state ownership.
- Review OCI reference resolution, digest verification, cache integrity,
  extraction, instance import, persisted contracts, upgrades, reinstall, and
  recovery after interruption or corruption.
- Add deterministic package and lifecycle regressions and remediate defects.

## Non-goals

- No supply-chain attack simulation, malicious package creation, exploit
  development, or testing against public registries beyond ordinary verified
  acquisition.
- No package format beyond the accepted Rocky RPM scope.

## Completed work

- [x] Reviewed helper privileges, workload identity reduction, inherited
  descriptors, OCI verification, RPM ownership/modes, state lifecycle, and recovery.
- [x] Added allocator package execution to RPM `%check` and verified packaged
  loading across supported glibc instances.

## Remaining work

- [x] Trace host privilege and installation state transitions.
- [x] Add clean-install, upgrade, corruption, interruption, and recovery cases.
- [x] Resolve findings and document operator recovery procedures.
- [ ] Human review and acceptance of the combined pull request.

## Acceptance criteria

- Workload execution cannot inherit supervisor credentials, privileged host
  descriptors, or an ambient privilege path.
- Package-created files and directories have explicit ownership, mode, and
  lifecycle behavior; erase and reinstall preserve only documented state.
- OCI content is verified before use and corrupted or mismatched content fails
  closed without replacing a valid cache entry.
- Persisted contracts never migrate silently when catalog semantics change.
- No unresolved in-scope high- or critical-impact finding remains.

## Verification plan

- `make test-rpm-metadata test-rpm` -> clean install, verification, upgrade,
  erase/reinstall, and state-preservation cases pass.
- `make test-oci-image` -> digest, cache, corruption, and offline reuse tests pass.
- `make test-host-lifecycle test-recovery` -> interruption and cleanup behavior
  passes without stale privileged processes or descriptors.
- `make test-security test` -> defensive and aggregate regressions pass.

## Evidence

- `make test-rpm-metadata test-rpm` -> offline build, zero rpmlint errors,
  clean install, `rpm -V`, Rocky/Ubuntu allocator loading, Alpine runtime,
  offline cache reuse, reinstall, erase, retained state, and recovery passed.
- `make test-security test-recovery` -> reduced UID/GID, zero capabilities,
  `NoNewPrivs`, bounded descriptors, interruption, and cleanup passed.
- Aggregate `make test` -> OCI digest, path, cache, image, and lifecycle checks passed.

## Decisions and limitations

- Controlled corrupt fixtures are integrity tests, not malicious artifacts or
  exploit demonstrations.

## Delivery strategy

- Working branch: `hdae/security-allocator-initiative`
- Intended base: `main`
- Pull request: https://github.com/gmkurtzer/ginux/pull/292
- Planned commits: privilege inventory; package/acquisition tests; remediation;
  recovery documentation and final evidence
- Actual commits: `3046ff27`, `19377734`, `94c4f4f4`, `2163caa3`

## Review policy and outcome

Technically passed under the authorized batch. Kept open for the final human
checkpoint on the combined pull request.

## Gate 51 reconciliation

- Pull request #292 is merged into `main`. Fresh Gate 51 security, recovery,
  RPM metadata, clean install/reinstall/erase, allocator package, and aggregate
  runs revalidate least privilege, verified acquisition, persistence, and
  operator recovery.
- Technical status remains passed; lifecycle status remains open pending the
  consolidated human acceptance checkpoint.
