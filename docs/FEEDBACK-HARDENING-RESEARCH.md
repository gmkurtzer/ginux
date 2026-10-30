# Feedback-Driven Hardening Research

## Purpose

The initial Ginux experiment asked whether Human-Directed Agentic Engineering
(HDAE) could produce an independent kernel capable of running an existing Linux
userspace. This secondary experiment asks a different question:

> Can concrete external kernel-engineering review be converted into a
> disciplined, test-driven hardening iteration without losing the behavior the
> original experiment demonstrated?

This is not a general cleanup or security-audit exercise. Each criticism is
checked against the current implementation, translated into an explicit
invariant, reproduced by a negative test where possible, corrected, and retained
as a regression gate. Work stops after every gate for human review.

## Demarcation

- `research-v1-baseline` identifies commit
  `36503bda2d87465765e1b94e65f9d100f27ed5be`, the published initial artifact.
- `codex/feedback-hardening` contains the complete secondary experiment.
- One pull request carries all gates to preserve an inspectable chronological
  record of the iteration.
- Commits remain small, signed off, and associated with one gate.
- Results, regressions, rejected approaches, and limitations are retained as
  evidence.

The authoritative lifecycle state is maintained in [`hdae/`](../hdae/): project
and architecture context, current status, ordered plan, and one independently
reviewable file per gate.

## Review findings in scope

The external review raised five concrete concerns:

1. mutable global state and unclear ownership;
2. single-CPU serialization through interrupt masking, without SMP or locking;
3. an organically grown, difficult-to-audit x86-64 implementation;
4. unsafe x86-64 `SYSCALL`, NMI, and machine-check entry behavior; and
5. userspace-copy operations without recoverable page-fault handling.

The security scope is deliberately limited to the two reported security defects:
privilege entry and userspace memory access. Passing their gates is not a claim
of general kernel security.

## Operating method

Every gate uses the same HDAE loop:

1. **Verify:** locate the relevant code and state the current invariant.
2. **Reproduce:** create the smallest deterministic test that demonstrates the
   problem or proves the criticism inapplicable.
3. **Specify:** document the intended invariant and failure semantics.
4. **Implement:** make the narrowest coherent change that satisfies it.
5. **Regress:** run the focused negative tests and existing ABI, domain, Rocky,
   storage, and performance gates.
6. **Review:** commit the evidence and stop for human evaluation.

A gate passes only when every stated criterion has fresh evidence.

## Gate map

1. **Baseline and claim reproduction:** capture current compatibility and
   performance evidence, reproduce each concern, and generate the first global
   state inventory without changing behavior.
2. **Privilege-entry hardening:** switch from userspace to guarded per-CPU entry
   storage before any stack use or Rust execution, and provide separate guarded
   IST stacks for NMI, double fault, and machine check.
3. **Fault-recovering userspace access:** route untrusted userspace access
   through audited primitives that recover faults as defined userspace errors.
4. **Global-state classification:** classify every writable global, approve its
   owner and lifecycle, establish the narrow allowlist, and enforce coverage in
   CI.
5. **Runtime-state ownership:** migrate CPU, domain, process, thread, signal,
   scheduler, and syscall scratch state into explicit owners.
6. **Platform/service-state ownership:** migrate filesystem, network, storage,
   device, controller, and supporting service state into explicit owners.
7. **x86-64 decomposition:** split the current 44,000-line CPU module along the
   now-validated ownership and mechanism boundaries without changing behavior.
8. **Synchronization and SMP readiness:** add reviewed atomics, locks, memory
   ordering, lock ordering, and interrupt-safe concurrency semantics.
9. **SMP operation:** start and schedule application processors, implement
   cross-CPU mechanisms, qualify one/two/four-vCPU operation, and make two
   vCPUs the ordinary QEMU test default.
10. **Qualification and comparison:** rerun the complete compatibility,
    lifecycle, Rocky, multi-domain, topology, adversarial, stress, and benchmark
    suites against the initial baseline.

## Dependency order

```text
baseline and reproduction
  -> privilege entry
  -> fault-safe userspace access
  -> global-state classification
  -> runtime ownership
  -> platform/service ownership
  -> structural decomposition
  -> synchronization
  -> SMP operation and 2-vCPU test default
  -> qualification
```

Security findings are fixed before refactoring can obscure their evidence.
Ownership is defined before state moves, decomposition follows proven ownership
boundaries, and SMP does not begin until synchronization is validated. Ordinary
QEMU tests change to two vCPUs only after SMP itself passes; explicit one-, two-,
and four-vCPU coverage remains mandatory.
