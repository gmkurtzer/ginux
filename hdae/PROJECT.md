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

Ginux interposes a userspace-kernel boundary. Every managed operation is
governed by a compiled instance contract before its host effect. Ginux derives
the narrowest sound combination of native kernel boundaries, kernel-resident
policy, and userspace mediation from the contract and usable host integrations.
Denied operations and operations requiring inspection, translation, emulation,
or virtual state remain Ginux decisions before completion. The goal is to make
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
   the complete Ginux instance contract and automatic enforcement plan;
2. systemd reproducibly reaches the defined multi-user readiness marker;
3. the instance sees Ginux-owned `/dev`, `/proc`, `/sys`, and cgroup interfaces;
4. instance configuration selects its image, resources, devices, contract, and
   audit behavior;
5. reproducible benchmarks quantify CPU, memory, syscall, process, storage,
   network, and boot overhead against native and conventional-container
   controls.

The post-milestone Gate 15 characterization makes the native, selective, and
paranoid syscall-control costs directly comparable through paired controls,
checked-in test templates, raw samples, tables, and graphs. Native remains a
control rather than a Ginux execution mode.

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
- Native x86-64 ELF execution with automatic enforcement planning: direct
  execution is limited to contract-allowed, cataloged transparent operations,
  while denial and complex semantics remain Ginux decisions.
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

- No managed operation reaches a host effect outside the compiled Ginux
  contract and its complete enforcement plan.
- "Direct passthrough" means the operation is
  explicitly contract-allowed and cataloged as semantically transparent. It
  never applies to an operation requiring argument inspection, descriptor or
  pointer validation, translation, emulation, or virtual state.
- Enforcement machinery is host implementation state, not portable recipe
  syntax. Ginux may combine native limits, BPF LSM/cgroup BPF, Landlock,
  seccomp, and userspace mediation, but may not weaken a contract to start.
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
