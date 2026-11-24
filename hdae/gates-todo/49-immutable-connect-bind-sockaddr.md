# Snapshot connect and bind addresses before host effect

## Source

[Issue #300](https://github.com/gmkurtzer/ginux/issues/300), a static report
against the shared-network `connect` and `bind` policy path.

## Goal

Ensure Internet `connect` and `bind` policy, translation, and host execution all
consume one supervisor-owned immutable sockaddr snapshot.

## Approved scope

- Extend the bounded native-network mapping/replay owner to Internet `connect`
  and `bind`.
- Evaluate destination policy from the captured bytes and make the host syscall
  consume those exact bytes.
- Apply configured source-address and ingress-port translation only to the
  supervisor-owned snapshot.
- Preserve ordinary errno, restart, audit, socket-object state, and translated
  `getsockname` behavior.
- Add deterministic regressions that mutate the original sockaddr after replay
  is staged and prove the host consumes only the snapshot.

## Non-goals

- Do not change allowed protocols, ports, source addresses, or translations.
- Do not change pathname Unix handling or the Gate 48 Unix-send fix.
- Do not add a network backend or post-connect teardown policy.

## Completed work

- [x] Confirmed current `main` reads the guest sockaddr for policy and later
      runs the host syscall against the original mutable pointer.
- [x] Confirmed the existing native-network snapshot lifecycle can own
      `connect` and `bind` without a new authority mechanism.
- [x] Routed Internet `connect` and `bind` through the bounded read-only
      snapshot owner and evaluated policy and translation from captured bytes.
- [x] Added ptrace-controlled send, bind, and connect mutation regressions that
      change the original port only after the immutable replay is staged.

## Remaining work

- [ ] Run focused Linux, security, aggregate, and exact-head CI verification.
- [ ] Commit, push, open the issue-linked pull request, and add reciprocal
      comments.

## Acceptance criteria

- Internet `connect` and `bind` execute only against the same immutable bytes
  used for policy evaluation.
- Mutating the original sockaddr after capture cannot change the host-visible
  family, address, port, or configured translation.
- Denied ports remain denied without host effect; allowed IPv4 and IPv6 calls
  preserve Linux results and restart behavior.
- Source binding, ingress port translation, `getsockname`, audit, and shared
  socket-object state remain correct.
- Malformed, inaccessible, truncated, unsupported, or oversized addresses fail
  before host effect with bounded Linux-style errors.
- Existing network send, socket alias, Unix, netlink, contract, systemd,
  security, and aggregate regressions remain green.

## Verification plan

- Extend the native fixture and ptrace-controlled snapshot oracle to mutate an
  allowed `connect` and `bind` sockaddr after replay is staged; expect only the
  captured destination to take effect and the snapshot mapping to reject writes.
- Preserve existing allowed, denied, translated-port, source-binding, alias,
  and `getsockname` assertions in `test-native-supervision`.
- `make check` -> formatting, strict lint, inventories, and generated docs pass.
- `make test-unit` -> all component tests pass.
- Linux `make test-native-supervision` -> focused immutable sockaddr and native
  networking regressions pass.
- `make test-contract`, `make test-security`, and `make test` -> policy,
  boundary, and aggregate regressions pass.
- Fresh pull-request CI at the exact head -> every required job passes.

## Evidence

- `native_sockaddr_port` peeks the first guest word for policy; admitted
  `connect` runs later against the original address.
- The bind translation path temporarily overwrites the guest word and restores
  it after host execution, leaving an avoidable mutable-memory race.
- `native_network::Pending` already provides bounded map, replay, unmap,
  restart, result, and read-only snapshot semantics for network sends.
- `make check` -> passed on current `main` plus Gate 49.
- `make test-unit` -> passed all workspace component and oracle tests.
- `PYTHONPYCACHEPREFIX=/private/tmp/ginux-pycache python3 -m py_compile
  scripts/test-native-network-snapshot.py` -> passed.
- `make build` -> all Rust x86-64 Linux targets passed; the subsequent host C
  fixture compile is unavailable with the macOS arm64 compiler, so focused
  native execution is delegated to exact-head Linux CI.

## Decisions and limitations

- Post-call peer verification cannot prevent a denied connection from briefly
  taking host effect; pre-effect immutable replay is the correct boundary.
- This gate treats the report as a violated security invariant even though the
  reporter did not dynamically win the narrow race.

## Delivery strategy

- Working branch: `hdae/immutable-connect-bind-sockaddr`
- Intended base: `main`
- Pull request: intended; it will close issue #300 only after exact-head review
  and merge.
- Planned commits: immutable connect/bind snapshot and deterministic
  regressions; HDAE evidence and reciprocal links.
- Actual commits: pending until the signed implementation commit is created.

## Review policy and outcome

Implementation and pull-request delivery are authorized by the user's current
request and the repository proactive-remediation policy. This is a security
checkpoint: keep the gate in `gates-todo/` for exact-head human review before
merge or issue closure.
