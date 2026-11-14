# Allocator hardening and concurrency

## Source

Conditional follow-up to accepted Gate 39.

## Goal

Harden the allocator's metadata and concurrency model so common caller memory
errors cannot silently steer allocator state.

## Approved scope

- Keep allocator metadata outside caller allocations and define its integrity
  boundary.
- Add checked size classes, randomized placement where supported, guard regions,
  invalid-free and double-free detection, and deterministic failure behavior.
- Implement and verify thread-local caching, cross-thread release, reclamation,
  fork behavior, and contention control.
- Document unsafe code and the assumptions at every raw-memory seam.

## Non-goals

- No claim that the allocator makes an unsafe application memory-safe.
- No exploit demonstrations or reproduction of historical allocator CVEs.
- No performance acceptance before Gate 42's matched comparison.

## Completed work

- [ ] None.

## Remaining work

- [ ] Implement the approved hardening and concurrency model.
- [ ] Add defensive invalid-input and concurrency regressions.
- [ ] Review every unsafe seam and residual assumption.

## Acceptance criteria

- Live caller allocations never overlap allocator metadata or another live
  allocation.
- Size arithmetic and class selection cannot wrap or index outside their tables.
- Controlled invalid and duplicate frees fail deterministically without mutating
  unrelated allocator state.
- Threaded allocation, cross-thread free, cache flush, and process-fork cases
  preserve invariants without deadlock.

## Verification plan

- Allocator invariant/property tests -> separation and arithmetic properties pass.
- Thread and Loom-style model tests -> covered synchronization schedules pass.
- Bounded defensive fuzzing -> no unclassified crash, hang, or invariant failure.
- `make test` plus allocator conformance -> Ginux and project regressions pass.

## Evidence

- Not run.

## Decisions and limitations

- Defensive malformed-call fixtures test allocator responses but do not create
  exploit chains or offensive payloads.

## Delivery strategy

- Working branch: `hdae/allocator-hardening-concurrency`
- Intended base: `main`
- Pull request: intended
- Planned commits: metadata model; hardening; concurrency; unsafe review and
  final evidence
- Actual commits: pending

## Review policy and outcome

Security checkpoint requiring explicit human acceptance. Persistence of this
gate does not authorize implementation.
