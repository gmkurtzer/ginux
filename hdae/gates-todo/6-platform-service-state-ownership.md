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
- [x] Consolidated virtual-network inventory, attachments, host link and IPv4
  configuration, routes, and generations from 13 ambient globals into one
  explicit host-network owner root.
- [x] Consolidated physical-memory allocation, host paging tables, controller
  publication state, and bootstrap kernel-stack storage into four explicit
  host or bootstrap owner roots.
- [x] Consolidated auxiliary address-space registration, controller image
  publication, native-volume completion, and unsafe-quiesce diagnostics into
  one explicit host address-space service owner.
- [x] Consolidated PCI interrupt routing/completion state and APIC timer,
  preemption, Linux wait, and device timeout state into two explicit host
  device owner roots.
- [x] Consolidated CPU entry slots, descriptor tables, TSS, IST/privilege
  stacks, and extended-user-state publication into one explicit per-CPU
  architecture owner root.

## Remaining work

- Run final Gate 6 regression evidence and close the delegated gate.

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
- After the host-network migration, `make architecture-check` passed with the
  inventory reduced from 519 to 507 objects, `make check` passed, and the full
  verification QEMU boot passed. `make test-c1` also passed every network
  create, inventory, link, route, address, DHCP, ping, restart, and attachment
  check before reaching its pre-existing embedded minimal-domain panic; the
  same later failure reproduced at parent commit `9bc3ca23`, and both runs
  cleaned up QEMU.
- After the core-host migration, `make architecture-check` passed with the
  writable-global inventory reduced from 507 to 493 objects, `make check`
  passed, and `make build-verification && bash tests/qemu/boot.sh` completed
  the full verification boot and clean QEMU shutdown.
- After the address-space service migration, `make architecture-check` passed
  with the inventory reduced from 493 to 487 objects, `make check` passed, and
  the full verification QEMU boot completed cleanly.
- After the PCI/timer migration, `make architecture-check` passed with the
  inventory reduced from 487 to 458 objects, `make check` passed, and the full
  verification QEMU boot completed cleanly.
- After the per-CPU architecture migration, `make architecture-check` passed
  with the inventory reduced from 458 to 450 objects and zero Gate 6 migration
  rows, `make check` passed, and the full verification QEMU boot completed
  cleanly.

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
  `9bc3ca23` owns filesystem/storage publication state; `d6b88ade` owns host
  network configuration; `1d0f262f` owns core host platform state;
  `f04ec028` owns host address-space service state; `7ac3b5aa` owns host
  interrupt and timer state; per-CPU architecture ownership pending commit.

## Review policy and outcome

Delegated closure authorized after platform/service ownership, failure,
isolation, and regression criteria all pass. Outcome: pending.
