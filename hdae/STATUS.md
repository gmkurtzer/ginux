# HDAE Status

## Recovery checkpoint

- Active skill: `.agents/skills/hdae-development/SKILL.md`
- Reload required after compaction, reset, resume, or handoff: yes
- Active project: feedback-driven kernel hardening
- Branch: `codex/feedback-hardening`
- Baseline tag: `research-v1-baseline`
- Active gate: Gate 10, qualification and comparison, in progress
- Last completed action: Gate 9 closed after implementing AP startup, per-CPU
  architecture and scheduler state, cross-CPU runtime mechanisms, domain-aware
  scheduling, explicit one/two/four-vCPU qualification, repeated stress, and a
  two-vCPU ordinary QEMU default
- Next action: run the frozen functional and adversarial qualification matrix,
  preserving exact failures and raw evidence before performance measurement

## Current repository state

Local and remote `main` identify commit `1852455`. Gates 1 through 9 passed and
closed under delegated review. Gate 10 is ready on the shared hardening branch.
The ordinary QEMU path now defaults to two vCPUs; the complete one/two/four-vCPU
matrix and two repeated stress passes succeed. Cross-CPU runtime, process
scheduling, affinity, caps, priority, QoS selection, accounting, process exit,
and mapping invalidation have deterministic evidence. Gate 10 must now rerun
the complete compatibility and workload matrix and compare it with the
published baseline without widening the security claims.

## Open decisions

- None. Per-CPU entry storage and distinct guarded NMI, double-fault, and
  machine-check ISTs are already approved architectural requirements.

## Limitations

- The floating upstream Alpine integration image currently stalls at Linux
  user entry on both Gate 5 and pre-Gate-5 commits. Deterministic Linux ABI and
  verification-boot suites pass; external Alpine compatibility is separate
  follow-up work.
- A Gate 7 user-access module extraction passed static and unit checks but
  failed deterministic QEMU twice and was reverted. Its assembly and Rust
  wrappers remain co-located with exception entry until emitted-code behavior
  can be compared without weakening the gate.

## Durable evidence

The detailed rationale is in
[`docs/FEEDBACK-HARDENING-RESEARCH.md`](../docs/FEEDBACK-HARDENING-RESEARCH.md).
Gate 1 evidence is in
[`docs/feedback-hardening/GATE-1-BASELINE.md`](../docs/feedback-hardening/GATE-1-BASELINE.md).
The authoritative execution state and gate outcomes are the files under this
`hdae/` directory.
