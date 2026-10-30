# Feedback-Hardening Plan

## Delivery strategy

- Working branch: `codex/feedback-hardening`
- Intended base: `main`
- Historical baseline: `research-v1-baseline`
- Review policy: delegated closure for all ten gates after complete fresh test
  evidence; stop only for a failed dependency, architectural decision, scope
  expansion, or consequential action outside the approved batch
- Commit policy: small, signed-off commits associated with one gate
- Pull request: one pull request from `codex/feedback-hardening` to `main` for
  the entire secondary experiment; verify its remote status before Gate 1 work

## Gate sequence

| Gate | Purpose | Depends on |
|---:|---|---|
| 1 | Baseline and claim reproduction | None |
| 2 | Harden x86-64 privilege entry | 1 |
| 3 | Add fault-recovering userspace access | 2 |
| 4 | Classify global state and define ownership | 3 |
| 5 | Migrate runtime state ownership | 4 |
| 6 | Migrate platform and service state ownership | 5 |
| 7 | Decompose the x86-64 implementation | 6 |
| 8 | Add synchronization and SMP readiness | 7 |
| 9 | Implement SMP and make 2-vCPU QEMU the default | 8 |
| 10 | Qualify and compare the result | 9 |
| 11 | Measure matched 1/2/4-vCPU Linux and Ginux topologies | 10 |
| 11.5 | Correct shared writable Linux pages across runtime CPUs | Gate 11 fault reproduction |

Gate 11 pauses after its fixed-thread measurements, runs Gate 11.5 as an
inserted correctness dependency, and resumes its scaled measurements only
after Gate 11.5 passes.

Each gate's approved scope, evidence, acceptance criteria, planned commits, and
stopping point live in its file under `gates-todo/`. Completed gates move to
`gates-done/`; intentionally postponed gates move to `gates-deferred/` with a
recorded reason. Gate numbers are stable.

## Execution rule

Only one gate is active at a time. The human approved the complete ten-gate
batch and delegated closure of each gate after all its criteria pass with fresh
test evidence. The agent advances automatically in order, but stops when a gate
fails, a dependency is invalidated, or a new decision would change architecture,
scope, risk, or an acceptance criterion. Evidence and limitations are recorded
before each gate moves to `gates-done/`.

## Planned commit shape

Security gates separate reproductions, mechanisms, and integration evidence
where practical. Global-state work separates classification from runtime and
platform/service migrations. Decomposition, synchronization, and SMP use
incremental subsystem commits rather than a single repository-wide rewrite.
All ten gates remain on one branch and one pull request, but no gate is squashed
into another gate's evidence.
