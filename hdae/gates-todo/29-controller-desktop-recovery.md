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

- [ ] None.

## Remaining work

- [ ] Implement recovery paths, stress/fault tests, and evidence.

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

- Not yet run.

## Decisions and limitations

- The serial CLI is the final recovery surface; window placement persistence is
  deferred.

## Delivery strategy

- Working branch: `codex/controller-desktop`
- Intended base: `main`
- Pull request: shared integrated PR for Gates 19–30
- Planned commits: recovery mechanisms; failure injection; stress/accounting
- Actual commits: pending

## Review policy and outcome

Delegated closure under the approved Gates 19–30 batch after all criteria pass.
