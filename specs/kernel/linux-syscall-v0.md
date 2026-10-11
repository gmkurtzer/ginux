# Linux x86-64 Syscall Entry v0

**Status:** Bounded error/`write`/`uname`/`getpid`/termination transitions
verified in QEMU; not a general syscall ABI.

The bounded static ELF proof invokes the architectural x86-64 `SYSCALL`
instruction with Linux register conventions. It first submits unsupported call
999 and requires Linux `-ENOSYS` (-38) after resumption. It then uses `rax = 1`
(`__NR_write`) with fd 1, the declared RW segment address, and an eight-byte
length. The kernel validates all three arguments, enables supervisor access to
user memory only around the bounded copy-in, verifies the expected bytes, and
returns 8. Next, `rax = 63` (`__NR_uname`) and the exact RW segment base request
a fixed 390-byte Linux `utsname`. The kernel zero-initializes every byte, fills
all six 65-byte fields, and enables supervisor access to user memory only for
the bounded copy-out. The probe checks success and the `Ginux` system name. It
then uses `rax = 39` (`__NR_getpid`), requires PID 1 in `rax` after resumption,
and finally uses `rax = 231` (`__NR_exit_group`) with `rdi = 42` (status).
Immediately before entry, the kernel:

- saves `EFER`, `STAR`, `LSTAR`, and `FMASK`;
- programs `STAR` for the existing kernel and user GDT selectors;
- points `LSTAR` at a dedicated low-level entry stub;
- configures `FMASK` to clear IF, DF, and AC; and
- sets `EFER.SCE`, reading every programmed MSR back before entering CPL3.

Unlike an interrupt gate, `SYSCALL` does not switch stacks. The entry stub
therefore disables interrupts, clears AC and DF, captures the user RSP, and
switches immediately to the kernel stack saved by the ring-3 entry trampoline
before calling Rust. The bounded handler requires the exact expected initial
stack pointer, a return RIP inside the RX page, and user flags without IOPL or
VM while an atomic boot-probe flag is armed. `write` and `getpid` return through
`SYSRETQ`. Before Rust dispatch, the assembly stub saves `rdi`, `rsi`, `rdx`,
`r8`, `r9`, and `r10`; it restores them before resumption, leaving only the
architecturally clobbered `rax`, `rcx`, and `r11` changed. The resumed code
checks the result plus sentinel values in `rdi` and `r10`, then invokes exit.
Exit returns directly to the saved kernel context. The verifier restores all
four MSRs before any later control-plane workload executes.

This milestone proves the hardware transition and Linux argument registers. It
does not provide persistent per-thread kernel stacks, `IRETQ` fallback, signal
interruption, restartable syscalls, scheduler integration, credentials, a
general file-descriptor table, output streams, arbitrary user buffers,
threads, or any syscall other than the exact bounded `write`, `uname`, `getpid`,
`exit`, and `exit_group` calls. Every other call returns `-ENOSYS` without disarming
the task, so a caller can handle the error and continue. The entire path
remains disarmed outside the proof.
