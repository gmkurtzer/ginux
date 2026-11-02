# Gate 38: Bound Blocking Socket Polls

## Source

[GitHub issue #22](https://github.com/gmkurtzer/ginux/issues/22).

## Goal

Prevent guest-controlled socket timeouts or timeout-free accepts from holding
the serialized kernel-entry path in an unbounded busy-wait.

## Approved scope

- Apply one fixed maximum poll budget to blocking accept, connect, send, recv,
  peek, and wait-all loops.
- Clamp guest-provided send/receive timeouts to that budget.
- Return the existing `EAGAIN`/partial-transfer behavior when the bounded
  budget expires.
- Add deterministic maximum, zero-timeout, huge-timeout, and partial-transfer
  regressions.

## Non-goals

- Do not redesign the Linux socket implementation around scheduler wait/wake
  in this bounded availability fix.
- Do not change socket readiness, nonblocking, or timeout option ABI.
- The separate `getrandom` loop mentioned in the report is not a socket path
  and remains separate follow-up work.
- Do not merge the PR or close issue #22.

## Completed work

- [x] Confirmed blocking socket loops execute under `kernel_entry_lock`.
- [x] Confirmed guest timeout conversion is currently unbounded.
- [x] Confirmed timeout-free `accept_blocking` loops forever.
- [x] Centralized zero, absent, finite, and overflowing timeout conversion under
  `BLOCKING_POLL_LIMIT`.
- [x] Bounded blocking accept, connect, send, receive, peek, and wait-all paths.
- [x] Added deterministic boot-time poll-budget verification and a QEMU marker.
- [x] Passed focused compilation, the complete static gate, and the complete
  QEMU boot regression.
- [x] Published PR #47 and added reciprocal issue/PR summaries.

## Remaining work

- [ ] GitHub CI is external evidence and must finish before merge.

## Acceptance criteria

- No blocking socket loop exceeds `BLOCKING_POLL_LIMIT` iterations.
- A huge `SO_RCVTIMEO` or `SO_SNDTIMEO` cannot enlarge the critical-section
  poll budget.
- A zero timeout and timeout-free accept remain bounded.
- Existing success and partial-transfer behavior remains intact.
- Static and complete QEMU verification pass.

## Verification plan

- Pure poll-budget regression -> zero, small, exact-limit, huge, and conversion
  overflow inputs never exceed the fixed limit.
- Source inspection -> every blocking socket loop uses the bounded helper and
  no open-ended accept loop remains.
- Existing Linux socket and ABI checks -> success, timeout, and partial I/O pass.
- `make check` and `make test` -> full static and boot checks pass.

## Evidence

- Issue #22's code-path report matches `kernel/src/linux_socket.rs`.
- The existing `BLOCKING_POLL_LIMIT` provides an established repository bound;
  it was not consistently applied to guest timeouts or accept.
- `cargo check --package ginux-kernel --target x86_64-unknown-none` passed.
- `make check` passed, including the unchanged 1,033-block unsafe baseline and
  the Linux compatibility matrix.
- `make test` passed and emitted
  `blocking Linux socket polls bounded independently of guest timeout`.
- Source inspection found every blocking socket loop obtains its iteration
  count through the centralized bounded helper; no open-ended accept loop
  remains.

## Decisions and limitations

- This gate applies the reporter's bounded-budget mitigation. Replacing bounded
  polling with scheduler-backed socket waits is a larger semantic redesign and
  remains future work.

## Delivery strategy

- Working branch: `hdae/issue-22-blocking-sockets`
- Intended base: `main`
- Pull request: [#47](https://github.com/gmkurtzer/ginux/pull/47), with a
  non-closing reference to issue #22
- Planned commits: gate definition; bounded poll mechanism and regression;
  evidence; delivery
- Actual commits: `b8538b6e` (gate definition), `38c52ae3` (mechanism and
  regression), plus the evidence/delivery commit containing this record

## Review policy and outcome

Passed and closed under the user-approved remaining-reports batch delegation.
The bounded-budget mitigation preserves the existing socket ABI; a scheduler-
backed wait redesign was not required.
