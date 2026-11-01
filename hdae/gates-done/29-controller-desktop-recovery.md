# Controller desktop lifecycle and recovery

## Goal

Prove that graphical clients, services, bridges, and domains can fail or restart
without losing controller access, leaking resources, or crossing isolation.

## Approved scope

- Add failure injection for compositor, GUI, native client, domain bridge,
  Wayland compositor, Linux client, and domain termination.
- Implement generation rollover, buffer/lease reclamation, focus restoration,
  service restart, diagnostics, and bounded retry/backoff.
- Stress concurrent lifecycle transitions under SMP.

## Non-goals

- High availability across kernel failure or persistence of window layout.

## Completed work

- [x] Added a recovery supervisor covering compositor, graphical controller,
  native client, domain bridge, Wayland compositor, Linux client, and domain
  failure.
- [x] Added generation rollover, stale-object rejection, trusted-focus restore,
  and explicit surface, buffer, event-queue, handle, and lease reclamation.
- [x] Added component/result diagnostics with monotonic sequence and bounded
  retry delay plus a three-consecutive-failure restart budget.
- [x] Exercised actual display-protocol disconnect, Wayland client reconnect,
  domain presentation failure/restart, and two-domain pixel/input isolation.
- [x] Added 512-cycle host stress and repeated 2-CPU/4-CPU QEMU stress runs.

## Remaining work

- [x] None.

## Acceptance criteria

- Every injected component failure returns display/input ownership to trusted
  controller state and preserves CLI access.
- Restarted components cannot use stale objects, buffers, leases, or events.
- Repeated start/stop/crash cycles show no unbounded process, frame, handle,
  queue, or capability growth.
- Concurrent multi-domain failure does not leak pixels/input or deadlock SMP.
- User-visible diagnostics identify the failed component and recovery result.

## Verification plan

- `make test-controller-desktop-recovery` -> complete fault matrix and resource
  accounting pass.
- `make test-controller-desktop-stress` -> repeated SMP lifecycle stress passes.
- `make check` -> regression suite passes.

## Evidence

- `make test-controller-desktop-recovery` -> passed the complete seven-component
  fault matrix, stale-generation checks, actual component recovery probes,
  resource accounting, diagnostics, and the ring-3 QEMU recovery marker.
- `make test-controller-desktop-stress` -> passed 512 host lifecycle cycles and
  two QEMU passes each at 2 CPUs and 4 CPUs without deadlock or leakage.
- `make check` -> passed architecture, formatting, compatibility, unsafe-policy,
  native ABI, workspace build, clippy, and unit-test regressions.
- `git diff --check` -> passed with no whitespace errors.

## Decisions and limitations

- The serial CLI is the final recovery surface; window placement persistence is
  deferred.
- Retry accounting is consecutive: successful recovery resets the budget, while
  three consecutive failures require operator action through the serial CLI.
- Diagnostic history is bounded to 32 entries and overwrites the oldest entry.

## Delivery strategy

- Working branch: `codex/controller-desktop`
- Intended base: `main`
- Pull request: shared integrated PR for Gates 19–30
- Planned commits: recovery mechanisms; failure injection; stress/accounting
- Actual commits: `f71d89ad` (recovery mechanisms, fault matrix, diagnostics,
  resource accounting, and SMP stress); closure record follows in the
  gate-boundary commit

## Review policy and outcome

Passed and closed under the delegated Gates 19–30 batch on 2026-10-31. Every
criterion has fresh automated evidence; no individual human checkpoint was
required.
