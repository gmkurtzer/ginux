# Gate 4: Global-State Classification and Ownership Design

## Goal

Account for every writable global and approve its intended owner, lifetime,
synchronization rule, and teardown behavior before migration begins.

## Approved scope

- Generate an inventory covering `static mut`, mutable interior state, global
  atomics, global arrays, descriptor tables, and architecture scratch state.
- Classify each item as initialization-only, per-CPU, per-domain, per-process,
  per-thread, or explicitly shared.
- Define target state containers, ownership boundaries, lifecycle, and lock
  ordering requirements.
- Add CI detection for newly introduced unclassified writable globals.

## Non-goals

- Broad state migration; that belongs to Gates 5 and 6.
- Cosmetic removal of legitimate immutable or initialization-only statics.
- Choosing synchronization primitives before ownership requirements are known.

## Completed work

None. Depends on Gate 3.

## Remaining work

- Build the machine-readable inventory and reviewed allowlist.
- Produce the ownership/lifecycle map and migration grouping.
- Add CI enforcement for classification coverage.

## Acceptance criteria

- Every writable global is classified, including state hidden by interior
  mutability.
- Every retained global has a documented owner, lifecycle, access invariant,
  synchronization requirement, and migration or allowlist disposition.
- The allowlist is narrow and limited to justified architecture/bootstrap
  anchors.
- CI rejects newly introduced unclassified writable globals.

## Evidence

Generated inventory, allowlist, ownership map, CI negative test, and human
review disposition for every classification group.

## Decisions and limitations

Classification does not itself make the state SMP-safe. Gates 5, 6, and 8
perform migration and synchronization.

## Delivery strategy

- Working branch: `codex/feedback-hardening`.
- Intended base: `main`.
- Pull request: the shared ten-gate pull request.
- Planned commits: inventory tool/report, ownership design, and CI enforcement.
- Actual commits: pending.

## Review policy and outcome

Checkpoint required. Stop for ownership-map and allowlist approval. Outcome:
pending.
