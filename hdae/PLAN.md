# Feedback-Hardening Plan

## Delivery strategy

- Working branch: `codex/feedback-hardening`
- Intended base: `main`
- Historical baseline: `research-v1-baseline`
- Review policy: stop for human approval after every gate
- Commit policy: small, signed-off commits associated with one gate
- Pull request: use the existing branch review path; verify its remote status
  before the first implementation commit

## Gate sequence

| Gate | Purpose | Depends on |
|---:|---|---|
| 1 | Baseline and claim reproduction | None |
| 2 | Harden x86-64 privilege entry | 1 |
| 3 | Add fault-recovering userspace access | 2 |
| 4 | Establish explicit state ownership | 3 |
| 5 | Add synchronization and SMP readiness | 4 |
| 6 | Decompose the x86-64 implementation | 5 |
| 7 | Implement SMP foundations | 6 |
| 8 | Qualify and compare the result | 7 |

Each gate's approved scope, evidence, acceptance criteria, planned commits, and
stopping point live in its file under `gates-todo/`. Completed gates move to
`gates-done/`; intentionally postponed gates move to `gates-deferred/` with a
recorded reason. Gate numbers are stable.

## Execution rule

Only one gate is active at a time. Before implementation, the human reviews
and approves that gate's scope and criteria. The agent then iterates within the
gate until the criteria pass or a decision is needed. Evidence and limitations
are recorded before the gate moves to `gates-done/`.

## Planned commit shape

Security gates separate reproductions, mechanisms, and integration evidence
where practical. Ownership, synchronization, decomposition, and SMP gates use
incremental subsystem commits rather than a single repository-wide rewrite.
No gate is squashed into another gate's evidence.
