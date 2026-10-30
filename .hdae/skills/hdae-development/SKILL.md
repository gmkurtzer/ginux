---
name: hdae-development
description: Apply Human-Directed Agentic Engineering by interviewing the human until the project is clear, persisting shared context, and planning, building, verifying, or maintaining software through human-approved gates and agent-executed loops. Use for substantial new or existing engineering work that needs durable architecture, milestones, evidence, and stopping points.
---

# HDAE Development

Human-Directed Agentic Engineering (HDAE) combines informed human direction
with bounded agent autonomy. The human owns product intent, domain decisions,
architecture, constraints, priorities, and review boundaries. The agent turns
that direction into artifacts and code, runs repeatable implementation and
diagnosis loops, and presents evidence at agreed boundaries.

HDAE is not continual approval of individual edits, and it is not unguided
"vibe coding." Give the agent room to iterate inside an approved gate while
returning architectural and product decisions to the human.

Read repository instructions and inspect the current project before proposing
new structure. Preserve useful existing plans, tests, terminology, and decision
records rather than creating parallel documents.

## Select the operation

- **Establish:** turn an idea into product intent, architecture, constraints,
  milestones, and executable gates.
- **Implement:** execute the approved gate or ordered gate batch.
- **Verify:** independently determine whether a gate passed.
- **Maintain:** convert defects, requests, and operational evidence into new
  bounded gates without losing the original architecture.
- **Retrospective:** examine how direction, evidence, and iteration affected the
  outcome and improve the process.

If the operation or current gate is unclear, establish it with the human before
making substantial changes.

## Discovery interview

Start Establish mode by interviewing the human. Do not require the human to
arrive with a completed brief or fill out a template.

1. Ask a small first round of the highest-value questions about the problem,
   intended outcome, users, and why it matters.
2. Use each answer to ask focused follow-ups about architecture, constraints,
   non-goals, risks, existing behavior, and proof of success.
3. Inspect the repository between question rounds so questions reflect the
   actual system rather than generic assumptions.
4. Continue until the answers and repository evidence are sufficient to propose
   a coherent project brief and gate map. Do not stop questioning merely because
   every topic has been mentioned once.
5. Summarize the resulting understanding in plain language and ask the human to
   correct it before treating it as authoritative.

Prefer one to three related questions at a time. Explain why a question changes
the architecture, scope, or acceptance criteria when that is not obvious.

## Discuss the plan before work

Before doing task work, discuss the plan with the human. At minimum, state the
intended outcome, work to perform, scope and non-goals, acceptance evidence, and
stopping point. This discussion always occurs; scale its detail to the work. A
small task may need only a short summary, while a long project needs explicit
goals, an ordered plan, dependencies, gates, and a task list with current
statuses.

For work expected to take a long time, tell the human when the autonomous inner
loop will begin, what it will cover, and when progress checkpoints will occur.
Give an ETA range and confidence when evidence supports one. If a useful ETA is
not yet possible, say why and identify the checkpoint at which it can be
reassessed. Update the plan and estimate when evidence changes; do not present
speculation as a commitment.

## Converge without spinning

Continue the autonomous inner loop while the approved gate is incomplete, the
next action remains in scope, and each cycle has a credible chance of producing
measurable progress or new evidence. Never repeat an unchanged attempt. Every
retry must change a hypothesis, input, implementation, environment, or
diagnostic and explain why that change could help.

When an approach stalls, preserve what was tried and choose a materially
different in-scope action with the highest expected information value. Stop and
return to the human when a required decision, credential, permission, or
dependency is missing; remaining alternatives would expand scope or risk;
repeated cycles produce no new evidence or progress; expected time or token cost
is disproportionate to the gate; or no credible in-scope alternative remains.
Report the attempts, evidence, alternatives, and recommendation, and persist the
checkpoint. Difficulty alone is not a blocker; exhaust sensible bounded
alternatives first.

## Recover context before continuing

At task start and after any compaction, context reset, resume, or handoff,
reload the complete active copy of this `SKILL.md` before task work continues.
Then reload repository instructions, any reference this skill requires for the
active operation, and the authoritative HDAE project checkpoint. Compacted chat
or a remembered summary is not a substitute for those sources.

Confirm the active gate, scope, status, evidence, open decisions, and exact next
action against the checkpoint before resuming the inner loop. Every long-running
checkpoint must identify the active skill, its project discovery path, and that
reload is required after a context shift. Read the recovery procedure in
[references/hdae-method.md](references/hdae-method.md) when establishing or
resuming durable project work.

## Maintain the filesystem gate ledger

Every approved gate must have one authoritative Markdown file under the
project's root `hdae/` directory. The directory is lifecycle state:
`gates-todo/` contains open work, `gates-done/` contains passed gates closed by
human review or confirmed batch delegation, and `gates-deferred/` contains
intentionally postponed gates. The leading decimal filename component is
execution position. Do not duplicate path-derived lifecycle or order in gate
indexes or metadata fields.

Keep completed work, remaining work, criteria, evidence, decisions, limitations,
and review policy in that gate file. A technically passed gate remains in
`gates-todo/` until it reaches its confirmed review boundary: explicit human
acceptance for a checkpoint gate or automatic closure for a delegated gate.
Move a gate to `gates-deferred/` only with the reason and return condition
recorded; move it back to `gates-todo/` when reactivated. Read the gate-ledger
procedure and shell commands in
[references/hdae-method.md](references/hdae-method.md) before creating,
inserting, selecting, deferring, completing, or migrating gates.

## Align batches and checkpoints

When the human requests several gates, a range, or all todo gates, inspect them
in numeric order and propose one execution batch before starting. Show the
included gates, dependencies, expected work, acceptance evidence, and a
checkpoint map. Recommend a checkpoint for gates involving unresolved product
or architecture decisions, subjective user acceptance, security or privacy
risk, migrations, irreversible or consequential external effects, material
scope changes, or any boundary the human requested. Recommend automatic
completion only for objective, reversible, in-scope work with complete criteria
and no new consequential decision.

Ask the human to confirm both the batch and checkpoint map. That confirmation
may delegate closure of passed non-checkpoint gates and advancement to the next
gate in this batch only. It does not authorize future gates, failed criteria,
scope expansion, consequential external action, or bypassing a required
permission. If the conversation already confirms the exact batch and map, do
not ask again. Read the multi-gate procedure in
[references/hdae-method.md](references/hdae-method.md) before running a batch.

## Mandatory gate alignment

Before implementation, align the gates with the human. Skip this only when the
current conversation already contains explicit approval of the exact gate or
batch, its acceptance criteria, and its checkpoint map.

1. Restate the outcome and important constraints in plain language.
2. Ask focused questions about missing product behavior, architecture,
   compatibility, operations, user experience, documentation, tests, and
   acceptance. Ask only questions that could change scope or a gate.
3. Inspect the relevant repository and existing evidence before finalizing the
   plan.
4. Present a proposed gate map in plain, concrete language. For every gate show
   its goal, observable outcome, included and excluded work, dependencies,
   deliverables, acceptance evidence, and proposed review policy. For
   substantial work, also show the ordered plan and task list that implement the
   gates.
5. Perform a coverage review across functionality, failure behavior,
   regression testing, user workflow, documentation, compatibility or
   migration, and operational or security concerns when relevant. Mark a
   category not applicable rather than silently omitting it.
6. Explicitly ask the human: “What is missing or incorrectly scoped, which
   gates may run, and where should I stop for review?”
7. Wait for approval. Do not treat silence, general enthusiasm, or approval of
   the overall idea as approval of a gate batch or checkpoint map.

When the human adds or changes a requirement, update the affected gate map and
acceptance criteria. If the change is material, obtain approval before
continuing implementation.

## Persist shared understanding

Do not rely on chat history as the project record. During Establish mode,
identify the repository's existing authoritative documents and propose where
the HDAE state will live. After the human confirms the summarized understanding,
write or update durable repository files covering:

- project intent, users, outcomes, scope, and non-goals;
- architecture, constraints, invariants, and important decisions;
- milestone and gate map with status, dependencies, and acceptance criteria;
- for long projects, traceability from goals through gates, criteria, tasks, and
  evidence, plus a record of approved scope changes;
- the active checkpoint: current gate, evidence, limitations, open questions,
  required human decisions, exact next action, active skill and discovery path,
  and whether skill reload is required after a context shift.

Use existing authoritative product and architecture documents when possible,
but keep the HDAE control plane under root `hdae/`: `PROJECT.md`,
`ARCHITECTURE.md`, `PLAN.md`, `STATUS.md`, and the three gate lifecycle
directories. Keep one source of truth for each fact and link existing project
documents rather than copying them.

For substantial or long-running work, read
[references/hdae-method.md](references/hdae-method.md) and use its scalable
planning, traceability, and completeness-audit guidance. Do not rely on a chat
plan or transient task UI as the authoritative project record.

At the beginning of every resumed task or after a context shift, read the
repository instructions and these authoritative HDAE files before proposing or
executing work. At every gate boundary and before handing off, update the active
checkpoint so another agent can continue without reconstructing decisions from
conversation history. If chat and persisted state disagree, surface the conflict
to the human instead of choosing silently.

When version control is available, include the state update with the related
gate commit when authorized and consistent with repository practice. Otherwise,
report the uncommitted state files explicitly; never imply that chat alone is a
durable handoff.

## Establish the project

1. Run the discovery interview until the desired outcome, users, non-goals,
   constraints, domain assumptions, and definition of success are clear enough
   for human confirmation.
2. Inspect the repository or intended environment and identify existing
   behavior, architecture, tests, and unresolved risks.
3. Persist the confirmed understanding and active checkpoint using the smallest
   useful set of authoritative repository documents.
4. Establish a reproducible baseline. Use a branch, tag, or checkpoint when it
   materially improves comparison or recovery and the human authorizes it.
5. Divide the work into reviewable milestones. Each gate must name deliverables,
   positive and negative acceptance criteria, regression checks, evidence, and
   a stopping point.
6. Complete the mandatory gate-alignment conversation. Planning alone does not
   authorize implementation.

Read [references/hdae-method.md](references/hdae-method.md) when designing or
assessing the project process.

## Implement an approved gate or batch

1. Restate the human-approved gate or batch, checkpoint map, scope, invariants,
   and acceptance commands, and create or locate each `gates-todo/` record. If
   any are missing, return to gate alignment.
2. Reproduce the current behavior or missing capability when practical.
3. Implement the narrowest coherent change that can satisfy the gate.
4. Iterate autonomously within that boundary under the convergence rules:
   build, run, observe, diagnose, revise, and rerun.
5. Keep the human informed at the promised checkpoints. Update task status,
   plan, risks, and ETA when actual evidence changes them.
6. Turn discovered regressions into repeatable tests rather than fixing only
   the visible symptom.
7. Preserve failures, limitations, commands, and material decisions as evidence.
8. Reconcile every deliverable and acceptance criterion against fresh evidence,
   including documentation and user-facing behavior. For long projects, perform
   the reference's forward, reverse, coverage, and orphan checks. Do not declare
   the gate complete because its main code path works.
9. Reconcile source, generated, vendored, and installed copies when relevant;
   inspect documentation, repository state, commits, and temporary artifacts.
10. Update the gate file's completed work, remaining work, evidence, decisions,
    limitations, review policy and outcome, and exact next action, plus
    `STATUS.md`.
11. At a checkpoint gate, present the completion summary and stop for human
    review. At a delegated gate, move it to `gates-done/` only after it passes,
    record that it closed under the confirmed batch policy, and continue to the
    next authorized gate. At batch completion, summarize every gate and stop.

Return immediately for human direction when work would change the architecture,
expand product scope, weaken an acceptance criterion, require consequential
external action, or choose among materially different product outcomes.

## Verify and maintain

Verify observable behavior from a clean and relevant state. A gate is
**passed** only when every mandatory criterion ran and succeeded; otherwise use
**failed**, **blocked**, **not run**, or **partial** precisely.
**Accepted** means the human reviewed and accepted a passed checkpoint gate.
**Closed** means a passed gate was either accepted or completed under a
human-confirmed batch delegation. Never imply that delegated closure was
individual human review.

A gate cannot pass until its authoritative file records every criterion and the
fresh evidence that satisfied it. Technical passage alone leaves the file in
`gates-todo/`; acceptance or confirmed delegated closure moves it to
`gates-done/`.

For maintenance, connect each issue or request to the authoritative product and
architecture documents. Add the smallest gate that reproduces the problem,
protects the intended behavior, and records the decision. Do not let accumulated
patches silently redefine the system.

## Retrospective

Identify which outcomes came from human direction, agent proposals, executable
evidence, or correction after failure. Improve documents, tests, gates, or
repository instructions only when the evidence shows a recurring need.
