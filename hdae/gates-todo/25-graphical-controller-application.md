# Native graphical controller application

## Goal

Deliver a native graphical system-configuration application that exposes the
controller's core kernel, device, network, storage, and domain workflows.

## Approved scope

- Use the same versioned management operations as the CLI for system status,
  domains, CPU/memory, devices, networks, storage, lifecycle, attach, and exec.
- Add resource/lease views, boot/failure notifications, shutdown, and an
  embedded controller terminal.
- Preserve CLI behavior and authority parity.

## Non-goals

- Replacing every advanced CLI command or introducing GUI-only management
  authority.

## Completed work

- [ ] None.

## Remaining work

- [ ] Implement workflows, error states, UI automation, and evidence.

## Acceptance criteria

- The GUI lists, creates, configures, starts, stops, restarts, attaches to, and
  executes commands in domains through the management ABI.
- System, device, network, storage, focus, and lease state are observable.
- Every mutation reports pending/success/failure without blocking input.
- Killing the GUI leaves CLI management operational and consistent.
- Equivalent scripted CLI and GUI operations produce the same resulting state.

## Verification plan

- `make test-graphical-controller` -> scripted operator workflow and failure
  assertions pass.
- `make test-controller-console` -> CLI remains fully functional.
- `make check` -> regression suite passes.

## Evidence

- Not yet run.

## Decisions and limitations

- The GUI and CLI are clients of one management ABI; neither owns canonical
  configuration state.

## Delivery strategy

- Working branch: `codex/controller-desktop`
- Intended base: `main`
- Pull request: shared integrated PR for Gates 19–30
- Planned commits: application shell/status; management workflows; automation
- Actual commits: pending

## Review policy and outcome

Delegated closure under the approved Gates 19–30 batch after all criteria pass.
