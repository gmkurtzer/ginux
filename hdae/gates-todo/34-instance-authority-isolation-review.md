# Instance authority and isolation review

## Source

Defensive security-review initiative; depends on Gates 32 and 33.

## Goal

Verify that process, descriptor, network, device, virtual-kernel, storage, and
lifecycle authority stays inside the owning instance and is revoked reliably.

## Approved scope

- Review virtual identities, task lifecycle, signals, descriptor sharing,
  networking, devices and ioctls, virtual `/proc` and `/sys`, OCI roots,
  writable state, hooks, console attachment, and recovery.
- Extend controlled multi-instance fixtures to test authority separation,
  revocation, reuse, restart, and failure cleanup.
- Remediate isolation defects and add deterministic regression coverage.

## Non-goals

- No exploit development, offensive escape exercise, or adversarial testing of
  external hosts or services.
- No new device-passthrough, namespace, or networking feature.
- No VM-equivalent isolation claim.

## Completed work

- [x] Reviewed virtual process, descriptor, device, root, console, contract,
  audit, network, and virtual-kernel authority and revocation.
- [x] Corrected transient control-state addressing and added bounded failure
  evidence to the concurrent-instance workflow.

## Remaining work

- [x] Review each authority class and its generation/lifetime model.
- [x] Add cross-instance and post-revocation negative tests.
- [x] Resolve in-scope defects and document residual host-kernel trust.
- [ ] Human review and acceptance of the combined pull request.

## Acceptance criteria

- An instance cannot observe or operate on another instance's virtual PIDs,
  descriptors, writable root, contract state, audit stream, console, devices,
  or private network state.
- Reused numeric identifiers do not revive stale authority.
- Stop, failure, forced teardown, reinstall, and restart revoke transient
  authority without damaging unrelated instances or host state.
- Virtual kernel views expose only current instance-owned state.
- No unresolved in-scope high- or critical-impact finding remains.

## Verification plan

- `make test-multi-instance` -> parallel and cross-instance isolation cases pass.
- `make test-process test-network test-devices test-kernel-views` -> focused
  authority suites pass.
- `make test-recovery` -> failure and teardown revoke resources deterministically.
- `make test-security test` -> defensive and aggregate regressions pass.

## Evidence

- `make test-multi-instance` -> parallel Rocky systemd, isolated overlays and
  virtual-kernel state, independent lock state, lifecycle, and shared networking passed.
- Aggregate `make test` -> process, device, network, kernel-view, and generation
  regressions passed.
- `make test-security test-recovery` -> teardown, supervisor-loss, and authority
  revocation paths passed.

## Decisions and limitations

- Tests use synthetic identities and controlled instance fixtures. They verify
  absence of unintended effects without constructing an exploit.
- Shared-host networking remains intentionally shared where documented; the
  review verifies contract enforcement and ownership rather than claiming a
  private network namespace.

## Delivery strategy

- Working branch: `hdae/security-allocator-initiative`
- Intended base: `main`
- Pull request: one combined Gates 32-42 pull request intended
- Planned commits: authority matrix; multi-instance tests; remediation; final
  evidence
- Actual commits: `94c4f4f4`, `2163caa3`

## Review policy and outcome

Technically passed under the authorized batch. Kept open for the final human
checkpoint on the combined pull request.
