# Security boundary and limitations

Ginux is research software and is **not production-ready**. This document
states what the current executable evidence supports, what remains trusted,
and what has not been established. It is not an independent security review or
a claim of universal exploit prevention.

## Supported boundary claims

- A managed syscall is resolved through the compiled instance contract and
  selected dispatch mode before its permitted host effect. Unknown operations
  fail closed.
- Paranoid dispatch sends every syscall through the Ginux userspace decision
  path. Selective dispatch permits direct execution only for operations that
  are both contract-allowed and independently cataloged as transparent and
  direct-safe. Denials and complex operations remain Ginux decisions.
- Ginux supplies instance-owned process identities, descriptors, devices,
  `/proc`, `/sys`, and cgroup views. Host kernel trees are not projected into
  an instance.
- The balanced native supervisor drops a root-origin host identity to UID/GID
  65534, clears supplementary groups, has no effective capabilities, enables
  `NoNewPrivs`, and closes inherited descriptors after acquiring the bootstrap
  resources required by the selected dispatch mode.
- Supervisor loss terminates traced workloads through `PTRACE_O_EXITKILL`.
  Unexpected trace stops fail the instance closed and trigger cleanup.
- Concurrent instances retain independent roots, writable state, contracts,
  consoles, audit identity, virtual PIDs, synthetic kernel views, and lifecycle
  state. Supported virtual process operations cannot address a peer instance's
  host process identity.

These claims apply only to the checked-in x86-64 Rocky Linux configurations,
catalog, tests, and named execution modes. They are not a complete Linux ABI
or distribution-compatibility guarantee.

## Residual supervisor authority

The balanced backend deliberately retains authority that it still needs:

- ptrace control over its managed tasks;
- one already-open descriptor for the assigned instance root;
- standard input, output, and error for the selected console or log;
- an optional controller channel used to forward console interrupts; and
- in paranoid mode, a reduced child broker holding the seccomp notification
  descriptor needed for `sched_yield` mediation.

The notification and controller brokers inherit the reduced host identity and
`NoNewPrivs`. They close the instance-root descriptor when they do not need it.
The retained root descriptor and ptrace relationship remain security-critical
Ginux authority; a defect in their validation or use can violate the intended
boundary.

### Filesystem mediation foundation

The native filesystem lane currently claims no syscall. Before later gates can
enable an operation family, both configured native routes now provide one
immutable x86-64 call context containing all six syscall arguments, virtual and
host task identity, instance lifetime and root generation, descriptor-table
generation, typed root/cwd/descriptor authority, and the effective contract
decision. A filesystem hook is eligible only when contract permission,
route-local confinement, operation ownership, object identity, rights, and
current generations all agree.

The generated [filesystem operation inventory](fs-operation-inventory.md)
assigns every currently identified filesystem-capable ABI entry to one owner or
explicit denial. Unknown ABI entries, unknown operations, incomplete authority,
stale generations, and contract denials fail closed. The retained root is
identified from its already-open descriptor; path text cannot replace that
identity. This foundation does not itself add filesystem compatibility or
authorize any new host effect.

## Trusted and excluded components

The Host kernel and ptrace implementation, CPU and hardware, boot chain,
approved Ginux binary and configuration, and operator remain trusted. The
balanced backend also relies on the host kernel for ELF loading, address
spaces, page faults, signals, and scheduling. A host-kernel vulnerability
reachable through an allowed direct-safe syscall or those execution services
is outside the boundary that Ginux currently implements.

Hardware side channels, malicious firmware, physical attacks, a compromised
host kernel, and operator or configuration compromise are excluded. Ginux has
not received formal verification, a third-party penetration test, or an
Independent security review.

Ginux has no guest kernel or hardware-enforced guest boundary. Its current
native backend is not VM- or microVM-equivalent isolation. See
[How Ginux differs from UML, gVisor, namespaces, and VMs](how-ginux-differs.md)
for the architectural comparison.

## Known compatibility gaps

Compatibility is intentionally contract-specific. Unsupported syscalls,
ioctls, device operations, procfs/sysfs nodes, namespaces, and Linux behavior
may return `EPERM`, `ENOSYS`, or another configured Linux error. The initial
milestone covers one Rocky Linux 10 x86-64 systemd baseline and does not claim
complete Linux conformance, arbitrary distributions, multiple architectures,
physical-device passthrough, nested containers, private network namespaces, or
production-scale multi-instance density. The bounded two-instance isolation
evidence and shared-host networking limitation are documented in
[multi-instance-isolation.md](multi-instance-isolation.md).

## Performance limitation

Gate 11's retained selective-mode matrix passes its CPU, memory, and mutex
rows. Its thread-yield rows remain approximately 0.917x to 0.953x native under
x86-64 QEMU TCG and therefore miss the unchanged 4% budget. Same-harness
controls attribute the remaining gap to the host seccomp/task-work path in
that environment. Gate 11 remains partial; Gate 13 must not be read as
accepting or hiding that performance limitation. A fresh matched comparison
against current `main` shows that Gate 13 itself adds no per-workload
regression beyond the existing 4% accepted-Ginux budget.

Gate 15 reports native, selective, and paranoid measurements side by side using
paired native controls and checked-in templates. Native is not a Ginux mode and
provides none of the instance boundary. Selective and paranoid results must be
read with their different syscall-control semantics; faster execution is not
evidence of equivalent enforcement. See [performance.md](performance.md) for
the reproducible protocol and interpretation rules.

## Reproducible evidence

Run the following from a checkout with the pinned images already cached:

```sh
make check-security-doc
make fuzz-security
make test-security
make test-recovery
make test-multi-instance
make demo-gate-13
```

`make fuzz-security` uses checked-in corpora, a fixed PRNG seed, a 4 KiB input
limit, five seconds per target, a 60-second suite bound, and a 256 MiB process
limit. A crash or timeout preserves its current input under
`build/security-fuzz/`. `make test-security` exercises malformed contracts,
selectors, ELF images, VFS indexes, kernel-view paths, device ranges, syscall
dispatch, filesystem capability generations, deterministic path-replacement
races, peer/host tripwires, audit accounting, host-effect denial, host-view
isolation, bounded resources, teardown, and live supervisor authority.
`make test-recovery` exercises configured startup failure, supervisor loss,
forced-stop cleanup, audit bounds, and controlled systemd shutdown.

Passing these tests is regression evidence for the stated configuration. It
does not replace careful source review, independent adversarial testing,
production hardening, or monitoring of the trusted host kernel.
