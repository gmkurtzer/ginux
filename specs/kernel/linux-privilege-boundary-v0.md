# Linux Privilege Boundary v0

**Status:** Fail-closed privileged-operation policy exercised by measured PID 1.

Container UID/GID 0 is an identity inside the generation-bound Linux user
namespace. It has zero Linux capability masks, an empty privileged-operation
mask, and no representation as a native domain ID or capability handle. Native
kernel authority remains available only through explicit native capability
tables; the Linux `SYSCALL` entry does not dispatch the native control ABI.

The Linux dispatcher classifies the following syscall families before parsing
their pointers or applying state:

- identity: `setuid`, `setgid`, `setreuid`, `setregid`, `setresuid`,
  `setresgid`, `setfsuid`, and `setfsgid`;
- root and mounts: `chroot`, `pivot_root`, `mount`, `umount2`, `swapon`, and
  `swapoff`;
- kernel control: `reboot`, hostname/domain-name mutation, module load/delete,
  and both kexec interfaces;
- namespace control: `unshare` and `setns`;
- hardware/observation: `iopl`, `ioperm`, and `perf_event_open`; and
- program injection: `bpf`.

When the current process is bound to the initial namespace, each classified
operation returns `-EPERM`. Absence or staleness of the process namespace fails
with `-ESRCH`. A policy grant cannot accidentally execute an unimplemented
operation: the dispatcher would still return `-ENOSYS` until a separate
implementation and authorization contract is added.

Measured PID 1 attempts `setuid`, `setgroups`, `chroot`, `mount`, `reboot`,
`iopl`, `unshare(CLONE_NEWUSER)`, and `bpf`; every call must return `-EPERM`.
It then issues an unclassified syscall and requires `-ENOSYS`, rechecks UID and
GID 0, and subsequently validates the unchanged immutable root mount. Only
then does it emit `PRIVILEGE_BOUNDARY_READY`.

This is the complete privilege boundary for the named minimal profile, not a
complete Linux security model. Seccomp, LSM hooks, audit, mutable credentials,
namespaced capabilities, namespace creation, device assignment, and any newly
admitted privileged syscall require their own policy and negative tests before
being added to the profile.
