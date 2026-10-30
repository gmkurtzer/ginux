# HDAE Status

## Recovery checkpoint

- Active skill: `.agents/skills/hdae-development/SKILL.md`
- Reload required after compaction, reset, resume, or handoff: yes
- Active project: feedback-driven kernel hardening
- Branch: `codex/feedback-hardening`
- Baseline tag: `research-v1-baseline`
- Active gate: Gate 11.5, SMP shared-writable-page correctness, in progress
- Last completed action: completed every fixed one-thread cell at 1, 2, and 4
  vCPUs; scaled 2-vCPU/2-thread CPU samples passed, but its shared-memory
  workload produced a Linux write page fault on a mapped shared page
- Next action: implement a permission-preserving local translation refresh for
  non-present Linux faults whose authoritative page-table leaf is already
  present and grants the attempted access, then rerun the focused stress

## Current repository state

Local and remote `main` identify commit `1852455`. The first ten
feedback-hardening gates passed and closed under delegated review on
`codex/feedback-hardening`; Gate 11 adds measurement-only qualification to the
same branch and PR.
The ordinary QEMU path defaults to two vCPUs; the complete one/two/four-vCPU
matrix and repeated stress pass. The final qualification report and raw/reduced
evidence are under `docs/feedback-hardening/`. No push or merge was performed.

## Open decisions

- None. The human approved Gate 11.5 on October 30, 2026. Stop for review after
  every objective criterion passes; Gate 11 remains paused meanwhile.

## Active diagnosis

- A fresh two-vCPU/two-thread run with one three-second CPU sample followed by
  one three-second shared-memory sample deterministically reproduced the locked
  write fault on its first boot.
- The hardware reported a non-present write fault while a serialized walk of
  the active root found an occupied, framed, writable leaf. The narrow current
  diagnosis is a CPU-local stale non-present translation after another runtime
  CPU publishes the shared page; the planned recovery invalidates only the
  faulting CPU and only when the existing leaf already grants the exact access.

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
