# Bounded Linux Process Lifecycle v0

**Status:** Internal lifecycle contract verified during x86-64 QEMU boot.

This contract introduces the kernel-owned process identity and lifecycle model
needed before Ginux can expose process-style Linux `clone`/`fork` and `wait4`.
It is deliberately separate from the existing bounded Linux thread table:
threads share one process address space, while every process record owns a
distinct nonzero address-space root.

## Identity and capacity

The v0 table contains four slots. A process is identified internally by the
tuple `(slot, generation, pid)` rather than by PID alone. Slot generations
increase on reuse and PIDs increase monotonically, so a handle retained after
reaping cannot select a replacement process.

The first record is PID 1 and has no parent. PID 1 can be created only once.
Every later record names a live parent and receives a distinct address-space
root. Creation fails without partial publication when the table is full, the
parent is absent or not running, the root is zero, the root is already owned,
or an identity counter is exhausted.

## Exit and wait ordering

A running process cannot become a zombie while it still owns its address-space
root. Teardown must first release the exact recorded root, then publish the
exit status. A mismatched root or stale handle changes no state.

Only the live recorded parent can reap a zombie, and only by the child's PID.
Waiting for a running child or an unrelated process has no effect. Reaping
removes the complete record and makes its slot reusable; the next occupant has
a new generation and PID.

The boot verifier constructs PID 1, rejects an aliased child root, creates PID
2, proves that it cannot be reaped or exited early, releases its root, publishes
status 42, and reaps it through PID 1. It then reuses the same slot for PID 3,
rejects the stale PID 2 handle, and repeats teardown with status 7.

## Deliberate limits

This is an internal ownership contract, not Linux process compatibility yet.
It is not connected to the syscall dispatcher and does not implement:

- address-space copying or copy-on-write;
- process-style `clone`, `fork`, `vfork`, `wait4`, or `waitid`;
- descriptor, credential, signal, mount, or namespace inheritance;
- concurrent process scheduling or process-directed signal delivery;
- orphan reparenting, subreapers, process groups, or sessions;
- resource accounting, limits, PID namespaces, or procfs generation from this
  table; or
- general capacity or PID/generation rollover recovery.

The minimal system profile therefore continues to report
`runtime process.fork-wait missing`. A later milestone may change that row only
after PID 1 creates, runs, observes, and reaps an independently owned ring-3
child through the Linux ABI.
