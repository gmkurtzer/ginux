# Minimal System Container Profile v0

**Status:** Measured candidate executes as PID 1, reports readiness, and exits cleanly.

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
line, and exits through `exit_group(42)`. Final process teardown closes both
the candidate's console descriptor and descriptors inherited across `exec`.
Before readiness, PID 1 enumerates `/proc` and `/proc/1`, reads the immutable
bounded `/proc/1/status` snapshot, and publishes its exact `init`, running,
PID 1, and parent PID 0 fields to the console. This moves procfs to `partial`;
general process enumeration and lifecycle updates remain absent.
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
PID 1 also enumerates the measured `/tmp` anchor, writes `ready\n` to the
bounded kernel-owned `ginux-ready` scratch file, closes and reopens it, and
validates the exact bytes. Process exit scrubs the complete 64-byte backing
store. This moves tmpfs to `partial`; it is not a mounted general tmpfs.
Before publishing readiness, PID 1 invokes Linux `fork`. The independently
owned PID 2 resumes at the actual post-fork instruction with `RAX = 0`, verifies
`getpid() == 2` and `getppid() == 1`, and exits with status 42. PID 1 first
proves invalid `wait4` options and an invalid status pointer do not reap the
zombie, then receives PID 2 and encoded status `42 << 8`; a repeated wait
returns `-ECHILD`. This makes `process.fork-wait` ready for the bounded profile.

This execution gate deliberately does not claim M5 completion. The immutable
root publishes an explicit `/sbin` directory containing the exact `init` node,
and bounded root enumeration reaches it through a second cursor page. PID 1 is
currently a bounded readiness-and-exit workload, not a persistent supervisor
or interactive console owner. General process concurrency, mutable or multiple
mounts, a general tmpfs, a complete procfs, and general user-namespace creation
and mapping APIs remain explicit gaps in the matrix.

Build the exact candidate root and signed development image with:

```sh
make system-image
```

The generated `build/initramfs-v0.cpio` is the image whose size and SHA-256
digest are recorded in the signed boot manifest. Generated digests are
commit-specific because the repository-built ELF payloads are part of the
image; releases must publish the generated manifest and source revision
together.
