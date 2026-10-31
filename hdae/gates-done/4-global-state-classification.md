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

- [x] Expanded the inventory to detect `static mut`, atomic/lock-like interior
  mutability, and custom `UnsafeCell` wrapper types.
- [x] Classified all 620 writable globals into eight ownership and lifecycle
  groups with an allowlist or migration disposition.
- [x] Defined the owner, lifetime, access invariant, synchronization direction,
  teardown expectation, and Gate 5/6 migration assignment for every group.
- [x] Added exact manifest enforcement and a negative unclassified-global test
  to `make architecture-check`.

## Remaining work

None.

## Acceptance criteria

- Every writable global is classified, including state hidden by interior
  mutability.
- Every retained global has a documented owner, lifecycle, access invariant,
  synchronization requirement, and migration or allowlist disposition.
- The allowlist is narrow and limited to justified architecture/bootstrap
  anchors.
- CI rejects newly introduced unclassified writable globals.

## Evidence

- `tools/architecture/inventory-writable-globals.sh` -> found 76 `static mut`
  declarations and 544 interior-mutable declarations, including seven custom
  `UnsafeCell` wrappers missed by the original scanner.
- `tools/architecture/check-writable-globals.sh` -> passed exact classification
  coverage for 620 symbols and passed its synthetic-unclassified-global
  negative test.
- `make architecture-check` -> passed with privilege-entry, user-access, and
  writable-global enforcement active.
- `make check` -> passed the complete static, compatibility, ABI, lint, and
  unit-test suite.

## Decisions and limitations

Classification does not itself make the state SMP-safe. Gates 5, 6, and 8
perform migration and synchronization.

The reviewed manifest assigns 482 symbols to Gate 5, 127 to Gate 6, and 11 to
the narrow bootstrap/verification allowlist. The required lock direction is
platform/CPU registry, runtime manager, domain, process, thread, then
descriptor/object; Gate 8 selects and validates concrete primitives.

## Delivery strategy

- Working branch: `codex/feedback-hardening`.
- Intended base: `main`.
- Pull request: the shared ten-gate pull request.
- Planned commits: inventory tool/report, ownership design, and CI enforcement.
- Actual commits: `a34e34b` (inventory, ownership model, manifest, negative
  enforcement, and architecture-check integration).

## Review policy and outcome

Delegated closure authorized after the inventory, ownership map, allowlist, and
CI enforcement all pass their stated checks. Outcome: passed and closed under
the confirmed batch delegation on October 29, 2026.
