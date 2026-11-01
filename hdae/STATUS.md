# HDAE Status

## Recovery checkpoint

- Active skill: `hdae-development`
- Skill path: `.agents/skills/hdae-development/SKILL.md`
- Reload after context shift: required
- Active project: native controller desktop
- Branch: `codex/controller-desktop`
- Intended base: `main` at `759cf2e2`
- Active gate: Gate 31, controller desktop visual design
- Status: objective 1024 by 768 visual and latency criteria pass; open at the
  required human visual and interaction checkpoint
- Last evidence: the first compact-font checkpoint was rejected by human review
  because the sidebar, rows, cards, and labels still retained oversized 1080p
  proportions. The resulting full-density revision passed
  `make test-controller-desktop-latency` on 2026-11-01 with exact 1024 by 768
  Overview, Settings, live-domain, and power-menu frames; 13-pixel body text;
  restrained 16-pixel headings; pointer/click navigation; 20.972 ms
  visible-frame p95; 21.238 ms maximum; and an 18.661 ms complete page change.
  The complete desktop suite, including 2-vCPU and 4-vCPU recovery stress,
  passed. The release host model passed at 835 us input-to-frame p95, 863 us
  steady-frame p95, and a bounded 3,175,728-byte working set. `make check`
  passed.
- Open decisions or limitations: the accepted architecture remains a native
  ring-3 controller shell with software rendering and an independent serial
  recovery CLI; final visual quality and real-QEMU interaction still require
  the confirmed human checkpoint
- Exact next action: commit the full-density revision, then present its retained
  real-QEMU frames and `make run-gui` workflow for human visual and interaction
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
- Gate 31: all objective criteria passed again on 2026-11-01 after human
  correction of the requested display size from 1920 by 1080 to the standard
  1024 by 768 mode. A subsequent human review requested denser typography and
  spacing at that resolution, then rejected the first font-only refinement as
  materially unchanged. Objective checks pass after the resulting full-density
  redesign, and the gate remains open for human visual and interaction
  acceptance.
- Gates 20–30: delegated closure after complete objective evidence; continue
  without another planned checkpoint and stop only for a genuine clarification,
  failed dependency, or material scope/architecture change

## Repository notes

Gates 1–12 and their inserted sub-gates remain locked historical records.
Unrelated open Gates 13–18 are preserved unchanged; this approved batch begins
at the next available integer and does not claim their completion.
