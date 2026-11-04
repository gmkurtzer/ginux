# Ginux Project Intent

## Name and terminology

**Ginux** is a configurable userspace kernel that runs isolated Linux
compatibility **domains**. Each domain is governed by a versioned **contract**
describing the kernel-facing interfaces that it may observe and use.

## Problem

Linux preserves a very broad userspace contract. Conventional containers
reduce parts of that surface, but managed processes still execute permitted
system calls directly against the host Linux kernel. A compromised process can
therefore attempt to exploit any host-kernel path that remains reachable.

Ginux interposes a userspace-kernel boundary. A managed syscall may not complete
as an unmediated host syscall. Ginux intercepts each request before its host
effect and either emulates it, performs a validated mediated operation through
the host, or denies it. The
goal is to make compatibility an explicit, configurable security contract
rather than an all-or-nothing inheritance from the host kernel.

## Intended users

- Platform and container-runtime engineers running untrusted or exposed Linux
  workloads.
- Security engineers who need a reviewable kernel-exposure contract and audit
  trail.
- Operators who need systemd-based Linux userspaces with stronger separation
  than conventional namespaces and syscall filters alone.

## Initial outcome

The initial milestone will boot a normal Rocky Linux 10 host with systemd as
the real host PID 1, install Ginux as an ordinary host executable, and run an
unmodified Rocky Linux 10 systemd userspace as virtual PID 1 inside one Ginux
domain. Ginux remains resident as the userspace kernel for the processes it
launches. The Rocky host boots from an official, checksum-pinned cloud QCOW2
image. Each domain virtually boots a digest-pinned Rocky OCI image as its root
filesystem; Ginux pulls and caches that image explicitly so ordinary domain
starts and tests do not require network access.

The milestone succeeds when:

1. no managed x86-64 syscall completes against the host without a Ginux
   contract decision;
2. systemd reproducibly reaches the defined multi-user readiness marker;
3. the domain sees Ginux-owned `/dev`, `/proc`, `/sys`, and cgroup interfaces;
4. domain configuration selects its image, resources, devices, contract, and
   audit behavior;
5. reproducible benchmarks quantify CPU, memory, syscall, process, storage,
   network, and boot overhead against native and conventional-container
   controls.

## Threat model

The primary adversary controls an unprivileged or privileged process inside a
domain and may supply malicious binaries, syscall arguments, ioctl payloads,
filesystem content, network traffic, races, and process interactions. The
adversary attempts to escape the domain, reach unintended host state, exploit
the host kernel, weaken the domain contract, or interfere with another domain.

The host kernel, hardware, Ginux binary and approved configuration, boot chain,
and operator are initially trusted. Hardware side channels, a compromised host
kernel, malicious firmware, and physical attacks are outside the initial threat
model. These exclusions must remain visible and must not be described as
solved.

## Initial scope

- Rust implementation on Linux x86-64.
- macOS arm64 development host using QEMU TCG to emulate the complete Linux
  x86-64 verification environment.
- A normally booted Rocky Linux 10 development VM with host systemd as PID 1;
  Ginux runs inside it as an ordinary process or service.
- An emulation-compatible syscall-interception mechanism selected by Gate 1;
  KVM acceleration is not required by the initial milestone.
- Native x86-64 ELF execution supervised with ptrace syscall-entry and
  syscall-exit stops, with the host kernel performing instruction execution
  and Ginux deciding every syscall before any permitted host effect.
- Three explicit execution profiles: `security` interprets instructions and
  virtualizes kernel-facing state; `balanced` executes instructions natively
  while limiting passthrough to a minimal confined loader/memory surface; and
  `performance` may permit a broader validated native surface after later
  measurement and review.
- One Rocky Linux 10 systemd domain from a cached, digest-pinned OCI image.
- Linux ABI interception, emulation, mediated host operations, denial, and
  audit logging.
- Domain-owned processes, address spaces, file descriptors, virtual kernel
  views, configuration, resources, and lifecycle.
- Virtual console, pseudo-devices, file-backed block devices or bounded
  partitions, and virtual network interfaces.
- Deterministic Makefile targets for build, checks, tests, image preparation,
  defensive boundary verification, and benchmarks.

## Non-goals for the initial milestone

- Running the host distribution root directly under Ginux.
- Replacing the Rocky host's init process or requiring `init=/sbin/ginux`.
- Pulling or assembling OCI images at every boot.
- Arbitrary physical-device, GPU, MMIO, or DMA passthrough.
- VFIO/IOMMU hardware assignment.
- Multiple architectures or compatibility ABIs beyond native x86-64.
- Complete Linux conformance or support for every distribution.
- KVM acceleration or KVM-specific domain behavior.
- Production readiness, independent security certification, or a universal
  claim that all kernel exploits are prevented.
- Multiple domains, containers within a domain, live migration, or distributed
  orchestration. The architecture must not prevent these later milestones.

## Product principles

- No managed syscall reaches a host effect before a Ginux contract decision.
- "Passthrough" means a Ginux-controlled operation whose syscall and arguments
  are validated before the native backend may resume it; it never means an
  unchecked syscall, descriptor, pointer, or ioctl payload.
- Unknown contract operations fail closed and produce useful audit evidence.
- Domain-visible kernel state is Ginux state, not an accidental view of host
  `/proc`, `/sys`, `/dev`, or cgroups.
- Device and resource ownership is explicit, exclusive by default, revocable,
  and bound to domain lifecycle.
- Compatibility claims are tied to executable tests and named environments.
- Performance is continuously measured; an optimization may not bypass the
  mediation boundary.
- Every human checkpoint includes a documented, runnable demonstration with
  expected observations and cleanup instructions.
