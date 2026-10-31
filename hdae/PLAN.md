# Feedback-Hardening Plan

## Delivery strategy

- Working branch: `codex/feedback-hardening`
- Intended base: `main`
- Historical baseline: `research-v1-baseline`
- Review policy: delegated closure for all ten gates after complete fresh test
  evidence; stop only for a failed dependency, architectural decision, scope
  expansion, or consequential action outside the approved batch. Gate 11 was
  added afterward as delegated measurement work; inserted correctness gates
  11.5 and 11.6 used explicit human checkpoints.
- Commit policy: small, signed-off commits associated with one gate
- Pull request: one pull request from `codex/feedback-hardening` to `main` for
  the entire secondary experiment; verify its remote status before Gate 1 work

Gate 12 is a separate follow-up delivery:

- Working branch: `codex/performance-parity`
- Intended base: merged `main` containing PR #7
- Review policy: autonomous iteration inside the approved gate, followed by a
  human checkpoint after all objective criteria pass
- Pull request: a new follow-up PR, not PR #7
- Commit policy: signed, coherent profiling, mechanism, regression, and final
  evidence commits

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
| 11.6 | Preserve exact Linux thread context across SMP returns | Gate 11 fault reproduction |
| 12 | Reach matched UP and SMP performance parity | 11, 11.5, 11.6 |

Gate 11 paused twice when measurement exposed correctness failures. Gates 11.5
and 11.6 corrected those failures and retained regressions before the scaled
matrix resumed.

Gate 12 uses the completed Gate 11 matrix as its historical baseline, collects
fresh paired measurements, profiles the largest costs, and iterates on general
kernel mechanisms until every fixed and scaled sysbench cell reaches at least
0.96x matched Linux performance. The target remains greater than 1.00x in every
cell.

Each gate's approved scope, evidence, acceptance criteria, planned commits, and
stopping point live in its file under `gates-todo/`. Completed gates move to
`gates-done/`; intentionally postponed gates move to `gates-deferred/` with a
recorded reason. Gate numbers are stable.

## Execution rule

Only one gate is active at a time. The human approved the initial ten-gate batch
and delegated closure of each gate after all its criteria pass with fresh test
evidence. Later approved gates follow their own recorded review policies. The
agent advances automatically in order, but stops when a gate fails, a dependency
is invalidated, or a new decision would change architecture, scope, risk, or an
acceptance criterion. Evidence and limitations are recorded before each gate
moves to `gates-done/`.

## Planned commit shape

Security gates separate reproductions, mechanisms, and integration evidence
where practical. Global-state work separates classification from runtime and
platform/service migrations. Decomposition, synchronization, and SMP use
incremental subsystem commits rather than a single repository-wide rewrite.
All gates remain on one branch and one pull request, but no gate is squashed
into another gate's evidence.
