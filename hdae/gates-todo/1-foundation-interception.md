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

- [x] Created the Rust workspace, cross-target build, Makefile interface, and
  checksum-pinned QEMU TCG x86-64 harness.
- [x] Implemented a PID-1 Ginus experiment using `PTRACE_SYSEMU`, a central
  decision function, and a fixed unmodified managed ELF.
- [x] Added positive emulation, checked mediation, denial, and direct-effect
  negative tests.
- [x] Added direct-Linux and Linux-namespace-container benchmark controls for
  CPU, syscall, and process-startup workloads with five samples and variance.
- [x] Documented the reference environment, threat model, mechanism decision,
  human demo, benchmark method, and current limitations.

## Remaining work

- [ ] Human runs or reviews `make demo-gate-1` and accepts or rejects the gate.

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

- Reference host: Darwin arm64; QEMU 10.1.2; Rust 1.98.1; x86-64 Linux target;
  Alpine 3.24.3 kernel 6.18.58; QEMU TCG with no KVM requirement.
- `make clean && make doctor && make build` -> clean cross-build succeeded;
  doctor found every host tool and the verified cached kernel; generated
  artifacts were rebuilt while the external cache was retained.
- `make check` -> formatting and target-correct Clippy checks passed with
  warnings denied for all four crates.
- `make test-unit` -> three dispatcher unit tests passed.
- `make test-intercept` -> QEMU booted Ginus as PID 1; managed `getpid`
  returned virtual PID 4242; managed `write` was checked but not executed;
  `uname` returned `EPERM`; the direct-write marker was absent; `GATE1: PASS`.
- `make test` -> passed. The target graph contains no acquisition command and
  used only the already verified cached kernel, so ordinary tests did not use
  the network.
- `make bench-baseline` -> 30 samples passed in one emulated x86-64 boot. Five
  samples were recorded for each direct/container and CPU/syscall/process
  combination with nanosecond units, mean, population variance, and standard
  deviation. This run's means were direct CPU 9,290,541 ns, syscall 33,301,371
  ns, process 28,975,450 ns; namespace-container CPU 9,597,893 ns, syscall
  33,766,498 ns, process 28,405,868 ns.
- `make demo-gate-1` -> printed the expected emulated, mediated, denied, bypass
  negative, pass, and cleanup observations and exited zero.
- Architecture reconciliation: the experiment proves central pre-effect
  dispatch and no direct-continue path for the fixed single-threaded payload.
  It intentionally does not yet implement the later Ginus-owned memory,
  descriptor, virtual-filesystem, device, or versioned domain-contract models.

## Decisions and limitations

- Rust and x86-64 are approved for the initial implementation.
- KVM is outside initial acceptance. Gate 1 selects an interception mechanism
  that works inside QEMU TCG on the macOS arm64 development host.
- `PTRACE_SYSEMU` is the Gate 1 bootstrap mechanism, not the final Ginus
  execution architecture. Full Ginus-owned task and memory behavior remains a
  Gate 3 decision.
- The benchmark container control uses conventional Linux mount, PID, UTS, and
  IPC namespaces without an OCI runtime. It measures matched kernel controls,
  not Ginus overhead; budgets remain deferred to Gate 11.
- Performance budgets remain open until baseline evidence exists.

## Delivery strategy

- Working branch: `hdae/foundation-interception`
- Intended base: `main`
- Pull request: https://github.com/gmkurtzer/usshim/pull/1; dedicated branch
  push and PR creation authorized on 2026-11-03; never merge or close without
  explicit authorization
- Planned commits: project scaffold and checks; interception proof and tests;
  benchmark controls and architecture evidence
- Actual commits: `b644e8b` (workspace and interception proof), `30e1c38`
  (benchmark controls and design evidence), `59d8130` (verification evidence),
  and the delivery checkpoint commit. The first two planned implementation
  commits were combined because the scaffold and executable proof formed one
  buildable change.

## Review policy and outcome

Technically passed and delivered in pull request 1 on 2026-11-03; human
acceptance remains pending. Present `make demo-gate-1`, fresh automated
evidence, and the pull request, then stop for acceptance before closing the
gate or beginning Gate 2.
