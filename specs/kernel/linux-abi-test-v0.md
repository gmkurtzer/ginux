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
An unset bit prints FAIL unless the interface is explicitly designated as a
policy exemption. The Linux module userspace API is currently policy-exempt
because module loading is reserved to the control plane.

The syscall ledger is generated at build time from every row in
`docs/LINUX-COMPATIBILITY.md`, so a syscall cannot silently disappear from the
runtime report. Runtime output has exactly three outcomes:

- `PASS`: a runtime conformance probe covers the documented Linux behavior.
- `FAIL`: the implementation or its conformance proof is incomplete, including
  absent calls that return `ENOSYS`.
- `POLICY-EXEMPT`: the kernel recognizes and deliberately denies the operation
  as part of the documented Ginux security policy.

Dispatch presence alone must never print PASS. As behavior and tests are
completed, one syscall at a time moves from FAIL to PASS. There is no
`UNVERIFIED` terminal category: lack of verification is a failure. All output
uses bounded buffered console writes so the diagnostic cannot exhaust its
native-call budget.

`test linux all` emits one table with `NAME/IDENTIFIER`, `TYPE`, `ID`, and
`RESULT` columns. The current profile reports 17 passing, 5 failing, and 1
policy-exempt interface. The syscall ledger reports 82 passing, 245 failing,
and 48 policy-exempt calls. The QEMU controller test executes the full report,
checks representative entries from every outcome, verifies continued
controller PID 1, and shuts down without a panic.
