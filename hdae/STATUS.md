# HDAE Status

## Recovery checkpoint

- Active skill: `.agents/skills/hdae-development/SKILL.md`
- Reload required after compaction, reset, resume, or handoff: yes
- Active project: feedback-driven kernel hardening
- Branch: `codex/feedback-hardening`
- Baseline tag: `research-v1-baseline`
- Active gate: Gate 11, matched UP and SMP benchmark matrix, blocked on a
  human scope decision
- Last completed action: completed the matched 1-vCPU/1-thread cell and the
  Linux plus boot portions of 2-vCPU/1-thread; the Ginux workload exposed a
  futex wait compatibility failure when no scheduler peer was ready
- Next action: decide whether Gate 11 may include a narrow futex/scheduler
  compatibility fix or must report fixed-thread SMP cells as unsupported

## Current repository state

Local and remote `main` identify commit `1852455`. The first ten
feedback-hardening gates passed and closed under delegated review on
`codex/feedback-hardening`; Gate 11 adds measurement-only qualification to the
same branch and PR.
The ordinary QEMU path defaults to two vCPUs; the complete one/two/four-vCPU
matrix and repeated stress pass. The final qualification report and raw/reduced
evidence are under `docs/feedback-hardening/`. No push or merge was performed.

## Open decisions

- Gate 11 is measurement-only as approved. Completing its fixed-thread SMP
  cells requires a compatibility change: syscall 202/FUTEX_WAIT_PRIVATE must
  block correctly when the current CPU has no ready scheduler peer. Human
  authorization is required before expanding the gate to that implementation
  work.

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
