# Gate 40: Acknowledge SMP mprotect Revocation

## Source

[GitHub issue #25](https://github.com/gmkurtzer/ginux/issues/25).

## Goal

Ensure `mprotect(PROT_READ)` cannot return success while another CPU retains a
writable translation for the same Linux address space.

## Approved scope

- Replace local-only invalidation in the active-image read-only fast path with
  the existing acknowledged cross-CPU runtime-translation shootdown.
- Return success only after every online CPU acknowledges each revoked page.
- Roll back the current PTE if a shootdown cannot complete.
- Retain existing validation, W^X, RELRO, and runtime-mapping behavior.

## Non-goals

- Do not redesign the SMP NMI shootdown mechanism.
- Do not change Linux `mprotect` or pkey ABI semantics.
- Do not merge the PR or close issue #25.

## Completed work

- [x] Confirmed the reported fast path clears `WRITABLE` and performs only a
  local `invlpg`.
- [x] Confirmed dynamic runtime permission changes already use
  `smp::invalidate_runtime_translation`.
- [x] Confirmed that mechanism waits for every online sibling CPU to
  acknowledge the invalidation.
- [x] Routed each active-image permission revoke through the acknowledged SMP
  invalidator and restore the old PTE if it cannot complete.
- [x] Passed focused compilation, the complete static gate, and default
  two-vCPU QEMU regression.
- [x] Published PR #49 and added reciprocal issue/PR summaries.

## Remaining work

- [ ] GitHub CI is external evidence and must finish before merge.

## Acceptance criteria

- No active-image `mprotect(PROT_READ)` success path uses local-only invalidation.
- Every affected page is invalidated locally and on every online sibling CPU
  before success.
- A failed shootdown does not report successful protection change.
- Existing memory-protection and complete QEMU regressions pass with two vCPUs.

## Verification plan

- Source-path assertion -> the fast path calls the acknowledged SMP invalidator
  after clearing `WRITABLE` and checks its result.
- Existing SMP mechanism proof -> cross-CPU interrupt and invalidation
  acknowledgement still passes on the default two-vCPU QEMU profile.
- Paired boot markers -> the default two-vCPU boot reports both acknowledged
  cross-CPU invalidation and the active-image read-only transition.
- `make check` and `make test` -> full static and boot checks pass.

## Evidence

- Issue #25's cited fast path and the existing acknowledged mechanism match
  current `kernel/src/address_space.rs` and `kernel/src/arch/x86_64/smp.rs`.
- `make check` passed with the unchanged 1,033-block unsafe baseline and Linux
  compatibility matrix.
- `make test` passed on the default two-vCPU profile, including both
  `cross-CPU interrupts and invalidation verified` and
  `Linux read-only memory transition validated`.
- Source inspection confirms no local-only `invlpg` remains in
  `protect_current_linux_range_read_only`.

## Decisions and limitations

- Reuse the established NMI shootdown rather than introducing a second
  translation-invalidation protocol.

## Delivery strategy

- Working branch: `hdae/issue-25-mprotect-shootdown`
- Intended base: `main`
- Pull request: [#49](https://github.com/gmkurtzer/ginux/pull/49), with a
  non-closing reference to issue #25
- Planned commits: gate definition; acknowledged revocation and regression;
  evidence; delivery
- Actual commits: `949b3b19` (gate definition), `0d2ce4b0` (acknowledged
  revocation), plus the evidence/delivery commit containing this record

## Review policy and outcome

Passed and closed under the user-approved remaining-reports batch delegation.
The established shootdown safely covers every online CPU without an ABI change.
