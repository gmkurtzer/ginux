# Minimal System Container Profile v0

**Status:** Pinned candidate image and machine-readable compatibility matrix; execution as PID 1 is the next gate.

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
eventual PID-1 path requires an initial stack naming `/sbin/init`, observes PID
1, writes the exact `GINUX_SYSTEM_INIT_V0` readiness line to the inherited
console, and exits through `exit_group(42)` for the automated profile gate.

This milestone pins and authenticates the candidate but deliberately does not
claim M5 completion. The current kernel still executes the comprehensive Linux
proof workloads. The archive carries the exact `sbin/init` node while `/sbin`
directory publication remains part of the next execution view. That milestone
must launch this distinct image through a container-specific entry path and
verify its output and teardown.
Process creation/reaping, a mount namespace, tmpfs, procfs, and a complete
user-namespace contract remain explicit missing rows in the matrix.

Build the exact candidate root and signed development image with:

```sh
make system-image
```

The generated `build/initramfs-v0.cpio` is the image whose size and SHA-256
digest are recorded in the signed boot manifest. Generated digests are
commit-specific because the repository-built ELF payloads are part of the
image; releases must publish the generated manifest and source revision
together.
