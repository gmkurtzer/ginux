# Gate 22: Failed Linux Init Frame Release

## Source

[GitHub issue #26](https://github.com/gmkurtzer/ginux/issues/26).

## Goal

Release the bootstrap page-table hierarchy after every non-suspended Linux init
attempt, including invalid runtime exits, without reporting a failed init as a
successful process exit.

## Approved scope

- Remove validity short-circuiting from hierarchy-frame release.
- Preserve the failed outcome for invalid init execution.
- Run static and full QEMU regressions.

## Non-goals

- Do not change suspended-process ownership transfer.
- Do not change Linux runtime validation policy.
- Do not close issue #26 or merge the pull request.

## Completed work

- [x] Confirmed `valid && runtime_scrubbed && release_frames(...)` skipped all
  six hierarchy frames when `valid` was false.
- [x] Decoupled cleanup success from process validity while retaining
  `LinuxProcessOutcome::Failed` for invalid execution.
- [x] Passed the full static and QEMU verification suites.

## Remaining work

- [ ] Maintainer review and acceptance.

## Acceptance criteria

- Invalid, non-suspended init execution attempts hierarchy-frame release.
- Invalid init execution remains a failed outcome.
- Valid exits and suspended ownership transfer retain existing behavior.
- `make check` and `make test` pass.

## Verification plan

- Static control-flow inspection -> `release_frames` is not guarded by
  `valid`, while the successful outcome remains guarded by `valid`.
- `make check` -> static, architecture, lint, and unit checks pass.
- `make test` -> full QEMU runtime verification passes.

## Evidence

- Original expression short-circuited before `release_frames` whenever the
  runtime returned `valid=false`.
- `make check` -> passed, including architecture checks, warning-denying
  clippy, formatting, and unit tests.
- `make test` on QEMU 10.1.2/TCG -> `GINUX_KERNEL_OK`; `Boot test passed.`
- Delivery: [PR #37](https://github.com/gmkurtzer/ginux/pull/37).
- Source response: [issue #26 comment](https://github.com/gmkurtzer/ginux/issues/26#issuecomment-6561988902).
- Reciprocal response: [PR #37 comment](https://github.com/gmkurtzer/ginux/pull/37#issuecomment-6561988454).

## Decisions and limitations

- The six hierarchy frames are launcher-owned on every non-suspended path;
  runtime validity does not change that ownership.

## Delivery strategy

- Working branch: `hdae/issue-26-linux-init-frame-release`
- Intended base: `main`
- Pull request: [PR #37](https://github.com/gmkurtzer/ginux/pull/37), open,
  non-closing reference to issue #26
- Planned commits: correction and evidence; delivery checkpoint
- Actual commits: `b79cdb8a` correction and evidence; delivery checkpoint
  pending

## Review policy and outcome

Delegated by the saved proactive scheduled-review policy. Keep this gate in
`gates-todo/` pending maintainer review; delivery requires a verified fix,
pushed branch, open pull request, and reciprocal links.
