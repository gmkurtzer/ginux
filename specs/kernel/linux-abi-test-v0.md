# Linux Userspace Interface Test v0

**Status:** Controller-visible report over the boot-verified bounded Linux
profile.

The controller command is `test linux`. The environment name is explicit so a
future BSD or other compatibility profile can define an independent contract,
implementation, and result set.

Native control operation 34 requires the controller's read-authorized host
capability and two zero arguments. It returns a 23-bit result mask only after
the kernel has completed and published its Linux boot verification. Invalid
authority, arguments, or unpublished results return `NATIVE_CALL_FAILED`.

Bits, in output order, cover syscall entry, ELF64 loading, initial stack and
auxiliary vectors, dynamic loading, TLS, processes and threads, signals,
memory management, descriptors and events, VFS/path lookup, procfs,
devfs/terminal behavior, tmpfs, credentials/user namespaces, clocks/timers,
the admitted ioctl profile, sysfs, mount/namespace APIs, sockets, cgroups,
seccomp/audit/LSM interfaces, vDSO/vvar, and the Linux module userspace API.

A set bit prints PASS and means the boot verifier proved the currently bounded
Ginux contract for that interface. It does not mean full Linux compatibility.
An unset bit prints FAIL and means unsupported or unverified. The controller
prints every result and a passed/failed/total summary, using bounded buffered
console writes so the diagnostic cannot exhaust its native-call budget.

The current profile publishes bits 0 through 15. Bits 16 through 22 remain
unset and therefore visibly fail. The QEMU controller test requires all 23
lines, the `16 passed, 7 failed, 23 total` summary, continued controller PID 1,
and clean shutdown without a panic.
