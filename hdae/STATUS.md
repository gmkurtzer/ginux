# Ginus HDAE Status

## Current checkpoint

- Gate 1 is accepted and closed under the corrected ordinary-host-process
  architecture. It remains delivered in open
  https://github.com/gmkurtzer/usshim/pull/1 and is not merged.
- Gate 2 is active on the dedicated `hdae/rocky-image-pipeline` branch. Until
  Gate 1 is merged, Gate 2 is stacked on its accepted branch state while its
  intended base remains `main`.
- Authoritative name: Ginus (Ginux Userspace).
- Runtime isolation unit: domain.
- Policy unit: versioned contract.
- Initial gate map: twelve checkpoint gates under `hdae/gates-todo/` at
  positions 1 through 11 and 13.
- The twelve remaining gates are authorized as an ordered batch, but each
  remains a human checkpoint requiring a working demonstration and explicit
  acceptance.

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

## Open decisions and limitations

- `PTRACE_SYSEMU` is accepted only as the Gate 1 bootstrap experiment. The
  final Ginus execution/task/memory design remains a Gate 3 decision.
- Rocky host image is pinned to official Rocky 10.2 GenericCloud build
  10.2-20260525.0. Gate 2 selected a pinned, network-disabled
  Docker/LinuxKit helper that mounts a temporary raw conversion read-only;
  configuration syntax, network backend, and performance budgets remain
  later-gate decisions.
- The repository and directory still use the historical `usshim` name; rename
  is not authorized in the current planning task.

## Exact next action

Implement and prove the selected offline QCOW2 extraction backend, then add
image validation, corruption and repeatability checks, and `make demo-gate-2`.

## Context recovery

- Active skill: `hdae-development`
- Skill path: `.agents/skills/hdae-development/SKILL.md`
- Reload after context shift: required
- Active gate: Gate 2, Rocky domain-userspace pipeline; preparation contract
  selected, implementation in progress
- Last evidence: final PID-independent Gate 1 checks and demo passed on
  2026-11-03; Gate 1 accepted
- Required human decision: none until the Gate 2 checkpoint unless its
  preparation backend requires a material architecture change
