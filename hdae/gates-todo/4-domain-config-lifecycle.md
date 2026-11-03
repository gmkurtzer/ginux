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

- [x] Selected strict, versioned TOML as the operator-facing configuration
  format with human approval on 2026-11-03.
- [x] Implemented bounded parsing, complete-field validation, canonical
  normalization, and effective configuration output.
- [x] Implemented the single-domain state machine, managed fixture startup,
  natural exit, requested stop, startup/runtime failure, resource cleanup,
  host SIGTERM handling, and child reaping.
- [x] Added unit, Rocky/QEMU integration, aggregate regression, documentation,
  and explicit human-demonstration coverage.

## Remaining work

- [ ] Human runs or reviews `make demo-gate-4` and accepts or rejects the gate.

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

- `make test-config` -> schema versioning, normalization, required-field,
  conflict, unknown-field, unsupported-feature, and fail-closed tests pass.
- `make test-domain` -> create/start/inspect/stop/failure/cleanup state-machine
  tests pass.
- Host-process harness test -> expected signals are handled, children are
  reaped, and shutdown returns the expected machine-checkable status.
- `make test` with network disabled -> exits zero.
- `make demo-gate-4` -> performs the documented valid and invalid lifecycle
  demonstrations, verifies cleanup, and exits zero.

## Evidence

- `cargo test -p ginus-domain` -> seven tests passed for versioned parsing,
  canonical normalization, comments/order equivalence, missing/duplicate/
  unknown fields, unsupported execution, host-authority rejection, lifecycle
  transitions, startup failure, runtime failure, and cleanup.
- `make test-config` -> Rocky 10.2 ran Ginus as an ordinary host process;
  equivalent TOML inputs produced byte-identical effective configuration, and
  unknown fields, the unsupported performance backend, and a host root path
  were rejected with `managed-started=false`.
- `make test-domain` -> the configured interpreted fixture ran with its
  configured argument/environment; created, running, stopped, and failed
  states were observed; malformed init cleanup succeeded; SIGTERM requested a
  stop; the host helper was reaped with zero zombies; and a repeat start passed.
- `make demo-gate-4` -> printed six named sections with concrete configuration,
  rejection, execution, failure-cleanup, signal, and reaping evidence and ended
  with `GATE4: PASS (configuration and single-domain lifecycle profile)`.
- `make test` -> formatting, all Clippy checks, 16 unit tests, Gate 1
  interception, Gate 3 runtime, Gate 4 configuration/lifecycle, and all eight
  Gate 2 offline-image checks passed.

## Decisions and limitations

- Strict TOML is the approved schema syntax. Unknown and duplicate fields are
  rejected rather than ignored.
- Runtime configuration mutation is deferred because it changes contract
  authority and audit requirements.
- Gate 4 supports only `security` plus `interpreter` and `network = "none"`.
  Recognized performance and mediated-network choices fail closed until their
  implementations receive later gate review.
- Storage and device entries are logical identifiers only in this gate. They
  grant no host path, descriptor, or backend authority; Gate 6 makes them
  operational.
- Gate 4 is temporarily stacked on accepted Gate 3 commit `8ef4340` because
  pull requests 1 through 3 remain open. Its intended eventual base is `main`.

## Delivery strategy

- Working branch: `hdae/domain-config-lifecycle`
- Intended base: `main`
- Pull request: intended; dedicated branch push and PR creation authorized on
  2026-11-03; never merge or close without explicit authorization
- Planned commits: versioned schema and normalization; lifecycle state machine;
  ordinary-host-process failure, cleanup, and configuration verification
- Actual commits: `34eb2e4` (strict TOML schema, normalization, lifecycle model,
  configuration documentation) and `ec858c4` (Ginus integration, Rocky/QEMU
  lifecycle harness, failure/signal/reaping tests, and demonstration)

## Review policy and outcome

Technically passed on 2026-11-03. Human checkpoint remains open because the
effective configuration becomes the authority record for later domains.
Present `make demo-gate-4`, fresh evidence, and the pull request before
acceptance.
