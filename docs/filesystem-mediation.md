# Filesystem mediation contract

Gate 53 implements the filesystem boundary requested by issue #290 for the
supported x86-64 balanced-native instance. It does not claim complete Linux
VFS compatibility. The generated
[operation inventory](fs-operation-inventory.md) is the syscall-by-syscall
source of truth for ownership, arguments, flag class, authority, executor, and
the exact result when an operation or variant is unsupported.

## Commit model

Every admitted pathname is copied once into a bounded supervisor buffer and
replayed from a private read-only tracee mapping. Policy and the host syscall
therefore consume the same bytes. Supervisor-produced path translations use
that mapping too; mutable tracee stack memory is not an executor input.

The host kernel retains the instance's already-open root, cwd, and open-file
objects. The supervisor never reopens an authorized workload pathname. A
per-instance commit barrier keeps the namespace generation stable between
policy resolution and syscall completion:

- observation, safe-open, executable-replacement, and inotify-watch paths hold
  a shared lease;
- namespace, metadata, cwd, and `fchdir` changes hold an exclusive lease; and
- the lease is tied to the exact supervised host task and is released on
  syscall completion, successful `exec`, or safe task teardown.

A successful `exec` has no ordinary syscall-exit stop, so its replay mapping,
signal-mask state, pending-exit state, and lease are retired at the exec event.
Each task lifetime receives a unique replay address. This prevents a mapping
left in a `CLONE_VM` sibling's old address space by exec from colliding with a
later task-table reuse. If a shared-address-space task dies during the private
replay protocol, the instance fails closed rather than releasing a mapping
that can still exist in a sibling. A separate-process address space disappears
with its task, allowing only that task's lease to be released.

Later readers are allowed while a writer waits. This is required for valid
FIFO peers: a blocking read-open cannot prevent the write-open that completes
it. A waiting namespace writer commits only after all admitted readers finish.

The trusted operator and host kernel remain outside this mechanism. Moving an
instance directory through an independent host process is outside the managed
workload threat model.

## Supported and unsupported forms

The operation inventory is intentionally wider than the implemented Linux
compatibility surface. It assigns an accountable route even to denied calls.
For every request, the complete six-argument context is checked by the
contract and the route-specific argument validator before the accountable
executor runs. Unsupported syscalls or flag combinations return the inventory
errno, normally `ENOSYS`; host-global `sync` and `syncfs` return `EPERM`.

Notable explicit exclusions are arbitrary mount and namespace construction,
opaque file handles, `pidfd_getfd`, asynchronous filesystem submission,
Landlock manipulation, and host-global filesystem control. `/proc`, `/sys`,
`/dev`, and the mount view remain synthetic Ginux objects. Proc-fd reopening
does not import an arbitrary host descriptor.

The accepted compatibility surface includes normal guest-root absolute,
cwd-relative, and admitted dirfd-relative paths; root-level `..`; scoped
ordinary symlinks; descriptor lifetime after unlink; writable instance state;
and the loader/systemd/package workflows covered by the named tests. Path
bytes are bounded at 4096 bytes per string, each task has one 16 KiB private
snapshot mapping, the commit barrier has 128 concurrent task slots, and replay
addresses are bounded to 4096 task lifetimes (64 MiB of reserved virtual-address
range, with only active or exec-retained slots mapped). Exhaustion fails the
instance closed. Existing descriptor, audit, virtual-object, and instance
limits remain enforced.

## Issue #290 traceability

| Requirement | Enforcement | Primary evidence |
|---|---|---|
| FS-001 | Immutable `CallContext` contains ABI, six arguments, task, instance, contract, root/cwd, and descriptor generations. | component context tests; FS-T08 |
| FS-002 | Independent pinned x86-64 set equals the generated ownership inventory; unknown entries and unsupported variants fail closed. | `make check`; FS-T01, FS-T08 |
| FS-003 | Contract, confinement, ownership, rights, and generations are one conjunctive route guard. | route unit tests; FS-T07, FS-T08 |
| FS-004 | Kernel-held root/cwd/dirfd objects retain distinct path semantics; provisioning strings are not reopened. | native VFS tests; FS-T01, FS-T04 |
| FS-005 | Ordered byte resolver follows links before later parents with bounded depth and no lexical-prefix containment. | resolver/property tests; FS-T01 |
| FS-006 | Immutable pathname replay plus the namespace commit lease joins authorization to kernel execution. | deterministic native FIFO/rename schedule; FS-T02 |
| FS-007 | Two-path policy checks both endpoints and parents; mutations take an exclusive lease. | FS-T02, FS-T03 |
| FS-008 | Follow and no-follow forms have separate inventory semantics and resolver tests. | FS-T01, FS-T03 |
| FS-009 | Descriptor routes require the current instance, lifetime, table generation, and API-equivalent rights; mutation is serialized. | FS-T04, FS-T06 |
| FS-010 | Launch closes inherited descriptors; imports, `SCM_RIGHTS`, pidfd import, and proc-fd reopening are mediated or denied. | native supervision; FS-T04, FS-T05 |
| FS-011 | chdir/fchdir serialize filesystem context; fork/clone descriptor inheritance and exec cleanup remain supervised. | process/native tests; FS-T04, FS-T08 |
| FS-012 | Proc/sys are synthetic; magic-link and opaque-handle transitions are virtualized or denied without host paths. | kernel-view tests; FS-T05 |
| FS-013 | Device and mount identity is typed; privileged namespace and alias interfaces are virtualized or explicitly denied. | device/kernel-view tests; FS-T05 |
| FS-014 | Mappings and transfer calls require descriptor-equivalent rights; the null-path `utimensat` descriptor form is preserved; other empty-path, handle, and async forms are bounded or denied. | FS-T04, FS-T05, FS-T06; systemd update-done regression |
| FS-015 | Input strings use bounded single-copy read-only replay; invalid pointers, overlong input, unsafe translation, and unsupported structures fail closed. | snapshot tests; FS-T07, FS-T08 |
| FS-016 | Contract rights and immutable-image rules apply before the resolved-object executor across pathname and descriptor aliases. | contract/systemd tests; FS-T03, FS-T06 |
| FS-017 | Stage-specific failures preserve errno, pre-effect failures do not mutate, and cleanup releases temporary authority. | FS-T07; recovery tests |
| FS-018 | Path, snapshot, task, descriptor, resolver, audit, and work queues have explicit bounded storage and fail closed on exhaustion. | bounded-resource tests; FS-T07, FS-T09 |
| FS-019 | Instance lifetime and generations invalidate stale work; safe task death releases only its own lease, shared-VM interruption fails closed, and supervisor loss kills tracees. | deterministic native task-death schedule; FS-T09; recovery/multi-instance tests |

FS-T01 through FS-T09 are stable test identifiers in
`crates/ginux-fs-policy/tests/gate53_suites.rs` and the corresponding Make
targets. Native integration adds a recorded check/use schedule: the host test
observes a reader blocked in the actual `open` syscall, releases a conflicting
rename, proves that task remains at one ptrace stop, admits the FIFO peer needed
for liveness, and verifies that the rename commits only after both readers
finish. The full systemd, recovery, security, contract, package, and aggregate
suites provide the valid-workload and regression controls; passing the policy
component tests alone is not a boundary claim.

## Residual limitations

Ginux trusts the host kernel's pathname implementation while the namespace
generation is held stable. It does not provide a hardware boundary, protect
against a compromised host kernel or trusted operator, or establish complete
Linux ABI compatibility. The 128-task commit table and 4096 task-lifetime
snapshot-address budget are current implementation limits. A workload that
deliberately keeps a read-side pathname syscall blocked can delay a namespace
writer in that same instance; it cannot block a peer instance, and ordinary
valid readers remain admissible to avoid FIFO deadlock.
