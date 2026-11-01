# HDAE Status

## Recovery checkpoint

- Active skill: `hdae-development`
- Skill path: `.agents/skills/hdae-development/SKILL.md`
- Reload after context shift: required
- Active project: native controller desktop
- Branch: `codex/controller-desktop`
- Intended base: `main` at `759cf2e2`
- Active gate: Gate 31, controller desktop visual design
- Status: technically passed; awaiting final human review of the actual QEMU
  framebuffer
- Last evidence: `make check`, `make test-controller-desktop-all`,
  `make test-controller-desktop-performance`, and real-QEMU
  `make test-controller-desktop-visible` passed on 2026-11-01; retained
  screenshots show keyboard and pointer state changes
- Open decisions or limitations: none inside Gate 19; GPU acceleration, raw GPU
  passthrough, and broad desktop compatibility are deferred
- Exact next action: present the retained QEMU framebuffer and obtain human
  visual acceptance; close Gate 31 only if accepted

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
- Gate 24: passed and closed under delegated review on 2026-10-31
- Gate 25: passed and closed under delegated review on 2026-10-31
- Gate 26: passed and closed under delegated review on 2026-10-31
- Gate 27: passed and closed under delegated review on 2026-10-31
- Gate 28: passed and closed under delegated review on 2026-10-31
- Gate 29: passed and closed under delegated review on 2026-10-31
- Gate 30: passed and closed under delegated review on 2026-10-31 after visible
  QEMU framebuffer, keyboard, pointer, performance, and regression evidence
- Gate 31: human checkpoints at concept acceptance and final QEMU visual review
- Gates 20–30: delegated closure after complete objective evidence; continue
  without another planned checkpoint and stop only for a genuine clarification,
  failed dependency, or material scope/architecture change

## Repository notes

Gates 1–12 and their inserted sub-gates remain locked historical records.
Unrelated open Gates 13–18 are preserved unchanged; this approved batch begins
at the next available integer and does not claim their completion.
