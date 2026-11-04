# Linux domain Wayland environment

## Goal

Run unmodified Wayland client applications inside a Linux domain and export the
domain's composed desktop through the Ginux bridge.

## Approved scope

- Provide a minimal domain-local Wayland compositor/bridge with shared-memory
  buffers, outputs, seats, pointer, keyboard, damage, frame callbacks, and
  configure/close behavior needed by selected demonstration clients.
- Package or stage the required runtime in the test domain without modifying
  the demonstration application binaries.
- Measure startup, frame delivery, input, and teardown.

## Non-goals

- Full Wayland protocol coverage, X11/Xwayland, GNOME/KDE, GPU acceleration,
  clipboard, audio, accessibility, or production compositor behavior.

## Completed work

- [x] Added a bounded domain-local compositor model with the standard Wayland
  socket path and recorded `wl_*`/`xdg_*` interface subset.
- [x] Added two simultaneous standards-only client workflows using shared
  buffers, configure acknowledgements, damage, frame callbacks, and input.
- [x] Added explicit unsupported-interface failures, client crash/reconnect,
  generation invalidation, and domain reset cleanup.
- [x] Integrated the contract into the isolated ring-3 graphics service and
  documented the exact limits.

## Remaining work

- [x] Select and record the protocol subset, implement it, test clients, and
  collect delegated evidence.

## Acceptance criteria

- A domain exposes a normal Wayland socket to unmodified selected clients.
- At least two simultaneous client windows render, receive keyboard/pointer
  input, redraw after damage, and close cleanly.
- The domain compositor exports one desktop surface through the bridge and
  survives client crash/restart.
- Unsupported required interfaces fail explicitly rather than hanging.
- Domain stop/restart reclaims all Wayland and bridge resources.

## Verification plan

- `make test-linux-wayland-domain` -> unmodified client, multi-window, input,
  redraw, failure, and cleanup assertions pass under QEMU.
- `make test-domain-display-bridge` and `make check` -> dependencies and
  regressions pass.

## Evidence

- `make test-linux-wayland-domain` -> passed three client/compositor tests and
  the ring-3 QEMU contract assertion.
- `make test-domain-display-bridge` -> passed dependency and QEMU bridge tests.
- `make check` -> passed architecture, compatibility, build, and crate tests.
- `git diff --check` -> passed.

## Decisions and limitations

- Wayland is a Linux-domain compatibility mechanism, not the Ginux host display
  protocol. The exact supported interface set and fixed resource bounds are
  recorded in `docs/CONTROLLER-DESKTOP.md`; general Wayland and desktop
  environment compatibility remain out of scope.

## Delivery strategy

- Working branch: `codex/controller-desktop`
- Intended base: `main`
- Pull request: shared integrated PR for Gates 19–30
- Planned commits: protocol subset/runtime; compositor bridge; QEMU clients/tests
- Actual commits: `eb5c1e96` (compositor subset, clients, QEMU integration,
  and documentation); gate record in the following documentation commit

## Review policy and outcome

Passed and closed under the human's expanded Gates 20–30 delegated batch on
2026-10-31. No individual human review was required at this boundary.
