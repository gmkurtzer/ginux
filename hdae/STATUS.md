# Ginus HDAE Status

## Current checkpoint

- Gate 1 is accepted and closed under the corrected ordinary-host-process
  architecture. It remains delivered in open
  https://github.com/gmkurtzer/usshim/pull/1 and is not merged.
- Gate 2 is accepted and closed. It remains delivered in open
  https://github.com/gmkurtzer/usshim/pull/2 and is not merged.
- Gate 3 is accepted and closed. It remains delivered on dedicated branch
  `hdae/execution-kernel`, stacked on the accepted Gate 2 state while its
  intended eventual base remains `main`, in open
  https://github.com/gmkurtzer/usshim/pull/3.
- Gate 4 is accepted and closed. It remains delivered on dedicated branch
  `hdae/domain-config-lifecycle`, stacked on the accepted Gate 3 state while
  its intended eventual base remains `main`, in open
  https://github.com/gmkurtzer/usshim/pull/4.
- Authoritative name: Ginus (Ginux Userspace).
- Runtime isolation unit: domain.
- Policy unit: versioned contract.
- Initial gate map: twelve checkpoint gates at positions 1 through 11 and 13;
  Gates 1 through 4 are accepted and eight gates remain open.
- The ordered gate batch is authorized, but every open gate remains a human
  checkpoint requiring a working demonstration and explicit acceptance.

## Governing outcome and invariants

A normal Rocky Linux 10 host must boot with systemd as real PID 1. Ginus runs
as an ordinary host process and launches an unmodified Rocky Linux 10 systemd
userspace as virtual domain PID 1 without allowing managed code to invoke host
syscalls directly. All managed interactions are emulated, mediated, or denied
according to the domain contract. Domain-visible kernel state and devices are
Ginus-owned.

See `hdae/PROJECT.md` and `hdae/ARCHITECTURE.md` for the authoritative intent,
scope, threat model, and invariants.

## Evidence

- Repository inspection before establishment: clean `main` branch with no
  pre-existing root `hdae/` project ledger.
- Human direction confirmed the Ginus name, domain terminology, Rust/x86-64
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
  normally with host systemd as PID 1; Ginus is an ordinary process or service
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
- Rocky Linux 10.2 booted normally with host systemd as real PID 1. Ginus ran
  as the unprivileged ordinary `ginus` process, mediated the fixed payload,
  exited zero, and left systemd plus `multi-user.target` healthy.
- The revised demo showed virtual `getpid` 4242, checked write mediation without
  host execution, `uname` denied with `EPERM`, no direct-write marker, matched
  Rocky-host benchmark samples, clean QEMU shutdown, and no remaining PID file
  or VM process.
- Human accepted Gate 1 on 2026-11-03 and directed advancement to Gate 2.
- Final Gate 1 verification after removing Ginus numeric host-PID checks passed
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
  explicitly stated that Ginus execution, VFS, and systemd boot are not Gate 2
  evidence. The revised demonstration passed on 2026-11-03.
- Human accepted Gate 2 on 2026-11-03 and directed advancement to Gate 3.
- Gate 3 implemented the security-focused x86-64 interpreter, Ginus-owned
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
- Human approved the Gate 5 backing strategy on 2026-11-03: provision the
  verified rootfs tar and Ginus index once into a persistent derived Rocky host
  QCOW2, boot disposable overlays above it, and keep domain writes in a bounded
  in-memory overlay.

## Open decisions and limitations

- Human direction selected the security-focused full x86-64 interpreter as the
  Gate 3 baseline. Gate 4 exposes execution profile/backend selection but
  rejects the performance profile until a separately reviewed backend exists.
- Rocky host image is pinned to official Rocky 10.2 GenericCloud build
  10.2-20260525.0. Gate 2 selected a pinned, network-disabled
  Docker/LinuxKit helper that mounts a temporary raw conversion read-only;
  network backend and performance budgets remain later-gate decisions.
- Gate 2 preparation requires Docker Desktop and a pinned privileged helper
  container. It is network-disabled and mounts only a temporary raw conversion
  read-only, but remains trusted build tooling rather than a runtime boundary.
- The repository and directory still use the historical `usshim` name; rename
  is not authorized in the current planning task.

## Exact next action

Implement the Gate 5 immutable Rocky base index and virtual path/object model on
`hdae/virtual-vfs-userspace`.

## Context recovery

- Active skill: `hdae-development`
- Skill path: `.agents/skills/hdae-development/SKILL.md`
- Reload after context shift: required
- Active gate: Gate 5, virtual VFS and dynamic userspace; authorized and in
  progress on `hdae/virtual-vfs-userspace`
- Last evidence: Gate 4 demo and complete aggregate regression suite passed on
  2026-11-03
- Required human decision: none until the Gate 5 demonstration checkpoint
