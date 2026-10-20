# Ginux

**An operating system built around native domains, with Linux compatibility as
a userspace personality.**

Ginux is an experimental, immutable OS designed to run familiar Linux
userspaces directly on hardware without a conventional host distribution or a
guest kernel for every workload. Its fundamental boundary is the **domain**:
a kernel-native unit of isolation, ownership, accounting, scheduling, and
failure containment.

Runtime allocation policy and remaining fixed-capacity migrations are tracked
in [the runtime capacity audit](docs/RUNTIME-CAPACITY-AUDIT.md).

The kernel provides the mechanisms that must be shared and trusted: CPU
scheduling, memory isolation, capabilities, IPC, interrupts, device access,
and personality-facing kernel objects. Everything else is divided into
explicit domains. A small controller runs in ring 3, services receive only the
authority they need, and each workload domain gets its own process,
filesystem, device, network, and identity views.

The long-term goal is a host that can run different **userspace personalities**
side by side. Linux is the first and primary personality. A future BSD
personality, for example, could run in another domain on the same kernel while
sharing the same underlying isolation, scheduling, storage, and device
mechanisms.

Ginux is currently a research and engineering prototype. It boots, runs its
statically linked Linux-ABI controller and isolated native services, executes
an unmodified Alpine Linux userspace, and launches Rocky Linux 10 systemd as
domain PID 1 under QEMU. Rocky reaches `multi-user.target` and exposes a
controller-attached root shell. Ginux does not yet provide complete Linux
compatibility, a clean systemd boot, or a second userspace personality.

## Why Ginux?

- **Domains are the host abstraction.** There is no general-purpose host
  distribution competing with workloads for authority, updates, or attack
  surface. OCI images populate Linux-domain roots; they do not define the
  security boundary.
- **Isolation is native.** Domains and capabilities are the security boundary;
  Linux namespaces, containers, and UID 0 are compatibility constructs inside
  that boundary.
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
+-----------------------------------------------------------------------+
| Control domain       | Native service domains | Workload domains      |
| Linux-ABI controller | drivers and protocols  | Linux; others later   |
+-----------------------------------------------------------------------+
| Domain boundary: ownership | capabilities | resources | lifecycle     |
+-----------------------------------------------------------------------+
| Personality contracts: syscalls | ELF | processes | VFS | devices     |
+-----------------------------------------------------------------------+
| Ginux kernel: scheduling | memory | IPC | IOMMU | core devices        |
+-----------------------------------------------------------------------+
| UEFI firmware and x86-64 hardware                                     |
+-----------------------------------------------------------------------+
```

This is a logical layout, not a requirement that every byte cross every
layer. Control operations remain capability checked. Once authorized, data may
move through shared pages or device queues without repeatedly traversing the
controller.

### Terminology

- A **domain** is the outer kernel-enforced boundary and resource-policy
  principal. The controller, isolated native services, and Linux workloads all
  execute in domains.
- A **personality** is the userspace contract exposed inside a workload domain.
  `personality=linux` selects Linux x86-64 ELF, syscall, process, VFS, and
  device semantics; it does not turn the Ginux kernel into Linux.
- An **image** supplies a domain's initial filesystem. OCI is one supported
  image distribution and assembly format, not an isolation mechanism.
- A **Linux container** is a compatibility construct inside a Linux-personality
  domain. Linux namespaces may subdivide that domain, but cannot cross or
  weaken its outer Ginux boundary.

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
domain. Every later service or workload domain receives authority delegated
from an existing holder. Delegation may narrow authority but cannot create
rights that were absent from the authenticated boot envelope.

Process ancestry controls lifecycle and accounting; it does not grant
authority. A process may be root inside a Linux domain while having no implicit
right to inspect the control domain, another workload domain, kernel memory,
or an unrelated device.

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

Domain resource settings do not have arbitrary host-wide ceilings.
`memory-mib` accepts any nonzero 64-bit MiB value, and OCI staging/rootfs
allocation has no configured byte cap. Values are bounded only by their ABI
representation, addressability, and resources actually available at runtime.

Each configured workload is a Ginux domain. That domain is the outer,
non-bypassable security boundary and owns its CPU and memory budget, process
tree, IPC objects, filesystem root and mount view, network view, and devices.
New domains default to `personality=linux`, selecting the Linux userspace
contract; `image=...` selects how that domain's system root is populated. A
domain without an explicit network attachment uses the control domain's host
network; this shares connectivity, not authority to reconfigure the host
interface.

Physical network devices are discovered by the kernel. Operators may create
named virtual devices with `network new`, chain them to physical or virtual
uplinks, and attach multiple devices by repeating `network=DEVICE` in a
`domain set` command. Virtual-device administrative state uses the same
`network start`, `network stop`, `network restart`, and `network status`
commands as physical devices. Attachments are independent kernel objects, not a
singleton namespace field, and names remain unchanged across the domain
boundary. The inventory/attachment control path is implemented; isolated
per-domain packet queues, address configuration, DHCP, DNS, routes, and Linux
interface enumeration remain active C3 dataplane work.

The filesystem root is an abstract domain-owned kernel object. OCI is an image
ingestion and population format, not an isolation mechanism: an embedded
image, an OCI-derived image, or a future persistent-volume snapshot can back
the same root-object contract without changing the domain boundary.

A Linux personality domain starts with its own root Linux PID, mount, user,
IPC, UTS, and network namespace views. Linux namespace operations such as
`clone`, `unshare`, and `setns` may create and join descendants of those roots,
but can never cross the enclosing Ginux domain. This makes nested Linux
containers a supported compatibility case: systemd or a container manager
inside a Linux domain may create further Linux namespaces, while every
resulting process and object remains owned and limited by the same outer Ginux
domain.

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
domain-local PID 1 is the product compatibility benchmark.

The architecture is intended to make personality selection a property of a
domain rather than of the whole host. A BSD personality is a plausible future
addition because it can reuse the same kernel mechanisms, but it would still
require an explicit BSD syscall and userspace ABI implementation. Loading an
ELF file alone is not compatibility.

The controller is a static Linux-ABI executable: console and lifecycle use
standard Linux syscalls, while host-specific management uses a versioned
device/ioctl UAPI. Narrow isolated Ginux services may use a small, versioned
service ABI and bounded static PIE ELF images. Ginux is not creating a third
general-purpose application syscall ecosystem.

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
7. The controller delegates authority to isolated native-service and workload
   domains, then starts each selected userspace init process.

The native host does not need a distribution root filesystem. Workload images,
writable data, logs, and persistent volumes are explicit objects rather than
files hidden in a mutable host installation.

Production deployments are intended to separate trust for the EFI image,
site configuration, and workload images. Changing runtime policy may reduce
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
- A workload root remains local to its personality domain. Linux privilege
  does not imply control-domain or native-kernel privilege.
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

The active runtime stabilization findings and required multi-domain QEMU
acceptance sequence are tracked in
[docs/STABILITY-AUDIT.md](docs/STABILITY-AUDIT.md). Runtime behavior is treated
as complete only when its real lifecycle gate passes; configuration-state
simulation is not sufficient.

| Area | What works now | Next boundary |
|---|---|---|
| Boot | Direct x86-64 UEFI boot as one EFI application under Q35/OVMF | Production signing, recovery, and physical hardware profiles |
| Controller | Persistent ring-3 command console with command-name completion, aligned status tables, and component, process, network, and domain lifecycle control | Dynamic argument completion and remote authenticated management |
| Domains | Default-Linux domains with concurrent isolated address spaces, root/mount and process views, retained CPU state, IPC, scheduling, lifecycle events, fault containment, generation-safe cleanup, and command execution in a running domain | Fresh per-exec PTYs, asynchronous exec sessions, and broader per-domain resource accounting and policy enforcement |
| Native services | Measured ring-3 network, protocol, and volume services with bounded ABIs and restart proofs | Production service supervision and broader device support |
| Storage | Self-initializing host volume with transactional configuration recovery and a persistent, digest-verified OCI layer cache | Add writable domain layers, explicit volumes, cache refresh/garbage collection, and logs |
| Devices | Virtio block/network paths, MSI-X, and VT-d DMA confinement in the QEMU profile | NVMe, high-speed networking, GPU support, and selected real servers |
| Linux personality | Bounded x86-64 ELF/dynamic-linker support, a growing syscall/VFS/process/signal ABI, per-domain procfs, Alpine musl/BusyBox, Rocky glibc/coreutils, Rocky systemd through `multi-user.target`, authenticated console login, and `domain exec` | Close systemd's remaining netlink, event, journal, cgroup, and masked-unit/symlink gaps |
| Other personalities | Domain and kernel mechanisms are designed for reuse | No BSD or other non-Linux personality is implemented yet |

The Linux layer is intentionally tracked as a compatibility contract, not as a
binary yes/no claim. Many interfaces are partial, and major surfaces including
general sockets, cgroups, sysfs, and broad device `ioctl` compatibility remain.
See the [Linux compatibility matrix](docs/LINUX-COMPATIBILITY.md) for the
current ledger and the
[ELF/runtime regression contract](docs/LINUX-RUNTIME-COMPATIBILITY.md) for the
executable-loading, dynamic-linking, TLS, heap, and virtual-memory gates.

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

`make run` keeps a sparse 64 GiB host volume at `build/qemu/volume.raw`. The
volume is initialized on its first boot and transactionally recovered on later
boots. Set `GINUX_VOLUME_SIZE` before the first boot to choose another sparse
size, or run `make reset-storage` to delete the development volume; the next
boot recreates a clean one. This checkpoint establishes and validates the
durable volume format. Domain definitions, resource settings, autostart flags,
virtual-network topology, attachments, and static/DHCP host-network policy are
recovered from it after a clean shutdown. Verified compressed OCI layers are
stored content-addressed in the same volume and reused after reboot without a
registry transfer. `make test-oci-cache` validates the cold-pull and warm-hit
lifecycle. Cache catalog v2 can also store and directly publish a complete,
digest-verified immutable root filesystem. Ginux selects it only when measured
size predicts it will beat restoring and unpacking the compressed layers. See
[the storage architecture](docs/STORAGE.md).

At the `ginux>` prompt:

```text
help
status
network new gnet0 type=virtual
domain new alpine
domain set alpine image=alpine:latest init=/bin/sh network=gnet0
domain start alpine
domain list
domain status alpine
domain attach alpine
domain exec alpine /bin/echo hello
domain exec alpine --interactive /bin/sh
```

`domain start NAME` queues startup and returns immediately. `domain list`
reports transitional `starting` and `booting` states while the work proceeds;
`domain status NAME` adds root-filesystem, execution, console, resource, image,
and network details. Use `domain start NAME --attach` when the initial console
should remain in the foreground. The example resolves and downloads Alpine
from Docker Hub, verifies and unpacks its layer, then attaches to the unmodified
Alpine shell. Multiple domains
can run concurrently; each keeps its own address space, root/mount view,
process view, and suspended execution state. Press Control-] to detach, use
`domain attach NAME` to return, and use `exit` to stop the attached Linux
personality domain. `shutdown` stops the host.

`domain exec NAME COMMAND [ARG...]` starts a child process in an already-running
domain without replacing or stopping PID 1. Add `--interactive` after the
domain name to attach the command to the domain console; exiting that command
returns to the controller and leaves the domain running. For example,
`domain exec rocky /usr/bin/passwd -d root` changes the root account inside
the domain while preserving the authenticated console-login path itself.

OCI layers are downloaded and verified in manifest order, then assembled with
replacement, whiteout, and opaque-directory semantics before publication.
OCI ends at that publication boundary. The resulting immutable root filesystem
is source-neutral: the domain's Linux personality owns VFS lookup, ELF loading,
process creation, scheduling, and init lifecycle. Domain execution therefore
does not depend on OCI state and uses the same path for root filesystems
published by future transports or local storage.
The requested canonical image reference and `linux/amd64` platform select the
persistent cache entry. Layer data is flushed before its transactional catalog
generation is committed; corrupt objects fail digest verification and are
never published as a domain root.
Full controlling-terminal and session semantics are still incomplete. Rocky
Linux 10 now starts an unmodified glibc `/bin/sh`; shell builtins, file status,
dynamic `cat`, and root-directory `ls` pass the QEMU lifecycle gate. This is a
meaningful ABI checkpoint, not a claim of general Linux compatibility. Rocky
Linux 10 systemd must use the init-bearing image and an explicit init path:

```text
domain new rocky
domain set rocky image=rockylinux/rockylinux:10-ubi-init init=/sbin/init
domain start rocky --attach
```

The base `rockylinux/rockylinux:10` image declares `/bin/bash` and does not
contain systemd; configuring `/sbin/init` against that tag is an image/content
error, which Ginux reports explicitly. The UBI-init image launches systemd as
PID 1 and reaches `multi-user.target`. Ginux installs a trusted
controller-attach console service that runs an interactive root shell; the
QEMU gate executes a command in that shell rather than accepting a prompt as
proof. Bash line editing is disabled until console readiness wakes are
complete, so the service uses the verified blocking TTY read path. This is not
yet a clean Linux boot: netlink, journal transport, complete FIFO semantics,
parts of inotify/epoll, procfs mount identification, and broader API-filesystem
behavior remain visible gaps. The process layer contains generation-checked copy-on-write,
but Linux personality forks currently use eager private copies: repeated Rocky
fork/exec/exit cycles exposed incomplete COW retirement bookkeeping, so the
optimization is disabled on the production path until that lifecycle is
validated end to end. Process metadata remains bounded, and major surfaces
such as cgroups, sysfs, broad
`ioctl` families, and systemd's remaining contracts still need implementation.

Useful development gates:

```sh
make build          # build the unified EFI image
make test           # run the core QEMU boot gate
make test-linux-runtime # verify ELF, loader, PIE, TLS, heap, and memory behavior
make test-c2        # verify configured domain lifecycle
make test-alpine    # verify runtime OCI download and the Alpine shell
make test-rocky-shell # verify Rocky glibc shell, fork/exec, cat, and ls
make test-multi-domain # verify concurrent Alpine domains and Rocky ingestion
make probe-rocky-systemd # boot Rocky systemd and require its console login prompt
make test-rocky-systemd # verify cold and cached warm authenticated console boots
make benchmark-rocky-systemd # measure init; enforce the 3 s KVM budget
make test-storage    # verify volume initialization, recovery, and reset
make check          # run architecture, ABI, format, lint, and unit checks
make gate           # run the clean reproducible project gate
```

The EFI filesystem is written to
`build/esp/EFI/BOOT/BOOTX64.EFI`. Run `make help` for all supported targets and
environment overrides.

On Apple Silicon, this x86-64 target runs under QEMU TCG rather than hardware
virtualization. Large systemd workloads are therefore substantially slower than
Alpine or the controller. The current Rocky init-to-console measurement is
about 4.08 seconds under TCG; KVM acceleration and its three-second gate become
available on an x86-64 Linux host. Use `make profile-rocky-systemd` for the
opt-in kernel phase and syscall profile.

## Repository guide

- [`boot/efi`](boot/efi) builds the direct UEFI entry and unified image.
- [`kernel`](kernel) contains the kernel, Linux personality work, executable
  loaders, and QEMU hardware paths.
- [`control-plane/runtime`](control-plane/runtime) is the embedded ring-3
  controller.
- [`services`](services) contains isolated native service runtimes.
- [`crates`](crates) contains shared `no_std` contracts and state machines.
- [`specs`](specs) records versioned boot, kernel, ABI, domain, image, and service
  contracts.
- [`tests`](tests) contains ABI, configuration, and end-to-end QEMU gates.
- [`tools`](tools) contains image preparation, manifest, architecture, and QEMU
  tooling.

## Technical references

- [Architecture](ARCHITECTURE.md) — full system direction and design choices.
- [Verification roadmap](docs/ROADMAP.md) — staged product checkpoints.
- [Linux compatibility matrix](docs/LINUX-COMPATIBILITY.md) — current ABI
  coverage and omissions.
- [Runtime use cases](docs/USE-CASES.md) — observable acceptance scenarios,
  current gates, and the next end-to-end tests.
- [Performance baseline](docs/PERFORMANCE.md) — benchmark definitions, current
  results, and accelerator-specific gates.
- [Native domains and authority](docs/decisions/0001-native-domains-and-authority.md)
  — the isolation and delegation model.
- [Unified image and driver boundary](docs/decisions/0002-unified-image-and-driver-boundary.md)
  — what belongs in the kernel, boot-time modules, and isolated services.
- [C3 Alpine verification](docs/checkpoints/C3.md) — the current interactive
  userspace checkpoint.

## License

Ginux is licensed under the [Apache License, Version 2.0](LICENSE). See
[`NOTICE`](NOTICE) for attribution information.
