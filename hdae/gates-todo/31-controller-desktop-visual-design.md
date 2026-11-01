# Controller GUI visual design and interaction

## Goal

Replace the proof-of-function controller screen with a coherent, readable,
responsive, and operational native Ginux GUI while preserving the accepted
architecture and serial recovery path.

## Approved scope

- Produce and iteratively review an 800 by 600 UI/UX concept image before code.
- Define the visual hierarchy, palette, typography, spacing, window chrome,
  system status presentation, management surface, and pointer treatment.
- Translate the accepted concept into the allocation-free native renderer.
- Decouple graphical input from blocking serial reads and coalesce pointer
  motion so movement remains responsive under QEMU.
- Route pointer buttons, hit testing, focus, navigation, and supported actions
  through the native controller application.
- Replace demonstration state with live kernel-backed management snapshots and
  preserve CLI/GUI state equivalence.
- Replace scaled 3 by 5 glyphs with a readable native-resolution font and
  refined software-rendered iconography.
- Preserve keyboard, pointer, serial recovery, windowing, and controller behavior.
- Verify the implemented design using actual QEMU framebuffer screenshots.
- Replace the rejected centered low-resolution window with a native 800 by 600
  controller shell that consumes the complete configured display.
- Remove the nonfunctional graphical command prompt and fake
  minimize/maximize/close controls; keep the serial CLI as the recovery path.
- Add a configurable theme/layout contract, a refined pointer, and a trusted
  power control with explicit reboot and shutdown choices.
- Select and fill a true 1024 by 768 GOP mode when firmware exposes it, with a
  deterministic fallback only when that mode is unavailable.
- Replace transient full-frame composition and per-pixel native blitting with
  a persistent, damage-aware presentation path sized on demand for the selected
  mode.
- Replace the softened 5 by 7 bitmap treatment with a legible, antialiased
  typeface suitable for a native-resolution systems-management interface.

## Non-goals

- Changing the controller trust boundary, domain model, or management semantics.
- GPU acceleration, runtime font loading, animation, or a production desktop.
- Copying an existing operating-system desktop or third-party brand.

## Completed work

- [x] Generated and reviewed an initial 800 by 600 visual concept.
- [x] Added the requested Events destination and accepted the resulting v2
  concept as the implementation reference.
- [x] Added a Settings destination for general system configuration, beginning
  with date, time, and timezone controls.
- [x] Implemented the accepted hierarchy, palette, cards, domain table,
  management surface, navigation, Events destination, status treatment, and
  software pointer in the allocation-free native renderer.
- [x] Updated deterministic fixtures, service image limits, sparse native
  service layout validation, and controller desktop documentation.
- [x] Passed real-QEMU visible input, complete desktop, performance, and full
  repository regression qualification.
- [x] Replaced coarse 160 by 100 composition with an interim 400 by 300 logical
  layout; later human review superseded this with the native 800 by 600 result.
- [x] Removed full-frame clearing from incremental input repaints so interactive
  frames remain complete rather than visibly redrawing from top to bottom.

## Remaining work

- [x] Replace the visibly pixelated 3 by 5 scaled font and coarse iconography
  with a readable native visual treatment suitable for routine use.
- [x] Make pointer motion independent of serial keyboard activity, bounded,
  coalesced, and predictably responsive.
- [x] Implement pointer-button routing, hit testing, focus, navigation, and
  clickable supported controller actions.
- [x] Populate all displayed system and domain fields from live kernel state;
  remove fresh per-frame demonstration backends.
- [x] Prove CLI and GUI actions observe the same underlying state.
- [ ] Obtain human visual acceptance of the implemented framebuffer.
- [x] Render at native 800 by 600 resolution without doubled logical pixels,
  unused desktop margins, or nested window chrome.
- [x] Replace the block pointer with a styled, high-contrast cursor.
- [x] Remove the graphical command prompt and expose reboot/shutdown through a
  discoverable power menu with confirmation-safe interaction.
- [x] Centralize palette, density, and layout choices in a configurable UI
  theme rather than scattering visual constants through the renderer.
- [x] Render and visibly qualify the shell at 1024 by 768.
- [x] Remove full-frame recomposition and per-pixel copy work from pointer and
  ordinary control interaction.
- [x] Replace the bitmap-cell typography with antialiased glyph rendering.
- [x] Measure the complete controller-to-GOP interaction path in real QEMU and
  retain machine-checkable latency evidence.

## Acceptance criteria

- The human accepts an explicit concept image as the implementation reference.
- The implemented QEMU desktop materially matches its hierarchy, palette,
  spacing, typography intent, controls, and pointer treatment.
- Text is legible, controls are visually distinguishable, and system state is
  understandable without relying on color alone.
- Pointer motion is smooth and predictable under QEMU without requiring serial
  keystrokes, and button transitions are neither lost nor duplicated.
- Navigation and supported controls are clickable; disabled controls are
  visibly and behaviorally inert.
- Displayed system and domain state updates after CLI or GUI changes without a
  controller restart or demonstration fixture.
- Keyboard and pointer interaction remain visible and functional while the
  serial controller remains an independent recovery path.
- Existing controller desktop performance and regression budgets pass.
- The configured 800 by 600 QEMU mode is filled by a native-resolution shell;
  no 400 by 300 composition is scaled to fit it.
- The controller has no fake window-management controls or nonfunctional
  graphical prompt, and the serial CLI continues to operate independently.
- A corner power control opens clearly labeled Reboot and Shut down actions;
  tests prove routing and host lifecycle behavior.
- Visual colors, spacing, and major dimensions come from one explicit theme and
  layout configuration with deterministic alternate-theme coverage.
- QEMU exposes a 1024 by 768 controller framebuffer and every major shell
  region is rendered at that native resolution; the test fails rather than
  silently accepting 800 by 600.
- Typography uses antialiased glyph coverage rather than scaled binary bitmap
  cells, and retained framebuffer evidence is readable at normal display scale.
- Pointer motion and ordinary clicks update only bounded damaged regions. A
  real-QEMU interaction test records end-to-visible-frame latency with a p95 at
  or below 50 ms and no sample above 100 ms on the qualification host.
- Full page changes avoid the prior userspace-to-kernel per-pixel syscall copy
  path and complete within 250 ms in the same real-QEMU qualification.

## Verification plan

- Human concept review -> accepted visual reference.
- Deterministic renderer tests -> stable expected pixels.
- Deterministic controller tests -> hit testing, button transitions, navigation,
  disabled controls, and CLI/GUI state equivalence pass.
- Deterministic renderer tests -> native dimensions, full-frame coverage,
  styled cursor, theme overrides, and power-menu states pass.
- `make test-controller-desktop-visible` -> real framebuffer, independent
  pointer motion, button clicks, keyboard input, and live-state changes pass.
- `make test-controller-desktop-all` -> integrated desktop regressions pass.
- `make test-controller-desktop-performance` -> published budgets pass.
- `make check` -> architecture, ABI, format, lint, and unit checks pass.
- Deterministic renderer tests -> 1024 by 768 layout, antialiased glyph alpha,
  bounded damage rectangles, and unchanged pixels outside damage pass.
- `make test-controller-desktop-visible` -> reports an exact 1024 by 768 GOP
  frame and retains a readable framebuffer image.
- `make test-controller-desktop-latency` -> real-QEMU pointer/click p95 is at
  most 50 ms, no sample exceeds 100 ms, and page presentation is at most 250 ms.

## Evidence

- `docs/images/controller-desktop-concept-v2.png` -> human-accepted 800 by 600
  visual reference containing Overview, Domains, Network, Storage, Devices, and
  Events navigation destinations.
- `cargo test -p ginux-windowing -p ginux-controller-ui` -> 8 deterministic
  renderer and controller tests passed.
- `make test-controller-desktop-visible` -> actual QEMU framebuffer rendered;
  injected keyboard and pointer input each changed the frame; PNG evidence was
  retained under `build/qualification/controller-desktop/`; rerun after adding
  Settings passed on 2026-11-01.
- `make test-controller-desktop-all` -> protocol, windowing, controller,
  multi-domain, Wayland, presentation, recovery, 2-CPU/4-CPU stress, boot, and
  visible framebuffer qualification passed.
- `make test-controller-desktop-performance` -> startup 1 us,
  input-to-frame p95 475 us, steady-frame p95 252 us, recovery-cycle p95 0 us,
  and modeled working set 1,950,000 bytes; all budgets passed.
- `make check` -> architecture, ABI, format, lint, unit, and compatibility
  checks passed.
- `make test-controller-desktop-visible` -> the configured QEMU framebuffer
  contains the complete native 800 by 600 controller shell; retained input
  frames no longer contain partially cleared regions.
- `make test-controller-desktop-all` -> complete desktop suite and 2-CPU/4-CPU
  recovery stress passed after the high-resolution renderer change.
- `make test-controller-desktop-performance` -> startup 1 us,
  input-to-frame p95 426 us, steady-frame p95 282 us, recovery-cycle p95 0 us,
  and modeled working set 1,950,000 bytes; all budgets passed.
- `cargo test -p ginux-windowing -p ginux-controller-ui` -> 13 tests passed,
  including exact pixels, click navigation, configurable theme/layout, power
  confirmation, and live-state frame changes.
- `make test-controller-desktop-visible` -> real QEMU interaction passed on
  2026-11-01: independent pointer motion, Settings and Overview clicks, native
  keyboard input, and a CLI-created domain changing the live GUI were retained
  as screenshots under `build/qualification/controller-desktop/`.
- `make test-controller-desktop-all` -> the complete verification boot,
  protocol/windowing/controller/domain suites, 2-CPU and 4-CPU recovery stress,
  and visible framebuffer qualification passed on 2026-11-01.
- `make test-controller-desktop-performance` -> presentation startup 0 us,
  input-to-frame p95 320 us, steady-frame p95 240 us, recovery-cycle p95 0 us,
  and modeled working set 1,950,000 bytes; all budgets passed.
- `make check` -> architecture, unsafe baseline, ABI, format, clippy, unit, and
  compatibility checks passed after final cleanup.
- `make test-controller-desktop-visible` -> native 800 by 600 full-shell QEMU
  frames passed with mixed-case softened text, live domain inventory, styled
  cursor, navigation clicks, and a retained power-menu screenshot.
- `make test-controller-desktop-all` -> complete desktop contract, recovery,
  2-CPU/4-CPU stress, boot, and visible framebuffer qualification passed after
  the native-resolution revision.
- `make test-controller-desktop-performance` -> presentation startup 0 us,
  input-to-frame p95 321 us, steady-frame p95 244 us, recovery-cycle p95 0 us,
  and modeled working set 1,950,000 bytes; all budgets passed.
- `cargo test -p ginux-windowing -p ginux-controller-ui` -> 14 tests passed,
  including deterministic proof that the embedded Inter atlas produces partial
  antialias coverage.
- `make test-controller-desktop-latency` -> exact 1920 by 1080 real-QEMU frames,
  navigation clicks, live CLI-to-GUI state, power menu, and input all passed;
  final visible interaction p95 was 43.624 ms, maximum was 57.258 ms, and
  complete Settings-page presentation was 57.258 ms. Machine-readable evidence
  is under `build/qualification/controller-desktop/`.
- `make test-controller-desktop-performance` -> the 1920 by 1080 release model
  passed: 1 us startup, 1,317 us input-to-frame p95, 959 us steady-frame p95,
  0 us recovery p95, and 8,324,400 modeled bytes.
- `make test-controller-desktop-all` -> complete protocol, windowing,
  controller, domain display, Wayland, presentation, recovery, verification
  boot, 2-vCPU/4-vCPU stress, exact 1080p framebuffer, and latency suite passed
  on 2026-11-01.
- `make test-controller-desktop-latency` -> exact 1024 by 768 real-QEMU frames,
  navigation, live-state, and power-menu checks passed after the requested size
  correction; visible interaction p95 was 20.899 ms, maximum was 21.182 ms,
  and complete Settings-page presentation was 18.125 ms.
- `make test-controller-desktop-performance` -> the 1024 by 768 release model
  passed at 1,040 us input-to-frame p95, 736 us steady-frame p95, and a bounded
  3,175,728-byte working set.
- `make test-controller-desktop-all` and `make check` -> complete desktop,
  2-vCPU/4-vCPU stress, architecture, ABI, formatting, lint, and unit
  regressions passed after the 1024 by 768 revision.
- `make test-controller-desktop-latency` -> the compact 1024 by 768 typography
  pass retained exact native frames, navigation, live state, and power-menu
  behavior; visible interaction p95 was 19.930 ms, maximum was 19.948 ms, and
  complete Settings-page presentation was 19.384 ms.
- `make test-controller-desktop-performance` -> the compact release model
  passed at 826 us input-to-frame p95, 835 us steady-frame p95, and a bounded
  3,175,728-byte working set.
- `make test-controller-desktop-all` -> the complete desktop suite and 2-vCPU/
  4-vCPU recovery stress passed after the compact typography revision.
- `make check` -> architecture, unsafe baseline, ABI, formatting, lint, unit,
  and compatibility checks passed after the compact typography revision.

## Decisions and limitations

- Initial direction is a restrained modern systems dashboard using dark
  navy/slate surfaces, a limited blue/teal status palette, compact information
  density, and a clean software-renderable pointer.
- The concept is a visual specification, not a bitmap shipped by the kernel.
- Human framebuffer review added Settings after the accepted concept; date,
  time, and timezone remain visible future settings unless the current kernel
  management ABI already supports them.
- Human framebuffer review rejected the technically passing 160 by 100 result
  as visibly too coarse; technical passage does not satisfy visual acceptance.
- Human framebuffer review on 2026-11-01 rejected the 400 by 300 logical result
  as still unreasonably pixelated and identified unusable pointer latency,
  absent clicking, and static demonstration data. Human direction explicitly
  incorporated all four findings into this existing gate rather than creating
  additional gates.
- Native service layout recognition now uses exact guard-page and stack
  geometry rather than assuming the final reserved image page is populated.
- The controller now owns persistent shell and live system-view state. It
  refreshes kernel-backed memory, time, domain, CPU, and lifecycle data without
  constructing a demonstration model for each frame.
- Pointer motion is coalesced in the bounded kernel input queue. The controller
  drains a bounded hardware batch per call, routes button transitions once,
  and repaints only the old and new pointer damage regions for motion-only
  updates.
- Controller stdin is intentionally nonblocking and returns `EAGAIN` when the
  serial recovery path has no byte. This keeps serial recovery independent
  without allowing it to stall native input and rendering.
- A larger dynamically mapped render surface was considered, but the current
  bounded native-controller mapping contract does not expose a suitable
  allocator. The earlier 400 by 300 compromise is now rejected by human review;
  this revision must provide native 800 by 600 composition with bounded,
  explicitly accounted controller memory.
- Human framebuffer review on 2026-11-01 removed the internal window metaphor:
  the trusted controller is the shell, so fake minimize/maximize controls and
  the graphical command prompt have no useful role. The serial CLI remains the
  independent recovery interface.
- Human framebuffer review later on 2026-11-01 rejected the nominal 800 by 600
  output as visually equivalent to a much lower resolution and found refresh
  latency painful. The prior renderer benchmark measured composition in
  isolation, not complete GOP presentation, so those results cannot close this
  criterion. This is a failed checkpoint and reopens implementation.
- The revised controller maps only the GOP pages authorized by its display
  capability, renders full pages directly into that persistent surface, and
  uses an 18 by 20 saved-under region for pointer-only damage. The PS/2 device
  is configured for its standard 200 Hz report rate.
- The antialiased ASCII atlas is generated from Inter under the SIL Open Font
  License 1.1 and packed at four bits of alpha per sample to keep the embedded
  controller image within its measured size bound.
- Human clarification corrected the final target from 1920 by 1080 to the
  standard 1024 by 768 GOP mode. The UI now renders one logical pixel per
  physical pixel, retains direct framebuffer mapping and bounded pointer
  damage, and fits within the laptop display without host-side downscaling.
- Human review of the first 1024 by 768 frame found that its 20-pixel type and
  card spacing retained 1080p proportions. The embedded face is now 16 pixels,
  summary cards have deliberate vertical breathing room, and the domain table
  starts below the complete card content rather than crowding it.
- Real-QEMU latency timestamps now stop when QEMU is asked to sample the
  framebuffer. Subsequent PPM encoding and filesystem output are observer
  overhead and are deliberately excluded from presentation latency.

## Delivery strategy

- Working branch: `codex/controller-desktop`
- Intended base: `main`
- Pull request: continue the integrated controller-desktop pull request
- Planned commits: accepted visual reference; native implementation and tests;
  responsive input and interaction; live management state; native-resolution
  visual refinement; native damage-aware presentation and typography; evidence
  and gate closure
- Actual commits: `eb71161a` (accepted visual reference); `af7cb035`
  (initial native implementation); `71c097fd` (Settings navigation);
  `d6682bc4` (high-resolution renderer); `d9f77bf3` (responsive input,
  clickable navigation, live state, typography, and interaction evidence);
  `9d8ac33b` (native 1080p GOP mapping, damage-aware presentation,
  antialiased Inter typography, and real-QEMU latency qualification);
  `e8f7636c` (native 1024 by 768 fit, 1:1 layout, and corrected presentation
  latency measurement); `61f909f9` (compact 16-pixel typography, rebalanced
  summary cards, refreshed qualification evidence, and updated documentation)

## Review policy and outcome

Human checkpoints are required for the concept image and the final implemented
QEMU interaction and framebuffer because visual quality and usability are
subjective. Objective implementation and regression loops may proceed
autonomously between those checkpoints. Human direction on 2026-11-01 approved
continuing all remaining work inside Gate 31 without intermediate checkoff.
All objective criteria now pass; the gate remains open only for final human
visual and interaction acceptance.
