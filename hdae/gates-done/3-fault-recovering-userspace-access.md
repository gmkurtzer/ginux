# Gate 3: Fault-Recovering Userspace Access

## Goal

Make all kernel access to untrusted userspace memory recoverable and
hardware-authoritative.

## Approved scope

- Audited copy-from-user, copy-to-user, string-copy, and atomic-user-access
  primitives.
- Page-fault fixups or an equivalently reviewable recovery mechanism.
- Linux-compatible `EFAULT` and partial-copy semantics.
- Balanced SMAP state on every path.

## Non-goals

- Filesystem or syscall redesign beyond migration to audited primitives.
- Treating software bookkeeping as proof that hardware access will succeed.

## Completed work

- [x] Added assembly copy-from-user and copy-to-user loops with page-fault
  fixups, remaining-byte results, and balanced `STAC`/`CLAC` on success and
  fault paths.
- [x] Migrated the generic control-domain copies and Linux fast-copy path to
  the recoverable primitive.
- [x] Added runtime tests for partial cross-page copies, wholly unmapped copies,
  and cleared AC state after every result.
- [x] Migrated every syscall-facing direct SMAP-open access in `cpu.rs` to the
  audited Linux or control-domain copy boundary.
- [x] Added a machine-enforced architecture audit and documented the remaining
  kernel-owned address-space construction and verification accesses.

## Remaining work

None.

## Acceptance criteria

- Unmapped, noncanonical, kernel, read-only, guard-page, and cross-page invalid
  pointers return defined userspace errors without a kernel panic.
- A mapping change between validation and access is recovered safely.
- No syscall directly dereferences an untrusted userspace pointer outside the
  audited access module.
- Software metadata cannot override a rejected hardware translation.
- User-copy, Linux ABI, and Rocky boot suites pass.

## Evidence

- `make check` -> passed after adding the fault-fixup mechanism and generic
  copy migration.
- `make test` -> passed; both copy directions recovered at the mapped/guard
  boundary with one byte remaining, wholly unmapped copies returned the full
  length, AC was clear afterward, and QEMU exited with `GINUX_KERNEL_OK`.
- `make check` -> passed after the complete subsystem migration; the new
  user-access architecture check passed and found no syscall-side SMAP opening
  or direct cast-pointer scalar dereference.
- `bash tests/qemu/boot.sh` -> passed after the final teardown correction;
  verification, reboot, corruption, Linux runtime, and native-service lifecycle
  checks completed with `GINUX_KERNEL_OK` and no surviving QEMU process.
- `make test-linux-abi` -> passed the controller report for 23 interfaces and
  375 syscalls (398 classified rows).
- `make test-rocky-systemd` -> passed two attached Rocky Linux 10 systemd boots
  to the console login target and the detached background-start lifecycle.

## Decisions and limitations

Copy primitives return the number of bytes not copied, matching the kernel
partial-copy convention. Public slice wrappers report bytes copied or complete
success. The exception path recognizes only bounded assembly fault regions,
rewrites the saved RIP to their local fixup, restores registers, clears AC, and
returns without consulting software mapping metadata.

Internal post-exec inspection and teardown operate on kernel-owned mappings and
therefore use the same raw recoverable primitive without applying Linux syscall
policy metadata. An initial migration routed thread-stack teardown through the
policy-aware path and caused stale process mapping state; the QEMU regression
suite exposed the fault loop, and the final implementation keeps policy checks
at syscall boundaries while preserving hardware fault recovery everywhere.

## Delivery strategy

- Working branch: `codex/feedback-hardening`.
- Intended base: `main`.
- Pull request: the shared ten-gate pull request.
- Planned commits: recovery mechanism, audited primitives, subsystem
  migrations, and negative/integration evidence.
- Actual commits: `46bce19` (fault-fixup mechanism) and `8e628be` (subsystem
  migration, architecture audit, and evidence record).

## Review policy and outcome

Delegated closure authorized after all criteria pass with fresh adversarial
pointer, API audit, and regression evidence. Outcome: passed and closed under
the confirmed batch delegation on October 29, 2026.
