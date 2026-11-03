# Ginus Project Intent

## Name and terminology

**Ginus** stands for **Ginux Userspace**. Ginus is a configurable userspace
kernel that runs isolated Linux compatibility **domains**. Each domain is
governed by a versioned **contract** describing the kernel-facing interfaces
that it may observe and use.

## Problem

Linux preserves a very broad userspace contract. Conventional containers
reduce parts of that surface, but managed processes still execute permitted
system calls directly against the host Linux kernel. A compromised process can
therefore attempt to exploit any host-kernel path that remains reachable.

Ginus interposes a userspace-kernel boundary. Managed code cannot issue host
system calls directly. Ginus intercepts each request and either emulates it,
performs a validated mediated operation through the host, or denies it. The
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
the real host PID 1, install Ginus as an ordinary host executable, and run an
unmodified Rocky Linux 10 systemd userspace as virtual PID 1 inside one Ginus
domain. Ginus remains resident as the userspace kernel for the processes it
launches. The Rocky host and domain userspace will be derived from an official,
checksum-pinned cloud QCOW2 image and cached so ordinary boots and tests do not
require network access.

The milestone succeeds when:

1. managed x86-64 code cannot execute host Linux system calls directly;
2. systemd reproducibly reaches the defined multi-user readiness marker;
3. the domain sees Ginus-owned `/dev`, `/proc`, `/sys`, and cgroup interfaces;
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

The host kernel, hardware, Ginus binary and approved configuration, boot chain,
and operator are initially trusted. Hardware side channels, a compromised host
kernel, malicious firmware, and physical attacks are outside the initial threat
model. These exclusions must remain visible and must not be described as
solved.

## Initial scope

- Rust implementation on Linux x86-64.
- macOS arm64 development host using QEMU TCG to emulate the complete Linux
  x86-64 verification environment.
- A normally booted Rocky Linux 10 development VM with host systemd as PID 1;
  Ginus runs inside it as an ordinary process or service.
- An emulation-compatible syscall-interception mechanism selected by Gate 1;
  KVM acceleration is not required by the initial milestone.
- One Rocky Linux 10 systemd domain from a cached cloud image.
- Linux ABI interception, emulation, mediated host operations, denial, and
  audit logging.
- Domain-owned processes, address spaces, file descriptors, virtual kernel
  views, configuration, resources, and lifecycle.
- Virtual console, pseudo-devices, file-backed block devices or bounded
  partitions, and virtual network interfaces.
- Deterministic Makefile targets for build, checks, tests, image preparation,
  defensive boundary verification, and benchmarks.

## Non-goals for the initial milestone

- Running the host distribution root directly under Ginus.
- Replacing the Rocky host's init process or requiring `init=/sbin/ginus`.
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

- No managed process directly continues a host syscall.
- "Passthrough" means a Ginus-controlled, validated, translated or proxied
  operation; it never means exposing a host descriptor or unchecked pointer.
- Unknown contract operations fail closed and produce useful audit evidence.
- Domain-visible kernel state is Ginus state, not an accidental view of host
  `/proc`, `/sys`, `/dev`, or cgroups.
- Device and resource ownership is explicit, exclusive by default, revocable,
  and bound to domain lifecycle.
- Compatibility claims are tied to executable tests and named environments.
- Performance is continuously measured; an optimization may not bypass the
  mediation boundary.
- Every human checkpoint includes a documented, runnable demonstration with
  expected observations and cleanup instructions.
