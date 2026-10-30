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

None. Depends on Gate 2.

## Remaining work

- Define fault-recovery representation and exception interaction.
- Inventory and migrate direct userspace dereferences.
- Add races and cross-page adversarial tests.

## Acceptance criteria

- Unmapped, noncanonical, kernel, read-only, guard-page, and cross-page invalid
  pointers return defined userspace errors without a kernel panic.
- A mapping change between validation and access is recovered safely.
- No syscall directly dereferences an untrusted userspace pointer outside the
  audited access module.
- Software metadata cannot override a rejected hardware translation.
- User-copy, Linux ABI, and Rocky boot suites pass.

## Evidence

User-pointer test matrix, direct-dereference audit, SMAP assertions, and full
regression transcript.

## Decisions and limitations

Partial-copy semantics and fixup design must be documented before coding.

## Delivery strategy

- Working branch: `codex/feedback-hardening`.
- Intended base: `main`.
- Pull request: the shared ten-gate pull request.
- Planned commits: recovery mechanism, audited primitives, subsystem
  migrations, and negative/integration evidence.
- Actual commits: pending.

## Review policy and outcome

Checkpoint required. Stop for adversarial pointer and API review. Outcome:
pending.
