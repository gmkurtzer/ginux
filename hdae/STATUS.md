# HDAE Status

## Recovery checkpoint

- Active skill: `.agents/skills/hdae-development/SKILL.md`
- Reload required after compaction, reset, resume, or handoff: yes
- Active project: feedback-driven kernel hardening
- Branch: `codex/feedback-hardening`
- Baseline tag: `research-v1-baseline`
- Active gate: proposed Gate 11.6, SMP Linux thread-context integrity, awaiting
  human alignment and execution approval
- Last completed action: Gate 11 resumed; scaled 2-vCPU/2-thread completed, but
  scaled 4-vCPU/4-thread batch 2 faulted in the thread-yield workload with an
  unmapped low atomic-counter address after batch 1 passed completely
- Next action: review Gate 11.6 scope and checkpoint policy; Gate 11 remains
  paused and no implementation has started

## Current repository state

Local and remote `main` identify commit `1852455`. The first ten
feedback-hardening gates passed and closed under delegated review on
`codex/feedback-hardening`; Gate 11 adds measurement-only qualification to the
same branch and PR.
The ordinary QEMU path defaults to two vCPUs; the complete one/two/four-vCPU
matrix and repeated stress pass. The final qualification report and raw/reduced
evidence are under `docs/feedback-hardening/`. No push or merge was performed.

## Required decision

- Approve, revise, or reject proposed Gate 11.6 on the current branch and PR.

## Active diagnosis

- The pre-fix two-vCPU/two-thread CPU-to-memory sequence deterministically
  reproduced a locked-write non-present fault while the serialized active-root
  walk reported a present writable leaf.
- Mapping mutations now complete a serialized NMI shootdown on every online
  CPU and fail closed with leaf rollback if acknowledgement fails. A local
  fault refresh remains as permission-preserving defense in depth.
- Three fresh 2-vCPU/2-thread and three fresh 4-vCPU/4-thread shared-memory
  workloads passed, as did `make check`, `make architecture-check`, `make
  test-smp`, and `make test-rocky-smp-futex`. No QEMU process or benchmark
  volume owner remained.

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
