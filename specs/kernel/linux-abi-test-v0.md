# Linux Userspace Interface Test v0

**Status:** Controller-visible runtime test and complete Linux x86-64 syscall
conformance ledger.

The controller commands are:

- `test linux` or `test linux summary`: run the report and print totals.
- `test linux interfaces`: print every non-syscall interface result.
- `test linux syscalls`: print all 375 native x86-64 syscall results.
- `test linux syscall NAME|NUMBER`: print one syscall result.
- `test linux all`: print every interface and syscall result.

The environment name is explicit so a future BSD or other compatibility
profile can define an independent contract, implementation, and result set.

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
An unset bit prints FAIL and means unsupported or unverified.

The syscall ledger is generated at build time from every row in
`docs/LINUX-COMPATIBILITY.md`, so a syscall cannot silently disappear from the
runtime report. Its outcomes are:

- `PASS`: a runtime conformance probe covers the documented Linux behavior.
- `UNVERIFIED`: dispatch exists, but its Linux semantics are partial or have no
  complete runtime conformance probe.
- `POLICY-DENIED`: the kernel recognizes and deliberately denies the operation.
- `UNSUPPORTED`: the syscall is absent and returns `ENOSYS`.

Dispatch presence alone must never print PASS. As behavior and tests are
completed, one syscall at a time moves from UNVERIFIED to PASS. All output uses
bounded buffered console writes so the diagnostic cannot exhaust its
native-call budget.

The current profile publishes 17 of the 23 interface bits. The syscall ledger
currently reports 81 complete runtime probes, 246 unverified implementations,
48 policy-denied calls, and no unsupported calls. The QEMU controller test
executes the full report, checks representative entries from every outcome,
verifies continued controller PID 1, and shuts down without a panic.
