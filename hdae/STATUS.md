# HDAE Status

## Recovery checkpoint

- Active skill: `hdae-development`
- Skill path: `.agents/skills/hdae-development/SKILL.md`
- Reload after context shift: required
- Active project: native controller desktop
- Branch: `codex/controller-desktop`
- Intended base: `main` at `759cf2e2`
- Active gate: Gate 31, controller desktop visual design
- Status: objective 1920 by 1080 visual and latency criteria pass; open at the
  required human visual and interaction checkpoint
- Last evidence: `make test-controller-desktop-latency` passed on 2026-11-01
  with an exact 1920 by 1080 GOP frame, antialiased Inter text, pointer/click
  navigation, live CLI-to-GUI state, 43.624 ms visible-frame p95, 57.258 ms
  maximum, and a 57.258 ms complete page change. The complete desktop suite,
  including 2-vCPU and 4-vCPU recovery stress, passed. The release host model
  passed
  at 1,317 us input-to-frame p95 and a bounded 8,324,400-byte working set.
- Open decisions or limitations: the accepted architecture remains a native
  ring-3 controller shell with software rendering and an independent serial
  recovery CLI; final visual quality and real-QEMU interaction still require
  the confirmed human checkpoint
- Exact next action: present commit `9d8ac33b`, the retained real-QEMU
  framebuffer, and the `make run-gui` workflow for human visual and interaction
  acceptance

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
- Gate 31: all objective criteria passed on 2026-11-01 after the failed visual
  checkpoint; it remains open for the required human visual and interaction
  acceptance of the 1920 by 1080 revision.
- Gates 20–30: delegated closure after complete objective evidence; continue
  without another planned checkpoint and stop only for a genuine clarification,
  failed dependency, or material scope/architecture change

## Repository notes

Gates 1–12 and their inserted sub-gates remain locked historical records.
Unrelated open Gates 13–18 are preserved unchanged; this approved batch begins
at the next available integer and does not claim their completion.
