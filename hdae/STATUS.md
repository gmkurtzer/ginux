# Ginus HDAE Status

## Current checkpoint

- Gate 1, foundation and interception proof, is technically passed on branch
  `hdae/foundation-interception` and is pending pull-request delivery and human
  acceptance.
- Authoritative name: Ginus (Ginux Userspace).
- Runtime isolation unit: domain.
- Policy unit: versioned contract.
- Initial gate map: thirteen checkpoint gates under `hdae/gates-todo/`.
- Gates 1 through 13 are authorized as an ordered batch, but each remains a
  human checkpoint requiring a working demonstration and explicit acceptance.
- Gate 2 must not begin until Gate 1 is explicitly accepted.

## Governing outcome and invariants

Ginus must boot as host PID 1 and run an unmodified Rocky Linux 10 systemd
userspace inside a domain without allowing managed code to invoke host syscalls
directly. All interactions are emulated, mediated, or denied according to the
domain contract. Domain-visible kernel state and devices are Ginus-owned.

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
  ordered 13-gate batch, required a runnable human demonstration for every
  checkpoint, and authorized pushing dedicated branches and opening PRs.
- Coverage review: functionality, failure behavior, deterministic tests, user
  configuration, documentation, compatibility, recovery, performance, and
  security are assigned to explicit gates; none is silently omitted.
- Gate 1 clean verification on 2026-11-03 passed `make doctor`, `make build`,
  `make check`, `make test-unit`, `make test-intercept`, `make test`,
  `make bench-baseline`, and `make demo-gate-1` under QEMU TCG.
- The demo showed virtual `getpid` 4242, checked write mediation without host
  execution, `uname` denied with `EPERM`, no direct-write marker, successful
  QEMU shutdown, and no remaining background process.
- The baseline recorded 30 nanosecond samples across matched direct-Linux and
  namespace-container CPU, syscall, and process workloads, with five
  repetitions and variance for each combination.

## Open decisions and limitations

- `PTRACE_SYSEMU` is accepted only as the Gate 1 bootstrap experiment. The
  final Ginus execution/task/memory design remains a Gate 3 decision.
- The exact Rocky image, image preparation backend, configuration syntax,
  network backend, performance budgets, and public CVE remain decisions
  assigned to later gates.
- The repository and directory still use the historical `usshim` name; rename
  is not authorized in the current planning task.

## Exact next action

Commit the Gate 1 evidence, push `hdae/foundation-interception`, open and record
its pull request, then stop for human acceptance after `make demo-gate-1`.

## Context recovery

- Active skill: `hdae-development`
- Skill path: `.agents/skills/hdae-development/SKILL.md`
- Reload after context shift: required
- Active gate: Gate 1 technically passed; delivery and human acceptance pending
- Last evidence: full clean Gate 1 command set passed on 2026-11-03
- Required human decision: accept or reject Gate 1 after its demonstration
