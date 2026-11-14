# Ginux-aware allocator memory contract

## Source

Conditional follow-up to accepted Gates 39 and 40.

## Goal

Make the allocator respond to Ginux's instance memory contract and virtual
pressure state without exposing host-global memory information.

## Approved scope

- Define a narrow stable interface for configured memory limit, current usage,
  pressure, page size, and supported memory-management features.
- Use virtual cgroup/proc state or a smaller reviewed Ginux interface rather than
  raw host trees.
- Return unused memory with approved `madvise` or `munmap` behavior as pressure
  rises and document the exact syscall contract required.
- Add deterministic limit, pressure, reclamation, and isolation tests.

## Non-goals

- No host memory or topology disclosure.
- No host-wide memory controller or enforcement outside the owning instance.
- No undocumented allocator-specific bypass around the instance contract.

## Completed work

- [ ] None.

## Remaining work

- [ ] Specify and approve the allocator-facing memory interface.
- [ ] Implement pressure-aware reclamation and feature probing.
- [ ] Prove instance isolation and update recipe examples.

## Acceptance criteria

- The allocator observes only instance-scoped values and behaves correctly when
  optional telemetry is unavailable.
- Reclamation reacts predictably to controlled pressure without exceeding the
  approved contract or exposing host state.
- Two instances with different limits receive different private views and cannot
  influence one another except through documented host resource contention.
- Required memory operations are explicit in the recipe and fail closed when
  unavailable.

## Verification plan

- Controlled memory-limit tests -> allocation and reclamation follow configured
  instance values.
- Virtual kernel-view tests -> no raw host memory values or paths are exposed.
- Two-instance pressure test -> telemetry and allocator response remain isolated.
- `make test-contract test-kernel-views test-security test` -> all affected and
  aggregate suites pass.

## Evidence

- Not run.

## Decisions and limitations

- The interface choice is an architecture checkpoint inside this gate; no
  implementation begins until the human approves it.

## Delivery strategy

- Working branch: `hdae/ginux-aware-memory-contract`
- Intended base: `main`
- Pull request: intended
- Planned commits: interface decision; virtual telemetry; allocator integration;
  isolation tests and documentation
- Actual commits: pending

## Review policy and outcome

Architecture and security checkpoint requiring explicit human acceptance.
Persistence of this gate does not authorize implementation.
