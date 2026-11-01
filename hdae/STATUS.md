# HDAE Status

## Recovery checkpoint

- Active skill: `hdae-development`
- Skill path: `.agents/skills/hdae-development/SKILL.md`
- Reload after context shift: required
- Active project: native controller desktop
- Branch: `codex/controller-desktop`
- Intended base: `main` at `759cf2e2`
- Active gate: Gate 21, controller framebuffer and input devices
- Status: Gates 19–20 closed; Gate 21 starting
- Last evidence: Gate 20 passed `make test-controller-graphics-runtime`,
  `make check`, `make architecture-check`, native ABI tests, and
  `git diff --check`; commit `b57bce11`
- Open decisions or limitations: none inside Gate 19; GPU acceleration, raw GPU
  passthrough, and broad desktop compatibility are deferred
- Exact next action: inspect UEFI/QEMU framebuffer and input handoff support,
  establish the Gate 21 device baseline, and implement the bounded
  kernel-to-controller device contracts and deterministic QEMU test

## Governing outcome

Ginux owns the physical display/input seat and presents a native ring-3
controller desktop. Native applications use a Ginux protocol; Linux domains run
their own Wayland environment and export one desktop surface that can be
windowed or full screen. The CLI remains the recovery path.

## Confirmed review policy

- Gate 19: accepted by the human on 2026-10-31
- Gate 20: passed and closed under delegated review on 2026-10-31
- Gates 20–30: delegated closure after complete objective evidence; continue
  without another planned checkpoint and stop only for a genuine clarification,
  failed dependency, or material scope/architecture change

## Repository notes

Gates 1–12 and their inserted sub-gates remain locked historical records.
Unrelated open Gates 13–18 are preserved unchanged; this approved batch begins
at the next available integer and does not claim their completion.
