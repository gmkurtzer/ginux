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

- [x] Selected the security-focused baseline: a bounded x86-64 instruction
  interpreter with Ginus-owned registers, memory, permissions, ELF loading,
  stack construction, and syscall events.
- [x] Implemented bounded ELF64 validation/loading, fixed virtual memory,
  page-level read/write/execute enforcement, and Linux initial stack layout.
- [x] Implemented the fixture-profile x86-64 interpreter and Linux syscall ABI
  event path with emulated identity/exit and validated output mediation.
- [x] Added structured ordered audit records and fail-closed unknown-syscall,
  invalid-pointer, oversized-write, unsupported-instruction, malformed-ELF,
  invalid-mapping, overlap, and write/execute denial behavior.
- [x] Added Rocky integration tests, aggregate regressions, documentation, and
  an explicit human demonstration.

## Remaining work

- [ ] Human runs or reviews `make demo-gate-3` and accepts or rejects the gate.

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

- `cargo test -p ginus-runtime` -> six tests passed for ELF loading, Linux stack
  layout, page permissions, copy boundaries, invalid/oversized pointers,
  unknown syscalls, malformed/W+X/out-of-range/oversized segments, and
  unsupported instructions.
- `make check` -> formatting and all target-correct Clippy checks passed with
  warnings denied, including the interpreter and static fixture.
- `make test-runtime` -> Rocky 10.2 ran Ginus as an ordinary host process;
  Ginus loaded and interpreted a 4,424-byte static ELF64 x86-64 fixture from
  bytes, produced the expected argument/environment output and five ordered
  audit records, denied syscall 999 with `-ENOSYS`, and rejected a truncated
  ELF before execution.
- `make demo-gate-3` -> printed the execution boundary, ELF/memory/stack
  evidence, managed output, all five syscall decisions, direct-host-syscall
  impossibility for interpreted code, malformed-input denial, explicit
  non-coverage, clean Rocky shutdown, and `GATE3: PASS`.
- `make test` -> Gate 1 interception, six runtime unit tests, Gate 3 Rocky
  execution, and Gate 2 offline repeatability/corruption regressions all passed.

## Decisions and limitations

- Static x86-64 fixtures define this gate's compatibility boundary; a passing
  fixture is not a general Linux compatibility claim.
- Contract configuration remains internal and test-oriented until Gate 10, but
  the central dispatch boundary is architectural and may not be bypassed.
- The interpreter is intentionally a bounded static-program compatibility
  profile, not a claim of complete x86-64 instruction coverage.
- Gate 4 will make execution profile selection explicit. Gate 3 implements only
  the security-focused interpreter; a performance-focused backend must be
  added and reviewed separately before it can be selected.
- Gate 3 is temporarily stacked on accepted Gate 2 commit `797b716` because
  pull requests 1 and 2 remain open. Its intended eventual base remains `main`.

## Delivery strategy

- Working branch: `hdae/execution-kernel`
- Intended base: `main`
- Pull request: https://github.com/gmkurtzer/usshim/pull/3; dedicated branch
  push and PR creation authorized on 2026-11-03; temporarily stacked on pull
  request 2; never merge or close without explicit authorization
- Planned commits: address-space and ELF loader; syscall ABI and minimal
  handlers; denial, audit, and malformed-input tests
- Actual commits: `e137733` (interpreted baseline), `370b661` (Ginus-owned
  memory, ELF, x86, syscall and audit core), `fb31678` (Ginus integration and
  demo), `52ef264` (invalid mapping regressions), and `f90aea4` (verification
  evidence)

## Review policy and outcome

Technically passed on 2026-11-03. Human checkpoint remains open because this
gate establishes the memory and syscall security boundary used by all later
compatibility work. Present `make demo-gate-3`, fresh evidence, and the pull
request before acceptance.
