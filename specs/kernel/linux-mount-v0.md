# Linux Mount Namespace v0

**Status:** Per-domain immutable-root namespace with bounded Linux mount support.

The kernel installs a Linux mount namespace only after publishing the
authenticated domain root in kernel-owned memory. Namespace identities are
generation checked; stale handles and root generations fail closed.

Every domain namespace starts with an immutable root mount using `read-only`,
`nosuid`, and `nodev`. Linux pathname, regular-file, and executable lookup all
require a live namespace binding. Fork inherits the current namespace unless
`CLONE_NEWNS` requests a generation-distinct copy. Process switches restore
the process's namespace.

Domain root may call Linux mount operations. That authority is confined to the
domain; the control domain cannot use it to mutate a workload domain. The PID
1 view currently exposes the root, procfs, and cgroup2 API mounts through
`/proc/1/mountinfo`:

```text
1 0 0:1 / / ro,nosuid,nodev - ginux-root rootfs ro
2 1 0:2 / /proc rw,nosuid,nodev,noexec,relatime - proc proc rw
3 1 0:3 / /sys/fs/cgroup rw,nosuid,nodev,noexec,relatime - cgroup2 cgroup2 rw
```

The current implementation accepts the mount flags and API-filesystem targets
needed by the systemd launch path. The root remains immutable. Complete mount
mutation, propagation, `umount`, pivot-root, chroot, writable filesystems, and
the remaining namespace syscalls are still compatibility work.

Mount is not intended to be policy-denied inside a domain. Linux root must be
able to construct nested Linux namespaces and run container stacks while the
enclosing Ginux domain remains the stronger security and resource boundary.
