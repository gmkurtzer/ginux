# Enforce shared-network destination policy

## Source

[Issue #283](https://github.com/gmkurtzer/ginux/issues/283), implemented from
`main` at `f5ee0402b64ed0765dfcc0e43adac52466f2f91f`.

## Goal

Ensure every configured-native shared-network syscall that supplies a remote
destination is checked against the same outbound contract as `connect`, using
supervisor-owned immutable arguments for the host effect.

## Scope and non-goals

- Cover `sendto`, `sendmsg`, and every message in `sendmmsg`, including
  destination-bearing TCP Fast Open use.
- Copy destination metadata once into supervisor-owned memory, validate that
  copy, and make the host syscall consume the validated copy.
- Apply the existing protocol/port policy and source-address binding behavior.
- Bound message counts, nested metadata, destination sizes, and payload sizes.
- Handle missing destinations and non-INET families explicitly and fail closed
  for unsupported shapes before host effect.
- Preserve `connect` and `bind` behavior and existing shared-network ABI.
- Do not broaden allowed destinations, add a new network backend, or change
  unrelated socket calls.

## Delivery strategy

- Branch: `hdae/shared-network-destination-policy`
- Base: `main`
- Delivery: signed commits and a pull request referencing issue #283 only
  after the actual fix and regressions are present.
- Review policy: security checkpoint; keep in `gates-todo/` pending exact-head
  review and fresh Linux integration.

## Acceptance criteria

- A denied destination returns `EPERM` through `connect`, `sendto`, `sendmsg`,
  and `sendmmsg`, with no packet handed to the host stack.
- An allowed UDP destination succeeds through all three send APIs.
- A sibling thread cannot change the destination between validation and host
  consumption; a deterministic mutation regression proves the host consumes
  the supervisor snapshot.
- Non-INET destination families and malformed or oversized nested structures
  fail closed before host effect.
- Connected-socket sends without a supplied destination keep working.
- Existing shared-network, contract, security, and aggregate tests remain
  green.

## Verification plan

- Add a static native fixture for allowed and denied `sendto`, `sendmsg`, and
  `sendmmsg` destinations with an isolated local receiver.
- Add a synchronized sibling-mutation test that changes caller memory only
  after capture and proves the sent packet still targets the validated copy.
- Run `make check`, `make test-unit`, `sudo make test-native-supervision`,
  `make test-contract`, `make test-security`, and aggregate `make test`.

## Planned commits

1. Add bounded supervisor-owned shared-network send snapshots and policy reuse.
2. Add positive, negative, malformed, and concurrent-mutation regressions.
3. Record fresh gate evidence and reciprocal delivery links.

## Evidence

- Current mode 3 applies `network_allows` only to syscall 42 (`connect`) and
  49 (`bind`).
- Current argument admission allows the full shared-network syscall range,
  including 44, 46, and 307, without inspecting their destinations.
- The issue's dynamic oracle demonstrates denied `connect` returning `EPERM`
  while `sendto` to the same UDP port succeeds.
- `make check` -> passed after the immutable network-send implementation.
- `make test-unit` -> passed all component tests.
- `PYTHONPYCACHEPREFIX=/private/tmp/ginux-pycache-283 python3 -m py_compile
  scripts/test-native-supervision.py scripts/test-native-network-snapshot.py`
  -> passed.
- Native fixture compilation on the macOS arm64 development host is not a
  valid Linux/x86-64 check; the repository's Ubuntu CI job is the required
  execution environment for `sudo make test-native-supervision`.
- GitHub Actions run `43557689897` passed `changes`, `static-and-unit`, and
  `launch-integration` at corrective head `195a38bd`. The Ubuntu launch job
  passed `sudo make test-native-supervision`, including allowed and denied
  `sendto`/`sendmsg`/`sendmmsg`, bounds, connected sends, and deterministic
  post-capture mutation.
- `make test-contract` -> passed.
- `ROCKY_SSH_PORT=22485 ... make test-security` -> passed the complete unit,
  eight-target fuzz, native VFS, contract runtime, device, and virtual-kernel
  matrix in isolated Rocky QEMU.
- `ROCKY_SSH_PORT=22484 ... make test` -> passed the full aggregate suite in
  isolated Rocky QEMU after using the existing verified cache through a
  container-visible worktree-local layout.
- Delivery: [PR #289](https://github.com/gmkurtzer/ginux/pull/289).
- Source update:
  https://github.com/gmkurtzer/ginux/issues/283#issuecomment-6871851836
- Reciprocal PR update:
  https://github.com/gmkurtzer/ginux/pull/289#issuecomment-6871851761

## Completed work

- [x] Added bounded read-only snapshots for `sendto` and `sendmsg` payload,
  destination, iovec, and control metadata.
- [x] Replayed bounded `sendmmsg` batches through snapshot-backed `sendmsg`
  calls while preserving Linux partial-result and `msg_len` behavior.
- [x] Applied outbound protocol/port policy to every explicit destination and
  retained connected sends with no supplied destination.
- [x] Added positive, denied, malformed, oversized, connected-send, and
  deterministic post-capture mutation regressions.

## Remaining work

- [x] Run the exact branch head through Linux native-supervision CI.
- [x] Run the remaining contract, security, and aggregate regression commands.
- [x] Deliver the pull request and reciprocal links, then stop for exact-head
  security review.

## Status

- Gate 29 is technically passed at corrective PR head `195a38bd`. Every
  acceptance criterion has fresh Linux or aggregate evidence. It remains in
  `gates-todo/` for exact-head security review and human acceptance.

## Actual commits

- `8476539c` — immutable bounded native network-send snapshots.
- `b2812bdc` — positive, negative, bounds, connected-send, and mutation tests.
- `195a38bd` — implementation checkpoint and local evidence.
