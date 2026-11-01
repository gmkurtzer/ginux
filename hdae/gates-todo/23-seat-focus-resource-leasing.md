# Seats, focus, and resource leasing

## Goal

Make display/input focus and exclusive resource use explicit, capability-bound,
temporary, observable, and safely revocable.

## Approved scope

- Implement seats, focus transitions, trusted chrome ownership, logical
  exclusive leases, secure attention, revocation, and owner-death recovery.
- Expose lease/focus state and history through controller status operations.
- Test race, timeout, stale-generation, and multi-domain isolation behavior.

## Non-goals

- Raw GPU or physical input-device passthrough and arbitrary device assignment.

## Completed work

- [ ] None.

## Remaining work

- [ ] Implement state machines and tests, then stop for human review.

## Acceptance criteria

- Exactly one surface owns keyboard focus and at most one owns pointer capture
  per seat; unfocused domains receive no physical input.
- A lease records resource, rights, generation, owner, expiry, and revoker.
- Secure attention synchronously restores trusted controller focus and revokes
  exclusive presentation/input regardless of client state.
- Crash, timeout, stop, stale handle, and concurrent request paths reclaim all
  leases without cross-domain delivery.
- Controller status exposes current focus and leases without granting authority.

## Verification plan

- `make test-controller-seat-leases` -> state-transition, race, stale-generation,
  secure-attention, owner-death, and isolation assertions pass.
- `make test-smp` -> concurrent focus/lease operations pass under SMP.
- `make check` -> regression suite passes.

## Evidence

- Not yet run.

## Decisions and limitations

- Exclusive use is a controller-mediated logical lease; raw physical transfer is
  deferred until a later IOMMU/device-virtualization design.

## Delivery strategy

- Working branch: `codex/controller-desktop`
- Intended base: `main`
- Pull request: shared integrated PR for Gates 19–30
- Planned commits: seat/focus state; lease authority/recovery; stress tests
- Actual commits: pending

## Review policy and outcome

Delegated closure under the human's expanded Gates 20–30 authorization on
2026-10-31. Continue after complete evidence; stop only for a clarification,
failed dependency, or scope/architecture change.
