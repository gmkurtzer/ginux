# Rocky host foundation and interception proof

## Source

Initial Ginus project establishment.

## Goal

Produce a buildable Rust foundation and executable proof that unmodified
x86-64 managed code can run under Ginus as an ordinary process on a normally
booted Rocky Linux 10 host, enter Ginus on a syscall, and cannot invoke the host
syscall ABI directly.

## Approved scope

- Establish the Rust workspace, top-level Makefile, test layout, and QEMU TCG
  Rocky Linux 10 x86-64 development host for the macOS arm64 machine.
- Pin, verify, and cache the official Rocky cloud image used to boot the Gate 1
  host; ordinary builds and tests must not fetch it implicitly.
- Boot Rocky normally to a healthy host systemd state, deploy Ginus, and invoke
  it manually as an ordinary unprivileged-or-bounded-privilege process.
- Record the initial threat model and execution-platform decision evidence.
- Evaluate emulation-compatible trap options and implement the smallest
  experiment needed to run a fixed unmodified x86-64 test ELF and intercept
  selected syscalls without KVM.
- Route intercepted operations through a central decision point that can allow
  emulation or denial; direct host continuation is forbidden.
- Establish matched direct-Linux and conventional-container benchmark controls
  inside the same booted Rocky host for a small CPU loop, syscall loop, and
  process startup measurement.
- Add `doctor`, `build`, `check`, `test`, `test-unit`, `test-intercept`, and
  `bench-baseline` Make targets.
- Add `make demo-gate-1` as a runnable human demonstration of intercepted,
  emulated, and denied syscalls with explicit expected output and cleanup.

## Non-goals

- General ELF compatibility, systemd, a persistent domain, or a complete
  syscall table.
- Preparing the Rocky domain userspace consumed beneath Ginus; Gate 2 owns that
  pipeline after Gate 1 proves the host/runtime boundary.
- Production scheduling, networking, storage, or device support.
- Selecting final numeric performance budgets.

## Completed work

- [x] Created the Rust workspace, cross-target build, and Makefile interface.
- [x] Implemented the reusable `PTRACE_SYSEMU` interception experiment, central
  decision function, and fixed unmodified managed ELF.
- [x] Added positive emulation, checked mediation, denial, and direct-effect
  negative tests.
- [x] Prototyped direct-Linux and Linux-namespace-container benchmark workloads
  with five samples and variance.

## Remaining work

- [ ] Pin and cache the official Rocky 10 x86-64 cloud image.
- [ ] Boot Rocky to normal systemd readiness under QEMU TCG.
- [ ] Convert Ginus from a PID-1 executable to an ordinary host process that
  returns control without rebooting or shutting down the host.
- [ ] Run interception tests and matched benchmarks inside the Rocky host.
- [ ] Prove the host remains healthy after the Ginus process exits.
- [ ] Update documentation and the human demonstration.
- [ ] Human runs or reviews revised `make demo-gate-1` and accepts or rejects
  the gate.

## Acceptance criteria

- The documented host prerequisites are machine-checkable and the project
  builds from a clean checkout on the reference environment.
- Rocky Linux 10 boots normally under QEMU TCG, host systemd remains real PID
  1, and the readiness check succeeds before Ginus starts.
- Ginus runs as an ordinary host process and exits back to the healthy Rocky
  host without rebooting, halting, or replacing host init.
- An unmodified x86-64 test ELF executes a defined instruction sequence and its
  selected syscall is handled by Ginus with the expected Linux-style result.
- A syscall denied by the test contract does not execute on the host and
  returns the expected denial result.
- Evidence demonstrates that managed execution cannot escape the interception
  path by issuing a direct host syscall in the tested execution mode.
- All interception decisions use one central dispatch boundary.
- Direct-Linux and conventional-container baseline measurements run inside the
  same Rocky host and record workload, environment, units, repetitions, and
  variance.
- A human can run `make demo-gate-1` and observe Rocky/systemd readiness, Ginus
  as a non-PID-1 process, a managed syscall answered by Ginus, a denied syscall
  blocked before host execution, post-Ginus host health, and clean VM teardown.
- `make test` succeeds without network access.

## Verification plan

- `make doctor` -> exits zero and reports every required host capability.
- `make host-image-fetch` -> explicitly downloads and checksum-verifies the
  pinned official Rocky image; no other target downloads it.
- `make host-boot-test` -> boots Rocky under QEMU TCG and proves systemd
  readiness with PID 1 before Ginus runs.
- `make build` -> produces the Ginus development binary and test artifacts.
- `make check` -> formatting, lint, and static checks exit zero.
- `make test-unit` -> all foundation unit tests pass.
- `make test-intercept` -> inside the Rocky host, Ginus has a non-1 host PID;
  positive syscall, denied syscall, direct-bypass negative, clean Ginus exit,
  and post-exit host-health checks pass.
- `make test` with network disabled -> exits zero.
- `make bench-baseline` -> emits reproducible Rocky-host native and
  conventional-container baseline records with environment metadata.
- `make demo-gate-1` -> prints the documented interception demonstration,
  validates each observation, cleans up, and exits zero.

## Superseded evidence

- The 2026-11-03 Alpine/initramfs prototype proved that `PTRACE_SYSEMU` can
  intercept the fixed ELF under QEMU TCG, but it incorrectly ran Ginus as host
  PID 1. It no longer satisfies this gate and is retained only as mechanism
  evidence.
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
- The earlier `make demo-gate-1` result is superseded because it replaced host
  init instead of returning to a normal Rocky host.
- Architecture reconciliation: the experiment proves central pre-effect
  dispatch and no direct-continue path for the fixed single-threaded payload.
  It intentionally does not yet implement the later Ginus-owned memory,
  descriptor, virtual-filesystem, device, or versioned domain-contract models.

## Decisions and limitations

- Rust and x86-64 are approved for the initial implementation.
- KVM is outside initial acceptance. Gate 1 selects an interception mechanism
  that works inside QEMU TCG on the macOS arm64 development host.
- Ginus is never the host PID 1 in the approved architecture. Host systemd is
  PID 1; only domain systemd later receives virtual PID 1 semantics.
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

Partial after the 2026-11-03 architecture correction. Pull request 1 must be
revised and all Rocky-host acceptance evidence rerun. Present the revised
`make demo-gate-1`, fresh evidence, and the pull request, then stop for
acceptance before closing the gate or beginning Gate 2.
