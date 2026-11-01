# Windowed and full-screen domain experience

## Goal

Make a running Linux desktop behave as one native managed window that can
transition safely and smoothly to and from exclusive full-screen presentation.

## Approved scope

- Add create, resize, focus, minimize, close, full-screen enter/exit, scaling,
  cursor, input capture, trusted status, and secure-attention UX.
- Integrate graphical controller lifecycle actions and domain status.
- Add deterministic end-to-end visual/input workflows.

## Non-goals

- Importing individual Linux windows, seamless clipboard, multi-monitor, or
  direct physical-device ownership.

## Completed work

- [ ] None.

## Remaining work

- [ ] Implement presentation UX, transitions, automation, and evidence.

## Acceptance criteria

- The same live domain surface moves between managed-window and full-screen
  modes without restart or stale input.
- Resize/configure handshakes preserve content and bounded latency.
- Trusted chrome identifies the domain and active input/resource ownership.
- Secure attention exits full screen and returns focus to the controller.
- Stop, restart, and crash produce clear UI state and no frozen exclusive view.

## Verification plan

- `make test-domain-desktop-experience` -> scripted launch, focus, resize,
  window/full-screen, secure-attention, lifecycle, and screenshot checks pass.
- `make check` -> regression suite passes.

## Evidence

- Not yet run.

## Decisions and limitations

- Full screen is exclusive presentation through a revocable logical lease, not
  physical device passthrough.

## Delivery strategy

- Working branch: `codex/controller-desktop`
- Intended base: `main`
- Pull request: shared integrated PR for Gates 19–30
- Planned commits: presentation state; trusted UX; end-to-end tests
- Actual commits: pending

## Review policy and outcome

Delegated closure under the approved Gates 19–30 batch after all criteria pass.
