# Human-Directed Agentic Engineering Method

Use this reference to design HDAE for projects of any size, whether new or
already established.

## Division of responsibility

The human provides:

- the problem worth solving and the intended user outcome;
- domain knowledge and important real-world constraints;
- architectural direction, trust boundaries, and product tradeoffs;
- priorities, non-goals, and acceptable risk;
- the gates authorized for execution and where human review is required.

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

## Iterative discovery

The initial prompt may contain only an idea. The agent is responsible for
turning that idea into a reviewable brief through conversation and repository
inspection.

Ask questions in small rounds. Begin with the outcome, users, and motivation;
then follow the answers into constraints, non-goals, architecture, compatibility,
operations, risks, and evidence. Continue until remaining uncertainty is either
resolved or explicitly recorded as an open decision. Summarize the complete
understanding and obtain correction or confirmation before planning gates.

Do not present a long questionnaire when adaptive follow-up would produce better
answers. Do not fill gaps with plausible assumptions simply to reach coding
faster.

## Minimal artifact model

Use existing repository conventions. Keep these responsibilities distinct when
the project needs them, but do not create documents for ceremony:

- **Product intent:** users, outcomes, scope, non-goals, and success.
- **Architecture:** components, boundaries, invariants, trust, and decisions.
- **Plan:** milestones, dependencies, and order.
- **Gate:** one bounded unit of authorized work and its acceptance criteria.
- **Task:** one trackable action mapped to an approved gate and criterion.
- **Evidence:** commands, environment, observations, failures, and status.
- **Decision record:** a durable explanation for consequential choices.

Link to authoritative material rather than duplicating it.

## Filesystem gate ledger

Every approved gate has exactly one Markdown file. Use this root structure:

```text
hdae/
|-- PROJECT.md
|-- ARCHITECTURE.md
|-- PLAN.md
|-- STATUS.md
|-- gates-todo/
|-- gates-done/
`-- gates-deferred/
```

The path is authoritative metadata:

- `gates-todo/` means the gate is actionable, blocked, failed, or technically
  passed but has not reached its confirmed closure boundary;
- `gates-done/` means the gate passed and was either accepted at a checkpoint or
  closed under a human-confirmed batch delegation; and
- `gates-deferred/` means the human intentionally postponed it and the file
  records why and what should cause reconsideration.

The filename is `<decimal-position>-<short-slug>.md`. Use integers for ordinary
appends and a decimal midpoint for insertion. For example, insert `1.5-auth.md`
between `1-foundation.md` and `2-api.md`; further insertions may use `1.25` or
`1.75`. Use only digits and one decimal point, remove unnecessary trailing
zeros, and never reuse a position. Do not add `id`, lifecycle, or order fields
that repeat the path. If an existing gate must be reordered, rename only that
gate.

Run these commands from the repository root. List every gate in execution order:

```sh
find hdae/gates-* -type f -name '*.md' -print |
    LC_ALL=C sort -t/ -k3,3n
```

Find the earliest open gate:

```sh
find hdae/gates-todo -type f -name '*.md' -print |
    LC_ALL=C sort -t/ -k3,3n |
    sed -n '1p'
```

When appending a gate, derive the next integer from all lifecycle directories,
not only `gates-todo/`. This still works when todo is empty:

```sh
last=$(find hdae/gates-* -type f -name '*.md' -print |
    LC_ALL=C sort -t/ -k3,3n | tail -n 1)
if [ -z "$last" ]; then
    next=1
else
    sequence=${last##*/}
    sequence=${sequence%%-*}
    next=$(awk -v n="$sequence" 'BEGIN { print int(n) + 1 }')
fi
printf '%s\n' "$next"
```

For insertion, calculate a decimal strictly between the neighboring prefixes
and confirm no gate already uses it before creating the file. Technical passage
alone leaves a gate in `gates-todo/`. Move it to `gates-done/` only after human
acceptance at a checkpoint or automatic closure authorized by the confirmed
batch policy. Move it to `gates-deferred/` only after explicit deferral,
retaining the same filename, and return it to `gates-todo/` when the recorded
condition is met.

The gate body owns substantive information, not routing metadata:

```markdown
# <Gate name>

## Goal
<Observable outcome.>

## Approved scope
- <Authorized work.>

## Non-goals
- <Excluded work.>

## Completed work
- [x] <Finished task.>

## Remaining work
- [ ] <Unfinished task or required review.>

## Acceptance criteria
- <Positive, negative, and regression behavior.>

## Evidence
- `<command or procedure>` -> <observed result.>

## Decisions and limitations
- <Material decision, failure, limitation, or deferral condition.>

## Delivery strategy
- Working branch: <current branch or dedicated branch>
- Intended base: <branch or not applicable>
- Pull request: <intended or not intended>
- Planned commits: <ordered coherent commits>
- Actual commits: <commit identifiers or pending>

## Review policy and outcome
<Checkpoint or delegated completion policy, its authorization, and outcome.>
```

Do not create generated todo/done indexes. The lifecycle directories and
numeric shell sort are the ledger. `PLAN.md` contains only project-wide strategy
and conventions; `STATUS.md` contains only the resumable active checkpoint.

## Scale planning to the work

Every task needs a plan discussion before execution, but not every task needs a
large planning system. For a small, bounded change, a concise outcome, scope,
verification, and stop point may be sufficient. For a long project, persist:

- explicit goals and non-goals;
- an ordered gate map with dependencies;
- a task list with pending, active, blocked, and completed states;
- acceptance criteria and required evidence;
- the proposed checkpoint map for any multi-gate execution batch;
- progress checkpoints, risks, decisions, and scope changes; and
- the current next action.

Before starting a long execution loop, tell the human what will run, its scope,
and when progress will be reported. Give an ETA range and confidence only when
there is enough evidence. Otherwise state the uncertainty and the checkpoint
that should make an estimate possible. Revise estimates when observations
invalidate them.

## Traceability and completeness for long projects

Use stable identifiers when they reduce ambiguity. Maintain the relationship:

```text
Goal -> Gate -> Acceptance criterion -> Task -> Evidence
```

Every goal must be covered by one or more gates. Every acceptance criterion
must have planned work and fresh evidence. Every task must belong to an approved
gate and criterion; new tasks that expand scope require a gate update and human
approval. Record removed or changed scope with its reason instead of silently
deleting it.

At planning and completion boundaries, perform four checks:

1. **Forward:** every goal reaches acceptance criteria, tasks, and evidence.
2. **Reverse:** every task and change maps to approved scope.
3. **Coverage:** every relevant functional, failure, regression, user,
   documentation, compatibility, operational, and security surface is included
   or explicitly not applicable.
4. **Orphan:** no goal, criterion, task, decision, failure, or open question is
   left unmapped or silently dropped.

## Durable context and continuation

Chat context is temporary. The repository must contain enough authoritative
state for a new agent or later session to resume safely.

Prefer existing product and architecture sources, linking rather than copying
them. Keep HDAE's own control files under root `hdae/`:

- `PROJECT.md`: intent, users, outcomes, scope, and non-goals;
- `ARCHITECTURE.md`: boundaries, invariants, constraints, and decisions;
- `PLAN.md`: project-wide strategy, dependencies, and planning conventions;
- `STATUS.md`: current gate, last verified evidence, limitations, open
  questions, required decisions, and next action.
- the lifecycle gate directories: completed work, remaining work, criteria,
  evidence, decisions, limitations, and human review for each gate.

Keep one source of truth for each fact. Link evidence and decision records rather
than copying them into every file.

At session start, reload repository instructions and the authoritative HDAE
state. Before a context shift, handoff, or gate stop, update `STATUS.md` or its
equivalent. It must answer:

1. What outcome and invariants govern the work?
2. Which gate is active and what is its exact status?
3. What changed and what evidence actually ran?
4. What remains incomplete, failed, blocked, or unverified?
5. Which human decision or exact next action is required?

Persisted state never overrides a newer explicit human decision. Reconcile the
difference and update the record.

### Context-recovery protocol

Compaction preserves useful task state but is not a verbatim copy of previously
loaded instructions. At task start and after compaction, context reset, resume,
or handoff, recover from durable sources in this order:

1. reload the complete active `hdae-development/SKILL.md` from the project's
   discovery path;
2. reload applicable repository instructions;
3. read each skill reference required for the active operation;
4. read the authoritative project intent, architecture, plan, and checkpoint;
5. reconcile the active gate, scope, status, evidence, open decisions, and next
   action before resuming work; and
6. surface any conflict to the human instead of choosing a version silently.

For long-running work, the checkpoint must include:

```text
Active skill: hdae-development
Skill path: <project discovery path to SKILL.md>
Reload after context shift: required
Active gate: <name and status>
Last evidence: <commands and observations>
Open decisions or limitations: <items or none>
Exact next action: <one resumable action>
```

Keep the bootstrap instruction short and durable. The installer-managed
`AGENTS.md` block exists only to cause this reload; this reference remains the
single detailed recovery procedure. A checkpoint restores project state, while
the reloaded skill restores the workflow and authority boundaries.

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

## Delivery strategy and commit structure

Every newly created gate records how its work will be delivered. Ask the human
whether the gate should use the current branch or a dedicated branch intended
for a pull request, unless an explicit choice already covers that exact gate.
Record:

- working branch and intended base branch when relevant;
- whether a pull request is intended;
- a planned sequence of coherent commits; and
- completed commit identifiers or material deviations from the plan.

Treat a gate as large when it crosses multiple independently reviewable
changes, components, migrations, or risk boundaries. Plan multiple commits for
large gates. Each commit should have one understandable purpose and leave the
tree testable when practical. Do not create noisy partial commits or split one
tightly coupled behavior merely to increase commit count. Repository rules for
sign-off, authorship, testing, and publication remain authoritative. A branch
choice does not authorize pushing or opening a pull request.

## Human gate-alignment conversation

The agent must make the proposed work visible enough for the human to find
omissions before implementation. Use this sequence:

1. summarize the intended outcome and constraints;
2. ask focused questions whose answers could change architecture, scope, or
   acceptance;
3. present the complete proposed gate map and dependencies;
4. show the coverage review below;
5. ask what is missing or incorrectly scoped; and
6. ask which gates may run as one batch and where human review is required.

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

## Multi-gate batches and checkpoint policy

A human may authorize one gate, a numeric range, or all current todo gates. Do
not interpret that request as permission to skip planning or choose review
boundaries silently. Before execution:

1. list the included gates in numeric order and check their dependencies;
2. summarize the work, expected evidence, risks, and batch stopping conditions;
3. propose `checkpoint` or `delegated` review for each gate; and
4. ask the human to confirm the exact batch and checkpoint map.

Recommend `checkpoint` when a gate contains an unresolved product or
architecture choice, subjective user acceptance, security or privacy risk, data
migration, irreversible change, consequential external effect, material scope
change, or an explicitly requested review. Recommend `delegated` only when the
work is objective, reversible, already within approved scope, fully testable,
and requires no new consequential decision. The human may override the proposal
before execution; persist the confirmed policy in each affected gate.

Execute the confirmed batch in numeric order. After each gate:

- record fresh evidence and reconcile every criterion;
- stop if a checkpoint gate passes and request review;
- if a delegated gate passes, record delegated closure, move it to
  `gates-done/`, and continue without asking again;
- stop when failure or blockage invalidates a dependency or later evidence;
- stop for any new decision, permission, scope, risk, or external effect outside
  the confirmed policy; and
- update `STATUS.md` with the current gate and next resumable action.

Do not silently skip a failed gate. Independent later gates may continue only
when the confirmed batch policy explicitly permits that behavior and the
failure cannot invalidate them. At the end, report each gate's passed, closed,
failed, blocked, deferred, or still-open state and the evidence supporting it.
Batch confirmation expires when the included gates finish or the agreed scope
or checkpoint map changes.

## Closed-loop execution

The outer human loop is:

```text
intent -> architecture and constraints -> gate batch and checkpoint map
       -> observed evidence -> agreed review boundary -> direction
```

The inner agent loop is:

```text
implement -> build/run -> observe -> diagnose -> revise -> verify
```

The agent may repeat the inner loop without hand-holding while it remains inside
the approved gate. In a confirmed batch it may advance through passed delegated
gates. It returns to the outer loop at a checkpoint, when the batch ends, when a
human decision is required, or when continued iteration would weaken scope or
evidence.

## Convergence and stall control

An autonomous cycle is productive only when its next action remains inside the
approved gate and has a credible chance of producing measurable progress or new
evidence. Use this decision loop:

```text
observe -> form a hypothesis -> choose the highest-information in-scope action
        -> run it -> compare the result -> record the evidence and next action
```

Do not repeat an unchanged attempt. A retry must deliberately change at least
one hypothesis, input, implementation, environment, or diagnostic, with a
reason that the change could alter the result. When the current approach stalls,
retain its evidence and try a materially different in-scope alternative. Prefer
the action that is most likely to discriminate between explanations or remove a
known blocker, rather than the action that merely consumes more work.

Stop the inner loop and return to the human when any of these conditions holds:

- a required decision, credential, permission, or external dependency is
  missing;
- the remaining alternatives would expand approved scope, risk, authority, or
  consequential external effects;
- repeated materially different cycles yield no new evidence or measurable
  progress;
- the expected time or token cost is disproportionate to the gate's value or
  remaining uncertainty;
- no credible in-scope alternative remains; or
- the gate has passed and reached a confirmed checkpoint, or the batch ended.

Difficulty, an initial failure, or an uncertain diagnosis is not by itself a
reason to stop. Exhaust sensible bounded alternatives first. At escalation,
persist the current checkpoint and report what was attempted, what each result
showed, which alternatives remain, why the loop stopped, and the recommended
human decision or next action. Resume from that checkpoint after direction; do
not silently cross the gate boundary or broaden the plan.

## Completion language

- **Passed:** all mandatory criteria ran and succeeded.
- **Accepted:** the human reviewed and accepted a passed gate.
- **Closed:** a passed gate was accepted or completed under a human-confirmed
  batch delegation.
- **Failed:** at least one mandatory criterion ran and failed.
- **Blocked:** an external dependency or required decision prevents execution.
- **Not run:** verification has not occurred.
- **Partial:** useful progress exists, but the gate remains open.

Time spent, code volume, or near-success does not complete a gate.

At a checkpoint or batch boundary, report the gate and status first, then
delivered work, evidence, coverage, repository or artifact state, incomplete or
unverified items, and the exact decision or next action required from the human.
Passing a gate authorizes the next one only when the confirmed batch policy says
so.
