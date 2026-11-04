# Ginux HDAE Status

## Current checkpoint

- Gate 1 is accepted and closed under the corrected ordinary-host-process
  architecture. Pull request https://github.com/gmkurtzer/ginux-us/pull/1 was
  merged by the human on 2026-11-04.
- Gate 2 is accepted and closed. Pull request
  https://github.com/gmkurtzer/ginux-us/pull/2 was merged by the human on
  2026-11-04.
- Gate 3 is accepted and closed. Pull request
  https://github.com/gmkurtzer/ginux-us/pull/3 was merged by the human on
  2026-11-04.
- Gate 4 is accepted and closed. Pull request
  https://github.com/gmkurtzer/ginux-us/pull/4 was merged by the human on
  2026-11-04.
- Gate 5 is accepted and closed. Pull request
  https://github.com/gmkurtzer/ginux-us/pull/5 was merged by the human on
  2026-11-04.
- Gate 6 is accepted and closed. Pull request
  https://github.com/gmkurtzer/ginux-us/pull/6 was merged by the human on
  2026-11-04.
- Gates 7, 7.5, and 7.75 are accepted and closed. Their pull requests were
  merged by the human on 2026-11-04.
- Gate 8 is accepted and closed. Pull request
  https://github.com/gmkurtzer/ginux-us/pull/10 was merged as `2306c2e`, and
  the human confirmed the gate complete on 2026-11-04.
- Authoritative name: Ginux.
- Runtime isolation unit: domain.
- Policy unit: versioned contract.
- Initial gate map: twelve checkpoint gates at positions 1 through 11 and 13;
  Gates 1 through 8 are accepted. Gate 8.5 is the active repository transition
  checkpoint before Gate 9.
- The ordered gate batch is authorized, but every open gate remains a human
  checkpoint requiring a working demonstration and explicit acceptance.

## Governing outcome and invariants

A normal Rocky Linux 10 host must boot with systemd as real PID 1. Ginux runs
as an ordinary host process and launches an unmodified Rocky Linux 10 systemd
userspace as virtual domain PID 1 without allowing managed code to invoke host
syscalls directly. All managed interactions are emulated, mediated, or denied
according to the domain contract. Domain-visible kernel state and devices are
Ginux-owned.

See `hdae/PROJECT.md` and `hdae/ARCHITECTURE.md` for the authoritative intent,
scope, threat model, and invariants.

## Evidence

- Repository inspection before establishment: clean `main` branch with no
  pre-existing root `hdae/` project ledger.
- Human direction confirmed the Ginux name, domain terminology, Rust/x86-64
  direction, Rocky Linux 10 systemd baseline, cached cloud QCOW2 source,
  virtual kernel trees, device assignment requirement, benchmark requirement,
  and dedicated branch/PR delivery per implementation gate.
- Human direction on 2026-11-03 selected macOS arm64 plus QEMU TCG as the
  development environment, removed KVM from initial acceptance, authorized the
  ordered gate batch, required a runnable human demonstration for every
  checkpoint, and authorized pushing dedicated branches and opening PRs.
- Human direction on 2026-11-03 removed the public-exploit demonstration gate.
  The project retains deterministic defensive contract, malformed-input,
  escape-attempt, fuzz, recovery, and authority-reduction verification.
- Human direction on 2026-11-03 corrected the host boundary: Rocky Linux boots
  normally with host systemd as PID 1; Ginux is an ordinary process or service
  that creates and supervises domains beneath it. Domain systemd is virtual
  PID 1. The earlier Gate 1 PID-1 result is retained as superseded evidence,
  not accepted product behavior.
- Coverage review: functionality, failure behavior, deterministic tests, user
  configuration, documentation, compatibility, recovery, performance, and
  security are assigned to explicit gates; none is silently omitted.
- Revised Gate 1 clean verification on 2026-11-03 passed `make doctor`,
  `make host-image-verify`, `make build`, `make check`, `make test-unit`,
  `make test-intercept`, `make test`, `make host-boot-test`,
  `make bench-baseline`, and `make demo-gate-1` under QEMU TCG.
- Rocky Linux 10.2 booted normally with host systemd as real PID 1. Ginux ran
  as the unprivileged ordinary `ginux` process, mediated the fixed payload,
  exited zero, and left systemd plus `multi-user.target` healthy.
- The revised demo showed virtual `getpid` 4242, checked write mediation without
  host execution, `uname` denied with `EPERM`, no direct-write marker, matched
  Rocky-host benchmark samples, clean QEMU shutdown, and no remaining PID file
  or VM process.
- Human accepted Gate 1 on 2026-11-03 and directed advancement to Gate 2.
- Final Gate 1 verification after removing Ginux numeric host-PID checks passed
  `make check`, `make test-unit`, and `make demo-gate-1`; normal host PID 1
  validation remains only in the Rocky boot harness.
- Gate 2 `make image-prepare`, `make test-image`, `make demo-gate-2`, clean-cache
  preservation checks, and aggregate `make test` passed. The pinned helper
  mounted the source XFS root read-only with networking disabled; repeat
  extraction produced manifest SHA-256
  `49e69bc6209b9d8ed084e221924b633c3126ccaa9a2f920bb8464249a5f66721`;
  corrupt and missing sources were rejected.
- After human feedback that the original demo was opaque, the revised
  `make demo-gate-2` reported eight named checks and their concrete values,
  asserted a read-only source mount plus no helper Ethernet/default route, and
  explicitly stated that Ginux execution, VFS, and systemd boot are not Gate 2
  evidence. The revised demonstration passed on 2026-11-03.
- Human accepted Gate 2 on 2026-11-03 and directed advancement to Gate 3.
- Gate 3 implemented the security-focused x86-64 interpreter, Ginux-owned
  memory/permissions, bounded static ELF loader, Linux initial stack, central
  syscall events, output mediation, audit records, and fail-closed errors.
  Six runtime unit tests, `make test-runtime`, `make demo-gate-3`, and aggregate
  `make test` passed. The interpreted fixture emitted its argument/environment,
  syscall 999 returned `-ENOSYS`, virtual `getpid` returned 4242, exit returned
  zero, and a truncated ELF was rejected before execution.
- Human accepted Gate 3 on 2026-11-03.
- Human authorized Gate 4 on 2026-11-03 and approved strict, versioned TOML as
  its operator-facing configuration format.
- Gate 4 implemented strict versioned TOML parsing and normalization, complete
  fail-closed validation, immutable effective configuration, a single-domain
  lifecycle, managed fixture execution, failure cleanup, signal-driven stop,
  and helper reaping. Seven domain unit tests, `make test-config`,
  `make test-domain`, `make demo-gate-4`, and aggregate `make test` passed.
- Human accepted Gate 4 on 2026-11-03 and directed advancement to Gate 5.
- Human corrected and approved the Gate 5 backing strategy on 2026-11-03: the
  domain virtually boots a digest-pinned Rocky OCI image through Ginux. Ginux
  explicitly pulls and caches the image, assembles its layers into an immutable
  indexed root, and supplies a bounded in-memory write overlay. An external OCI
  runtime must not execute managed code, and the host QCOW2 remains independent.
- Gate 5 now has a verified pinned OCI acquisition/cache pipeline and a Ginux
  immutable root index. `make test-oci-image` passed six acquisition,
  integrity, identity, corruption, repeatability, and offline checks.
- `make test-vfs` passed in the QEMU TCG Rocky host. Ginux resolved Rocky
  identity and Bash from the OCI root, created and discarded its own overlay
  file, kept the host filesystem unchanged, and rejected a truncated index.
- Gate 5 native execution milestone commit `8e5847a` added the `ptrace-native`
  backend. `make check`, `make test-unit`, and `make test-native-vfs` passed.
  The QEMU Rocky host loaded the unmodified OCI Bash and image loader, Ginux
  audited 238 syscall entries, emulated `getpid` as 4242, denied unsupported
  calls, printed `GINUX_NATIVE_SHELL: PASS`, stopped at `exit_group`, and
  preserved the Bash digest.
- Gate 5 multi-layer assembly commit `19ade45` removed the one-layer-only
  preparation restriction. `make test-oci-layers` and the expanded seven-check
  `make test-oci-image` passed replacement, whiteout, opaque-directory,
  repeatability, integrity, identity, and offline checks. A fresh
  `make test-native-vfs` passed against the resulting assembled root.
- The expanded native QEMU test also passed fail-closed supervisor loss: after
  a held Rocky Bash reported readiness, killing Ginux caused
  `PTRACE_O_EXITKILL` to remove the tracee. Normal execution, emulated `getpid`,
  denied `getppid`, immutable Bash content, and cleanup passed in the same run.
- Human direction on 2026-11-03 approved a balanced execution profile for Gate
  5: native instruction execution with minimal confined passthrough for
  read-only loader/image, memory, signal, and bootstrap-console operations;
  sensitive interfaces remain emulated, proxied, or denied. The security
  profile retains interpreted execution and Ginux-virtual descriptors.
- The approved balanced model is now executable. Local checks passed explicit
  syscall-surface and negative policy tests. QEMU passed Rocky Bash with
  classified read-only image and console passthrough, virtual identity, denied
  ioctl, blocked `O_CREAT` with no file created, blocked socket creation, and
  fail-closed supervisor loss.
- The latest native QEMU run also passed unexpected-stop handling: after an
  external `SIGSTOP`, Ginux killed and reaped the tracee and cleaned domain
  state. Four VFS unit tests now include independent overlay-instance isolation
  plus traversal, symlink-loop, invalid-descriptor, overlay, and teardown paths.
- `make demo-gate-5` passed all eight named checks and reported its explicit
  non-claims. A fresh aggregate `make test` also passed all earlier-gate lint,
  unit, QEMU interception/runtime/config/domain, and image-pipeline regressions.
- Gate 5 is delivered in open pull request
  https://github.com/gmkurtzer/ginux-us/pull/5 from dedicated branch
  `hdae/virtual-vfs-userspace`. It is not merged or closed.
- Human accepted Gate 5 on 2026-11-03 and directed advancement to Gate 6.
- Gate 6 began on `hdae/device-ownership`. The first milestone added the
  bounded Ginux device registry, virtual descriptor namespace, assignment
  visibility, exclusive ownership and revocation, console/null/zero/random
  frontends, fail-closed ioctls and mappings, bounded partitions, and the
  isolated packet-queue model.
- `cargo test -p ginux-devices`, its strict clippy check, and the initial
  component targets passed. `make test-devices` then passed end-to-end in the
  Rocky QEMU host with normalized assignment, a disposable file-backed bounded
  partition, isolated packet exchange, denial checks, flush, revocation, and
  backend reuse.
- `make bench-block` and `make bench-network` passed in Rocky x86-64 under QEMU
  TCG and recorded workload, repetitions, bytes, wall time, process CPU time,
  throughput, latency, and environment without imposing a budget.
- `make demo-gate-6` passed all 6 named checks. A fresh `make test` passed
  formatting, strict lint, 31 unit tests, prior-gate regressions, Gate 2 image
  checks, and the Gate 6 Rocky QEMU device path.
- Gate 6 is delivered in open pull request
  https://github.com/gmkurtzer/ginux-us/pull/6 from dedicated branch
  `hdae/device-ownership`. It is not merged or closed.
- Human accepted Gate 6 on 2026-11-03 and directed advancement to Gate 7.
- Human direction added usable multi-user login, virtual PID inspection,
  lockdown checks, and a DNF update workflow to the Gate 9 systemd milestone.
  The DNF repository source remains an explicit decision required before Gate
  9 begins and does not block Gate 7.
- Gate 7 began on `hdae/process-runtime`. Its first milestone added a bounded
  Ginux process table with virtual lifecycle and identity, generation-safe task
  handles, virtual credentials and capability dropping, signal masks/queues,
  futex and eventfd primitives, deterministic exhaustion, and shutdown cleanup.
- The initial `cargo test -p ginux-process`, strict clippy, and
  `make test-process` run passed six component tests. Native supervisor
  integration, stress, QEMU fixtures, aggregate regression, and the demo remain
  incomplete.
- The second Gate 7 milestone added signal dispositions and exec reset,
  monotonic/realtime clocks, one-shot and periodic timers, and bounded
  epoll-style event readiness. Seven process-component tests and strict clippy
  passed; native integration and interruption semantics remain incomplete.
- The Rocky QEMU `make test-process` fixture passed 256 repeated virtual
  lifecycle cycles plus signals, virtual credentials, futex, eventfd, epoll,
  timers, stale identity denial, and zero-task cleanup. This fixture currently
  validates the Ginux process model.
- Gate 7 native supervision now auto-attaches forked tracees and translates
  `fork`, `getpid`, `getppid`, and `wait4` across host and virtual identities.
  The Rocky QEMU stress passed 32 fork/wait cycles with two concurrent children
  per round, 32 expected child exits, interrupted-wait restart handling, and
  zero-task cleanup.
- Gate 7 stress found and fixed raw `PTRACE_PEEKTEXT` ABI handling, the
  auto-attached child's initial trace phase, and host-PID argument restoration
  across an interrupted/restarted `wait4`.
- Fresh `make test-runtime`, `make test-native-vfs`, `make demo-gate-7`, and
  aggregate `make test` passed. The demo reports nine named checks; the native
  regression retains fail-closed supervisor-loss and unexpected-stop evidence.
- Gate 7 is technically passed. Pull request
  https://github.com/gmkurtzer/ginux-us/pull/7 was merged by the human on
  2026-11-04, but the gate still awaits explicit human checkpoint acceptance.
- Gate 7.5 completed the transitional active-project rename, selected `ginux`
  as the executable, and selected `ginux-*` package names; its deterministic
  stale-name check passed.
- Post-rename `make check`, `make test`, `make test-native-vfs`, and
  `make demo-gate-7` passed. The demonstration again passed all nine checks.
- The first aggregate run found a predecessor generated rootfs manifest in the
  persistent cache. The cache now rejects that marker and rebuilds it; the
  image suite and full aggregate suite passed after the migration fix.
- The transitional GitHub repository rename preserved PRs 1 through 7 as
  reachable merged records. Gate 7.5 remains delivered in open
  https://github.com/gmkurtzer/ginux-us/pull/8 through repository redirects.
- Human direction on 2026-11-04 selected Ginux as the final project and
  repository name because the full kernel project is pivoting to this
  userspace-kernel architecture. The local checkout directory must not move.
- Gate 7.75 active surfaces and demonstrations now use Ginux. `make check`,
  `make test`, `make test-native-vfs`, and `make demo-gate-7` passed; the
  demonstration again passed all nine process-runtime checks.
- Gate 7.75 is delivered in open
  https://github.com/gmkurtzer/ginux-us/pull/9 from `hdae/ginux-rename`.
  GitHub rejected renaming the repository to `gmkurtzer/ginux` with HTTP 422
  because that repository already exists and is non-empty.
- Human direction on 2026-11-04 accepted Gates 7, 7.5, and 7.75 and directed
  advancement to Gate 8. Repository integration into the existing Ginux project
  is deferred until the separately planned move on the following morning.
- Gate 8 began on `hdae/virtual-kernel-views`. Its first milestone added the
  no-std kernel-view component, bounded process snapshots, virtual procfs task,
  memory, uptime, mount, hostname, and boot-identity reads, explicit read-only
  mutation denial, and host-PID exclusion tests.
- Gate 8 virtual sysfs now projects only assigned devices with virtual
  class/type/major/minor/uevent data and virtual CPU inventory. Unassigned
  host-device lookup and all sysfs mutation fail explicitly.
- Gate 8 now also has a bounded cgroup v2 hierarchy and domain-owned machine,
  boot, hostname, mount, PID, UTS, and cgroup namespace identities. Six
  component tests cover positive systemd-style operations and negative
  privilege, mutation, capacity, traversal, host-PID, and device leakage paths.
- Gate 8 Rocky QEMU integration passed virtual procfs, sysfs, cgroup, and
  identity markers while host cgroup, mount, and block inventories remained
  byte-identical. `make demo-gate-8` passed all seven named checks.
- Fresh aggregate `make test` passed 44 component tests, all prior-gate QEMU
  regressions, and the Gate 8 QEMU fixture.
- Gate 8 was merged through https://github.com/gmkurtzer/ginux-us/pull/10 as
  `2306c2e`; the human accepted its demonstration and evidence on 2026-11-04.
- Both repositories were fetched after that merge and verified to have no open
  pull requests or issues. The final standalone-kernel commit is `4d4a53bb`.

## Open decisions and limitations

- Human direction selected the security-focused handcrafted x86-64 interpreter
  for Gate 3 and accepted its evidence. On 2026-11-03 the human selected native
  host ELF execution under continuous ptrace supervision for Gate 5
  compatibility. The host handles instructions, ELF loading, memory, faults,
  signals, and scheduling; Ginux must stop every syscall before effect and fail
  closed on lost tracing. Approved passthrough calls require ptrace entry/exit
  stops because `PTRACE_SYSEMU` always skips the syscall implementation.
- Rocky host image is pinned to official Rocky 10.2 GenericCloud build
  10.2-20260525.0. Gate 2 selected a pinned, network-disabled
  Docker/LinuxKit helper that mounts a temporary raw conversion read-only;
  performance budgets remain a later-gate decision.
- Gate 2 preparation requires Docker Desktop and a pinned privileged helper
  container. It is network-disabled and mounts only a temporary raw conversion
  read-only, but remains trusted build tooling rather than a runtime boundary.
- Product, source, and cache identifiers use Ginux. Gate 8.5 is moving this
  accepted userspace-kernel lineage into the existing Ginux repository while
  retaining the separate checkout and repository until a later archival
  decision.
- The selected Rocky OCI image has one layer; deterministic synthetic fixtures
  now cover general multi-layer replacement, whiteout, and opaque-directory
  semantics.
- The native Bash proof uses a read-only unprivileged chroot and limits kernel
  descriptors to immutable image content and the bootstrap console. Gate 6
  adds Ginux-owned device objects rather than expanding native passthrough.
- Gate 6 uses a Ginux-owned isolated packet queue as its mediated
  network backend. It does not provide host or external network connectivity;
  later connectivity backends must preserve the same virtual interface and
  mediation boundary.
- Unicorn Engine 2 is not part of the Gate 5 path. It is preserved as deferred
  Gate 14, with reconsideration required after the native compatibility path is
  demonstrated and before production-boundary claims are made.

## Exact next action

Human review of https://github.com/gmkurtzer/ginux/pull/152. Inspect the joined
history, userspace source tree, unified README, preserved `bootable-kernel`
branch and `bootable-kernel-final` tag, and the Gate 8.5 evidence before
accepting or merging the transition.

## Context recovery

- Active skill: `hdae-development`
- Skill path: `.agents/skills/hdae-development/SKILL.md`
- Reload after context shift: required
- Active gate: Gate 8.5, unified repository transition, authorized on dedicated
  branch `codex/userspace-transition`; implementation, publication, and all
  objective verification pass; pull request 152 awaits human review
- Last evidence: both lineages are ancestors of the transition head; application
  source matches staging commit `2306c2e`; `make check` and unrestricted
  `ROCKY_SSH_PORT=22242 make test` passed, including all 44 component tests and
  every Rocky QEMU regression; GitHub Actions run 41926232434 passed the
  replacement userspace `make check` and `make test-unit` job
- Required human decision: accept or request changes to pull request 152; the
  Gate 9 DNF repository choice remains deferred until before Gate 9
  implementation
