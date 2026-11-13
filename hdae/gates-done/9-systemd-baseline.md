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
- Provide a general Rocky QEMU development host with Ginux installed in
  `PATH`, the pinned domain image cached, and no domain implicitly created or
  started.
- Provide the minimum operator-facing lifecycle commands: `ginux domain
  create`, `list`, `start`, `status`, and `stop`; `start --attach` must connect
  the terminal to the domain console while detached start returns control to
  the host.

## Non-goals

- Booting every Rocky service, arbitrary distributions, graphical targets,
  real hardware discovery, or production service coverage.
- Per-interface or source-address selection, ingress/egress port policy, and
  port translation; these are Gate 9.75 work.
- Weakening contract, device, or host-leakage invariants merely to satisfy a
  systemd probe.
- Declaring general Linux or systemd compatibility from one target.

## Completed work

- [x] Pin and verify the Rocky Linux 10 UBI init image and prepare its OCI
  filesystem through the existing immutable cache pipeline.
- [x] Add the explicit Gate 9 systemd launch profile, virtual PID 1 identity,
  writable disposable domain root, shared-network contract classification,
  and a reproducible Rocky-host probe.
- [x] Establish the first real systemd failure baseline under supervision.
- [x] Add bounded virtual mount, identity, console, event, timer, and cgroup
  compatibility through manager initialization.
- [x] Add bounded multi-task tracing, virtual PID and wait translation, child
  attach ordering, and supervised signal delivery through `multi-user.target`.
- [x] Reach the Ginux-owned login console, authenticate through `pam_unix`,
  execute commands in the managed shell, and verify virtual identity, process
  visibility, and the initial lockdown probes.
- [x] Mediate shared DNS, HTTPS, bind, listen, accept, and normal address
  conflicts without an unchecked managed syscall path.
- [x] Complete Rocky's enabled upstream-repository `dnf -y update` against a
  disposable domain root while preserving the immutable OCI base.
- [x] Translate authenticated systemd poweroff into bounded domain shutdown
  and verify supervisor cleanup.
- [x] Add restricted-network cold, warm, repeat, configured-failure, benchmark,
  live-update, and operator demonstration targets.
- [x] Add a deterministic failed-DNF transaction check that verifies the RPM
  package set remains readable and unchanged, managed shutdown completes, and
  the cached OCI base remains byte-identical.
- [x] Give the pinned systemd OCI image its own explicit fetch, verify, and
  prepare targets and make every Gate 9 target depend on that exact cache.
- [x] Provide a human-operated QEMU workflow that logs the operator into the
  Rocky host, leaves Ginux stopped until manual launch, attaches the terminal
  to the domain console, and returns to the host after domain shutdown.
- [x] Add an automated regression for the exact host login, manual launch,
  domain login, arbitrary command, controlled shutdown, return, and cleanup
  sequence.
- [x] Replace the Gate-specific host entry with a general `make run-host` that
  installs Ginux and cached image assets but creates no domain.
- [x] Implement and verify the minimum persistent `ginux domain` lifecycle,
  including attached and detached starts and stale-state detection.
- [x] Update the human demonstration to use the installed controller rather
  than a pre-staged low-level native command.

## Remaining work

- None. The human accepted the gate on 2026-11-06 after completing the
  interactive Gate 9 and Gate 9.5 workflow.

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
- A human-operated target boots the QEMU Rocky host, logs the operator into its
  shell with no domain present. The operator can create, inspect, start, attach
  to, stop, and list domains through the installed `ginux domain` interface.
  Attached console use remains exploratory until shutdown returns control to
  the host; exiting the host cleans up the VM.
- Detached start records enough lifecycle state for `list`, `status`, and
  `stop` to report and control the domain without relying on the invoking
  shell. Stale state is detected and reported as stopped rather than running.

## Verification plan

- `make test-systemd` -> cold boot, warm boot, repeated boot, readiness,
  login, virtual PID inventory, lockdown probes, configured failure, and
  controlled shutdown pass with external networking disabled.
- `make test-systemd-live` -> shared networking and the live upstream
  `dnf -y update` transaction pass.
- `make test-systemd-interactive` -> the real interactive runner reaches the
  Rocky host shell with no domain, creates one through `ginux domain`, checks
  list/status, starts it with `--attach`, authenticates, runs an arbitrary
  command, shuts systemd down, returns to the host, preserves audit evidence,
  verifies stopped state, and cleans up QEMU.
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

- `cargo test -p ginux-core` -> 6 tests passed, including the shared-network
  systemd contract while the earlier balanced contract continued to deny
  sockets.
- `cargo test -p ginux-domain` -> 11 tests passed, including strict parsing and
  normalization of `network.backend = "shared"`.
- `cargo clippy -p ginux-init --bin ginux --target
  x86_64-unknown-linux-gnu --release -- -D warnings` and the matching release
  build passed.
- `./scripts/with-rocky-host.sh ./scripts/probe-systemd.sh` -> the unmodified
  Rocky `/sbin/init` ran as virtual PID 1 under continuous syscall supervision,
  reached its attempt to mount `/run`, received `ENOSYS` for the unsupported
  tmpfs mount, and exited deterministically rather than bypassing Ginux.
- `ROCKY_SSH_PORT=22309 ./scripts/with-rocky-host.sh
  ./scripts/probe-systemd.sh` -> virtual mount, uname, console, epoll,
  signalfd, inotify, timerfd, and cgroup compatibility advanced systemd to
  manager startup; the next deterministic blocker is supervised `clone` for
  the `(sd-gens)` generator process.
- `cargo test -p ginux-core` -> 6 tests passed after the systemd-only timerfd
  surface was added; the balanced profile still denies that surface.
- Strict `ginux-init` release clippy and build passed after the compatibility
  tranche.
- `ROCKY_SSH_PORT=22314 ./scripts/with-rocky-host.sh
  ./scripts/probe-systemd.sh` -> systemd created and supervised its generator
  and service descendants, exposed virtual PIDs, and reached
  `multi-user.target`; it remained alive until the probe's bounded timeout.
- `ROCKY_SSH_PORT=22315 ./scripts/with-rocky-host.sh sh -c
  './scripts/test-process.sh && ./scripts/test-native-vfs.sh'` -> Gate 7 process
  regression passed; the Gate 5 native suite exposed an overbroad initial-stop
  classification that was corrected.
- `ROCKY_SSH_PORT=22317 ./scripts/with-rocky-host.sh
  ./scripts/test-native-vfs.sh` -> native shell, denial probes, supervisor-loss,
  unexpected-stop cleanup, and immutable-image regression all passed.
- `ROCKY_SSH_PORT=22371 ./scripts/with-rocky-host.sh
  ./scripts/probe-systemd.sh` -> systemd reached `multi-user.target`; root
  authenticated through the disposable test profile; the managed shell
  reported virtual PID 34, parent 26, UID/GID 0, and its domain-local proc
  record; host paths and protected descriptors were hidden; unassigned KVM and
  block devices were absent; mount and namespace creation were denied; exiting
  returned to the login prompt.
- `make test-systemd` -> three fresh cached-image boots passed with external
  networking disabled. Each reached the managed login, authenticated, ran
  virtual PID and lockdown checks, and shut down through the virtual poweroff
  path. A forced unreachable-repository DNF transaction failed as intended,
  the sorted `rpm -qa` package set remained readable, nonempty, and identical,
  controlled shutdown completed, and the cached OCI rootfs SHA-256 was
  unchanged. The configured missing-init case exited nonzero with one
  deterministic diagnosis, no shell fallback, and no retained managed task or
  resource.
- `make bench` -> on macOS arm64 using Rocky 10.2 x86-64 under QEMU TCG with
  two vCPUs and 2048 MiB host memory: 14,772 ms boot-to-login, 29,577 ms total
  instrumented runtime, 19,668 KiB peak traced-tree RSS, 14,716 KiB
  login-ready RSS, 4 peak sampled host processes, 37 peak Ginux-managed tasks,
  and 4 idle domain `/proc` entries. These are observations, not budgets.
- `make test-systemd-live` -> shared DNS/HTTPS and bind/listen/accept/conflict
  checks passed; enabled `baseos`, `appstream`, and `extras` repositories
  selected Rocky mirror infrastructure; metadata was current at the recorded
  run; 58 packages were upgraded; DNF printed `Complete!`; systemd then
  powered off and Ginux reported `GINUX_NATIVE_CHECK: PASS`. The run took
  872,979 ms and audited 1,035,090 managed syscalls under TCG.
- `make check` -> formatting and strict lint passed for every workspace
  component and x86-64 fixture.
- `make test` -> all 46 component tests and the interception, runtime,
  configuration, domain, image, device, process, and virtual-kernel-view QEMU
  regressions passed.
- `make test-native-vfs` -> native shell, mutation/network denial,
  supervisor-loss termination, unexpected-stop cleanup, and immutable Bash
  checks passed after the assertions were updated to require virtual PID audit
  identity.
- `make test-oci-image` -> all 7 digest, assembly, repeatability, identity,
  corruption, offline, and layer-semantics checks passed.
- `make demo-gate-9` -> all 8 documented operator checks passed: systemd as
  virtual PID 1, multi-user readiness, authenticated managed shell, virtual
  PID/lockdown visibility, shared networking, upstream DNF update, controlled
  systemd poweroff, and clean Ginux teardown.
- `make test-systemd-interactive` -> passed the exact operator path: Rocky QEMU
  host login, manual Ginux launch, domain `root` authentication, arbitrary
  shell command, virtual-PID-1 poweroff, return to the Rocky shell, audit-log
  validation, host exit, and QEMU cleanup. Native supervisor diagnostics and
  syscall audit were separated from the attached domain console and retained
  in `/var/tmp/ginux-systemd-audit.log` for inspection.
- Fresh `make check` passed formatting and strict lint for the full workspace.
- Fresh `make test-systemd` passed cold, warm, repeated, offline,
  deterministic-DNF-failure, configured-startup-failure, and controlled
  shutdown checks after the staging refactor and console/audit separation.
- Fresh `make test-systemd-interactive` passed the general-host workflow: the
  host began with no domains; `ginux domain create/list/status` worked;
  duplicate creation failed; attached start reached login and returned after
  controlled shutdown; detached start reported running and stopped; stale PID
  identity reported stopped; and exiting the host cleaned up QEMU.
- Fresh `make check`, `make test-systemd`, and aggregate `make test` passed
  after the controller and host workflow changes. The offline systemd suite
  again passed cold, warm, repeated, DNF-failure, startup-failure, and shutdown
  checks; the aggregate suite passed all 46 component tests and mandatory QEMU
  regressions.
- Human checkpoint on 2026-11-06 -> Gate 9 accepted together with corrective
  Gate 9.5 after the interactive operator workflow and final evidence review.

## Decisions and limitations

- The exact enabled unit set and readiness marker are recorded before this gate
  executes and become the named compatibility claim.
- The validated image is `rockylinux/rockylinux:10-ubi-init`, pinned to OCI
  index `sha256:81a2afe3645266b059214fc91bff27c5a51862abc61bf244b867e51d1093567c`
  and linux/amd64 manifest
  `sha256:7a18ce2c9c1e0d8735f3e72685f9666150af25036e7f7b5ee4eede3754496f8c`.
  Ginux launches `/sbin/init --system --unit=multi-user.target`; the named
  readiness evidence is systemd's managed `console-getty.service`,
  `getty.target`, and `multi-user.target` sequence ending in
  `Reached target multi-user.target - Multi-User System.`, followed by the
  Rocky login prompt. Boot readiness has a 50-second timeout; the upstream DNF
  transaction has an independent 1,800-second TCG watchdog.
- The expected shutdown result is systemd's `Powering off.`, a validated
  virtual-PID-1 reboot request translated to domain shutdown, zero exit,
  `GINUX_NATIVE_CHECK: PASS`, and removal of the disposable domain root by the
  harness.
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
  translation are approved as Gate 9.75 rather than being folded into this
  systemd gate.
- The first systemd baseline identifies bounded mount compatibility and
  multi-task supervision as the next required implementation tranche. The
  probe is partial evidence only; it does not satisfy readiness or login.
- Reaching `multi-user.target` does not claim that every started unit is
  healthy. The minimal UBI userspace still emits expected warnings and
  unavailable-service failures, including its absent D-Bus broker execution
  path; Gate 9 claims only the named readiness, login, network, update, and
  shutdown workflow.
- DNF modifies only the freshly extracted disposable domain root. Every test
  re-extracts the digest-verified OCI cache, and the OCI regression confirms
  that cached base content remains immutable and repeatable. The deterministic
  DNF failure check additionally verifies an unchanged readable RPM package
  set and an unchanged cached-root hash after failure and domain teardown.
- The first operator controller is intentionally bounded to
  `create/list/start/status/stop` and one installed Rocky systemd example.
  Broader typed domain options and CLI overrides remain Gate 9.75 work.
- Detached lifecycle state records both the supervisor PID and its Linux start
  time. Missing, exited, zombie, malformed, or PID-reused state reports stopped
  instead of controlling an unrelated process.
- New architecture needs discovered during convergence require human direction
  or a corrective gate rather than silent scope expansion.

## Delivery strategy

- Working branch: `hdae/systemd-baseline`
- Intended base: `main`
- Pull request: intended; dedicated branch push and PR creation authorized on
  2026-11-03; never merge or close without explicit authorization
- Planned commits: systemd boot profile and harness; bounded compatibility
  tranches; readiness, failure, shutdown, and measurement evidence
- Actual commits: `daca8006` (pinned systemd baseline and probe); `8af72885`
  (virtual bootstrap compatibility); `e595fc28` (multi-task supervision);
  `f9920311` (authenticated console and lockdown); `9265509d` (shared network
  and DNF mediation); `707581cd` (RPM filesystem operations); `69988717`
  (disposable write backing); `07630954` (controlled shutdown); `a93024b6`
  (repeat/failure/measurement harness); `c2d24a00` (machine-readable benchmark
  measurements); `d83995dd` (virtual-PID audit regression); `35e3763c`
  (bounded DNF watchdog); `598b9821` (failed-DNF consistency evidence and exact
  systemd-image targets); `1d90364b` (interactive demonstration); `64510197`
  (general host and persistent operator lifecycle). The completed Gate 9 and
  Gate 9.5 work reached `main` through signed merge commit `a8b46d21` rather
  than through the originally planned standalone Gate 9 pull request.

## Review policy and outcome

Passed and accepted. The human completed the operator review and accepted Gate
9 on 2026-11-06 together with corrective Gate 9.5. This record is closed.
