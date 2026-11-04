# Boundary hardening

## Source

Initial Ginux-us milestone completion and requirement for defensible limitations,
recovery, and regression evidence.

## Goal

Exercise Ginux-us against malformed ABI inputs and escape attempts, reduce Ginux-us
host authority, verify recovery and performance regressions, and publish an
honest security and compatibility boundary for the initial milestone.

## Approved scope

- Add structured malformed syscall, ioctl, ELF, configuration, filesystem,
  process, virtual-file, device, and audit-input tests.
- Add bounded fuzz targets for security-critical parsers and dispatchers with
  reproducible seed corpora and crash-artifact handling.
- Add direct-syscall, host-descriptor, host-path, cross-object, contract-bypass,
  device-range, host-view, resource-exhaustion, and teardown escape attempts.
- Reduce Ginux-us host capabilities, descriptors, filesystem visibility, and
  syscall surface after bootstrap as far as the validated architecture permits.
- Verify startup failure, domain crash, forced stop, audit saturation, and host
  shutdown or recovery behavior.
- Rerun functional, systemd, contract, security, and performance comparisons.
- Document supported behavior, threat-model exclusions, known gaps, operational
  requirements, and measured performance.

## Non-goals

- Claiming formal verification, complete Linux compatibility, production
  readiness, independent audit, or protection from excluded threats.
- Adding new product features, distributions, hardware backends, architectures,
  or multi-domain orchestration.
- Silently fixing material newly discovered defects outside this gate; such
  defects require corrective gates.

## Completed work

- [ ] None; gate not started.

## Remaining work

- [ ] Build malformed-input, fuzz, and escape suites.
- [ ] Minimize and verify Ginux-us host authority.
- [ ] Exercise recovery and cleanup behavior.
- [ ] Run full functional, security, and benchmark regressions.
- [ ] Publish initial security, compatibility, operations, and performance
  limitations.

## Acceptance criteria

- Every identified security-critical parser and dispatch boundary has either a
  deterministic malformed-input suite or a documented reason and alternative
  evidence.
- Bounded fuzz runs complete for the approved duration and corpus without an
  untriaged crash, hang, memory-safety failure, or invariant violation.
- Escape fixtures cannot complete unmediated host syscalls, use managed values as
  host descriptors, observe unassigned host state, exceed block ranges, weaken
  the contract, or retain resources after teardown.
- Ginux-us drops or confines unneeded host capabilities, descriptors, paths, and
  syscalls after bootstrap; the effective residual authority is documented and
  tested.
- Failure and recovery fixtures reach their expected states without silent
  fallback, resource leakage, or uncontrolled host shutdown.
- Systemd, contract, defensive boundary checks, and approved performance
  budgets remain passing after hardening.
- Documentation states supported claims and unresolved risks precisely enough
  that it does not imply production readiness or universal exploit prevention.
- `make demo-gate-13` lets a human run a bounded escape and recovery sequence,
  inspect residual host authority, and verify the final limitations report.

## Verification plan

- `make test-security` -> malformed-input, authority, bypass, leakage,
  exhaustion, and teardown suites pass.
- Documented bounded fuzz command or Make target -> completes the approved run
  with zero untriaged failures and preserves reproducible artifacts for any
  discovered defect.
- `make test-recovery` -> startup failure, domain crash, forced stop, audit
  saturation, and shutdown/recovery cases pass.
- `make test test-systemd test-contract test-security` -> all mandatory suites
  exit zero.
- `make bench-compare` -> all approved mandatory budgets pass after hardening.
- Documentation consistency check -> claims map to current executable evidence
  and every known limitation remains visible.
- `make demo-gate-13` -> performs the documented boundary, recovery, residual
  authority, and report inspection demonstration and exits zero.

## Evidence

- Not run.

## Decisions and limitations

- Fuzz duration, target list, and resource ceiling are recorded before this
  gate executes so completion cannot be declared from an arbitrary short run.
- An independent security audit and production-readiness program remain future
  work.

## Delivery strategy

- Working branch: `hdae/boundary-hardening`
- Intended base: `main`
- Pull request: intended; dedicated branch push and PR creation authorized on
  2026-11-03; never merge or close without explicit authorization
- Planned commits: malformed-input and fuzz harnesses; escape and authority
  reduction; recovery and cleanup; full regression evidence and limitations
- Actual commits: pending

## Review policy and outcome

Human checkpoint. Present `make demo-gate-13`, fresh evidence, and the pull
request. This gate closes the initial milestone only after explicit acceptance
of the evidence, residual risks, and scoped claims.
