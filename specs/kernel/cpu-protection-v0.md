# x86-64 CPU Protection Tables v0

**Status:** Experimental single-CPU initialization.

The bootstrap processor installs kernel-owned CPU protection tables after activating the transitional page tables:

- A GDT containing 64-bit kernel code and data descriptors.
- A 64-bit TSS loaded through its system descriptor.
- A 16 KiB interrupt-stack-table allocation dedicated to double faults.
- An IDT with present interrupt gates for all 32 architectural exception
  vectors.

Interrupts remain disabled. The kernel triggers `INT3` during every QEMU boot test; the assembly entry stub preserves general-purpose registers, invokes a Rust handler on the kernel stack, and returns with `IRETQ`. Successful continuation proves that the GDT, IDT, code selector, stack discipline, and basic exception return path work together.

The breakpoint exception is the sole recoverable exception in this revision.
Every other architectural exception enters a common fatal path with a
normalized vector and error code. The fatal path reports both values, reports
CR2 for page faults, emits `GINUX_KERNEL_PANIC`, and halts. The double-fault
gate uses IST1 so it does not depend on the interrupted stack.

The kernel verifies all 32 entries before continuing its boot test. Later
revisions must capture complete structured fault frames, introduce per-CPU
GDT/TSS/IDT state, and test fatal paths without making the normal suite depend
on a reset or timeout.
