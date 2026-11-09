# Boundary hardening

## Source

Initial Ginux milestone completion and requirement for defensible limitations,
recovery, and regression evidence.

## Goal

Exercise Ginux against malformed ABI inputs and escape attempts, reduce Ginux
host authority, verify recovery and performance regressions, and publish an
honest security and compatibility boundary for the initial milestone.

## Approved scope

- Add structured malformed syscall, ioctl, ELF, configuration, filesystem,
  process, virtual-file, device, and audit-input tests.
- Add bounded fuzz targets for security-critical parsers and dispatchers with
  reproducible seed corpora and crash-artifact handling.
- Add direct-syscall, host-descriptor, host-path, cross-object, contract-bypass,
  device-range, host-view, resource-exhaustion, and teardown escape attempts.
- Reduce Ginux host capabilities, descriptors, filesystem visibility, and
  syscall surface after bootstrap as far as the validated architecture permits.
- Verify startup failure, instance crash, forced stop, audit saturation, and host
  shutdown or recovery behavior.
- Rerun functional, systemd, contract, security, and performance comparisons.
- Document supported behavior, threat-model exclusions, known gaps, operational
  requirements, and measured performance.

## Non-goals

- Claiming formal verification, complete Linux compatibility, production
  readiness, independent audit, or protection from excluded threats.
- Adding new product features, distributions, hardware backends, architectures,
  or multi-instance orchestration.
- Silently fixing material newly discovered defects outside this gate; such
  defects require corrective gates.

## Completed work

- [x] Human authorized autonomous execution and separate pull-request delivery.
- [x] Inventoried parser, dispatcher, escape, recovery, authority, and existing
  regression surfaces.
- [x] Added deterministic malformed-input and bounded fuzz coverage for the
  security-critical parsers and dispatch inputs.
- [x] Added escape, authority, resource-exhaustion, teardown, and host-view
  verification through the existing QEMU boundary suites.
- [x] Reduced the post-bootstrap native supervisor to host UID/GID 65534, zero
  effective capabilities, `NoNewPrivs`, and a bounded descriptor set.
- [x] Exercised startup failure, supervisor loss, unexpected stops, contract
  denial, repeated systemd boot, immutable-image failure, and controlled
  shutdown recovery.
- [x] Published the initial supported-claims, residual-authority,
  compatibility, operational, security, and performance limitations report.
- [x] Ran the complete functional, contract, security, recovery, systemd, and
  aggregate regression suites after hardening.

## Remaining work

- [ ] Resolve or explicitly change the unchanged 4% Gate 11 performance
  criterion: all five authoritative selective thread-yield rows remain below
  the required 0.960x floor.
- [ ] Human review and acceptance of the partial result, residual risks, and
  scoped claims.

## Acceptance criteria

- Every identified security-critical parser and dispatch boundary has either a
  deterministic malformed-input suite or a documented reason and alternative
  evidence.
- Bounded fuzz runs complete for the approved duration and corpus without an
  untriaged crash, hang, memory-safety failure, or invariant violation.
- Escape fixtures cannot complete unmediated host syscalls, use managed values as
  host descriptors, observe unassigned host state, exceed block ranges, weaken
  the contract, or retain resources after teardown.
- Ginux drops or confines unneeded host capabilities, descriptors, paths, and
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
- `make test-recovery` -> startup failure, instance crash, forced stop, audit
  saturation, and shutdown/recovery cases pass.
- `make test test-systemd test-contract test-security` -> all mandatory suites
  exit zero.
- `make bench-compare` -> all approved mandatory budgets pass after hardening.
- Documentation consistency check -> claims map to current executable evidence
  and every known limitation remains visible.
- `make demo-gate-13` -> performs the documented boundary, recovery, residual
  authority, and report inspection demonstration and exits zero.

## Evidence

- `make check-security-doc` -> passed; claims, residual authority, known gaps,
  performance limitations, and independent-review status are visible.
- `make check` -> passed formatting, strict lint, builds, generated contract
  documentation, and static terminology checks.
- `GINUX_FUZZ_SECONDS=5 make fuzz-security` -> passed all eight deterministic
  targets for five seconds each with no crash, timeout, invariant failure, or
  retained failure artifact; each target executed more than 140,000 cases in
  the recorded run.
- `make test-security` -> passed malformed-input, fuzz, native authority,
  contract-denial, block-range, virtual-view, resource-exhaustion, and teardown
  checks.
- `make test-recovery` -> passed supervisor-loss, unexpected-stop, contract,
  systemd cold/warm/repeat, immutable-image failure, configured-startup
  failure, and controlled-shutdown checks.
- `make test-contract` -> passed 36 contract and instance tests plus generated
  ABI coverage consistency.
- `make test` -> passed the aggregate unit, image, interception, runtime,
  configuration, instance, device, process, contract, and virtual-kernel-view
  regression matrix.
- `make demo-gate-13` -> passed deterministic fuzzing, denied host effects,
  residual-authority inspection, supervisor-loss and unexpected-stop cleanup,
  contract denial, and limitations-report inspection.
- Pull request 255 CI -> `static-and-unit` and `launch-integration` both passed
  on the delivered `hdae/boundary-hardening` branch.
- `make bench-compare` against the default stale exploratory artifact failed
  and is not authoritative; that artifact predates the retained Gate 11
  optimizations.
- `scripts/bench-compare.sh build/gate11-balanced-final-v2/results.tsv
  build/gate11-balanced-final-v2/metadata.txt /tmp/ginux-no-accepted-results
  /tmp/ginux-no-accepted-metadata` -> failed exactly the five authoritative
  thread-yield rows at 0.938x, 0.930x, 0.953x, 0.921x, and 0.917x against the
  required 0.960x floor; CPU, memory, and mutex rows passed.

## Decisions and limitations

- The deterministic bounded fuzz suite covers instance TOML parsing and
  normalization, contract selectors and resolution, ELF loading, VFS index
  parsing, virtual-kernel path lookup, device names/ranges, syscall/ioctl
  dispatch inputs, and audit accounting.
- Each target runs from a checked-in seed corpus for five seconds with a 4 KiB
  maximum generated input. The complete run is bounded to 60 seconds, one
  process, and a 256 MiB address-space limit. The fixed PRNG seed and target
  name are recorded; the current input is written before execution so a crash
  or timeout leaves a reproducible artifact under `build/security-fuzz/`.
- An independent security audit and production-readiness program remain future
  work.
- Gate 13 is technically partial. Its security, recovery, functional,
  documentation, and demonstration evidence passes, but the approved
  performance criterion does not. The existing Gate 11 evidence attributes the
  residual selective-mode thread deficit to the host seccomp/task-work floor
  under x86-64 QEMU TCG after exhausting the approved in-scope optimizations.
- The default `build/gate11/` result is an early exploratory matrix and must
  not be treated as current retained-code performance. The authoritative final
  matrix is `build/gate11-balanced-final-v2/`.

## Delivery strategy

- Working branch: `hdae/boundary-hardening`
- Intended base: `hdae/contract-maintenance-lock`
- Pull request: https://github.com/gmkurtzer/ginux/pull/255, stacked on Gate
  12.5; dedicated branch push and PR creation authorized by the human's
  2026-11-08 batch direction; never merge or close without explicit
  authorization
- Planned commits: malformed-input and fuzz harnesses; escape and authority
  reduction; recovery and cleanup; full regression evidence and limitations
- Actual commits: `0b954c76` (gate start), `83b170c6` (bounded security
  fuzzing), `e6e56e96` (native supervisor authority reduction), `d6cda551`
  (boundary and recovery verification), `83f0fc99` (evidence checkpoint), plus
  the pull-request record commit

## Review policy and outcome

Human checkpoint. The technical result is partial because the mandatory
performance criterion remains failed. Present `make demo-gate-13`, fresh
evidence, the exact benchmark limitation, and the pull request. This gate
cannot close the initial milestone unless the performance criterion passes or
the human explicitly changes it and then accepts the evidence, residual risks,
and scoped claims. Pull request 255 is delivered with green CI and awaits that
decision; it has not been merged or closed.
