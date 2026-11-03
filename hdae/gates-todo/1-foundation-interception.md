# Foundation and interception proof

## Source

Initial Ginus project establishment.

## Goal

Produce a buildable Rust foundation and executable proof that unmodified
x86-64 managed code can run under Ginus, enter Ginus on a syscall, and cannot
invoke the host syscall ABI directly.

## Approved scope

- Establish the Rust workspace, top-level Makefile, test layout, and QEMU TCG
  Linux x86-64 development harness for the macOS arm64 host.
- Record the initial threat model and execution-platform decision evidence.
- Evaluate emulation-compatible trap options and implement the smallest
  experiment needed to run a fixed unmodified x86-64 test ELF and intercept
  selected syscalls without KVM.
- Route intercepted operations through a central decision point that can allow
  emulation or denial; direct host continuation is forbidden.
- Establish matched direct-Linux and conventional-container benchmark controls
  inside the same emulated x86-64 environment for a small CPU loop, syscall
  loop, and process startup measurement.
- Add `doctor`, `build`, `check`, `test`, `test-unit`, `test-intercept`, and
  `bench-baseline` Make targets.
- Add `make demo-gate-1` as a runnable human demonstration of intercepted,
  emulated, and denied syscalls with explicit expected output and cleanup.

## Non-goals

- General ELF compatibility, systemd, a persistent domain, or a complete
  syscall table.
- Rocky image acquisition.
- Production scheduling, networking, storage, or device support.
- Selecting final numeric performance budgets.

## Completed work

- [ ] None; gate not started.

## Remaining work

- [ ] Create the Rust and Makefile scaffold.
- [ ] Implement the emulation-compatible interception experiment.
- [ ] Add positive and negative interception tests.
- [ ] Capture baseline benchmark results and environment metadata.
- [ ] Reconcile the experiment with every architecture invariant.

## Acceptance criteria

- The documented host prerequisites are machine-checkable and the project
  builds from a clean checkout on the reference environment.
- An unmodified x86-64 test ELF executes a defined instruction sequence and its
  selected syscall is handled by Ginus with the expected Linux-style result.
- A syscall denied by the test contract does not execute on the host and
  returns the expected denial result.
- Evidence demonstrates that managed execution cannot escape the interception
  path by issuing a direct host syscall in the tested execution mode.
- All interception decisions use one central dispatch boundary.
- Direct-Linux and conventional-container baseline measurements run inside the
  same emulated x86-64 environment and record workload, environment, units,
  repetitions, and variance.
- A human can run `make demo-gate-1` and observe a managed syscall answered by
  Ginus, a denied syscall blocked before host execution, and a successful
  cleanup result.
- `make test` succeeds without network access.

## Verification plan

- `make doctor` -> exits zero and reports every required host capability.
- `make build` -> produces the Ginus development binary and test artifacts.
- `make check` -> formatting, lint, and static checks exit zero.
- `make test-unit` -> all foundation unit tests pass.
- `make test-intercept` -> positive syscall, denied syscall, and direct-bypass
  negative tests pass with machine-checkable results.
- `make test` with network disabled -> exits zero.
- `make bench-baseline` -> emits reproducible native and conventional-container
  baseline records with environment metadata.
- `make demo-gate-1` -> prints the documented interception demonstration,
  validates each observation, cleans up, and exits zero.

## Evidence

- Not run.

## Decisions and limitations

- Rust and x86-64 are approved for the initial implementation.
- KVM is outside initial acceptance. Gate 1 selects an interception mechanism
  that works inside QEMU TCG on the macOS arm64 development host.
- Performance budgets remain open until baseline evidence exists.

## Delivery strategy

- Working branch: `hdae/foundation-interception`
- Intended base: `main`
- Pull request: intended; dedicated branch push and PR creation authorized on
  2026-11-03; never merge or close without explicit authorization
- Planned commits: project scaffold and checks; interception proof and tests;
  benchmark controls and architecture evidence
- Actual commits: pending

## Review policy and outcome

Human checkpoint. Present `make demo-gate-1`, fresh automated evidence, and the
pull request, then stop for acceptance before closing the gate or beginning
Gate 2.
