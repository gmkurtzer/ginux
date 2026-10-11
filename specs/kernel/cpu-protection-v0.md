# x86-64 CPU Protection Tables v0

**Status:** Experimental single-CPU initialization.

The bootstrap processor installs kernel-owned CPU protection tables after activating the transitional page tables:

- A GDT containing 64-bit kernel code and data descriptors.
- A 64-bit TSS loaded through its system descriptor.
- A guarded 16 KiB privilege stack dedicated to ring-3 entries.
- A 16 KiB interrupt-stack-table allocation dedicated to double faults.
- Absent lower and upper guard pages around the bootstrap,
  privilege-transition, and double-fault stacks.
- An IDT with present interrupt gates for all 32 architectural exception
  vectors.

Interrupts remain disabled while kernel bootstrap code runs. The kernel triggers
`INT3` during every QEMU boot test; the assembly entry stub preserves
general-purpose registers, invokes a Rust handler on the kernel stack, and
returns with `IRETQ`. Successful continuation proves that the GDT, IDT, code
selector, stack discipline, and basic exception return path work together.

The kernel breakpoint exception remains recoverable. Other exceptions from
CPL0 enter a common fatal path with a normalized vector and error code. The
fatal path reports both values and the instruction address, reports CR2 for
page faults, emits `GINUX_KERNEL_PANIC`, and halts. Exceptions from CPL3 instead
enter the scheduler containment path. Its hardware proof deliberately raises
invalid opcode in one context, destroys only that context, and resumes its peer
without a kernel panic. The double-fault gate uses IST1 so it does not depend
on the interrupted stack.

TSS RSP0 names a dedicated privilege stack rather than the suspended kernel
call stack. This prevents repeated user calls from overwriting the kernel
continuation. The pages immediately below and above every native kernel stack
are absent in the active kernel and domain page tables. The privilege stack is
shared by the single bootstrap CPU. Ring-3 execution
admits two non-nested APIC timer interrupts, whose interrupt gate clears IF; kernel handlers
never re-enable interrupts, so privilege-stack uses cannot nest in this proof.

The kernel verifies all 32 entries before continuing its boot test. Later
revisions must capture complete structured fault frames, introduce per-CPU
GDT/TSS/IDT state, and test fatal paths without making the normal suite depend
on a reset or timeout.

All assembly-to-Rust callbacks use the explicit `sysv64` ABI. The UEFI target's
default C ABI is Microsoft x64 and must not be assumed by handwritten stubs.
