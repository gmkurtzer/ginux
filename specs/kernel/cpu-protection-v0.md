# x86-64 CPU Protection Tables v0

**Status:** Experimental single-CPU initialization.

The bootstrap processor installs kernel-owned CPU protection tables after activating the transitional page tables:

- A GDT containing 64-bit kernel code and data descriptors.
- A 64-bit TSS loaded through its system descriptor.
- A 16 KiB interrupt-stack-table allocation dedicated to double faults.
- An IDT with a recoverable breakpoint gate and a fatal double-fault gate.

Interrupts remain disabled. The kernel triggers `INT3` during every QEMU boot test; the assembly entry stub preserves general-purpose registers, invokes a Rust handler on the kernel stack, and returns with `IRETQ`. Successful continuation proves that the GDT, IDT, code selector, stack discipline, and basic exception return path work together.

The double-fault entry uses IST1 and never returns. Later revisions must install handlers for all architectural exceptions, capture structured fault frames and error codes, introduce per-CPU GDT/TSS/IDT state, add stack guards, and test the double-fault path without making the normal test suite dependent on a triple-fault reset.
