# HDAE Status

## Recovery checkpoint

- Active skill: `.agents/skills/hdae-development/SKILL.md`
- Reload required after compaction, reset, resume, or handoff: yes
- Active project: feedback-driven kernel hardening
- Branch: `codex/feedback-hardening`
- Baseline tag: `research-v1-baseline`
- Active gate: Gate 11, matched UP and SMP benchmark matrix, in progress
- Last completed action: corrected the two-vCPU no-ready-peer private-futex
  path with a Linux-permitted spurious wake and added a deterministic Rocky
  regression; static, architecture, focused futex, and 1/2/4-vCPU SMP tests pass
- Next action: commit the narrow compatibility fix, then resume the saved
  topology matrix from its first incomplete Ginux workload sample

## Current repository state

Local and remote `main` identify commit `1852455`. The first ten
feedback-hardening gates passed and closed under delegated review on
`codex/feedback-hardening`; Gate 11 adds measurement-only qualification to the
same branch and PR.
The ordinary QEMU path defaults to two vCPUs; the complete one/two/four-vCPU
matrix and repeated stress pass. The final qualification report and raw/reduced
evidence are under `docs/feedback-hardening/`. No push or merge was performed.

## Open decisions

- None. The human authorized a narrow syscall 202/FUTEX_WAIT_PRIVATE fix for
  the no-ready-peer state; unrelated scheduler work and performance tuning
  remain outside Gate 11.

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
Gate 10 evidence and final limits are in
[`docs/feedback-hardening/GATE-10-QUALIFICATION.md`](../docs/feedback-hardening/GATE-10-QUALIFICATION.md).
