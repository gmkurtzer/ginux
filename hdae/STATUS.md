# HDAE Status

## Recovery checkpoint

- Active skill: `hdae-development`
- Skill path: `.agents/skills/hdae-development/SKILL.md`
- Reload after context shift: required
- Active project: native controller desktop
- Branch: `codex/controller-desktop`
- Intended base: `main` at `759cf2e2`
- Active gate: Gate 19, controller desktop architecture
- Status: planning artifacts being persisted; architecture deliverables not yet
  verified
- Last evidence: current `origin/main` was fetched and the branch was created
  from commit `759cf2e2`
- Open decisions or limitations: none inside Gate 19; GPU acceleration, raw GPU
  passthrough, and broad desktop compatibility are deferred
- Exact next action: create the approved Gate 19–30 ledger, complete Gate 19's
  architecture decision and protocol/state specifications, run documentation
  verification, commit, and stop for the Gate 19 human checkpoint

## Governing outcome

Ginux owns the physical display/input seat and presents a native ring-3
controller desktop. Native applications use a Ginux protocol; Linux domains run
their own Wayland environment and export one desktop surface that can be
windowed or full screen. The CLI remains the recovery path.

## Confirmed checkpoint map

- Gate 19: human architecture checkpoint
- Gates 20–22: delegated after objective evidence
- Gate 23: human resource/focus checkpoint
- Gates 24–26: delegated after objective evidence
- Gate 27: human Linux/Wayland checkpoint
- Gates 28–30: delegated after objective evidence, then batch report

## Repository notes

Gates 1–12 and their inserted sub-gates remain locked historical records.
Unrelated open Gates 13–18 are preserved unchanged; this approved batch begins
at the next available integer and does not claim their completion.
