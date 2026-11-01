# Gate 21: Virtio-Net Common-Capability Length

## Source

[GitHub issue #30](https://github.com/gmkurtzer/ginux/issues/30).

## Goal

Reject a virtio-net common capability unless it contains the complete 64-bit
queue-device field at offset `0x30`.

## Approved scope

- Correct the virtio-net common-region lower bound.
- Retain an exact boundary regression in the existing boot-time parser checks.

## Non-goals

- Do not change queue negotiation or MMIO layout.
- Do not close issue #30 or merge the pull request.

## Completed work

- [x] Confirmed the old `> 0x30` check accepted lengths `0x31..0x37` before
  later 64-bit accesses at `0x30`.
- [x] Required the complete eight-byte field and added rejected/accepted
  boundary probes.
- [x] Passed the full static and QEMU verification suites.

## Remaining work

- [ ] Deliver the signed commit, pull request, and reciprocal links.

## Acceptance criteria

- Length `0x37` is rejected and `0x38` is accepted.
- Existing valid virtio-net discovery and queue operation still pass.
- `make check` and `make test` pass.

## Verification plan

- `make check` -> static, architecture, lint, and unit checks pass.
- `make test` -> the parser boundary probe and full virtio-net boot suite pass.

## Evidence

- Static inspection confirmed a `u64` read at `COMMON_QUEUE_DEVICE` (`0x30`).
- `make check` -> passed, including formatting, warning-denying clippy,
  architecture checks, and all unit tests.
- `make test` on QEMU 10.1.2/TCG -> `GINUX_KERNEL_OK`; `Boot test passed.`

## Decisions and limitations

- The boundary is expressed from the field offset and `size_of::<u64>()` so it
  cannot drift from the accessed field width.

## Delivery strategy

- Working branch: `hdae/issue-30-virtio-net-common-length`
- Intended base: `main`
- Pull request: intended, non-closing reference to issue #30
- Planned commits: correction and regression; delivery checkpoint
- Actual commits: correction and regression commit pending

## Review policy and outcome

Delegated by the saved proactive scheduled-review policy. Keep this gate in
`gates-todo/` pending maintainer review; delivery requires a verified fix,
pushed branch, open pull request, and reciprocal links.
