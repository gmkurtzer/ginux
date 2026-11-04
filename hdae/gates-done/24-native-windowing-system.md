# Native windowing system and toolkit

## Goal

Provide enough native desktop, window management, text, and widget support to
build real Ginux controller applications.

## Approved scope

- Implement desktop, panels, windows, decorations, z-order, focus, move, resize,
  minimize, maximize, close, cursor, clipping, damage, and frame scheduling.
- Provide font/text rendering, layouts, menus, buttons, labels, text fields,
  tables, event dispatch, and a small native client toolkit.
- Distinguish trusted controller UI from ordinary client content.

## Non-goals

- Theme ecosystem, animation framework, accessibility API, arbitrary fonts, or
  compatibility with existing GUI toolkits.

## Completed work

- [x] Added a bounded, allocation-free native desktop/compositor model with
  z-order, focus, movement, resize, minimize, maximize, restore, and close.
- [x] Added software rendering with clipping, bounded damage, bounded frame
  callbacks, decorations, cursor-target input dispatch, and distinct trusted
  controller chrome.
- [x] Added labels, menus, buttons, text fields, and tables plus deterministic
  text rendering and text edit/navigation/selection/backspace/submit behavior.
- [x] Added a native demo contract, exact-pixel screenshot fixture, scripted
  input/lifecycle tests, and ring-3 execution.

## Remaining work

- [x] No remaining work.

## Acceptance criteria

- Multiple native windows can be created, moved, resized, reordered, minimized,
  maximized, focused, and closed without visual corruption or input leakage.
- Damage limits redraw to changed regions and frame callbacks are bounded.
- Text fields support insertion, navigation, selection, backspace, and submit.
- A native demo exercises every required widget and trusted UI marker.
- Deterministic screenshots and input scripts match approved fixtures.

## Verification plan

- `make test-native-windowing` -> scripted window/input/widget assertions pass.
- `make test-native-windowing-screenshots` -> rendered fixtures match expected
  pixels within an explicitly documented tolerance.
- `make check` -> regression suite passes.

## Evidence

- `make test-native-windowing` -> 4 host lifecycle/widget/render tests and the
  ring-3 QEMU windowing contract passed.
- `make test-native-windowing-screenshots` -> exact 96x64 pixel fixture hash
  `55c32738580bb202` matched with zero tolerance.
- `make check` -> complete regression suite passed; unsafe baseline remained
  1033 blocks.
- `git diff --check` -> passed.

## Decisions and limitations

- The toolkit is intentionally sufficient rather than general purpose; the
  graphical controller is its product test.
- The ring-3 contract uses a 32x24 framebuffer to respect the deliberately
  small graphical-service stack; the full screenshot fixture remains 96x64.
- Text uses a deterministic compact built-in glyph renderer; arbitrary fonts
  remain a declared non-goal.

## Delivery strategy

- Working branch: `codex/controller-desktop`
- Intended base: `main`
- Pull request: shared integrated PR for Gates 19–30
- Planned commits: compositor policy; rendering/text; widgets/toolkit; tests
- Actual commits: `85c9f646`

## Review policy and outcome

Passed and closed on 2026-10-31 under the delegated Gates 20–30 batch policy.
