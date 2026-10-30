# HDAE Status

## Recovery checkpoint

- Active skill: `.agents/skills/hdae-development/SKILL.md`
- Reload required after compaction, reset, resume, or handoff: yes
- Active project: feedback-driven kernel hardening
- Branch: `codex/feedback-hardening`
- Baseline tag: `research-v1-baseline`
- Active gate: Gate 5, runtime state ownership, in progress
- Last completed action: consolidated Linux signal queues, keyring roots, and
  AIO roots into explicit per-instance owners and taught the inventory to
  recognize atomic owner structs; `make check`, `make test-linux-abi`, and the
  complete verification QEMU sequence passed
- Next action: consolidate the remaining mount, user, tmpfs, overlay,
  file-cache, SysV IPC, and address-space runtime roots before migrating the
  408-entry x86-64 execution-state bank

## Current repository state

Local and remote `main` identify commit `1852455`. Gates 1 through 4 passed and
closed under delegated review. Gate 5 is active; 436 classified runtime
globals remain assigned to it after the signal/keyring/AIO consolidation.

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
