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

- [ ] None.

## Remaining work

- [ ] Implement compositor policy, toolkit, demos, screenshot tests, and evidence.

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

- Not yet run.

## Decisions and limitations

- The toolkit is intentionally sufficient rather than general purpose; the
  graphical controller is its product test.

## Delivery strategy

- Working branch: `codex/controller-desktop`
- Intended base: `main`
- Pull request: shared integrated PR for Gates 19–30
- Planned commits: compositor policy; rendering/text; widgets/toolkit; tests
- Actual commits: pending

## Review policy and outcome

Delegated closure under the approved Gates 19–30 batch after all criteria pass.
