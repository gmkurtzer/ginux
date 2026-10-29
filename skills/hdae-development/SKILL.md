---
name: hdae-development
description: Apply Human-Directed Agentic Engineering to plan, build, verify, or maintain software through human-owned architecture and acceptance decisions plus agent-executed implementation loops. Use when a user wants to structure substantial new or existing engineering work with explicit documents, milestones, gates, evidence, and stopping points.
---

# HDAE Development

Human-Directed Agentic Engineering (HDAE) combines informed human direction
with bounded agent autonomy. The human owns product intent, domain decisions,
architecture, constraints, priorities, and acceptance. The agent turns that
direction into artifacts and code, runs repeatable implementation and diagnosis
loops, and presents evidence at agreed boundaries.

HDAE is not continual approval of individual edits, and it is not unguided
"vibe coding." Give the agent room to iterate inside an approved gate while
returning architectural and product decisions to the human.

Read repository instructions and inspect the current project before proposing
new structure. Preserve useful existing plans, tests, terminology, and decision
records rather than creating parallel documents.

## Select the operation

- **Establish:** turn an idea into product intent, architecture, constraints,
  milestones, and executable gates.
- **Implement:** execute only the active human-approved gate.
- **Verify:** independently determine whether a gate passed.
- **Maintain:** convert defects, requests, and operational evidence into new
  bounded gates without losing the original architecture.
- **Retrospective:** examine how direction, evidence, and iteration affected the
  outcome and improve the process.

If the operation or current gate is unclear, establish it with the human before
making substantial changes.

## Mandatory gate alignment

Before implementation, align the gates with the human. Skip this only when the
current conversation already contains an explicit approval of the exact active
gate and its acceptance criteria.

1. Restate the outcome and important constraints in plain language.
2. Ask focused questions about missing product behavior, architecture,
   compatibility, operations, user experience, documentation, tests, and
   acceptance. Ask only questions that could change scope or a gate.
3. Inspect the relevant repository and existing evidence before finalizing the
   plan.
4. Present a proposed gate map. For every gate show its outcome, included and
   excluded work, dependencies, acceptance evidence, and stopping point.
5. Perform a coverage review across functionality, failure behavior,
   regression testing, user workflow, documentation, compatibility or
   migration, and operational or security concerns when relevant. Mark a
   category not applicable rather than silently omitting it.
6. Explicitly ask the human: “What is missing or incorrectly scoped, and which
   gate may begin?”
7. Wait for approval. Do not treat silence, general enthusiasm, or approval of
   the overall idea as approval to implement every gate.

When the human adds or changes a requirement, update the affected gate map and
acceptance criteria. If the change is material, obtain approval before
continuing implementation.

## Establish the project

1. Use focused Q&A to understand the desired outcome, users, non-goals,
   constraints, domain assumptions, and definition of success.
2. Inspect the repository or intended environment and identify existing
   behavior, architecture, tests, and unresolved risks.
3. Collaboratively record only the documents the work needs: typically product
   intent, architecture and invariants, a phased plan, and acceptance gates.
4. Establish a reproducible baseline. Use a branch, tag, or checkpoint when it
   materially improves comparison or recovery and the human authorizes it.
5. Divide the work into reviewable milestones. Each gate must name deliverables,
   positive and negative acceptance criteria, regression checks, evidence, and
   a stopping point.
6. Complete the mandatory gate-alignment conversation. Planning alone does not
   authorize implementation.

Read [references/hdae-method.md](references/hdae-method.md) when designing or
assessing the project process.

## Implement an approved gate

1. Restate the human-approved active gate, scope, invariants, and acceptance
   commands. If any of those are missing, return to gate alignment.
2. Reproduce the current behavior or missing capability when practical.
3. Implement the narrowest coherent change that can satisfy the gate.
4. Iterate autonomously within that boundary: build, run, observe, diagnose,
   revise, and rerun.
5. Turn discovered regressions into repeatable tests rather than fixing only
   the visible symptom.
6. Preserve failures, limitations, commands, and material decisions as evidence.
7. Reconcile every deliverable and acceptance criterion against fresh evidence,
   including documentation and user-facing behavior. Do not declare the gate
   complete because its main code path works.
8. Stop at the gate boundary and let the human accept, reject, redirect, or
   reprioritize the next work.

Return immediately for human direction when work would change the architecture,
expand product scope, weaken an acceptance criterion, require consequential
external action, or choose among materially different product outcomes.

## Verify and maintain

Verify observable behavior from a clean and relevant state. A gate is
**passed** only when every mandatory criterion ran and succeeded; otherwise use
**failed**, **blocked**, **not run**, or **partial** precisely.

For maintenance, connect each issue or request to the authoritative product and
architecture documents. Add the smallest gate that reproduces the problem,
protects the intended behavior, and records the decision. Do not let accumulated
patches silently redefine the system.

## Retrospective

Identify which outcomes came from human direction, agent proposals, executable
evidence, or correction after failure. Improve documents, tests, gates, or
repository instructions only when the evidence shows a recurring need.
