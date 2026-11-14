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

- [ ] None.

## Remaining work

- [ ] Complete the approved compatibility and differential matrix.
- [ ] Complete assurance work and reconcile every failure.
- [ ] Publish matched performance evidence and residual limitations.

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

- Not run.

## Decisions and limitations

- Upstream challoc is a comparison control and source of architectural evidence,
  not a claim that Ginux independently reproduced its guarantees.
- Final claims are limited to the tested ABI, platforms, workloads, and proof
  boundaries.

## Delivery strategy

- Working branch: `hdae/allocator-compatibility-proof-performance`
- Intended base: `main`
- Pull request: intended
- Planned commits: compatibility matrix; distribution workloads; assurance;
  benchmark reports; packaging and final evidence
- Actual commits: pending

## Review policy and outcome

Final allocator checkpoint requiring explicit human acceptance. Persistence of
this gate does not authorize implementation.
