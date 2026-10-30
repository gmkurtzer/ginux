# Feedback-Hardening Plan

## Delivery strategy

- Working branch: `codex/feedback-hardening`
- Intended base: `main`
- Historical baseline: `research-v1-baseline`
- Review policy: stop for human approval after every gate
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
where practical. Global-state work separates classification from runtime and
platform/service migrations. Decomposition, synchronization, and SMP use
incremental subsystem commits rather than a single repository-wide rewrite.
All ten gates remain on one branch and one pull request, but no gate is squashed
into another gate's evidence.
