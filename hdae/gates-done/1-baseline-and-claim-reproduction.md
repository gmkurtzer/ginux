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

- [x] Preserved the published artifact as `research-v1-baseline`.
- [x] Ran the frozen baseline command matrix and retained timings and failures.
- [x] Added a deterministic source-based claim reproduction.
- [x] Added a machine-readable writable-global inventory.
- [x] Classified every external review claim as confirmed or rejected with
  source evidence.
- [x] Revalidated Gates 2 through 10 against the observed entry, user-copy,
  ownership, structure, and concurrency evidence.

## Remaining work

- None.

## Acceptance criteria

- Existing passing checks are reproducible from the baseline tag.
- Every review claim has code references and a deterministic reproduction or a
  documented reason it cannot be reproduced.
- New tests describe current behavior without defining an insecure behavior as
  the desired final result.
- The proposed order and dependencies for Gates 2 through 10 are revalidated
  against the evidence.

## Evidence

- `docs/feedback-hardening/GATE-1-BASELINE.md` records the command results,
  timings, limitations, and claim disposition.
- `tools/architecture/reproduce-hardening-feedback.sh` deterministically
  confirms the unsafe entry ordering, missing NMI/machine-check ISTs, and
  nonrecovering user-copy path; it rejects the cited bookkeeping fallback.
- `tools/architecture/inventory-writable-globals.sh` reports 82 `static mut`
  declarations and 535 recognized interior-mutable declarations.
- `make check` passed.
- `make test-linux-abi` passed for 23 interfaces and 375 syscalls.
- `make test-storage`, `make test-domain-responsiveness`, and
  `make test-rocky-systemd` passed.
- `make benchmark-compare` reproduced the existing matched single-vCPU data.
- `research-v1-baseline` and the branch both reproduce a boot-suite failure;
  the branch now advances through PID 1 and exposes a later invalid
  thread-return context. `make test-multi-domain` was separately prevented by
  an OCI registry transfer failure. Both are retained rather than hidden.

## Decisions and limitations

No conclusion is accepted solely because it appeared in the external review.
QEMU cannot deterministically deliver a real machine check at every instruction
window, so the entry-window finding combines source-order proof with the IDT/TSS
reproduction. The published baseline itself does not pass `make test`; the
branch's baseline repairs reveal rather than create the later thread failure.

## Delivery strategy

- Working branch: `codex/feedback-hardening`.
- Intended base: `main`.
- Pull request: one pull request for all ten gates.
- Planned commits: baseline harness/evidence, negative reproductions, and
  inventory/reporting.
- Actual commits: `a458837` repairs stale boot verification paths; `7e9fe82`
  adds the claim reproducer, inventory tool, and baseline report.

## Review policy and outcome

Delegated closure was authorized for the approved ten-gate batch. Every Gate 1
criterion is satisfied with fresh evidence. Outcome: passed and closed.
