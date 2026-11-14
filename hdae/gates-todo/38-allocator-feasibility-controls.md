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

- [ ] None.

## Remaining work

- [ ] Establish matched allocator controls and required interfaces.
- [ ] Evaluate upstream integration and independent implementation options.
- [ ] Present an architecture recommendation for explicit human approval.

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

- Not run.

## Decisions and limitations

- In Ginux's balanced backend, the allocator is a workload-side shared library
  using the tracee's host-backed address space; it is not part of the syscall
  dispatcher.
- An independent allocator is conditional on this gate's human-approved
  architecture decision.

## Delivery strategy

- Working branch: `hdae/allocator-feasibility-controls`
- Intended base: `main`
- Pull request: intended
- Planned commits: control harness; ABI and contract inventory; matched results;
  architecture decision
- Actual commits: pending

## Review policy and outcome

Architecture checkpoint requiring explicit human acceptance. Persistence of
this gate does not authorize implementation or subsequent allocator gates.
