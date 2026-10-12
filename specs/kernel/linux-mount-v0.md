# Linux Mount Namespace v0

**Status:** Bounded immutable-root namespace exercised by measured PID 1.

The kernel installs one initial Linux mount namespace only after publishing the
authenticated initramfs root in kernel-owned memory. The namespace table has
four fixed slots. Each identity contains its table slot, a monotonically
increasing per-slot generation, and a monotonically increasing namespace ID.
Exact-handle lookup rejects stale generations and IDs. The initial namespace
cannot be retired.

The namespace contains exactly one root mount. It binds the exact published
root generation and fixed `read-only`, `nosuid`, and `nodev` flags. A root
lookup succeeds only while the namespace handle and the referenced root
generation are both live. Linux pathname, measured-directory, regular-file,
and exec-root resolution all require that live binding; absence or staleness
fails closed.

The current proof runtime serially activates the initial namespace before
admitting the live Linux process registry and deactivates it after process
teardown. It verifies the namespace is still live and read-only at teardown.
The bounded PID 1 view exposes the root as inode 19 at
`/proc/1/mountinfo`, with exact contents:

```text
1 0 0:1 / / ro,nosuid,nodev - ginux-root rootfs ro
```

PID 1 enumerates that record, reads all 51 bytes, writes them to its measured
console, and closes the descriptor before readiness. The QEMU gate requires
both the installation markers and the exact userspace-visible line.

This contract does not implement `mount`, `umount`, remount, bind mounts,
propagation, pivot-root, chroot, namespace cloning, per-process namespace
selection, a writable root, or a general VFS. The additional table slots and
retirement rules are construction and stale-handle proofs, not public support
for creating namespaces.
