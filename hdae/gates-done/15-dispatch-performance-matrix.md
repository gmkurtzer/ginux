# Dispatch-mode performance matrix

## Source

Post-Gate-11 request to make the cost and protection of each supported syscall
control model directly comparable and independently reproducible.

## Goal

Produce a reproducible native, selective, and paranoid performance matrix with
tables and graphs that show the cost of each Ginux syscall-control model while
keeping its security semantics visible. Publish reusable configuration
templates and update all affected project documentation so another operator can
recapitulate the measurements.

## Approved scope

- Reuse the pinned Rocky `sysbench` binary and matched-host protocol established
  by Gate 11.
- Measure native Rocky as the no-Ginux control, selective Ginux, and paranoid
  Ginux with identical workload arguments, vCPU allocations, affinity, memory
  limits, warmups, repetitions, and host environment.
- Cover CPU, memory, thread-yield, and mutex workloads at 1, 2, and 4 vCPUs,
  retaining both single-thread and allocation-scaled rows where applicable.
- Generate one combined machine-readable result set, a Markdown report, and
  graphs showing native-relative performance for every row and mode.
- Keep raw measurements, environment metadata, source revision, image identity,
  configuration digest, variance, and paired-control provenance visible.
- Add checked-in template configurations for the native, selective, and
  paranoid test profiles, plus a documented suite template operators can copy
  and adjust without editing benchmark scripts.
- Update the README, performance and contract documentation, security-boundary
  documentation, HDAE architecture, plan, status, and any benchmark help or
  examples affected by the new workflow.
- Add deterministic report and configuration tests that do not require QEMU,
  plus a bounded human demonstration and the full reproducible matrix.

## Non-goals

- Add an unmediated Ginux execution mode. Native Rocky is the `none` control;
  Ginux continues to expose only selective and paranoid syscall control.
- Reopen Gate 11's unresolved native-parity criterion or silently introduce new
  performance budgets.
- Optimize benchmark failures, weaken contracts, or change direct-safe syscall
  classification. Any newly justified optimization requires a separate gate.
- Claim that these workloads represent all applications or deployment shapes.
- Commit generated raw benchmark output or machine-specific results as source.

## Planned work

- [x] Confirm Gate 13 is merged and carry its accepted closure record forward.
- [x] Reaffirm Gate 14 as deferred.
- [x] Define versioned Gate 15 profile and suite templates.
- [x] Add a multi-mode collection driver that uses paired native controls.
- [x] Add combined validation, reporting, graph generation, and deterministic
  fixture tests.
- [x] Add stable Make targets and a bounded Gate 15 demonstration.
- [x] Update every affected project and operator document.
- [x] Run static, deterministic, functional, and full benchmark verification.
- [x] Record final evidence and limitations.
- [x] Deliver the pull request after explicit human authorization.

## Acceptance criteria

- The native, selective, and paranoid rows use the same pinned executable,
  workload arguments, vCPU allocation, thread count, memory limit, affinity,
  warmup, repetitions, and Rocky host session.
- Every selective and paranoid row has an explicitly paired native sample;
  incompatible metadata fails validation rather than producing a graph.
- The matrix covers CPU, memory, threads, and mutex at 1/2/4 vCPUs, with one
  thread at every allocation and allocation-scaled rows for 2 and 4 vCPUs.
- Combined output reports raw samples, mean, standard deviation or an equally
  explicit variance measure, native-relative ratio, percentage difference, and
  sample count for every row.
- The Markdown report and generated graph distinguish native, selective, and
  paranoid, identify higher/lower-is-better units, and never hide a failing or
  unusually slow row behind an aggregate score.
- Checked-in templates fully specify every test profile and suite control needed
  to reproduce the run. Invalid modes, workloads, allocations, repetitions,
  durations, or incomplete templates fail before collection.
- Documentation explains how to prepare dependencies, run a bounded sample,
  run the full matrix, inspect raw data, regenerate reports, and interpret the
  security/performance tradeoff.
- Selective and paranoid semantics remain unchanged and their existing contract,
  security, systemd, and aggregate regression suites pass.
- `make demo-gate-15` performs a bounded reproducible comparison, validates the
  combined data, generates the report and graphs, and exits zero.

## Verification plan

- Template validation test -> accepts all checked-in profiles and rejects
  malformed or incomplete fixtures without QEMU.
- Report fixture test -> produces deterministic combined TSV, Markdown, and SVG
  outputs and rejects missing rows, mismatched metadata, and invalid values.
- `make bench-modes-prepare` -> verifies or acquires the pinned benchmark image
  and executable using the existing explicit network-enabled preparation path.
- `make bench-modes` -> runs the full native/selective/paranoid matrix and
  writes raw results and metadata under an operator-selected build directory.
- `make bench-modes-report` -> validates an existing matrix and regenerates its
  combined report and graphs without rerunning workloads.
- `make demo-gate-15` -> runs a bounded matrix from the checked-in templates and
  exits zero after validation and report generation.
- `make check test-contract test-security test-systemd test` -> all mandatory
  regressions exit zero after the reporting and harness changes.

## Documentation coverage

- `README.md`: purpose, quick start, outputs, and interpretation.
- `docs/performance.md`: complete reproducibility protocol, template reference,
  result schema, graphs, limitations, and troubleshooting.
- `docs/configurable-contract.md`: selective/paranoid semantics and native
  control terminology.
- `docs/security-boundary.md`: protection/performance interpretation and limits.
- `hdae/PROJECT.md`, `hdae/ARCHITECTURE.md`, `hdae/PLAN.md`, `hdae/STATUS.md`:
  intent, boundary, ordering, active checkpoint, and evidence.
- Make help/comments and demonstration output: discoverable commands and exact
  generated artifact locations.

## Delivery strategy

- Working branch: `hdae/dispatch-performance-matrix`
- Intended base: `main`
- Pull request: https://github.com/gmkurtzer/ginux/pull/257, merged into `main`
  on 2026-11-11 after explicit human review.
- Planned commits: gate and templates; collection/validation/reporting; tests
  and Make interface; documentation; final benchmark evidence and checkpoint.

## Review policy and outcome

Accepted and closed by the human during ledger reconciliation on 2026-11-11.
Pull request 257 was merged into `main` on 2026-11-11 after the complete
benchmark matrix and regression evidence passed. Gate 11 continues to own the
documented selective thread-yield performance gap.

## Evidence

- `make test-gate15-report` passes deterministic template validation, fixture
  combination, report and SVG generation, and malformed/incomplete input
  rejection.
- `make demo-gate-15` passes a bounded native/selective/paranoid QEMU run and
  generates validated combined TSV, Markdown, and SVG outputs.
- `make bench-modes` completes the full 5-second, five-repetition matrix at
  1/2/4 vCPUs. It records 400 measured samples across CPU, memory,
  thread-yield, and mutex workloads, with paired native samples and compatible
  metadata for both Ginux modes.
- `make bench-modes-report` validates and regenerates
  `build/gate15/results.tsv`, `build/gate15/report.md`, and
  `build/gate15/performance.svg` without rerunning workloads.
- `xmllint --noout build/gate15/performance.svg` passes.
- `make check`, `make test-contract`, `make test-security`, and
  `make test-systemd` pass.
- A complete elevated `make test` passes all static, deterministic, unit, and
  QEMU-backed integration tests. The sandboxed attempt could not bind QEMU's
  local SSH forwarding port; the same suite passed outside that network
  sandbox without source changes.
- Selective CPU rows range from `0.9922x` to `1.0064x`, memory from `0.9737x`
  to `1.0235x`, and mutex from `0.9642x` to `1.0028x` native performance.
  Selective thread-yield remains the outlier at `0.9216x` to `0.9401x`.
- Paranoid CPU rows range from `0.9807x` to `1.0106x`, memory from `0.9748x`
  to `1.0115x`, and mutex from `0.9618x` to `1.0417x`. Paranoid thread-yield
  exposes the expected all-syscall mediation cost at `0.0275x` to `0.0350x`.
- Measurement metadata identifies source commit `9f56f9f5`, a clean working
  tree, the pinned Rocky image and sysbench checksum, workload controls,
  affinity, host identity, configuration digests, and alternating run order.

## Decisions and limitations

- Native Rocky is the no-mediation control. Gate 15 does not add a `none` value
  to `[contract.syscalls].control`.
- Gate 15 characterizes retained behavior. Gate 11 remains the owner of native
  parity and any approved performance exceptions.
- Selective thread-yield's 5.99% to 7.84% deficit and paranoid thread-yield's
  96.50% to 97.25% deficit are presented explicitly rather than converted into
  a Gate 15 failure budget. Any further optimization belongs to a separately
  approved Gate 11 continuation.
- Generated machine-specific results remain under ignored `build/gate15/`;
  checked-in templates, tooling, and documentation are sufficient to reproduce
  them on another supported Rocky host.
