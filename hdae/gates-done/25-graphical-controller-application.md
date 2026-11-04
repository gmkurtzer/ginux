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

- [x] Added a graphical controller application model whose operations map to
  the existing versioned native management ABI for host, domain, network, and
  storage state.
- [x] Added domain list/create/configure/start/stop/restart/attach/exec and
  system/device/network/storage/focus/lease observation workflows.
- [x] Added bounded asynchronous jobs and visible pending/success/failure
  states without blocking input.
- [x] Added a deterministic native controller shell with menus, domain table,
  actions, notifications, and embedded terminal field.
- [x] Proved CLI/GUI state equivalence and CLI continuity after GUI death.

## Remaining work

- [x] No remaining work.

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

- `make test-graphical-controller` -> 4 workflow/parity/render tests plus the
  ring-3 graphical controller contract and full QEMU boot passed.
- `make test-controller-console` -> complete interactive serial CLI workflow,
  lifecycle, network, editing, help, persistence, and shutdown test passed.
- `make check` -> complete regression suite passed; unsafe baseline remained
  1033 blocks.
- `git diff --check` -> passed.

## Decisions and limitations

- The GUI and CLI are clients of one management ABI; neither owns canonical
  configuration state.
- The application layer is capability-neutral: it emits the same native ABI
  operation/command pairs as the CLI and holds no GUI-only authority.
- A stale console regression was corrected by separating repeatable published
  init validation from the deliberately one-shot boot mapping verifier.

## Delivery strategy

- Working branch: `codex/controller-desktop`
- Intended base: `main`
- Pull request: shared integrated PR for Gates 19–30
- Planned commits: application shell/status; management workflows; automation
- Actual commits: `2268a8e5`

## Review policy and outcome

Passed and closed on 2026-10-31 under the delegated Gates 20–30 batch policy.
