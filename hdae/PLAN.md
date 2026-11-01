# Native Controller Desktop Plan

## Delivery strategy

- Working branch: `codex/controller-desktop`
- Intended base: `main`
- Pull request: one integrated pull request for Gates 19–30
- Origin: design discussion prompted by GitHub issue #12; no contributor code is
  merged by this plan
- Commit policy: signed, coherent commits at gate boundaries where practical
- Push policy: no push without current user authorization

The single-PR exception is deliberate: the native display protocol, compositor,
window system, controller application, domain bridge, and Linux presentation
path form one vertical feature and cannot be reviewed honestly as independent
product deliveries.

## Gate sequence

| Gate | Purpose | Depends on | Review |
|---:|---|---|---|
| 19 | Fix controller desktop architecture and trust model | Existing kernel/controller baseline | Accepted |
| 20 | Establish a graphical ring-3 controller runtime | 19 | Delegated |
| 21 | Add framebuffer and input device support | 20 | Delegated |
| 22 | Implement the native display protocol | 20, 21 | Delegated |
| 23 | Implement seats, focus, and resource leases | 22 | Delegated |
| 24 | Build the native windowing system and toolkit | 23 | Delegated |
| 25 | Build the graphical controller application | 24 | Delegated |
| 26 | Bridge virtual display and input into domains | 23 | Delegated |
| 27 | Run a minimal Wayland environment in a Linux domain | 26 | Delegated |
| 28 | Deliver windowed and full-screen domain presentation | 24, 25, 27 | Delegated |
| 29 | Harden lifecycle and failure recovery | 28 | Delegated |
| 30 | Qualify performance, isolation, UX, and documentation | 29 | Delegated batch end |

## Execution policy

The human accepted Gate 19 and then delegated continuous execution and closure
of Gates 20–30 on 2026-10-31. A gate closes only after every criterion has fresh
evidence. Stop for a clarification that materially affects behavior, a failed
dependency with no safe in-scope path, changed architecture, scope expansion,
or a new consequential security decision.

## Coverage

- Function and failure paths: Gates 20–29.
- Deterministic tests and regressions: every gate, consolidated by Gate 30.
- Operator and developer workflows: Gates 24, 25, 28, and 30.
- Documentation and examples: Gates 19 and 30.
- Compatibility: Gate 27; broader desktop environments are explicitly deferred.
- Persistence and migration: no persistent desktop state in this iteration.
- Recovery and maintenance: Gate 29.
- Performance and security boundaries: Gates 19, 23, 29, and 30.
- Privacy: not applicable beyond preventing cross-domain display/input leakage.
