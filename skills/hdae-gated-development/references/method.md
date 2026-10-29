# HDAE Method and Gate Design

Use this reference when creating or evaluating a substantial engineering plan.

## Artifact responsibilities

Prefer the repository's existing conventions. When these concepts need
distinct artifacts, keep their responsibilities separate:

- **Architecture:** system boundaries, invariants, trust model, and important
  decisions.
- **Goal:** desired outcome, scope, non-goals, and completion definition.
- **Plan:** ordered phases and dependencies.
- **Gate:** one phase's authorized work and acceptance criteria.
- **Context:** discoveries, decisions, and unresolved questions needed by that
  phase.
- **Evidence log:** commands, observed results, failures, and review status.

Do not create every artifact automatically or duplicate the same facts among
them. Link to the authoritative source.

## Qualities of a useful gate

A gate should be:

- **Observable:** success is visible in system behavior or a meaningful
  artifact.
- **Executable:** verification names concrete commands, scenarios, or review
  procedures.
- **Falsifiable:** negative cases can show that the implementation is wrong.
- **Bounded:** the gate can finish and be reviewed independently.
- **Regressive:** previously demonstrated capabilities are rechecked.
- **Evidence-bearing:** another person can see what ran and what happened.
- **Decision-aware:** unresolved choices are surfaced before implementation.

Avoid gates such as “improve quality,” “finish compatibility,” or “make it
secure.” Replace them with measurable properties and explicit limitations.

## Gate template

```markdown
## Gate: <outcome>

Status: proposed | approved | in progress | passed | failed | blocked

### Purpose
<Why this gate exists and which risk or capability it addresses.>

### Scope
- <Authorized change>

### Non-goals
- <Explicitly deferred work>

### Deliverables
- <Code, tests, documentation, or operational behavior>

### Acceptance criteria
- <Observable positive behavior>
- <Negative or failure behavior>
- <Regression behavior>

### Evidence
- `<command>` -> <required observation>

### Stop
Stop for human review. Do not begin <dependent gate> until accepted.
```

## Iteration loop

For each gate:

1. verify the current state;
2. reproduce the missing behavior or defect;
3. specify the intended invariant;
4. implement within the approved boundary;
5. test positive, negative, and regression paths;
6. record evidence and limitations;
7. stop for review.

When a real workload exposes a failure that synthetic tests missed, first turn
the observed behavior into a repeatable regression test. Do not patch only the
visible transcript.

## Completion language

Use precise statuses:

- **Passed:** all mandatory criteria ran and succeeded.
- **Failed:** at least one mandatory criterion ran and failed.
- **Blocked:** an external dependency or required decision prevents execution.
- **Not run:** verification has not been performed.
- **Partial:** useful work exists, but the gate remains open.

Near-success, time spent, or a passing subset does not make a gate complete.
