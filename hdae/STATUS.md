# HDAE Status

## Recovery checkpoint

- Active skill: `.agents/skills/hdae-development/SKILL.md`
- Reload required after compaction, reset, resume, or handoff: yes
- Active project: feedback-driven kernel hardening
- Branch: `codex/feedback-hardening`
- Baseline tag: `research-v1-baseline`
- Active gate: Gate 5, runtime state ownership, in progress
- Last completed action: consolidated 13 Linux process-table, address-space,
  fork scratch, and diagnostic globals into one per-instance process-runtime
  owner; `make check`, `make test-linux-abi`, and the complete verification
  QEMU sequence passed
- Next action: design the domain-generation-owned socket runtime so its large
  self-referential buffer/storage relationship is preserved without retaining
  14 independent ambient globals, then migrate the x86-64 execution-state bank

## Current repository state

Local and remote `main` identify commit `1852455`. Gates 1 through 4 passed and
closed under delegated review. Gate 5 is active; 461 classified runtime
globals remain assigned to it after the process ownership consolidation.

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
