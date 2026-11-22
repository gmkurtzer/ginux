# Guarded allocator performance parity

## Source

Follow-up to the human performance review of Gate 42 and pull request 292.

## Goal

Profile and optimize the opt-in Ginux guarded allocator until every required
matched 1-, 2-, and 4-thread benchmark row performs within 5% of the native
distribution allocator, without weakening allocator hardening, instance
contracts, or Ginux isolation.

## Approved scope

- Replace the short allocator microbenchmark with a stable fixed-duration
  harness using warmups, retained samples, CPU affinity, raw results, and exact
  environment provenance.
- Measure native glibc, packaged jemalloc, and the Ginux guarded allocator in
  the same Rocky Linux x86-64 environment.
- Cover small-object replacement churn, mixed size classes, calloc/realloc and
  aligned allocation, cross-thread allocation/free, large allocations and
  reclamation, and representative packaged Ginux-instance workloads.
- Profile allocation, deallocation, lookup, sharding, contention, cache misses,
  memory accounting, and reclamation before selecting optimizations.
- Optimize the guarded allocator's hot paths, including thread-local caching,
  stable shard ownership, reusable free-slot structures, batched accounting,
  and pointer metadata lookup when supported by profiling evidence.
- Preserve raw before/after samples and publish throughput, latency, scaling,
  variance, RSS, and fragmentation evidence without dropping failed rows.
- Run the complete allocator, security, recovery, multi-instance, packaging,
  and aggregate Ginux regression matrices after optimization.

## Non-goals

- Do not make the Ginux allocator the default in this gate. The system
  allocator remains the default and the guarded allocator remains opt-in.
- Do not weaken guard pages, invalid-free detection, checked arithmetic,
  instance-private memory limits, fail-closed behavior, or other accepted
  security properties to meet the performance target.
- Do not change syscall dispatch, the host kernel, or unrelated Ginux runtime
  architecture.
- Do not claim universal allocator parity from the bounded required matrix.

## Completed work

- [x] Recorded the Gate 42 baseline: 0.680x native at one thread, 0.665x at
  two threads, and 0.573x mean at four threads, with high four-thread variance.
- [x] Established that the system allocator remains the default and that the
  guarded allocator is experimental and opt-in.

## Remaining work

- [ ] Build and validate the stable fixed-duration benchmark matrix.
- [ ] Capture profiles and a reproducible hotspot/accounting baseline.
- [ ] Implement profiling-backed allocator optimizations in reviewable steps.
- [ ] Iterate across every required row until all pass or no credible in-scope
  optimization remains.
- [ ] Run the complete regression and packaged-product verification matrix.
- [ ] Publish exact before/after evidence, residual limitations, and rejected
  alternatives.
- [ ] Human testing and acceptance of the dedicated pull request.

## Acceptance criteria

- Every required workload at 1, 2, and 4 threads reports mean throughput of at
  least 0.95x its matched native glibc control and mean latency no greater than
  1.05x native. Any failing row fails the gate; averaging passing and failing
  rows together is prohibited.
- Each performance row uses at least two unreported warmups and ten retained
  fixed-duration samples of at least five seconds, with CPU affinity and run
  ordering recorded. Coefficient of variation above 10% requires diagnosis and
  rerun rather than acceptance.
- Profiling evidence identifies the material pre-optimization costs and shows
  how each accepted optimization changes them.
- Existing allocator conformance, invalid-operation handling, concurrency,
  fuzz/model, memory-limit, pressure, isolation, and compatibility tests remain
  green without weakening their assertions.
- The current small-object fragmentation proxy remains bounded at no more than
  2.0x live requested memory, and long-running tests show no unbounded RSS or
  metadata growth.
- Rocky and Ubuntu glibc packaged workloads continue to load and run the
  allocator; Alpine remains a supported Ginux runtime outside the glibc
  allocator-preload claim.
- Documentation continues to identify the allocator as opt-in unless a later,
  separately approved product gate changes that policy.
- If exhaustive profiling-backed optimization cannot meet every 5% row, this
  gate remains partial or failed and reports the limiting mechanisms and
  concrete alternatives; the target is not weakened silently.

## Verification plan

- New allocator benchmark harness self-tests -> reject missing warmups,
  insufficient duration/samples, incompatible provenance, unstable rows, or
  omitted controls.
- `make bench-allocator-parity` -> collect the complete matched workload and
  1/2/4-thread matrix with raw profiles and samples.
- `make bench-allocator-parity-report` -> fail unless every required row meets
  the 0.95x throughput, 1.05x latency, and stability criteria.
- `make test-allocator-rocky` -> claimed ABI, concurrent behavior, invalid
  operations, limits, reclamation, and distribution integration pass.
- `make test-security test-recovery test-multi-instance` -> accepted security,
  lifecycle, and isolation properties remain intact.
- `make test-rpm-metadata test-rpm` -> clean packaged-product installation,
  verification, supported distribution loading, cache, upgrade, removal, and
  recovery pass.
- `make check test` -> static, unit, launch, and aggregate regressions pass.

## Evidence

- Gate 42 baseline and raw data:
  `docs/allocator-performance.md` and `benchmarks/allocator/rocky-qemu.tsv`.
- New evidence: pending implementation.

## Decisions and limitations

- Performance readiness and default selection are separate decisions. Passing
  this gate would establish bounded performance parity, not authorize making
  the allocator the default.
- QEMU/TCG remains a reproducible matched environment, but a native Linux or
  KVM confirmation run should accompany it when available and must be reported
  separately rather than mixed into the matched control.
- Gate 43 depends on human acceptance and merge of the allocator foundation in
  Gate 42 and pull request 292.

## Delivery strategy

- Working branch: `hdae/allocator-performance-parity` after Gate 42 acceptance
- Intended base: `main`
- Pull request: one dedicated Gate 43 pull request intended
- Planned commits: benchmark stabilization; profiling baseline; allocator
  hot-path optimizations; full regression and performance evidence
- Actual commits: pending

## Review policy and outcome

Human checkpoint. The gate may not close until every mandatory performance row
and regression criterion passes and the human explicitly accepts the results.
