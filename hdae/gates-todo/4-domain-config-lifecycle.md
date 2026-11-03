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
  environment, CPU and memory limits, contract profile, storage, devices,
  networking, audit behavior, and failure policy.
- Validate the complete configuration before managed execution begins.
- Retain and expose the normalized effective configuration and lifecycle state.
- Implement one domain per Ginus process for the initial milestone.
- Add bounded host-PID-1 signal, child-reaping, shutdown, and fatal-startup
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
- Host-PID-1 signal and reaping tests demonstrate deterministic shutdown and no
  zombie leakage for the tested process model.
- Configuration never grants an implicit host device, descriptor, path, or
  contract capability.

## Verification plan

- `make test-config` -> schema versioning, normalization, defaults, conflict,
  unknown-field, unsupported-feature, and fail-closed tests pass.
- `make test-domain` -> create/start/inspect/stop/failure/cleanup state-machine
  tests pass.
- PID-1 harness test -> expected signals are handled, children are reaped, and
  shutdown returns the expected machine-checkable status.
- `make test` with network disabled -> exits zero.

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
- Pull request: intended; do not push or open without explicit authorization
- Planned commits: versioned schema and normalization; lifecycle state machine;
  PID-1 failure, cleanup, and configuration verification
- Actual commits: pending

## Review policy and outcome

Human checkpoint. The effective configuration becomes the authority record for
later domains and must be accepted before backends consume it.

