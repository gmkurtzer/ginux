# Graphical controller runtime foundation

## Goal

Start, supervise, and stop a trusted ring-3 graphical service beside the
existing CLI while preserving serial recovery and kernel isolation.

## Approved scope

- Add the graphical service image, measured launch path, capabilities, address
  space, scheduler integration, and lifecycle reporting.
- Establish bounded IPC with controller management and future display services.
- Add deterministic launch, fault, restart, and CLI-responsiveness tests.

## Non-goals

- Rendering, input drivers, windows, or domain display.

## Completed work

- [ ] None.

## Remaining work

- [ ] Implement, test, document, and record evidence.

## Acceptance criteria

- The service executes in ring 3 with only declared grants and a distinct
  identity/address space.
- Its failure does not restart, block, or replace the CLI controller.
- Start/stop/restart leaves no process, grant, frame, or scheduler leak.
- Existing controller, SMP, ABI, storage, and domain lifecycle tests pass.

## Verification plan

- `make test-controller-graphics-runtime` -> launch, IPC, fault, restart, and
  cleanup assertions pass.
- `make check` -> complete regression suite passes.

## Evidence

- Not yet run.

## Decisions and limitations

- The graphical compositor is a trusted ring-3 controller service, not kernel
  window policy and not code inside the existing CLI process.

## Delivery strategy

- Working branch: `codex/controller-desktop`
- Intended base: `main`
- Pull request: shared integrated PR for Gates 19–30
- Planned commits: service image/ABI; supervision/tests
- Actual commits: pending

## Review policy and outcome

Delegated closure under the approved Gates 19–30 batch after all criteria pass.
