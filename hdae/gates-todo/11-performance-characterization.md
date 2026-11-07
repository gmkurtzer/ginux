# Performance characterization and optimization

## Source

Approved requirement that Ginux quantify and minimize overhead without
weakening mediation.

## Goal

Produce reproducible native, conventional-container, and Ginux comparisons,
make native Rocky execution the primary control, and optimize Ginux until
mandatory workloads reach native parity or remain within the explicitly
approved tolerance while preserving every security invariant and compatibility
criterion.

## Approved scope

- Define matched CPU, memory, syscall, process, filesystem, block, network,
  systemd boot, and idle workloads using the same Rocky userspace where
  practical.
- Run the same `sysbench` binary with identical arguments for CPU, memory,
  threads, and mutex workloads directly on the Rocky host and inside a Ginux
  instance. Include matched 1-, 2-, and 4-vCPU runs where the workload supports
  parallelism; retain single-thread results at every CPU count so scaling and
  fixed mediation costs remain distinguishable.
- Run both sides inside the same Rocky QEMU host session, with matched CPU
  affinity, memory limits, filesystem state, warmup, and background load, so
  the comparison measures Ginux overhead rather than different virtual
  machines or host conditions.
- Record hardware, host kernel, Ginux commit, image identity, configuration,
  contract, repetitions, warmup, variance, and raw measurements.
- Use native execution as 1.00x. Success is parity with or better than native.
  The initial mandatory tolerance is no more than 4% slower than native for
  throughput or 4% higher than native for latency and elapsed time. A result
  outside that tolerance fails the workload and requires profiling and an
  optimization attempt; it cannot be averaged away by faster unrelated tests.
- Permit a workload-specific exception only after human review of the raw
  baseline, the unavoidable mediation cost, and the proposed numeric budget.
  Record the exception in this gate before using it; never silently loosen a
  threshold after observing a failure.
- Profile Ginux and optimize the highest-impact in-scope paths.
- Add benchmark comparison and report generation suitable for regression use.
- Preserve the approved native result set as a versioned baseline. Subsequent
  benchmark runs must flag both native-to-Ginux budget failures and regressions
  against the last accepted Ginux result, with environment incompatibility
  reported separately from a performance change.
- Rerun functional, security-boundary, and systemd suites after optimizations.

## Non-goals

- Claiming universal near-native performance.
- Comparing unmatched workloads or hiding unfavorable results.
- Bypassing interception, validation, virtual objects, audit requirements, or
  isolation to improve a benchmark.
- Production-scale capacity, multi-instance density, or hardware acceleration not
  already in scope.

## Completed work

- [x] Human accepted Gate 10.5 and authorized Gate 11 execution.
- [x] Created dedicated branch `hdae/performance-characterization` from merged
  `main` for one Gate 11 pull request.
- [x] Pinned Rocky/EPEL `sysbench-1.0.20-19.el10_2.x86_64`, staged the exact
  executable and resolved Rocky libraries into the instance, and verified
  matching SHA-256
  `7cf6b71521436bd32a8ab197851035ff1632bee383fec5a35468d61969e7c18d`.
- [x] Proved the controller runtime executes an arbitrary configured one-shot
  workload rather than the legacy native-shell fixture; a one-vCPU CPU probe
  completed under Ginux. This is setup evidence, not baseline evidence.
- [x] Defined the initial matched protocol: one pinned executable, identical
  arguments, one-GiB address-space limit, matched affinity, one warmup, three
  alternating samples, and independent 1/2/4-vCPU fixed-thread and scaling
  rows inside one four-vCPU Rocky QEMU session.
- [x] Collected the unoptimized baseline checkpoint. CPU and memory passed all
  rows; every thread and mutex row failed the default 4% budget. Optimization
  was paused for human review of this checkpoint.
- [x] Human approved the unoptimized baseline on 2026-11-07 and directed
  profiling and optimization toward the unchanged native-parity goal and 4%
  per-row mandatory tolerance.
- [x] Corrected the mutex protocol to use the pinned sysbench 1.0.20 defaults
  after proving the original approximately 15 ms run measured startup cost.
  The corrected 1/2/4-vCPU exploratory rows all satisfy the 4% budget.
- [x] Profiled thread-yield mediation. Syscall 24 (`sched_yield`) accounted for
  3,801 of the first 4,097 entry records and exhausted the 8,192-record audit
  budget in the one-vCPU run.
- [x] Measured bounded diagnostic upper limits: disabling audit improved the
  one-vCPU row from about 3.3 to 5.3 events/s, while a temporary one-stop,
  no-op `sched_yield` experiment reached only 5.8 events/s versus about 635
  native. The semantically invalid experiment was removed.
- [x] Added `sched_yield` to the versioned contract catalog and replaced linear
  syscall-rule lookup with a constant-time index.
- [x] Replaced two-stop ptrace handling for `sched_yield` with a seccomp user
  notification broker. Every call still reaches Ginux before completion; the
  broker rechecks the immutable contract, maps the host TID to its virtual PID,
  emits bounded audit, and returns the mediated result without an extra host
  yield after notification has already descheduled the caller.
- [x] Made supervisor and broker audit records cross-process safe. A shared,
  futex-backed mutex serializes complete records without spinning or issuing a
  wake syscall on the uncontended path. The full audit corpus contains no
  interleaved or malformed records.
- [x] Exhausted the credible userspace hot-path reductions: entry-only ptrace,
  seccomp user notification, audit disablement, contract-decision indexing,
  broker/tracee CPU separation, same-CPU placement, removal of the redundant
  host yield, and fixed-semantics kernel-resident diagnostics.

## Remaining work

- [x] Finalize the initial matched benchmark protocol and default 4% budget
  from prior evidence. Final-duration confirmation remains part of completion.
- [x] Package or install one pinned `sysbench` build for use by both the native
  control and Ginux instance, and verify its identity before every comparison.
- [ ] Implement comparison and report automation.
- [x] Collect the initial repeated baseline results used to select profiling
  targets. Final accepted results require a longer confirmation run.
- [x] Profile and optimize the thread-yield hot path through the limit of the
  approved userspace-dispatch architecture. The mandatory row still fails and
  requires an architecture decision; other Gate 11 workloads remain open.
- [ ] Persist the accepted baseline and add automatic per-workload regression
  checks to the normal performance test target.
- [ ] Run full regression and document remaining costs.

## Acceptance criteria

- Each benchmark compares semantically matched work and clearly labels any
  unavoidable difference.
- Native Rocky and Ginux use the same executable, workload parameters, input
  data, CPU count, affinity, memory limit, warmup, and host session. The report
  shows the native value, Ginux value, ratio, percentage difference, variance,
  and pass/fail result for every row.
- Mandatory `sysbench` CPU, memory, threads, and mutex comparisons cover 1, 2,
  and 4 vCPUs where applicable. A single-thread control is retained at each
  allocation, and no aggregate score can conceal a failing row.
- Raw results and the generated report contain enough environment and protocol
  metadata for another operator to reproduce the run.
- Success is native parity or better. Unless a workload-specific exception is
  approved and recorded in advance, each mandatory row must be within 4% of
  native in the unfavorable direction.
- Final results satisfy all approved mandatory budgets, or the gate is reported
  failed or partial rather than weakening a threshold silently.
- A deliberately perturbed fixture proves that `make bench-compare` exits
  nonzero when either the native budget or accepted Ginux regression budget is
  exceeded.
- Optimizations preserve all architecture invariants and every mandatory
  functional and security regression.
- The report identifies fixed overhead, workload-sensitive overhead, variance,
  known bottlenecks, and unsupported conclusions.
- `make demo-gate-11` lets a human run a bounded comparison and inspect raw
  matched results, budget decisions, and the generated report.

## Verification plan

- `make bench-baseline` -> emits matched native and conventional-container
  control records.
- `make bench` -> emits Ginux records for all mandatory workloads.
- `make bench-compare` -> checks result compatibility and approved numeric
  budgets, returning nonzero for any mandatory row more than 4% behind native
  or beyond its separately approved workload budget, and for material
  regressions from the accepted Ginux baseline.
- `make bench-report` -> generates the versioned summary from raw results.
- Sysbench matrix -> runs the pinned binary and identical CPU, memory, threads,
  and mutex commands natively and through Ginux at matched CPU allocations;
  the report contains one directly comparable row per workload and CPU count.
- `make bench-block` and `make bench-network` -> device-path measurements are
  included in comparison evidence.
- `make test test-systemd test-contract test-security` -> all available
  mandatory regressions exit zero after optimization.
- `make demo-gate-11` -> performs the documented bounded benchmark comparison,
  validates approved budgets, generates the report, and exits zero.

## Evidence

- `make bench-prepare` acquired and cached the exact Rocky/EPEL sysbench build
  and digest-pinned Rocky instance image in a dedicated persistent benchmark
  host overlay. `make bench-baseline` then completed offline.
- `build/gate11/results.tsv` contains three raw samples per side and row;
  `build/gate11/raw.tar.gz` contains the corresponding sysbench transcripts;
  `build/gate11/metadata.txt` records the matched environment and protocol;
  and `build/gate11/report.md` is generated from those inputs.
- All five CPU rows passed between 0.994x and 1.012x native performance. All
  five memory rows passed between 0.995x and 1.000x.
- All five thread-yield rows failed between 0.005x and 0.011x native
  performance. All five mutex rows failed between 0.703x and 0.842x.
- This is the required pre-optimization checkpoint. No optimization has been
  applied to these results. The human accepted this checkpoint on 2026-11-07
  as the profiling baseline without approving any tolerance exception.
- A corrected one-repetition exploratory mutex matrix passed every row: 0.994x
  at 1 vCPU/1 thread, 0.994x at 2 vCPUs/1 thread, 1.006x at 2/2, 0.997x at
  4/1, and 1.006x at 4/4. Longer repeated confirmation remains required.
- Focused one-vCPU thread-yield profiling measured 3.299 events/s through the
  normal audited path versus 651.154 native. The audit contained 8,192 bounded
  records (1.6 MiB); 3,801 of 4,097 recorded entries were syscall 24.
- Diagnostic-only audit disablement measured 5.255 events/s versus 635.919
  native. A temporary one-stop `PTRACE_SYSEMU` no-op upper-bound experiment
  measured 5.802 versus 635.428 native. It intentionally omitted real yield
  semantics and was removed immediately after measurement.
- `strace -c` on the original path attributed 62.55% of supervisor time to
  3,649 `wait4` calls and 30.99% to 8,257 `ptrace` calls: 93.54% combined.
  Seccomp-assisted entry-only ptrace reached 14.831 events/s before the user
  notification broker replaced it.
- The clean-tree repeated audited 1/2/4-vCPU matrix in
  `build/gate11-final-clean/report.md` reached 19.731, 19.619, 42.329,
  19.526, and 75.861 events/s. That is 0.031x to 0.036x native and remains a
  mandatory failure, but the one-vCPU result is 5.98x the original audited
  3.299 events/s. Metadata identifies committed source `6de49677` and a clean
  working tree.
- A three-second, three-repetition audit-disabled diagnostic measured 21.789
  versus 645.097 native events/s. One sysbench event contains 1,000 yields, so
  the remaining mediated path costs about 45.9 microseconds per yield versus
  about 1.55 microseconds natively; audit accounts for only about 5% of the
  final audited result.
- A temporary kernel-resident seccomp `ALLOW` diagnostic preserved the host
  `sched_yield` behavior and measured 606.222 versus 647.075 events/s
  (0.937x, 6.31% slower). It bypassed the required userspace dispatch and was
  removed. A separate compiled return-success/no-yield diagnostic reached
  1,859.211 versus 621.813 events/s but was rejected and removed because it
  did not perform the requested yield.
- Fresh regression evidence after the retained optimization passes
  `make check`, `make test-contract`, `make test-native-vfs`, and the complete
  cold/warm/repeat/DNF-failure/startup-failure/shutdown systemd suite.
- The clean matrix passed every CPU row at 0.967x to 0.978x and every mutex row
  at 0.995x to 1.002x. One short 2-vCPU/1-thread memory row had 10.39% Ginux
  variance and failed; a longer five-second, five-repetition confirmation in
  `build/gate11-memory-v2t1-confirm/report.md` passed at 1.003x native with
  1.52% Ginux variance. Its 2-thread companion also passed at 1.003x.

## Decisions and limitations

- Human direction on 2026-11-06 established a default 4% mandatory tolerance
  and native parity as the optimization goal. Workload-specific exceptions
  remain unresolved until matched baseline evidence exists; human approval is
  required before any exception can replace that default.
- CPU-bound overhead is expected to differ materially from syscall-, process-,
  storage-, and network-heavy overhead; results remain per workload.
- Native means an ordinary process running directly on the Rocky host kernel,
  not a second VM. The conventional-container result provides context but does
  not replace the native control or relax the Ginux threshold.
- The initial checkpoint uses one-second sysbench intervals and three recorded
  repetitions after warmup to keep the exploratory matrix bounded. The final
  accepted baseline will use longer intervals and/or more repetitions after
  profiling stabilizes the failing paths.
- The focused evidence shows that audit is a secondary cost and the current
  ptrace implementation is the dominant thread-yield cost. The temporary
  one-stop/no-op experiment did not establish that all optimized one-stop
  implementations are exhausted: it retained the existing wait/register and
  supervisor path and intentionally did not execute real yield semantics.
- Human clarification on 2026-11-07 requires continued code-level profiling
  and optimization until every credible in-scope implementation improvement is
  exhausted. Gate 11 must not stop merely because the current tracing design
  misses the budget. If the final implementation still cannot reach the goal,
  alternatives must be supported by measured evidence from the exhausted
  optimization paths.
- Linux seccomp user notification exposes one receive ioctl and one response
  ioctl per decision; it has no batch response operation. The measured
  audit-disabled result therefore represents the practical ceiling for the
  approved per-call userspace decision path, not a formatting or lookup
  bottleneck.
- Reaching the 4% thread-yield budget now requires one of three explicit
  alternatives: permit a kernel-resident compiled decision for selected
  fixed-semantics calls (and design corresponding audit), move decision/audit
  into an in-kernel eBPF/LSM or module component, or retain userspace mediation
  with a human-approved workload-specific budget. Native x86-64/KVM should
  also be measured before setting a durable budget because the current macOS
  arm64 QEMU TCG environment amplifies context-switch and seccomp costs. None
  of these architecture or budget changes is approved by this gate.

## Delivery strategy

- Working branch: `hdae/performance-characterization`
- Intended base: `main`
- Pull request: intended; dedicated branch push and PR creation authorized on
  2026-11-03; never merge or close without explicit authorization
- Planned commits: matched benchmark protocols and budget record; comparison
  and report tooling; targeted optimizations; final regression and evidence
- Actual commits: pending

## Review policy and outcome

Human checkpoint before approving numeric budgets and again at completion. No
performance claim is accepted solely from an aggregate score. Present
`make demo-gate-11`, raw evidence, and the pull request before final acceptance.
