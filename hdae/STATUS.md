# HDAE Status

## PR #85 acceptance follow-up (2026-11-04)

Active skill: `.agents/skills/hdae-development/SKILL.md`; full reload and method
reference required after context shift. Active gate:
[`46-scheduler-reproducible-acceptance`](gates-todo/46-scheduler-reproducible-acceptance.md).
Branch: `fix/background-domain-preemption`; status: partial. Current main
`c8d12653` integrated in `713ca279`; desktop nonblocking reads preserved.
The user authorized continuation of PR #85 to remove private-fixture/KVM access
dependencies. Source-built cache, portable harness and public CI are in progress.
Next: validate TCG runtime and oracle controls, then the full 1/2/4 CPU matrix,
static/full boot regression, and publish exact evidence. No merge is authorized.
The controller-desktop checkpoint below is historical upstream context.

## Recovery checkpoint

- Active skill: `hdae-development`
- Skill path: `.agents/skills/hdae-development/SKILL.md`
- Reload after context shift: required
- Active project: native controller desktop
- Branch: `codex/controller-desktop`
- Intended base: current `main`; integration merge with `7fdc880e` is in progress
- Active gate: none in the controller-desktop batch; Gate 31 is closed
- Status: Gates 19–31 are complete; Gate 31 passed all objective criteria and
  received final human visual and interaction acceptance on 2026-11-01
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
  passed. A follow-up power-control refinement replaced the block glyph with a
  bounded antialiased button; its real-QEMU menu test passed at 20.844 ms p95,
  20.973 ms maximum, and 20.485 ms for a complete page change.
- Current-main integration evidence: merged `main` at `7fdc880e`, preserved the
  timestamp ABI at operation 39, reassigned the branch-local display/input
  operations to 40/41, and corrected native-network layout detection to use the
  current 161-page image contract. `make check` and `make test` pass. The
  consolidated controller-desktop suite passes its unit, QEMU, and 2/4-vCPU
  recovery stages; the visible-frame test passes separately with 48.915 ms p95,
  49.075 ms maximum, and an 18.286 ms page change after removing the
  sandbox-only QEMU monitor-socket restriction. The release performance
  qualification passes at 869 us input-to-frame p95, 504 us steady-frame p95,
  and a bounded 3,175,728-byte modeled working set.
- Open decisions or limitations: the accepted architecture remains a native
  ring-3 controller shell with software rendering and an independent serial
  recovery CLI; unrelated open Gates 13–18 remain outside this completed batch
- Exact next action: commit and push the validated integration merge to pull
  request #39, then wait for exact-head CI and a later review pass before merge

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
  materially unchanged. The resulting full-density redesign and refined power
  control passed the complete objective suite and received final human visual
  and interaction acceptance on 2026-11-01. Gate 31 is closed.
- Gates 20–30: delegated closure after complete objective evidence; continue
  without another planned checkpoint and stop only for a genuine clarification,
  failed dependency, or material scope/architecture change

## Repository notes

Gates 1–12 and their inserted sub-gates remain locked historical records.
Unrelated open Gates 13–18 are preserved unchanged; this approved batch begins
at the next available integer and does not claim their completion.
