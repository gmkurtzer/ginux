# How Ginux differs from UML, gVisor, namespaces, and VMs

Ginux is a research userspace kernel that places a configurable contract
between a Linux workload and host Linux. The category is not new. This document
explains the particular point Ginux explores, the mechanisms it currently uses,
and the boundaries it does not provide.

The questions addressed here recur in the
[Hacker News discussion](https://news.ycombinator.com/item?id=50450007) about
the project. Technical comparisons below use the projects' own documentation.

## The problem Ginux explores

Linux maintains a broad, stable userspace ABI because general-purpose software
depends on it. That commitment is essential for Linux, but a particular
workload often needs much less authority. Ginux asks whether an operator can
describe that smaller interface as one portable, reviewable instance recipe:

- which syscalls and ioctls may execute;
- which filesystem paths, devices, sockets, and resources are visible;
- which operations are read-only, writable, virtualized, mediated, or denied;
- what `/proc`, `/sys`, process identities, machine identity, and lifecycle the
  workload observes; and
- which named workload and tests establish compatibility with that contract.

This is deliberate ABI reduction, not accidental Linux incompatibility. Ginux
does not promise that an arbitrary Linux program will run.

## How the current implementation runs code

The balanced backend uses host Linux to load native x86-64 ELF files, create
address spaces and tasks, handle page faults and signals, schedule threads, and
execute instructions on the host CPU. Ginux is an ordinary trusted host process
attached to those tasks through ptrace.

Each managed task installs a seccomp filter. Except for the current selective
`sched_yield` optimization, the filter produces a ptrace seccomp event before a
requested syscall reaches its host implementation. Ginux reads the syscall
number, registers, and any required managed memory, resolves the compiled
instance contract, and then does one of the following:

1. **Emulate** the result from Ginux-owned state.
2. **Mediate** a validated, translated operation through narrow Ginux-owned host
   authority.
3. **Deny** the request without executing the requested host syscall.

In selective mode, contract-approved `sched_yield` may currently execute
directly. In paranoid mode it is handled through seccomp user notification so
every managed syscall receives a Ginux userspace decision. The implementation
retains ptrace for task creation, exits, signals, virtual process identity, and
the rest of the mediated syscall surface.

This design means Ginux is not a hardware virtual machine. Host Linux, ptrace,
seccomp, ELF loading, address spaces, page faults, scheduling, signals, and CPU
execution remain trusted. The exact supported claims and retained authority are
listed in [Security boundary and limitations](security-boundary.md).

## Why Ginux is not User-Mode Linux

[User-Mode Linux (UML)](https://docs.kernel.org/virt/uml/user_mode_linux_howto_v2.html)
runs a complete Linux kernel as an ordinary Linux process. Its userspace talks
to that Linux kernel, and the UML kernel operates paravirtual devices that map
to host files, sockets, pipes, and other resources.

Ginux does not run a guest Linux kernel and does not present virtual hardware
to one. It implements only the contract selected for an instance and delegates
native execution services to the host. UML's reuse of Linux is valuable when
the goal is a complete Linux kernel environment. Ginux instead makes omission
and incompatibility explicit design tools.

## Why namespaces and seccomp are not the same layer

Linux namespaces partition particular host-kernel objects. Cgroups account for
and limit resources. Seccomp filters host syscalls. These are useful mechanisms,
and Ginux relies on host primitives internally, but the workload still uses the
host kernel's System API for everything left reachable.

Ginux adds a userspace System-API layer. It can synthesize a virtual result,
translate an operation, maintain instance-specific kernel state, or reject an
operation before its requested host implementation. A namespace can give two
processes different PID views; Ginux maintains virtual process identities and
generates its own `/proc` objects as part of the same versioned contract that
governs syscalls, devices, networking, and lifecycle.

That additional layer is not automatically safer. Its implementation is more
code that must be reviewed and tested, and the host services it still uses
remain in the trusted computing base.

## gVisor is the closest established comparison

[gVisor describes itself](https://gvisor.dev/docs/architecture_guide/intro/)
as an application kernel rather than a hypervisor or syscall filter. Like
Ginux, it captures application interactions and implements kernel-facing
behavior outside the host's ordinary application syscall path. gVisor is
therefore the closest established architectural relative.

The projects are not equivalent:

- gVisor has years of production engineering and implements a broad Linux
  System API for application and container compatibility;
- gVisor has distinct
  [KVM, systrap, and ptrace platforms](https://gvisor.dev/docs/architecture_guide/platforms/)
  for address spaces and syscall interception;
- Ginux currently has a much smaller verified compatibility surface and a
  ptrace/seccomp native backend; and
- Ginux's research focus is a self-contained, workload-specific contract that
  deliberately grants less than the full Linux ABI and unifies that decision
  with virtual state, resources, lifecycle, and audit evidence.

This is differentiation in emphasis and experiment design, not a claim that
Ginux invented application kernels or currently matches gVisor's compatibility,
isolation, maturity, or performance.

## Why Ginux is not a VM or microVM

A VM or microVM runs a guest kernel behind a hardware virtualization boundary.
The guest application normally reaches the guest kernel first, and the
hypervisor handles a smaller set of exits from that guest environment.

Ginux has no guest kernel and no hardware-enforced guest boundary. Its native
workload shares host-kernel execution services under userspace supervision.
That can avoid guest hardware and kernel machinery, but it also leaves a larger
trusted host surface and provides different isolation semantics. Current Ginux
evidence must not be represented as VM-equivalent containment.

## What is distinctive about Ginux

The distinctive claim is intentionally narrow and testable:

1. **One self-contained contract.** A portable instance recipe combines ABI
   policy with virtual filesystem, device, network, process, resource,
   identity, lifecycle, and audit requirements.
2. **Compatibility is named, not assumed.** Ginux claims only that a specified
   workload passes a specified executable gate under a specified contract.
3. **Incompatibility is purposeful.** Unsupported Linux behavior may be denied
   because reducing the exposed contract is the objective, not merely missing
   implementation work.
4. **Policy and virtual behavior share one decision model.** A request may be
   denied, translated, or answered from Ginux-owned state rather than merely
   allowed through to host Linux.
5. **The prototype is itself research evidence.** HDAE made it practical to
   build, measure, discard, and redirect executable system designs. AI use is
   not evidence of correctness; the architecture, tests, limitations, and
   human review determine what the project can claim.

Ginux remains research software. It has not received independent security
review, does not implement the complete Linux ABI, and is not ready for
production workloads.
