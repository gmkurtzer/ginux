# HDAE Status

## Recovery checkpoint

- Active skill: `.agents/skills/hdae-development/SKILL.md`
- Reload required after compaction, reset, resume, or handoff: yes
- Active project: profiling-guided UP and SMP performance parity
- Branch: `codex/performance-parity`
- Baseline tag: `research-v1-baseline`
- Active gate: Gate 12, approved and open
- Last completed action: PR #7 merged into `main`; a clean follow-up branch was
  created and the Gate 12 scope and acceptance boundary were approved
- Next action: reproduce the merged baseline, add bounded profiling, and
  attribute the largest scheduler, futex, context-switch, timer, IPI, or memory
  costs before optimizing them

## Current repository state

Local and remote `main` identify merged PR #7 at commit `a2a1eace`. The first
eleven feedback-hardening gates and inserted correctness gates are complete.
Gate 12 is isolated on `codex/performance-parity` for a separate follow-up PR.
The ordinary QEMU path defaults to two vCPUs; the complete one/two/four-vCPU
matrix and repeated stress pass. Gate 11's comparison and reduced evidence are
under `docs/feedback-hardening/`. No push or merge was performed.

## Open decisions

- None. The human approved autonomous Gate 12 iteration with a final review
  checkpoint. Stop early only for a major architecture choice, benchmark or
  correctness weakening, scope expansion, or a disproportional dead end.

## Active diagnosis

- Fault diagnostics proved the saved thread slot retained the valid shared
  counter in `r11`, while a syscall-based switch resumed it with RFLAGS in
  `r11`. The SYSRET epilogue was correct for an ordinary syscall but incorrect
  for a target asynchronously saved by an interrupt.
- Scheduler-backed cross-thread syscall switches now use the exact-register
  IRET epilogue; ordinary same-thread syscalls retain SYSRET.
- `make check`, `make test-smp`, `make test-rocky-smp-futex`, `make
  test-rocky-smp-shared-page`, and `make test-rocky-smp-thread-context` passed.
  The last command completed nine samples at each of 2 and 4 vCPUs. No tested
  volume owner remained.
- The exact original 4-vCPU/4-thread `cpu memory threads` sequence passed three
  batches and 27 total workload markers, including the formerly failing second
  batch.
- The full matched topology matrix then completed. Every sysbench cell contains
  nine Linux and nine Ginux samples with the same binary hash; every boot cell
  contains three samples. Four-thread Ginux scheduling remains a measured
  performance limitation rather than a correctness failure.

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
Gate 11 evidence and final limits are in
[`docs/feedback-hardening/GATE-11-TOPOLOGY-BENCHMARKS.md`](../docs/feedback-hardening/GATE-11-TOPOLOGY-BENCHMARKS.md).
