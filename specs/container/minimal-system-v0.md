# Minimal System Container Profile v0

**Status:** M5 bounded profile complete. The measured candidate executes as PID
1, reports readiness, remains blocked as the console-owning supervisor until a
native control-plane shutdown event, and exits cleanly.

The first system-container profile is assembled entirely from repository-owned,
reproducible inputs. `profiles/minimal-system-v0/profile.tsv` is the canonical
machine-readable contract. The manifest builder validates it, embeds it as
`/etc/ginux-system-profile`, and includes it in the signed root-image digest.
The profile binds x86-64, `/sbin/init`, `/dev/console`, and the immutable `newc`
root. Each remaining row records a required observable as `ready`, `partial`,
or `missing`; these states describe implementation coverage and are not a
permission to weaken Linux behavior.

The candidate `/sbin/init` is a separately linked static Linux ELF artifact,
not an alias for the existing proof workload. The kernel resolves it only from
the authenticated root index and validates its bounded ELF plan, canonical
entry point, RX page count, writable page count, and non-executable stack. Its
PID-1 path receives an initial stack naming `/sbin/init`, observes PID 1, opens
the measured `/dev/console`, writes the exact `GINUX_SYSTEM_INIT_V0` readiness
line, waits for the native shutdown event described below, and exits through
`exit_group(42)`. Final process teardown closes both the candidate's console
descriptor and descriptors inherited across `exec`.
Before readiness, PID 1 enumerates `/proc` and `/proc/1`, reads the immutable
bounded `/proc/1/status` snapshot, and publishes its exact `init`, running,
PID 1, and parent PID 0 fields to the console. After fork returns, the parent
enumerates PID 2 in `/proc`, enumerates its `status` record, and verifies its
zombie state, PID 2, and parent PID 1. After `wait4` reaps it, the same pathname
returns `-ENOENT`. This makes procfs ready for the bounded two-process profile;
general process metadata and arbitrary process counts remain absent.
The kernel binds PID 1 to the initial generation-tagged Linux mount namespace.
Its sole root mount references the live measured-root generation and has fixed
`ro,nosuid,nodev` flags. PID 1 reads the exact corresponding
`/proc/1/mountinfo` record before readiness. This makes `root.mount` ready for
this bounded profile; mount creation, bind mounts, remounting, propagation,
namespace cloning, and a general mount syscall remain outside it.
PID 1 is also bound to the initial generation-tagged user namespace. Its
single-entry UID and GID maps translate container ID 0 to outside ID 100000,
Linux capability sets are empty, and `setgroups` is permanently denied. PID 1
enumerates and validates `/proc/1/uid_map`, `/proc/1/gid_map`, and
`/proc/1/setgroups`; PID 2 inherits the exact namespace handle and confirms
UID/GID 0 after fork. This makes `namespace.user` ready for the bounded profile.
Creating or joining namespaces, multiple mappings, map mutation, mutable
credentials, and namespace nesting remain outside it.
The namespace's privileged-operation mask is empty. Before reading status and
mount state, PID 1 proves that representative identity, chroot, mount, reboot,
I/O privilege, namespace, and BPF requests all return `-EPERM`; an unknown
syscall separately returns `-ENOSYS`. UID/GID 0 and the immutable mount remain
unchanged. This makes `privilege-boundary` ready for the bounded profile, not a
claim that every future Linux privileged syscall has already been classified.
PID 1 enumerates the immutable `/dev` directory and validates the exact
`null`, `zero`, `console`, and `tty` entries. It proves scalar discard and EOF
behavior through a read-write `/dev/null` descriptor, zero filling through a
read-only `/dev/zero` descriptor, and terminal ioctl behavior through the
`/dev/tty` alias while retaining `/dev/console` for readiness output. It closes
every auxiliary device descriptor before publishing `DEVFS_READY`. This makes
`devfs` ready for the bounded profile; dynamic device discovery, controlling
terminal ownership, pseudoterminals, and additional device classes remain
outside it.
PID 1 first observes the measured `/tmp` anchor as empty, creates the bounded
kernel-owned `ginux-ready` file with `O_CREAT | O_EXCL`, writes `ready\n`,
enumerates the new dirent, closes and reopens the file, and validates the exact
bytes. It then unlinks the closed file and proves that lookup fails and `/tmp`
is empty again. Creation assigns a new generation, descriptor access requires
that exact generation, and unlink or final teardown scrubs the complete
64-byte backing store. This makes `tmpfs.tmp` ready for this bounded profile;
it is not a claim of general Linux tmpfs compatibility.
Before publishing readiness, PID 1 invokes Linux `fork`. The independently
owned PID 2 resumes at the actual post-fork instruction with `RAX = 0`, verifies
`getpid() == 2` and `getppid() == 1`, and exits with status 42. PID 1 first
proves invalid `wait4` options and an invalid status pointer do not reap the
zombie, then receives PID 2 and encoded status `42 << 8`; a repeated wait
returns `-ECHILD`. This makes `process.fork-wait` ready for the bounded profile.
PID 1 then publishes `PROCFS_READY`, tying the observable process records to
the same fork, zombie, wait, and reap lifecycle.
Before fork, PID 1 saves the inherited `SIGUSR1` and `SIGUSR2` actions. It
installs a caught `SIGUSR1`, blocks and generates it with `kill`, observes the
pending bit, consumes the exact signal and `siginfo_t` synchronously through
`rt_sigtimedwait`, and proves the pending set is empty. It then unblocks the
signal, proves an ignored `SIGUSR2` is discarded, restores both inherited
actions, and publishes `SIGNAL_READY`. Together with the dedicated measured
signal runtime's caught-handler, `rt_sigreturn`, synchronous-fault, alternate
stack, ordering, and selected fatal-disposition proofs, this makes
`signal.standard` ready for the bounded profile.
Before fork, PID 1 also applies `FUTEX_WAIT_PRIVATE` to an aligned readable word
whose value differs from the expected value, requires `-EAGAIN`, and proves the
word remains unchanged. `FUTEX_WAKE_PRIVATE` on the same address with no waiter
must return zero before PID 1 publishes `FUTEX_READY`. The dedicated measured
two-thread runtime separately proves real scheduler blocking, exact-address
explicit wake, relative timeout, and clear-child-TID zero-and-wake at child
exit. Together these checks make `futex.private` ready for the bounded profile;
general queues, multiple waiters, cross-process keys, bitsets, requeueing,
priority inheritance, and robust-list recovery remain outside it. The complete
contract is recorded in `specs/kernel/linux-futex-v0.md`.

After completing the child lifecycle and publishing `GINUX_SYSTEM_INIT_V0`,
PID 1 retains its open console descriptor and enters an untimed
`rt_sigtimedwait` for blocked `SIGTERM`. With no runnable peer, the native side
records its exact generation-bound return context and blocked wait rather than
returning `-ENOSYS`. The control plane verifies that PID 1 remains live, injects
a kernel-origin shutdown event, and resumes that same context. PID 1 validates
the returned signal and `siginfo_t`, writes `SUPERVISOR_SHUTDOWN` through the
same console descriptor, restores its mask, closes the console, and exits. The
QEMU gate requires the readiness and shutdown markers exactly once. This makes
`supervisor.pid1` ready and completes the bounded M5 gate. It does not yet
provide an interactive input terminal, arbitrary external signals, restart
policy, or an unbounded multi-process service manager. The exact lifecycle is
specified in `specs/kernel/linux-supervisor-v0.md`.

The immutable root publishes an explicit `/sbin` directory containing the exact
`init` node, and bounded root enumeration reaches it through a second cursor
page. General process concurrency, mutable or multiple mounts, a general tmpfs
or procfs, and general user-namespace creation and mapping APIs remain outside
the bounded matrix.

Build the exact candidate root and signed development image with:

```sh
make system-image
```

The generated `build/initramfs-v0.cpio` is the image whose size and SHA-256
digest are recorded in the signed boot manifest. Generated digests are
commit-specific because the repository-built ELF payloads are part of the
image; releases must publish the generated manifest and source revision
together.
