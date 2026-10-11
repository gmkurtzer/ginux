# Linux x86-64 Syscall Entry v0

**Status:** One-shot `exit` transition verified in QEMU; not a general syscall
ABI.

The bounded static ELF proof invokes the architectural x86-64 `SYSCALL`
instruction with Linux register conventions: `rax = 60` (`__NR_exit`) and
`rdi = 42` (status). Immediately before entry, the kernel:

- saves `EFER`, `STAR`, `LSTAR`, and `FMASK`;
- programs `STAR` for the existing kernel and user GDT selectors;
- points `LSTAR` at a dedicated low-level entry stub;
- configures `FMASK` to clear IF, DF, and AC; and
- sets `EFER.SCE`, reading every programmed MSR back before entering CPL3.

Unlike an interrupt gate, `SYSCALL` does not switch stacks. The entry stub
therefore disables interrupts, clears AC and DF, captures the user RSP, and
switches immediately to the kernel stack saved by the ring-3 entry trampoline
before calling Rust. The bounded handler accepts only syscall 60, status 42,
and the exact expected initial-stack pointer while an atomic boot-probe flag is
armed. It then returns directly to the saved kernel context rather than using
`SYSRET`. The verifier restores all four MSRs before any later control-plane
workload executes.

This milestone proves the hardware transition and Linux argument registers. It
does not provide persistent per-thread kernel stacks, `SYSRET` or `IRETQ`
resumption, signal interruption, restartable syscalls, scheduler integration,
credentials, file descriptors, pointer-bearing arguments, `exit_group`, or any
syscall other than the one-shot exit probe. An unexpected call cannot receive a
successful result from this path, and the path is disarmed outside the proof.
