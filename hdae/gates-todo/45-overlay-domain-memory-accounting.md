# Overlay domain memory accounting

## Goal

Charge every writable-overlay frame to the active container domain so ordinary
writes, copy-up, xattrs, and shared-mapping metadata cannot bypass the domain's
memory limit, while host overlay setup retains kernel-owned allocation.

## Approved scope

- Route overlay node, mapping, data, and xattr pages through the active domain's
  frame owner and limit.
- Fail closed when a container slot exists but its frame policy is unavailable.
- Release and rollback overlay frames using their recorded allocator owner.
- Retain deterministic regressions for quota exhaustion, restoration,
  metadata accounting, cross-domain isolation, and host fallback.

## Non-goals

- Changing the domain memory-limit model or unit.
- Charging immutable shared rootfs/file-cache pages to private domain memory.
- Altering rootfs copy-up semantics beyond enforcing its existing memory limit.

## Completed work

- [x] Reproduced the ownership gap by inspecting every overlay allocation and
  release path.
- [x] Confirmed the active instance already resolves the current container
  slot and that the memory allocator exposes generation-safe owner accounting.

## Remaining work

- [x] Implement domain-aware overlay allocation and owner-aware cleanup.
- [x] Add deterministic quota lifecycle and isolation regressions.
- [x] Run the static and complete QEMU regression suites.
- [ ] Publish the dedicated pull request and link issue #80.

## Acceptance criteria

- A container cannot allocate overlay frames beyond its configured owner limit,
  including ordinary data writes and rootfs copy-up.
- Node, shared-mapping, data, and xattr pages all count against the active
  container owner.
- Allocation fails closed if a container is active but its domain policy cannot
  be resolved; it never falls back to `FrameOwner::KERNEL` in that state.
- Rollback and teardown restore charged owner capacity; unlink preserves Linux
  open-handle semantics without allowing further allocation past the quota.
- Exhausting one domain does not consume or block another domain's capacity.
- Host overlay setup continues to use kernel-owned frames.

## Verification plan

- `make check` -> formatting, lint, architecture, unsafe inventory, and unit
  checks pass.
- `make test` -> the complete deterministic QEMU suite passes with new markers
  proving overlay owner charging, exhaustion, cleanup restoration, isolation,
  metadata coverage, and host fallback.
- Inspect every `linux_overlay.rs` allocation and release path -> no container
  overlay path can silently allocate or release as `FrameOwner::KERNEL`.

## Evidence

- Current `origin/main` inspection -> all overlay allocation and matching
  cleanup paths are hard-coded to `FrameOwner::KERNEL`; issue reproduced.
- `make check` -> passed, including formatting, architecture, unsafe inventory,
  and workspace unit checks.
- `make test` -> passed the complete two-vCPU QEMU and Linux runtime suite; the
  new deterministic marker is `ginux: Linux overlay domain quota lifecycle
  enforced`.
- Source inspection after implementation -> node, mapping, data, and xattr
  frames all use the same fail-closed domain policy; every rollback and teardown
  release derives the actual owner from the frame lease.

## Decisions and limitations

- Frame leases remain the ownership record: cleanup derives the actual owner
  with `memory::frame_owner`, so teardown is correct even outside the allocating
  container's current execution context.
- The host fallback is permitted only when no container slot is active.
- Linux unlink semantics retain unlinked-but-open overlay data. Capacity is
  restored on rollback, destructive whiteout replacement, and runtime teardown;
  adding descriptor reference-counted early reclamation is outside this
  accounting defect and is not required to prevent the budget bypass.

## Delivery strategy

- Working branch: `hdae/issue-80-overlay-domain-accounting`
- Intended base: `main`
- Pull request: intended, dedicated to issue #80
- Planned commits: gate definition; ownership implementation and deterministic
  regression; final evidence/checkpoint update
- Actual commits: `a02052cf` (gate definition), `b379e28b` (implementation and
  regression); final checkpoint commit pending

## Review policy and outcome

Delegated implementation and pull-request publication are authorized by the
hourly repository-maintenance schedule. Merge remains subject to a later
scheduled pass with a current green exact-head validation and the schedule's
two-pass merge policy.
