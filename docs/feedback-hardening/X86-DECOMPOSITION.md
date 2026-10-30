# x86-64 decomposition contract

Gate 7 decomposes `kernel/src/arch/x86_64/cpu.rs` without changing runtime
behavior. The pre-extraction file contains 48,510 lines. Extraction follows
the ownership boundaries established by Gates 4 through 6; line count is an
observation, not the acceptance target.

## Dependency direction

The intended dependency direction is:

```text
Linux personality policy
        |
        v
process/thread/signal/syscall mechanisms
        |
        v
x86 execution, entry, descriptor, and user-access mechanisms
        |
        v
paging, APIC, PCI, timer, and hardware primitives
```

Architecture mechanisms must not call into unrelated Linux filesystem,
network, IPC, or policy implementations. Linux-facing modules may use narrow
architecture operations, but architecture modules receive data and outcomes
rather than owning Linux policy.

## Planned boundaries

1. `cpu/architecture.rs`: per-CPU entry storage, GDT, TSS, IDT, IST and
   privilege stacks, descriptor installation, and interrupt-gate management.
2. `cpu/user_access.rs`: SMAP-scoped access and fault-recovering raw user-copy
   primitives. Assembly labels and exception-fixup ranges remain explicit.
3. `cpu/execution.rs`: ring transitions, saved register frames, extended user
   state, and architecture return paths.
4. `cpu/linux_abi.rs`: Linux syscall identifiers, ABI constants, and narrow
   argument/result types. This module contains no device ownership.
5. `cpu/linux_signal.rs`, `cpu/linux_thread.rs`, `cpu/linux_time.rs`,
   `cpu/linux_poll.rs`, and `cpu/linux_futex.rs`: cohesive Linux mechanisms
   extracted from dispatch while retaining domain/process-owned state.
6. `cpu/linux_syscall.rs`: syscall dispatch only; subsystem policy remains in
   the existing Linux filesystem, socket, mount, IPC, user, and process
   modules.
7. `cpu/diagnostics.rs`: verification-only probes, traces, and reporting.

## Unsafe and assembly invariants

- `SYSCALL` entry switches to the trusted per-CPU entry stack before Rust,
  compiler-generated code, calls, or pushes execute.
- GS base identifies a live `CpuEntryState` owned by the current processor.
- NMI, double fault, and machine check have distinct guarded IST stacks.
- IDT/GDT/TSS mutation occurs only during CPU initialization or through an
  interrupt-disabled gate update after the source is quiesced.
- User copies execute only inside the audited SMAP window and convert page
  faults inside registered copy ranges into bounded short-copy results.
- Extracted modules may not create new mutable owner roots or bypass the Gate 6
  ownership manifest.

## Verification rule

Every extraction runs formatting, architecture checks, the repository test
suite, Linux ABI coverage, and the deterministic QEMU verification boot before
the next boundary moves. The final gate reruns compatibility and performance
baselines and compares them with the Gate 1 record.

## Extraction record

- Architecture ownership moved first, establishing `CPU_ARCHITECTURE` as the
  single owner of per-CPU entry slots, descriptor tables, and guarded stacks.
- Linux syscall identifiers and ABI constants moved independently so policy
  definitions no longer obscure architecture mechanisms.
- Entry-state installation, GDT/TSS/IDT construction, interrupt-gate mutation,
  and guarded-stack accessors then moved beside their owner. The static
  privilege-entry check follows these mechanisms into `cpu/architecture.rs`
  while continuing to inspect entry assembly in `cpu.rs`.
- Extended-state enablement and container/process XSAVE lifecycle mechanisms
  moved to `cpu/execution.rs`, leaving Linux process policy outside the
  architecture mechanism while retaining explicit runtime ownership.

After these extractions, `cpu.rs` contains 47,037 lines,
`cpu/architecture.rs` contains 423 lines, `cpu/execution.rs` contains 238
lines, and `cpu/linux_abi.rs` contains 868 lines. These measurements record
progress only; cohesive responsibility and dependency direction remain the
gate criteria.
