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

This execution gate deliberately does not claim M5 completion. The immutable
root publishes an explicit `/sbin` directory containing the exact `init` node,
and bounded root enumeration reaches it through a second cursor page. PID 1 is
currently a bounded readiness-and-exit workload, not a persistent supervisor
or interactive console owner. Process creation/reaping, a mount namespace,
tmpfs, procfs, and a complete user-namespace contract remain explicit missing
rows in the matrix.

Build the exact candidate root and signed development image with:

```sh
make system-image
```

The generated `build/initramfs-v0.cpio` is the image whose size and SHA-256
digest are recorded in the signed boot manifest. Generated digests are
commit-specific because the repository-built ELF payloads are part of the
image; releases must publish the generated manifest and source revision
together.
