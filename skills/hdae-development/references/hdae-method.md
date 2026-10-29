# Human-Directed Agentic Engineering Method

Use this reference to design HDAE for projects of any size, whether new or
already established.

## Division of responsibility

The human provides:

- the problem worth solving and the intended user outcome;
- domain knowledge and important real-world constraints;
- architectural direction, trust boundaries, and product tradeoffs;
- priorities, non-goals, and acceptable risk;
- acceptance of completed gates and direction for the next gate.

The agent provides:

- repository and implementation analysis;
- draft product, architecture, plan, and gate artifacts;
- implementation inside an approved boundary;
- automated builds, tests, experiments, diagnosis, and iteration;
- evidence, limitations, alternatives, and surfaced decisions.

Shared work includes Q&A, architecture refinement, gate definition, review of
observed behavior, and deciding what evidence is sufficient. Agent proposals do
not become architecture merely because they are plausible; human preferences do
not make a gate pass without evidence.

## Minimal artifact model

Use existing repository conventions. Keep these responsibilities distinct when
the project needs them, but do not create documents for ceremony:

- **Product intent:** users, outcomes, scope, non-goals, and success.
- **Architecture:** components, boundaries, invariants, trust, and decisions.
- **Plan:** milestones, dependencies, and order.
- **Gate:** one bounded unit of authorized work and its acceptance criteria.
- **Evidence:** commands, environment, observations, failures, and status.
- **Decision record:** a durable explanation for consequential choices.

Link to authoritative material rather than duplicating it.

## Gate quality

A useful gate is:

- **Observable:** success appears in behavior or a meaningful artifact.
- **Executable:** verification names concrete commands or procedures.
- **Falsifiable:** negative cases can prove the work wrong.
- **Bounded:** it can finish and be reviewed independently.
- **Regressive:** earlier capabilities are rechecked.
- **Evidence-bearing:** another person can see what ran and what happened.
- **Decision-aware:** unresolved human choices are surfaced before coding.

Avoid gates such as “improve quality,” “finish compatibility,” or “make it
secure.” Replace them with specific properties and limitations.

## Human gate-alignment conversation

The agent must make the proposed work visible enough for the human to find
omissions before implementation. Use this sequence:

1. summarize the intended outcome and constraints;
2. ask focused questions whose answers could change architecture, scope, or
   acceptance;
3. present the complete proposed gate map and dependencies;
4. show the coverage review below;
5. ask what is missing or incorrectly scoped; and
6. ask which specific gate is approved to begin.

The coverage review considers each relevant surface:

- functional behavior and failure paths;
- automated tests, regression checks, and required evidence;
- user workflows, interfaces, and discoverability;
- documentation and examples;
- compatibility, migration, persistence, and rollback;
- deployment, monitoring, maintenance, and recovery; and
- performance, security, privacy, and other nonfunctional constraints.

Not every project needs a gate for every surface. The agent must either include
the relevant work or state why it is not applicable. This is a completeness
check, not permission to expand the user's goal.

At gate completion, repeat the coverage review against the approved scope and
show evidence for every acceptance criterion. Missing or unverified work keeps
the gate partial.

## Gate template

```markdown
## Gate: <observable outcome>

Status: proposed | approved | in progress | passed | failed | blocked

### Purpose
<User, product, architectural, or risk reason.>

### Scope
- <Authorized work>

### Non-goals
- <Deferred or prohibited work>

### Deliverables
- <Code, tests, documentation, or behavior>

### Acceptance criteria
- <Positive behavior>
- <Negative or failure behavior>
- <Regression behavior>

### Evidence
- `<command or procedure>` -> <required observation>

### Stop
Stop for human review before beginning <dependent work>.
```

## Closed-loop execution

The outer human loop is:

```text
intent -> architecture and constraints -> gate -> observed evidence
       -> human review and direction -> next gate
```

The inner agent loop is:

```text
implement -> build/run -> observe -> diagnose -> revise -> verify
```

The agent may repeat the inner loop without hand-holding while it remains inside
the approved gate. It returns to the outer loop when the gate passes, a human
decision is required, or continued iteration would weaken scope or evidence.

## Completion language

- **Passed:** all mandatory criteria ran and succeeded.
- **Failed:** at least one mandatory criterion ran and failed.
- **Blocked:** an external dependency or required decision prevents execution.
- **Not run:** verification has not occurred.
- **Partial:** useful progress exists, but the gate remains open.

Time spent, code volume, or near-success does not complete a gate.
