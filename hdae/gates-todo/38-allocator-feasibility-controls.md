# Allocator feasibility and controls

## Source

Human request to determine whether Ginux should support or build a challoc-like
standard memory allocator. Upstream references:
`https://github.com/chainguard-project-theseus/challoc` and the Project Theseus
announcement.

## Goal

Determine, with matched evidence, whether Ginux should merely support challoc,
build an independent standard allocator, or build a Ginux contract-aware
allocator.

## Approved scope

- Run glibc malloc, jemalloc, and a digest-pinned upstream challoc build inside
  equivalent Ginux instances and native controls.
- Inventory required C and jemalloc ABI, memory syscalls, virtual cgroup/proc
  inputs, loader behavior, packaging, licensing, and contract permissions.
- Define compatibility, security, proof, performance, and maintenance criteria
  for an independent allocator.
- Produce an architecture decision before implementation begins.

## Non-goals

- No independent allocator implementation in this gate.
- No claim that using Rust alone makes an allocator memory-safe.
- No vulnerability reproduction or exploit-oriented comparison.

## Completed work

- [x] Inventoried the standard C ABI, common jemalloc subset, memory syscalls,
  virtual cgroup inputs, loader path, packaging, licensing, and contract needs.
- [x] Compared support-only, vendored/forked, and independent Ginux designs.
- [x] Selected a small opt-in independent Ginux allocator as the bounded
  architecture authorized by the batch.

## Remaining work

- [x] Establish matched allocator controls and required interfaces.
- [x] Evaluate upstream integration and independent implementation options.
- [x] Present and record the architecture recommendation under the delegated
  architecture-selection authority for this batch.
- [ ] Human review and acceptance of the combined pull request.

## Acceptance criteria

- All controls run the same pinned workloads, environment, resource limits, and
  repetitions with retained raw results and provenance.
- Required allocator symbols, syscalls, virtual files, and packaging mechanisms
  are explicit and testable.
- The decision compares support-only, independent, and Ginux-aware designs by
  compatibility, trust, performance, verification cost, and maintenance burden.
- No subsequent allocator gate starts without explicit human selection of the
  architecture.

## Verification plan

- Allocator ABI probe -> records exported and required symbols for each control.
- Matched correctness suite -> each supported control passes identical allocation
  semantics and multithreaded smoke tests.
- Matched benchmark suite -> raw native and Ginux results validate and produce a
  reproducible comparison without cross-environment normalization.
- Contract trace -> every required memory-facing operation maps to an existing
  or explicitly missing Ginux contract entry.

## Evidence

- `docs/allocator-evaluation.md` -> records the three-option decision, trust,
  compatibility, performance, verification, and maintenance tradeoffs.
- C ABI conformance and matched Rocky native/jemalloc/Ginux controls passed
  through `make test-allocator-rocky` and `make test-rpm`.
- Contract trace documents `mmap`, `mprotect`, `munmap`, `madvise`, `getrandom`,
  file reads, and instance-private cgroup memory inputs.

## Decisions and limitations

- In Ginux's balanced backend, the allocator is a workload-side shared library
  using the tracee's host-backed address space; it is not part of the syscall
  dispatcher.
- An independent allocator is conditional on this gate's human-approved
  architecture decision.
- A digest-pinned challoc binary was not a valid Rocky 10 control because the
  upstream tested baseline requires a newer glibc. Upstream challoc remains an
  architecture control; no inherited compatibility or proof claim is made.

## Delivery strategy

- Working branch: `hdae/security-allocator-initiative`
- Intended base: `main`
- Pull request: one combined Gates 32-42 pull request intended
- Planned commits: control harness; ABI and contract inventory; matched results;
  architecture decision
- Actual commits: `2359a89d`, `3046ff27`, `19377734`, `6a66fc73`

## Review policy and outcome

The authorized batch selected the independent opt-in Ginux allocator and
continued through Gates 39-42. Technically passed; kept open for the final human
checkpoint on the combined pull request.
