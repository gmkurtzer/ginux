# Enforce clone policy in configured native mode

## Source

[Issue #284](https://github.com/gmkurtzer/ginux/issues/284), reviewed against
`main` at `33948c62b0eb59546f0b8a0a0d05592a083c481b`.

## Goal

Apply the existing managed-process clone invariant to configured native mode
before any host `clone` or replayed `clone3` can create a child.

## Scope and non-goals

- Reuse the existing clone flag/shape policy for mode-3 `clone`.
- Apply the same policy to the supervisor-owned `clone3` snapshot, including
  its separate exit signal.
- Prove `CLONE_UNTRACED` is rejected with `ENOSYS` in both ABIs.
- Preserve supported fork, thread, and `CLONE_INTO_CGROUP` behavior.
- Do not change network policy or PID translation beyond preventing the
  unsupported child from being created.

## Delivery strategy

- Branch: `hdae/native-clone-policy`
- Base: `main`
- Delivery: signed commit and pull request referencing issue #284.
- Review policy: keep in `gates-todo/` pending exact-head review.

## Acceptance criteria and verification

- Mode-3 `clone` and `clone3` reject `CLONE_UNTRACED`, `CLONE_PARENT`, host
  namespace flags, and unsupported address-space sharing before host effect.
- Supported configured-native child creation and clone3 snapshot tests pass.
- `make check` and `make test-unit` pass.
- Linux `sudo make test-native-supervision` passes the new negative regression.

## Plan

1. Route configured `clone` through the existing process policy.
2. Validate immutable clone3 flags and exit signal before replay.
3. Add negative fixture coverage and run local checks before PR delivery.

## Evidence

- Current mode 3 uses `decide_systemd` without `process_hooks::claim`.
- Current clone3 replay validates snapshot ownership but not process flags.
- `make check` passed on macOS.
- `make test-unit` passed on macOS.
- GitHub workflow 43552139427 passed `changes`, `static-and-unit`, and
  `launch-integration`, including the privileged native-supervision regression.
- Delivery: [PR #287](https://github.com/gmkurtzer/ginux/pull/287).
- Source update:
  https://github.com/gmkurtzer/ginux/issues/284#issuecomment-6870951901
- Reciprocal PR update:
  https://github.com/gmkurtzer/ginux/pull/287#issuecomment-6870952239

## Status

- Implemented, locally and in Linux CI verified, pushed, and delivered in PR
  #287; awaiting exact-head pull-request review.
