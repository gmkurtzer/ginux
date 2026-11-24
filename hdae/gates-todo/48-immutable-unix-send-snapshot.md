# Snapshot pathname Unix sends before host effect

## Source

[Issue #297](https://github.com/gmkurtzer/ginux/issues/297), follow-up to the
socket-alias repair delivered by Gate 47 and pull request #308.

## Goal

Ensure a pathname Unix `sendto`, `sendmsg`, or `sendmmsg` executes only from one
supervisor-owned immutable snapshot, so a sharing sibling cannot change the
destination, payload, vectors, or ancillary data after validation.

## Approved scope

- Reuse the bounded native-network snapshot mapping and replay lifecycle for
  pathname Unix sends.
- Snapshot and validate every pathname, payload vector, message header, and
  ancillary-data region before any host send effect.
- Keep abstract Unix addresses and unsupported socket families fail-closed.
- Keep outbound `SCM_RIGHTS` denied from the same immutable snapshot.
- Cover connected and pathname-addressed `sendto`, `sendmsg`, and `sendmmsg`.
- Add deterministic regression evidence for immutable replay and existing
  pathname/ancillary denials.

## Non-goals

- Do not add general `SCM_RIGHTS` support.
- Do not change instance-root pathname containment.
- Do not change Internet, netlink, connect, or bind policy; issue #300 remains
  a separate gate.

## Completed work

- [x] Confirmed current `main` validates pathname Unix `sendmsg` from guest
      memory and then allows the host kernel to reread that mutable memory.
- [x] Confirmed the existing bounded snapshot owner can be extended without a
      new network backend or authority model.
- [x] Routed connected and pathname-addressed Unix `sendto`, `sendmsg`, and
      `sendmmsg` through the bounded read-only snapshot mapping.
- [x] Added a deterministic Linux regression that mutates the caller pathname
      and payload only after Ginux has staged the replay, proves the snapshot
      mapping is read-only, and verifies only the original bytes reach the
      original receiver.
- [x] Passed static and complete component-unit verification on macOS.

## Remaining work

- [x] Extend snapshot validation and replay to pathname Unix sends.
- [x] Add deterministic regressions for mutation and ancillary-data denial.
- [ ] Run the Linux focused regression plus security and aggregate verification.
- [x] Commit, push, open the issue-linked pull request, and add reciprocal
      comments.

## Acceptance criteria

- The host send consumes only a supervisor-owned read-only snapshot of every
  userspace object for pathname Unix `sendto`, `sendmsg`, and `sendmmsg`.
- Mutation after the snapshot cannot alter the host-visible pathname, payload,
  vector layout, or ancillary data.
- Connected pathname Unix sends continue to work.
- Abstract Unix destinations, malformed or oversized inputs, unsupported
  families, and outbound `SCM_RIGHTS` remain denied before host effect.
- Existing Internet, netlink, socket-alias, console, process, contract,
  security, and aggregate regressions remain green.

## Verification plan

- Extend the native fixture and supervision oracle with pathname Unix sends and
  a controlled post-snapshot mutation; expect only snapshotted bytes and no
  transferred descriptor.
- `make check` -> formatting, strict lint, inventories, and generated docs pass.
- `make test-unit` -> all component tests pass.
- Linux `make test-native-supervision` -> the immutable Unix-send regression and
  existing native networking matrix pass.
- `make test-contract`, `make test-security`, and `make test` -> existing
  policy, boundary, and aggregate regressions pass.
- Fresh pull-request CI at the exact head -> every required job passes.

## Evidence

- Exact-head CI initially reached the new regression but timed out receiving
  the datagram because the root-created Unix socket nodes were not writable by
  Ginux's deliberately unprivileged host tracee. The fixture now mirrors the
  established inbound-rights setup by making only its disposable test socket
  nodes mode `0777`; no runtime permission or mediation rule changed.

- Current `native_unix_send_allowed` reads the guest `msghdr`, destination, and
  control data, while the admitted syscall later executes against the original
  guest pointers.
- The Internet send path already copies the same object graph into a private
  read-only tracee mapping and replays the host syscall from that mapping.
- `make check` passed formatting, strict lint, inventories, and generated-doc
  consistency on implementation commit `9d0e30d7`.
- `make test-unit` passed all component unit and documentation tests on the
  same commit.
- `python3 -m py_compile scripts/test-native-unix-send-snapshot.py` passed with
  an isolated bytecode cache. The Linux/root execution remains pending CI or a
  Rocky host because this macOS host cannot execute the native ptrace fixture.

## Decisions and limitations

- Pathname containment is necessary but not sufficient: validation and host
  effect must consume the same bytes.
- The bounded immutable snapshot is the existing authority boundary and avoids
  inventing a second Unix-specific replay mechanism.

## Delivery strategy

- Working branch: `hdae/immutable-unix-sendmsg-snapshot`
- Intended base: `main`
- Pull request: intended; it will reference issue #297 without closing it until
  the gate passes exact-head review.
- Planned commits: immutable Unix-send replay and deterministic regressions;
  HDAE evidence and reciprocal links.
- Actual commits: `9d0e30d7` (immutable pathname Unix replay and deterministic
  regression), `2d7b6a37` (gate and local evidence), plus the pending delivery
  record commit.
- Pull request: https://github.com/gmkurtzer/ginux/pull/315
- Source comment: https://github.com/gmkurtzer/ginux/issues/297#issuecomment-7117505682
- Pull-request comment: https://github.com/gmkurtzer/ginux/pull/315#issuecomment-7117505588

## Review policy and outcome

Implementation and pull-request delivery are authorized by the user's current
request and the repository proactive-remediation policy. This is a security
checkpoint: keep the gate in `gates-todo/` for exact-head human review before
merge or issue closure.
