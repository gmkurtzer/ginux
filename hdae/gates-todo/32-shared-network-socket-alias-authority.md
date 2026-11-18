# Preserve shared-network policy across socket aliases

## Source

[Issue #297](https://github.com/gmkurtzer/ginux/issues/297), dynamically
reproduced against `main` at `8c7e6ba5`.

## Goal

Ensure every alias of a managed socket retains the same supervisor-owned
classification and network policy as the original descriptor, including tasks
sharing a Linux descriptor table.

## Approved scope

- Move managed-socket classification into the authoritative native descriptor
  table established by Gate 30.
- Mirror socket identity and state across successful `dup`, `dup2`, `dup3`,
  `fcntl(F_DUPFD)`, and `fcntl(F_DUPFD_CLOEXEC)` operations.
- Clear replaced, closed, close-range, and close-on-exec socket aliases with
  ordinary Linux descriptor-table semantics.
- Preserve network destination checks, immutable snapshots, source binding,
  netlink validation, and pathname-Unix restrictions through every alias.
- Add deterministic regressions for same-task aliases, `CLONE_FILES` sharers,
  non-sharing forks, descriptor replacement, close, close-range, and exec.

## Non-goals

- Do not change which network destinations or socket families are permitted.
- Do not address the separate default-policy, sockaddr TOCTOU, snapshot-range,
  `syncfs`, or netlink-ACK reports from issues #298 through #302.
- Do not replace the host socket implementation or add a new network backend.

## Completed work

- [x] Confirmed that current `main` tracks sockets per task and numeric fd.
- [x] Confirmed successful duplicate operations update only console authority,
      leaving socket aliases unclassified and routed to the generic executor.
- [x] Defined the corrective gate and dedicated delivery branch.

## Remaining work

- [ ] Move socket state to the shared descriptor-table authority.
- [ ] Propagate and invalidate aliases for every duplication and close path.
- [ ] Add positive, denied, shared-table, fork, exec, and reuse regressions.
- [ ] Run the focused and aggregate verification matrix.
- [ ] Push the signed implementation and open a reciprocal issue-linked PR.

## Acceptance criteria

- Denied destinations remain denied through every duplicate API and no packet
  reaches the host receiver.
- Allowed destinations and connected sends behave identically through aliases.
- AF_NETLINK and unsupported non-INET aliases cannot bypass their bounded
  validators or fail-closed paths.
- `CLONE_FILES` tasks observe one authoritative socket table; a non-sharing
  fork receives a consistent copy.
- Replacement, close, close-range, and exec remove exactly the affected alias
  without corrupting surviving aliases or unrelated descriptors.
- Existing console, pipe, virtual-kernel, shared-network, systemd, security,
  static, and unit regressions remain green.

## Verification plan

- Extend the native fixture and `scripts/test-native-supervision.py` with a
  policy-denied UDP receiver and all duplicate APIs; expect `EPERM` and zero
  host packets for every alias.
- Add forced `CLONE_FILES`, fork, exec, replacement, close, and close-range
  cases; expect descriptor classification to follow Linux table semantics.
- `make check` -> formatting and strict lint pass.
- `make test-unit` -> all component tests pass.
- `sudo make test-native-supervision` -> focused socket-alias tests pass on
  Linux without privileged access outside the disposable test host.
- `make test-contract`, `make test-security`, and `make test` -> existing
  contract, boundary, and aggregate regressions pass.
- Fresh PR CI at the exact head -> all required jobs pass.

## Evidence

- Issue #297 demonstrates the tracked descriptor denying UDP port 9999 while
  `dup`, `dup2`, and `F_DUPFD` aliases deliver three packets to that port.
- Static tracing confirms all shared-network send arms require a
  `socket_fds` entry, while duplicate exit handling updates only console state.
- Current `NativeSupervisorTask.socket_fds` is copied per task even when the
  kernel tasks share `CLONE_FILES`; `NativeDescriptorTable` is the existing
  authoritative sharing and mutation-barrier boundary.

## Decisions and limitations

- The correct authority is the shared descriptor table and referenced socket
  object, never a task-local numeric fd map.
- This is a security checkpoint. Technical passage does not authorize merge or
  gate closure without exact-head review and human acceptance.

## Delivery strategy

- Working branch: `hdae/network-socket-alias-policy`
- Intended base: `main`
- Pull request: intended; it will reference issue #297 and contain the actual
  fix and regressions before delivery.
- Planned commits: authoritative socket-table state; alias/lifecycle handling;
  deterministic regressions; evidence and reciprocal links.
- Actual commits: pending.

## Review policy and outcome

Automatic implementation and pull-request delivery are authorized by the
repository's scheduled proactive-remediation policy. Keep Gate 32 in
`gates-todo/` as a security checkpoint pending human review; never merge it
automatically through the HDAE implementation loop.
