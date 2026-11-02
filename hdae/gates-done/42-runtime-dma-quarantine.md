# Gate 42: Quarantine Runtime DMA on Teardown Failure

## Source

[GitHub issue #28](https://github.com/gmkurtzer/ginux/issues/28).

## Goal

Never return a runtime-volume DMA frame to the allocator unless both the device
and its IOMMU translation are confirmed retired.

## Approved scope

- Centralize the release-or-quarantine decision for runtime-volume DMA.
- Keep the generation-tagged kernel-owned lease live when device quiescence or
  translation retirement fails.
- Apply the same rule when backend startup fails after IOMMU activation.
- Add deterministic tests for all retirement-result combinations.

## Non-goals

- Do not attempt continued use of a device after ambiguous teardown.
- Do not redesign persistent service-domain frame quarantine.
- Do not merge the PR or close issue #28.

## Completed work

- [x] Confirmed `with_runtime_volume` scrubs/releases its DMA frame before
  checking device and translation teardown results.
- [x] Confirmed backend-start failure also releases after ignoring a failed
  translation retirement.
- [x] Confirmed retaining a live kernel-owned lease prevents allocator reuse.
- [x] Centralized the release-or-quarantine decision and applied it to normal
  teardown plus backend-start failure.
- [x] Added a four-case retirement matrix proving all failed combinations keep
  live ownership while success/success scrubs and releases.
- [x] Passed focused compilation, the complete static gate, and complete QEMU
  regression.
- [x] Published PR #50 and added reciprocal issue/PR summaries.

## Remaining work

- [ ] GitHub CI is external evidence and must finish before merge.

## Acceptance criteria

- A failed device quiesce leaves the DMA lease live and kernel-owned.
- A failed translation retirement leaves the DMA lease live and kernel-owned.
- A frame is scrubbed and released only when both operations succeed.
- Startup failure after translation activation follows the same rule.
- Static and complete QEMU verification pass.

## Verification plan

- Pure retirement matrix -> success/success releases; all three failure
  combinations retain live ownership and are cleaned up only by the verifier.
- Backend-start path inspection -> translation failure cannot reach frame
  release.
- `make check` and `make test` -> full static and boot checks pass.

## Evidence

- Issue #28's cited sequence matches current `kernel/src/lib.rs`.
- A live `FrameLease` owned by `FrameOwner::KERNEL` is ineligible for allocator
  reuse, so retaining it is a fail-closed quarantine.
- `make check` passed with the unchanged 1,033-block unsafe baseline and Linux
  compatibility matrix.
- `make test` passed and emitted
  `failed runtime volume DMA retirement quarantined`.
- The deterministic matrix restored the original kernel live-frame count after
  explicitly cleaning only its three simulated quarantines.

## Decisions and limitations

- Ambiguous teardown intentionally leaks the frame until reboot; reuse would
  violate the DMA revocation invariant.

## Delivery strategy

- Working branch: `hdae/issue-28-dma-quarantine`
- Intended base: `main`
- Pull request: [#50](https://github.com/gmkurtzer/ginux/pull/50), with a
  non-closing reference to issue #28
- Planned commits: gate definition; fail-closed retirement and matrix;
  evidence; delivery
- Actual commits: `7c6184de` (gate definition), `20494327` (fail-closed
  retirement and matrix), plus the evidence/delivery commit containing this
  record

## Review policy and outcome

Passed and closed under the user-approved remaining-reports batch delegation.
No broader recovery policy was required; ambiguous teardown leaks one frame
until reboot by design.
