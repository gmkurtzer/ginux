# Performance characterization and optimization

## Source

Approved requirement that Ginux quantify and minimize overhead without
weakening mediation.

## Goal

Produce reproducible native, conventional-container, and Ginux comparisons,
agree on explicit performance budgets, and optimize material overhead while
preserving every security invariant and compatibility criterion.

## Approved scope

- Define matched CPU, memory, syscall, process, filesystem, block, network,
  systemd boot, and idle workloads using the same Rocky userspace where
  practical.
- Record hardware, host kernel, Ginux commit, image identity, configuration,
  contract, repetitions, warmup, variance, and raw measurements.
- Set numeric acceptance budgets with human review after baseline evidence and
  before optimization claims are judged.
- Profile Ginux and optimize the highest-impact in-scope paths.
- Add benchmark comparison and report generation suitable for regression use.
- Rerun functional, security-boundary, and systemd suites after optimizations.

## Non-goals

- Claiming universal near-native performance.
- Comparing unmatched workloads or hiding unfavorable results.
- Bypassing interception, validation, virtual objects, audit requirements, or
  isolation to improve a benchmark.
- Production-scale capacity, multi-domain density, or hardware acceleration not
  already in scope.

## Completed work

- [ ] None; gate not started.

## Remaining work

- [ ] Finalize matched benchmark protocols and budgets from prior evidence.
- [ ] Implement comparison and report automation.
- [ ] Collect statistically useful baseline results.
- [ ] Profile and optimize material overhead.
- [ ] Run full regression and document remaining costs.

## Acceptance criteria

- Each benchmark compares semantically matched work and clearly labels any
  unavoidable difference.
- Raw results and the generated report contain enough environment and protocol
  metadata for another operator to reproduce the run.
- Numeric budgets for every mandatory workload are recorded in this open gate
  and approved before they are used to pass or fail the optimized result.
- Final results satisfy all approved mandatory budgets, or the gate is reported
  failed or partial rather than weakening a threshold silently.
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
  budgets, returning nonzero for mandatory regressions.
- `make bench-report` -> generates the versioned summary from raw results.
- `make bench-block` and `make bench-network` -> device-path measurements are
  included in comparison evidence.
- `make test test-systemd test-contract test-security` -> all available
  mandatory regressions exit zero after optimization.
- `make demo-gate-11` -> performs the documented bounded benchmark comparison,
  validates approved budgets, generates the report, and exits zero.

## Evidence

- Not run.

## Decisions and limitations

- Numeric budgets are intentionally unresolved until Gate 1 and later path
  measurements provide evidence. Human approval is required before this gate's
  optimization phase treats them as acceptance thresholds.
- CPU-bound overhead is expected to differ materially from syscall-, process-,
  storage-, and network-heavy overhead; results remain per workload.

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
