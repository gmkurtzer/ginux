# Linux User Namespace v0

**Status:** Bounded immutable namespace exercised by measured PID 1 and PID 2.

The kernel installs one initial Linux user namespace before constructing Linux
processes. The fixed table has four slots. A namespace identity contains its
slot, monotonically increasing per-slot generation, and monotonically
increasing namespace ID. Exact lookup rejects stale generation or ID values;
the initial namespace cannot be retired.

The v0 namespace has one immutable UID mapping and one immutable GID mapping:

```text
inside 0 -> outside 100000, length 1
```

All real, effective, and saved credentials are container ID 0. The sole
supplementary group is container GID 0, but `setgroups` is permanently denied.
The Linux permitted, effective, and inheritable capability masks are all zero.
Outside IDs are descriptive isolation identities only: they are never decoded
as native domain IDs, capability handles, or authorization.

Every Linux process-table entry contains the exact namespace handle. PID 1 is
bound during registry creation. Fork copies that handle into the preparing
child before resources are published; transaction verification rejects a
different or stale child binding. The live PID 2 then executes `getuid` and
`getgid` and observes zero before exit.

The bounded PID 1 proc view exposes immutable mode-`0444` files with exact
contents:

```text
/proc/1/uid_map    0 100000 1\n
/proc/1/gid_map    0 100000 1\n
/proc/1/setgroups  deny\n
```

PID 1 enumerates all three records, reads and validates them, and emits
`USERNS_READY` before readiness. The QEMU gate also requires namespace
installation, outside-mapping, and fork-inheritance markers.

This contract does not implement `clone(CLONE_NEWUSER)`, `unshare`, `setns`,
namespace nesting, multiple or writable mappings, subordinate-ID allocation,
credential mutation, file ownership translation, credential-based VFS
permissions, namespaced Linux capabilities, or privileged Linux operations.
The separately classified fail-closed privileged surface is defined by
[Linux Privilege Boundary v0](linux-privilege-boundary-v0.md); any operation
beyond it remains a separate compatibility and policy milestone.
