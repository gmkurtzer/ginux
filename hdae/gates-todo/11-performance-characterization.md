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
- Add an instance-configured syscall dispatch mode. `paranoid` preserves a
  pre-effect Ginux userspace decision for every syscall. `selective` permits
  direct host execution only when the effective contract allows an operation
  and Ginux's versioned catalog marks it transparent and direct-safe.
- Default omitted dispatch configuration to `selective` so the normal path is
  lightweight. Keep `paranoid` as an explicit operator choice.
- In both modes, route denials and operations requiring inspection,
  translation, emulation, virtual state, pointer or descriptor validation, or
  modified results through Ginux. A denied operation returns its configured
  Linux error without invoking the requested host syscall.
- Add benchmark comparison and report generation suitable for regression use.
- Preserve the approved native result set as a versioned baseline. Subsequent
  benchmark runs must flag both native-to-Ginux budget failures and regressions
  against the last accepted Ginux result, with environment incompatibility
  reported separately from a performance change.
- Rerun functional, security-boundary, and systemd suites after optimizations.

## Non-goals

- Claiming universal near-native performance.
- Comparing unmatched workloads or hiding unfavorable results.
- Inferring direct execution from an allow rule without an independent
  direct-safety classification, or bypassing required validation, virtual
  objects, denial audit, or isolation to improve a benchmark.
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
- [x] Human approved the configurable dispatch architecture on 2026-11-07:
  paranoid mode mediates every syscall; selective mode directly executes only
  contract-allowed, cataloged direct-safe operations; denial and complex
  semantics remain Ginux userspace responsibilities.
- [x] Implemented strict per-instance `paranoid` and `selective` dispatch.
  Omitted configuration initially defaulted to paranoid and unknown values
  fail closed.
- [x] Marked only `sched_yield` direct-safe in contract catalog 2. Selective
  filters require both effective permission and that independent catalog bit;
  every other operation remains traced through Ginux.
- [x] Changed the selective seccomp program to the Linux allow-cache-compatible
  instruction subset. The supervisor reads mediated syscall numbers from the
  saved registers, preserving architecture validation and denial behavior while
  allowing the kernel to cache direct-safe decisions.
- [x] Added portable native-budget, accepted-baseline regression, environment
  compatibility, deliberately perturbed fixture, and human-demo automation.
- [x] Stabilized the process identity regression after the faster selective
  path exposed a polling race. The test now freezes the supervisor after its
  tracee appears and inspects the ptrace-stopped tracee deterministically.
- [x] Human direction on 2026-11-07 changed omitted dispatch configuration to
  selective for the lightest normal path. Paranoid remains explicit and its
  denial/mediation regression fixtures remain mandatory.
- [x] Removed the unused seccomp user-notification listener from selective
  instances. Paranoid instances retain the listener required for their
  userspace `sched_yield` decisions.
- [x] Added optional per-run benchmark audit capture so syscall-path profiles
  remain tied to the exact native/Ginux sample that produced them.
- [x] Established a matched paranoid-dispatch benchmark matrix and profiled its
  userspace notification, audit, contract-decision, PID-mapping, and scheduling
  costs without permitting any syscall to complete before a Ginux decision.
- [x] Re-evaluated each retained selective-mode optimization for paranoid mode,
  retaining only changes that preserve all-syscall userspace mediation and
  demonstrating rejected or ineffective alternatives with measured evidence.

## Remaining work

- [x] Finalize the initial matched benchmark protocol and default 4% budget
  from prior evidence. Final-duration confirmation remains part of completion.
- [x] Package or install one pinned `sysbench` build for use by both the native
  control and Ginux instance, and verify its identity before every comparison.
- [x] Implement comparison and report automation.
- [x] Implement strict `paranoid` and `selective` instance configuration,
  defaulting omitted configuration to selective and retaining explicit
  paranoid recipes for all-syscall mediation tests.
- [x] Implement the first selective direct-safe operation (`sched_yield`) and
  prove that denied and complex operations still enter Ginux before effect.
- [x] Add mode-specific configuration, runtime, denial, audit, and performance
  verification.
- [x] Collect the initial repeated baseline results used to select profiling
  targets. Final accepted results require a longer confirmation run.
- [x] Profile and optimize the thread-yield hot path through the limit of the
  approved userspace-dispatch architecture. The mandatory row still fails and
  requires an architecture decision; other Gate 11 workloads remain open.
- [ ] Persist the accepted baseline and add automatic per-workload regression
  checks to the normal performance test target. The checks are implemented;
  publishing an accepted baseline is intentionally pending because four
  mandatory rows in the final retained-code matrix still fail.
- [x] Run full regression and document remaining costs.
- [x] Complete the human-authorized extended selective-mode profile sweep:
  profile every mandatory workload, apply each evidence-backed in-scope
  optimization, explore materially different credible alternatives, and run a
  final clean 1/2/4-vCPU matrix plus mandatory regressions before stopping.
- [x] Completed the human-authorized paranoid-mode follow-on sweep: collected the
  full matched matrix, profiled the broker path, applied invariant-preserving
  improvements, and rerun the matrix plus mandatory regressions.

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
- Existing recipes normalize to selective dispatch unless they explicitly opt
  into paranoid mode. Unknown dispatch modes fail closed.
- In paranoid mode, `sched_yield` and every other syscall reach Ginux before
  completion. In selective mode, an allowed `sched_yield` executes directly,
  while a configured denial returns from Ginux with no host syscall effect and
  virtual or inspection-dependent operations remain mediated.
- Paranoid-mode optimization evidence must identify which selective
  improvements transfer, which do not, and why; no retained change may turn a
  paranoid syscall into a direct kernel path.
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
- Dispatch-mode tests -> reject unknown configuration; prove paranoid
  all-syscall mediation; prove selective direct-safe execution; and prove that
  denial, translation, emulation, and inspection paths remain in Ginux.
- Paranoid benchmark matrix -> runs the same pinned workloads, allocations,
  affinity, memory limit, warmup, and repetitions as the selective matrix,
  records dispatch mode as compatibility metadata, and verifies the paranoid
  runtime marker plus mediated `sched_yield` evidence.

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
- The final clean selective matrix in `build/gate11-selective-final/` used
  five-second workloads and five repetitions at 1, 2, and 4 vCPUs. All CPU
  rows passed at 0.996x to 1.001x, all memory rows passed at 0.969x to 1.009x,
  and all mutex rows passed at 0.990x to 0.996x. All five thread-yield rows
  remain mandatory failures: 0.949x, 0.932x, 0.935x, 0.949x, and 0.920x
  native performance (5.09% to 7.99% slower).
- Before the allow-cache-compatible filter, the stable one-vCPU thread row was
  0.932x native. A focused post-change probe reached 0.971x; the complete final
  matrix above is authoritative because it covers every allocation with the
  longer repeated protocol.
- `make test-bench-compare` passes and proves nonzero outcomes for a native
  budget violation and an accepted-Ginux regression, plus a distinct exit 2
  for incompatible environments. Running `scripts/bench-compare.sh` against
  the final matrix returns 1 and names exactly the five failing thread rows.
- Fresh retained-code verification passes `make check`, `make test-contract`,
  `make test-contract-runtime`, `make test-native-vfs`, `make test-process`,
  `make test-kernel-views`, and the complete `make test-systemd` cold, warm,
  repeat, offline, DNF-failure, startup-failure, and shutdown suite.
- After changing the omitted-value default, `make check`, `make test-contract`,
  `make test-bench-compare`, and live QEMU `make test-config`,
  `make test-contract-runtime`, and the complete `make test-systemd` suite pass.
  The Rocky systemd example runs selective dispatch; explicit paranoid fixtures
  continue proving all-syscall mediation and pre-effect socket denial.
- The extended clean retained-code matrix in
  `build/gate11-selective-exhaustive-final/` used five-second workloads and five
  alternating repetitions. All CPU rows passed at 0.994x to 1.002x, all memory
  rows passed at 0.992x to 1.037x, and all mutex rows passed at 0.994x to
  0.998x. Four thread rows failed at 0.909x to 0.935x; the 4-vCPU/1-thread row
  passed at 0.968x but does not eliminate the repeated thread-path deficit.
- A matching audit-disabled thread matrix still measured 0.923x to 0.954x
  native. Removing Ginux audit formatting and writes therefore does not recover
  the residual selective-mode loss.
- A final no-Ginux diagnostic installed one constant `SECCOMP_RET_ALLOW`
  filter and then executed the identical one-thread workload without any
  tracer. Its five-run median was 619.289 versus 657.868 events/s natively,
  or 0.941x. The kernel's minimal seccomp fast path alone reproduces the
  remaining deficit under x86-64 QEMU TCG.
- Fresh retained-code verification passes `make check`, `make test-contract`,
  `make test-bench-compare`, isolated Rocky QEMU `make test-contract-runtime`,
  aggregate `make test`, `make test-native-vfs`, and the complete cold, warm,
  repeat, offline, DNF-failure, startup-failure, and shutdown
  `make test-systemd` matrix.
- The explicit paranoid five-second/five-repetition matrix in
  `build/gate11-paranoid-final/` passes all CPU rows at 1.000x to 1.009x, all
  memory rows at 0.992x to 1.016x, and all mutex rows at 0.991x to 0.997x.
  All five thread-yield rows fail at 0.028x to 0.037x native because every
  repeated yield requires a Ginux userspace round trip.
- A three-second audit-disabled paranoid thread matrix reached 0.029x to
  0.035x native. Audit formatting and writes are therefore secondary to the
  notification boundary and cannot recover the missing performance.
- `perf stat` on a one-vCPU, one-thread, three-second paranoid run recorded
  91,376 context switches for approximately 45,000 mediated yields, 2.833
  seconds of system time, and 0.372 seconds of user time. This is approximately
  two scheduler transitions per decision and localizes the cost below the Rust
  contract and audit logic.
- Pinning the broker and supervisor to CPU 3 while one- and two-vCPU workloads
  remained on CPUs 0 and 1 measured 0.033x to 0.037x native, so CPU separation
  did not materially change the notification cost.
- A diagnostic with one notification receiver per configured vCPU regressed
  the two- and four-vCPU rows to 0.010x to 0.025x because receivers contended
  on one seccomp listener. It was reverted; the retained single broker remains
  the fastest measured implementation.
- Selective optimizations that transfer are already retained in paranoid mode:
  constant-time indexed contract decisions, one precomputed yield rule,
  bounded audit with logarithmic drop reporting, a minimal dispatch filter,
  and removal of the redundant second host yield. Selective listener removal
  cannot transfer because paranoid requires that listener; direct-safe
  execution cannot transfer without violating the mode's defining invariant.
- Benchmark collection now accepts explicit `selective` or `paranoid`
  dispatch, writes the effective recipe into the result directory, records the
  mode as compatibility metadata, validates the corresponding runtime marker,
  and rejects accepted-baseline comparisons across modes.
- Fresh paranoid-phase verification passes `make check`, `make test-contract`,
  `make test-bench-compare`, isolated Rocky QEMU `make test-contract-runtime`,
  and the complete aggregate `make test` suite on isolated port 22450.

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
- Human direction on 2026-11-07 selected configurable dispatch: paranoid keeps
  the original all-syscall userspace path; selective compiles explicit contract
  permission plus Ginux's direct-safety classification into a direct path.
  Denials and complex semantics remain in Ginux. Native x86-64/KVM should also
  be measured before setting a durable budget because the current macOS arm64
  QEMU TCG environment amplifies context-switch and seccomp costs.
- Later human direction on 2026-11-07 selected selective as the default for
  lightweight execution. This changes only the omitted-value default; it does
  not expand the direct-safe catalog or weaken explicit paranoid mode.
- Selective `sched_yield` no longer enters the Ginux userspace dispatcher or
  audit path. Its remaining 5.09% to 7.99% deficit is the host-kernel seccomp
  filter/cache and task-work cost as amplified by x86-64 QEMU TCG, not contract
  lookup, audit formatting, ptrace stops, or broker round trips.
- A diagnostic run with `SECCOMP_FILTER_FLAG_SPEC_ALLOW` measured 0.939x native,
  worse than the retained path, and would disable a speculative-execution
  mitigation. It was reverted and is not an acceptable optimization.
- Per-workload audit capture showed that CPU, memory, and mutex overhead is
  limited to bounded setup and low-frequency mediated operations. Selective
  thread runs contain no `sched_yield` audit records, confirming that the
  benchmark hot syscall never reaches the userspace dispatcher.
- A live tracee probe reported `Seccomp: 2` and `Seccomp_filters: 1`; the
  residual cost is not caused by accidentally stacking multiple filters.
- A diagnostic filter that also allowed the observed non-PI futex operations
  directly produced zero futex audit records but did not materially improve
  the thread matrix (0.917x to 0.936x). The broader direct-safe surface was
  reverted because it added authority without measurable benefit.
- Pinning the Ginux supervisor to CPU 3 while the four-vCPU workload shared
  that CPU measured 0.927x and 0.932x. A separate five-vCPU QEMU run reserved
  CPU 4 exclusively for the supervisor while native and Ginux workloads both
  remained constrained to CPUs 0 through 3; all rows still measured 0.926x to
  0.941x. Supervisor CPU contention is therefore not the residual bottleneck.
- `perf stat` separated the scheduler effects. At one vCPU/one thread, native
  used 80 context switches versus 1,419 for Ginux; at four vCPUs/four threads,
  native used 277 versus 21,911. Task-clock stayed comparable. Together with
  the zero-userspace-dispatch evidence, this localizes the remaining loss to
  the kernel seccomp/task-work and scheduling path under QEMU TCG.
- No workload-specific tolerance exception has been approved. Gate 11 remains
  partial until the human chooses whether to require native x86-64/KVM
  confirmation, approve a thread-specific budget, or authorize a kernel-side
  enforcement architecture beyond the approved userspace-kernel scope.
- Human direction on 2026-11-07 authorizes up to ten hours of additional
  autonomous selective-mode profiling and optimization. The 4% per-row budget
  and all security invariants remain unchanged; elapsed time does not make the
  gate pass, and any remaining architecture change still returns to human
  review.
- Human direction on 2026-11-07 extends the open Gate 11 investigation to
  paranoid dispatch. The same native-parity goal, 4% budget, dedicated branch,
  checkpoint policy, and security invariants apply. Selective optimizations may
  transfer only when every paranoid syscall still receives a pre-completion
  Ginux userspace decision.
- Paranoid's userspace notification cost is intentionally much larger than
  selective's kernel-resident allow path. Linux seccomp user notification
  requires one receive and one response ioctl plus the broker and tracee
  wakeups for each decision. The measured two context switches per yield are
  intrinsic to that boundary; removing them would make the operation selective
  or require a new kernel-side enforcement architecture.

## Delivery strategy

- Working branch: `hdae/performance-characterization`
- Intended base: `main`
- Pull request: intended; dedicated branch push and PR creation authorized on
  2026-11-03; never merge or close without explicit authorization
- Planned commits: matched benchmark protocols and budget record; comparison
  and report tooling; targeted optimizations; final regression and evidence
- Actual commits: `f8dd4497` (dispatch decision and gate record), `da7d9ba9`
  (configuration and selective runtime), `e177f24d` (cache-compatible selective
  filter), `efbe11f8` (budget and regression automation), `2a021baf`
  (deterministic tracee regression), `1b79c033` (selective dispatch default),
  `811ad59c` (selective listener cleanup and audit-profile capture),
  `a747bcfd` (exhaustive selective profiling), paranoid-mode benchmark tooling
  and evidence (pending)

## Review policy and outcome

Human checkpoint before approving numeric budgets and again at completion. No
performance claim is accepted solely from an aggregate score. Present
`make demo-gate-11`, raw evidence, and the pull request before final acceptance.
Current outcome: **partial**. The implementation and regression work are
complete, but four mandatory thread rows in the final retained-code matrix
exceed the unchanged 4% budget and no accepted baseline has been published.
