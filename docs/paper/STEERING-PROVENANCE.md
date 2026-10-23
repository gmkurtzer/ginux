# Ginux Steering and Decision Provenance

This document is a curated companion to the Ginux technical report. It records
how human direction and Codex implementation interacted during the research.
It is not a verbatim transcript and does not claim access to private model
reasoning. Its evidence is the user-visible conversation, repository history,
versioned architecture and planning documents, and executable tests.

## Provenance categories

- **Human-specified:** the governing requirement or constraint came directly
  from the project lead.
- **Agent-proposed:** Codex selected or proposed the mechanism within existing
  requirements.
- **Collaboratively refined:** the result emerged through proposals,
  questions, corrections, and acceptance across several turns.
- **Test-driven correction:** observed behavior caused the project lead to
  reject a claimed milestone or change the next gate.
- **Implementation detail:** Codex chose the concrete data structures,
  algorithms, assembly boundaries, or test mechanics without line-by-line
  human instruction.

These categories describe decision provenance, not source-code authorship.
Codex produced nearly all implementation changes, while human direction set
the objective, corrected the architecture, evaluated behavior, and decided
when the research had demonstrated enough.

## Collaboration sequence

### 1. Research objective and initial architecture

The initial question was whether an AI agent could construct a simple, fast,
secure kernel while preserving enough Linux userspace compatibility to run a
real distribution. Rocky Linux 10 on x86-64 under QEMU/KVM was selected as the
eventual compatibility target. The project lead required a stable kernel ABI,
modular nonessential drivers, graceful driver-failure containment, runtime
immutability, and hard privilege boundaries.

The architecture was developed through Q&A before implementation. The early
container-native concept was refined into a very small host with an embedded
controller and kernel-native workload boundaries. The controller was required
to configure and monitor the system without becoming kernel code.

Evidence:

- `ARCHITECTURE.md`
- Commit `28c79c1` (`Define container-native system architecture`)
- Commit `d3692e2` (`Bootstrap x86-64 UEFI development flow`)

### 2. Unified EFI boot and the privilege boundary

The project lead proposed investigating whether initial userspace could be
embedded with the kernel rather than delivered through a separate initramfs.
The resulting design packaged the kernel and controller in one EFI artifact,
but launched the controller as an ordinary ring-3 process. Later discussion
explicitly rejected a private controller execution ABI for normal process
behavior: the controller should be a static Linux-compatible userspace binary,
with Ginux-specific operations limited to a versioned management interface.

Codex implemented UEFI entry, memory discovery, page tables, protection
structures, exceptions, APIC timing, the first isolated address space, and
ring-3 entry. The project lead specified the boundary and acceptance behavior;
the low-level mechanisms were agent-selected implementation details.

Evidence:

- Commits `97f778a` through `ab89034`
- `ARCHITECTURE.md`, boot and controller sections

### 3. Native domains rather than renamed containers

The project lead required domains to be natural kernel objects rather than
Linux namespaces or OCI containers. Each domain was expected to own or receive
its process tree, address spaces, mounts, root filesystem, IPC, credentials,
networks, devices, limits, and failure policy. Domains were also required to
serve as resource-policy principals so latency-sensitive work could receive
different CPU or I/O treatment from scavenger workloads.

Codex translated those requirements into generation-tagged capabilities,
domain-owned frames and address spaces, bounded IPC, resource admission,
revocation, service restart, and scheduler state. The exact capability layouts,
generation checks, and cleanup protocols were agent-proposed mechanisms.

Evidence:

- `docs/decisions/0001-native-domains-and-authority.md`
- Commits beginning with `ed317c3`, `cdc55a7`, and `18373df`
- `tests/qemu/` domain and fault-containment gates

### 4. Control paths, fast paths, and shared pages

The requirements combined strong authority boundaries with an explicit goal of
high performance. Codex proposed separating authorization from repeated data
movement: a protected control path establishes grants, after which an approved
data path may use shared mappings or hardware queues. This became the sentence
in the report that prompted the provenance question.

That mechanism was not dictated verbatim by the project lead. It was an
agent-derived design within human-specified constraints for speed, security,
revocation, and domain ownership. It was retained through architecture review
and recorded in the domain authority decision.

Evidence:

- `docs/decisions/0001-native-domains-and-authority.md`, consequence 10
- Frame-ownership, capability-revocation, DMA, and memory-isolation commits

### 5. Controller and operator experience

The project lead repeatedly shaped the controller by using it. Requirements
included an intuitive command tree, persistent controller lifetime, shell
editing, tables, domain configuration defaults, multiple key/value settings,
network device management, attach/detach behavior, domain execution, and
nonblocking detached startup. Controller crashes, short command buffers,
singleton state, incorrect PID display, and foreground output during detached
startup were rejected as milestone failures.

Codex implemented the parser, help system, lifecycle commands, terminal input,
tables, background dispatch, and management interfaces. The workflow was
human-directed; its implementation was delegated.

Evidence:

- Commits beginning with `7b88e5b`, `2e0411f`, `c3ce00c`, and `fbcc177`
- `docs/USE-CASES.md`
- Controller QEMU tests

### 6. OCI as transport, not runtime

The project lead corrected language that treated OCI as the execution runtime.
OCI was limited to registry resolution, architecture selection, verified image
content, and initial filesystem assembly. The Ginux domain remained the
runtime, isolation, scheduling, and authority boundary.

The image pipeline was also required to pull automatically on domain start,
show useful layer progress, select the native architecture, cache content
persistently, and avoid a build-pinned Alpine image. Codex implemented registry
authentication, TLS, manifest selection, digest verification, layer unpacking,
cache restoration, and domain-root publication.

Evidence:

- `ARCHITECTURE.md`, OCI and domain sections
- OCI integration and persistence tests
- `docs/NEXT-GOAL.md`

### 7. Compatibility discovered through real userspace

Compatibility was not declared from a syscall count alone. The progression was
deliberate:

1. Small static ELF and syscall probes.
2. An Alpine shell and BusyBox filesystem behavior.
3. Fork, exec, pipes, signals, terminals, procfs, and dynamic linking.
4. Rocky Linux glibc programs and larger mappings.
5. Rocky systemd as domain PID 1.
6. Service startup, an interactive console, stop/restart, and `domain exec`.

Operator tests repeatedly exposed false success. An Alpine shell that started
but could not list its root revealed filesystem and fork/exec defects. Failure
to start a second image revealed singleton runtime state. Systemd warnings and
stalls exposed missing procfs, mount, socket, epoll, timer, credential, and
terminal behavior. Each finding became a new implementation task and, where
possible, an automated regression gate.

Evidence:

- `docs/LINUX-COMPATIBILITY.md`
- `docs/USE-CASES.md`
- `docs/NEXT-GOAL.md`
- Linux ABI, Alpine, Rocky, multi-domain, and systemd tests

### 8. Domain privilege and nested operation

The project lead rejected container-style policy denials for operations such as
mounting. The domain boundary was intended to be stronger than a conventional
container boundary, allowing a Linux system inside a domain to use mounts,
namespaces, and eventually nested container and virtualization facilities
without receiving host authority. UID 0 was therefore authoritative inside the
Linux domain but not over the Ginux host.

Codex implemented the subset exercised by the current Rocky and systemd gates
and documented the remaining namespace, cgroup, device, and KVM gaps. The
desired authority model was human-specified; coverage and implementation were
agent work evaluated against real userspace.

Evidence:

- `docs/decisions/0001-native-domains-and-authority.md`
- `ARCHITECTURE.md`, Linux compatibility profile
- `docs/LINUX-COMPATIBILITY.md`

### 9. Performance, regression, and stopping decisions

The project lead required actual QEMU validation and repeatedly rejected slow,
blocking, or misleading behavior. Examples included attached shells becoming
unusably slow, systemd taking minutes, detached starts blocking the controller,
and orphaned QEMU processes degrading the development host. Codex profiled the
paths, added timings and regression tests, and iterated on scheduling,
filesystem publication, runtime dispatch, and cleanup.

The final stopping point was also a human decision. The compatibility campaign
could continue indefinitely; the project was frozen when it had demonstrated
the research claims while still documenting substantial production gaps. This
is why the report describes feasibility rather than completion.

Evidence:

- `docs/NEXT-GOAL.md`
- Performance and lifecycle tests under `tests/qemu/`
- Frozen commit identified in the paper

## Responsibility boundary

The collaboration can be summarized as follows:

| Responsibility | Primary source |
|---|---|
| Research question and acceptable outcome | Human direction |
| Architecture constraints and major corrections | Human direction with iterative Q&A |
| Concrete kernel mechanisms and implementation | Codex |
| Compilation, QEMU execution, log analysis, and regression automation | Codex |
| Interactive operator evaluation and rejection of false success | Human direction |
| Documentation drafts and compatibility ledgers | Codex, under review |
| Production-readiness judgment and stopping point | Human direction |

This was neither autonomous kernel generation nor line-by-line human
programming. It was expert-directed agentic engineering: the agent received
real authority over implementation and iteration, while an experienced system
designer supplied the model of correctness and repeatedly tested whether the
artifact matched it.

## Publication note

A future public supplement can include the complete user-visible conversation
export. That transcript should be published separately from the paper because
of its size, with timestamps and tool output preserved where available. This
curated document is intended to make the provenance legible without requiring
readers to search the entire conversation.
