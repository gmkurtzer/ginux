# Linux x86-64 Syscall Entry v0

**Status:** Bounded error, I/O, identity, TLS, and termination transitions
verified in QEMU; not a general syscall ABI.

The bounded static ELF proof invokes the architectural x86-64 `SYSCALL`
instruction with Linux register conventions. It submits unsupported call
999 and requires Linux `-ENOSYS` (-38) after resumption. It then uses `rax = 1`
(`__NR_write`) with fd 1, the declared RW segment address, and an eight-byte
length. The kernel accepts 1 through 256 bytes, validates the descriptor and
complete readable VMA range, enables supervisor access only around the bounded
copy-in, emits the exact bytes to the pinned console, and returns the requested
length. A zero-length request or one above the limit returns `-EINVAL`. Next,
`rax = 0` (`__NR_read`) exposes descriptor 0 as a currently disconnected
console-input stream. It returns zero for EOF without reading or validating the
caller buffer; output, closed, and unknown descriptors return `-EBADF`. This is
an explicit bootstrap state until an authorized input service and wait path
exist, not an interactive console implementation.
`rax = 63`
(`__NR_uname`) and the exact RW segment base request
a fixed 390-byte Linux `utsname`. The kernel zero-initializes every byte, fills
all six 65-byte fields, and enables supervisor access to user memory only for
the copy-out, which is subject to the same declared-extent check. The probe
checks success and the `Ginux` system name.
`rax = 20` (`__NR_writev`) accepts one through four 16-byte x86-64 iovec
records for descriptor 1, with a nonzero aggregate of at most 256 bytes. The
kernel copies the complete vector table and every referenced range into a
bounded staging buffer before emitting anything, so an invalid later vector
returns `-EFAULT` without exposing an earlier prefix. Invalid descriptors return
`-EBADF`; zero-length vectors, excessive vector counts, and excessive aggregate
lengths return `-EINVAL`.
`rax = 79` (`__NR_getcwd`) copies the initial container working directory `/`
and its terminator to a writable VMA and returns 2. A one-byte buffer returns
`-ERANGE`; an out-of-range destination returns `-EFAULT`. This is a bounded
identity for the initial root view, not a general VFS or directory state.
`rax = 80` (`__NR_chdir`) accepts exactly `/`, preserving that fixed root view.
Other paths return `-ENOENT`, and an invalid or unterminated pathname returns
the bounded pathname-copy error. No general directory lookup or mutable current
directory state is implied.
`rax = 89` (`__NR_readlink`) recognizes exactly `/proc/self/exe`, copied as a
bounded NUL-terminated pathname, and returns `/init` without a terminator. A
zero output size returns `-EINVAL`; invalid pathname or output ranges return
`-EFAULT`. Other paths return `-ENOENT`. This is not a general procfs.
`rax = 267` (`__NR_readlinkat`) exposes the same exact link when the directory
descriptor is `AT_FDCWD`. Its pathname, output, length, and errors use the same
bounded implementation. Other directory descriptors return `-EBADF`; this is
not a general directory-descriptor or `*at` interface.
`rax = 95` (`__NR_umask`) atomically replaces the single process's permission
mask, returns the previous value, and ignores bits outside `0777`. The initial
value is `022`; the proof changes it twice and restores it before exit.
`rax = 5` (`__NR_fstat`) reports descriptors 0 and 1 as fixed character devices
with mode `S_IFCHR | 0600`, one link, and a 4096-byte preferred block size. The
144-byte x86-64 `stat` result is zero-initialized and copied only after its full
destination range is validated. Unknown descriptors return `-EBADF`; an invalid
range returns `-EFAULT` without changing its valid prefix. This is descriptor
metadata for the bounded console profile, not a general inode or VFS query.
`rax = 16` (`__NR_ioctl`) accepts `TCGETS` and `TIOCGWINSZ` on descriptors 0 and
1. `TCGETS` copies a zero-initialized 36-byte x86-64 Linux `termios`, explicitly
describing the current console as having no kernel line discipline. `TIOCGWINSZ`
copies an eight-byte Linux `winsize` reporting 24 rows and 80 columns, with
pixel dimensions zero. Unknown descriptors return `-EBADF`, unsupported
requests return `-ENOTTY`, and invalid destinations return `-EFAULT`. This is a
fixed early-console contract, not mutable terminal state or a general ioctl
mechanism.
`rax = 8` (`__NR_lseek`) reports `-ESPIPE` for either open console endpoint,
regardless of offset or origin, and `-EBADF` for a closed or unknown descriptor.
The bounded console is a stream and never claims a mutable file position.
`rax = 3` (`__NR_close`) irreversibly clears descriptor 0 or 1 from the bounded
process-local console table. Closing an unknown or already closed descriptor
returns `-EBADF`; subsequent `fstat` and `ioctl` calls observe the closure. No
file, pipe, or socket allocation, inheritance, or general table exists yet.
`rax = 32` (`__NR_dup`) copies an open console descriptor into the lowest of
the same two slots, preserving whether it is the input or output endpoint. An
invalid source returns `-EBADF`; no free slot returns `-EMFILE`. The probe
closes fd 0, duplicates fd 1 into it, writes through the new fd 0, and closes
both slots. There is still no general allocation or inherited table.
`rax = 33` (`__NR_dup2`) atomically replaces either bounded target slot with an
open source's endpoint type. Equal source and target descriptors return the
target unchanged. A closed source or a descriptor outside the two-slot limit
returns `-EBADF`.
`rax = 72` (`__NR_fcntl`) supports `F_DUPFD`, `F_DUPFD_CLOEXEC`, `F_GETFD`,
`F_SETFD`, and `F_GETFL`. The duplication commands copy an open endpoint into
the lowest available slot at or above their nonnegative minimum; a minimum
outside the two-slot table returns `-EINVAL`, and exhaustion returns `-EMFILE`.
`F_DUPFD_CLOEXEC` sets the new slot's `FD_CLOEXEC` bit, while `F_DUPFD`, `dup`,
and a replacing `dup2` clear it. `F_SETFD` retains only `FD_CLOEXEC`; `F_GETFD`
reports it. The current profile has no second `exec` operation yet, so the flag
is process-local observable state rather than an exercised inheritance action.
`F_GETFL` reports `O_RDONLY` for the input endpoint and `O_WRONLY` for output or
duplicated-output endpoints. Closed or unknown descriptors return `-EBADF`;
other mutation and duplication commands return `-EINVAL`.
`rax = 97` (`__NR_getrlimit`) and `rax = 302` (`__NR_prlimit64`) expose immutable
limits for `RLIMIT_STACK` and `RLIMIT_NOFILE`. The stack soft and hard limits
are one page, matching the bounded mapped stack; the descriptor limits are 2,
matching the highest descriptor number the profile can accept plus one.
`prlimit64` accepts PID 0 or 1 and an optional old-limit destination. Other
PIDs return `-ESRCH`, unsupported resources return `-EINVAL`, and any non-null
new-limit pointer returns `-EPERM`. Output is copied only after the full
16-byte `rlimit` range passes writable-VMA validation, so a range crossing the
declared ELF extent returns `-EFAULT` without a partial write. The syscall
entry uses an internal value outside Linux's `-1..-4095` errno range to return
to the kernel, keeping Linux `-EPERM` observable in ring 3.
`rax = 204` (`__NR_sched_getaffinity`) accepts PID 0 or 1, requires an
eight-byte-or-larger caller mask, writes the one-bit CPU 0 mask, and returns 8.
An unknown PID returns `-ESRCH`, an undersized mask returns `-EINVAL`, and an
invalid output range returns `-EFAULT` without a partial write. `rax = 309`
(`__NR_getcpu`) reports CPU 0 and NUMA node 0 through independently optional
four-byte pointers. Both destinations are validated before either is written;
the obsolete cache pointer is ignored as on Linux. These results describe the
single-bootstrap-processor profile and do not claim SMP scheduling support.
`rax = 24` (`__NR_sched_yield`) succeeds as a no-op because this bounded
process has exactly one runnable thread.
The probe uses `rax = 12` (`__NR_brk`) to query an initial break at `0x404000`,
grow within one separately owned zero-filled RW/NX page, write through that
mapping, and shrink the break. A request beyond `0x405000` follows Linux syscall
semantics by returning the unchanged current break rather than a negative
error. The page remains mapped for the lifetime of this bounded process and is
scrubbed during teardown. A valid `write` copies from the grown heap. A second
eight-byte write crossing the current break returns `-EFAULT`, proving syscall
copy ranges do not expand to the whole mapped page.
The probe also performs a valid `write` from unused stack space and requires
`-EFAULT` for an eight-byte range crossing the stack's upper guard boundary.
Another valid `write` copies an embedded payload from the declared RX segment,
while `uname` copy-out to that segment returns `-EFAULT`. Copy direction thus
respects VMA permissions rather than treating every mapped page as writable.
`rax = 14` (`__NR_rt_sigprocmask`) maintains one eight-byte x86-64 Linux signal
mask for the sole thread. It supports `SIG_BLOCK`, `SIG_UNBLOCK`, `SIG_SETMASK`,
and a null-set query; `SIGKILL` and `SIGSTOP` are always cleared. The kernel
validates the set size and every complete input/output range before mutation,
so `-EINVAL` and `-EFAULT` leave the prior mask intact. The proof restores an
empty mask before exit. This is process state only: signal actions, generation,
delivery, pending sets, alternate stacks, and interrupted-syscall behavior are
not implemented yet.
`rax = 13` (`__NR_rt_sigaction`) stores and queries one 32-byte x86-64 action
record for each of signals 1 through 64. Complete action and old-action ranges
are validated before state changes; action masks also clear `SIGKILL` and
`SIGSTOP`. Installing handlers for those two uncatchable signals returns
`-EINVAL`. Handler, flags, and restorer values are retained as opaque ABI state
until delivery is implemented. The proof exercises default queries, normalized
registration, fault atomicity, and restoration of every action before exit.
`rax = 131` (`__NR_sigaltstack`) reports an initially disabled 24-byte x86-64
alternate-stack record. It accepts `SS_DISABLE` or a zero-flag stack of at least
2048 bytes wholly contained in one currently writable probe VMA. Invalid
records return `-EINVAL`, undersized stacks return `-ENOMEM`, and inaccessible
records or stack ranges return `-EFAULT` before state changes. The proof enables,
queries, rejects invalid replacements, and disables the stack before exit.
`SS_ONSTACK`, automatic disarming, and actual signal-stack entry remain
undefined until signal delivery exists.
Before that overwrite, `rax = 158` (`__NR_arch_prctl`) with `ARCH_SET_FS` and
the exact RW segment base establishes a bounded TLS pointer; an FS-relative
load in ring 3 verifies the original payload. `ARCH_GET_FS` copies the active
base to a bounded user pointer. An invalid destination returns `-EFAULT`, and
an unsupported operation returns `-EINVAL`; both errors are recoverable. The
kernel saves the prior supervisor FS-base MSR before entry, restores it after
exit, and reads it back.
Before `getpid`, the probe also invokes `getuid`, `getgid`, `geteuid`, and
`getegid`; each returns zero. These are Linux-container credentials only. They
do not select a native domain, add a capability, or authorize a kernel control
operation. `getresuid` and `getresgid` copy real/effective/saved triples of
zero. All three destinations are validated before any write, so an invalid
third pointer cannot leave a partial result. `rax = 115` (`__NR_getgroups`)
reports one immutable supplementary group, container-local GID 0. A zero-sized
query returns the required count without accessing its pointer, a negative size
returns `-EINVAL`, and a full four-byte destination is required before copying.
`rax = 116` (`__NR_setgroups`) always returns `-EPERM`, including for an empty
list, because this profile's container credentials are immutable. `rax = 135`
(`__NR_personality`) reports `PER_LINUX` for the query value `0xffffffff` and
accepts an idempotent `PER_LINUX` selection. Every flag and alternate execution
domain returns `-EINVAL`, so a process cannot weaken the fixed execution policy
through personality state. The initial process namespace has
one process and one thread:
`getpid` and `gettid` return 1, `getppid` returns 0, and `getpgrp`,
`getpgid(0)`, and `getsid(0)` return 1. `getpgid` and `getsid` return `-ESRCH`
for an unknown PID. `rax = 157` (`__NR_prctl`) supports `PR_SET_NAME` and
`PR_GET_NAME` for this sole thread. The 16-byte name starts as `init`, setters
copy at most 15 bytes and always retain a trailing NUL, and getters atomically
copy all 16 bytes. Invalid user pointers return `-EFAULT`, unknown operations
return `-EINVAL`, and the proof restores `init` before exit so teardown can
verify process-local state. Other `prctl` policy and security controls are not
yet implemented, except for `PR_SET_NO_NEW_PRIVS` and `PR_GET_NO_NEW_PRIVS`.
The former accepts only value 1 with every unused argument zero and
irreversibly sets the process bit; the latter accepts only zero unused
arguments and reports it. Rejected calls return `-EINVAL` without changing the
bit. The proof requires it to remain set through exit. Future exec and
credential-mutation paths must honor this boundary before they are admitted.
Next, `rax = 218` (`__NR_set_tid_address`) registers a
four-byte word inside the declared RW segment and returns TID 1. The bounded
proof rejects an out-of-segment registration with `-EFAULT`. Exit zeroes the
registered word;
because this profile has no second thread or futex support, there is no waiter
to wake. `rax = 273` (`__NR_set_robust_list`) registers one writable 24-byte
robust-list head for that thread. Other lengths return `-EINVAL`, and a head
crossing the declared writable extent returns `-EFAULT`. `rax = 274`
(`__NR_get_robust_list`) reports the retained head and length for PID 0 or 1;
foreign PIDs return `-ESRCH`. Both eight-byte output ranges are validated before
either is changed. Exit discards the registration, but no robust futex traversal
or recovery is attempted.

`rax = 10` (`__NR_mprotect`) supports one irreversible transition: the page
named by the parsed `PT_GNU_RELRO` range may become read-only/NX for the rest of
the process lifetime. The normalized range must be present, nonempty, and no
larger than one page; the address and 4096-byte length must be page-aligned and
exact, and the only accepted protection is `PROT_READ`. All other requests
return `-EINVAL`. The kernel clears the hardware leaf's writable bit, invalidates
that translation, and records the VMA change before resuming userspace. The
probe confirms that a subsequent `write` can still copy from the page while
`uname` copy-out returns `-EFAULT`. Repeating the transition is rejected. This
is a bounded RELRO mechanism, not a general memory-management API. The probe
finally uses `rax = 231`
(`__NR_exit_group`) with `rdi = 42` (status).
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
or credential-based permissions, a general file-descriptor table, arbitrary
user buffers,
threads, general TLS state, or any syscall other than the exact bounded
`read`, `write`, `writev`, `close`, `dup`, `dup2`, bounded `fcntl`, `fstat`,
`lseek`, console `ioctl`, `brk`, `uname`, `getcwd`, `chdir`, bounded
`readlink`/`readlinkat`,
bounded read-only `mprotect`,
`rt_sigaction`, `rt_sigprocmask`, `sigaltstack`, `umask`, read-only
`getrlimit`/`prlimit64`,
`sched_yield`, `sched_getaffinity`, `getcpu`,
`arch_prctl(ARCH_SET_FS/ARCH_GET_FS)`,
credential and supplementary-group queries, fixed `personality`, bounded `prctl` process names, the bounded process/session
identity queries above,
`set_tid_address`, bounded robust-list registration/query, `exit`, and
`exit_group` calls. There is no thread creation, TID allocation,
clear-child-TID futex wake, or robust futex recovery yet. Every other call
returns `-ENOSYS` without disarming
the task, so a caller can handle the error and continue. The entire path
remains disarmed outside the proof. The current copy helpers cover only the one
declared RW segment and the bounded heap below its current break; general
stack-page ranges are also accepted. General VM-area lookup and cross-area
copies remain future work.
Invalid `write` descriptors return `-EBADF`, unsupported lengths return
`-EINVAL`, and a `write` or `uname` range outside that segment returns
`-EFAULT`. Boot verification covers the exact lower and upper boundaries, the
first byte beyond the segment, and address-addition overflow. The ring-3 probe
also submits fd 2, a 257-byte write, and `uname` at the first mapped address
beyond `p_memsz`; it requires the three errors above and then completes the
valid syscall sequence, proving errors do not silently terminate the task.
