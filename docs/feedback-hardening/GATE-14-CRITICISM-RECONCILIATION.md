# Gate 14 Criticism Reconciliation

## Scope and fresh evidence

This report re-evaluates the nine public checklist items in GitHub issue #5
against current `main` commit `22d20a63`. The issue originally evaluated
commit `1852455`.

Fresh verification passed after the corrective implementation: `make check`,
`make architecture-check`, `make linux-compat-check`, and the complete `make
test` QEMU boot. The compatibility check reports 53 Complete, 257 Partial, 48
Policy-denied, 17 Unsupported, and 0 Missing rows.

## Claim matrix

| Claim | Classification | Current evidence | Follow-up |
|---|---|---|---|
| NMI and machine check lack trusted IST stacks during syscall entry | Resolved | Current architecture installs distinct guarded NMI, double-fault, and machine-check ISTs; the privilege-entry architecture check and QEMU NMI regressions pass. | None for the reported defect. |
| User copies lack fault recovery and trust software bookkeeping | Resolved | Current code has bounded assembly fixups, hardware-authoritative access, balanced SMAP handling, and partial-copy/error regressions; the user-access architecture check and QEMU suite pass. | None for the reported defect. |
| Signal/process globals cannot survive SMP | Resolved for the reported defect | Ownership checks classify 484 globals and 404 managed backings; current QEMU verification boots two processors and exercises concurrent Linux processes. | Continue enforcing the existing inventory. |
| Exclusion relies on one CPU and interrupt masking | Resolved | Ranked ticket and IRQ-safe locks, lock ordering checks, AP startup, cross-CPU execution, and SMP regressions are present and pass. | None for the reported defect. |
| `cpu.rs` is 44,324 lines and contains Linux policy | Partially resolved with a ratcheted extraction | It is now 42,752 lines. Linux random, boot-ID, and getrandom policy moved to a focused module while ownership and QEMU checks pass. It remains large and needs continued decomposition. | Continue mechanical extractions with static and QEMU checks. |
| Unsafe code lacks confined boundaries and direct kernel lint coverage | Partially resolved with enforcement | `make check` now runs warning-denying kernel clippy. Architecture checks ratchet the current 1,033 unsafe blocks, 109 `SAFETY:` comments, and one top-level `static mut`. | Lower the unsafe ceiling and increase documented boundaries over time. |
| `make test` fails, CI is absent, and the kernel has no unit tests | Corrected for the reported delivery gap | `make test` passes; GitHub Actions runs `make check` and `make test`; a `no_std` kernel-policy crate provides host tests for kernel-consumed validators. Hardware-bound behavior remains QEMU-tested. | Expand host-testable policy boundaries without replacing integration coverage. |
| The 82 Complete rows include ENOSYS-only entries and socket limits conflict with Complete | Corrected | Seventeen ENOSYS-only entries are Unsupported, twelve bounded socket calls are Partial, generated runtime status parsing recognizes Unsupported, and the checker enforces the new totals. | Preserve status semantics through the deterministic checker. |
| The 3.31x startup value compares non-equivalent TCG profiles | Resolved in repository reporting | Benchmarking and paper text retain the measured value but explicitly identify unequal startup paths and reject a controlled kernel-performance interpretation. | Preserve those caveats beside future uses. |

## Corrective implementation

The residual work retains separate HDAE gates and acceptance evidence, but the
maintainer explicitly directed it into existing PR #9 after the initial
reconciliation-only delivery proved insufficient:

1. Linux personality decomposition.
2. Kernel unsafe-boundary inventory and lint enforcement.
3. Deterministic CI plus a pure-kernel unit-test seam.
4. Compatibility-ledger status semantics and evidence linkage.

All four gates are implemented on `codex/issue-5-criticism-reconciliation`.

## Private report checkpoint

Issue #5 mentions an additional privately reported entry-path finding. No
description, reproduction, or exploitability claim is recorded here. It must
remain in the repository's private security process.

## Maintainer response

Thank you for the detailed review. I re-evaluated all nine public findings
against current `main` and ran the repository's architecture, build,
compatibility, and complete QEMU verification checks.

The reported privilege-entry, user-copy, state-ownership, and single-CPU
exclusion defects are resolved in current code, and the original `make test`
failure is also resolved. Four independently actionable areas remain:
`cpu.rs` is still too large, unsafe/lint enforcement is incomplete, CI and
kernel unit-test coverage are still missing, and the compatibility ledger's
Complete labels remain internally inconsistent for ENOSYS-only and partially
supported families. Those should be handled as separate gates and pull
requests. The 3.31x startup value remains only as a caveated operational
comparison of non-equivalent profiles, not a kernel-performance claim.

The privately reported item is intentionally excluded from this public record.
