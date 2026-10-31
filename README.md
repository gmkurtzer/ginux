# Ginux

**A Rust-first x86-64 operating system built around native domains and Linux
userspace compatibility.**

[![DOI](https://zenodo.org/badge/DOI/10.5281/zenodo.23682548.svg)](https://doi.org/10.5281/zenodo.23682548)

Ginux is an independent kernel and operating-system prototype. It boots directly
through UEFI without a Linux host kernel, provides its own memory management,
scheduler, process model, VFS, networking, device support, and Linux ABI, and
runs an embedded administration controller in ring 3. Its primary workload is
an unmodified Linux userspace running inside a kernel-native **domain**.

The repository contains the complete bootable system, controller, native
services, Linux compatibility implementation, hardware paths, build tools,
architecture specifications, and QEMU test harnesses. The current development
target is an x86-64 Q35 virtual machine with OVMF, Intel IOMMU, and virtio
devices. It runs through KVM on an x86-64 Linux host and QEMU TCG on Apple
Silicon.

Today, Ginux can:

- build as one bootable `GINUX.EFI` image;
- start its embedded controller as an ordinary ring-3 process;
- configure devices, networking, storage, and persistent domain definitions;
- pull, verify, cache, and assemble OCI images into domain-local roots;
- create and run multiple domains with separate process, address-space,
  filesystem, mount, capability, and lifecycle state;
- execute selected unmodified Alpine musl/BusyBox and Rocky Linux glibc
  programs; and
- boot Rocky Linux 10 systemd through `multi-user.target`.

Ginux is experimental. Linux ABI coverage, device support, filesystems,
networking, resource enforcement, security validation, and real-hardware
coverage are incomplete. It is useful for research, architecture experiments,
and testing—not production workloads or as a replacement for Linux.

The name is a joke in the same family as Linux: **Ginux** combines Linux, GPT,
and Greg. To keep the author's ego within its configured resource limit, the
`G` officially stands for GPT.

## Design goals

- **Linux userspace without Linux underneath.** Linux is a personality exposed
  by Ginux, not the host kernel.
- **Domains as the native system boundary.** Domains own processes, address
  spaces, filesystems, devices, resources, authority, and lifecycle.
- **A self-contained boot image.** The kernel, controller, and core boot
  requirements ship in one EFI release unit.
- **A small userspace control plane.** The embedded controller configures and
  observes the kernel without executing in ring 0.
- **Explicit, revocable authority.** Kernel capabilities grant access to named
  objects and are tied to domain generations for cleanup after failure.
- **OCI as transport, not runtime.** OCI delivers and assembles filesystems;
  Ginux domains provide execution and isolation.
- **Compatibility as an observable contract.** Real Alpine and Rocky Linux
  workloads supplement synthetic ABI tests.

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
| Performance | Matched 1/2/4-vCPU QEMU comparison using the same Rocky userspace and sysbench binary | Not a native-hardware or general kernel benchmark; four-thread scheduling remains weak |
| Security | Capability and generation mechanisms exercised by tests | No independent audit or production security claim |

The validated development target is QEMU's x86-64 Q35 machine with OVMF,
Intel IOMMU, and virtio devices. QEMU uses KVM on a suitable x86-64 Linux host
and TCG on Apple Silicon Macs.

### Linux compatibility status

The Linux 6.12 x86-64 ledger classifies all 375 syscall numbers: 82 complete,
245 partial, and 48 policy-denied. Classification means every syscall number has
an explicit disposition; it does not mean complete Linux conformance. Loader,
process, VFS, terminal, socket, IPC, namespace, cgroup, pseudo-filesystem, and
device behavior is implemented only to the extent recorded in the
[compatibility matrix](docs/LINUX-COMPATIBILITY.md).

The project uses real workloads as compound integration tests. Alpine exercises
the musl/BusyBox path. Rocky exercises glibc, dynamic linking, fork/exec, systemd,
services, procfs, terminals, and related kernel interfaces. A successful tested
workflow is evidence for that workflow only; it is not a guarantee that an
arbitrary Linux binary or distribution will run.

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
- [`skills`](skills) contains portable HDAE development and scheduled PR/issue
  review Agent Skills shared by Codex and Claude Code.

## Using the agent skills

The canonical skills live under [`skills/`](skills). Git-tracked relative
symlinks expose the same files to Codex under `.agents/skills/` and to Claude
Code under `.claude/skills/`. Start a new agent session from this repository
after checking out the branch that contains those links so the agent discovers
them at startup.

### HDAE development

Use this skill when starting or restructuring substantial work. It begins with
focused Q&A, helps the human establish product intent and architecture, creates
milestones with executable gates, and gives the agent autonomy to iterate
inside each approved gate. It stops at gate boundaries for human direction.

In Codex, start with:

```text
Use $hdae-development to help me develop this project.
```

In Claude Code, use the same request with the Claude skill command:

```text
/hdae-development Help me develop this project.
```

The user may begin with only an idea. The agent is responsible for eliciting the
structured project brief through focused follow-up questions, confirming it,
and writing it into durable project state before implementation. The skill also
requires the agent to propose complete gates, ask what is missing, wait for
approval, and update the persisted status at every stopping point. After
approving a gate, tell the agent to implement that named gate using the same
skill.

### Recurring PR and issue review

Use this skill to establish read-only reviews of new or updated GitHub issues
and pull requests. The agent discovers the repository, checks for a connected
GitHub account or authenticated `gh` CLI, guides installation or interactive
login when needed, and asks for the frequency, time/day anchor, timezone,
scope, report destination, and notification preference. It validates access
with a review before creating the schedule.

In Codex, start with:

```text
Use $pr-issue-review to set up recurring, read-only reviews of new or updated
issues and pull requests for this repository. Discover and validate the GitHub
repository and authentication, then ask me how often and when the review should
run. Preview one review before creating the schedule. Do not post, label, close,
approve, merge, commit, or push anything.
```

In Claude Code, use:

```text
/pr-issue-review Set up recurring, read-only reviews of new or updated issues
and pull requests for this repository. Discover and validate the GitHub
repository and authentication, then ask me how often and when the review should
run. Preview one review before creating the schedule. Do not post, label, close,
approve, merge, commit, or push anything.
```

Codex can use its native recurring automation when available. Claude Code uses
an available native scheduler or proposes an explicit repository workflow or
host scheduler for approval. In either environment, the skill verifies that
the unattended runtime can access the repository before creating the task; it
never asks for a token to be pasted into the conversation.

## Design and evidence

- [Architecture](ARCHITECTURE.md)
- [Verification roadmap](docs/ROADMAP.md)
- [Feedback-driven hardening research](docs/FEEDBACK-HARDENING-RESEARCH.md)
- [Linux compatibility matrix](docs/LINUX-COMPATIBILITY.md)
- [Linux runtime compatibility](docs/LINUX-RUNTIME-COMPATIBILITY.md)
- [Runtime use cases](docs/USE-CASES.md)
- [Performance methodology](docs/PERFORMANCE.md)
- [Storage architecture](docs/STORAGE.md)
- [Stability audit](docs/STABILITY-AUDIT.md)
- [Native domains and authority](docs/decisions/0001-native-domains-and-authority.md)
- [Unified image and driver boundary](docs/decisions/0002-unified-image-and-driver-boundary.md)

## Research background

Ginux was built to test whether a coding agent, directed by someone with the
relevant systems experience, could construct an independent kernel that runs
existing Linux userspace. Over eleven days, one persistent OpenAI Codex task
using GPT-5.6 Sol produced approximately 139,000 retained lines of
implementation and test code.

The development method is called **Human-Directed Agentic Engineering (HDAE)**.
Human direction established the architecture, constraints, milestones,
acceptance criteria, and stopping point. The agent implemented, booted, tested,
diagnosed, and revised the system against executable gates. Architecture and
decision documents kept the work aligned; real userspaces exposed failures that
synthetic tests missed.

The result demonstrates rapid construction of deep systems software, not a
production alternative to Linux. The project does not reproduce Linux's broad
compatibility, hardware enablement, review, vendor ecosystem, community, or
long-term maintenance.

The technical report documents the method, architecture, evaluation, and
limitations:

- [DOI: 10.5281/zenodo.23682548](https://doi.org/10.5281/zenodo.23682548)
- [LaTeX source](docs/paper/ginux-ai-systems-report.tex)
- [Steering chronology](docs/paper/STEERING-PROVENANCE.md)
- [Benchmark artifacts](docs/paper/artifact/benchmark-2026-10-23)

The paper evaluates commit
`d11c126a048a51fe7eb9c4105f31760add1027ea`, dated October 23, 2026. Later
repository changes are not silently treated as part of that experimental
checkpoint.

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
