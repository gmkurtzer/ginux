# HDAE Status

## Recovery checkpoint

- Active skill: `.agents/skills/hdae-development/SKILL.md`
- Reload required after compaction, reset, resume, or handoff: yes
- Active project: feedback-driven kernel hardening
- Branch: `codex/feedback-hardening`
- Baseline tag: `research-v1-baseline`
- Active gate: Gate 2, x86-64 privilege-entry hardening, in progress
- Last completed action: Gate 2 installed CPU-indexable GS entry state, switched
  off the user RSP before stack use, assigned separately guarded NMI/DF/MCE
  ISTs, and added static architecture verification; `make check` passes
- Next action: isolate the late interpreted-thread return-context corruption,
  retain it as a regression, then add nested-NMI and adversarial-stack runtime
  coverage before rerunning the complete Gate 2 suite

## Current repository state

Local and remote `main` identify commit `1852455`. Gate 2 is partial. Its static
entry audit and `make check` pass, but `make test` still fails at the known late
interpreted-thread invalid return context (`RIP 0x404d87`) and then reaches a
kernel page fault. The QEMU harness cleaned up the failed run.

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
