# Ginus HDAE Status

## Current checkpoint

- Project establishment records are committed on `main`; the implementation
  batch is authorized.
- Authoritative name: Ginus (Ginux Userspace).
- Runtime isolation unit: domain.
- Policy unit: versioned contract.
- Initial gate map: thirteen checkpoint gates under `hdae/gates-todo/`.
- Gates 1 through 13 are authorized as an ordered batch, but each remains a
  human checkpoint requiring a working demonstration and explicit acceptance.
- Gate 1 is authorized to begin after this alignment update is committed.

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
- Implementation evidence: not run; no gate has begun.

## Open decisions and limitations

- The exact Rocky image, emulation-compatible trap design, image preparation backend,
  configuration syntax, network backend, performance budgets, and public CVE
  remain decisions assigned to later gates.
- The repository and directory still use the historical `usshim` name; rename
  is not authorized in the current planning task.

## Exact next action

Commit this alignment update, create branch `hdae/foundation-interception`, and
execute Gate 1 through fresh automated evidence and `make demo-gate-1`. Push
the branch, open its pull request, and stop for human acceptance.

## Context recovery

- Active skill: `hdae-development`
- Skill path: `.agents/skills/hdae-development/SKILL.md`
- Reload after context shift: required
- Active gate: Gate 1 authorized; alignment update pending before branch work
- Last evidence: planning records only; implementation not run
- Required human decision: accept or reject Gate 1 after its demonstration
