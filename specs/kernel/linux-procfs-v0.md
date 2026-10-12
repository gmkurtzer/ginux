# Linux Bounded Procfs v0

**Status:** Implemented for `minimal-system-v0`.

The measured read-only root supplies the `/proc` mount-point anchor. The Linux
compatibility layer projects only live records from its bounded,
generation-safe process table; procfs does not own process lifetime or grant
authority over a process.

PID 1 is always enumerated while the profile is active. `/proc/1/status`
reports the fixed name `init`, running state, PID 1, and PPID 0. Its directory
also exposes the exact read-only `mountinfo`, `uid_map`, `gid_map`, and
`setgroups` records defined by the initial mount and user namespaces.

The real fork child becomes PID 2. Once its address-space resources have been
scrubbed and its process record reaches `Zombie`, `/proc` enumerates directory
`2`; `/proc/2` enumerates `status`; and that file reports `init`, zombie state,
PID 2, and PPID 1. Successful `wait4` removes the exact process record, after
which `/proc/2` and its status file return `-ENOENT`. No stale PID 2 pathname is
resolved after reap.

PID 1 proves the initial records, child appearance, zombie identity, parent
relationship, and post-reap disappearance before publishing `PROCFS_READY`.

Arbitrary process counts, general mutable status, task/thread directories,
descriptor links, memory maps, sysctl, pressure data, tracing interfaces, and
post-reap records are outside v0.
