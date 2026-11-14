# Enforce shared-network destination policy

## Source

[Issue #283](https://github.com/gmkurtzer/ginux/issues/283), reviewed against
`main` at `33948c62b0eb59546f0b8a0a0d05592a083c481b`.

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

## Status

- Confirmed and aligned. Implementation is active; no pull request exists yet
  because the delivery must include the complete corrective code and tests.

