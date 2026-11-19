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

- [x] Selected virtual cgroup `memory.max` and `memory.current` as the narrow
  allocator interface; no raw host memory tree is exposed.
- [x] Derived the hard limit from instance `resources.memory_mib` and current
  usage from only supervised instance processes.
- [x] Added bounded allocator credits and pressure reclamation using approved
  `madvise`/`munmap` behavior.

## Remaining work

- [x] Specify and select the narrow allocator-facing memory interface under the
  batch's delegated interface-selection authority.
- [x] Implement pressure-aware reclamation and feature probing.
- [x] Prove instance isolation and update documentation/examples.
- [ ] Human review and acceptance of the combined pull request.

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

- Allocator unit/model tests -> bounded accounting never wraps or exceeds the
  instance-visible limit.
- Exact-head review regression -> actual mapped capacity is reserved once,
  failed arena initialization rolls the reservation back, and caller-live
  pressure returns to its baseline after full-arena free and reuse.
- `make test-kernel-views`, aggregate `make test`, and `make test-multi-instance`
  -> virtual memory views and distinct instance limits remain private.
- `make test-security test-recovery` -> virtual-kernel, pressure, and failure
  behavior passed without host-tree exposure.

## Decisions and limitations

- The interface choice is an architecture checkpoint inside this gate; no
  implementation begins until the human approves it.

## Delivery strategy

- Working branch: `hdae/security-allocator-initiative`
- Intended base: `main`
- Pull request: https://github.com/gmkurtzer/ginux/pull/292
- Planned commits: interface decision; virtual telemetry; allocator integration;
  isolation tests and documentation
- Actual commits: `e5f7da76`, `36c742e5`, `94c4f4f4`, `de194940`

## Review policy and outcome

The batch selected virtual cgroup views as the narrowest supported interface.
Technically passed; kept open for the final human checkpoint on the combined PR.
