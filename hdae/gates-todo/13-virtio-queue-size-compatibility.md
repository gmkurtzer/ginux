# Gate 13: Virtio Queue-Size Compatibility

## Source

GitHub issue #4: QEMU 10.0.13 reports 1024-entry virtio-net queues while the
runtime verification gate requires exactly 256.

## Goal

Accept every queue size already supported by the runtime instead of rejecting
a valid device solely because QEMU selected 1024 rather than 256.

## Approved scope

- Make runtime verification use the existing queue-size validator.
- Retain negative checks for zero, non-power-of-two, and undersized queues.
- Add deterministic coverage for valid 256 and 1024 queue sizes.

## Non-goals

- Do not pin or install a QEMU version.
- Do not change queue allocation, notification layout, or device negotiation.
- Do not close issue #4 or merge the pull request.

## Completed work

- [x] Confirmed initialization already accepts power-of-two sizes of at least 8.
- [x] Replaced the fixed 256 assertion with the shared validator.
- [x] Added 1024-positive and 1000-negative checks.

## Remaining work

- [x] Run the verification commands.
- [x] Push the branch and open pull request #10.
- [x] Record the stable pull-request URL.

## Acceptance criteria

- Runtime verification accepts valid 256- and 1024-entry queue pairs.
- Runtime verification rejects zero, 255, 1000, and other invalid sizes.
- Existing architecture and build checks pass.

## Verification plan

- `make architecture-check` -> all architecture checks pass.
- `make check` -> static checks and builds pass.

## Evidence

- Source inspection: all device initialization paths call
  `validate_network_queue_size`; the old verification check alone required 256.
- `make architecture-check` -> passed all architecture checks.
- `make check` -> passed builds, static checks, compatibility checks, and unit
  tests.
- Pull request: https://github.com/gmkurtzer/ginux/pull/10
- Issue response: https://github.com/gmkurtzer/ginux/issues/4#issuecomment-6540049160

## Decisions and limitations

- This verifies the deterministic queue contract. A QEMU 10.0.13 runtime is
  not required because issue #4 identifies the assertion failure precisely.

## Delivery strategy

- Working branch: `codex/issue-4-virtio-queue-size`
- Intended base: `main`
- Pull request: intended; non-closing reference to issue #4
- Planned commits: implementation and gate evidence; delivery checkpoint
- Actual commits: `d4f673d8` (implementation, gate, and verification evidence)

## Review policy and outcome

Delegated by the saved proactive scheduled-review policy. All criteria passed
and delivery is complete through open pull request #10; the gate remains open
for maintainer review.
