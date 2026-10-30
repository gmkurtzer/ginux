# Gate 4: Explicit State Ownership

## Goal

Eliminate ambient mutable runtime state whose owner, synchronization, or
lifetime is implicit.

## Approved scope

- Classify every writable global as initialization-only, per-CPU, per-domain,
  per-process, per-thread, or explicitly shared and synchronized.
- Move runtime scratch and signal state into owned structures.
- Define construction, reset, teardown, and destruction paths.
- Maintain a narrow reviewed architecture/bootstrap allowlist.

## Non-goals

- Cosmetic prohibition of legitimate immutable or initialization-only statics.
- Selecting final SMP primitives before ownership is explicit.

## Completed work

None. Depends on Gate 3.

## Remaining work

- Turn Gate 1 inventory into an approved ownership map.
- Migrate state subsystem by subsystem with lifecycle tests.
- Add CI rejection for new unclassified writable globals.

## Acceptance criteria

- No writable global remains unclassified, including state hidden by interior
  mutability.
- No raw mutable static remains outside the reviewed allowlist.
- Every shared object names its synchronization primitive and lock order.
- Domain teardown, process exit, controller restart, and repeated boot prove
  reset or reclamation by the owner.
- Compatibility and performance show no material regression.

## Evidence

Machine-checked inventory, ownership map, lifecycle tests, allowlist review,
and regression/performance comparison.

## Decisions and limitations

Hardware descriptor anchors may remain global only with explicit initialization
and access invariants.

## Delivery strategy

Planned commits: inventory enforcement followed by small subsystem ownership
migrations and lifecycle evidence.

## Review policy and outcome

Checkpoint required. Stop for state-ownership and lifecycle review. Outcome:
pending.
