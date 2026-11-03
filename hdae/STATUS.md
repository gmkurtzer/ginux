# Ginus HDAE Status

## Current checkpoint

- Project establishment records are drafted on `main` for human review.
- Authoritative name: Ginus (Ginux Userspace).
- Runtime isolation unit: domain.
- Policy unit: versioned contract.
- Initial gate map: thirteen checkpoint gates under `hdae/gates-todo/`.
- No implementation gate is authorized or active.

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
- Coverage review: functionality, failure behavior, deterministic tests, user
  configuration, documentation, compatibility, recovery, performance, and
  security are assigned to explicit gates; none is silently omitted.
- Implementation evidence: not run; no gate has begun.

## Open decisions and limitations

- Human review of the detailed gate ledger is required before Gate 1 begins.
- The exact Rocky image, KVM trap design, image preparation backend,
  configuration syntax, network backend, performance budgets, and public CVE
  remain decisions assigned to later gates.
- The repository and directory still use the historical `usshim` name; rename
  is not authorized in the current planning task.

## Exact next action

Review the proposed gate files with the human. Correct any missing or
incorrectly scoped criteria, then obtain explicit approval for Gate 1 and its
checkpoint before creating branch `hdae/foundation-interception`.

## Context recovery

- Active skill: `hdae-development`
- Skill path: `.agents/skills/hdae-development/SKILL.md`
- Reload after context shift: required
- Active gate: none; establishment checkpoint
- Last evidence: planning records only; implementation not run
- Required human decision: approve or revise Gate 1 after reviewing the ledger
