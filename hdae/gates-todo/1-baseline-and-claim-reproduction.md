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

- Freeze the baseline command matrix, starting with `make check`, `make test`,
  `make test-linux-abi`, `make test-linux-runtime`, `make test-multi-domain`,
  `make test-rocky-systemd`, `make test-storage`, and
  `make test-domain-responsiveness`, plus the existing matched benchmark suite.
- Add deterministic negative reproductions and retain their initial outcomes.
- Generate the writable-global inventory, including interior mutability.
- Record confirmed, partial, rejected, and not-yet-reproducible findings.

## Acceptance criteria

- Existing passing checks are reproducible from the baseline tag.
- Every review claim has code references and a deterministic reproduction or a
  documented reason it cannot be reproduced.
- New tests describe current behavior without defining an insecure behavior as
  the desired final result.
- The proposed order and dependencies for Gates 2 through 10 are revalidated
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

- Working branch: `codex/feedback-hardening`.
- Intended base: `main`.
- Pull request: one pull request for all ten gates.
- Planned commits: baseline harness/evidence, negative reproductions, and
  inventory/reporting.
- Actual commits: pending.

## Review policy and outcome

Delegated closure authorized for the approved ten-gate batch after all criteria
pass with fresh evidence. Outcome: in progress.
