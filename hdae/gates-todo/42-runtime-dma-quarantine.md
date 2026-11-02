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

## Remaining work

- [ ] Release DMA only after both teardown acknowledgements.
- [ ] Retain and prove live ownership for every failed combination.
- [ ] Run focused, static, and complete QEMU verification.
- [ ] Push, open the remediation PR, add reciprocal links, and confirm CI.

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

## Decisions and limitations

- Ambiguous teardown intentionally leaks the frame until reboot; reuse would
  violate the DMA revocation invariant.

## Delivery strategy

- Working branch: `hdae/issue-28-dma-quarantine`
- Intended base: `main`
- Pull request: intended; non-closing reference to issue #28
- Planned commits: gate definition; fail-closed retirement and matrix;
  evidence; delivery
- Actual commits: pending

## Review policy and outcome

Delegated in the user-approved remaining-reports batch; stop only if quarantine
requires a broader recovery policy.
