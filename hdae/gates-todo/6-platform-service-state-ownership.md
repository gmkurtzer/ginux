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

- [x] Consolidated the isolated registry/network service heap, network buffers,
  DNS state, TLS workspaces, pull metadata, and progress state from 20 ambient
  globals into three explicit service-generation owner roots.
- [x] Consolidated image-store state, initramfs publication, per-domain rootfs
  indexes, packet mailboxes, and the live volume device from 29 ambient globals
  into five explicit host or domain publication owner roots.

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

- After the isolated network-service migration, `make architecture-check`
  passed with the writable-global inventory reduced from 560 to 543 objects.
- `cargo check -p ginux-network-service-runtime --target
  x86_64-unknown-none` and `make check` passed.
- `make build-verification && bash tests/qemu/boot.sh` passed the complete
  service fault, replacement, network request-generation, ABI, lifecycle, and
  cleanup sequence.
- After the filesystem/storage publication migration, `make
  architecture-check` passed with the inventory reduced from 543 to 519
  objects, `make check` passed, and the complete verification QEMU boot passed
  image-store, root publication, packet fault/replacement, volume recovery,
  lifecycle, and cleanup evidence.

## Decisions and limitations

Truly host-global hardware objects require explicit host ownership; “global” is
not itself prohibited when ownership and synchronization are concrete.

## Delivery strategy

- Working branch: `codex/feedback-hardening`.
- Intended base: `main`.
- Pull request: the shared ten-gate pull request.
- Planned commits: filesystem/storage ownership, network/socket ownership,
  device/controller ownership, and failure/lifecycle evidence.
- Actual commits: `5ba79480` owns isolated network-service state;
  filesystem/storage publication ownership pending commit.

## Review policy and outcome

Delegated closure authorized after platform/service ownership, failure,
isolation, and regression criteria all pass. Outcome: pending.
