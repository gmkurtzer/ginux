# Linux x86-64 Syscall Entry v0

**Status:** Bounded error, I/O, identity, TLS, and termination transitions
verified in QEMU; not a general syscall ABI.

The bounded static ELF proof invokes the architectural x86-64 `SYSCALL`
instruction with Linux register conventions. It first submits unsupported call
999 and requires Linux `-ENOSYS` (-38) after resumption. It then uses `rax = 1`
(`__NR_write`) with fd 1, the declared RW segment address, and an eight-byte
length. The kernel validates all three arguments, enables supervisor access to
user memory only around a copy-in constrained by the RW segment's declared ELF
memory extent, verifies the expected bytes, and returns 8. Next, `rax = 63`
(`__NR_uname`) and the exact RW segment base request
a fixed 390-byte Linux `utsname`. The kernel zero-initializes every byte, fills
all six 65-byte fields, and enables supervisor access to user memory only for
the copy-out, which is subject to the same declared-extent check. The probe
checks success and the `Ginux` system name.
Before that overwrite, `rax = 158` (`__NR_arch_prctl`) with `ARCH_SET_FS` and
the exact RW segment base establishes a bounded TLS pointer; an FS-relative
load in ring 3 verifies the original payload. `ARCH_GET_FS` copies the active
base to a bounded user pointer. An invalid destination returns `-EFAULT`, and
an unsupported operation returns `-EINVAL`; both errors are recoverable. The kernel saves the prior
supervisor FS-base MSR before entry, restores it after exit, and reads it back.
Before `getpid`, the probe also invokes `getuid`, `getgid`, `geteuid`, and
`getegid`; each returns zero. These are Linux-container credentials only. They
do not select a native domain, add a capability, or authorize a kernel control
operation. The probe then uses `rax = 39` (`__NR_getpid`), requires PID 1 after
resumption, and `rax = 186` (`__NR_gettid`), which returns TID 1 for the only
thread. It finally uses `rax = 231` (`__NR_exit_group`) with `rdi = 42` (status).
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
Exit returns directly to the saved kernel context. The verifier restores and
reads back all four syscall MSRs plus FS base before any later control-plane
workload executes.

This milestone proves the hardware transition and Linux argument registers. It
does not provide persistent per-thread kernel stacks, `IRETQ` fallback, signal
interruption, restartable syscalls, scheduler integration, mutable credentials
or credential-based permissions, a general file-descriptor table, output
streams, arbitrary user buffers,
threads, general TLS state, or any syscall other than the exact bounded
`write`, `uname`, `arch_prctl(ARCH_SET_FS/ARCH_GET_FS)`, credential queries, `getpid`,
`gettid`, `exit`, and `exit_group` calls. There is no thread creation or TID
allocation yet. Every other call returns `-ENOSYS` without disarming
the task, so a caller can handle the error and continue. The entire path
remains disarmed outside the proof. The current copy helpers cover only the one declared RW
segment; general VM-area lookup and cross-segment copies remain future work.
Invalid `write` descriptors return `-EBADF`, unsupported bounded lengths return
`-EINVAL`, and a `write` or `uname` range outside that segment returns
`-EFAULT`. Boot verification covers the exact lower and upper boundaries, the
first byte beyond the segment, and address-addition overflow. The ring-3 probe
also submits fd 2, a seven-byte write, and `uname` at the first mapped address
beyond `p_memsz`; it requires the three errors above and then completes the
valid syscall sequence, proving errors do not silently terminate the task.
