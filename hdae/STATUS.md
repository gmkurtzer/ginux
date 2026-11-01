# HDAE Status

## Recovery checkpoint

- Active skill: `hdae-development`
- Skill path: `.agents/skills/hdae-development/SKILL.md`
- Reload after context shift: required
- Active project: native controller desktop
- Branch: `codex/controller-desktop`
- Intended base: `main` at `759cf2e2`
- Active gate: Gate 24, native windowing system and toolkit
- Status: Gates 19–23 closed; Gate 24 starting
- Last evidence: Gate 23 passed `make test-controller-seat-leases`,
  `make test-smp`, `make check`, the display-protocol concurrency/recovery
  tests, ring-3 QEMU conformance, and `git diff --check`; commit `f942f71c`
- Open decisions or limitations: none inside Gate 19; GPU acceleration, raw GPU
  passthrough, and broad desktop compatibility are deferred
- Exact next action: load Gate 24 and implement the bounded compositor,
  window-state model, trusted chrome, software rendering, native toolkit, and
  deterministic window/input/damage tests over the Gate 23 seat model

## Governing outcome

Ginux owns the physical display/input seat and presents a native ring-3
controller desktop. Native applications use a Ginux protocol; Linux domains run
their own Wayland environment and export one desktop surface that can be
windowed or full screen. The CLI remains the recovery path.

## Confirmed review policy

- Gate 19: accepted by the human on 2026-10-31
- Gate 20: passed and closed under delegated review on 2026-10-31
- Gate 21: passed and closed under delegated review on 2026-10-31
- Gate 22: passed and closed under delegated review on 2026-10-31
- Gate 23: passed and closed under delegated review on 2026-10-31
- Gates 20–30: delegated closure after complete objective evidence; continue
  without another planned checkpoint and stop only for a genuine clarification,
  failed dependency, or material scope/architecture change

## Repository notes

Gates 1–12 and their inserted sub-gates remain locked historical records.
Unrelated open Gates 13–18 are preserved unchanged; this approved batch begins
at the next available integer and does not claim their completion.
