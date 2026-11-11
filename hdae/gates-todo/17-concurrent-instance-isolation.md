# Concurrent-instance isolation

## Source

Human direction after Gate 16 was abandoned to validate parallel instances and
prove isolation across the boundaries Ginux currently claims.

## Goal

Run multiple Rocky Linux systemd instances concurrently and prove that each
instance's state, contract, virtual kernel views, resources, console, audit,
and lifecycle remain independent while accurately preserving the documented
shared-host networking semantics.

## Approved scope

- Create and run at least two independently named Rocky systemd instances in
  parallel from the same cached OCI image.
- Verify independent portable recipes, deployment state, runtime material,
  writable overlays, logs, audit streams, consoles, supervisor identities,
  virtual process identities, `/proc`, `/sys`, devices, and contract decisions.
- Exercise different effective contracts concurrently and prove that an allow,
  deny, lock, unlock, failure, stop, restart, or mutation in one instance does
  not alter the other.
- Verify that one instance cannot enumerate, signal, attach to, open, modify,
  or otherwise acquire authority over the other instance's managed processes
  or private state through supported interfaces.
- Test duplicate starts, stale identity records, concurrent lifecycle changes,
  abrupt supervisor loss, cleanup, repeated start/stop cycles, and child-process
  activity without cross-instance failure.
- Validate the existing shared-host networking model: ordinary host address and
  port conflicts retain first-successful-bind semantics, while translated
  ports, persisted deployment policy, audit identity, and cleanup remain bound
  to the owning instance.
- Fix isolation or concurrency defects discovered by this evidence when they
  fit the existing architecture. Escalate any fix requiring a new trust model,
  network namespace, or incompatible product decision into a separate gate.
- Add deterministic focused tests, a complete Rocky QEMU integration test,
  operator documentation, and `make demo-gate-17`.

## Non-goals

- Adding Linux network namespaces, a virtual switch, private address spaces,
  nested containers, orchestration, live migration, or distributed control.
- Claiming that shared-host networking provides namespace-equivalent isolation.
- Adding a kernel module, changing dispatch semantics, weakening contracts, or
  resolving Gate 11's performance budget.
- Production-scale density or arbitrary instance counts.

## Completed work

- [x] Human approved Gate 17 scope, existing shared-network semantics, separate
  branch and pull request, and execution through PR delivery.
- [x] Created dedicated branch `hdae/concurrent-instance-isolation` from
  synchronized `main`.

## Remaining work

- [ ] Establish a two-instance baseline and preserve any failures.
- [ ] Implement deterministic concurrency and cross-instance isolation tests.
- [ ] Fix in-scope defects found by the tests.
- [ ] Add the complete Rocky QEMU workflow and bounded human demonstration.
- [ ] Update architecture, security limitations, README, and operator guidance.
- [ ] Run focused and aggregate regression evidence.
- [ ] Push the branch and open the Gate 17 pull request.
- [ ] Obtain human review and acceptance.

## Acceptance criteria

- Two instances boot Rocky systemd concurrently, reach independently usable
  login/readiness state, and report different live supervisor/runtime process
  identities.
- Each instance sees only its own virtual PIDs, process tree, `/proc`, `/sys`,
  machine identity, console traffic, device objects, audit records, and
  lifecycle state; no supported lookup or operation exposes the peer.
- Writes made to one instance's overlay are absent from the other and survive
  only according to the owning instance's persistence semantics.
- Distinct contracts are enforced concurrently without policy, digest, audit,
  lock-state, or denial leakage between instances.
- Stopping, crashing, locking, unlocking, restarting, or corrupting stale
  lifecycle metadata for one instance does not stop, mutate, or misreport the
  other.
- Duplicate start of the same instance fails deterministically without
  affecting either live instance.
- Shared-network tests prove expected ordinary port conflicts and independent
  translated-port ownership, policy, audit identity, and cleanup; documentation
  clearly distinguishes these semantics from network-namespace isolation.
- Repeated concurrent lifecycle and child-process stress completes without
  leaked supervisors, tracees, FIFOs, translated ports, or stale running state.
- Existing contract, virtual-kernel, security, recovery, systemd, and aggregate
  suites remain green.
- `make demo-gate-17` performs a bounded operator-visible two-instance workflow,
  explains expected observations, and exits nonzero on any failed assertion.

## Verification plan

- Focused controller fixture -> parallel starts, unique identities, duplicate
  start rejection, independent state, and lifecycle cleanup pass.
- Focused runtime fixture -> distinct contracts, audit identity, virtual process
  views, overlay contents, and cross-instance authority rejection pass.
- Shared-network fixture -> ordinary bind conflict, translated-port ownership,
  policy identity, and cleanup pass without claiming namespace isolation.
- Failure/recovery fixture -> supervisor loss, stale metadata, restart, and peer
  survival pass.
- Repeated stress fixture -> multiple parallel start/child/stop cycles leave no
  managed process or deployment-state leak.
- `make test-multi-instance` -> complete deterministic and Rocky QEMU isolation
  workflow exits zero.
- `make demo-gate-17` -> bounded human-readable workflow exits zero.
- `make check test-contract test-security test-recovery test-systemd test` ->
  all mandatory regressions exit zero.

## Evidence

- Pending.

## Decisions and limitations

- Gate 17 validates the current shared-host network design. It does not claim
  private network namespaces; ordinary host bind conflicts are expected.
- A supported operation must not acquire peer-instance authority merely because
  both runtimes are ordinary host processes under the same Ginux installation.
- The gate proves a bounded two-instance baseline, not production density.

## Delivery strategy

- Working branch: `hdae/concurrent-instance-isolation`
- Intended base: `main`
- Pull request: intended
- Planned commits: gate and baseline; isolation/concurrency implementation and
  tests; Rocky integration and demonstration; documentation and evidence
- Actual commits: pending

## Review policy and outcome

Human checkpoint. The human authorized implementation and pull-request delivery
on 2026-11-11. The gate remains open until the PR evidence is reviewed and
accepted.
