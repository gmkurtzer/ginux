---
name: hdae-gated-development
description: Design, execute, verify, or review Human-Directed Agentic Engineering work using explicit architecture, phased plans, executable acceptance gates, evidence, and human stopping points. Use when a user asks to establish a gated engineering process, turn a substantial goal into milestones, implement an approved gate, or assess whether a gate has actually passed. Do not use for an ordinary isolated edit unless the user requests this process.
---

# HDAE Gated Development

Apply Human-Directed Agentic Engineering without turning it into ceremony. Keep
the human responsible for intent, architecture, boundaries, and acceptance;
use the agent for implementation, diagnosis, verification, and iteration.

Read applicable repository instructions and inspect the live repository before
proposing artifacts or work. Preserve its existing architecture, terminology,
plans, tests, and approval boundaries.

## Select the operation

- **Plan:** convert an idea, requirement, or review finding into architecture,
  milestones, gates, and stopping points. Planning does not authorize
  implementation.
- **Implement:** work only on the current user-approved gate. Do not begin the
  following gate.
- **Verify:** determine from fresh evidence whether a gate passed. Do not treat
  implementation claims or documentation status as proof.
- **Retrospective:** compare the plan with the recorded execution, retain
  failures and corrections, and recommend focused process improvements.

If the requested operation or authorization boundary is materially ambiguous,
ask before changing the repository.

## Plan

1. Establish the outcome, relevant architecture, non-goals, constraints, and
   unresolved decisions through focused Q&A.
2. Inspect existing design documents, plans, tests, and repository history;
   choose one authoritative location for each kind of information.
3. Define a reproducible baseline. Propose a tag or branch only when useful,
   and create it only with authorization.
4. Divide the work into the smallest reviewable phases that produce meaningful
   evidence.
5. For every phase, define deliverables, observable acceptance criteria,
   negative tests, required evidence, regression checks, and a stopping point.
6. Show dependencies between gates. A failed gate must block dependent work.
7. Present the plan for approval before implementation.

For the detailed method and gate-quality checklist, read
[references/method.md](references/method.md).

## Implement

1. Confirm the active gate and its approved scope.
2. Reproduce the current behavior or failure before changing it when practical.
3. Make the narrowest coherent change that satisfies the gate.
4. Test proportionally to risk, including the gate's negative and regression
   cases.
5. Record commands, results, failures, limitations, and material decisions in
   the repository's existing evidence location.
6. Commit with the repository's required convention when authorized.
7. Stop and report at the gate boundary. Do not silently continue because the
   next phase appears straightforward.

Preserve unrelated user changes. Never represent skipped, unavailable, or
failing checks as passing.

## Verify

- Run the acceptance commands from a clean and relevant state.
- Inspect the resulting behavior, not merely exit codes or expected strings.
- Check negative paths and regression surfaces.
- Distinguish **passed**, **failed**, **blocked**, and **not run**.
- Cite the evidence supporting every status.
- Mark a gate complete only when every mandatory criterion passes.

## Retrospective

Identify where human direction, architecture, tests, or agent behavior changed
the outcome. Retain rejected approaches and regressions when they explain a
material decision. Recommend changes to repository instructions, plans, tests,
or this skill only when the evidence shows the change will recur and improve
future work.
