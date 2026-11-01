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

- [x] Added the versioned UEFI GOP framebuffer handoff and bounded fixed-mode
  validation.
- [x] Added controller-owned framebuffer pixel operations with volatile
  write/readback verification.
- [x] Added bounded normalized keyboard, pointer-motion, and pointer-button
  queues with stable sequence numbers, overflow rejection, and reset.
- [x] Added capability-scoped display/input operations used by the isolated
  graphical ring-3 service, including wrong-capability rejection.
- [x] Extended the signed boot authority envelope and native C/Rust ABI
  contracts for the new devices.
- [x] Added deterministic QEMU assertions for device, authorization, cleanup,
  CLI, and existing platform regressions.

## Remaining work

- [x] None.

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

- `make test-controller-display-input` -> passed the verification boot under
  two-vCPU QEMU/TCG, including GOP pixel write/readback, ordered normalized
  input injection, queue bounds, capability rejection, neutral reset, service
  retirement, serial controller startup, clean shutdown, and the existing
  virtio/IOMMU/Linux runtime assertions.
- `make check` -> passed formatting, clippy, native C ABI, architecture,
  compatibility, and all workspace unit/doc tests.
- `make architecture-check` -> passed with 485 classified writable globals and
  the unchanged kernel unsafe baseline of 1,033 blocks.
- `git diff --check` -> passed.

## Decisions and limitations

- Initial output is software rendered through the QEMU-supported framebuffer;
  physical input/output remains controller owned.
- Gate verification injects normalized keyboard and relative-pointer events at
  the controller device boundary. Hardware-specific input ingestion remains
  behind that boundary and does not change the service protocol.
- The initial mode is the UEFI GOP mode selected by firmware; mode switching,
  multiple displays, and accelerated scanout remain non-goals.

## Delivery strategy

- Working branch: `codex/controller-desktop`
- Intended base: `main`
- Pull request: shared integrated PR for Gates 19–30
- Planned commits: framebuffer; input; deterministic QEMU tests
- Actual commits: `ac630cd2` (devices, ABI, ring-3 validation, QEMU tests) and
  `a3245f67` (framebuffer write readback).

## Review policy and outcome

Passed and closed under the delegated Gates 19–30 batch on 2026-10-31. Every
criterion has fresh deterministic evidence; no individual human review was
required by the confirmed checkpoint map.
