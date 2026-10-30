# HDAE Status

## Recovery checkpoint

- Active skill: `.agents/skills/hdae-development/SKILL.md`
- Reload required after compaction, reset, resume, or handoff: yes
- Active project: feedback-driven kernel hardening
- Branch: `codex/feedback-hardening`
- Baseline tag: `research-v1-baseline`
- Active gate: Gate 8, synchronization and SMP readiness, starting
- Last completed action: Gate 8 established ranked ticket and IRQ-safe locks,
  verified their boot contract, and migrated the physical frame allocator from
  single-CPU `UnsafeCell` access to rank-20 cross-CPU serialization with the
  complete deterministic QEMU suite passing
- Next action: audit and migrate scheduler and process-table shared state,
  then run focused lifecycle/concurrency and complete QEMU regressions

## Current repository state

Local and remote `main` identify commit `1852455`. Gates 1 through 7 passed and
closed under delegated review. Gate 8 is active on the shared hardening branch.

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
