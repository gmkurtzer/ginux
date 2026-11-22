# Deny host-global filesystem synchronization

## Source

[Issue #299](https://github.com/gmkurtzer/ginux/issues/299).

## Goal

Make the runtime contract and filesystem-operation inventory deliberately deny
`sync(2)` and `syncfs(2)` because both can force host-global writeback.

## Scope

- Add `sync` to the contract vocabulary as an explicit `EPERM` denial.
- Change `syncfs` from descriptor mediation to an explicit `EPERM` denial.
- Classify both operations as host-global authority in the filesystem inventory.
- Prove wildcard syscall grants cannot promote either denial.

## Acceptance criteria

- A permissive `syscall.*` grant still resolves syscalls 162 and 306 to
  `Action::Deny(EPERM)`.
- The generated ABI coverage and filesystem inventory describe both denials.
- Static checks and unit tests pass.

## Non-goals

- No per-instance filesystem or scoped writeback mechanism is introduced.
- No other filesystem operation is enabled or reclassified.

## Status

Technically passed on `hdae/deny-host-global-sync`; pending pull-request CI and
human acceptance.

## Evidence

- `cargo test -p ginux-contract host_global_sync_is_deliberately_denied`
- `cargo test -p ginux-fs-policy`
- `make check`
- `make test-unit`
