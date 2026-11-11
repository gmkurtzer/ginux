# Kernel-assisted automatic syscall control

## Source

Human direction after accepting Gate 15 to investigate kernel assistance as a
credible route around the measured ptrace and seccomp hot-path costs.

## Goal

Determine, with working prototypes and measurements, whether a Linux kernel
module or another kernel-native mechanism can enforce Ginux instance contracts
with substantially lower overhead while preserving pre-effect denial. Make the
runtime choose the appropriate enforcement backend automatically from the
contract and host capabilities instead of requiring instance authors to select
ptrace, a module, or another dispatcher.

## Approved scope

- Inventory the exact operations currently requiring ptrace, seccomp trace,
  seccomp user notification, userspace emulation, or post-effect observation.
- Research current supported Linux extension points using primary kernel
  documentation and source, including a purpose-built kernel module and
  credible BPF, LSM, seccomp, namespace, cgroup, or pidfd-assisted designs.
- Establish explicit semantic requirements: denial before host effect,
  per-instance policy identity, inheritance across fork/clone/exec, race-free
  policy replacement, cleanup on supervisor death, and fail-closed behavior.
- Implement bounded prototypes for every credible path whose feasibility cannot
  be decided from documentation and source inspection alone.
- Build and load experimental kernel code only inside the disposable Rocky QEMU
  host; do not modify or load code into the macOS development host kernel.
- Benchmark working prototypes against native, current selective, and current
  paranoid controls using the Gate 15 paired methodology, including the
  thread-yield hot path and representative allowed and denied operations.
- Measure latency, throughput, context switches, CPU time, and correctness for
  allowed direct execution, denied pre-effect execution, and operations that
  still require userspace emulation or translation.
- Define automatic backend planning from contract requirements and detected
  host capabilities. Portable instance recipes describe required semantics,
  never implementation mechanisms.
- If more than one backend is justified, implement deterministic selection,
  explicit runtime reporting, safe fallback rules, and tests for each path.
- Update architecture, security, performance, configuration, operator, and HDAE
  documentation with the supported result and rejected alternatives.
- Deliver all Gate 16 work on one dedicated branch and pull request.

## Non-goals

- Loading experimental code into the macOS host kernel.
- Silently weakening an instance contract to obtain performance.
- Treating a mechanism as viable when it only observes an operation after its
  host effect or can be bypassed by raw syscalls, static binaries, or threads.
- Committing to a permanent out-of-tree module before compatibility,
  maintenance, signing, packaging, and fail-closed deployment costs are known.
- Removing the current userspace fallback until a replacement passes its full
  semantic and regression evidence.
- Merging or closing the Gate 16 pull request without human direction.

## Planned work

- [x] Establish the current interception and emulation requirement matrix.
- [x] Research supported kernel extension points and document feasibility.
- [x] Implement the smallest viable kernel-assisted prototype.
- [x] Prototype any materially different credible path needed to decide the
  architecture.
- [x] Add automatic capability discovery and contract-to-backend planning.
- [x] Add semantic, failure, lifecycle, and bypass-resistance tests.
- [x] Run paired performance measurements and profile remaining overhead.
- [x] Update all affected project and operator documentation.
- [x] Run mandatory regressions and a bounded human demonstration.
- [x] Push the branch, open a pull request, and stop for human review.

## Acceptance criteria

- A checked-in requirement matrix maps every mediated operation class to the
  semantic reason it needs interception and the mechanisms capable of meeting
  that requirement.
- Every evaluated mechanism has an evidence-based disposition: implemented,
  retained as fallback, deferred with a concrete dependency, or rejected with
  a reproducible reason.
- At least one kernel-assisted prototype is built, loaded, exercised, and
  unloaded inside the Rocky QEMU host, unless primary-source evidence proves
  every kernel-assisted design impossible before implementation; such a result
  must identify the missing kernel API precisely.
- A denied syscall is rejected before its host effect, including from a raw
  syscall fixture, a static binary, a cloned thread, and a post-exec child.
- Allowed operations avoid a userspace round trip on every backend claiming a
  kernel-resident fast path.
- Instance termination, supervisor failure, module unload attempts, stale
  policy identifiers, and unsupported host capabilities fail closed without
  leaking policy or leaving a managed task uncontrolled.
- Portable instance recipes contain contract semantics but no required backend
  selector. The runtime chooses a backend deterministically, reports the choice
  and reason, and never weakens semantics during fallback.
- A force/debug override, if retained for testing, is host/operator state and
  cannot make an otherwise unsatisfied contract start successfully.
- Paired benchmark evidence compares native, current selective, current
  paranoid, and every working kernel-assisted path. Results include raw samples,
  variance, native-relative ratios, and provenance.
- The recommendation states whether an out-of-tree module is justified,
  identifies its compatibility and operational costs, and gives an implementable
  preferred path plus fallback path.
- Existing contract, security, systemd, lifecycle, and aggregate regression
  suites pass unchanged or with explicitly justified extensions.

## Verification plan

- Static requirement-matrix validator -> every catalog operation class has a
  semantic requirement and mechanism disposition.
- Kernel prototype build test -> compiles against the Rocky QEMU host kernel
  headers with warnings treated as errors.
- QEMU module lifecycle test -> loads, reports capabilities, enforces a bounded
  policy, handles failure cases, and unloads or refuses unload safely.
- Raw-syscall enforcement fixtures -> allowed calls complete directly and
  denied calls return the configured error before any host mutation.
- Fork/clone/exec tests -> policy identity and enforcement survive every
  supported process transition without an uncontrolled interval.
- Supervisor-loss and stale-policy tests -> managed tasks stop or remain
  fail-closed and kernel state is reclaimed deterministically.
- Backend-planner unit and integration tests -> choose the narrowest mechanism
  satisfying the complete contract; reject hosts with no sound implementation.
- Gate 15-derived benchmark target -> produces paired native/current/kernel
  results and machine-checkable reports for all working prototypes.
- `make check test-contract test-security test-systemd test` -> all mandatory
  regressions exit zero.
- `make demo-gate-16` -> reports detected capabilities, selected backend,
  pre-effect denial proof, lifecycle result, and bounded performance comparison.

## Coverage review

- Functionality and failures: included through planning, enforcement, fallback,
  lifecycle, and fail-closed criteria.
- Automated evidence: included through unit, QEMU, raw-syscall, transition,
  failure, benchmark, and regression checks.
- User workflow: recipes remain semantic; runtime selection and explanation are
  observable without requiring backend expertise.
- Documentation: architecture, security, performance, configuration, operator,
  and HDAE records are included.
- Compatibility and rollback: current userspace enforcement remains the
  reference fallback until a replacement satisfies the same contract.
- Operations and maintenance: module loading, kernel compatibility, signing,
  packaging, capability discovery, upgrade, and unload behavior are included in
  the recommendation.
- Performance and security: both are primary acceptance surfaces; privacy is not
  separately applicable beyond existing audit-data controls.

## Evidence

- `docs/kernel-assisted-requirements.tsv` maps 14 mediated operation classes to
  pre-effect needs, context, direct-path eligibility, preferred mechanism,
  fallback, and disposition. `make test-gate16-requirements` validates the
  complete matrix.
- Primary Linux documentation eliminated Syscall User Dispatch, tracepoints,
  unrestricted `fmod_ret`, syscall-table patching, and libc rewriting as sound
  general enforcement. The evidence matrix retains native kernel primitives,
  BPF LSM/cgroup BPF, Landlock where its monotonic model fits, seccomp, and
  userspace fallback.
- The disposable Rocky host runs kernel
  `6.12.0-211.16.1.el10_2.0.1.x86_64` with BPF LSM, cgroup BPF, seccomp,
  kprobes, modules, and Landlock ABI 6 available.
- `make test-gate16-kernel-assist` builds and loads both a bounded cgroup-bound
  kprobe module and a BPF LSM prototype. Raw, static, thread, fork, and exec
  attempts return `EPERM` before mutation. Invalid identifiers are rejected,
  policy replacement is atomic, controller exit remains fail-closed, active
  policy prevents module unload, and BPF pin removal detaches deterministically.
- A fresh alternating, paired five-sample TCG run on 2026-11-10 is checked in
  under `benchmarks/gate16/results/`. Unrelated `sched_yield` throughput is
  `0.997x` native for the module and `1.004x` for BPF LSM; allowed `mkdirat`
  create/remove throughput is `1.000x` and `1.001x`. Denied `mkdirat`
  throughput is 2.176M/s for seccomp, 1.605M/s for the module, and 0.320M/s for
  the later object-aware BPF LSM hook.
- `ginux-instance` now derives deterministic enforcement components from
  semantic contract requirements and usable host integrations. Supported
  kernel APIs are preferred, the module requires explicit host opt-in,
  userspace handles uncovered semantics, and an unsatisfied plan fails closed.
- Portable recipes omit execution backend and syscall-control mechanics.
  Normal execution reports `dispatch=automatic` and the selected enforcement
  component set. Retained selective/paranoid selectors exist only for accepted
  comparison fixtures and explicit test overrides.
- Fresh 2026-11-10 runs of `make check`, `make test-contract`,
  `make test-security`, `make test-systemd`, and `make test` pass, including
  all unit, fuzz, authority, lifecycle, contract-runtime, Rocky systemd, and
  aggregate QEMU integrations. `make test-gate16-kernel-assist` then passed
  the semantic/lifecycle checks and produced the checked-in benchmark refresh.

## Decisions and limitations

- Human direction makes automatic backend selection the intended product model.
  Instance contracts express required behavior, not implementation machinery.
- Multiple implementations are permitted when they serve materially different
  contracts or host capabilities, but no fallback may weaken requested policy.
- Kernel-module experimentation is confined to the disposable Rocky QEMU host.
- The out-of-tree module is not recommended as the default. Its kprobe entry
  redirection is kernel-version and architecture specific and carries build,
  signing, upgrade, incident-response, and rollback costs. It remains a useful
  experimental accelerator.
- The preferred production path compiles each contract into a composition of
  native kernel primitives, BPF LSM/cgroup BPF, Landlock when applicable,
  seccomp for scalar syscall denial, and userspace only for emulation,
  translation, virtual kernel state, or unsupported object semantics.

## Delivery strategy

- Working branch: `hdae/kernel-assisted-dispatch`
- Intended base: `main`
- Pull request: https://github.com/gmkurtzer/ginux/pull/258; do not merge or
  close without human direction.
- Planned commits: gate and research matrix; prototypes; automatic planner and
  integration; verification and benchmarks; documentation and final evidence.
- Actual commits: `584f26bc` (gate definition), `3502b4e6` (module/BPF
  prototypes, research, and planner), and `5b51300b` (automatic recipes,
  requirement matrix, lifecycle coverage, benchmarks, and documentation),
  followed by `24bb489d` (paired evidence and authoritative architecture).

## Review policy and outcome

Human checkpoint. The human authorized autonomous research, prototyping,
testing, implementation of one or more definitive paths, documentation, branch
push, and pull-request creation. Stop at the completed pull request for human
review; do not merge or close it.
