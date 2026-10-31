# HDAE Status

## Recovery checkpoint

- Active skill: `.agents/skills/hdae-development/SKILL.md`
- Reload required after compaction, reset, resume, or handoff: yes
- Active project: profiling-guided UP and SMP performance parity
- Branch: `codex/performance-parity`
- Baseline tag: `research-v1-baseline`
- Active gate: Gate 12, approved and open
- Last completed action: completed the final matched matrix and full
  qualification suite; every performance cell meets the approved `0.96x`
  floor
- Next action: stop at the Gate 12 human checkpoint for result review and gate
  acceptance

## Current repository state

Local and remote `main` identify merged PR #7 at commit `a2a1eace`. The first
eleven feedback-hardening gates and inserted correctness gates are complete.
Gate 12 is isolated on `codex/performance-parity` for a separate follow-up PR.
The ordinary QEMU path defaults to two vCPUs; the complete one/two/four-vCPU
matrix and repeated stress pass. Gate 11's comparison and reduced evidence are
under `docs/feedback-hardening/`. No push or merge was performed.

## Open decisions

- Gate 12 is technically passed and awaits the required human review checkpoint
  before moving from `gates-todo/` to `gates-done/`.

## Active diagnosis

- Four-vCPU runtime work now dispatches independently to CPUs 1 through 3.
  A targeted nine-sample four-thread run produced a 2,924 events/s median,
  versus the matched Linux baseline of 1,254 events/s. The prior baseline was
  0.014x Linux.
- Coordinated fast futex waits and arbitrary-count wakes preserve an atomic
  wait-publication/wake boundary. Armed interval timers no longer disable safe
  fast syscalls before a signal is actually pending.
- Profiling found that glibc issued about 699,000 valid
  `FUTEX_WAIT_BITSET|FUTEX_CLOCK_REALTIME` calls during a four-thread memory
  sample. Ginux had returned `EINVAL` for every call. Full bitset operation,
  mask, and absolute-timeout handling now preserves Linux semantics; the
  coordinator uses Linux-permitted spurious wakeups while remote workers run.
- A focused three-sample four-vCPU run produced medians of 2,540 MiB/s for
  memory and 1.061 seconds for mutex, versus matched Linux medians of 2,496
  MiB/s and 1.096 seconds. Thread yield remained above the Linux baseline.
- The final exact-code matrix contains nine matched samples per cell. Every
  cell reaches at least `0.970x`; eighteen of twenty unique cells beat Linux.
  Fixed 2-vCPU CPU throughput is `0.982x`, and scaled 4-vCPU mutex is `0.970x`.
- `make check`, `make architecture-check`, `make test-smp`, `make
  test-rocky-smp-futex`, `make test-rocky-smp-shared-page`, and `make
  test-rocky-smp-thread-context` pass.

## Prior correctness diagnosis

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
