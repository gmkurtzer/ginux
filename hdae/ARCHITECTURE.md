# Hardening Architecture and Invariants

This document records the cross-gate architectural constraints for the
feedback-driven hardening experiment. Existing system architecture remains in
the repository's `docs/` design records.

## Governing invariants

1. **Trusted privilege entry.** No Rust code, compiler prologue, push, or call
   may use a userspace-controlled stack after x86-64 `SYSCALL` entry. NMI,
   double fault, and machine check each use guarded, trusted entry storage.
2. **Hardware-authoritative user access.** Access to userspace memory occurs
   only through audited primitives. A fault becomes the defined userspace error
   rather than a kernel panic; software metadata cannot override a rejected
   hardware translation.
3. **Explicit state ownership.** Every writable long-lived object has a named
   owner, lifecycle, synchronization rule, and reset/destruction path.
4. **Real concurrency semantics.** Disabling local interrupts is not global
   mutual exclusion. Shared state uses primitives appropriate to process,
   interrupt, and multi-CPU contexts with a documented lock order.
5. **Per-CPU architecture state.** Entry stacks, GDT, TSS, IST, current task,
   and local interrupt state are per-CPU before additional processors run.
6. **Behavior-preserving decomposition.** Module extraction follows validated
   ownership boundaries and passes the full pre-extraction suite after every
   step.
7. **SMP as normal operation.** Ginux must operate with one, two, and four
   processors. Once the SMP gate passes, two processors become the default for
   ordinary QEMU tests so concurrency remains exercised continuously.
8. **Honest performance parity.** Performance work optimizes general kernel
   mechanisms. Linux and Ginux use the same benchmark binary, workload,
   machine model, CPU model, memory, duration, topology, and sample count;
   benchmark-specific shortcuts are prohibited.

## Current evidence anchors

- `kernel/src/arch/x86_64/cpu.rs:1914` begins singleton syscall scratch state.
- `kernel/src/arch/x86_64/cpu.rs:2233` stores `RSP`, `RCX`, and `R11` before
  loading the trusted return stack.
- `kernel/src/arch/x86_64/cpu.rs:8864` assigns an IST only to vector 8.
- `kernel/src/address_space.rs:2994` validates mappings and then directly
  dereferences userspace addresses without a fault-fixup mechanism.
- `kernel/src/address_space.rs:3053` contains a fast user-copy path that relies
  on kernel-maintained leaf metadata before direct access.
- `kernel/src/memory.rs:708` explicitly records a single-CPU serialization
  assumption.
- `kernel/src/arch/x86_64/cpu.rs` is currently 44,324 lines.

These anchors establish investigation scope, not final conclusions. Gate 1
must reproduce and characterize each claim before implementation begins.

## Dependency rationale

Privilege-entry and user-copy security defects are characterized and fixed
before broad refactoring can erase their original evidence. State ownership is
first classified, then migrated in separate runtime and platform/service
stages. Decomposition follows those ownership boundaries. Synchronization is
validated before SMP activation, and SMP is enabled only on that prepared
foundation.
