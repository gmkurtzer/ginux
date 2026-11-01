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

- [x] Added a no-std native display protocol/client library with explicit V0
  negotiation and required-feature rejection.
- [x] Added generation-scoped client capabilities plus connection, registry,
  seat, surface, buffer, callback, damage, commit, frame-completion, release,
  configure, focus, pointer, and keyboard state/events.
- [x] Bounded clients, objects, object IDs, wire messages, dimensions, strides,
  damage rectangles, and event queues.
- [x] Enforced immutable published-buffer state until compositor release and
  transactional frame-completion event capacity.
- [x] Added two-client isolation, disconnect/stale-capability cleanup, table-
  driven malformed messages, overflow, cross-client, and lifetime tests.
- [x] Executed the same conformance probe inside the graphical ring-3 service.

## Remaining work

- [x] None.

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

- `make test-native-display-protocol` -> all four host conformance/adversarial
  tests passed and the two-vCPU QEMU verification boot confirmed the same
  protocol contract inside the isolated graphical ring-3 service.
- `make check` -> passed formatting, clippy, native C ABI, architecture,
  compatibility, and all workspace unit/doc tests.
- `git diff --check` -> passed.

## Decisions and limitations

- The first protocol is Ginux-native and deliberately smaller than Wayland.
- V0 uses fixed-capacity tables and one composed surface tree per server; later
  gates attach compositor policy and actual shared page mappings to this
  already-enforced publication/release state machine.
- Input delivery is explicitly addressed to a client/surface in this gate;
  Gate 23 supplies seat focus and lease policy.

## Delivery strategy

- Working branch: `codex/controller-desktop`
- Intended base: `main`
- Pull request: shared integrated PR for Gates 19–30
- Planned commits: protocol ABI; server/client implementation; negative tests
- Actual commits: `0ab16189` (protocol library, ring-3 conformance, and tests)

## Review policy and outcome

Passed and closed under the delegated Gates 19–30 batch on 2026-10-31. Every
criterion has fresh deterministic evidence; no individual human review was
required by the confirmed checkpoint map.
