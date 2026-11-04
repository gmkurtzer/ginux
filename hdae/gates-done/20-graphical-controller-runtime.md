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

- [x] Added a measured static-PIE graphical controller service to the embedded
  root and native ABI.
- [x] Launched the service in its own ring-3 address space with only a
  write-only console capability.
- [x] Verified bounded console IPC, deliberate fault containment, stable-
  identity restart, clean exit, capability revocation, frame cleanup, and
  serial-controller recovery.
- [x] Expanded the immutable measured-boot publication envelope to carry the
  additional service without changing demand-allocated runtime storage.
- [x] Added the deterministic QEMU lifecycle assertion and full build/static
  regression coverage.

## Remaining work

- [x] None.

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

- `make test-controller-graphics-runtime` -> passed the multi-boot QEMU suite;
  both service generations emitted the bounded ready message, the faulted
  generation was contained, the replacement preserved service identity, all
  grants and frames retired, and the CLI reached `ginux>` and shut down.
- `make check` -> passed architecture checks, Linux compatibility validation,
  ABI conformance, formatting, clippy, and unit tests, including the new
  graphics bootstrap ABI layout/negative tests.
- `make architecture-check` -> passed with the existing kernel unsafe baseline
  unchanged at 1033 blocks.
- `git diff --check` -> passed.

## Decisions and limitations

- The graphical compositor is a trusted ring-3 controller service, not kernel
  window policy and not code inside the existing CLI process.
- The immutable measured boot archive is copied before a general kernel heap is
  available, so its explicit contiguous publication envelope is 192 pages.
  Runtime domain memory and image storage remain demand allocated.

## Delivery strategy

- Working branch: `codex/controller-desktop`
- Intended base: `main`
- Pull request: shared integrated PR for Gates 19–30
- Planned commits: service image/ABI; supervision/tests
- Actual commits: `b57bce11` combines the tightly coupled image, ABI,
  supervision, and deterministic lifecycle test in one coherent commit.

## Review policy and outcome

Passed and closed under the human-confirmed delegated Gates 20–30 batch on
2026-10-31.
