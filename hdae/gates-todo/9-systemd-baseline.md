# Systemd baseline

## Source

Approved Rocky Linux 10 systemd compatibility baseline. Human direction on
2026-11-03 added a usable multi-user login, virtual PID inspection, lockdown
verification, and a DNF update workflow as milestone evidence.

## Goal

Boot the prepared Rocky Linux 10 systemd binary as virtual PID 1 in one Ginux
domain, reach a usable multi-user login prompt, inspect its virtual process
tree and security boundary, complete a bounded DNF update workflow, and perform
a controlled shutdown.

## Approved scope

- Define a minimal, explicit Rocky systemd boot profile and readiness marker.
- Integrate existing execution, VFS, device, process, and virtual-kernel
  components without bypass paths.
- Add serial or structured boot logs sufficient to diagnose deterministic
  failures.
- Implement missing bounded compatibility behavior discovered within the
  approved profile; material new subsystems require a corrective gate.
- Add cold, warm, repeated, failure, and shutdown tests.
- Record boot time, peak and idle memory, CPU use, and managed process count.
- Provide a working login path at the multi-user target and verify that the
  authenticated shell remains a managed domain process.
- Inspect virtual PIDs and selected lockdown probes from the logged-in domain.
- Run a deterministic DNF metadata query and package update transaction through
  an explicitly mediated repository path without exposing host authority.
- Use Rocky Linux's enabled default upstream repositories and perform an actual
  noninteractive `dnf -y update` through that mediated path.
- Add a `shared` network backend that is the default when no network attachment
  is configured. It uses the host's default routing, source-address selection,
  and socket namespace for both outbound and inbound operations while retaining
  Ginux interception, validation, mediation, and audit.

## Non-goals

- Booting every Rocky service, arbitrary distributions, graphical targets,
  real hardware discovery, or production service coverage.
- Per-interface or source-address selection, ingress/egress port policy, and
  port translation; these are Gate 9.5 work.
- Weakening contract, device, or host-leakage invariants merely to satisfy a
  systemd probe.
- Declaring general Linux or systemd compatibility from one target.

## Completed work

- [ ] None; gate not started.

## Remaining work

- [ ] Define the exact image, units, readiness marker, timeout, and shutdown
  result.
- [ ] Integrate systemd and close bounded compatibility gaps.
- [ ] Add repeatable boot, failure diagnostics, and shutdown verification.
- [ ] Capture baseline boot resource and timing measurements.
- [ ] Demonstrate login, virtual PID inspection, lockdown probes, and the
  bounded DNF transaction.

## Acceptance criteria

- Rocky systemd runs as virtual PID 1 and reaches the named multi-user readiness
  marker before the documented timeout.
- The readiness marker is produced by an expected managed unit, not by the host
  harness or a bypass path.
- Controlled shutdown terminates managed tasks, flushes configured writable
  state, releases devices and backends, and gives the harness the expected
  result.
- A configured startup failure produces a deterministic failed state and useful
  logs rather than silently falling back to host init or a shell.
- The complete boot succeeds repeatedly from the cached image with network
  acquisition disabled.
- Boot evidence records duration, peak and idle resource measurements, and
  environment metadata without yet asserting final performance budgets.
- Every earlier mandatory suite remains passing.
- A human can authenticate at the Ginux-owned console, obtain a managed shell,
  inspect a domain-only PID tree with systemd as PID 1, and cannot observe or
  signal host-only processes.
- The logged-in shell passes the named lockdown probes for host paths,
  descriptors, credentials, capabilities, devices, and denied kernel
  interfaces.
- DNF can query metadata and complete the selected package update transaction
  from Rocky Linux's enabled default upstream repositories through the
  mediated repository path; `dnf -y update` exits successfully, and failure
  and rollback leave the base image immutable and domain state consistent.
- With no explicit network attachment, managed sockets use the host's default
  network and normal host bind semantics. Outbound DNS and TCP work, inbound
  bind/listen/accept work, and an address/port already owned by another host or
  domain process fails with the Linux-compatible address-in-use result.
- Shared networking does not bypass central dispatch, disclose supervisor
  descriptors, or permit an unchecked host syscall.
- `make demo-gate-9` lets a human watch Rocky systemd run as virtual PID 1,
  log in at multi-user, inspect managed services and PIDs, run the lockdown and
  DNF checks, and shut down cleanly.

## Verification plan

- `make test-systemd` -> cold boot, warm boot, repeated boot, readiness,
  login, virtual PID inventory, lockdown probes, configured failure, live
  upstream DNF transaction, and controlled shutdown cases pass.
- Offline boot subtest with external network disabled and the domain image
  already prepared -> reaches readiness and exits zero without an acquisition
  request; the live DNF subtest is explicitly excluded from this offline case.
- Domain teardown inventory -> no managed task, device assignment, overlay,
  or backend resource remains after the test.
- `make bench` systemd profile -> emits boot and idle measurements with
  environment metadata.
- `make test` -> all mandatory regression suites exit zero.
- `make demo-gate-9` -> performs the documented visible systemd boot,
  readiness, inspection, and shutdown sequence and exits zero.

## Evidence

- Not run.

## Decisions and limitations

- The exact enabled unit set and readiness marker are recorded before this gate
  executes and become the named compatibility claim.
- Human direction on 2026-11-04 selected Rocky Linux's enabled default upstream
  repositories and an actual `dnf -y update`, rather than a cached local
  repository fixture.
- Upstream repository contents and mirror selection change over time. The test
  therefore makes the procedure and pass/fail result reproducible while
  recording selected repository URLs, metadata timestamps, and package changes;
  it does not claim byte-for-byte deterministic upstream content.
- Human direction on 2026-11-04 selected host-shared networking as the Gate 9
  default. All inbound and outbound operations share the normal host socket
  namespace and routing; the first successful bind wins. Ginux mediation and
  audit remain mandatory.
- Interface/source-address selection, port allow/deny policy, and port
  translation are approved as Gate 9.5 rather than being folded into this
  systemd gate.
- New architecture needs discovered during convergence require human direction
  or a corrective gate rather than silent scope expansion.

## Delivery strategy

- Working branch: `hdae/systemd-baseline`
- Intended base: `main`
- Pull request: intended; dedicated branch push and PR creation authorized on
  2026-11-03; never merge or close without explicit authorization
- Planned commits: systemd boot profile and harness; bounded compatibility
  tranches; readiness, failure, shutdown, and measurement evidence
- Actual commits: pending

## Review policy and outcome

Human checkpoint. Systemd readiness is the principal compatibility milestone
and its exact meaning and evidence require explicit acceptance. Present
`make demo-gate-9`, fresh evidence, and the pull request before acceptance.
