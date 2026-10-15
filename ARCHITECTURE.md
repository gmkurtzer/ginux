# Architecture: An Immutable, Container-Native OS for AI Infrastructure

**Status:** Initial design proposal (v0.4)
**Purpose:** Foundational specification for a Git repository and agent-assisted engineering.
**Scope:** Architecture and requirements, not a claim of implementation or proven feasibility.

## 1. Vision

Build a new, Rust-first operating system that boots directly via UEFI and acts as a small, immutable, capability-secure container host for AI/HPC servers. One authenticated `GINUX.EFI` contains the kernel, boot-required modules, a statically linked Linux-ABI controller payload, recovery support, and a component catalog. The kernel launches the controller as the first ring-3 process; that controller starts Linux-compatible system containers and the few optional native services that remain outside the kernel. There is no conventional host distribution or host root filesystem.

The product acceptance benchmark is an unmodified Rocky Linux 10 x86-64 userspace booting `/sbin/init` as PID 1 inside a system container and behaving like a normal independently networked system. Smaller fixtures may prove individual interfaces, but they are test scaffolding rather than the product environment. Linux compatibility is a container interface; the native host is not a Linux distribution.

**North star:** A small, verifiable kernel and boot-required module set; one embedded but ring-3 Linux-ABI controller; normal Linux system containers; a stable native kernel/driver ABI; immutable executable integrity and hard authority boundaries; and no unnecessary cycles in critical data paths.

The design is intentionally narrower than Linux in hardware support. A binary compiled for a supported stable native system or driver ABI must remain compatible within its specified contract. Binary Linux `.ko` compatibility is not a proof-of-concept requirement, but the architecture must not preclude a future authenticated compatibility layer for selected modules.

## 2. Non-negotiable design principles

1. **Container-native Linux compatibility:** Boot an unmodified Rocky Linux 10 x86-64 userspace as a system container, with systemd as PID 1, its own kernel-object views, and its own network identity. Expand from that named profile to package management, SSH, OCI workloads, NVIDIA/CUDA, and AI/HPC applications.
2. **Purpose-built privileged core:** Scheduling, memory protection, capabilities, container kernel objects, boot-critical storage/filesystems, networking, and primary device support belong in the kernel or authenticated kernel modules. Nonessential and independently recoverable components may run in isolated native domains.
3. **One initial authority:** The kernel launches one embedded controller payload as the initial ring-3 authority. It may delegate or revoke authority, but it cannot create authority beyond the immutable boot envelope.
4. **Security by construction:** Explicit capabilities, no ambient host authority, hardware-enforced memory and DMA isolation, least privilege, revocation, and defense in depth. Process trees define ownership and lifecycle; capability graphs define authority. Linux namespaces are a compatibility interface, not the native security boundary.
5. **Runtime immutability:** Kernel and module executable pages, embedded controller bytes, policy, and read-only data cannot be replaced or made writable at runtime. The authenticated boot configuration fixes the kernel/module set for that boot. Runtime state may change only through authorized interfaces and within the boot authority envelope.
6. **Performance by measurement:** Favor direct authorized data paths, shared memory, batching, and hardware queues; isolate control paths. No architectural purity at the expense of demonstrated critical-path regressions.
7. **Rust-first:** Implement owned code in Rust wherever feasible. Restrict assembly and `unsafe` to small documented, reviewed, and preferably verified boundaries. Existing C or other-language components may be isolated behind stable service contracts when rewriting them would add risk.
8. **Stable, documented interfaces:** A Windows-like stable kABI plus versioned, machine-readable native system, service, and driver contracts. Preserve backwards compatibility within published guarantees; do not expose unstable Rust layouts, raw kernel pointers, or unversioned internal structures.
9. **Direct UEFI boot:** Boot a UEFI PE/COFF image without requiring GRUB, shim, or a separate bootloader. Support optional platform Secure Boot chains as deployment policy requires.
10. **Independent signing authorities:** Firmware/image trust and administrator configuration trust are distinct; users must not need firmware or shim signing keys to authorize configuration.
11. **Cryptography outside the core:** Keep bulk cryptographic implementations in a separately versioned, stable cryptographic module hosted by an isolated service, with an explicitly defined FIPS 140-3 validation boundary when that deployment profile is pursued.
12. **Specification-led development:** Machine-checkable contracts, independent validation, formal proofs for selected properties, adversarial testing, and automated compatibility testing are first-class deliverables.

## 3. Target platform and scope

### Initial target

- Architecture: x86-64.
- First development profile: an Apple Silicon Mac running an x86-64 Q35/OVMF guest with QEMU's Intel IOMMU exposed under QEMU TCG; UTM may be used as an optional interactive frontend.
- First accelerated profile: the same x86-64 Q35/OVMF guest and device model under QEMU with KVM acceleration on an x86-64 Linux host.
- First interactive checkpoint: boot the unified image and interact with the embedded ring-3 controller through its console.
- First product Linux profile: Rocky Linux 10 x86-64, booted as a system container with `/sbin/init` (systemd) as PID 1. The repository-pinned `minimal-system-v0` image remains an implementation fixture only.
- Later hardware profile: one explicitly selected server with PCIe, IOMMU, NVMe, high-speed Ethernet/RDMA, and an NVIDIA GPU.
- Later workloads: SSH, package operations, OCI workloads, CUDA, PyTorch, distributed AI training/inference, and HPC applications.

### Explicitly deferred

SELinux enforcement, eBPF, netfilter compatibility, binary Linux `.ko` loading, advanced Linux tracing, nested container engines, broad consumer hardware, graphical desktops, arbitrary USB devices, printers, and comprehensive virtualization are not required for the proof of concept. Their deferral must not close the stable extension points needed for later support.

## 4. System architecture

```text
+----------------------------------------------------------------+
| Rocky Linux system container | additional distribution/app containers |
| systemd PID 1 | console | later SSH, CUDA, PyTorch, and HPC           |
+----------------------------------------------------------------+
| Targeted Linux container compatibility                          |
| syscalls | procfs/sysfs/devfs | cgroups | namespaces | creds    |
+----------------------------------------------------------------+
| Embedded ring-3 controller and optional isolated domains        |
| lifecycle | configuration | monitoring | nonessential drivers  |
+----------------------------------------------------------------+
| Minimal immutable Rust kernel                                   |
| scheduling | VM | capabilities | container objects | IPC       |
| boot storage/filesystems | network | primary devices | IOMMU    |
+----------------------------------------------------------------+
| UEFI firmware / virtual or physical hardware                    |
+----------------------------------------------------------------+
```

This is a **logical** architecture, not a requirement that every operation traverse each layer. Protected control paths establish authority; authorized data paths may use shared memory, mapped queues, batching, or narrowly justified in-kernel handling. The kernel/service boundary is decided subsystem by subsystem using security invariants and measurements rather than a pure microkernel or monolithic label.

### Native domains

A **domain** is the native unit of isolation, ownership, accounting, and failure containment. A domain owns processes/address spaces, a capability table, memory and CPU budgets, IPC endpoints, device grants, network endpoints, filesystem views, and failure policy. Linux system containers are domains with Linux-compatible behavior. The controller is a statically linked Linux-ABI process; optional isolated services may retain their narrower native service ABI.

The kernel maintains one global host PID namespace spanning native userspace
processes and Linux-compatible processes in every container. The controller's
capability-gated `/proc` view exposes that complete namespace. Each container
receives a filtered PID-namespace projection with its own init as PID 1; a
container-local PID therefore maps to a distinct host PID. Domain generations
remain internal isolation identities and are never presented as PIDs.

The kernel creates exactly one initial userspace authority: the native control-plane domain. All other domains descend from authority it delegates. Process ancestry determines lifecycle and accounting, but never grants authority by itself.

### Embedded Linux-ABI controller

The controller is packaged inside `GINUX.EFI` but executes in ring 3 as a statically linked Linux-ABI userspace process and the only initial holder of boot-authorized host capabilities. It uses ordinary Linux `read`, `write`, and `exit` syscalls. Ginux-specific management is exposed through a versioned device/ioctl UAPI rather than a private syscall convention. Its authenticated local configuration declares which containers start automatically, their image and resource grants, and the init path (normally `/sbin/init`). It:

- Starts, stops, inspects, and restarts native services and containers.
- Creates domain process trees and assigns CPU, memory, storage, network, and device resources.
- Delegates and revokes capabilities within the immutable boot authority envelope.
- Provides the native administrative API and a command-tree CLI with generated
  help, usage, nested dispatch, and argument validation.
- Receives health, failure, resource, and audit events.
- Cannot modify the sealed kernel/module set, kernel memory, the active policy floor, trust roots, or synthesize authority not present in its parent capability set.

A later management service may supply configuration remotely through mutually authenticated host-to-host trust, including desktop clients. Remote policy cannot exceed the locally sealed authority envelope.

Administrative observation, lifecycle control, memory debugging, and authority delegation are distinct capabilities. Production policy may allow the control plane to enumerate and terminate every process without allowing it to read every container's memory or secrets.

### Kernel core responsibilities

- CPU/thread scheduling, context switching, interrupt and exception handling.
- Virtual memory, address-space isolation, and protection primitives.
- Capability creation, delegation, revocation, and enforcement.
- Low-overhead IPC, shared-memory grants, synchronization primitives.
- IOMMU, DMA, MMIO, interrupt routing, and device-access authorization.
- Domain lifecycle, resource accounting, and enforcement primitives.
- Container-local PID, mount, user, IPC, cgroup, time, hostname, device, and network object views with stronger isolation than current Linux namespaces.
- Per-container loopback, interfaces, IP addresses, routes, sockets, and network-policy enforcement.
- Globally unique virtual interfaces use `gnetN` names. The same name is exposed to the control plane and inside the assigned domain; interfaces are never implicitly renamed at a domain boundary. Device listings show the plain domain name without a type prefix.
- Control-domain and per-domain network state includes DNS resolver configuration. DHCP imports DNS configuration when supplied and otherwise preserves an explicit resolver, while static configuration may set it directly.
- Every domain receives an always-up, immutable `lo` interface with `127.0.0.1/8`; operators do not create or configure loopback. The control domain attempts DHCP on its primary physical interface at boot and remains manageable for manual configuration if no lease is available.
- Boot-critical storage, filesystem, networking, GPU/primary-device support in the kernel or authenticated kernel modules.
- Boot handoff and integrity of the immutable kernel, module set, controller, and active authority envelope.

### Userspace service responsibilities

- Native control-plane orchestration and administrative interfaces.
- Authenticated local configuration, monitoring, maintenance, audit collection, and optional remote management.
- Device services for USB/input/printing and other noncritical peripherals.
- Optional or experimental drivers whose failure can be contained without preventing boot.
- Cryptographic services, key management, logging, orchestration, and administrative control.

### Device model

`/dev` exists inside Linux-compatible containers as a per-container compatibility view. Nodes connect to kernel drivers, authenticated kernel modules, or explicitly authorized isolated device domains according to the device's boot and performance requirements. Compatibility must cover required `ioctl`, `mmap`, `poll`/`epoll`, event notification, permissions, and device-specific behavior, not merely filenames. GPU, NVMe, and RDMA control paths remain protected while authorized data paths minimize IPC and copying through shared memory and hardware queues. IOMMU-backed isolation and safe revocation remain mandatory.

The preferred extension order is: stable native kABI module; source adaptation against that kABI; isolated userspace driver for nonessential hardware; and, later, a bounded Linux `.ko` compatibility layer. Modules are authenticated and selected before the runtime integrity seal. Arbitrary post-seal module insertion is incompatible with runtime immutability.

### Storage and container images

The native host does not require a traditional distribution root filesystem. It needs authenticated boot artifacts, native service binaries, container images, persistent volumes, logs, and crash evidence.

- The native boot path does not require a separate initramfs. Boot-required code and controller payloads are sections of the authenticated EFI image. CPIO remains acceptable only as a Linux-container test or image format.
- The first minimal system container should use a verified read-only root image plus explicitly writable temporary paths. Later distribution images add explicitly authorized persistent paths.
- C3 container configuration and OCI image storage are deliberately boot-local. The controller keeps configuration in memory and stores verified, content-addressed image objects in a bounded RAM-backed tmpfs. `container start NAME` automatically pulls a configured image when its digest is absent, streams it through bounded staging while verifying every digest, and atomically publishes only the complete immutable object. Failed or partial pulls are discarded, and all configuration, cached images, writable overlays, and temporary data disappear on host reboot.
- The current single-file tmpfs proof is not the C3 image store. C3 must extend it into a quota-enforced multi-object store, avoid unnecessary whole-image copies, and give the QEMU profile an explicit memory budget large enough for the pinned Rocky image and kernel. Exhaustion must fail the initiating operation without destabilizing the host or other domains.
- Persistence is a later controller-managed transition. The controller will be able to select and initialize a local volume, copy and verify boot-local state into it, stage that volume as the pending persistent-state source, and request a reboot. The next boot authenticates the selection and validates the volume before activation; an incomplete or invalid activation fails closed to the existing boot path. A mutable data volume must never silently replace the authenticated kernel, embedded controller, or sealed boot authority envelope.
- Boot storage is exposed by a kernel driver or authenticated kernel module through a stable block-device contract, beginning with `virtio-blk` and later selected NVMe hardware.
- The first M6 storage primitive is a versioned two-slot persistent-volume record with prepare/flush/commit/flush ordering, checksum validation, and fail-closed recovery. Shared `no_std` format and transaction crates support both the kernel reference backend and the measured native service. The Q35 bootstrap discovers one modern `virtio-blk` function, validates and maps only its bounded configuration regions, negotiates `VERSION_1`, `ACCESS_PLATFORM`, and `FLUSH`, and retains PCI configuration, MSI-X tables, the IDT, and local-APIC control in the kernel. Manifest object 7 is attenuated into a read-only observer grant and a separate write-only native volume-service grant. The freestanding Rust service is authenticated from the measured root as a bounded static PIE, relocated into W^X pages, and receives only a guarded stack, read-only bootstrap record, one generation-owned DMA page, the virtio common page, and the exact queue-notification page. That bootstrap record is native ABI v0: a shared, language-neutral, fixed 64-byte structure with magic, version, size, exact feature bits, device-visible DMA address, opaque capability, and reserved-zero validation. It configures queue zero, constructs serialized read/write/flush chains, and blocks through a generation-bound capability call while a kernel-owned MSI-X route delivers completions. Its first request rejects a future generation, proves a real 100 ms APIC timeout does not consume the current generation, retries that generation successfully after notification, and rejects replay after completion. A fresh boot commits generation one to reserved sectors four and five; a second boot recovers the exact record without rewriting it. A separate corruption boot invalidates the native committed slot, requires authenticated fail-closed rejection without rewriting, and proves the device and DMA lease are cleaned up while the kernel continues booting. The service reports `COMPLETED` or `REJECTED` through native lifecycle ABI v0 instead of encoding success as a deliberate fault. On clean exit, the kernel masks and disables MSI-X, revokes bus mastering, resets the device, retires the interrupt route, disables and retires the service's IOMMU translation, validates stable DMA state, restores its own root, scrubs and releases all service frames, and revokes the service capability before publishing retirement. Separate invalid-opcode and compiled device-service `HLT` proofs retain genuine fault containment and metadata coverage; the latter faults with an incomplete notified request, so fault teardown resets hardware and retires the IOMMU domain before scrubbing DMA without trusting queue completion. After complete retirement, a fresh domain generation with the same logical service identity receives newly delegated authority, newly owned mappings, DMA, and translation-table leases, reconfigures the reset device, and must recover the record or repeat fail-closed rejection before undergoing the same cleanup. The earlier embedded machine-code writer has been removed. Concurrent requests, persistent vector allocation, sub-page MMIO protection, physical-media power-loss validation, and multi-device IOMMU domains remain future milestones.
- The Q35 bootstrap also validates one ACPI DMAR remapping unit and maps exactly one VT-d register page supervisor-only, writable, uncached, and non-executable. It probes the version and capabilities while requiring an initially disabled, zero-root state. Because the QEMU unit does not set `INCLUDE_PCI_ALL`, the kernel validates its bounded device scopes and requires an exact endpoint scope for the discovered virtio-blk source ID. Six owned, zeroed frames encode an exact legacy root/context/four-level hierarchy for a single identity-mapped DMA page. The QEMU device is configured for platform DMA and must negotiate `VIRTIO_F_ACCESS_PLATFORM`; otherwise it would use a bypass address space. The kernel first directs a read payload at a separate sentinel-filled page outside the hierarchy, requires that page to remain unchanged, validates the exact VT-d source ID, address, and write-permission fault record, clears the record, invalidates caches, and then completes the real read/write/flush transaction through the permitted page. Each isolated native volume-service generation subsequently receives a newly allocated hierarchy for its exact generation-owned DMA page before bus mastering is enabled. Clean exit, pre-activation ABI rejection, driver fault, notified-timeout cancellation, restart, recovery, and corrupt-media rejection all require device quiescence before translation is disabled and its tables are scrubbed; the DMA page is released last. Consecutive generations must have distinct generation-tagged DMA and translation-table leases. Multi-device domains and physical-hardware validation remain later M6 milestones.
- The first M6 network-hardware slice exposes one modern-only QEMU virtio-net function at `00:03.0` with platform DMA required, a pinned MAC, and a restricted user-network backend. The kernel discovers exactly one matching function, validates its copied modern-transport and disabled MSI-X capability chain, rejects malformed copies, and requires an exact DMAR endpoint scope. OVMF's PCI authority is revoked before physical-memory initialization. Common, device, notification, and MSI-X spans are separately bounded supervisor-only, uncached, NX mappings without cacheable aliases; together with block and VT-d they form the exact seven-span sealed MMIO table. The kernel accepts exactly `VERSION_1`, `ACCESS_PLATFORM`, `MAC`, and `MRG_RXBUF`, validates the stable MAC and queue geometry, and uses one fresh DMA page plus six fresh VT-d table leases encoding only source `00:03.0` and that page. VT-d becomes active before queue authority and bus mastering is enabled last.

  The first transmit proof publishes one 12-byte modern header plus a fixed 19-byte Ethernet probe on queue 1, routes MSI-X entry 1 to vector 35, and requires exactly one interrupt, one used-ring completion, unchanged payload, and intact guards. The receive proof then publishes one 1535-byte write-only queue-0 descriptor and a padded ARP request to QEMU's restricted gateway. After the isolated TX completion, vector 34 must deliver exactly one RX interrupt and one completion. The kernel validates `num_buffers == 1`, exact gateway and guest MAC/IP identities, ARP reply fields, used-length bounds, and canaries. A negative receive redirects the descriptor to an owned sentinel page absent from the active hierarchy, requires the page to remain unchanged, validates the exact `00:03.0` VT-d write-permission fault and address, clears the fault, and then proves recovery with a valid ARP exchange. Each proof masks and disables MSI-X, revokes bus mastering, resets the device, and retires its temporary IDT routes before VT-d tables and DMA are scrubbed. A separate freestanding Rust network-service PIE now builds against native control and lifecycle ABI v4; its exact bytes are authenticated in the signed root and its bounded ELF plan, relative relocations, W^X mappings, guarded stack, and complete temporary retirement are verified at boot. The measured entry executes at ring 3 in a capability-empty domain, validates control ABI v4, exits through lifecycle ABI v0, and leaves no frames live. A separate fixed 80-byte network bootstrap ABI defines exact common-configuration, RX/TX notification, DMA, generation-capability, and MAC fields without exposing PCI, interrupt-controller, or IOMMU internals. A second measured generation validates that record and its read-only logical network capability while PCI decoding, bus mastering, MSI-X, and VT-d remain disabled, then scrubs every owned frame. An otherwise valid record with a nonzero reserved field is rejected in a separate contained generation before device activation; its capability, mappings, and DMA are retired. A further valid generation receives an active, exact VT-d hierarchy for its sole DMA page while PCI decoding and bus mastering remain disabled; lifecycle teardown disables and scrubs translation before releasing DMA. A faulted translated generation proves the same ordering after malformed bootstrap rejection. The next read/write generation negotiates the device and binds zeroed eight-entry RX/TX rings itself; a capability call lets the kernel validate the exact empty layout and active translation before enabling bus mastering last. Clean exit resets and disables the NIC before retiring VT-d and DMA. A separate kernel-injected failed activation return occurs only after the device is active, causing the compiled service's normal fatal path; fault teardown proves live queues and bus mastering are quiesced before VT-d and DMA retirement. A fresh generation then binds TX queue 1 to MSI-X entry 1, publishes one fixed transmit descriptor, notifies queue 1, and blocks through a generation-bound native wait. Kernel-owned vector 35 wakes only the exact domain, object, and request; replay is rejected, and teardown retires the route before VT-d and DMA. Receive ownership, protocol handling, and container networking remain next.
  Bootstrap feature `RX_REUSE` advances both rings through a second ARP exchange without resetting the device. An additive 88-byte bootstrap v1 then grants the measured driver a distinct write-only ingress-packet capability. After validating the second reply, it copies exactly 42 Ethernet bytes into a bounded kernel-owned FIFO and retires all NIC, interrupt, translation, DMA, and producer authority. A separately measured freestanding Rust protocol-service PIE initially receives only ingress-packet read authority through a fixed 56-byte bootstrap record. It has no device, DMA, MMIO, interrupt, or ingress-write authority; after driver retirement it consumes the exact sequence, validates Ethernet and ARP endpoint fields, rejects a locally malformed opcode, proves replay denial, and exits with complete capability and frame reclamation. Other fresh read-only ring-3 consumers reject future or stale sequences without mutation and receive exact frames in order. The four-entry FIFO preserves packets across producer replacement, rejects a fifth publication while full, reuses drained entries through wraparound, and retains monotonic sequence allocation. A full FIFO can block its bound producer until consumption frees one slot; wakeup grants no reservation, so the producer retries the exact next sequence. A read-only consumer may block on the exact next sequence; a completed publication wakes only that waiter, which must separately consume the frame. Control-plane management can stop a blocked consumer, retire its wait and resources, and let a replacement claim the unchanged sequence. A bounded relative wait can expire through the local APIC, preempt a non-cooperative peer, release the unchanged sequence; publication before the deadline cancels the timer. General protocol loops, UDP/TCP, sockets, routing, and container networking remain future work.
  Protocol bootstrap ABI v1 keeps one measured Rust domain resident while it blocks for and validates two separately published consecutive frames, rejects replay, and retires every wait key and frame on clean exit. Additive ABI v2 continues onto the next exact-sequence wait; control-plane stop authority cancels that wait, revokes packet authority, scrubs the address space, and publishes retirement without granting the service management authority. A reusable memory-safe `no_std` parser and one-entry generation-local ARP neighbor table learn the first peer mapping, refresh an identical mapping, and reject a conflicting MAC without mutation. Graceful drain, expiry, multi-neighbor replacement, and protocol-state handoff remain future work.
  Additive protocol bootstrap ABI v3 runs a bounded ARP-then-IPv4 dispatch generation. After learning the exact peer MAC, the same memory-safe `no_std` parser validates a separately FIFO-delivered fixed IPv4 header, exact length, checksum, non-fragmented boundary, ICMP protocol number, and configured endpoints entirely in ring 3; a locally corrupted checksum is rejected. IPv4 options/reassembly and non-echo ICMP semantics remain future work.
  Additive protocol bootstrap ABI v4 validates the ICMPv4 echo-request type, code, complete-message checksum, fixed proof identifier and sequence, and empty payload after v3 IPv4 validation. A corrupted copied checksum is rejected in ring 3.
  Additive 72-byte protocol bootstrap ABI v5 delegates two distinct objects: ingress `READ` and egress `WRITE`. The memory-safe protocol crate constructs the exact echo reply by swapping Ethernet/IPv4 endpoints, changing ICMP type, and recomputing both checksums. The service cannot read egress, write ingress, or access the device. Its copied reply survives complete protocol-domain retirement in a separate four-entry egress FIFO. A fresh ring-3 consumer with egress `READ` only rejects publication and future-sequence access, receives the exact reply, and rejects replay. Object 9 is a signed authority-envelope root independent from ingress object 8. Additive network bootstrap ABI v2 then gives a fresh network generation device `READ|WRITE` and egress `READ`; it consumes a second reply into confined DMA, rejects future sequence and replay, transmits through queue 1, and completes through kernel-owned MSI-X. Bootstrap v3 makes that bridge restart-safe: operation 26 claims without removal, operation 27 acknowledges only after the kernel validates TX completion, and fault teardown releases the claim while preserving the packet for a fresh network generation. Bootstrap v4 starts the measured network service before output exists: operation 28 blocks its object-9 `READ` capability on the exact next sequence, an isolated write-only producer publishes the ICMP reply, and only that object/sequence waiter wakes before reliable claim, MSI-X-driven TX, and acknowledgement. Operation 29 provides the write-side boundary: a full egress FIFO blocks its bound producer, destructive consume or completed acknowledgement wakes only the object-9 space waiter, and the producer retries without a reserved slot. A QEMU proof couples that wakeup to real virtio-net: claim retains full pressure, validated MSI-X completion permits acknowledgement, acknowledgement wakes the producer, and the exact fifth sequence joins the ordered queue. Bootstrap v5 keeps one network-service generation, TX descriptor, DMA lease, queue, VT-d hierarchy, and device capability live across two consecutive replies. Available, used, and kernel interrupt generations advance in order; each queue head remains claimed until its own validated completion authorizes acknowledgement, and replay is rejected after both acknowledgements. Bootstrap v6 fills and drains all four egress entries in the same generation. Each exact kernel-owned network completion renews a bounded 23-call service epoch, enabling continued work without allowing yield, denial, packet traffic, failure, or timeout to manufacture calls. Bootstrap v7 continues as an indefinite exact-sequence loop after the batch. Operation 30 lets its write-only producer sleep until complete drain, after which the service blocks on sequence five and the control plane stops it. Stop quiesces the active NIC before interrupt, VT-d, DMA, capability, and domain retirement. Ingress waits, timers, and backpressure remain isolated. The kernel independently validates exact sequence/FIFO behavior and ordered wait, device, VT-d, DMA, capability, and domain retirement.
  A host-management action can instead request graceful drain of the exact resident v7 generation. The driver preserves and acknowledges all accepted egress entries, the write-only producer closes only after complete FIFO drain, and producer retirement wakes the blocked next-sequence driver with a distinct drained result. The driver exits normally without overwriting the last validated DMA payload; the kernel validates all four TX, interrupt, and acknowledgement generations before ordered NIC, interrupt, VT-d, DMA, capability, address-space, and domain retirement. Wrong caller, wrong capability, wrong target, and duplicate requests fail closed. A controlled replacement proof separately stops one blocked v7 driver and starts a distinct domain generation with the same logical service identity, freshly derived device and egress authority, and entirely fresh DMA and VT-d table leases. The replacement reinitializes the reset NIC and resumes at the exact next monotonic egress sequence before undergoing the same ordered stop.
  A further fault generation publishes sequence two and immediately enters the compiled driver's fatal path while the NIC is active. Containment resets the device, retires both routes and VT-d, scrubs DMA, and revokes producer authority without discarding the copied frame; another fresh read-only consumer receives it exactly once. Packet ownership therefore survives both clean producer exit and driver failure.
- A separately forced native-network notified timeout keeps TX MSI-X masked until the calibrated APIC timer expires. The kernel then masks and disables MSI-X, revokes bus mastering, resets the NIC, retires vector 35, and retires VT-d and DMA before returning cancellation. The service reports `RESTART_REQUIRED`; a fresh domain generation with new authority and leases must reinitialize the reset NIC and complete an interrupt-driven transmit.
- The automated native-volume sequence performs both a clean replacement and a post-fault replacement. Each replacement has a distinct domain generation, fresh capability table, address space, and DMA lease while retaining only the logical service identity; the post-fault generation must recover the exact committed record or preserve corruption rejection.
- Block waits distinguish prepared from notified requests. A prepared timeout is retryable without consuming its generation; a notified timeout is ambiguous, so the kernel masks interrupts, revokes bus mastering, resets the device, retires and scrubs DMA, and requires a fresh service generation before recovery.
- Every compiled native volume-service generation queries scalar discovery and copies the fixed structured control descriptor before reading bootstrap state or touching the device. It requires ABI v4 with opaque-capability, fault-containment, and structured-descriptor features and validates the descriptor's bounds and reserved field. The QEMU gate separately injects an older scalar version and a nonzero descriptor reserved field into real generations, proves each faults before activating the device, retires their authority and DMA, and recovers persistent state through fresh compatible generations.
- Existing filesystem implementations may be ported or wrapped as authenticated native modules, or as isolated services when boot does not depend on them. Non-Rust implementations are permitted where a rewrite would add risk; module safety requirements do not depend on implementation language.
- Container image layers should be content-addressed and read-only; per-container writable overlays and persistent volumes are separate, explicitly authorized objects.
- Filesystem faults must not silently corrupt the kernel or other domains. Recovery, containment, mounted-domain behavior, and crash consistency must be specified per filesystem and placement model.

### Initial Linux compatibility profile

The acceptance goal is a repository-pinned Rocky Linux 10 x86-64 userspace reaching a systemd-managed console. Required behavior includes x86-64 ELF loading, dynamic linking, signals, futexes, clocks, process lifecycle, mounts, sockets, cgroups v2, console devices, and the `/proc`, `/sys`, `/dev`, credential, and isolation behavior systemd and Rocky exercise. Every system container receives independent PID, mount, user, IPC, cgroup, time, hostname, device, and network views; UID 0 remains local to the container. Unsupported interfaces must fail predictably rather than silently provide weaker semantics. Smaller images remain useful for incremental tests but do not replace the Rocky acceptance gate.

Linux ABI compatibility is libc-neutral. Ginux never selects a musl, glibc, or other libc mode: it resolves the configured init inside the container root, reads the executable's ELF `PT_INTERP`, and admits that declared interpreter from the same immutable root under the normal ELF and resource policy.

## 5. ABI and binary component contracts

### 5.1 Three distinct interfaces

1. **Linux userspace ABI:** Syscalls and observable Linux behavior required by the controller, the pinned Rocky Linux 10 profile, and later distribution/application profiles. Ginux-specific controller operations use a versioned Linux device/ioctl UAPI.
2. **Native service ABI:** Stable, language-neutral binary contract for narrowly scoped isolated services; it is not a second general-purpose application syscall ABI.
3. **Kernel/driver and service ABI:** A stable native kABI for authenticated modules plus IPC, capabilities, shared-memory, resource, lifecycle, failure, and device contracts for isolated components.

### 5.2 Native system and driver ABI requirements

- ELF-based native service format; define exact loading model, calling conventions, entrypoints, alignment, lifecycle, and version negotiation.
- ABI-defined fixed-width structures, opaque handles, explicit ownership and error semantics; no dependency on Rust compiler-private ABI.
- C-compatible ABI surfaces where appropriate, allowing services and drivers written in Rust, C, C++, Zig, assembly, or other languages.
- Machine-readable interface definitions, generated bindings, conformance suites, and ABI regression checks.
- Minimum and target ABI versions, feature-bit negotiation, runtime feature queries, and compatibility adapters where necessary.
- A binary compiled against a supported stable native ABI should continue to function on later conforming kernel releases, subject to documented feature negotiation and lifecycle constraints.
- Interfaces must specify cancellation, timeouts, restart, resource cleanup, revocation, and behavior when either endpoint fails.
- No native ABI exposes raw kernel pointers, unstable internal structures, or authority derived from process ancestry.
- Every kernel module is authenticated, version-compatible, selected by boot policy, mapped according to W^X, and included in the immutable measurement. Userspace components remain isolated and capability-limited.

### 5.3 Linux `.ko` compatibility

Linux `.ko` files depend on Linux-specific exported symbols, data layouts, relocations, build configuration, versioning, and internal semantics. Unmodified Linux `.ko` loading is not a proof-of-concept acceptance criterion. Ginux must nevertheless preserve an architectural path to a bounded compatibility layer for selected, authenticated `.ko` files. Near-term drivers may be implemented as native kABI modules, source-adapted to that kABI, or placed in isolated userspace when they are nonessential and recoverable. No design decision may make future `.ko` investigation impossible without a recorded ADR.

### 5.4 NVIDIA priority

Early feasibility must establish whether NVIDIA's available open kernel-module sources can be adapted as a native module, split across kernel and an isolated driver domain, or supported through a later compatibility layer while retaining matching, unmodified userspace CUDA components. Evaluate firmware dependencies, memory management, GPU faults, unified and pinned memory, MMU/IOMMU interactions, interrupts, command submission, peer memory, licensing, and supportability. The NVIDIA userspace and driver components may update together; the stable Ginux kABI must allow the kernel to update independently within its published compatibility contract. An **actual CUDA workload on a selected GPU** is the hardware go/no-go milestone. Do not claim compatibility before it is demonstrated.

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

Build one UEFI PE/COFF executable with a minimal Rust/assembly EFI entry path and embedded kernel, boot-required modules, controller payload, recovery support, and component catalog. Obtain firmware memory map and platform metadata, authenticate the image and independent configuration, establish required resources, call `ExitBootServices()`, then enter the kernel. The kernel initializes its required mechanisms, launches the embedded controller in ring 3, and seals executable integrity and the authority envelope. Do not require a separate bootloader or initramfs, though deployments may choose a shim or other chain for their Secure Boot policy.

### 7.2 Boot sequence

1. UEFI loads and authenticates `GINUX.EFI`.
2. Boot code validates the embedded component catalog and independently authenticated local configuration.
3. The kernel initializes memory, scheduling, interrupts, IPC, boot storage/filesystems, networking, and required IOMMU/device support from built-in code and authenticated modules.
4. Kernel/module executable integrity, immutable data, and the boot authority envelope are sealed.
5. The kernel creates the initial controller domain, maps the embedded controller payload, and transfers execution to it in ring 3 with exactly the capabilities authorized by configuration.
6. The controller starts optional native services and opens configured container images through kernel storage/filesystem interfaces.
7. For each autostart entry, the controller creates an isolated system-container domain with its own kernel-object and network views.
8. The controller executes the configured init path, normally `/sbin/init`, as PID 1 in each container.
9. The local administrative API and console report component, container, health, and readiness state.

### 7.3 Configuration lifecycle

```text
edit config at runtime
       -> validate
       -> authorize/sign or otherwise authenticate
       -> stage atomically as pending
       -> reboot
       -> verify trust, version, rollback policy and compatibility
       -> activate immutable configuration
       -> boot controller and configured Linux containers
```

- Human-readable source (e.g. `/etc/<project>/boot.toml`) may be edited while running.
- A staging utility generates a canonical manifest, validates it, and authorizes it under a **separate configuration trust root**.
- Kernel and firmware/shim signing keys are **not** required for administrator configuration changes.
- Boot code accepts only trusted, complete, consistent manifests and passes a compact typed immutable configuration and initial capability graph to the kernel.
- Support atomic A/B boot artifacts, health checks, recovery and rollback; security policy determines whether older signed states may be restored.
- Active kernel configuration, trust roots, approved host code, security-policy floor, device ceiling, and maximum authority cannot be modified until reboot.
- Runtime operations such as container lifecycle, resource allocation, IP addresses, routes, mounts, and device grants remain dynamic when authorized within the fixed envelope. Runtime policy may reduce authority but cannot expand the boot ceiling.

### 7.4 Trust choices

Support deployer-managed public-key verification and optional TPM-backed key protection, measurements, sealed secrets, and anti-rollback. TPM is not mandatory. The EFI image is a unified authenticated artifact, while independently signed configuration and container images may be updated without rebuilding it. Separate signing identities and delegation policies for firmware, Ginux images, Linux container images, and site configuration.

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
- Benchmark against Rocky Linux 10 on the identical QEMU/KVM or physical profile. Initial measurements include context-switch and IPC latency, system-container boot time, container startup, TCP throughput/p99 latency, block throughput/p99 latency, CPU-bound overhead, memory overhead, and later GPU throughput/utilization.

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

Work proceeds through hard verification checkpoints. Complete the implementation and automated gate, commit it, and stop for human verification before starting the next checkpoint. [The roadmap](docs/ROADMAP.md) is the detailed execution contract.

| Checkpoint | Demonstrable result | Human verification boundary |
|---|---|---|
| C0: Architecture alignment | This document, ADRs, and roadmap consistently describe the unified image, embedded ring-3 controller, kernel/module device boundary, Rocky benchmark, and future `.ko` path | Review documentation and signed commit |
| C1: Interactive unified boot | One `GINUX.EFI` boots on the Mac QEMU profile and presents controller diagnostics, including measured-root `ls` and live-domain `ps` | User boots and interacts; no container yet |
| C2: Runtime container lifecycle | No container exists implicitly; the resident host-PID-1 controller creates and configures one named, boot-local minimal fixture, then starts, observes, stops, and restarts it without relaunching itself (**implemented; awaiting human verification**) | User verifies creation, configuration, lifecycle, and failure recovery |
| C3: Alpine shell container | Control networking automatically pulls a missing digest-pinned Alpine x86-64 OCI image into a verified boot-local tmpfs cache; explicit static/DHCP container networking is configured; `/bin/sh` executes with an attachable console and retained boot log | User interacts with the unmodified Alpine BusyBox shell, detaches back to the controller with `Ctrl-]`, reattaches, and verifies that reboot clears the C3 cache |
| C4: Operational Rocky | Required mounts, devices, processes, credentials, cgroups, clocks, and console behavior work normally | User interacts with a functioning Rocky system |
| C5: Networked persistent Rocky | Rocky owns an IP, routes, loopback, sockets, and persistent volume; the controller can provision local storage and stage verified persistent state for activation on reboot; SSH works inside the container | User logs in over SSH and verifies controlled activation and reboot persistence |
| C6: Two isolated systems | Two systemd containers run concurrently with distinct kernel-object and network views | User attempts cross-container isolation checks |
| C7: Remote authenticated management | A mutually authenticated client can inspect and manage configured hosts without exceeding local policy | User verifies trust enrollment, authorization, and revocation |

Performance, security, stable-kABI, driver, GPU, cryptographic-validation, and hardware milestones continue after these bootstrapping checkpoints. They require their own explicit acceptance gates rather than being silently folded into C0-C7.

## 12. Open design decisions

1. Which operations, if any, require an in-kernel fast path rather than shared-memory IPC to an isolated service?
2. What exact objects, operations, failure semantics, version negotiation, and compatibility guarantees make up the stable native system/driver ABI?
3. Which control-plane observation and debugging capabilities are enabled in production, particularly access to container memory and secrets?
4. Which GPU functions belong in a built-in driver, authenticated module, or isolated domain, and what later compatibility layer is required?
5. What exact digest and construction recipe define the Rocky Linux 10 x86-64 acceptance image, and which smaller fixtures prove each compatibility increment?
6. Which existing filesystem implementation and on-disk format should be reused or adapted as the first native filesystem module?
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

## 14. Immediate engineering sequence

The next work is exactly the C1-C7 sequence in the roadmap. Do not begin a later checkpoint until the preceding checkpoint has a passing automated gate, a signed commit, and human verification. Cross-cutting investigations may inform decisions, but they may not expand a checkpoint's implementation scope without an ADR and an explicit roadmap change.

**Engineering discipline:** Do not silently relax compatibility, security, or performance requirements. Record tradeoffs in architecture decision records (ADRs), including measured evidence and proof implications.
