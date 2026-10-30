# Gate 6: Platform and Service State Ownership

## Goal

Move device, filesystem, network, storage, controller, and supporting service
state into explicit host, device, service, or domain-owned structures.

## Approved scope

- Filesystem/mount/tmpfs/image-store state.
- Network devices, packets, sockets, and protocol-service state.
- Volume, device, controller, and remaining platform state.
- Initialization, failure recovery, restart, teardown, and multi-domain tests.

## Non-goals

- Runtime process/thread/signal state completed by Gate 5.
- SMP activation or final synchronization implementation.

## Completed work

None. Depends on Gate 5.

## Remaining work

- Migrate every platform/service ownership group from Gate 4.
- Eliminate hidden singleton service instances and implicit reset paths.
- Add service-failure and multi-domain isolation tests.

## Acceptance criteria

- Platform and service state is accessible only through its explicit owner.
- Device/service failure, controller restart, domain teardown, and repeated boot
  leave no stale authority or cross-domain state.
- No platform/service writable global remains outside the reviewed allowlist.
- Multi-domain storage, networking, filesystem, and lifecycle tests pass.
- Existing compatibility and performance do not materially regress.

## Evidence

Before/after inventory, owner API review, injected-failure tests, multi-domain
isolation results, and complete regressions.

## Decisions and limitations

Truly host-global hardware objects require explicit host ownership; “global” is
not itself prohibited when ownership and synchronization are concrete.

## Delivery strategy

- Working branch: `codex/feedback-hardening`.
- Intended base: `main`.
- Pull request: the shared ten-gate pull request.
- Planned commits: filesystem/storage ownership, network/socket ownership,
  device/controller ownership, and failure/lifecycle evidence.
- Actual commits: pending.

## Review policy and outcome

Checkpoint required. Stop for platform/service ownership review. Outcome:
pending.
