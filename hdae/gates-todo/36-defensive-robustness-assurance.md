# Defensive robustness and assurance

## Source

Defensive security-review initiative; depends on Gates 32 through 35.

## Goal

Turn the reviewed boundary into durable fuzz, property, concurrency,
resource-exhaustion, and formal-assurance evidence without offensive testing.

## Approved scope

- Fuzz parsers, configuration normalization, OCI metadata, syscall argument
  decoders, lifecycle transitions, and selected virtual-kernel interfaces with
  bounded deterministic corpora.
- Add property and model tests for decision precedence, generation monotonicity,
  revocation, atomic state updates, concurrency, and bounded resource behavior.
- Pilot an appropriate Rust verification or model-checking tool on a small set
  of critical invariants and document its trusted assumptions.

## Non-goals

- No exploit generation, payload corpus, public-target scanning, or proof that
  all vulnerabilities are impossible.
- No requirement to force one formal tool onto code it cannot model soundly.

## Completed work

- [x] Added the allocator to the bounded deterministic fuzz campaign.
- [x] Added exhaustive allocator geometry, class, accounting, and shard-probe
  models plus concurrent C stress and failure diagnostics.
- [x] Documented why whole-runtime formal proof is unsupported and the bounded
  model/property substitute used instead.

## Remaining work

- [x] Select bounded targets from the Gate 32 inventory.
- [x] Add reproducible fuzz/property/concurrency suites.
- [x] Complete and document the formal-assurance pilot.
- [ ] Human review and acceptance of the combined pull request.

## Acceptance criteria

- Every selected parser and state machine has a reproducible bounded test with
  checked-in seeds and no unclassified crash or hang.
- Property tests cover deny precedence, stale-generation rejection, atomic
  replacement, revocation, and resource bounds.
- The formal pilot proves named invariants or records a reproducible technical
  limitation and a better alternative; unsupported proof is never claimed.
- Failures become minimal regression fixtures before the gate can pass.

## Verification plan

- `make fuzz-security` -> all bounded defensive fuzz targets complete cleanly.
- `make test-security test-recovery` -> property, concurrency, exhaustion, and
  recovery tests pass.
- Tool-specific proof/model command -> named invariants pass from a clean tree,
  or the gate records the reproducible limitation and accepted substitute.
- `make test` -> aggregate regressions pass.

## Evidence

- `make fuzz-security` -> all nine targets passed five-second bounded campaigns,
  including 158,877 allocator cases in the final run.
- `make test-allocator` -> seven arithmetic, geometry, accounting, allocation,
  reallocation, guard, and shard-coverage model tests passed.
- `make test-security test-recovery` and aggregate `make test` -> property,
  exhaustion, concurrency, failure, and recovery checks passed.

## Decisions and limitations

- Fuzzing is confined to project-owned parsers and controlled local fixtures.
- Formal assurance is scoped to small critical components; it does not certify
  the complete runtime or host kernel.
- No sound external model was available for the combined raw-syscall, ptrace,
  loader, and concurrent-caller boundary; the limitation and substitute are
  explicit in `docs/security-review.md`.

## Delivery strategy

- Working branch: `hdae/security-allocator-initiative`
- Intended base: `main`
- Pull request: one combined Gates 32-42 pull request intended
- Planned commits: target selection; fuzz/property harnesses; proof pilot;
  findings and final evidence
- Actual commits: `48577ef8`, `36c742e5`, `da24dff7`

## Review policy and outcome

Technically passed under the authorized batch. Kept open for the final human
checkpoint on the combined pull request.
