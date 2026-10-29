---
name: hdae-review-scheduler
description: Design, dry-run, configure, and verify recurring pull-request and issue reviews using Human-Directed Agentic Engineering gates. Use when a maintainer asks to establish scheduled repository review or demonstrate an HDAE-governed automation. Default to read-only review and stop at each gate for approval.
---

# HDAE Review Scheduler

Turn recurring repository review into a bounded HDAE workflow. Keep review
judgment evidence-based and keep all repository mutations under maintainer
control.

Read the repository instructions first. Then read the canonical
[HDAE gate method](../hdae-gated-development/references/method.md) and
[maintainer-triage workflow](../maintainer-triage/SKILL.md). Treat both as
governing requirements rather than copying their contents into the schedule.

## Select the operation

- **Plan:** identify the repository, review scope, cadence, timezone, report
  destination, notification preference, and authority boundary.
- **Dry run:** perform one read-only review with the proposed prompt and review
  window.
- **Configure:** create or update the recurring schedule only after the dry-run
  gate passes and the maintainer approves the exact schedule and prompt.
- **Verify:** inspect the saved schedule and evaluate its first real run against
  the acceptance criteria.

If the user does not specify an operation, begin with Plan. Never silently
advance across gates.

## Plan

1. Resolve the exact repository and confirm that the available GitHub access is
   read-only or otherwise appropriately scoped.
2. Obtain the cadence and timezone. Do not infer a business-hours schedule from
   the system clock.
3. Define which issues and pull requests are in scope and how far each run
   should look back. Use an overlap window so delayed updates are not missed.
4. Define where the report should appear and which results warrant a
   notification.
5. Default the automation to analysis and response drafts only. Posting,
   labeling, closing, reviewing, merging, committing, and pushing remain out of
   scope unless separately authorized at execution time.
6. Present the four gates and the proposed recurring prompt for approval.

Read [references/review-automation.md](references/review-automation.md) for the
gate criteria, prompt template, and report contract.

## Dry run

Perform the proposed review once without creating a schedule. Use the same
repository, scope, lookback, and output contract intended for recurring runs.
Do not execute code from untrusted pull-request branches. Record the query
window and distinguish confirmed findings from claims that need information.

The dry-run gate passes only when repository access works, relevant activity is
retrieved, unchanged noise is suppressed, and the report contains actionable
evidence or an explicit no-material-activity result. Stop for review.

## Configure

Use the environment's supported recurring automation or scheduler. Prefer a
native project-bound scheduler when available; otherwise propose a repository
workflow or host scheduler and obtain approval before adding it.

Create exactly the approved cadence, timezone, target, and prompt. Do not embed
credentials in prompts or repository files. The scheduled task must remain
read-only and must not run contribution-controlled code with privileged
credentials.

After creation, inspect the saved configuration and report its identifier,
status, next run, target repository, and authorization boundary. Stop before
claiming operational success.

## Verify

Evaluate the first scheduled run, not a simulated success message. Confirm that
it ran on schedule, inspected the intended window, followed the report
contract, avoided external mutation, and surfaced errors honestly. Mark the
gate passed, failed, blocked, or not run with evidence.

Do not broaden permissions or repair a failing schedule by weakening its safety
or evidence requirements.
