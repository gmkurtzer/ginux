# Gate 43: Quarantine IOMMU Tables After Failed Rollback

## Source

[GitHub issue #29](https://github.com/gmkurtzer/ginux/issues/29).

## Goal

Never scrub or reuse translation-table frames after partial IOMMU activation
unless rollback proves translation disabled and the root pointer cleared.

## Approved scope

- Return an explicit activation outcome distinguishing clean failure from
  failed rollback.
- Validate every disable, cache invalidation, root-clear, command, status, and
  final inactive-state result during rollback.
- Retire an inactive hierarchy only after clean rollback; otherwise retain all
  six live kernel-owned table leases.
- Add deterministic ownership tests for safe retirement and quarantine.

## Non-goals

- Do not attempt to reuse an IOMMU or device after ambiguous activation.
- Do not redesign the VT-d hierarchy or hardware command protocol.
- Do not merge the PR or close issue #29.

## Completed work

- [x] Confirmed `rollback_activation` ignores every operation result.
- [x] Confirmed `DeviceTranslationProfile::activate` retires the inactive
  hierarchy after every activation failure.
- [x] Confirmed retained kernel-owned table leases cannot be reallocated.

## Remaining work

- [ ] Propagate clean-failure versus rollback-failure outcomes.
- [ ] Retire tables only after proven inactive rollback.
- [ ] Add deterministic safe-retirement and quarantine evidence.
- [ ] Run focused, static, and complete QEMU verification.
- [ ] Push, open the remediation PR, add reciprocal links, and confirm CI.

## Acceptance criteria

- Rollback attempts every required disable and root-clear operation and checks
  the final hardware state.
- Clean rollback permits scrubbing/releasing all translation frames.
- Failed rollback leaves every table lease live and kernel-owned.
- Callers cannot collapse a failed rollback into ordinary inactive failure.
- Static and complete QEMU verification pass.

## Verification plan

- Retirement matrix -> clean rollback releases; failed rollback retains all six
  generation-tagged table leases until verifier cleanup.
- Source-path inspection -> activation returns distinct clean-failure and
  rollback-failure outcomes and the profile branches accordingly.
- Existing VT-d activation, DMA-fault recovery, teardown, and replacement
  proofs -> no normal-path regression.
- `make check` and `make test` -> full static and boot checks pass.

## Evidence

- Issue #29's cited rollback and profile paths match current
  `kernel/src/arch/x86_64/iommu.rs`.

## Decisions and limitations

- Ambiguous rollback intentionally leaks six table frames until reboot; reuse
  could permit a still-active IOMMU to walk attacker-influenced memory.

## Delivery strategy

- Working branch: `hdae/issue-29-iommu-rollback-quarantine`
- Intended base: `main`
- Pull request: intended; non-closing reference to issue #29
- Planned commits: gate definition; explicit rollback outcome and quarantine
  matrix; evidence; delivery
- Actual commits: pending

## Review policy and outcome

Delegated in the user-approved remaining-reports batch; stop only if safe
rollback requires a different VT-d command protocol.
