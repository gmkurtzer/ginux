# Architecture: An Immutable, Container-Native OS for AI Infrastructure

**Status:** Initial design proposal (v0.3)
**Purpose:** Foundational specification for a Git repository and agent-assisted engineering.
**Scope:** Architecture and requirements, not a claim of implementation or proven feasibility.

## 1. Vision

Build a new, Rust-first operating system that boots directly via UEFI and acts as a small, immutable, capability-secure container host for AI/HPC servers. The kernel boots one native control-plane domain. That control plane starts isolated native services and Linux-compatible containers; there is no conventional host distribution.

The first Linux environment is a small, pinned x86-64 **system-container test image** with a minimal PID 1 and console. It exists to drive the compatibility contract without making a conventional host distribution part of the trusted base. Rocky Linux, systemd, SSH, package management, and application stacks are later container-compatibility profiles. Linux compatibility is a container interface rather than a requirement that the native host itself behave like a complete Linux system.

**North star:** A minimal, verifiable privileged core; one explicitly authorized native control plane; Linux-compatible containers above isolated services; stable language-neutral driver and service interfaces; immutable executable integrity and authority boundaries; and no unnecessary cycles in critical data paths.

The design is intentionally narrower than Linux in hardware support. A binary compiled for a supported stable native system or driver ABI must remain compatible within its specified contract. Ginux does not expose Linux kernel internals or load unmodified Linux `.ko` files into its kernel; relevant drivers are ported into isolated domains.

## 2. Non-negotiable design principles

1. **Container-native Linux compatibility:** First run a minimal unmodified Linux userspace as a system container, then expand the defined compatibility profiles to systemd distributions such as Rocky Linux, package management, SSH, networking, OCI workloads, NVIDIA/CUDA, and AI/HPC applications.
2. **Minimal privileged core:** Keep scheduling, address-space protection, capability enforcement, essential interrupt and hardware mediation, and fast IPC in the core. Put everything else in isolated services when feasible.
3. **One initial authority:** The kernel creates one native control-plane domain from the signed boot manifest. It may delegate or revoke authority, but it cannot create authority beyond the immutable boot envelope.
4. **Security by construction:** Explicit capabilities, no ambient host authority, hardware-enforced memory and DMA isolation, least privilege, revocation, and defense in depth. Process trees define ownership and lifecycle; capability graphs define authority. Linux namespaces are a compatibility interface, not the native security boundary.
5. **Runtime immutability:** Kernel code and read-only data cannot be replaced or made writable at runtime. No third-party code executes in the kernel address space. Runtime state may change only through authorized interfaces and within the boot authority envelope.
6. **Performance by measurement:** Favor direct authorized data paths, shared memory, batching, and hardware queues; isolate control paths. No architectural purity at the expense of demonstrated critical-path regressions.
7. **Rust-first:** Implement owned code in Rust wherever feasible. Restrict assembly and `unsafe` to small documented, reviewed, and preferably verified boundaries. Existing C or other-language components may be isolated behind stable service contracts when rewriting them would add risk.
8. **Stable, documented interfaces:** Versioned, machine-readable native system, service, and driver ABIs. Preserve backwards compatibility within published guarantees; do not expose unstable Rust layouts, raw kernel pointers, or internal structures.
9. **Direct UEFI boot:** Boot a UEFI PE/COFF image without requiring GRUB, shim, or a separate bootloader. Support optional platform Secure Boot chains as deployment policy requires.
10. **Independent signing authorities:** Firmware/image trust and administrator configuration trust are distinct; users must not need firmware or shim signing keys to authorize configuration.
11. **Cryptography outside the core:** Keep bulk cryptographic implementations in a separately versioned, stable cryptographic module hosted by an isolated service, with an explicitly defined FIPS 140-3 validation boundary when that deployment profile is pursued.
12. **Specification-led development:** Machine-checkable contracts, independent validation, formal proofs for selected properties, adversarial testing, and automated compatibility testing are first-class deliverables.

## 3. Target platform and scope

### Initial target

- Architecture: x86-64.
- First development profile: an Apple Silicon Mac running an x86-64 Q35/OVMF guest under QEMU TCG; UTM may be used as an optional interactive frontend.
- First accelerated profile: the same x86-64 Q35/OVMF guest and device model under QEMU with KVM acceleration on an x86-64 Linux host.
- First milestone workload: boot the kernel, seal the authority envelope, create one isolated domain, and start a minimal native control-plane process.
- First Linux compatibility profile, after the native boot/domain path is established: the repository-pinned `minimal-system-v0` x86-64 system-container test image with `/sbin/init` and `/dev/console`. Its signed candidate executes as PID 1, reports readiness, retains the console while blocked in an untimed supervisor wait, accepts an explicit native control-plane shutdown event, and exits cleanly. The bounded machine-readable M5 matrix is complete; broader Linux compatibility remains profile-driven work rather than an implied claim.
- Later hardware profile: one explicitly selected server with PCIe, IOMMU, NVMe, high-speed Ethernet/RDMA, and an NVIDIA GPU.
- Later workloads: SSH, package operations, OCI workloads, CUDA, PyTorch, distributed AI training/inference, and HPC applications.

### Explicitly deferred

SELinux enforcement, eBPF, netfilter compatibility, Linux kernel modules, advanced Linux tracing, nested container engines, broad consumer hardware, graphical desktops, arbitrary USB devices, printers, and comprehensive virtualization are not required for the initial release. The architecture should permit later support through isolated services and versioned interfaces without granting third-party kernel privilege.

## 4. System architecture

```text
+----------------------------------------------------------------+
| Minimal Linux system container | later distribution/app containers |
| PID 1 | console | later systemd, SSH, CUDA, PyTorch, and HPC        |
+----------------------------------------------------------------+
| Targeted Linux container compatibility                          |
| syscalls | procfs/sysfs/devfs | cgroups | namespaces | creds    |
+----------------------------------------------------------------+
| Native control plane and isolated service domains               |
| lifecycle | VFS/storage | network | devices | GPU | crypto      |
+----------------------------------------------------------------+
| Minimal immutable Rust kernel                                   |
| scheduling | VM protection | capabilities | IPC | interrupts   |
| IOMMU/DMA mediation | domain/resource enforcement               |
+----------------------------------------------------------------+
| UEFI firmware / virtual or physical hardware                    |
+----------------------------------------------------------------+
```

This is a **logical** architecture, not a requirement that every operation traverse each layer. Protected control paths establish authority; authorized data paths may use shared memory, mapped queues, batching, or narrowly justified in-kernel handling. The kernel/service boundary is decided subsystem by subsystem using security invariants and measurements rather than a pure microkernel or monolithic label.

### Native domains

A **domain** is the native unit of isolation, ownership, accounting, and failure containment. A domain owns processes/address spaces, a capability table, memory and CPU budgets, IPC endpoints, device grants, network endpoints, filesystem views, and failure policy. Linux containers are domains with Linux-compatible behavior; native control, storage, network, GPU, and crypto services are domains without a Linux ABI requirement.

The kernel creates exactly one initial userspace authority: the native control-plane domain. All other domains descend from authority it delegates. Process ancestry determines lifecycle and accounting, but never grants authority by itself.

### Native control plane

The control plane is the first userspace process tree and the only initial holder of the boot-authorized host capabilities. It:

- Starts, stops, inspects, and restarts native services and containers.
- Creates domain process trees and assigns CPU, memory, storage, network, and device resources.
- Delegates and revokes capabilities within the immutable boot authority envelope.
- Provides the native administrative API and CLI.
- Receives health, failure, resource, and audit events.
- Cannot load kernel code, modify kernel memory, weaken the active policy floor, introduce a new trust root, or synthesize authority not present in its parent capability set.

Administrative observation, lifecycle control, memory debugging, and authority delegation are distinct capabilities. Production policy may allow the control plane to enumerate and terminate every process without allowing it to read every container's memory or secrets.

### Kernel core responsibilities

- CPU/thread scheduling, context switching, interrupt and exception handling.
- Virtual memory, address-space isolation, and protection primitives.
- Capability creation, delegation, revocation, and enforcement.
- Low-overhead IPC, shared-memory grants, synchronization primitives.
- IOMMU, DMA, MMIO, interrupt routing, and device-access authorization.
- Domain lifecycle, resource accounting, and enforcement primitives.
- Minimal boot handoff and integrity of the immutable kernel and active authority envelope.

### Userspace service responsibilities

- Native control-plane orchestration and administrative interfaces.
- Targeted Linux container compatibility and virtual filesystems.
- Filesystem implementations, storage management, network protocols, and network policy.
- Device services for USB/input/printing and other noncritical peripherals.
- GPU driver domain, with constrained MMIO/DMA and controlled memory grants.
- Cryptographic services, key management, logging, orchestration, and administrative control.

### Device model

`/dev` exists inside Linux-compatible containers as a compatibility namespace. Nodes connect to authorized isolated device services; their implementations do not execute in the kernel. Compatibility must cover required `ioctl`, `mmap`, `poll`/`epoll`, event notification, permissions, and device-specific behavior, not merely filenames. GPU, NVMe, and RDMA control paths remain protected while authorized data paths minimize IPC and copying through shared memory and hardware queues. IOMMU-backed isolation and safe revocation remain mandatory.

### Storage and container images

The native host does not require a traditional distribution root filesystem. It needs authenticated boot artifacts, native service binaries, container images, persistent volumes, logs, and crash evidence.

- The first proof of concept may unpack a CPIO/initramfs into memory and use `tmpfs`, avoiding an early persistent-filesystem dependency.
- The first minimal system container should use a verified read-only root image plus explicitly writable temporary paths. Later distribution images add explicitly authorized persistent paths.
- A later isolated storage service should expose `virtio-blk` and then selected NVMe hardware through a stable block-device contract.
- The first M6 storage primitive is a versioned two-slot persistent-volume record with prepare/flush/commit/flush ordering, checksum validation, and fail-closed recovery. Shared `no_std` format and transaction crates support both the kernel reference backend and the measured native service. The Q35 bootstrap discovers one modern `virtio-blk` function, validates and maps only its bounded configuration regions, negotiates `VERSION_1` and `FLUSH`, and retains PCI configuration, MSI-X tables, the IDT, and local-APIC control in the kernel. Manifest object 7 is attenuated into a read-only observer grant and a separate write-only native volume-service grant. The freestanding Rust service is authenticated from the measured root as a bounded static PIE, relocated into W^X pages, and receives only a guarded stack, read-only bootstrap record, one generation-owned DMA page, the virtio common page, and the exact queue-notification page. It configures queue zero, constructs serialized read/write/flush chains, and blocks through a generation-bound capability call while a kernel-owned MSI-X route delivers completions. Its first request rejects a future generation, proves a real 100 ms APIC timeout does not consume the current generation, retries that generation successfully after notification, and rejects replay after completion. A fresh boot commits generation one to reserved sectors four and five; a second boot recovers the exact record without rewriting it. A separate corruption boot invalidates the native committed slot, requires authenticated fail-closed rejection without rewriting, and proves the device and DMA lease are cleaned up while the kernel continues booting. On its deliberate terminal fault, the kernel masks and disables MSI-X, revokes bus mastering, resets the device, retires the interrupt route, validates the stable DMA completion, restores its own root, scrubs and releases all service frames, and revokes the service capability without panicking. After complete retirement, a fresh domain generation with the same logical service identity receives newly delegated authority, newly owned mappings and DMA, reconfigures the reset device, and must recover the record or repeat fail-closed rejection before undergoing the same cleanup. The earlier embedded machine-code writer has been removed. Concurrent requests, persistent vector allocation, sub-page MMIO protection, physical-media power-loss validation, and IOMMU confinement remain future milestones.
- Existing filesystem implementations may be ported or wrapped inside isolated services, including non-Rust implementations where a rewrite would add risk. Kernel memory safety and service isolation must not depend on the filesystem implementation language.
- Container image layers should be content-addressed and read-only; per-container writable overlays and persistent volumes are separate, explicitly authorized objects.
- Filesystem-service crashes must not panic the kernel. Recovery guarantees for mounted domains and crash consistency must be specified per filesystem.

### Initial Linux compatibility profile

The initial goal is a repository-pinned minimal x86-64 system-container image reaching a PID-1-managed console. Required early behavior includes x86-64 ELF loading, dynamic linking where the chosen image requires it, signals, futexes, clocks, process lifecycle, `tmpfs`, console devices, and the bounded `/proc`, `/sys`, `/dev`, credential, and namespace behavior exercised by that image. The exact image digest and machine-readable feature matrix are release inputs. A later distribution profile adds systemd, glibc, package management, sockets, cgroups v2, and the broader namespace behavior needed by Rocky Linux or another selected distribution. Unsupported interfaces must fail predictably rather than silently provide weaker semantics.

## 5. ABI and binary component contracts

### 5.1 Three distinct interfaces

1. **Linux container ABI:** Syscalls and observable Linux behavior required by the pinned minimal system-container profile and later distribution/application profiles.
2. **Native system ABI:** Stable, language-neutral binary contract between the kernel and native control/service domains. It does not permit arbitrary third-party kernel execution.
3. **Service/driver ABI:** Stable IPC, capabilities, shared-memory, resource, lifecycle, failure, and device contracts for isolated components.

### 5.2 Native system and driver ABI requirements

- ELF-based native service format; define exact loading model, calling conventions, entrypoints, alignment, lifecycle, and version negotiation.
- ABI-defined fixed-width structures, opaque handles, explicit ownership and error semantics; no dependency on Rust compiler-private ABI.
- C-compatible ABI surfaces where appropriate, allowing services and drivers written in Rust, C, C++, Zig, assembly, or other languages.
- Machine-readable interface definitions, generated bindings, conformance suites, and ABI regression checks.
- Minimum and target ABI versions, feature-bit negotiation, runtime feature queries, and compatibility adapters where necessary.
- A binary compiled against a supported stable native ABI should continue to function on later conforming kernel releases, subject to documented feature negotiation and lifecycle constraints.
- Interfaces must specify cancellation, timeouts, restart, resource cleanup, revocation, and behavior when either endpoint fails.
- No native ABI exposes raw kernel pointers, unstable internal structures, or authority derived from process ancestry.
- Third-party components are authenticated, isolated, and capability-limited by policy; no third-party binary executes in the kernel address space.

### 5.3 Linux `.ko` compatibility

Linux `.ko` files depend on Linux-specific exported symbols, data layouts, relocations, build configuration, versioning, and internal semantics. Ginux does not load them into its kernel. Relevant drivers may be source-ported into isolated native driver domains or adapted behind a bounded compatibility library. Unmodified Linux `.ko` loading is not a v0 acceptance criterion.

### 5.4 NVIDIA priority

Early feasibility must establish whether NVIDIA's available open kernel-module sources can be ported into an isolated driver domain while retaining matching, unmodified userspace CUDA components. Evaluate firmware dependencies, memory management, GPU faults, unified and pinned memory, MMU/IOMMU interactions, interrupts, command submission, peer memory, licensing, and supportability. The NVIDIA userspace and driver-domain components may update together; the stable Ginux driver ABI must allow the kernel to update independently. An **actual CUDA workload on a selected GPU** is the hardware go/no-go milestone. Do not claim compatibility before it is demonstrated.

## 6. Security and isolation

- Capability-based resource authority with explicit delegation, attenuation, generation-safe handles, and revocation.
- Every runtime authority is created by the boot manifest or derived as a subset of existing authority; runtime operations cannot synthesize greater authority.
- Independent address spaces for drivers/services; no implicit root-equivalent access to hardware.
- IOMMU-backed DMA containment, controlled page pinning, and bounded shared memory.
- Service restart/failure containment where hardware and semantics permit.
- W^X enforcement; kernel code and immutable data become read-only after initialization; no runtime kernel module loader.
- Signed or measured native services and verified read-only system artifacts, with mutable data kept in explicit stores or volumes.
- Namespaces, cgroups, seccomp, and Linux capabilities implemented to the extent required by the selected Linux container profile; map them to stronger native primitives where possible. SELinux enforcement is deferred from the first profile.
- UID 0 is superuser only inside its Linux container. The native host has no UID-based omnipotent principal; host administration requires explicit capabilities.
- Separate code signing, authorization, runtime integrity, and attestation concepts.
- Threat model must distinguish attacks that are prevented, detected, contained, recoverable, accepted, or out of scope. It must cover a compromised control plane or service, compromised container root, hostile device input, firmware, DMA, side channels, denial of service, and boot/configuration tampering.

**Security caveat:** The control plane is highly trusted but not omnipotent. Its compromise may expose or destroy resources it is authorized to control, but the kernel must still prevent kernel modification and authority expansion beyond the active boot envelope. Any future unrestricted ring-0 extension would join the trusted computing base and invalidate these assumptions.

## 7. Boot and configuration

### 7.1 Direct EFI boot

Build a UEFI PE/COFF executable with a minimal Rust/assembly EFI entry path. Obtain firmware memory map and platform metadata, authenticate and validate the boot manifest and signed native system bundle, establish required resources, call `ExitBootServices()`, then enter the kernel. The kernel initializes its minimum mechanisms, seals executable integrity and the authority envelope, and creates the initial control-plane domain. Do not require a separate bootloader, though deployments may choose a shim or other chain for their Secure Boot policy.

### 7.2 Boot sequence

1. UEFI loads the authenticated Ginux image.
2. Boot code verifies and activates the boot manifest and native system bundle.
3. The kernel initializes memory, scheduling, interrupts, IPC, and required IOMMU/device mediation.
4. Kernel code and immutable data are sealed read-only and the boot authority envelope becomes fixed.
5. The kernel creates the initial control-plane domain with exactly the capabilities authorized by the manifest.
6. The control plane starts approved storage, network, device, logging, and compatibility services.
7. Storage exposes the pinned minimal Linux system-container image.
8. The control plane creates its system-container domain and starts the image's minimal PID 1.
9. The administrative API reports health and readiness.

### 7.3 Configuration lifecycle

```text
edit config at runtime
       -> validate
       -> authorize/sign or otherwise authenticate
       -> stage atomically as pending
       -> reboot
       -> verify trust, version, rollback policy and compatibility
       -> activate immutable configuration
       -> boot services and Linux userspace
```

- Human-readable source (e.g. `/etc/<project>/boot.toml`) may be edited while running.
- A staging utility generates a canonical manifest, validates it, and authorizes it under a **separate configuration trust root**.
- Kernel and firmware/shim signing keys are **not** required for administrator configuration changes.
- Boot code accepts only trusted, complete, consistent manifests and passes a compact typed immutable configuration and initial capability graph to the kernel.
- Support atomic A/B boot artifacts, health checks, recovery and rollback; security policy determines whether older signed states may be restored.
- Active kernel configuration, trust roots, approved host code, security-policy floor, device ceiling, and maximum authority cannot be modified until reboot.
- Runtime operations such as container lifecycle, resource allocation, IP addresses, routes, mounts, and device grants remain dynamic when authorized within the fixed envelope. Runtime policy may reduce authority but cannot expand the boot ceiling.

### 7.4 Trust choices

Support deployer-managed public-key verification and optional TPM-backed key protection, measurements, sealed secrets, and anti-rollback. TPM is not mandatory. A unified signed boot artifact is desirable but not mandatory; the trust model must work when configuration is updated independently of the EFI image. Separate signing identities and delegation policies for firmware, OS images, native service bundles, Linux container images, and site configuration.

## 8. Cryptography and FIPS boundary

**Architectural objective:** Keep cryptographic implementations in a separately versioned, stable crypto module or isolated service so kernel releases can evolve without routinely changing the cryptographic module's validated boundary or implementation.

- Define the cryptographic module boundary, operational environment, approved algorithms, interfaces, integrity tests, self-tests, key lifecycle, and approved-mode behavior early.
- Keep the crypto module's binary, service ABI, dependencies, and execution assumptions stable across ordinary kernel updates where feasible.
- Distinguish crypto **implementation** (outside the core) from enforcement of isolation and access rights (core responsibility).
- Bootstrap trust is unavoidable: the EFI entry/early boot path must authenticate configuration and code before it can rely on the runtime crypto module. Document the bootstrap verifier and its own assurance/validation scope.
- Determine the isolated crypto-service design that best supports performance, key protection, stable interfaces, and a defensible FIPS 140-3 boundary without expanding the kernel trusted computing base.
- **Validation qualification:** Module separation can reduce the frequency and scope of changes affecting a FIPS certificate, but does not automatically exempt kernel/operational-environment changes from CMVP change assessment or applicable retesting. Preserve evidence for the specific validated configuration and changes.

## 9. Performance model

- Optimize the **measured end-to-end path**, not kernel LOC or IPC count alone.
- Establish permissions in a protected control path, then use direct authorized shared-memory/device-queue data paths when safe.
- Avoid repeated copies, unnecessary context switches, global locks, and scheduler jitter.
- Optimize NUMA locality, CPU/GPU affinity, pinned memory, PCIe topology, NVMe, and RDMA.
- Keep hardware-specific optimization profiles configurable at **next boot** rather than requiring unique source trees.
- Treat no regression against a named Linux baseline as the north star, not an unmeasurable universal assertion. Define gates per workload and path.
- Benchmark against the selected Linux baseline on the identical QEMU/KVM or physical profile. Initial measurements include context-switch and IPC latency, system-container boot time, container startup, TCP throughput/p99 latency, block throughput/p99 latency, CPU-bound overhead, memory overhead, and later GPU throughput/utilization. Rocky Linux may become a later distribution baseline once that profile is selected.

## 10. Verification and agent-assisted development

### Verification targets

- Formal specifications for memory isolation, capability safety, IPC, resource ownership, DMA authorization, and boot/runtime authority transitions.
- First end-to-end invariant: every runtime capability is authorized by the boot manifest or derived as a strict subset of an existing capability; no runtime operation can expand the authority envelope.
- Early DMA invariant: a device cannot access a physical page without an explicit authorized grant, and revocation removes access before the page may be reused.
- Proofs for selected critical components and invariants using suitable Rust verification tools (e.g. Verus/Kani) and protocol models (e.g. TLA+); choose tooling after feasibility experiments.
- Unsafe Rust inventory with documented invariants and review requirements.
- Differential syscall and behavioral testing against Linux; fuzzing of parsers, syscall interfaces, IPC, device boundaries, and compatibility services.
- Reproducible builds, signed artifacts, supply-chain provenance, and continuous ABI checks.

**No claim of a universally bug-free system.** Proofs apply to explicit specifications and assumptions; hardware, firmware, compilers, external modules, and unverified services remain relevant.

### Agent workflow

- Architecture/specification agents define contracts and testable acceptance criteria.
- Implementation agents work on isolated modules against those contracts.
- Independent verification/adversarial agents generate counterexamples, fuzz, review `unsafe`, and check ABI drift.
- Integration agents run QEMU, hardware-in-the-loop, pinned Linux-profile compatibility, and performance suites.
- Human maintainers approve threat model, interfaces, trust roots, exceptions, and merge decisions.

No agent-authored change merges solely on the author agent's tests or self-reported proof. Keep proofs, test logs, benchmarks, and compatibility evidence as CI artifacts.

## 11. Milestones and acceptance criteria

| Stage | Demonstrable result | Gate |
|---|---|---|
| M0: Feasibility/spec | Threat model, domain/capability model, ABI sketches, minimal Linux compatibility inventory, NVIDIA driver-port study | Fix the authority model, initial service boundary, and QEMU/KVM profile |
| M1: Immutable boot | Direct UEFI boot under QEMU/KVM; verified manifest and bundle create one control-plane domain | Kernel/integrity sealing and verified authority-envelope transition |
| M2: Domains and control | Rust scheduling, VM, IPC, capabilities, process trees, resource accounting, interrupts, control API | Isolation/concurrency tests and authority non-escalation property |
| M3: Native services | Console, in-memory storage, network/device skeletons, service supervision | Service failure cannot panic kernel; capability cleanup/restart tests |
| M4: Linux baseline | Run unmodified static and dynamic Linux binaries inside a Linux-compatible domain | Targeted syscall, process, VFS, and credential tests |
| M5: Minimal system container | Boot the pinned minimal Linux image to a PID-1-managed console | Required process, mount, credential, procfs/devfs, and lifecycle matrix passes |
| M6: Persistence and network | The minimal system container uses persistent storage and native virtual networking | Crash consistency, restart, throughput, p99 latency, and isolation tests |
| M7: OCI workloads | Run selected OCI application containers under native control | Image, overlay, resource, seccomp, and networking tests |
| M8: GPU | Run selected NVIDIA GPU + CUDA/PyTorch workload through isolated driver domain | Driver failure containment, correctness, and matched throughput benchmark |
| M9: Hardening | Formal proof portfolio, fuzzing, attestation, ABI stability checks | Security review and reproducible release |

Stages are conceptual and may overlap. **M0 NVIDIA feasibility begins immediately**, while the actual hardware/CUDA gate remains later, because its memory and driver requirements can change the architecture before substantial implementation investment.

## 12. Open design decisions

1. Which operations, if any, require an in-kernel fast path rather than shared-memory IPC to an isolated service?
2. What exact objects, operations, failure semantics, version negotiation, and compatibility guarantees make up the stable native system/driver ABI?
3. Which control-plane observation and debugging capabilities are enabled in production, particularly access to container memory and secrets?
4. Can NVIDIA open kernel-module sources be adapted into an isolated driver domain without losing required functionality/performance?
5. What exact digest and construction recipe define the first minimal Linux system-container image, and which observable interfaces does it exercise? Which later distribution image becomes the first broad compatibility profile?
6. Which existing filesystem implementation and on-disk format should back the first persistent storage service?
7. What bootstrap cryptography is necessary before the separately maintained runtime crypto module is available?
8. Which configurations and operational environments are in scope for future FIPS validation?
9. What Linux baseline and acceptable performance regression limits apply to each named workload?
10. Which failures can be transparently recovered, which require restarting a container/domain, and which require reboot?
11. What is the governance model and release/support policy?

## 13. Suggested repository layout

```text
/
  README.md
  ARCHITECTURE.md
  docs/
    threat-model.md
    boot-trust.md
    abi/
    linux-compatibility.md
    fips-boundary.md
    verification.md
    performance.md
    decisions/
  specs/
    kernel/
    domains/
    ipc/
    capabilities/
    abi/
  boot/
  kernel/
  control-plane/
  services/
    linux-abi/
    storage/
    network/
    devices/
    gpu/
    crypto/
  drivers/
  tools/
  tests/
    qemu/
    rocky-container/
    abi/
    security/
    performance/
  .github/workflows/
```

## 14. First Codex tasks

1. Record the native domain/control-plane model, immutable authority envelope, capability derivation rules, and explicit absence of third-party kernel execution in an ADR.
2. Specify the stable native system/driver ABI, including object lifetimes, version/feature negotiation, cancellation, revocation, cleanup, restart, and failure semantics.
3. Specify the boot manifest, signed native bundle, independent trust authorities, atomic staging, rollback, immutable integrity state, and initial capability graph.
4. Build a tiny Rust UEFI proof of concept under the pinned QEMU/KVM profile that seals the boot state and creates one minimal control-plane domain.
5. Pin a minimal x86-64 system-container test image and build a machine-readable syscall/feature/observable-behavior matrix for reaching its PID-1-managed console.
6. Specify and test the end-to-end authority non-escalation invariant.
7. Produce an evidence-backed NVIDIA driver feasibility report distinguishing the Linux interface layer, isolated native port, matched userspace components, and required memory/device primitives.
8. Choose the initial in-memory and persistent storage paths, including service-crash and consistency behavior.

**Engineering discipline:** Do not silently relax compatibility, security, or performance requirements. Record tradeoffs in architecture decision records (ADRs), including measured evidence and proof implications.
