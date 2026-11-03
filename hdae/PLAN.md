# Ginus HDAE Plan

## Strategy

Build Ginus from the security boundary outward. First prove that unmodified
x86-64 code can run without direct access to host syscalls. Then add a cached
Rocky userspace, domain configuration, Linux runtime behavior, virtual kernel
objects, systemd compatibility, explicit contract controls, performance
evidence, an exploit demonstration, and boundary hardening.

Every implementation gate uses a dedicated `hdae/<gate-slug>` branch based on
`main` and is intended for its own pull request. Local commits must be signed
off. No branch is pushed and no pull request is opened or merged without
current explicit authorization.

All thirteen initial gates are human checkpoints. A later conversation may
approve a precise multi-gate batch and delegate objective gates, but this plan
does not assume that authorization.

## Ordered gate map

1. **Foundation and interception proof**: Rust scaffold, threat model,
   Makefile contract, native benchmark baseline, and KVM syscall-interception
   proof.
2. **Rocky image pipeline**: pinned, verified, cached, and offline-prepared
   Rocky Linux 10 userspace.
3. **Execution kernel**: address spaces, ELF loading, syscall ABI, central
   dispatch, and small unmodified programs.
4. **Domain configuration and lifecycle**: versioned configuration,
   normalization, create/start/inspect/stop, resources, and failure behavior.
5. **Virtual VFS and dynamic userspace**: virtual descriptors, root and
   overlay, dynamic loader behavior, and a Rocky shell.
6. **Device ownership and virtual hardware**: virtual `/dev`, console, block
   or bounded partition, virtual networking, exclusivity, revocation, and
   device-path benchmarks.
7. **Process runtime**: process, thread, signal, synchronization, time,
   credential, and event behavior required by systemd.
8. **Virtual kernel views**: Ginus-owned `/proc`, `/sys`, cgroup v2, mounts,
   namespaces, and machine identity without host leakage.
9. **Systemd baseline**: Rocky systemd as virtual PID 1 reaching a defined
   multi-user readiness marker and controlled shutdown.
10. **Configurable contract and audit**: versioned syscall, ioctl, device,
    filesystem, and kernel-view policy with fail-closed validation and logs.
11. **Performance characterization and optimization**: native, conventional
    container, and Ginus comparisons against approved budgets.
12. **Exploit demonstration**: controlled public vulnerability succeeds in the
    vulnerable baseline and is blocked by the Ginus contract while systemd
    remains healthy.
13. **Boundary hardening**: malformed ABI tests, fuzzing, escape attempts,
    Ginus privilege reduction, recovery, performance regression, and explicit
    limitations.

Gates are stored in `hdae/gates-todo/` and remain there until they pass and are
accepted at their checkpoint. Closed gate records move unchanged to
`hdae/gates-done/`. Intentionally postponed gates move to
`hdae/gates-deferred/` only with a recorded reason and return condition.

## Build and verification interface

Gate 1 establishes the top-level Makefile. Later gates add stable targets while
preserving these rules:

- `make test` is deterministic and requires no network access.
- Only an explicit acquisition target such as `make image-fetch` may download
  external content.
- Cached images and generated benchmark data stay out of Git unless a small,
  reviewed fixture or report is intentionally committed.
- Each target returns nonzero when mandatory verification fails.
- Aggregate targets never hide a failing mandatory subtarget.

Planned public targets are:

```text
make doctor
make build
make check
make test
make test-unit
make test-intercept
make image-fetch
make image-verify
make image-prepare
make test-image
make test-runtime
make test-config
make test-domain
make test-vfs
make test-process
make test-devices
make test-block
make test-network
make test-kernel-views
make test-cgroup
make test-systemd
make test-contract
make test-exploit
make test-security
make test-recovery
make bench-baseline
make bench
make bench-compare
make bench-block
make bench-network
make bench-report
```

Targets become mandatory only in the gate that introduces them. Gate records
define their expected result.

## Traceability

- Kernel-exposure control is proven by Gates 1, 3, 10, 12, and 13.
- General systemd compatibility is built by Gates 2, 3, 5, 7, 8, and 9.
- Per-domain configuration and ownership are built by Gates 4, 6, 8, and 10.
- Device mediation is built and measured by Gates 6, 10, 11, and 13.
- Performance is measured from Gate 1 and judged in Gates 9, 11, and 13.
- Reproducibility and offline testing are established by Gates 1 and 2 and
  remain regression criteria for every later gate.

## Deferred roadmap

After the initial thirteen gates, likely milestones include multiple concurrent
domains, conventional containers within a domain, network and storage backend
expansion, physical-device assignment with a separately reviewed IOMMU/VFIO
model, non-x86 architectures, live lifecycle controls, distribution expansion,
and production operational hardening. None is authorized by this plan.

