# Feedback-Driven Hardening Research

## Purpose

The initial Ginux experiment asked whether Human-Directed Agentic Engineering
(HDAE) could produce an independent kernel capable of running an existing Linux
userspace. This secondary experiment asks a different question:

> Can external expert review be converted into a disciplined, test-driven
> hardening iteration without losing the behavior the original experiment
> demonstrated?

This is not a general cleanup exercise. Each criticism must be checked against
the current implementation, translated into an explicit invariant, reproduced
by a negative test where possible, corrected, and retained as a regression
gate. Work stops after every gate for review before the next gate begins.

## Demarcation and evidence

- `research-v1-baseline` identifies commit
  `36503bda2d87465765e1b94e65f9d100f27ed5be`, the published initial research
  artifact before feedback-driven hardening.
- `codex/feedback-hardening` contains the secondary experiment.
- Commits remain small, signed off, and associated with one gate.
- Results, regressions, rejected approaches, and remaining limitations are
  recorded rather than summarized away.

The initial review raised five concrete concerns:

1. mutable global state and unclear ownership;
2. single-CPU serialization through interrupt masking;
3. an organically grown and difficult-to-audit x86-64 implementation;
4. unsafe x86-64 `SYSCALL`, NMI, and machine-check entry behavior; and
5. user-copy operations without recoverable page-fault handling.

The review is treated as technical input, not as an assumed conclusion. The
first gate must reproduce and characterize each claim before implementation
changes begin.

## Operating method

Every gate follows the same HDAE loop:

1. **Verify:** locate the relevant code and state the current invariant.
2. **Reproduce:** create the smallest deterministic test that demonstrates the
   failure or proves the criticism inapplicable.
3. **Specify:** document the intended invariant and failure semantics.
4. **Implement:** make the narrowest change that satisfies the invariant.
5. **Regress:** run the new negative test and the existing ABI, domain, Rocky,
   storage, and performance gates.
6. **Review:** commit the evidence and stop for human evaluation.

A gate does not pass because code was refactored or a test happened to boot.
Its stated acceptance criteria must all pass. A failed gate blocks dependent
work.

## F0: Baseline and claim reproduction

Status: **Demarcated; characterization pending**

Deliverables:

- Preserve the initial artifact with `research-v1-baseline`.
- Capture the existing verification, Rocky boot, and benchmark results.
- Inventory mutable global state and its current synchronization assumptions.
- Add focused reproductions for syscall-entry stack exposure, missing exception
  stacks, invalid user pointers, and page-table/bookkeeping disagreement.
- Record which external claims are confirmed, partially confirmed, or rejected.

Gate:

- Existing passing checks remain reproducible from the baseline tag.
- Every review claim has code references and a deterministic reproduction or a
  documented reason that it cannot be reproduced.
- New tests initially describe current behavior without silently weakening
  their expected final result.

Stopping point: review the evidence and approve the hardening order.

## F1: Harden x86-64 privilege entry

Deliverables:

- A minimal assembly `SYSCALL` entry path that does not push, call Rust, or use
  the userspace stack before switching to a guarded kernel entry stack.
- Per-CPU entry state instead of singleton syscall scratch storage.
- Distinct guarded IST stacks for NMI, double fault, and machine check.
- Defined nested-NMI behavior and a controlled transition from the entry stack
  to the current task's kernel stack.

Gate:

- NMI, double fault, and machine check use distinct, nonzero IST entries with
  lower and upper guard pages.
- Invalid, unmapped, noncanonical, and guard-page user stack pointers cannot
  panic or corrupt the kernel during a syscall.
- Injected NMI at every instrumented entry stage is contained.
- No Rust code or compiler-generated prologue executes before a trusted kernel
  stack is active.
- Existing Linux ABI and Rocky boot gates still pass.

Stopping point: assembly and exception-entry review.

## F2: Fault-recovering userspace access

Deliverables:

- Audited `copy_from_user`, `copy_to_user`, string-copy, and atomic-user-access
  primitives with page-fault fixups or an equivalent recovery mechanism.
- Hardware page tables as the final authority for access validation.
- Defined Linux-compatible partial-copy and `EFAULT` behavior.
- Balanced SMAP access state on success, fault, and early return.

Gate:

- Unmapped, noncanonical, kernel, read-only, guard-page, and cross-page invalid
  pointers return the defined userspace error without a kernel panic.
- A mapping change between validation and access is recovered safely.
- No syscall directly dereferences an untrusted userspace pointer outside the
  audited access module.
- Software mapping metadata cannot override a rejected hardware translation.
- User-copy fault tests, the Linux ABI suite, and Rocky boot all pass.

Stopping point: adversarial user-pointer test and API review.

## F3: Eliminate unowned mutable global state

Global state is a separate gate. The objective is not a cosmetic ban on global
symbols: immutable boot tables, hardware descriptor tables, and narrow per-CPU
anchors can be legitimate. The objective is to eliminate ambient mutable state
whose owner, synchronization, or lifetime is implicit.

Deliverables:

- A generated inventory classifying every writable global as immutable after
  initialization, bootstrap-only, per-CPU, per-domain, per-process, per-thread,
  or explicitly shared and synchronized.
- State containers with clear construction, ownership, reset, and destruction
  paths.
- Signal queues and delivery state owned by domain/process/thread objects.
- Replacement of singleton runtime scratch state with stack-local, task-local,
  or per-CPU state.
- A narrow reviewed allowlist for unavoidable architecture-level mutable
  statics.

Gate:

- No writable global remains unclassified.
- No raw mutable static remains outside the reviewed architecture/bootstrap/
  per-CPU allowlist, regardless of whether it is spelled `static mut` or hidden
  behind interior mutability.
- Every shared object names its synchronization primitive and lock order.
- Domain teardown, process exit, controller restart, and repeated boot tests
  prove state is reset or reclaimed by its owner.
- CI rejects newly introduced unclassified writable global state.
- Existing compatibility and performance gates show no material regression.

Stopping point: state-ownership and lifecycle review.

## F4: Synchronization and SMP readiness

Deliverables:

- Audited atomics, spin locks, IRQ-safe locks, and a documented lock order.
- SMP-safe frame allocation, scheduling, process tables, sockets, filesystems,
  domain management, and device state.
- Removal of assumptions that disabling local interrupts provides global
  mutual exclusion.
- Host-side model or stress tests for synchronization primitives and core state
  machines where practical.

Gate:

- Shared mutable state is protected by an appropriate primitive.
- Interrupt handlers cannot deadlock by acquiring a lock held by interrupted
  code on the same CPU.
- Lock-order and ownership assertions fail closed during verification builds.
- Randomized concurrent lifecycle, signal, IPC, filesystem, and allocator
  stress completes without corruption, deadlock, or leaked ownership.

Stopping point: synchronization design review before enabling another CPU.

## F5: Decompose the x86-64 implementation

This is an incremental, behavior-preserving decomposition rather than a
big-bang rewrite.

Deliverables:

- Separate modules for entry/return, GDT/TSS/IDT, exceptions, per-CPU/APIC,
  paging/user access, process execution, signals, syscall dispatch, filesystem,
  networking, time, polling, futexes, and diagnostics.
- Narrow interfaces between architecture mechanisms and Linux personality
  behavior.
- Documented safety invariants for unsafe blocks and assembly boundaries.

Gate:

- Every extraction preserves the full pre-extraction test result.
- Module dependencies follow a documented direction without circular ownership.
- Architecture modules do not own unrelated Linux subsystem policy.
- The compatibility and benchmark baselines remain within their accepted
  tolerances.

Stopping point: architecture review before SMP activation.

## F6: SMP foundations

Initial scope is fixed 1-, 2-, and 4-vCPU x86-64 systems. CPU hotplug is a later
decision.

Deliverables:

- ACPI/MADT CPU discovery and application-processor startup.
- Per-CPU GDT, TSS, IST, stacks, current-task state, and local APIC timers.
- Cross-CPU wakeups, IPIs, scheduler operation, and TLB shootdowns.
- Domain-aware CPU placement, accounting, limits, priorities, and QoS policy.

Gate:

- Ginux boots reliably with one, two, and four vCPUs.
- Linux userspace reports and uses the configured CPU count.
- Processes and threads execute concurrently on different CPUs.
- Cross-CPU signals, futexes, pipes, sockets, process exit, and mapping changes
  work correctly.
- Multi-domain CPU limits and priorities are enforced.
- Repeated stress runs produce no deadlock, corruption, stale translation, or
  kernel panic.

Stopping point: SMP functional and scheduling-policy review.

## F7: Qualification and comparison

Deliverables:

- Full Linux ABI, lifecycle, Rocky, storage, networking, and multi-domain runs
  on one, two, and four CPUs.
- Adversarial privilege-entry and user-pointer suites.
- Updated boot, syscall, memory, threading, mutex, storage, and networking
  benchmarks.
- A final comparison against the `research-v1-baseline` tag.

Gate:

- Invalid userspace behavior does not panic the kernel.
- Multiple Rocky domains boot and operate concurrently.
- Single-CPU compatibility and performance do not materially regress.
- Multi-CPU workloads demonstrate useful scaling.
- Remaining security, compatibility, hardware, and performance limitations are
  stated explicitly.

Stopping point: publish the secondary experiment's results, including failed
or incomplete gates.

## Dependency order

The gates intentionally follow this order:

```text
baseline and reproduction
  -> privilege entry
  -> fault-safe user access
  -> explicit state ownership
  -> synchronization
  -> structural decomposition
  -> SMP
  -> qualification
```

SMP does not begin on top of implicit global ownership. Structural cleanup does
not precede the security reproductions in a way that could erase evidence of
the original defects.
