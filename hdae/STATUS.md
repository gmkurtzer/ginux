# HDAE Status

## Recovery checkpoint

- Active skill: `.agents/skills/hdae-development/SKILL.md`
- Reload required after compaction, reset, resume, or handoff: yes
- Active project: feedback-driven kernel hardening
- Branch: `codex/feedback-hardening`
- Baseline tag: `research-v1-baseline`
- Active gate: none; the ten-gate map is approved and Gate 1 awaits execution
  authorization
- Last completed action: aligned the gate map with human direction: real SMP is
  required, ordinary QEMU tests become 2-vCPU after SMP passes, security scope
  is limited to the two reported defects, global state is comprehensively
  addressed, and all gates use one branch and one pull request
- Next action: authorize Gate 1 before adding reproductions or changing kernel
  code

## Current repository state

Local and remote `main` identify commit `1852455`. Local and remote
`codex/feedback-hardening` identify the approved planning commit before this
alignment update. The working branch contains no hardening implementation yet.

## Open decisions

- Authorize Gate 1 execution.
- Freeze the exact baseline command set at the start of Gate 1; the proposed
  minimum is recorded in that gate and may not be silently reduced.

## Durable evidence

The detailed rationale is in
[`docs/FEEDBACK-HARDENING-RESEARCH.md`](../docs/FEEDBACK-HARDENING-RESEARCH.md).
The authoritative execution state and gate outcomes are the files under this
`hdae/` directory.
