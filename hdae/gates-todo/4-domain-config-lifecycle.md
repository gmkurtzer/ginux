# Domain configuration and lifecycle

## Source

Approved Ginus domain model and per-instance configuration requirement.

## Goal

Define and implement a versioned, fail-closed domain configuration and a
deterministic single-domain create, start, inspect, stop, failure, and cleanup
lifecycle.

## Approved scope

- Define a versioned configuration schema and normalization rules.
- Cover domain identity, prepared root reference, init and arguments,
  environment, CPU and memory limits, execution profile/backend, contract
  profile, storage, devices, networking, audit behavior, and failure policy.
- Make the security/performance tradeoff and backend trust boundary explicit;
  unsupported execution profiles fail before managed code starts.
- Validate the complete configuration before managed execution begins.
- Retain and expose the normalized effective configuration and lifecycle state.
- Implement one domain per Ginus process for the initial milestone.
- Add bounded host-process signal, child-reaping, shutdown, and fatal-startup
  behavior needed by the lifecycle harness.

## Non-goals

- Multiple simultaneous domains, live migration, orchestration, or remote APIs.
- Runtime mutation or hotplug beyond deterministic stop and teardown.
- Implementing every configured backend; later gates make device and contract
  fields operational.

## Completed work

- [ ] None; gate not started.

## Remaining work

- [ ] Select and document the serialization format.
- [ ] Implement parsing, normalization, validation, and effective output.
- [ ] Implement lifecycle state and host-PID-1 control behavior.
- [ ] Add invalid-config, failure, cleanup, and restart tests.

## Acceptance criteria

- Equivalent input configurations normalize to a stable effective form with an
  explicit schema version.
- Missing, unknown security-critical, conflicting, or unsupported values fail
  before any managed code executes.
- A valid domain follows only documented lifecycle transitions and exposes its
  current state and effective configuration.
- Startup failure and requested stop release all resources acquired by the
  lifecycle implementation and leave no runnable managed task.
- Host-process signal and reaping tests demonstrate deterministic shutdown and
  no zombie leakage for the tested process model.
- Configuration never grants an implicit host device, descriptor, path, or
  contract capability.
- `make demo-gate-4` lets a human inspect normalized configuration, start and
  stop a domain, and observe fail-closed rejection of an invalid configuration.

## Verification plan

- `make test-config` -> schema versioning, normalization, defaults, conflict,
  unknown-field, unsupported-feature, and fail-closed tests pass.
- `make test-domain` -> create/start/inspect/stop/failure/cleanup state-machine
  tests pass.
- Host-process harness test -> expected signals are handled, children are reaped, and
  shutdown returns the expected machine-checkable status.
- `make test` with network disabled -> exits zero.
- `make demo-gate-4` -> performs the documented valid and invalid lifecycle
  demonstrations, verifies cleanup, and exits zero.

## Evidence

- Not run.

## Decisions and limitations

- Schema syntax is intentionally selected in this gate; the semantic fields
  listed above are already approved.
- Runtime configuration mutation is deferred because it changes contract
  authority and audit requirements.

## Delivery strategy

- Working branch: `hdae/domain-config-lifecycle`
- Intended base: `main`
- Pull request: intended; dedicated branch push and PR creation authorized on
  2026-11-03; never merge or close without explicit authorization
- Planned commits: versioned schema and normalization; lifecycle state machine;
  PID-1 failure, cleanup, and configuration verification
- Actual commits: pending

## Review policy and outcome

Human checkpoint. The effective configuration becomes the authority record for
later domains and must be accepted before backends consume it. Present
`make demo-gate-4`, fresh evidence, and the pull request before acceptance.
