# HDAE Status

## Recovery checkpoint

- Active skill: `hdae-development`
- Skill path: `.agents/skills/hdae-development/SKILL.md`
- Reload after context shift: required
- Active project: native controller desktop
- Branch: `codex/controller-desktop`
- Intended base: `main` at `759cf2e2`
- Active gate: Gate 31, controller desktop visual design
- Status: objective implementation and qualification complete; awaiting the
  final human visual and interaction checkpoint for Gate 31
- Last evidence: `make check`, `make test-controller-desktop-all`,
  `make test-controller-desktop-performance`, and real-QEMU
  `make test-controller-desktop-visible` passed on 2026-11-01. Retained native
  800 by 600 frames prove click navigation, independent pointer motion, native
  keyboard input, live domain inventory, and the Reboot/Shut down menu.
- Open decisions or limitations: final visual quality and usability still
  require the confirmed human checkpoint; software rendering and the serial
  recovery CLI remain architectural constraints
- Exact next action: present the retained framebuffer evidence and `make
  run-gui` workflow for final human visual and interaction acceptance

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
