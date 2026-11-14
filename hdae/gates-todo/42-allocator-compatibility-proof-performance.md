# Allocator compatibility, assurance, and performance

## Source

Conditional closure gate for the allocator initiative after accepted Gates 38
through 41.

## Goal

Establish reproducible compatibility, defensive assurance, and matched
performance evidence for the allocator without overstating its maturity.

## Approved scope

- Differentially test allocator behavior against the approved glibc, jemalloc,
  and upstream challoc controls.
- Exercise representative Rocky, Ubuntu, and Alpine workloads inside Ginux and
  matching native controls.
- Add the approved jemalloc extended API subset if Gate 38 requires it.
- Apply fuzzing, model checking, and formal verification to selected critical
  allocator invariants with explicit assumptions.
- Produce matched latency, throughput, memory, fragmentation, and scaling
  reports with raw data and provenance.

## Non-goals

- No universal drop-in compatibility, complete formal verification, or blanket
  claim that the allocator is safer or faster than every alternative.
- No exploit development or offensive vulnerability comparison.

## Completed work

- [x] Completed claimed ABI, common jemalloc subset, defensive assurance, and
  Rocky/Ubuntu glibc compatibility coverage.
- [x] Verified Alpine as a supported Ginux runtime while explicitly excluding
  the glibc allocator preload from the musl compatibility claim.
- [x] Retained exact raw samples, provenance, variance, latency, throughput,
  RSS, fragmentation proxy, and generated report/graph.
- [x] Completed the full Ginux regression and packaged-product matrices.

## Remaining work

- [x] Complete the approved compatibility and differential matrix.
- [x] Complete assurance work and reconcile every failure.
- [x] Publish matched performance evidence and residual limitations.
- [ ] Human testing and acceptance of the combined pull request.

## Acceptance criteria

- Every claimed ABI is covered by deterministic native and Ginux tests across
  the approved distributions and workloads.
- Differential mismatches are resolved, explicitly documented as intentional,
  or removed from the compatibility claim.
- Selected critical invariants have reproducible proof or model evidence with
  their trusted assumptions; unsupported coverage is stated plainly.
- Performance reports retain raw samples, environment provenance, variance, and
  paired controls and do not hide failed rows.
- Packaging, loading, examples, and operator documentation reproduce the tested
  configurations from a clean install.

## Verification plan

- Allocator conformance and differential suites -> all claimed behavior passes.
- Distribution/workload matrix -> Rocky, Ubuntu, and Alpine cases pass in the
  documented native and Ginux environments.
- Proof/model/fuzz commands -> selected invariants and bounded targets pass with
  recorded tool versions and assumptions.
- Matched benchmark workflow -> validated raw data, tables, and graphs are
  reproducible from checked-in templates.
- Complete Ginux regression matrix -> allocator integration does not weaken
  contract or instance isolation.

## Evidence

- `make test-allocator-rocky` -> conformance and 45 exact-commit matched samples
  passed; raw data and provenance are under `benchmarks/allocator/`.
- Final mean Ginux/native throughput: 0.680x at 1 thread, 0.665x at 2, and
  0.573x at 4. The 4-thread Ginux variance is retained in the report.
- `make test-rpm-metadata test-rpm` -> Rocky and Ubuntu glibc loading, Alpine
  runtime, install, verify, cache, reinstall, erase, and recovery passed.
- `make fuzz-security`, `make test-security test-recovery`, aggregate
  `make test`, and `make test-multi-instance` -> assurance and project
  regressions passed.

## Decisions and limitations

- Upstream challoc is a comparison control and source of architectural evidence,
  not a claim that Ginux independently reproduced its guarantees.
- Final claims are limited to the tested ABI, platforms, workloads, and proof
  boundaries.
- No challoc binary row is reported because its upstream tested glibc baseline
  is incompatible with Rocky 10; fabricating an unsupported build would not be
  a valid matched control.
- The benchmark is a bounded replacement-allocation workload under QEMU/TCG,
  not a general application-performance claim.

## Delivery strategy

- Working branch: `hdae/security-allocator-initiative`
- Intended base: `main`
- Pull request: https://github.com/gmkurtzer/ginux/pull/292
- Planned commits: compatibility matrix; distribution workloads; assurance;
  benchmark reports; packaging and final evidence
- Actual commits: `19377734`, `48577ef8`, `76209bc8`, `6a66fc73`, `da24dff7`,
  `9f7a9ae7`

## Review policy and outcome

Technically passed. Final checkpoint for the authorized Gates 32-42 batch:
deliver one combined pull request and stop for explicit human testing and acceptance.
