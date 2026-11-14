# Virtual set_tid_address result in configured native mode

## Source

[Issue #285](https://github.com/gmkurtzer/ginux/issues/285), reviewed against
`main` at `33948c62b0eb59546f0b8a0a0d05592a083c481b`.

## Goal

Keep configured native workloads from observing their host TID through
`set_tid_address` while preserving the host clear-on-exit registration.

## Scope and non-goals

- Rewrite a successful mode-3 `set_tid_address` result to the task's virtual
  identity at the supervised syscall exit.
- Add a configured-native regression that proves `set_tid_address`, `getpid`,
  and `gettid` agree on the virtual identity.
- Preserve the host syscall and its clear-on-exit behavior.
- Do not change clone policy, networking, or the process-native path.

## Delivery strategy

- Branch: `hdae/native-set-tid-virtual`
- Base: `main`
- Delivery: signed commit and pull request referencing issue #285.
- Review policy: keep this gate in `gates-todo/` for pull-request review; a
  later exact-head review may integrate it under the saved repository policy.

## Acceptance criteria and verification

- A successful configured-native `set_tid_address` returns virtual PID 1,
  never the host TID.
- The existing non-configured virtual-TID behavior remains unchanged.
- `make check` passes.
- `make test-unit` passes.
- Linux `sudo make test-native-supervision` exercises both paths and passes.

## Plan

1. Add the exit-stop translation using the existing task virtual identity.
2. Extend the native fixture regression to configured mode.
3. Run static and local unit checks; rely on Linux CI for the privileged native
   integration, then deliver the branch and reciprocal links.

## Evidence

- Current `supervise_native` executes syscall 218 but has no result rewrite;
  `supervise_process_native` already performs the equivalent translation.
- `make check` passed on macOS.
- `make test-unit` passed on macOS.
- GitHub workflow 43551883397 passed `changes`, `static-and-unit`, and
  `launch-integration`, including the privileged native-supervision regression.
- Delivery: [PR #286](https://github.com/gmkurtzer/ginux/pull/286).
- Source update:
  https://github.com/gmkurtzer/ginux/issues/285#issuecomment-6870903120
- Reciprocal PR update:
  https://github.com/gmkurtzer/ginux/pull/286#issuecomment-6870903441

## Status

- Implemented, locally and in Linux CI verified, pushed, and delivered in PR
  #286; awaiting exact-head pull-request review.
