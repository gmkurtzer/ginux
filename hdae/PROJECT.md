# Ginux Project Intent

## Name and terminology

**Ginux** is a configurable userspace kernel that runs isolated Linux
compatibility **instances**. Each instance is governed by a versioned **contract**
describing the kernel-facing interfaces that it may observe and use.

## Problem

Linux preserves a very broad userspace contract. Conventional containers
reduce parts of that surface, but managed processes still execute permitted
system calls directly against the host Linux kernel. A compromised process can
therefore attempt to exploit any host-kernel path that remains reachable.

Ginux interposes a userspace-kernel boundary. Every managed syscall is governed
by a compiled instance contract before its host effect. An instance may select
paranoid dispatch, in which every syscall enters the Ginux userspace dispatcher,
or selective dispatch, in which only explicitly allowed operations whose
semantics are cataloged as transparent may execute directly in the host kernel.
Denied operations and operations requiring inspection, translation, emulation,
or virtual state always enter Ginux before completion. The goal is to make
compatibility an explicit, configurable security contract rather than an
all-or-nothing inheritance from the host kernel.

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
instance. Ginux remains resident as the userspace kernel for the processes it
launches. The Rocky host boots from an official, checksum-pinned cloud QCOW2
image. Each instance virtually boots a digest-pinned Rocky OCI image as its root
filesystem; Ginux pulls and caches that image explicitly so ordinary instance
starts and tests do not require network access.

The milestone succeeds when:

1. no managed x86-64 syscall completes against the host without enforcement of
   the Ginux instance contract and selected dispatch mode;
2. systemd reproducibly reaches the defined multi-user readiness marker;
3. the instance sees Ginux-owned `/dev`, `/proc`, `/sys`, and cgroup interfaces;
4. instance configuration selects its image, resources, devices, contract, and
   audit behavior;
5. reproducible benchmarks quantify CPU, memory, syscall, process, storage,
   network, and boot overhead against native and conventional-container
   controls.

## Threat model

The primary adversary controls an unprivileged or privileged process inside a
instance and may supply malicious binaries, syscall arguments, ioctl payloads,
filesystem content, network traffic, races, and process interactions. The
adversary attempts to escape the instance, reach unintended host state, exploit
the host kernel, weaken the instance contract, or interfere with another instance.

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
- Native x86-64 ELF execution with a configurable syscall-dispatch model: the
  paranoid mode supervises every syscall through Ginux, while the selective
  mode directly passes only contract-allowed, cataloged transparent operations
  and keeps denial and complex semantics in Ginux.
- Three explicit execution profiles: `security` interprets instructions and
  virtualizes kernel-facing state; `balanced` executes instructions natively
  while limiting passthrough to a minimal confined loader/memory surface; and
  `performance` may permit a broader validated native surface after later
  measurement and review.
- One Rocky Linux 10 systemd instance from a cached, digest-pinned OCI image.
- Linux ABI interception, emulation, mediated host operations, denial, and
  audit logging.
- Instance-owned processes, address spaces, file descriptors, virtual kernel
  views, configuration, resources, and lifecycle.
- Virtual console, pseudo-devices, file-backed block devices or bounded
  partitions, an initial host-shared network backend, and later explicit
  per-instance network selection and policy.
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
- KVM acceleration or KVM-specific instance behavior.
- Production readiness, independent security certification, or a universal
  claim that all kernel exploits are prevented.
- Multiple instances, containers within an instance, live migration, or distributed
  orchestration. The architecture must not prevent these later milestones.

## Product principles

- No managed syscall reaches a host effect outside the compiled Ginux contract
  and selected dispatch mode.
- "Direct passthrough" exists only in selective mode and means the operation is
  explicitly contract-allowed and cataloged as semantically transparent. It
  never applies to an operation requiring argument inspection, descriptor or
  pointer validation, translation, emulation, or virtual state.
- Paranoid mode sends every syscall through Ginux before completion. Selective
  mode sends every denial and every complex operation through Ginux, while its
  cataloged direct-safe allow set may execute without a synchronous userspace
  round trip.
- Unknown contract operations fail closed and produce useful audit evidence.
- Instance-visible kernel state is Ginux state, not an accidental view of host
  `/proc`, `/sys`, `/dev`, or cgroups.
- Device and resource ownership is explicit, exclusive by default, revocable,
  and bound to instance lifecycle.
- Host-shared networking still passes through Ginux mediation. Sharing the
  host socket namespace does not create an unchecked syscall path.
- Compatibility claims are tied to executable tests and named environments.
- Performance is continuously measured; an optimization may not bypass the
  mediation boundary.
- Every human checkpoint includes a documented, runnable demonstration with
  expected observations and cleanup instructions.
