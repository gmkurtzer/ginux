# Native Ginux display protocol

## Goal

Implement a bounded, versioned, capability-scoped protocol for native surfaces,
buffers, damage, frames, and input events.

## Approved scope

- Implement connection, registry, surface, buffer, seat, and callback objects.
- Implement shared-buffer publication, damage, commit, frame completion, release,
  configure, focus, pointer, and keyboard events.
- Add generated or table-driven validation and adversarial protocol tests.

## Non-goals

- Stable public ABI beyond version zero, network transport, or Wayland wire
  compatibility.

## Completed work

- [ ] None.

## Remaining work

- [ ] Implement protocol, client library, fuzz/negative fixtures, and evidence.

## Acceptance criteria

- Negotiation rejects incompatible versions and unknown required features.
- Object IDs, message lengths, buffer dimensions/strides, damage, and event
  queues are bounded and validated before use.
- Published buffers cannot be mutated through compositor mappings until release.
- Disconnect destroys all owned objects and buffers without affecting peers.
- Two independent native clients render and receive isolated input/events.

## Verification plan

- `make test-native-display-protocol` -> conformance, multi-client isolation,
  malformed-message, buffer-lifetime, and disconnect tests pass.
- `make check` -> regression suite passes.

## Evidence

- Not yet run.

## Decisions and limitations

- The first protocol is Ginux-native and deliberately smaller than Wayland.

## Delivery strategy

- Working branch: `codex/controller-desktop`
- Intended base: `main`
- Pull request: shared integrated PR for Gates 19–30
- Planned commits: protocol ABI; server/client implementation; negative tests
- Actual commits: pending

## Review policy and outcome

Delegated closure under the approved Gates 19–30 batch after all criteria pass.
