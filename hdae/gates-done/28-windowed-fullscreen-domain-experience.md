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

- [x] Added one generation-bound presentation object for a Linux domain surface.
- [x] Added windowed, minimized, full-screen, stopped, failed, and restarted states.
- [x] Added bounded configure/acknowledge resize with last-frame preservation.
- [x] Added controller-owned trusted chrome, focus, logical exclusive ownership,
  secure-attention revocation, and input suppression after focus loss.
- [x] Added exact deterministic fixtures for windowed, full-screen, and failed
  presentation plus a ring-3 graphics-service contract probe.
- [x] Increased the graphics-service stack from four to eight guarded pages after
  QEMU exposed bounded stack exhaustion in the expanded contract probe.

## Remaining work

- [x] None.

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

- `make test-domain-desktop-experience` -> passed six host-side presentation
  tests, deterministic screenshot hashes, the ring-3 presentation marker, and
  the complete QEMU boot contract.
- `make check` -> passed architecture, formatting, compatibility, unsafe-policy,
  native ABI, workspace build, and unit-test regressions.
- `git diff --check` -> passed with no whitespace errors.

## Decisions and limitations

- Full screen is exclusive presentation through a revocable logical lease, not
  physical device passthrough.
- Resize changes the generation-bound bridge endpoint but preserves the stable
  presentation identity and last completed content until a new frame arrives.
- The implementation remains a bounded software-rendered QEMU model; broad
  desktop compatibility remains outside this gate.

## Delivery strategy

- Working branch: `codex/controller-desktop`
- Intended base: `main`
- Pull request: shared integrated PR for Gates 19–30
- Planned commits: presentation state; trusted UX; end-to-end tests
- Actual commits: `9a646a26` (presentation state, trusted UX, tests, and ring-3
  integration); closure record follows in the gate-boundary commit

## Review policy and outcome

Passed and closed under the delegated Gates 19–30 batch on 2026-10-31. Every
criterion has fresh automated evidence; no individual human checkpoint was
required.
