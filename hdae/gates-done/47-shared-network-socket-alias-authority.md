# Preserve shared-network policy across socket aliases

## Source

[Issue #297](https://github.com/gmkurtzer/ginux/issues/297), dynamically
reproduced against `main` at `8c7e6ba5` and rechecked against current `main`.

## Goal

Ensure every alias of a managed socket retains the same supervisor-owned
classification and network policy as the original descriptor, including tasks
sharing a Linux descriptor table.

## Approved scope

- Move managed-socket classification into the authoritative native descriptor
  table established by Gate 30.
- Represent each host socket as one supervisor-owned socket object referenced
  by descriptor aliases. Mutable object state (source binding, translated
  ports, protocol/family, and netlink acknowledgement state) is shared across
  aliases and copied descriptor tables; descriptor-local close-on-exec state is
  not stored in the socket object.
- Mirror socket-object authority across successful `dup`, `dup2`, `dup3`,
  `fcntl(F_DUPFD)`, and `fcntl(F_DUPFD_CLOEXEC)` operations.
- Clear replaced, closed, close-range, and close-on-exec socket aliases with
  ordinary Linux descriptor-table semantics.
- Follow `F_SETFD`, `FIOCLEX`, `FIONCLEX`, `dup3(O_CLOEXEC)`,
  `F_DUPFD_CLOEXEC`, and `close_range(CLOSE_RANGE_CLOEXEC)` precisely.
- Deny `SCM_RIGHTS` descriptor transfer before host effect until Ginux has a
  separately reviewed descriptor-import design. Passing a managed socket
  through ancillary data must never create an unclassified alias.
- Reject inbound `SCM_RIGHTS` before the receiving task resumes: close every
  host descriptor installed by `recvmsg` or `recvmmsg`, return `EPERM`, and
  fail the instance closed if the result cannot be parsed or reclaimed.
- Apply descriptor-table metadata changes only after the host operation's
  Linux semantics establish a mutation, while retaining the Gate 30 mutation
  barrier. Failed operations leave state unchanged except that Linux may
  consume a valid descriptor on `close(2)` errors other than `EBADF`.
- Preserve network destination checks, immutable snapshots, source binding,
  netlink validation, and pathname-Unix restrictions through every alias.
- Add deterministic regressions for same-task aliases, `CLONE_FILES` sharers,
  non-sharing forks, descriptor replacement, close, close-range, and exec.

## Non-goals

- Do not change which network destinations or socket families are permitted.
- Do not address the separate sockaddr TOCTOU report in issue #300.
- Do not revisit issues #298, #299, #301, or #302, which have independent
  current-main dispositions.
- Do not replace the host socket implementation or add a new network backend.
- Do not introduce general descriptor import through `SCM_RIGHTS`; this gate
  fails it closed rather than inferring authority from a received host fd.

## Completed work

- [x] Confirmed that current `main` tracks sockets per task and numeric fd.
- [x] Confirmed successful duplicate operations update only console authority,
      leaving socket aliases unclassified and routed to the generic executor.
- [x] Defined the corrective gate and dedicated delivery branch.
- [x] Moved managed sockets into supervisor-owned objects referenced by aliases
      in the authoritative descriptor table.
- [x] Preserved object state and descriptor-local close-on-exec behavior across
      duplicate, fork, `CLONE_FILES`, unshare, close, close-range, and exec.
- [x] Denied `SCM_RIGHTS` through `sendmsg` and `sendmmsg` before host effect.
- [x] Added positive, denied, translated-port, shared-table, fork, exec,
      replacement, capacity, ancillary-data, and failed-operation regressions.
- [x] Passed the focused Linux-native, unit, contract, security, and aggregate
      verification matrix.

## Remaining work

- [x] Pushed the signed implementation and opened reciprocal issue-linked pull
      request #308.
- [x] Obtained passing PR CI on the implementation and delivery head.
- [x] Close and reject inbound `SCM_RIGHTS` descriptors before userspace can
      observe or use them.
- [x] Reran focused and aggregate local verification after the inbound fix.
- [x] Obtained passing exact-head PR verification after the inbound fix.
- [ ] Obtain human security-checkpoint review.

## Acceptance criteria

- Denied destinations remain denied through every duplicate API and no packet
  reaches the host receiver.
- Allowed destinations and connected sends behave identically through aliases.
- A supported state transition through one alias is observed through every
  alias of the same socket object, including source binding and translated
  ports; AF_NETLINK remains fail-closed at creation.
- AF_NETLINK creation remains fail-closed, and unsupported non-INET aliases
  cannot bypass their bounded validators or fail-closed paths.
- Outbound `SCM_RIGHTS` transfer is denied before host effect. Inbound
  `SCM_RIGHTS` descriptors are closed before the receiving task resumes, the
  call returns `EPERM`, and no unclassified descriptor survives.
- `CLONE_FILES` tasks observe one authoritative socket table; a non-sharing
  fork receives a consistent copy.
- Replacement, close, close-range, and exec remove exactly the affected alias
  without corrupting surviving aliases or unrelated descriptors.
- Every close-on-exec setter and creator updates only descriptor-local state;
  `close_range(CLOSE_RANGE_CLOEXEC)` marks rather than closes, and exec removes
  only marked aliases.
- Failed duplicate, replacement, `close(EBADF)`, and close-range operations
  leave the supervisor table unchanged. A forced `CLONE_FILES` sibling cannot
  interleave a descriptor mutation between the host result and supervisor
  bookkeeping. Other `close(2)` errors follow Linux's early fd-release rule.
- Existing console, pipe, virtual-kernel, shared-network, systemd, security,
  static, and unit regressions remain green.

## Verification plan

- Extend the native fixture and `scripts/test-native-supervision.py` with a
  policy-denied UDP receiver and all duplicate APIs; expect `EPERM` and zero
  host packets for every alias.
- Add forced `CLONE_FILES`, fork, exec, replacement, close, and close-range
  cases; expect descriptor classification to follow Linux table semantics.
- Add shared-object state tests that connect through one alias and send through
  another, and prove the current AF_NETLINK creation boundary remains
  fail-closed.
- Add `F_SETFD`, `FIOCLEX`, `FIONCLEX`, `dup3(O_CLOEXEC)`,
  `F_DUPFD_CLOEXEC`, `close_range(CLOSE_RANGE_CLOEXEC)`, failed-operation, and
  deterministic mutation-barrier cases.
- Add an ancillary-data fixture that attempts `SCM_RIGHTS`; expect `EPERM`, no
  receiver-visible descriptor, and no host-side transfer. Drive inbound rights
  from a host-side Unix socket for both `recvmsg` and `recvmmsg`; expect every
  installed descriptor to be closed before the tracee resumes.
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
- Before this fix, `NativeSupervisorTask.socket_fds` was copied per task even
  when the kernel tasks shared `CLONE_FILES`; `NativeDescriptorTable` was the
  existing authoritative sharing and mutation-barrier boundary.
- `make check` passed formatting, strict lint, inventories, and generated-doc
  consistency after the implementation.
- `make test-unit` passed every component unit and documentation test.
- The complete `test-native-supervision` command set passed in the Rocky 10
  QEMU guest, including socket-alias authority, yield-broker, clone-snapshot,
  clone-restart, and immutable-network-snapshot checks.
- `make test-contract` passed contract unit and generated coverage checks.
- `make test-security` passed all nine bounded fuzz targets and the native VFS,
  contract-runtime, device, and virtual-kernel QEMU checks.
- A self-contained `make test` passed static, CLI, host-lifecycle, unit,
  intercept, runtime, configuration, instance, image, device, process, and
  virtual-kernel regressions.
- Pull request #308 CI passed `changes`, `launch-integration`, and
  `static-and-unit` on implementation and delivery head `e569089e`.
- The Rocky host-driven native regression passed descriptors from an external
  Unix-stream peer: `recvmsg` installed two descriptors and `recvmmsg`
  installed one. Ginux closed all three before the tracee resumed, returned
  `EPERM` from both calls, and retained all existing socket-alias, credential,
  clone, and network behavior.
- Fresh `make check`, `make test-unit`, `make test-contract`, `make
  test-security`, and aggregate `make test` passed after inbound rejection was
  added. The security run also passed all nine bounded fuzz targets and its
  native VFS, contract-runtime, device, and virtual-kernel QEMU checks.
- Pull request #308 CI passed `changes`, `launch-integration`, and
  `static-and-unit` on inbound-fix evidence head `d1abc738`; the integration
  job included the complete native-supervision regression.

## Decisions and limitations

- The correct authority is the shared descriptor table and referenced socket
  object, never a task-local numeric fd map.
- Socket-object identity is shared across ordinary fork copies because Linux
  descriptor-table copies still reference the same open file description.
- `SCM_RIGHTS` is fail-closed for this gate; supporting imported descriptors
  requires a future explicit authority and lifecycle design.
- Inbound receive calls hold the authoritative descriptor-table mutation
  barrier until every returned descriptor is closed. The receiver sees
  `EPERM` only after cleanup. Malformed ancillary results, interruption during
  cleanup, and more than the bounded 253 collected descriptors fail the whole
  instance closed so no unclassified descriptor survives.
- Malformed ancillary data retains its ordinary kernel or bounded-snapshot
  error; only a positively identified `SCM_RIGHTS` transfer is rewritten to
  `EPERM`, avoiding unrelated errno changes.
- Host-executed `close(2)` removes an alias on every result except `EBADF`,
  matching Linux's early descriptor release; an emulated protected close does
  not mutate the supervisor table.
- This is a security checkpoint. Technical passage does not authorize merge or
  gate closure without exact-head review and human acceptance.

## Delivery strategy

- Working branch: `hdae/network-socket-alias-policy`
- Intended base: `main`
- Pull request: https://github.com/gmkurtzer/ginux/pull/308 references issue
  #297 without closing it and contains the actual fix and regressions.
- Planned commits: authoritative socket-table state; alias/lifecycle handling;
  deterministic regressions; evidence and reciprocal links.
- Actual commits: `c973279f` (socket-object authority and lifecycle),
  `7563ebcc` (deterministic alias and ancillary regressions), `4cda05da`
  (Linux close-result bookkeeping and regression), and `8dd928ab` (inbound
  descriptor reclamation and host-driven regressions); subsequent signed
  documentation commits record local evidence, pull-request delivery, and CI.

## Review policy and outcome

Automatic implementation and pull-request delivery are authorized by the
repository's scheduled proactive-remediation policy. Keep Gate 47 in
`gates-todo/` as a security checkpoint pending human review; never merge it
automatically through the HDAE implementation loop. Local technical evidence
passes after the inbound fix. Exact-head pull-request CI and human acceptance
are split at the checkpoint: CI passes on inbound-fix evidence head
`d1abc738`, while human acceptance remains pending.

## Gate 51 reconciliation

- Pull request #308 is merged into `main`; authoritative socket objects,
  descriptor-local close-on-exec state, mutation barriers, outbound denial,
  and inbound `SCM_RIGHTS` reclamation are present in the current tree.
- Fresh Gate 51 static, unit, aggregate, contract, security, recovery,
  systemd, and multi-instance verification rechecks the surrounding network,
  descriptor, and lifecycle boundaries; the original exact-head Linux suite
  remains the criterion-specific descriptor-transfer evidence.
- Technical status: passed. Lifecycle status remains open pending the
  consolidated human security acceptance checkpoint. General `SCM_RIGHTS`
  support remains intentionally outside this gate.
