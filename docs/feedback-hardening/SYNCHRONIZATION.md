# Synchronization and lock order

Gate 8 replaces the single-processor assumption with explicit synchronization.
Disabling interrupts is only a local-CPU operation and is never treated as
cross-CPU exclusion.

## Primitives

- `SpinLock<T>` is a fair ticket lock for shared state that is never acquired
  by a local interrupt handler while interrupted code may hold it.
- `IrqSpinLock<T>` disables local interrupts, acquires the same ticket lock,
  releases it, and then restores the exact prior interrupt-enable state. It is
  used when process and interrupt contexts can reach the same state.
- Atomics remain appropriate for independent counters, flags, generation
  publication, and immutable-object publication. Multi-field invariants use a
  lock rather than a collection of unrelated atomics.

Both locks use acquire/release ordering. Every lock has a rank, and each CPU
tracks its currently held rank. Acquisition must proceed in strictly ascending
rank order and release in reverse order. Violations fail closed in all builds.

## Global order

1. Architecture and per-CPU state (`10`)
2. Physical memory (`20`)
3. Address spaces and page tables (`30`)
4. Scheduler (`40`)
5. Domains and processes (`50`)
6. Filesystems and file caches (`60`)
7. IPC and signals (`70`)
8. Devices and networking (`80`)
9. Diagnostics (`90`)

Code must not call into a lower-ranked owner while holding a higher-ranked
lock. Slow device waits, blocking scheduler operations, and userspace copies
must occur outside spinlock critical sections unless the gate evidence proves
the bounded operation is required and safe.

## Interrupt rules

- Interrupt handlers may take only locks documented as IRQ-safe.
- A local interrupt must not spin on a lock held by the code it interrupted.
- Restoring interrupts happens only after the final IRQ-safe guard releases.
- NMI and machine-check paths do not acquire ordinary locks.
- Cross-CPU wakeups and TLB invalidation use explicit IPI protocols in Gate 9.
