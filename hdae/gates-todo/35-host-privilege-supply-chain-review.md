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

- [ ] None.

## Remaining work

- [ ] Trace host privilege and installation state transitions.
- [ ] Add clean-install, upgrade, corruption, interruption, and recovery cases.
- [ ] Resolve findings and document operator recovery procedures.

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

- Not run.

## Decisions and limitations

- Controlled corrupt fixtures are integrity tests, not malicious artifacts or
  exploit demonstrations.

## Delivery strategy

- Working branch: `hdae/host-privilege-supply-chain-review`
- Intended base: `main`
- Pull request: intended
- Planned commits: privilege inventory; package/acquisition tests; remediation;
  recovery documentation and final evidence
- Actual commits: pending

## Review policy and outcome

Security checkpoint requiring explicit human acceptance. Persistence of this
gate does not authorize implementation.
