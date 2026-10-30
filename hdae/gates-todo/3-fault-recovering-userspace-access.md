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

## Remaining work

- Inventory and migrate direct userspace dereferences.
- Add the direct-access architecture audit and remaining Linux pointer cases.

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

## Decisions and limitations

Copy primitives return the number of bytes not copied, matching the kernel
partial-copy convention. Public slice wrappers report bytes copied or complete
success. The exception path recognizes only bounded assembly fault regions,
rewrites the saved RIP to their local fixup, restores registers, clears AC, and
returns without consulting software mapping metadata.

## Delivery strategy

- Working branch: `codex/feedback-hardening`.
- Intended base: `main`.
- Pull request: the shared ten-gate pull request.
- Planned commits: recovery mechanism, audited primitives, subsystem
  migrations, and negative/integration evidence.
- Actual commits: fault-fixup mechanism commit pending.

## Review policy and outcome

Delegated closure authorized after all criteria pass with fresh adversarial
pointer, API audit, and regression evidence. Outcome: pending.
