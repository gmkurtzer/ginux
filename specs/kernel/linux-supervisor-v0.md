# Bounded Linux PID 1 Supervisor Contract v0

**Status:** Ready for the `minimal-system-v0` M5 gate.

The measured `/sbin/init` performs the bounded namespace, filesystem, device,
signal, futex, and child-lifecycle checks before publishing exactly one
`GINUX_SYSTEM_INIT_V0` line through `/dev/console`. It does not close that
descriptor or exit after readiness.

The added supervisor path expands this measured static init from one to two RX
pages. Its separately mapped writable/NX segment remains disjoint, and the
kernel rejects any candidate whose exact executable or writable page counts,
entry point, or permissions differ from the pinned plan.
The static exec transition also installs an empty TLS context and clears the
prior image's FS base before `/sbin/init` can run; a saved supervisor context
cannot inherit a thread pointer from the replaced image.

PID 1 blocks `SIGTERM` and enters an untimed `rt_sigtimedwait` for that signal
with a writable 128-byte `siginfo_t` output. For this exact system-image state,
the kernel saves the complete post-syscall register frame in both the
generation-tagged Linux thread slot and live process registry, marks the thread
blocked, and returns to the native control boundary. No timeout or runnable
Linux peer is required.

The native side accepts the parked state only when PID 1 is still the live
initial process, no Linux thread is running, exactly one signal wait was
published, and the saved process and thread contexts agree. It then generates
one kernel-origin `SIGTERM`, writes the corresponding initialized `siginfo_t`,
wakes the exact waiter, changes that thread from ready to running, and resumes
the same address-space root, instruction pointer, stack, flags, FS base, and
general registers with return value 15.

PID 1 requires signal number 15 and `SI_KERNEL`, writes exactly one
`SUPERVISOR_SHUTDOWN` line using the console descriptor retained across the
wait, unblocks `SIGTERM`, closes the console, and calls `exit_group(42)`. Final
teardown still requires descriptor cleanup, tmpfs teardown, thread retirement,
process-registry consistency, and reclaimed wait state.

The automated QEMU/TCG gate requires the readiness marker, shutdown marker,
native lifecycle validation, and clean exit exactly once. The injected shutdown
is immediate only to keep the boot test finite; the userspace wait itself has
no deadline and cannot advance until the native control event is issued.

This contract does not provide interactive console input, arbitrary signal
injection, daemon restart policy, orphan adoption beyond the bounded child,
multiple concurrent children, or a long-running production scheduler loop.
Those are later profiles rather than inferred behavior.
