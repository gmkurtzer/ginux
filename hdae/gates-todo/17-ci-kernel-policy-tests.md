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
- [x] Added three host-side tests covering positive and negative inputs.
- [x] Added the crate's lint and tests to `make check`.

## Remaining work

- [x] Run local static and QEMU verification.
- [ ] Confirm the workflow on GitHub after pushing pull request #9.

## Acceptance criteria

- Pull requests and pushes to `main` run `make check` and `make test`.
- The workflow has read-only repository permissions.
- Pure policy tests execute on the host and cover rejection paths.
- The kernel consumes the tested policy functions.
- Existing QEMU verification passes.

## Verification plan

- `cargo test --package ginux-kernel-policy` -> three tests pass.
- `make check` -> includes kernel-policy lint/tests and passes.
- `make test` -> complete QEMU verification boot passes.
- GitHub Actions -> both jobs pass after the branch is pushed.

## Evidence

- `cargo test --package ginux-kernel-policy` -> 3 passed, 0 failed.
- `make check` -> passed.
- `make test` -> passed with `GINUX_KERNEL_OK` and `Boot test passed.`

## Decisions and limitations

- CI confirmation is necessarily pending until the workflow is pushed.
- The user explicitly directed the corrective work into existing pull request
  #9, overriding the default one-gate-per-PR delivery split.

## Delivery strategy

- Working branch: `codex/issue-5-criticism-reconciliation`
- Intended base: `main`
- Pull request: existing PR #9
- Planned commits: CI and test seam; delivery checkpoint
- Actual commits: `0a3078ec` (CI workflow, host-testable policy, and tests)

## Review policy and outcome

Delegated by the saved proactive scheduled-review policy; remains open pending
GitHub Actions evidence and maintainer review in pull request #9.
