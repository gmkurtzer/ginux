# Preserve loopback netlink acknowledgement errors

## Source

[Issue #302](https://github.com/gmkurtzer/ginux/issues/302).

## Goal

Retire validated loopback netlink sequences without changing the host kernel's
success or failure acknowledgement returned to the instance.

## Scope

- Keep sequence correlation for validated loopback netlink messages.
- Preserve a zero success acknowledgement unchanged.
- Preserve every negative kernel errno unchanged.
- Add focused success, failure, and unmatched-sequence regressions.

## Acceptance criteria

- A matching success ACK retires its sequence and remains zero.
- A matching failure ACK retires its sequence and retains the exact errno.
- Untracked sequences and non-ACK values are ignored.
- Static checks and unit tests pass.

## Non-goals

- No new netlink message or address shape is admitted.
- No error, including `EEXIST`, is converted to success.

## Status

Pull request #305 is merged into `main`. Technical status remains passed;
lifecycle status remains open pending the consolidated Gate 51 human
acceptance checkpoint.

## Evidence

- `cargo test -p ginux-core loopback_ack`
- `cargo clippy -p ginux-core --all-targets -- -D warnings`
- `make check`
- Fresh Gate 51 `make check test-cli test-unit`, `make test-security`, the
  complete Rocky systemd matrix, and aggregate `make test` revalidate exact
  loopback acknowledgement result preservation on current `main`.
