# Domain display and input bridge

## Goal

Expose an isolated virtual display and virtual input path to each authorized
domain and connect it to one native controller surface.

## Approved scope

- Add per-domain virtual display buffers, modes, damage, frame completion, and
  keyboard/pointer event queues.
- Bind bridge endpoints to domain capabilities, generations, lifecycle, focus,
  and resource limits.
- Provide a Linux-facing device or userspace transport suitable for Gate 27.

## Non-goals

- Wayland protocol implementation, GPU passthrough, or multiple exported
  surfaces per domain.

## Completed work

- [ ] None.

## Remaining work

- [ ] Implement bridge, Linux endpoint, isolation/stress tests, and evidence.

## Acceptance criteria

- Each domain can publish only to its own bounded desktop surface.
- Only the focused authorized domain receives physical input; synthetic test
  input remains separately labeled and authorized.
- Resize, stop, restart, and disconnect invalidate stale buffers and endpoints.
- Concurrent domains cannot read pixels, events, handles, or timing queues from
  one another.
- Backpressure bounds frame and input production without stalling the controller.

## Verification plan

- `make test-domain-display-bridge` -> frame, input, resize, backpressure,
  lifecycle, stale-generation, and cross-domain negative tests pass.
- `make test-smp` and `make check` -> concurrency and regression suites pass.

## Evidence

- Not yet run.

## Decisions and limitations

- A domain exports one composed desktop surface; individual Linux windows are
  not imported into the native window tree.

## Delivery strategy

- Working branch: `codex/controller-desktop`
- Intended base: `main`
- Pull request: shared integrated PR for Gates 19–30
- Planned commits: bridge ABI; Linux endpoint; isolation/lifecycle tests
- Actual commits: pending

## Review policy and outcome

Delegated closure under the approved Gates 19–30 batch after all criteria pass.
