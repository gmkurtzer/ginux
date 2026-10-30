# Gate 1: Baseline and Claim Reproduction

## Goal

Create a reproducible baseline and determine precisely which external review
claims are confirmed, partially confirmed, or rejected before changing code.

## Approved scope

- Capture current verification, Rocky boot, domain, storage, and benchmark
  results from both the baseline tag and working branch.
- Inventory writable global state and classify its current ownership and
  synchronization assumptions.
- Reproduce syscall-entry stack exposure, missing NMI/machine-check ISTs,
  invalid user-pointer behavior, and page-table/bookkeeping disagreement.
- Measure module size and dependency concentration without refactoring it.

## Non-goals

- No security fix, ownership migration, synchronization change, or module
  extraction.
- No acceptance-test weakening.

## Completed work

- Preserved the published artifact as `research-v1-baseline`.
- Confirmed planning-level source anchors in `hdae/ARCHITECTURE.md`.

## Remaining work

- Select and record the complete baseline command matrix.
- Add deterministic negative reproductions and retain their initial outcomes.
- Generate the writable-global inventory, including interior mutability.
- Record confirmed, partial, rejected, and not-yet-reproducible findings.

## Acceptance criteria

- Existing passing checks are reproducible from the baseline tag.
- Every review claim has code references and a deterministic reproduction or a
  documented reason it cannot be reproduced.
- New tests describe current behavior without defining an insecure behavior as
  the desired final result.
- The proposed order and dependencies for Gates 2 through 8 are revalidated
  against the evidence.

## Evidence

- Command transcripts and timings.
- Reproduction tests and source references.
- Generated global-state inventory and classification report.
- Claim disposition table.

## Decisions and limitations

No conclusion is accepted solely because it appeared in the external review.
QEMU injection limits and any untestable hardware behavior must be explicit.

## Delivery strategy

Planned commits: baseline harness/evidence, negative reproductions, and
inventory/reporting. All remain on `codex/feedback-hardening`.

## Review policy and outcome

Checkpoint required. Stop for human review of evidence and approval of the
hardening order. Outcome: pending.
