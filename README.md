# Ginux

**An operating system built around containers, not retrofitted with them.**

Ginux is an experimental, immutable host OS designed to run familiar Linux
userspaces directly on hardware without a conventional host distribution and
without a guest kernel for every container.

The kernel provides the mechanisms that must be shared and trusted: CPU
scheduling, memory isolation, capabilities, IPC, interrupts, device access,
and container-facing kernel objects. Everything else is divided into explicit
security domains. A small controller runs in ring 3, services receive only the
authority they need, and each container gets its own process, filesystem,
device, network, and identity views.

The long-term goal is a host that can run different **userspace personalities**
side by side. Linux is the first and primary personality. A future BSD
personality, for example, could run in another domain on the same kernel while
sharing the same underlying isolation, scheduling, storage, and device
mechanisms.

Ginux is currently a research and engineering prototype. It boots, runs its
native controller and isolated services, and executes an unmodified Alpine
Linux userspace in QEMU. It does not yet provide complete Linux compatibility,
boot Rocky Linux with systemd, or implement a second userspace personality.

## Why Ginux?

- **Containers are the host abstraction.** There is no general-purpose host
  distribution competing with workloads for authority, updates, or attack
  surface.
- **Isolation is native.** Domains and capabilities are the security boundary;
  Linux namespaces and UID 0 are compatibility views inside that boundary.
- **The booted system is fixed.** One EFI image carries the kernel, required
  components, controller, and recovery metadata. Runtime authority cannot
  exceed the envelope established at boot.
- **Failures have owners.** Services, devices, memory, and outstanding work are
  tied to domain generations so they can be revoked and cleaned up together.
- **Compatibility lives at the edge.** Applications see the userspace ABI they
  expect while the host remains purpose-built.
- **Fast paths stay fast.** Protected control paths establish authority; shared
  memory, batching, and hardware queues carry authorized data directly.

## The model

```text
+------------------------------------------------------------------+
| Linux domain             | Future userspace personality domain   |
| Alpine / Rocky / apps    | e.g. a selected BSD userspace         |
+------------------------------------------------------------------+
| Personality contracts: syscalls, ELF, processes, VFS, devices    |
+------------------------------------------------------------------+
| Ring-3 controller        | Isolated native services              |
| lifecycle and policy     | storage, network, protocol, crypto    |
+------------------------------------------------------------------+
| Ginux kernel                                                    |
| scheduling | memory | capabilities | IPC | domains | IOMMU       |
+------------------------------------------------------------------+
| UEFI firmware and x86-64 hardware                               |
+------------------------------------------------------------------+
```

This is a logical layout, not a requirement that every byte cross every
layer. Control operations remain capability checked. Once authorized, data may
move through shared pages or device queues without repeatedly traversing the
controller.

## Domains: the unit of separation

A domain is the native unit of isolation, ownership, accounting, and failure
containment. It owns or is granted:

- processes, threads, and address spaces;
- a sealed, domain-local capability table;
- memory and CPU budgets;
- IPC endpoints and shared-memory grants;
- filesystem and kernel-object views;
- network endpoints and device grants; and
- a lifecycle and failure policy.

The kernel creates one initial userspace authority: the embedded control-plane
domain. Every later service or container receives authority delegated from an
existing holder. Delegation may narrow authority but cannot create rights that
were absent from the authenticated boot envelope.

Process ancestry controls lifecycle and accounting; it does not grant
authority. A process may be root inside a Linux domain while having no implicit
right to inspect the host, another container, kernel memory, or an unrelated
device.

Domain identities are generation tagged. When a service exits or faults, stale
capabilities, replies, interrupt routes, DMA mappings, and device ownership can
be rejected before its memory is reused. A replacement is a new generation,
not a continuation of the failed security context.

A domain is also the kernel's resource-policy principal. Hard limits and
service policy are deliberately separate: CPU and memory limits define what a
domain may never exceed, while per-resource service classes define how the
kernel arbitrates contention. CPU, memory pressure, storage I/O, and network
I/O each have an independent class. This permits, for example, latency-critical
CPU service with ordinary storage service, or a scavenger domain that runs only
from capacity left by higher classes. The CPU scheduler already selects the
highest ready domain class and remains round-robin within a class; memory and
I/O subsystems must consult their corresponding domain policy as their
contention queues are introduced. Only an ancestor domain may change a
descendant's policy, so a workload cannot promote itself.

Container resource settings do not have arbitrary host-wide ceilings.
`memory-mib` accepts any nonzero 64-bit MiB value, and OCI staging/rootfs
allocation has no configured byte cap. Values are bounded only by their ABI
representation, addressability, and resources actually available at runtime.

Each configured container is backed by exactly one Ginux domain. The domain is
the outer, non-bypassable security boundary and owns the container's CPU and
memory budget, process tree, IPC objects, filesystem root and mount view,
network view, and devices. A container without an explicit network attachment
uses the control domain's host network; this shares connectivity, not the
authority to reconfigure the host interface.

The filesystem root is an abstract domain-owned kernel object. OCI is an image
ingestion and population format, not an isolation mechanism: an embedded
image, an OCI-derived image, or a future persistent-volume snapshot can back
the same root-object contract without changing the domain boundary.

A Linux personality domain starts with its own root Linux PID, mount, user,
IPC, UTS, and network namespace views. Linux namespace operations such as
`clone`, `unshare`, and `setns` may create and join descendants of those roots,
but can never cross the enclosing Ginux domain. This makes nested containers a
supported compatibility case: systemd or a container manager inside a Linux
domain may create further Linux namespaces, while every resulting process and
object remains owned and limited by the same outer Ginux domain.

## Userspace personalities

A userspace personality is the complete operating-system contract presented to
a domain. It includes more than an executable format:

- syscall numbers and behavior;
- process, thread, signal, and credential semantics;
- executable loading and dynamic linking;
- virtual filesystem behavior and paths such as `/proc`, `/sys`, and `/dev`;
- sockets, clocks, synchronization, terminals, and `ioctl` families; and
- the domain's expected namespace and device model.

Linux x86-64 is the only personality under implementation today. Linux ELF is
therefore the practical workload format, and Rocky Linux 10 with systemd as
container PID 1 is the product compatibility benchmark.

The architecture is intended to make personality selection a property of a
domain rather than of the whole host. A BSD personality is a plausible future
addition because it can reuse the same kernel mechanisms, but it would still
require an explicit BSD syscall and userspace ABI implementation. Loading an
ELF file alone is not compatibility.

Native Ginux services are separate from workload personalities. They use a
small, versioned service ABI and bounded static PIE ELF images. Ginux is not
trying to create a third general-purpose application ecosystem.

## Boot process

The target boot sequence is deliberately short:

1. UEFI loads a single `GINUX.EFI` PE/COFF application.
2. The boot path validates the embedded component catalog, measured payloads,
   and independently authorized configuration.
3. Firmware state and the final memory map are handed to the kernel, and boot
   services are exited.
4. The kernel establishes paging, interrupts, scheduling, capabilities,
   required storage and network mechanisms, and IOMMU protection.
5. Executable mappings and the maximum authority for the boot are sealed.
6. The embedded controller starts in ring 3 with exactly its configured
   capabilities.
7. The controller delegates authority to native services and container
   domains, then starts each selected userspace init process.

The native host does not need a distribution root filesystem. Container
images, writable data, logs, and persistent volumes are explicit objects rather
than files hidden in a mutable host installation.

Production deployments are intended to separate trust for the EFI image,
site configuration, and container images. Changing runtime policy may reduce
authority immediately, but expanding the boot authority ceiling requires a new
authenticated boot.

## Security model

Ginux aims to make authority visible, narrow, and revocable:

- Capabilities name objects and permitted operations; IDs, paths, PIDs, and
  ancestry are not substitutes for authority.
- The controller is highly trusted but still runs outside ring 0 and cannot
  synthesize capabilities, rewrite kernel memory, or replace sealed code.
- Native services use separate W^X address spaces with guarded stacks and
  explicitly mapped bootstrap data.
- SMEP, SMAP, UMIP, supervisor-only mappings, and non-executable data mappings
  harden the CPU boundary on x86-64.
- Device DMA is confined with the IOMMU. Bus mastering, interrupt routes, MMIO,
  queues, and DMA pages are activated and revoked in a defined order.
- A service fault triggers domain-specific cleanup. Device access is quiesced
  before translation tables and memory are released.
- Container root remains local to its personality domain. Linux privilege does
  not imply native host privilege.
- Boot manifests, executable identities, bounds, and reserved fields are
  checked fail closed.

This design reduces ambient authority; it does not make a universal claim of
bug-free or formally verified security. Firmware, hardware, compilers, unsafe
code, and unverified components remain part of the threat model.

## Performance model

Isolation is useful only if real workloads can afford it. Ginux separates
control from data:

- capability checks establish who may use a resource;
- shared memory and mapped hardware queues avoid unnecessary copies;
- batching reduces transitions and interrupt overhead;
- IOMMU mappings allow direct device access without granting all of RAM;
- NUMA placement, CPU affinity, pinned memory, PCIe topology, NVMe, RDMA, and
  GPU paths can be optimized for AI and HPC workloads; and
- benchmarks compare the same workload and hardware profile against Rocky
  Linux rather than relying on architectural claims.

The kernel-versus-service boundary is decided per subsystem. Boot-critical or
latency-sensitive mechanisms may live in the kernel or authenticated boot-time
modules. Components that benefit from independent restart and narrow authority
may run as isolated services. Measurements, not a monolithic-versus-microkernel
label, decide the final path.

## Current state

The active runtime stabilization findings and required multi-container QEMU
acceptance sequence are tracked in
[docs/STABILITY-AUDIT.md](docs/STABILITY-AUDIT.md). Runtime behavior is treated
as complete only when its real lifecycle gate passes; configuration-state
simulation is not sufficient.

| Area | What works now | Next boundary |
|---|---|---|
| Boot | Direct x86-64 UEFI boot as one EFI application under Q35/OVMF | Production signing, recovery, and physical hardware profiles |
| Controller | Persistent ring-3 command console with command-name completion, aligned status tables, and component, process, network, and container lifecycle control | Dynamic argument completion, remote authenticated management, and persistent configuration |
| Domains | Concurrent container domains with isolated address spaces, root/mount and process views, retained CPU state, IPC, scheduling, lifecycle events, fault containment, and generation-safe cleanup | Broader per-domain resource accounting and policy enforcement |
| Native services | Measured ring-3 network, protocol, and volume services with bounded ABIs and restart proofs | Production service supervision and broader device support |
| Devices | Virtio block/network paths, MSI-X, and VT-d DMA confinement in the QEMU profile | NVMe, high-speed networking, GPU support, and selected real servers |
| Linux personality | Bounded x86-64 ELF/dynamic-linker support, a growing syscall/VFS/process/signal ABI, an automatically mounted per-domain procfs, and an unmodified Alpine musl/BusyBox shell with rootfs-backed commands | Complete Alpine checkpoint, then Rocky Linux 10 and systemd |
| Other personalities | Domain and kernel mechanisms are designed for reuse | No BSD or other non-Linux personality is implemented yet |

The Linux layer is intentionally tracked as a compatibility contract, not as a
binary yes/no claim. Many interfaces are partial, and major surfaces including
general sockets, cgroups, sysfs, and broad device `ioctl` compatibility remain.
See the [Linux compatibility matrix](docs/LINUX-COMPATIBILITY.md) for the
current ledger.

The current validated development target is QEMU's x86-64 Q35 machine with
OVMF, Intel IOMMU, and virtio devices. QEMU uses KVM on a suitable x86-64 Linux
host and TCG otherwise, including on Apple Silicon Macs. Physical-server
support is a later milestone.

## Try it

Requirements:

- Rust 1.99.0 through `rustup`;
- Clang with the `x86_64-none-elf` target;
- QEMU with x86-64 and OVMF support;
- GNU Make.

Check the host, build Ginux, and boot it:

```sh
make doctor
make run
```

At the `ginux>` prompt:

```text
help
status
container new alpine
container set alpine image=alpine:latest init=/bin/sh
container start alpine --attach
```

The final command resolves and downloads Alpine from Docker Hub, verifies and
unpacks its layer, then enters the unmodified Alpine shell. Multiple domains
can run concurrently; each keeps its own address space, root/mount view,
process view, and suspended execution state. Press Control-] to detach, use
`container attach NAME` to return, and use `exit` to stop the attached
container. `shutdown` stops the host.

OCI layers are downloaded and verified in manifest order, then assembled with
replacement, whiteout, and opaque-directory semantics before publication.
Full controlling-terminal and session semantics are still incomplete. The
Rocky `10-ubi-init` image now reaches its dynamic linker, but systemd remains
blocked by the fixed-size runtime mmap bookkeeping; replacing that bookkeeping
with domain-memory-backed dynamic metadata is the next compatibility step.

Useful development gates:

```sh
make build          # build the unified EFI image
make test           # run the core QEMU boot gate
make test-c2        # verify configured container lifecycle
make test-alpine    # verify runtime OCI download and the Alpine shell
make test-multi-container # verify concurrent Alpine domains and Rocky ingestion
make probe-rocky-systemd # probe Rocky systemd's documented mmap boundary
make check          # run architecture, ABI, format, lint, and unit checks
make gate           # run the clean reproducible project gate
```

The EFI filesystem is written to
`build/esp/EFI/BOOT/BOOTX64.EFI`. Run `make help` for all supported targets and
environment overrides.

## Repository guide

- [`boot/efi`](boot/efi) builds the direct UEFI entry and unified image.
- [`kernel`](kernel) contains the kernel, Linux personality work, executable
  loaders, and QEMU hardware paths.
- [`control-plane/runtime`](control-plane/runtime) is the embedded ring-3
  controller.
- [`services`](services) contains isolated native service runtimes.
- [`crates`](crates) contains shared `no_std` contracts and state machines.
- [`specs`](specs) records versioned boot, kernel, ABI, container, and service
  contracts.
- [`tests`](tests) contains ABI, configuration, and end-to-end QEMU gates.
- [`tools`](tools) contains image preparation, manifest, architecture, and QEMU
  tooling.

## Technical references

- [Architecture](ARCHITECTURE.md) — full system direction and design choices.
- [Verification roadmap](docs/ROADMAP.md) — staged product checkpoints.
- [Linux compatibility matrix](docs/LINUX-COMPATIBILITY.md) — current ABI
  coverage and omissions.
- [Native domains and authority](docs/decisions/0001-native-domains-and-authority.md)
  — the isolation and delegation model.
- [Unified image and driver boundary](docs/decisions/0002-unified-image-and-driver-boundary.md)
  — what belongs in the kernel, boot-time modules, and isolated services.
- [C3 Alpine verification](docs/checkpoints/C3.md) — the current interactive
  userspace checkpoint.

## License

Ginux is licensed under the [Apache License, Version 2.0](LICENSE). See
[`NOTICE`](NOTICE) for attribution information.
