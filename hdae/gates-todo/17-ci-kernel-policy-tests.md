# Gate 17: CI and Host-Testable Kernel Policy

## Source

GitHub issue #5, testing and CI finding.

## Goal

Continuously run repository checks and the QEMU verification boot, and move
pure kernel policy behind a host-testable boundary.

## Approved scope

- Add pull-request and `main` GitHub Actions coverage for `make check` and
  `make test`.
- Extract pure container-name, init-path, and personality validation into a
  `no_std` crate used by the kernel.
- Add deterministic host-side tests for accepted and rejected policy inputs.

## Non-goals

- Do not claim that host tests replace QEMU or hardware-bound verification.
- Do not move all boot self-tests in this change.
- Do not close issue #5 or merge the pull request.

## Completed work

- [x] Added read-only GitHub Actions jobs for static and QEMU verification.
- [x] Added `ginux-kernel-policy` and wired the kernel to its validators.
- [x] Added four host-side tests covering positive and negative inputs.
- [x] Added the crate's lint and tests to `make check`.
- [x] Made the VT-d fault-policy check accept the no-PASID encodings emitted
  by QEMU 10 and QEMU 11 while retaining strict field and reserved-bit checks.

## Remaining work

- [x] Run local static and QEMU verification.
- [x] Confirm the workflow on GitHub after pushing pull request #9.

## Acceptance criteria

- Pull requests and pushes to `main` run `make check` and `make test`.
- The workflow has read-only repository permissions.
- Pure policy tests execute on the host and cover rejection paths.
- The kernel consumes the tested policy functions.
- Existing QEMU verification passes.

## Verification plan

- `cargo test --package ginux-kernel-policy` -> four tests pass.
- `make check` -> includes kernel-policy lint/tests and passes.
- `make test` -> complete QEMU verification boot passes.
- GitHub Actions -> both jobs pass after the branch is pushed.

## Evidence

- `cargo test --package ginux-kernel-policy` -> 4 passed, 0 failed.
- `make check` -> passed.
- `make test` -> passed with `GINUX_KERNEL_OK` and `Boot test passed.`
- GitHub Actions run `41178529192` reached the verification kernel on QEMU
  11.1.2 and exposed a no-PASID encoding change from `0xffff` to `0`; the new
  policy test covers both encodings and the complete local QEMU boot passes.
- GitHub Actions run https://github.com/gmkurtzer/ginux/actions/runs/41178914309
  -> static passed in 1m46s and QEMU passed in 3m24s.

## Decisions and limitations

- The workflow is confirmed on the pull-request head with read-only repository
  permissions.
- The user explicitly directed the corrective work into existing pull request
  #9, overriding the default one-gate-per-PR delivery split.

## Delivery strategy

- Working branch: `codex/issue-5-criticism-reconciliation`
- Intended base: `main`
- Pull request: existing PR #9
- Planned commits: CI and test seam; delivery checkpoint
- Actual commits: `0a3078ec` (CI workflow, host-testable policy, and tests),
  `40b29d3a`, `94cf22a2`, and `0269501f` (CI portability), `89dfa8e0`
  (QEMU 11 VT-d compatibility)

## Review policy and outcome

Delegated by the saved proactive scheduled-review policy; remains open pending
GitHub Actions evidence and maintainer review in pull request #9.
