# HDAE Status

## Recovery checkpoint

- Active skill: `.agents/skills/hdae-development/SKILL.md`
- Reload required after compaction, reset, resume, or handoff: yes
- Active project: feedback-driven kernel hardening
- Branch: `codex/feedback-hardening`
- Baseline tag: `research-v1-baseline`
- Active gate: Gate 2, x86-64 privilege-entry hardening, in progress
- Last completed action: Gate 2 diagnosed the late interpreted-thread failure
  as stale last-preempted identity after slot reuse, fixed retirement cleanup,
  and passed the complete `make test` boot and Linux runtime suite
- Next action: add nested-NMI and adversarial-stack runtime coverage, then rerun
  the complete Gate 2 suite

## Current repository state

Local and remote `main` identify commit `1852455`. Gate 2 is partial. Its static
entry audit, `make check`, and complete `make test` boot suite pass. Nested-NMI
and hostile-stack runtime evidence remain outstanding.

## Open decisions

- None. Per-CPU entry storage and distinct guarded NMI, double-fault, and
  machine-check ISTs are already approved architectural requirements.

## Durable evidence

The detailed rationale is in
[`docs/FEEDBACK-HARDENING-RESEARCH.md`](../docs/FEEDBACK-HARDENING-RESEARCH.md).
Gate 1 evidence is in
[`docs/feedback-hardening/GATE-1-BASELINE.md`](../docs/feedback-hardening/GATE-1-BASELINE.md).
The authoritative execution state and gate outcomes are the files under this
`hdae/` directory.
