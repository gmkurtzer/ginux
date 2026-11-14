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

- [x] Implemented the selected standard C allocation ABI and bounded common
  jemalloc entry-point subset in a Rust shared library.
- [x] Added native C conformance, threaded stress, diagnostics, RPM `%check`,
  and opt-in `LD_PRELOAD` integration.
- [x] Corrected shard collisions so a full preferred shard probes all sibling
  shards before returning `ENOMEM`.

## Remaining work

- [x] Implement and document the approved ABI.
- [x] Add native and Ginux conformance fixtures.
- [x] Package and load the allocator without runtime-specific application edits.
- [ ] Human review and acceptance of the combined pull request.

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

- `make test-allocator` -> seven allocator unit/model tests passed.
- `make test-allocator-rocky` -> standard ABI conformance and all 45 matched
  native/jemalloc/Ginux benchmark samples passed at exact source commit.
- `make test-rpm` -> the installed library loaded and ran inside supported
  Rocky and Ubuntu glibc instances; package verification and reinstall passed.

## Decisions and limitations

- Gate 38's architecture decision is a hard dependency and may cancel or reshape
  this gate before implementation.
- The packaged library is glibc x86-64. Alpine's musl runtime remains supported
  by Ginux but is outside this allocator ABI claim.

## Delivery strategy

- Working branch: `hdae/security-allocator-initiative`
- Intended base: `main`
- Pull request: https://github.com/gmkurtzer/ginux/pull/292
- Planned commits: ABI specification; core allocator; conformance; Ginux loading
  and packaging; final evidence
- Actual commits: `2359a89d`, `3046ff27`, `2b151a26`, `19377734`, `da24dff7`

## Review policy and outcome

Technically passed under the authorized batch after Gate 38 selected the
architecture. Kept open for the final human checkpoint on the combined pull request.
