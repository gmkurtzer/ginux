# Controller desktop architecture

## Source

Design discussion prompted by GitHub issue #12; no contributor code is adopted.

## Goal

Freeze a reviewable native controller-desktop architecture, trust model,
protocol boundary, state model, and initial non-goals before implementation.

## Approved scope

- Record the controller compositor, native client, domain bridge, and Linux
  Wayland boundaries.
- Specify physical-seat ownership, focus, leases, secure attention, surface and
  buffer lifecycles, and failure recovery.
- Define version-zero protocol and testable state-machine requirements.

## Non-goals

- Implementation, driver changes, or adoption of issue #12 code.
- GPU acceleration, raw GPU passthrough, or broad desktop compatibility.

## Completed work

- [x] Human approved the architecture direction, gate batch, branch/PR strategy,
  and checkpoints in conversation.
- [x] Project intent, architecture invariants, and gate sequence are persisted.
- [x] ADR 0003 fixes the controller, kernel, native-client, and Linux-domain
  trust and process boundaries.
- [x] `docs/CONTROLLER-DESKTOP.md` specifies GND0, object/buffer/surface
  lifecycles, window policy, seats, focus, leases, the domain bridge, recovery,
  and the Gate 20–30 verification map.
- [x] Architecture consistency and forward/reverse coverage checks passed.

## Remaining work

- None.

## Acceptance criteria

- The decision identifies every trusted component and authority boundary.
- The protocol defines versioning, objects, buffer ownership, events, bounds,
  errors, and disconnect behavior without relying on ambient global state.
- Seat, focus, lease, full-screen, secure-attention, and crash paths have
  explicit state transitions and recovery ownership.
- The document states why Linux Wayland remains inside a domain and why a domain
  exports one composed surface.
- Every later gate maps to an architectural component and invariant.

## Verification plan

- `make architecture-check` -> architecture and source policy checks pass.
- `rg -n "TODO|TBD|FIXME" docs/decisions/0003-controller-desktop.md docs/CONTROLLER-DESKTOP.md` -> no unresolved architecture placeholders.
- Manual forward/reverse coverage review -> every Gate 20–30 outcome maps to the
  specification and no specification deliverable is orphaned.

## Evidence

- `git diff --check` -> passed.
- unresolved-marker scan across ADR 0003 and the desktop specification -> no
  `TODO`, `TBD`, or `FIXME` markers.
- Gate 20–30 mapping/ledger loop -> every later gate has one specification
  mapping and one authoritative todo record.
- `make architecture-check` -> passed, including privilege entry, user access,
  writable-global classification, runtime ownership, synchronization, and
  kernel unsafe baselines.
- Manual forward/reverse coverage review -> every approved functional, failure,
  regression, workflow, documentation, compatibility, operational,
  performance, and security boundary is mapped or explicitly deferred.

## Decisions and limitations

- Custom Ginux display protocol in ring 3; no host Wayland compositor.
- Linux domains use their own Wayland compositor and export one desktop surface.
- Physical devices remain controller-owned; exclusive use is a revocable lease.

## Delivery strategy

- Working branch: `codex/controller-desktop`
- Intended base: `main`
- Pull request: intended, shared integrated PR for Gates 19–30
- Planned commits: HDAE project/gate plan; architecture decision/specification
- Actual commits: `20eaffb9` (project and gate plan); `395d0843`
  (architecture decision, specification, and technical evidence)

## Review policy and outcome

Passed and accepted by the human on 2026-10-31. Gate 19 is closed.
