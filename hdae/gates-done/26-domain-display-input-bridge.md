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

- [x] Added generation-bound per-domain display and input endpoints.
- [x] Added bounded frame and input queues with explicit backpressure.
- [x] Added Linux-facing transport records and a ring-3 contract probe.
- [x] Added focus-gated physical input and labeled synthetic input.
- [x] Added resize, stop, restart, stale-generation, and cross-domain tests.
- [x] Extended strict native address-space validation for the eight-page
  graphics service without enlarging unrelated services.

## Remaining work

- [x] Implement bridge, Linux endpoint, isolation/stress tests, and evidence.

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

- `make test-domain-display-bridge` -> passed three host contract tests and the
  ring-3 QEMU bridge assertion.
- `make test-smp` -> passed 1-, 2-, and 4-CPU boots plus two 2-CPU stress runs.
- `make check` -> passed architecture, compatibility, build, and crate tests.
- `git diff --check` -> passed.

## Decisions and limitations

- A domain exports one composed desktop surface; individual Linux windows are
  not imported into the native window tree.

## Delivery strategy

- Working branch: `codex/controller-desktop`
- Intended base: `main`
- Pull request: shared integrated PR for Gates 19–30
- Planned commits: bridge ABI; Linux endpoint; isolation/lifecycle tests
- Actual commits: `0d7c4504` (bridge, integration, and tests); gate record in
  the following documentation commit

## Review policy and outcome

Passed and closed under the approved Gates 19–30 delegated batch on
2026-10-31. No individual human review was required at this boundary.
