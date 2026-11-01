# Gate 23: Getcwd Maximum-Path Capacity

## Source

[GitHub issue #21](https://github.com/gmkurtzer/ginux/issues/21).

## Goal

Return a maximum-length canonical current directory plus its leading slash and
terminating NUL without indexing beyond the kernel's fixed output buffer.

## Approved scope

- Size the `getcwd` staging buffer for the root slash, maximum path, and NUL.
- Preserve Linux `ERANGE` and `EFAULT` behavior.
- Run static and full QEMU regressions.

## Non-goals

- Do not change the project-wide path-capacity contract.
- Do not close issue #21 or merge the pull request.

## Completed work

- [x] Confirmed a 256-byte copied path produces a 257-byte absolute path and
  requires 258 staging bytes including NUL.
- [x] Increased the staging buffer from `PATH_CAPACITY + 1` to
  `PATH_CAPACITY + 2`.
- [x] Passed the full static and QEMU verification suites.

## Remaining work

- [ ] Deliver the signed commit, pull request, and reciprocal links.

## Acceptance criteria

- A maximum-length copied path fits with the leading slash and NUL terminator.
- Smaller caller buffers still return `ERANGE`.
- Invalid user destinations still return `EFAULT`.
- `make check` and `make test` pass.

## Verification plan

- Boundary arithmetic inspection -> `1 + PATH_CAPACITY + 1` equals the output
  array capacity.
- `make check` -> static, architecture, lint, and unit checks pass.
- `make test` -> full Linux-runtime QEMU verification passes.

## Evidence

- The original array held 257 bytes while the maximum `required` value was
  258, making `&output[..required]` panic.
- `make check` -> passed, including architecture checks, warning-denying
  clippy, formatting, and unit tests.
- `make test` on QEMU 10.1.2/TCG -> `GINUX_KERNEL_OK`; `Boot test passed.`

## Decisions and limitations

- This preserves the existing accepted path length rather than reducing it to
  work around the missing terminator byte.

## Delivery strategy

- Working branch: `hdae/issue-21-getcwd-capacity`
- Intended base: `main`
- Pull request: intended, non-closing reference to issue #21
- Planned commits: correction and evidence; delivery checkpoint
- Actual commits: pending

## Review policy and outcome

Delegated by the saved proactive scheduled-review policy. Keep this gate in
`gates-todo/` pending maintainer review; delivery requires a verified fix,
pushed branch, open pull request, and reciprocal links.
