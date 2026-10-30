# Gate 11: Matched UP and SMP Benchmark Matrix

## Goal

Measure Rocky userspace on Linux and Ginux under matched one-, two-, and
four-vCPU QEMU configurations, separating fixed single-thread behavior from
thread counts scaled with available CPUs.

## Approved scope

- Run Rocky-on-Linux and Rocky-on-Ginux with one, two, and four vCPUs.
- Measure boot, CPU, memory, thread-yield, and mutex workloads.
- Run a fixed one-thread series at every topology and a scaled series with
  workload threads equal to vCPU count.
- Use the same QEMU machine, CPU model, guest memory, sysbench binary,
  workload duration, and sample count for both kernels.
- Extend the benchmark harness only where required to run and summarize the
  matrix reproducibly.
- Correct the narrowly discovered `FUTEX_WAIT_PRIVATE` scheduler behavior when
  an SMP CPU has no ready peer, with deterministic regression coverage.
- Preserve raw logs, reduced tables, environment details, and QEMU cleanup
  evidence.

## Non-goals

- Performance tuning or implementation changes based on the results.
- Unrelated futex operations or scheduler redesign.
- Claims of native-hardware, production, or general performance parity.
- Treating the Linux and Ginux boot paths as architecturally identical.
- Storage or network throughput benchmarking.

## Completed work

- [x] Freeze the matrix, controls, repetitions, and output schema.
- [x] Add reproducible matrix orchestration and summary generation.
- [x] Correct the two-vCPU no-ready-peer private-futex result and add a
  deterministic Rocky regression.
- [ ] Run the matched Linux and Ginux measurements. The 1-vCPU/1-thread cell
  passed; Linux and boot measurements passed at 2-vCPU/1-thread, but Ginux
  sysbench exposed an unsupported futex wait state before producing samples.
- [ ] Verify raw samples, binary identity, results, and QEMU cleanup.
- [ ] Publish the comparison and its limitations.

## Remaining work

- [ ] Complete every approved task and acceptance criterion.

## Acceptance criteria

- Linux and Ginux each produce results for 1, 2, and 4 vCPUs.
- Fixed-mode workloads use one thread at all three CPU counts.
- Scaled-mode workloads use 1, 2, and 4 threads respectively.
- Each sysbench cell contains nine successful samples; each boot cell contains
  three successful samples after any unmeasured warm-up.
- Both systems use the same pinned x86-64 sysbench binary and matched QEMU
  machine, CPU model, memory, workload duration, and topology.
- Results report medians for both systems, the Ginux/Linux ratio, and scaling
  relative to each system's own one-vCPU result.
- Every launched QEMU process is reaped and no tested volume remains open.
- Existing static, architecture, and SMP regression checks still pass after
  harness changes.
- A deterministic two-vCPU Linux regression proves that a private futex wait
  with no immediately ready peer returns a Linux-compatible result instead of
  `ENOSYS` or a hang.

## Verification plan

- `make benchmark-topology-matrix` -> complete all matched measurements and
  write raw logs plus reduced TSV summaries.
- `make benchmark-topology-compare` -> validate controls and sample counts,
  then print the final comparison tables.
- `make check && make architecture-check && make test-smp` -> preserve prior
  build, architecture, and 1/2/4-vCPU behavior.
- `make test-rocky-smp-futex` -> a one-thread Rocky sysbench workload under two
  vCPUs completes without an unexpected futex error, kernel panic, or hang.
- QEMU process and tested-volume inspection after the matrix -> no orphaned
  process or open benchmark volume.

## Evidence

- `make check && make architecture-check` -> passed before measurement.
- Fixed 1-vCPU/1-thread -> nine successful samples per workload for both
  systems and three successful boot samples per system.
- Fixed 2-vCPU/1-thread Linux -> nine successful samples per workload and
  three successful boot samples.
- Fixed 2-vCPU/1-thread Ginux boot -> three successful boot samples.
- Fixed 2-vCPU/1-thread Ginux sysbench -> failed before its first CPU sample:
  futex syscall 202 with operation 128 reached the no-ready-peer path and
  sysbench reported `The futex facility returned an unexpected error code.`
- Interrupting the failed matrix left no QEMU process and no open benchmark
  volume.
- `make test-rocky-smp-futex` -> passed after the narrow compatibility fix; the
  one-thread CPU workload completed under two vCPUs.
- `make check && make architecture-check && make test-smp` -> passed after the
  fix, including 1/2/4-vCPU qualification and two repeated two-vCPU stress runs.

## Decisions and limitations

- The 1-vCPU/1-thread cell is shared by the fixed and scaled series rather than
  rerun under two labels.
- Boot is reported at all CPU counts, but interpreted separately because Linux
  and Ginux follow different firmware, kernel, image, and userspace paths.
- QEMU TCG on the macOS development host is the experiment environment; the
  results do not predict native hardware performance.
- Completing the fixed-thread SMP measurements requires either a narrowly
  authorized futex/scheduler compatibility fix or recording those Ginux cells
  as unsupported. The human authorized the narrow compatibility fix on
  October 30, 2026; performance tuning remains outside this gate.
- Linux explicitly permits spurious futex wakeups. When CPU 0 has no local
  ready peer but an SMP worker is active, Ginux returns a successful spurious
  wake so libc rechecks the userspace futex word on the intact SYSRET path.

## Delivery strategy

- Working branch: `codex/feedback-hardening`
- Intended base: `main`
- Pull request: existing PR #7
- Planned commits: benchmark harness and controls; batched-sample lifecycle;
  futex no-peer regression and compatibility fix; measured evidence and gate
  closure
- Actual commits: `b7ab9b14` benchmark matrix; `cedea0ae` batched lifecycle;
  `31de6894` blocker checkpoint

## Review policy and outcome

Delegated completion was approved with the measurement request and direction
to add the work to the existing pull request. Close automatically only after
all objective criteria pass; stop for any changed benchmark meaning, unmatched
control, implementation tuning, or scope expansion.
