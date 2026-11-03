# Gate 34: Iterative PAX Header Chains

## Source

[GitHub issue #17](https://github.com/gmkurtzer/ginux/issues/17).

## Goal

Parse arbitrarily long, input-bounded PAX header chains without recursive
kernel stack growth while preserving valid PAX override behavior.

## Approved scope

- Replace recursive PAX chaining with an iterative walk.
- Retain path and link override validation and precedence.
- Add a long-chain regression that previously risked stack exhaustion.

## Non-goals

- Do not redesign the tar index or OCI cache trust model.
- Do not merge the PR or close issue #17.

## Completed work

- [x] Confirmed each PAX header recursively calls `parse_entry` and does not
  count toward `TAR_MAX_ENTRIES`.
- [x] Replaced recursive chaining with checked offset iteration while retaining
  earliest-header override precedence.
- [x] Added a 4,000-header regression that resolves to one valid entry.

## Remaining work

- [x] Implement iterative PAX parsing and regression coverage.
- [x] Run `make check` and `make test`.
- [x] Push, open the remediation PR, and add reciprocal links.
- [x] Confirm final pull-request CI.

## Acceptance criteria

- A long chain of empty PAX headers followed by one valid entry parses without
  recursion or panic.
- Existing PAX path/link override validation and precedence remain intact.
- Static and complete QEMU verification pass.

## Verification plan

- `cargo test --package ginux-oci` -> long-chain and existing OCI tests pass.
- Source inspection -> no recursive `parse_entry` call remains.
- `make check` and `make test` -> full static and boot checks pass.

## Evidence

- Current `parse_entry` recursively enters the next header for every type `x`
  entry, including an empty PAX payload.
- `cargo test --package ginux-oci` -> all 10 tests passed, including the
  4,000-header chain and multi-header override precedence.
- Warning-denying `ginux-oci` clippy passed; source search confirms the
  recursive call is absent.
- `make check` -> passed all static, architecture, unit, and warning-denying
  checks; all 10 `ginux-oci` tests passed in the complete suite.
- `make test` -> the complete two-vCPU QEMU verification boot reached
  `GINUX_KERNEL_OK` and reported `Boot test passed.`
- [PR #44](https://github.com/gmkurtzer/ginux/pull/44) contains the dedicated
  remediation branch; issue #17 and the PR contain reciprocal summaries.
- PR #44 GitHub `static` and `qemu` checks passed on the delivery head.

## Decisions and limitations

- Iteration removes stack growth without imposing an arbitrary compatibility
  cap; byte bounds and checked offset progress remain authoritative.

## Delivery strategy

- Working branch: `hdae/issue-17-pax-chain`
- Intended base: `main`
- Pull request: [#44](https://github.com/gmkurtzer/ginux/pull/44); non-closing
  reference to issue #17
- Planned commits: iterative parser and regression; generated/evidence;
  delivery checkpoint
- Actual commits: `e9976442` gate definition; `4294c0a7` iterative parser and
  regression; `7ac928e2` verification evidence; delivery-link checkpoint
  pending

## Review policy and outcome

Delegated in the user-approved remaining-reports batch. Stop only for failed
evidence or a new architectural decision; all objective Gate 34 criteria pass,
and final review occurs after the batch.
