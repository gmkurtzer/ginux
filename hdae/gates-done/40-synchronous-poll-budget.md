# Gate 40: Bound Synchronous Poll Waits

## Source

[GitHub issue #62](https://github.com/gmkurtzer/ginux/issues/62).

## Goal

Prevent `poll`, `ppoll`, and `select` socket waits from holding the serialized
kernel-entry path in an unbounded spin when the guest requests no timeout.

## Approved scope

- Apply the existing `BLOCKING_POLL_LIMIT` to the synchronous poll helper.
- Preserve readiness, timeout, result-copy, and error behavior within the bound.
- Add deterministic boot evidence for the timeout-free path.

## Non-goals

- Do not redesign poll waits around a new scheduler primitive.
- Do not change non-socket polling or the Linux poll ABI.

## Completed work

- [x] Confirmed the timeout-free synchronous helper used an open-ended loop.
- [x] Reused the established socket poll budget for synchronous poll waits.
- [x] Added a deterministic timeout-free verifier and QEMU marker.

## Remaining work

- None.

## Acceptance criteria

- No synchronous socket poll exceeds `BLOCKING_POLL_LIMIT` iterations.
- A timeout-free wait returns zero when the budget expires and no descriptor is ready.
- Ready descriptors and finite deadlines retain their existing behavior.
- `make check` and `make test` pass.

## Verification plan

- Boot verifier -> an empty timeout-free synchronous poll exhausts the finite budget and returns zero.
- Source inspection -> the helper has no open-ended loop.
- `make check` -> static and policy checks pass.
- `make test` -> full QEMU boot passes and emits the synchronous poll budget marker.

## Evidence

- Issue #62 matches `linux_poll_synchronously`: `duration == None` produced no
  deadline and the helper previously had no other exit when nothing was ready.
- `make check` passed with the 1,032-block unsafe baseline and complete policy checks.
- `make test` passed, emitted the synchronous poll budget marker, and ended with
  `GINUX_KERNEL_OK` and `Boot test passed`.

## Decisions and limitations

- This uses the repository's established 20,000-iteration availability bound.
  Scheduler-backed waits remain a separate architectural improvement.

## Delivery strategy

- Working branch: `hdae/issue-62-poll-budget`
- Intended base: `main`
- Pull request: [#70](https://github.com/gmkurtzer/ginux/pull/70), with a
  non-closing reference to issue #62
- Planned commits: gate definition and bounded mechanism; evidence and delivery
- Actual commits: `72b3a757` (gate, implementation, regression, and evidence),
  followed by the delivery-state commit containing this update

## Review policy and outcome

Passed and closed under the user's authorized proactive-remediation policy.
All technical criteria passed and the dedicated branch and PR #70 exist; merge
remains subject to the separate two-pass policy.
