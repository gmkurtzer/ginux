# Gate 12: UP and SMP Performance Parity

## Goal

Make Rocky-on-Ginux meet or beat matched Rocky-on-Linux performance across the
complete fixed-thread and scaled-thread benchmark matrix. A result within four
percent of Linux is acceptable; the target is faster than Linux in every cell.

## Approved scope

- Reproduce and freeze fresh merged-`main` Linux and Ginux baselines.
- Profile scheduler, yield, futex, context-switch, timer, IPI, memory, and
  related kernel paths with bounded diagnostics that are disabled by default.
- Optimize general kernel mechanisms in order of measured impact.
- Run targeted benchmark cells during iteration and the complete matrix for
  final acceptance.
- Add deterministic correctness or performance regressions for mechanisms
  changed by the gate.
- Publish before/after results, profiling evidence, implementation rationale,
  and remaining limitations.

## Non-goals

- Sysbench-specific kernel behavior, benchmark detection, or workload changes.
- Different QEMU resources, binaries, sample counts, or durations for Linux and
  Ginux.
- Native-hardware or broad production-performance claims.
- Treating boot timing as a kernel-speed comparison or a passage criterion.
- Unrelated feature, compatibility, UI, storage, or networking work.
- Weakening correctness, security, isolation, or prior gate acceptance.

## Completed work

- [x] Confirm PR #7 merged cleanly into `main`.
- [x] Create `codex/performance-parity` from merged `main`.
- [x] Align the goal, scope, acceptance threshold, review policy, and delivery
  strategy with the human.
- [x] Reproduce and preserve a fresh complete baseline.
- [x] Add bounded child-exit profiling and attribute the first dominant costs.
- [x] Generalize remote Linux runtime-thread dispatch across every online
  application CPU.
- [x] Add coordinated futex wait/wake fast paths and preserve prior SMP
  correctness regressions.

## Remaining work

- [ ] Optimize measured general kernel mechanisms.
- [ ] Add regression coverage for changed mechanisms.
- [ ] Re-run targeted cells until stable.
- [ ] Run and validate the complete matched matrix.
- [ ] Publish final evidence and stop for human review.

## Acceptance criteria

- Fixed-mode workloads use one thread at one, two, and four vCPUs.
- Scaled-mode workloads use one, two, and four threads at the corresponding
  vCPU counts.
- Every CPU, memory, thread-yield, and mutex cell contains nine successful
  Linux samples and nine successful Ginux samples.
- Every favorable-direction Ginux/Linux median ratio is at least 0.96x. The
  optimization target is greater than 1.00x in every cell.
- Both systems use the identical pinned x86-64 sysbench binary, Q35 machine,
  CPU model, 2 GiB memory, workload duration, and topology.
- No benchmark-specific shortcut, detected workload behavior, or asymmetric
  resource change exists.
- `make check`, `make architecture-check`, `make test-smp`,
  `make test-rocky-smp-futex`, `make test-rocky-smp-shared-page`, and
  `make test-rocky-smp-thread-context` pass after implementation.
- Every QEMU process is reaped and no tested benchmark volume remains open.
- Committed evidence reports fresh before/after medians, ratios, profiling
  attribution, changes, and limitations.

## Verification plan

- `make benchmark-topology-matrix` from an isolated baseline directory -> nine
  samples per sysbench cell and three observational boot samples per topology.
- `make benchmark-topology-compare` -> reject incomplete or unmatched cells and
  emit the baseline comparison.
- Targeted forced runs for each changed hot path -> show reproducible movement
  before the next implementation cycle.
- Final forced `make benchmark-topology-matrix` followed by
  `make benchmark-topology-compare` -> every favorable ratio is at least 0.96x
  with the target greater than 1.00x.
- `make check && make architecture-check && make test-smp && make
  test-rocky-smp-futex && make test-rocky-smp-shared-page && make
  test-rocky-smp-thread-context` -> all commands exit successfully.
- Process and exact-volume inspection after runs -> no QEMU process or open
  tested volume remains.

## Evidence

- Historical Gate 11 reduced results are under
  `docs/feedback-hardening/gate-11-results/`.
- `make benchmark-topology-matrix` with forced isolated output completed every
  baseline cell; reduced results and initial attribution are recorded in
  `docs/feedback-hardening/GATE-12-BASELINE.md`.
- Fixed 2-vCPU profiling measured about 1.27 million calls per four-second
  sample, split almost evenly between `futex` and `sched_yield`. The current
  successful-spurious-wake path explains the extra futex loop.
- Source inspection confirmed only one remote Linux runtime-thread job exists
  and it is always dispatched to CPU 1, matching the 4-vCPU scaling collapse.
- A targeted nine-sample 4-vCPU/4-thread run reached a 2,924 events/s median,
  2.33x the matched Linux median of 1,254 events/s. The focused futex,
  shared-page, and exact-thread-context SMP regression commands all pass.
- The targeted 4-vCPU/4-thread mutex median remains about 1.46 seconds versus
  Linux at 1.096 seconds, so the gate remains open.

## Decisions and limitations

- Passage uses a 0.96x floor because the human defined two-to-four percent as
  acceptable. Optimization continues toward greater than 1.00x while credible
  in-scope improvements remain.
- Boot remains observational because the compared startup paths differ.
- The agent may iterate autonomously but must stop for a major architecture
  decision, benchmark or correctness weakening, material scope expansion, or a
  disproportional dead end.

## Delivery strategy

- Working branch: `codex/performance-parity`
- Intended base: `main` after merged PR #7
- Pull request: new follow-up PR
- Planned commits: gate and baseline; bounded profiling; one or more coherent
  mechanism optimizations with regressions; final matched evidence and closure
- Actual commits: pending

## Review policy and outcome

Human checkpoint after all objective criteria pass. Autonomous implementation
and verification inside the approved gate are authorized; the gate remains in
`gates-todo/` until the human accepts it.
