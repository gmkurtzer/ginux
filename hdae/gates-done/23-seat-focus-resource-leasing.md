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

- [x] Added explicit keyboard focus, pointer capture, trusted-controller focus,
  bounded transition history, and read-only seat status.
- [x] Added generation-bound display, keyboard, and pointer leases with bounded
  rights, expiry, trusted revocation, and observable status.
- [x] Added synchronous secure attention, timeout, surface destruction,
  disconnect, and stale-client recovery.
- [x] Added isolated physical-input routing and transactional focus changes.
- [x] Added host concurrency tests, a ring-3 contract probe, and 1/2/4-CPU QEMU
  regression coverage.

## Remaining work

- [x] No remaining work.

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

- `cargo test --package ginux-display-protocol` -> 6 tests passed, including
  concurrent lease contention, secure attention, timeout, owner death, stale
  handles, and cross-client input isolation.
- `make test-controller-seat-leases` -> ring-3 contract probe and full boot test
  passed.
- `make test-smp` -> 1-, 2-, and 4-CPU qualification plus two 2-CPU stress
  passes succeeded.
- `make check` -> complete static, ABI, compatibility, and unit regression suite
  passed; unsafe baseline remained 1033 blocks.
- `git diff --check` -> passed.

## Decisions and limitations

- Exclusive use is a controller-mediated logical lease; raw physical transfer is
  deferred until a later IOMMU/device-virtualization design.
- Controller status copies identity and lease metadata but never returns an
  authority-bearing client handle.
- The boot harness's obsolete manifest-grant assertion was corrected from 10
  to the 12 grants introduced by the accepted display/input architecture.

## Delivery strategy

- Working branch: `codex/controller-desktop`
- Intended base: `main`
- Pull request: shared integrated PR for Gates 19–30
- Planned commits: seat/focus state; lease authority/recovery; stress tests
- Actual commits: `f942f71c`

## Review policy and outcome

Passed and closed on 2026-10-31 under the human's delegated Gates 20–30
authorization. Every acceptance criterion has fresh objective evidence.
