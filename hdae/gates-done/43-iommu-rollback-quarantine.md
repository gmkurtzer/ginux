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
- [x] Added explicit active, clean-inactive, and failed-rollback outcomes.
- [x] Made rollback attempt and validate disable, cache invalidation, root
  clearing, command completion, and final inactive hardware state.
- [x] Retire tables after clean failure and quarantine them after ambiguous
  rollback.
- [x] Added deterministic release/quarantine ownership evidence.
- [x] Passed target-aware static and complete QEMU verification.

## Remaining work

- [x] Pushed the branch, opened PR #51, and added reciprocal issue/PR links.
- [ ] Confirm final GitHub CI.

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
- `make check` -> passed; architecture checks, Clippy, unit tests, and the
  unchanged unsafe baseline of 1,033 blocks all passed.
- `make test` -> passed on the default two-vCPU QEMU profile, including
  `ginux: failed IOMMU activation rollback quarantined` and the existing VT-d
  activation, fault recovery, teardown, native-service, and replacement
  proofs.
- The deterministic retirement matrix released all six frames after a clean
  failure, retained all six live kernel-owned frames after ambiguous rollback,
  then cleaned its test quarantine and restored the baseline frame count.
- [PR #51](https://github.com/gmkurtzer/ginux/pull/51) carries the dedicated
  remediation; issue #29 and the PR contain reciprocal implementation and
  evidence summaries.

## Decisions and limitations

- Ambiguous rollback intentionally leaks six table frames until reboot; reuse
  could permit a still-active IOMMU to walk attacker-influenced memory.
- A generic host-wide `cargo check --workspace --all-targets` is not a valid
  Apple-Silicon check for the repository's x86-64 freestanding services;
  target-aware `make check` passed.

## Delivery strategy

- Working branch: `hdae/issue-29-iommu-rollback-quarantine`
- Intended base: `main`
- Pull request: intended; non-closing reference to issue #29
- Planned commits: gate definition; explicit rollback outcome and quarantine
  matrix; evidence; delivery
- Actual commits: `65c8db6f` (gate definition), `5e6eeee1` (mechanism and
  deterministic evidence), and the gate-closing evidence commit containing
  this update

## Review policy and outcome

Passed and closed under the user-approved remaining-reports batch delegation.
No merge or issue closure was performed.
