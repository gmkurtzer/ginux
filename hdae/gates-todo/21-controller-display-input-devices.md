# Controller display and input devices

## Goal

Provide controller-owned framebuffer, keyboard, and pointer devices with
normalized events and deterministic QEMU behavior.

## Approved scope

- Add a software framebuffer path and bounded mode/scanout interface.
- Add keyboard and relative-pointer input with normalized key/button/motion
  events, overflow handling, and device reset.
- Grant device access only to the graphical controller service.

## Non-goals

- GPU acceleration, DRM/KMS, raw domain passthrough, touch, or multiple displays.

## Completed work

- [ ] None.

## Remaining work

- [ ] Implement devices, negative tests, QEMU injection, and evidence.

## Acceptance criteria

- QEMU boots at one fixed supported mode and updates visible pixels correctly.
- Injected keyboard and pointer events arrive once, in order, with bounded queues.
- Malformed modes, buffers, coordinates, and unauthorized callers are rejected.
- Device reset and client failure return a clean neutral input state.
- Serial CLI and existing virtio/IOMMU tests remain functional.

## Verification plan

- `make test-controller-display-input` -> pixel fixture, injected input, bounds,
  authorization, overflow, and reset assertions pass.
- `make check` -> regression suite passes.

## Evidence

- Not yet run.

## Decisions and limitations

- Initial output is software rendered through the QEMU-supported framebuffer;
  physical input/output remains controller owned.

## Delivery strategy

- Working branch: `codex/controller-desktop`
- Intended base: `main`
- Pull request: shared integrated PR for Gates 19–30
- Planned commits: framebuffer; input; deterministic QEMU tests
- Actual commits: pending

## Review policy and outcome

Delegated closure under the approved Gates 19–30 batch after all criteria pass.
