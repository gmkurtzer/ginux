# Execution kernel

## Source

Initial Ginus architecture and successful Gate 1 interception boundary.

## Goal

Run small unmodified static Linux x86-64 programs inside a Ginus-managed
address space with Linux-compatible syscall entry, central contract dispatch,
and no direct host syscall access.

## Approved scope

- Implement bounded ELF64 loading for the static test profile.
- Implement domain address-space creation, mappings, protected copy-in and
  copy-out, stack setup, and task register state.
- Implement the Linux x86-64 syscall calling convention and error returns.
- Add a minimal explicit syscall set needed by the approved fixtures, including
  process exit and basic output through a Ginus-owned object.
- Introduce structured dispatch decisions and audit records sufficient to prove
  emulate and deny paths.
- Treat unknown syscalls as an explicit fail-closed contract result.

## Non-goals

- Dynamic linking, fork or threads, general filesystem behavior, systemd, or
  broad Linux syscall coverage.
- Direct forwarding of a managed syscall to the equivalent host syscall.
- User-configurable production contracts; Gate 10 completes that interface.

## Completed work

- [ ] None; gate not started.

## Remaining work

- [ ] Implement memory and ELF foundations.
- [ ] Implement Linux syscall entry and minimal handlers.
- [ ] Add audit and denial behavior.
- [ ] Add malformed ELF, pointer, mapping, and syscall tests.

## Acceptance criteria

- Approved static fixtures load from bytes controlled by the test and execute
  with the expected argv, environment, stack, and memory protections.
- Managed reads and writes cannot address Ginus or unrelated host memory.
- Every managed syscall reaches the central dispatcher and produces the
  expected emulated or denied result and audit record.
- Unknown syscall numbers cannot reach the host and return the configured
  Linux-compatible failure.
- Malformed ELF metadata, invalid pointers, oversized lengths, and invalid
  mappings fail deterministically without crashing Ginus.
- Earlier interception and offline-test criteria remain passing.
- `make demo-gate-3` lets a human run the managed static fixture, inspect its
  Linux-style result and audit decision, and observe a malformed-input denial.

## Verification plan

- `make test-runtime` -> static ELF success, argument layout, memory protection,
  copy boundary, known syscall, unknown syscall, and malformed-input tests pass.
- `make test-intercept` -> Gate 1 interception and bypass-negative tests remain
  passing.
- `make check` -> formatting, lint, and static checks exit zero.
- `make test` with network disabled -> all mandatory tests exit zero.
- `make demo-gate-3` -> performs the documented managed execution and negative
  case, cleans up, and exits zero.

## Evidence

- Not run.

## Decisions and limitations

- Static x86-64 fixtures define this gate's compatibility boundary; a passing
  fixture is not a general Linux compatibility claim.
- Contract configuration remains internal and test-oriented until Gate 10, but
  the central dispatch boundary is architectural and may not be bypassed.

## Delivery strategy

- Working branch: `hdae/execution-kernel`
- Intended base: `main`
- Pull request: intended; dedicated branch push and PR creation authorized on
  2026-11-03; never merge or close without explicit authorization
- Planned commits: address-space and ELF loader; syscall ABI and minimal
  handlers; denial, audit, and malformed-input tests
- Actual commits: pending

## Review policy and outcome

Human checkpoint because this gate establishes the memory and syscall security
boundary used by all later compatibility work. Present `make demo-gate-3`,
fresh evidence, and the pull request before acceptance.
