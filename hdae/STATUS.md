# HDAE Status

## Recovery checkpoint

- Active skill: `.agents/skills/hdae-development/SKILL.md`
- Reload required after compaction, reset, resume, or handoff: yes
- Active project: feedback-driven kernel hardening
- Branch: `codex/feedback-hardening`
- Baseline tag: `research-v1-baseline`
- Active gate: Gate 4, global-state classification and ownership design, ready
  to begin
- Last completed action: Gate 3 migrated syscall-side userspace access to
  fault-recovering primitives and passed `make check`, the full verification
  boot, the 398-row Linux ABI report, and attached plus detached Rocky systemd
  lifecycle tests
- Next action: generate the complete writable-global inventory, classify every
  item by owner and lifetime, and turn that classification into a checked
  allowlist and migration map

## Current repository state

Local and remote `main` identify commit `1852455`. Gates 1 through 3 passed and
closed under delegated review. Gate 4 is active; its dependency is satisfied.

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
