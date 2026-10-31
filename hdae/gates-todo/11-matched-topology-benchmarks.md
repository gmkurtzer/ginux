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
- [ ] Run the matched Linux and Ginux measurements. Every fixed one-thread
  cell passed at 1, 2, and 4 vCPUs. The scaled 2-vCPU/2-thread cell now passes
  on both systems. Scaled 4-vCPU/4-thread Linux passed; Ginux completed its
  first full batch, then faulted in the second batch's thread-yield workload.
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
- `make benchmark-topology-matrix` -> completed all fixed one-thread cells at
  1, 2, and 4 vCPUs with nine samples per workload and three measured boot
  samples. Scaled 2-vCPU/2-thread Linux completed; Ginux produced three CPU
  samples around 809--825 events/s, then faulted during the first memory sample
  on a locked write to mapped shared address `549771632656`.
- The failed scaled run terminated QEMU and left the benchmark volume unopened
  according to `lsof`; process enumeration was unavailable in the sandbox.
- After accepted Gate 11.5, the resumed scaled 2-vCPU/2-thread Ginux cell
  completed all nine samples per workload. The scaled 4-vCPU/4-thread Linux
  cell completed, and Ginux completed its first full batch plus the second
  batch's CPU and memory samples. The second batch then faulted during
  `threads_1` at RIP `4283650` on `lock inc [r11]`, with CR2 `66050`,
  page-fault error 7, and no page-table or process-registry mapping for page
  `65536`.

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
- The scaled shared-memory fault is not a performance result. It is a separate
  SMP shared-writable-page correctness gap assigned to Gate 11.5; fixed-thread
  measurements remain valid. Gate 11.5 passed and was accepted on October 30,
  2026, so the scaled matrix resumes with the corrected implementation.
- The later four-thread low-address fault is also not a performance result. It
  occurs after shared-memory coverage passes and was assigned to Gate 11.6 for
  thread-context and scheduling diagnosis. Gate 11.6 passed and was accepted
  on October 30, 2026, so the scaled matrix resumes with exact cross-thread
  register restoration.

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
