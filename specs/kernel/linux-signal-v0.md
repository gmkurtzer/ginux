# Linux Standard Signal Profile v0

**Status:** Implemented for `minimal-system-v0`.

The bounded x86-64 Linux signal surface supports standard signals 1 through
31 with an eight-byte per-thread mask, 32-byte action records, coalesced
pending bits, first-source metadata, synchronous signal frames, and exact
generation-tagged thread ownership. `SIGKILL` and `SIGSTOP` cannot be caught or
blocked.

The dedicated measured signal runtime proves caught `SIGSEGV` and `SIGUSR1`,
`SA_SIGINFO`, `SA_RESTORER`, `SA_ONSTACK`, `SA_NODEFER`, `SA_RESETHAND`,
alternate-stack delivery, pending-signal priority, context edits, and exact
`rt_sigreturn` restoration. It also proves inherited ignored disposition and
the selected default-fatal `SIGPIPE` path, including process cleanup. The full
syscall and frame layout is specified in
[Linux Syscall Surface v0](linux-syscall-v0.md).

The actual system-image PID 1 independently exercises the non-handler control
path. It saves inherited actions, installs a caught `SIGUSR1`, blocks it,
generates it with `kill`, verifies its pending bit through `rt_sigpending`, and
consumes the generated `SI_USER` record through `rt_sigtimedwait`. It verifies
the pending set is then empty, unblocks the signal, installs and exercises an
ignored `SIGUSR2`, restores both inherited actions exactly, and publishes
`SIGNAL_READY`. Final teardown requires an empty mask and pending set, cleared
source metadata, no active signal frame, and the inherited actions restored.

Real-time signals, arbitrary Linux default dispositions, job-control signals,
nested delivery, floating-point/XSAVE frame state, external asynchronous
sources, general process-directed selection, and restartable-syscall semantics
are outside v0.
