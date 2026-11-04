# Configurable contract and audit

## Source

Core Ginus product requirement: each domain exposes an explicit, versioned,
configurable Linux contract with logging and no direct host syscall path.

## Goal

Allow an operator to select a validated contract that governs syscall, ioctl,
device, filesystem, and virtual-kernel behavior for one domain and produces a
complete, useful, bounded audit trail of contract decisions.

## Approved scope

- Define a versioned contract schema referenced by normalized domain
  configuration.
- Express emulate, mediated, and deny decisions; there is no direct-continue
  decision.
- Cover syscall numbers and argument constraints, admitted ioctl families,
  device classes, filesystem operations or paths, and virtual kernel views used
  by the systemd baseline.
- Validate the complete contract before starting managed code and fail closed
  on unknown security-critical syntax or unsupported requirements.
- Make the effective contract immutable for the lifetime of the initial domain.
- Emit structured, rate-aware audit events for decisions, denials, malformed
  requests, lifecycle, and effective contract identity.
- Provide at least a systemd-compatible profile and a stricter negative-test
  profile.

## Non-goals

- Arbitrary policy code, runtime contract mutation, remote policy distribution,
  or automatic policy generation.
- Logging raw secrets, arbitrary managed memory, or unbounded attacker-controlled
  payloads.
- Treating audit-only observation as enforcement.

## Completed work

- [ ] None; gate not started.

## Remaining work

- [ ] Define schema, normalization, versioning, and validation.
- [ ] Connect every approved interface to contract decisions.
- [ ] Implement bounded structured audit output and rate behavior.
- [ ] Add profile, bypass, malformed-policy, and log-safety tests.

## Acceptance criteria

- Every managed syscall and admitted ioctl in the tested profiles has an
  explicit effective decision and contract version.
- No policy action directly continues a managed syscall on the host.
- Unknown operations and unknown security-critical policy fields fail closed.
- The strict profile denies its selected operations while the systemd profile
  retains the accepted systemd baseline.
- Contract denial prevents the corresponding host effect and emits the expected
  bounded audit record.
- Audit records identify domain, effective contract, operation, decision, and
  result while redacting or bounding configured sensitive data.
- Audit flooding cannot grow memory or disk use without the documented bound
  and exposes dropped-event accounting.
- Effective configuration and contract evidence can reproduce which policy was
  active for a test result.
- `make demo-gate-10` lets a human compare permissive and strict contracts,
  observe a mediated operation and denial, and inspect bounded audit records.

## Verification plan

- `make test-contract` -> schema, normalization, unknown-field, profile,
  emulate, mediate, deny, bypass-negative, immutable-policy, and audit tests
  pass.
- Systemd profile: `make test-systemd` -> remains passing.
- Strict profile negative fixtures -> operations fail with expected results,
  host-state snapshots remain unchanged, and matching audit events exist.
- Audit flood fixture -> resource bounds and dropped-event accounting match the
  documented policy.
- `make test` -> all mandatory regression suites exit zero.
- `make demo-gate-10` -> performs the documented profile comparison, mediation,
  denial, audit, and cleanup demonstration and exits zero.

## Evidence

- Not run.

## Decisions and limitations

- Schema syntax is chosen in Gate 4 for domain configuration and extended here
  only as needed for contract semantics.
- Runtime policy changes are deferred because they require authority,
  synchronization, and audit rules not approved here.

## Delivery strategy

- Working branch: `hdae/configurable-contract`
- Intended base: `main`
- Pull request: intended; dedicated branch push and PR creation authorized on
  2026-11-03; never merge or close without explicit authorization
- Planned commits: contract schema and validation; dispatcher and backend
  integration; structured audit and bounds; profiles and security regressions
- Actual commits: pending

## Review policy and outcome

Human checkpoint because this gate defines the operator-visible security
contract and the evidence used for later kernel-exposure claims. Present
`make demo-gate-10`, fresh evidence, and the pull request before acceptance.
