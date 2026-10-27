# Ginux

**An independent Rust-first kernel that runs selected Linux userspace workloads,
built as a case study in Human-Directed Agentic Engineering (HDAE).**

[![DOI](https://zenodo.org/badge/DOI/10.5281/zenodo.23682548.svg)](https://doi.org/10.5281/zenodo.23682548)

Ginux asks a deliberately difficult question: can a human-directed coding agent
build a new kernel that runs an existing Linux userspace?

Over eleven days, one persistent OpenAI Codex task using GPT-5.6 Sol produced a
focused x86-64 research kernel containing approximately 139,000 retained lines
of implementation and test code. Ginux boots through UEFI, provides its own
memory management, scheduler, process model, filesystems, networking, devices,
and Linux compatibility layer, and runs its controller in ring 3. The evaluated
system executes selected unmodified Alpine programs and completes a Rocky Linux
10 systemd boot through `multi-user.target`.

This is a research prototype, not a production kernel or a proposed replacement
for Linux. Compatibility is partial, hardware coverage is narrow, the full test
target retains a known failure, and the project has not received an independent
security audit. Ginux demonstrates that AI can rapidly integrate established
systems mechanisms under informed human direction. It does not reproduce the
hardware support, compatibility, review, community, or long-term maintenance
that Linux has earned over decades.

The name is a joke in the same family as Linux: **Ginux** combines Linux, GPT,
and Greg. To keep the author's ego within its configured resource limit, the
`G` officially stands for GPT.

## Paper

The technical report describes the research questions, HDAE method, system
architecture, evaluation, limitations, and implications:

- [DOI: 10.5281/zenodo.23682548](https://doi.org/10.5281/zenodo.23682548)
- [LaTeX source](docs/paper/ginux-ai-systems-report.tex)
- [Steering chronology](docs/paper/STEERING-PROVENANCE.md)
- [Benchmark artifacts](docs/paper/artifact/benchmark-2026-10-23)

The paper evaluates commit
`d11c126a048a51fe7eb9c4105f31760add1027ea`, dated October 23, 2026. Later
repository changes, including publication materials, are not silently treated
as part of that experimental checkpoint.

## Research questions and result

Ginux studies three questions:

1. **Capability:** Can HDAE produce an independent kernel that boots and
   executes selected unmodified Linux distribution workloads?
2. **Method:** What documentation, steering, executable gates, and feedback
   loops sustain a long-running agentic systems project?
3. **Outcome quality:** What compatibility and performance does the resulting
   system demonstrate, and where do its claims stop?

The bounded result is **yes** for the tested workloads. Ginux runs selected
Alpine musl/BusyBox and Rocky Linux glibc programs, boots Rocky systemd through
`multi-user.target`, and executes the same sysbench binary used on a stock Rocky
Linux comparison system. Benchmark-guided iteration corrected severe memory
and thread-path defects and reached approximate parity in three narrow,
single-vCPU workloads under cross-architecture QEMU translation. Those results
are diagnostic evidence, not a general performance claim.

The Linux 6.12 x86-64 syscall ledger classifies all 375 syscall numbers:

- 82 complete;
- 245 partial; and
- 48 policy-denied.

Classification is not conformance. General Linux and POSIX compatibility,
complete cgroups and namespaces, broad device `ioctl` support, GPU support,
real-hardware coverage, and arbitrary application compatibility remain outside
the demonstrated result. See the
[Linux compatibility matrix](docs/LINUX-COMPATIBILITY.md) for the detailed
ledger.

## Human-Directed Agentic Engineering

HDAE combines human-defined architecture, constraints, milestones, and
acceptance criteria with an agent that implements, executes, diagnoses, and
revises its work. The Ginux process was not a single prompt and was not
unconstrained "vibe coding."

The working loop was:

1. establish the research objective and interrogate it through Q&A;
2. write and continuously refine architecture and decision documents;
3. divide construction into observable milestones;
4. define executable acceptance gates before expanding scope;
5. let the agent implement, boot, test, diagnose, and iterate against those
   gates;
6. use real userspaces to expose behavior that synthetic tests missed; and
7. stop when the evidence answered the research question, not when the project
   resembled a production operating system.

Domain knowledge was essential. Human direction selected the Linux userspace
contract, separated OCI transport from runtime isolation, recognized incorrect
fork/exec and systemd behavior, established domain ownership semantics, and
decided which apparent successes were real. The agent supplied implementation
velocity and a persistent execution loop; written architecture and executable
evidence kept that loop aligned.

This method can be applied to new or existing projects of different sizes. Its
important ingredients are informed ownership, explicit design, bounded goals,
repeatable tests, and stopping criteria—not the size or novelty of Ginux.

## System description

```text
+-----------------------------------------------------------------------+
| Embedded ring-3 controller: configuration, devices, domains, lifecycle |
+-----------------------------------------------------------------------+
| Linux-personality domains: Alpine, Rocky, and other Linux userspaces   |
+-----------------------------------------------------------------------+
| Native domains: ownership | capabilities | resources | failure policy  |
+-----------------------------------------------------------------------+
| Ginux kernel: processes | VFS | networking | scheduling | memory | IPC |
+-----------------------------------------------------------------------+
| UEFI firmware and the validated x86-64 QEMU hardware profile           |
+-----------------------------------------------------------------------+
```

### Boot architecture and controller

UEFI loads one `GINUX.EFI` image containing the kernel, boot catalog, and
authenticated controller payload. Small assembly boundaries enter and leave
UEFI, install exception, interrupt, and syscall entries, and switch privilege
and address-space state; policy and most mechanisms are implemented in Rust.

The controller is part of the single EFI release unit rather than a service
installed in a host distribution. At boot, the kernel maps the statically
linked controller into ring 3. It configures the kernel and devices, discovers
host state, manages networking and persistent storage, maintains the OCI cache,
defines domains, and controls their resources, attachments, autostart, and
lifecycle. Ordinary process behavior uses the Linux ABI; protected management
operations use a versioned device/`ioctl` interface checked by the kernel.

This arrangement keeps the bootable system self-contained without running the
controller in ring 0. The controller can delegate or revoke authority granted
by the sealed boot envelope, but it cannot create new kernel authority.

### Domains and personalities

A **domain** is a native Ginux kernel object and the outer unit of isolation,
ownership, accounting, scheduling, lifecycle, and failure containment. A
domain owns or is granted:

- processes, threads, and address spaces;
- a generation-tagged capability table;
- CPU and memory policy;
- IPC objects and shared-memory grants;
- filesystem roots and mount views;
- network endpoints and device grants; and
- lifecycle and failure policy.

Domains are stronger boundaries than OCI containers or Linux namespaces. OCI
supplies an initial filesystem; Ginux supplies isolation and execution. A
single Linux-personality domain can look like a conventional computer, with
systemd as PID 1 and a distribution userspace above it. Multiple domains share
one Ginux kernel while retaining separate kernel-owned state.

A **personality** is the userspace contract visible inside a domain: executable
format, syscalls, processes, signals, VFS behavior, devices, terminals, clocks,
networking, and namespace expectations. Linux x86-64 is the only implemented
personality. Other personalities, such as BSD, remain architectural
possibilities rather than tested features.

### Filesystems, devices, and persistence

The kernel VFS presents domain-local roots assembled from verified OCI layers,
a writable overlay, and selected `tmpfs`, `procfs`, and `devfs` behavior. A
persistent host-owned volume stores controller configuration and a
content-addressed OCI cache across reboots.

The validated QEMU profile includes virtio block and network paths, MSI-X, and
VT-d DMA confinement. Measured Rust driver and protocol services can run in
separate ring-3 domains with bounded MMIO, DMA, queue, and packet authority.
General driver support, production storage guarantees, broad filesystem
coverage, and a physical-hardware matrix remain future work.

## Current demonstrated state

| Area | Demonstrated | Important boundary |
|---|---|---|
| Boot | Unified x86-64 UEFI image under Q35/OVMF | No production signing, recovery, or physical-hardware profile |
| Controller | Embedded static ring-3 administration process | No production remote management plane |
| Domains | Multiple domain objects with distinct process, address-space, root, mount, capability, and lifecycle state | Resource and device isolation remain incomplete in several paths |
| Linux userspace | Selected Alpine and Rocky programs; Rocky systemd reaches `multi-user.target` | No distribution-wide compatibility claim |
| Storage | Persistent configuration and verified OCI layer cache | Writable domain persistence and production recovery remain incomplete |
| Networking | Host DHCP/DNS, virtual-link configuration, loopback, and tested lifecycle | Independent per-domain packet paths and leases remain unfinished |
| Devices | QEMU virtio block/network, MSI-X, and VT-d paths | No GPU, NVMe, broad hardware, or stable third-party driver ABI |
| Performance | Narrow single-vCPU QEMU comparison using the same sysbench binary | Not a native, multicore, or general kernel benchmark |
| Security | Capability and generation mechanisms exercised by tests | No independent audit or production security claim |

The validated development target is QEMU's x86-64 Q35 machine with OVMF,
Intel IOMMU, and virtio devices. QEMU uses KVM on a suitable x86-64 Linux host
and TCG on Apple Silicon Macs.

## Build and run

Requirements:

- Rust 1.99.0 through `rustup`;
- Clang with the `x86_64-none-elf` target;
- QEMU with x86-64 and OVMF support; and
- GNU Make.

Check the host, build Ginux, and boot it:

```sh
make doctor
make run
```

`make run` creates a sparse development volume at `build/qemu/volume.raw`.
`make reset-storage` deletes it; the next boot recreates a clean volume.

At the `ginux>` prompt, a minimal Alpine workflow is:

```text
domain new alpine
domain set alpine image=alpine:latest init=/bin/sh
domain start alpine --attach
```

Press Control-] to detach. Reattach or execute another process with:

```text
domain attach alpine
domain exec alpine /bin/echo hello
domain exec alpine --interactive /bin/sh
```

To exercise the tested Rocky systemd path:

```text
domain new rocky
domain set rocky image=rockylinux/rockylinux:10-ubi-init init=/sbin/init memory-mib=2048
domain start rocky --attach
```

The base `rockylinux/rockylinux:10` image does not contain systemd; the tested
init-bearing image is `rockylinux/rockylinux:10-ubi-init`.

`domain start NAME` queues startup and returns to the controller. Use
`domain list` for transitional state and `domain status NAME` for detailed
root, execution, console, resource, image, and network state. OCI is only the
transport and filesystem assembly format; it is not the runtime or security
boundary.

## Verification

Useful gates include:

```sh
make build                 # build the unified EFI image
make test-linux-runtime    # ELF, loader, PIE, TLS, heap, and VM behavior
make test-alpine           # registry pull and Alpine shell
make test-rocky-shell      # Rocky glibc shell and process/file behavior
make test-multi-domain     # concurrent domain lifecycle
make test-rocky-systemd    # cold and cached Rocky systemd boot
make test-storage          # volume initialization, recovery, and reset
make check                 # architecture, ABI, format, lint, and unit checks
```

The evaluated checkpoint does **not** have a fully passing aggregate `make
test`: a post-`execve()` system-supervisor lifecycle probe remains a recorded
failure. This limitation is preserved in the paper rather than hidden behind
the narrower passing gates.

The EFI image is written to `build/esp/EFI/BOOT/BOOTX64.EFI`. Run `make help`
for the full target and environment-variable list.

## Repository guide

- [`boot/efi`](boot/efi) builds the UEFI entry and unified image.
- [`kernel`](kernel) contains the kernel, Linux personality, executable
  loaders, and QEMU hardware paths.
- [`control-plane/runtime`](control-plane/runtime) contains the embedded
  ring-3 controller.
- [`services`](services) contains isolated native-service runtimes.
- [`crates`](crates) contains shared `no_std` contracts and state machines.
- [`specs`](specs) records versioned boot, kernel, ABI, domain, image, and
  service contracts.
- [`tests`](tests) contains ABI, lifecycle, and end-to-end QEMU gates.
- [`tools`](tools) contains image, manifest, architecture, and QEMU tooling.

## Design and evidence

- [Architecture](ARCHITECTURE.md)
- [Verification roadmap](docs/ROADMAP.md)
- [Linux compatibility matrix](docs/LINUX-COMPATIBILITY.md)
- [Linux runtime compatibility](docs/LINUX-RUNTIME-COMPATIBILITY.md)
- [Runtime use cases](docs/USE-CASES.md)
- [Performance methodology](docs/PERFORMANCE.md)
- [Storage architecture](docs/STORAGE.md)
- [Stability audit](docs/STABILITY-AUDIT.md)
- [Native domains and authority](docs/decisions/0001-native-domains-and-authority.md)
- [Unified image and driver boundary](docs/decisions/0002-unified-image-and-driver-boundary.md)

## Citation

Please cite the technical report using its Zenodo record:

```bibtex
@techreport{kurtzer2026ginux,
  author = {Gregory M. Kurtzer},
  title = {Ginux: Building an Independent Kernel That Runs Linux Userspace
           through Human-Directed Agentic Engineering},
  year = {2026},
  doi = {10.5281/zenodo.23682548},
  url = {https://doi.org/10.5281/zenodo.23682548}
}
```

## License

Ginux is licensed under the [Apache License, Version 2.0](LICENSE). See
[`NOTICE`](NOTICE) for attribution information. The technical report is
published separately through Zenodo under its declared record license.
