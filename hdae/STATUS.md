# HDAE Status

## Recovery checkpoint

- Active skill: `hdae-development`
- Skill path: `.agents/skills/hdae-development/SKILL.md`
- Reload after context shift: required
- Active project: native controller desktop
- Branch: `codex/controller-desktop`
- Intended base: `main` at `759cf2e2`
- Active gate: Gate 31, controller desktop visual design
- Status: all objective Gate 31 criteria pass; final human framebuffer and
  interaction acceptance remains required
- Last evidence: `make check`, `make test-controller-desktop-all`,
  `make test-controller-desktop-performance`, and final real-QEMU
  `make test-controller-desktop-visible` passed on 2026-11-01. Retained frames
  prove click navigation, independent pointer motion, native keyboard input,
  and a CLI-created domain appearing in the live GUI.
- Open decisions or limitations: visual quality and usability require the
  confirmed final human checkpoint; the renderer remains a bounded software
  composition without runtime font loading or GPU acceleration by design
- Exact next action: human runs `make run-gui`, exercises pointer, clicks,
  keyboard, and live domain state, then accepts Gate 31 or reports specific
  remaining visual or interaction defects

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
- Gate 31: expanded by human direction on 2026-11-01 to include responsive
  input, clickable controls, live state, and visual quality; proceed
  autonomously to final QEMU interaction and visual review
- Gates 20–30: delegated closure after complete objective evidence; continue
  without another planned checkpoint and stop only for a genuine clarification,
  failed dependency, or material scope/architecture change

## Repository notes

Gates 1–12 and their inserted sub-gates remain locked historical records.
Unrelated open Gates 13–18 are preserved unchanged; this approved batch begins
at the next available integer and does not claim their completion.
