# Ginus Architecture

## System boundary

```text
Host Linux kernel
  `- Rocky Linux host systemd: real host PID 1
       `- ginus: trusted ordinary host process and userspace kernel
            `- domain
                 |- versioned contract and normalized configuration
                 |- virtual CPU contexts and address spaces
                 |- virtual processes, descriptors, devices, and kernel views
                 `- Rocky Linux systemd: virtual domain PID 1
                      `- managed services and processes
```

The host boots and operates normally without Ginus replacing its init. Ginus
remains resident after an operator or host service starts it; it does not
install restrictions and then disappear. Domain systemd and its descendants
execute as managed code and see a Linux-compatible interface implemented by
Ginus.

## Security invariants

1. Managed execution cannot invoke the host syscall ABI directly.
2. Every managed syscall enters one central Ginus dispatch path before any host
   effect occurs.
3. Dispatch first resolves the domain contract, then emulates, performs a
   mediated host operation, or denies the request.
4. Managed pointers, structures, strings, and ioctl payloads are copied through
   the domain memory model and validated before use.
5. Managed descriptor numbers resolve only to Ginus objects. Host descriptors
   are never installed into or disclosed to managed code.
6. `/dev`, `/proc`, `/sys`, cgroups, mounts, process identities, and machine
   identity are derived from Ginus state.
7. Device assignment is explicit and exclusive by default. Domain teardown
   revokes its authority and releases its resources.
8. Direct device memory, MMIO, DMA, and unknown ioctls are unavailable unless a
   later contract explicitly introduces a separately reviewed safe mechanism.
9. Invalid contracts and unsupported security-critical configuration fail
   closed.
10. Benchmark optimizations may change implementation but not these invariants.

## Major components

### Host integration

Ginus is an ordinary Rocky host executable. It validates host prerequisites
and domain configuration, initializes logging and recovery behavior, opens
only the host resources needed by configured backends, and starts the domain.
Host systemd may supervise Ginus as a service, but manual invocation is the
Gate 1 baseline. Ginus failure stops its domains without replacing or shutting
down the host init system.

### Execution platform

Development and verification run on a macOS arm64 host through QEMU TCG
emulating a normally booted Rocky Linux 10 x86-64 host. Host systemd remains
real PID 1 and Ginus runs as an ordinary process inside that environment.
Ginus acts as the application kernel for its managed userspace rather than
booting another guest Linux kernel beneath the domain. Gate 1 selects and
proves an emulation-compatible syscall-trap mechanism, address-space control,
and prevention of direct host syscalls. KVM acceleration is a deferred
optimization and is not an initial acceptance requirement.

### Domain memory and task model

Ginus owns managed address spaces, mappings, executable loading, task and
thread identities, scheduling-facing state, signals, clocks, credentials,
capabilities, and lifecycle. Managed PIDs and addresses are not authority to
host processes or memory.

### Contract dispatcher

Every syscall and ioctl resolves through a versioned domain contract. A
decision has one of these effects:

- **Emulate:** answer entirely from Ginus state.
- **Mediate:** validate and translate the request, then use a narrow
  Ginus-owned host operation.
- **Deny:** return the configured Linux-compatible error or terminate the
  managed task when policy requires it.
- **Audit:** record the decision, relevant normalized arguments, result, domain
  identity, and contract version without leaking secrets.

There is no direct-continue action.

### Virtual VFS and image backend

The initial root is prepared reproducibly from a checksum-pinned official Rocky
Linux 10 cloud QCOW2 image. The source image is cached outside Git. Ginus
presents a virtual root with an immutable prepared base and a domain-local
writable overlay. The exact initial preparation and backing strategy is a Gate
2 decision; it must not require network access during ordinary boot or tests.

### Virtual kernel filesystems

- `/dev` resolves names to Ginus device objects.
- `/proc` describes Ginus-managed processes and domain state.
- `/sys` describes only contract-visible virtual devices and attributes.
- `/sys/fs/cgroup` provides the cgroup v2 behavior required by the validated
  systemd baseline.

Host trees are not bind-mounted into the domain.

### Device model

A domain configuration assigns device frontends to Ginus-controlled backends.
Initial device classes include console and pseudo-devices, a file-backed block
device or bounded partition view, and a virtual network interface. Ginus owns
backend handles and mediates reads, writes, ioctls, mappings, and lifecycle.

Raw host block-device assignment, physical-device passthrough, VFIO, GPU
support, direct MMIO, and DMA are later architecture work. They may not be
introduced as ordinary configuration extensions because they change the threat
model.

### Configuration and lifecycle

Each domain has a versioned configuration containing identity, root image,
init, environment, resource limits, contract profile, storage, devices,
networking, audit behavior, and failure policy. Ginus validates and normalizes
the complete configuration before starting managed code. The effective
configuration is immutable evidence for that domain instance; mutable runtime
controls, if later added, require explicit authorization and audit semantics.

### Observability and benchmarking

Audit events expose contract decisions and domain lifecycle without exposing
host authority. Benchmarks compare identical workloads natively, in a
conventional container, and under Ginus. The suite covers CPU, memory, syscall
latency, process operations, filesystem and block I/O, networking, boot time,
and idle overhead.

## Important open decisions

- Exact emulation-compatible execution and syscall-trap mechanism after Gate 1
  evidence.
- Rocky QCOW2 version, checksum, preparation tool, and virtual-root backing.
- Configuration serialization format and command-line interface.
- Initial network backend and packet mediation boundary.
- Numeric performance budgets, set from reproducible baseline evidence before
  Gate 11 begins.
- Final repository and directory rename from `usshim` to `ginus`.

Changing a security invariant or resolving an open decision with material
architectural impact requires human review and an update to the affected open
gate before implementation continues.
