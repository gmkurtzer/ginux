# Gate 35: Directory Whiteout Subtrees

## Source

[GitHub issue #18](https://github.com/gmkurtzer/ginux/issues/18).

## Goal

Make a plain OCI directory whiteout hide both the named lower-layer directory
and every descendant so removed content cannot reappear by full-path lookup.

## Approved scope

- Apply plain whiteouts to the exact target and its subtree in the OCI reader.
- Remove the exact target and descendants from the published-rootfs path index.
- Add regression coverage for the reported two-layer resurrection case and
  adjacent-prefix behavior.

## Non-goals

- Do not redesign OCI overlay construction or mutable runtime overlays.
- Do not merge the PR or close issue #18.

## Completed work

- [x] Confirmed `whiteout_hides` matches only the exact target.
- [x] Confirmed the published-rootfs index removes only the exact target for a
  plain whiteout even though subtree removal already exists for opaque markers.
- [x] Made plain whiteouts hide the exact target and descendants in the OCI
  reader and published-rootfs path index.
- [x] Added the reported two-layer subtree regression and an adjacent-prefix
  assertion.

## Remaining work

- [x] Implement exact-and-descendant plain-whiteout handling in both paths.
- [x] Add focused regression coverage.
- [x] Run focused, static, and complete QEMU verification.
- [ ] Push, open the remediation PR, add reciprocal links, and confirm CI.

## Acceptance criteria

- A later `.wh.secret` hides both `secret` and `secret/key` from lower layers.
- A whiteout for `secret` does not hide an adjacent `secret2` path.
- Published-rootfs indexing applies the same exact-and-subtree semantics.
- Static and complete QEMU verification pass.

## Verification plan

- `cargo test --package ginux-oci` -> the reported resurrection and prefix
  boundary regressions pass with all existing OCI tests.
- Source inspection -> plain published-rootfs whiteouts remove the target and
  call the existing descendant-removal path for that target.
- `make check` and `make test` -> full static and boot checks pass.

## Evidence

- Issue #18's code-path analysis matches the current implementation: both
  lookup paths preserve descendants after removing only the exact directory.
- `cargo test --package ginux-oci` -> all 9 tests passed, including a lower
  `secret/key`, upper `.wh.secret`, and retained adjacent `secret2/key`.
- Warning-denying `ginux-oci` clippy passed.
- Source inspection confirms plain published-rootfs whiteouts remove the exact
  target and then reuse `remove_index_children` for its descendants.
- `make check` -> all static, architecture, unit, and warning-denying checks
  passed.
- `make test` -> the complete two-vCPU QEMU verification boot reached
  `GINUX_KERNEL_OK` and reported `Boot test passed.`

## Decisions and limitations

- Reuse existing path-boundary and index-removal mechanisms; no new overlay
  state or compatibility cap is introduced.

## Delivery strategy

- Working branch: `hdae/issue-18-whiteout-subtree`
- Intended base: `main`
- Pull request: intended; non-closing reference to issue #18
- Planned commits: gate definition; whiteout behavior and regression;
  generated/evidence; delivery checkpoint
- Actual commits: `61882c29` gate definition; `344f4d84` whiteout behavior and
  regression; evidence and delivery checkpoints pending

## Review policy and outcome

Delegated in the user-approved remaining-reports batch. Stop only for failed
evidence or a new architectural decision; final review occurs after the batch.
