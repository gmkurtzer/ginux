# HDAE Status

## Recovery checkpoint

- Active skill: `.agents/skills/hdae-development/SKILL.md`
- Reload required after compaction, reset, resume, or handoff: yes
- Active project: feedback-driven kernel hardening
- Branch: `codex/feedback-hardening`
- Baseline tag: `research-v1-baseline`
- Active gate: Gate 3, fault-recovering userspace access, ready to begin
- Last completed action: Gate 2 passed six-stage NMI injection, hostile initial
  stack validation, `make check`, and the complete `make test` QEMU boot suite
- Next action: inventory direct userspace dereferences and define the Gate 3
  fault-fixup representation before migrating audited copy primitives

## Current repository state

Local and remote `main` identify commit `1852455`. Gate 2 passed and closed
under delegated review. Gate 3 is open; its first step is a complete direct
userspace-access inventory tied to a fault-recovery design.

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
