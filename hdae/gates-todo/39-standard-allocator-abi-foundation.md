# Standard allocator ABI foundation

## Source

Conditional allocator implementation gate following the human-approved decision
from Gate 38.

## Goal

Build an independently testable Rust shared library that provides the essential
standard C allocation ABI and can run transparently inside a Ginux instance.

## Approved scope

- Implement `malloc`, `free`, `calloc`, `realloc`, aligned allocation, usable
  size, and required glibc compatibility entry points selected by Gate 38.
- Support transparent dynamic loading or linking in normal unmodified workloads.
- Define allocator ownership, alignment, overflow, zero-size, error, fork, and
  initialization semantics.
- Add single-threaded and multithreaded conformance tests outside and inside
  Ginux.

## Non-goals

- No security or performance superiority claim.
- No jemalloc extended API unless Gate 38 makes it mandatory for the initial ABI.
- No Ginux-specific memory-pressure behavior; Gate 41 owns that integration.

## Completed work

- [ ] None.

## Remaining work

- [ ] Implement and document the approved ABI.
- [ ] Add native and Ginux conformance fixtures.
- [ ] Package and load the allocator without runtime-specific application edits.

## Acceptance criteria

- Every selected ABI symbol has documented semantics and positive, boundary, and
  failure tests.
- Alignment and size arithmetic are checked; overflow fails without allocating.
- Valid concurrent allocation and release do not corrupt allocator or caller
  state under deterministic stress.
- The same library passes the approved suite natively and inside Ginux through
  the documented loading path.

## Verification plan

- Allocator unit and ABI tests -> all selected symbols and edge cases pass.
- C conformance fixtures -> compile and run against the replacement library.
- Threaded stress and model tests -> no invalid state, deadlock, or leak beyond
  documented retained caches.
- Ginux integration fixture -> an unmodified workload uses the allocator and
  exits successfully under the approved contract.

## Evidence

- Not run.

## Decisions and limitations

- Gate 38's architecture decision is a hard dependency and may cancel or reshape
  this gate before implementation.

## Delivery strategy

- Working branch: `hdae/security-allocator-initiative`
- Intended base: `main`
- Pull request: one combined Gates 32-42 pull request intended
- Planned commits: ABI specification; core allocator; conformance; Ginux loading
  and packaging; final evidence
- Actual commits: pending

## Review policy and outcome

Authorized for autonomous execution after Gate 38 selects the architecture in
the human-approved batch. Keep the gate open for one final human checkpoint on
the combined pull request.
