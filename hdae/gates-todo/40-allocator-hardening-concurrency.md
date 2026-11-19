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

- [x] Separated allocator metadata from caller allocations and added guard
  pages, checked size classes, randomized placement, and ownership validation.
- [x] Added sharded atomic slot ownership, cross-thread release, guarded large
  mappings, pressure reclamation, fork-aware locking, and deterministic invalid free.
- [x] Fixed reclamation ordering, checked aggregate reservation, and shard
  fallback defects found during review and stress.

## Remaining work

- [x] Implement the approved hardening and concurrency model.
- [x] Add defensive invalid-input and concurrency regressions.
- [x] Review every unsafe seam and residual assumption.
- [ ] Human review and acceptance of the combined pull request.

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

- `make test-allocator` -> checked geometry, accounting, guards, and shard
  coverage passed.
- Exact-head review repair separates reserved capacity from live requested
  bytes, rolls reservations back on every initialization failure, and proves
  full-arena fallback, free, pressure recovery, and slot reuse without
  invented capacity.
- GitHub's parallel macOS unit runner exposed shared allocator-test accounting;
  the stateful allocator tests now serialize their global-counter assertions,
  and the replacement CI run passed.
- `make fuzz-security` -> bounded allocator fuzz target passed 158,877 cases in
  the final five-second campaign.
- `make test-allocator-rocky` -> single- and multithreaded conformance plus
  1/2/4-thread benchmark stress completed without allocation failure.
- `docs/security-assurance.md` inventories every approved allocator unsafe/FFI seam.

## Decisions and limitations

- Defensive malformed-call fixtures test allocator responses but do not create
  exploit chains or offensive payloads.

## Delivery strategy

- Working branch: `hdae/security-allocator-initiative`
- Intended base: `main`
- Pull request: https://github.com/gmkurtzer/ginux/pull/292
- Planned commits: metadata model; hardening; concurrency; unsafe review and
  final evidence
- Actual commits: `02ee1e57`, `36c742e5`, `48577ef8`, `da24dff7`, `de194940`,
  `a0c5b3bf`

## Review policy and outcome

Technically passed under the authorized batch. Kept open for the final human
checkpoint on the combined pull request.
