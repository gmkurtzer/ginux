# Controller desktop visual design

## Goal

Replace the proof-of-function controller screen with a coherent, readable,
professional native Ginux desktop while preserving the accepted architecture
and interaction model.

## Approved scope

- Produce and iteratively review an 800 by 600 UI/UX concept image before code.
- Define the visual hierarchy, palette, typography, spacing, window chrome,
  system status presentation, command surface, and pointer treatment.
- Translate the accepted concept into the allocation-free native renderer.
- Preserve keyboard, pointer, serial recovery, windowing, and controller behavior.
- Verify the implemented design using actual QEMU framebuffer screenshots.

## Non-goals

- Changing the controller architecture, management semantics, or domain model.
- GPU acceleration, general font loading, animation, or a production desktop.
- Copying an existing operating-system desktop or third-party brand.

## Completed work

- [x] Generated and reviewed an initial 800 by 600 visual concept.
- [x] Added the requested Events destination and accepted the resulting v2
  concept as the implementation reference.

## Remaining work

- [ ] Implement the accepted visual system in the native renderer.
- [ ] Update deterministic visual fixtures and documentation.
- [ ] Run real-QEMU visible-input, desktop, performance, and regression tests.
- [ ] Obtain human visual acceptance of the implemented framebuffer.

## Acceptance criteria

- The human accepts an explicit concept image as the implementation reference.
- The implemented QEMU desktop materially matches its hierarchy, palette,
  spacing, typography intent, controls, and pointer treatment.
- Text is legible, controls are visually distinguishable, and system state is
  understandable without relying on color alone.
- Keyboard and pointer interaction remain visible and functional.
- Existing controller desktop performance and regression budgets pass.

## Verification plan

- Human concept review -> accepted visual reference.
- Deterministic renderer tests -> stable expected pixels.
- `make test-controller-desktop-visible` -> real framebuffer and input changes.
- `make test-controller-desktop-all` -> integrated desktop regressions pass.
- `make test-controller-desktop-performance` -> published budgets pass.
- `make check` -> architecture, ABI, format, lint, and unit checks pass.

## Evidence

- `docs/images/controller-desktop-concept-v2.png` -> human-accepted 800 by 600
  visual reference containing Overview, Domains, Network, Storage, Devices, and
  Events navigation destinations.

## Decisions and limitations

- Initial direction is a restrained modern systems dashboard using dark
  navy/slate surfaces, a limited blue/teal status palette, compact information
  density, and a clean software-renderable pointer.
- The concept is a visual specification, not a bitmap shipped by the kernel.

## Delivery strategy

- Working branch: `codex/controller-desktop`
- Intended base: `main`
- Pull request: continue the integrated controller-desktop pull request
- Planned commits: accepted visual reference; native implementation and tests;
  evidence and gate closure
- Actual commits: pending

## Review policy and outcome

Human checkpoints are required for the concept image and the final implemented
QEMU framebuffer because visual quality is subjective. Objective implementation
and regression loops may proceed autonomously between those checkpoints.
