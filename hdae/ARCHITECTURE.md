# Ginux Architecture

## System boundary

```text
Host Linux kernel
  `- Rocky Linux host systemd: real host PID 1
       `- ginux: trusted ordinary host process and userspace kernel
            `- domain
                 |- versioned contract and normalized configuration
                 |- virtual CPU contexts and address spaces
                 |- virtual processes, descriptors, devices, and kernel views
                 `- Rocky Linux systemd: virtual domain PID 1
                      `- managed services and processes
```

The host boots and operates normally without Ginux replacing its init. Ginux
remains resident after an operator or host service starts it; it does not
install restrictions and then disappear. Domain systemd and its descendants
execute as managed code and see a Linux-compatible interface implemented by
Ginux.

## Security invariants

1. No managed syscall instruction may complete as an unmediated host syscall.
2. Every managed syscall stops before execution and enters one central Ginux
   dispatch path before any host effect occurs.
3. Dispatch first resolves the domain contract, then emulates, performs a
   mediated host operation, or denies the request.
4. Managed pointers, structures, strings, and ioctl payloads are copied through
   the active backend's controlled memory-access path and validated before use.
5. Security-profile descriptor numbers resolve only to Ginux objects. The
   balanced native profile may use kernel descriptors solely for immutable OCI
   image content and an explicitly assigned bootstrap console inside a private
   domain filesystem view. Their use remains syscall-mediated; supervisor or
   unrelated host descriptors and host paths are never disclosed.
6. `/dev`, `/proc`, `/sys`, cgroups, mounts, process identities, and machine
   identity are derived from Ginux state.
7. Device assignment is explicit and exclusive by default. Domain teardown
   revokes its authority and releases its resources.
8. Direct device memory, MMIO, DMA, and unknown ioctls are unavailable unless a
   later contract explicitly introduces a separately reviewed safe mechanism.
9. Invalid contracts and unsupported security-critical configuration fail
   closed.
10. Benchmark optimizations may change implementation but not these invariants.

## Major components

### Host integration

Ginux is an ordinary Rocky host executable. It validates host prerequisites
and domain configuration, initializes logging and recovery behavior, opens
only the host resources needed by configured backends, and starts the domain.
Host systemd may supervise Ginux as a service, but manual invocation is the
Gate 1 baseline. Ginux failure stops its domains without replacing or shutting
down the host init system.

### Execution platform

Development and verification run on a macOS arm64 host through QEMU TCG
emulating a normally booted Rocky Linux 10 x86-64 host. Host systemd remains
real PID 1 and Ginux runs as an ordinary process inside that environment.
Ginux acts as the application kernel for its managed userspace rather than
booting another guest Linux kernel beneath the domain. Gate 5 uses native
x86-64 execution supervised by ptrace syscall-entry and syscall-exit stops: the
host kernel loads the ELF and its interpreter, the host CPU executes
instructions, and every managed syscall stops before its kernel implementation
runs. Ginux may then emulate or deny it by replacing the syscall before effect,
or allow a validated operation to execute and observe its exit. Losing
supervision terminates the tracee and fails the domain closed. Gate 1's
`PTRACE_SYSEMU` proof remains the strict no-host-effect interception fixture;
it cannot by itself execute an approved syscall.

This backend intentionally trusts more host-kernel surface than a userspace CPU
interpreter. ELF loading, virtual memory, page faults, signals, scheduling, and
ptrace remain host-kernel responsibilities and possible attack surface. The
Gate 3 handcrafted interpreter remains evidence for a stricter profile but is
not the Gate 5 compatibility path. A Unicorn Engine backend is explicitly
deferred as the alternative path for returning CPU and memory execution to
Ginux while retaining the same contract dispatcher. KVM acceleration remains
outside the initial milestone.

Execution profile and syscall action are separate choices:

- **Security:** Ginux interprets instructions; syscalls are emulated, proxied,
  or denied, and descriptors remain Ginux-virtual.
- **Balanced:** the host executes instructions under ptrace. A minimal,
  validated set of read-only loader, immutable-image, memory, signal, and
  bootstrap-console operations may continue inside a private domain view.
  Writable storage, devices, networking, ioctls, credentials, and virtual
  kernel state are emulated, proxied through Ginux objects, or denied.
- **Performance:** native execution may use a broader validated passthrough set,
  but only after explicit configuration, benchmarks, and security review.

Gate 5 implements the balanced profile. Backend selection may later be made per
process, but mixed interpreter/native process semantics are not part of this
gate.

### Domain memory and task model

Ginux owns the domain's virtual identities, descriptors, devices, contract, and
lifecycle. With native ptrace supervision, host tracees provide address spaces
and tasks under continuous Ginux supervision; their host identities and
addresses are never exposed as domain authority. Ginux copies and validates
tracee memory before mediation. In interpreter backends, managed virtual
addresses instead index only Ginux-owned memory and permission metadata.

### Contract dispatcher

Every syscall and ioctl resolves through a versioned domain contract. A
decision has one of these effects:

- **Emulate:** answer entirely from Ginux state.
- **Mediate:** validate and translate the request, then use a narrow
  Ginux-owned host operation.
- **Deny:** return the configured Linux-compatible error or terminate the
  managed task when policy requires it.
- **Audit:** record the decision, relevant normalized arguments, result, domain
  identity, and contract version without leaking secrets.

There is no unchecked direct-continue action. Balanced and performance
passthrough are explicit contract decisions after argument and authority
validation, and their exits are observed and audited.

### Virtual VFS and image backend

The initial domain root is a digest-pinned Rocky Linux 10 OCI image. An explicit
acquisition operation pulls and verifies its manifest, configuration, and
layers into a host-owned cache outside Git. Ginux deterministically assembles
the layers into an immutable indexed virtual root and adds a bounded,
domain-local writable overlay. The OCI configuration supplies launch metadata,
but Ginux—not `runc`, `crun`, `containerd`, or another OCI runtime—constructs
the execution view and supervises native loading through ptrace syscall stops.
Managed code
receives neither host paths nor host descriptors. Ordinary domain starts and
tests require no network access.

### Virtual kernel filesystems

- `/dev` resolves names to Ginux device objects.
- `/proc` describes Ginux-managed processes and domain state.
- `/sys` describes only contract-visible virtual devices and attributes.
- `/sys/fs/cgroup` provides the cgroup v2 behavior required by the validated
  systemd baseline.

Host trees are not bind-mounted into the domain.

### Device model

A domain configuration assigns device frontends to Ginux-controlled backends.
Initial device classes include console and pseudo-devices, a file-backed block
device or bounded partition view, and a virtual network interface. Ginux owns
backend handles and mediates reads, writes, ioctls, mappings, and lifecycle.

Raw host block-device assignment, physical-device passthrough, VFIO, GPU
support, direct MMIO, and DMA are later architecture work. They may not be
introduced as ordinary configuration extensions because they change the threat
model.

Gate 6's initial network backend is a bounded Ginux-owned packet queue. It
provides deterministic send/receive and lifecycle semantics without opening a
host socket or TUN/TAP descriptor and deliberately has no external
connectivity. A later connectivity backend must preserve the same virtual
interface and explicit packet-mediation boundary.

Gate 9 adds the first externally connected backend. Its default `shared` mode
uses the host network and normal host routing, source-address selection, and
socket namespace. With no network configuration, domain sockets behave like
other host applications: outbound and inbound operations are available, and
normal bind conflicts apply, so the first successful bind owns an address and
port. Ginux still intercepts, validates, mediates, and audits every managed
network operation; managed code does not receive unchecked supervisor
descriptors or an unmediated syscall path.

Gate 9.5 adds explicit selection of a host interface or source address,
ingress and egress policy, and port translation such as host
`device-or-address:8080` to domain port `80`. Selecting an attachment such as
`eth0:1` gives the domain the normal shared use of that complete host network
attachment; it does not reserve the attachment or create an isolated network.
Two domains attached to the same host network therefore behave like two host
applications, including first-successful-bind ownership. Translation is the
explicit exception: Ginux owns the selected external address and port and
routes it to the configured domain port. These controls use the same network
frontend rather than creating a separate compatibility path.

### Configuration and lifecycle

Each domain has a versioned configuration containing identity, root image,
init, environment, resource limits, execution profile, contract profile,
storage, devices, networking, audit behavior, and failure policy. The
execution profile makes the performance/security tradeoff and backend trust
boundary explicit. Ginux validates and normalizes the complete configuration
before starting managed code. The effective configuration is immutable
evidence for that domain instance; mutable runtime controls, if later added,
require explicit authorization and audit semantics.

Gate 9.5 expands this into the comprehensive domain specification. It includes
hostname, access, volumes and host mappings, network attachment and port
policy, resources, devices, contract and audit behavior, failure policy, and
pre-boot hooks with an explicitly defined execution context. Command-line
arguments may override file values through one typed normalization and
validation path. Precedence is built-in defaults, then the configuration file,
then command-line overrides. The normalized effective configuration is
recorded before domain startup.

### Observability and benchmarking

Audit events expose contract decisions and domain lifecycle without exposing
host authority. Benchmarks compare identical workloads natively, in a
conventional container, and under Ginux. The suite covers CPU, memory, syscall
latency, process operations, filesystem and block I/O, networking, boot time,
and idle overhead.

## Important open decisions

- Numeric performance budgets, set from reproducible baseline evidence before
  Gate 11 begins.
- Whether each pre-boot hook runs in the trusted host context or the mediated
  domain context, and the authority available to each hook. Gate 9.5 must
  resolve this before hook execution is implemented.

Changing a security invariant or resolving an open decision with material
architectural impact requires human review and an update to the affected open
gate before implementation continues.

Gate 7.5 performed the transitional project rename and selected `ginux` as the
command-line executable. Gate 7.75 superseded the transitional product and
repository name with Ginux after the full kernel project pivoted to this
userspace-kernel architecture. Gate 8.5 joins that accepted implementation to
the original Ginux repository without changing its source tree. The final
standalone kernel remains on `bootable-kernel` and at
`bootable-kernel-final`; both histories are parents of the continuing
userspace-kernel line. The human approved preserving accepted gate records and
existing Git history as immutable lineage evidence on 2026-11-04.

Gate 4 selected strict, versioned TOML as the operator-facing domain
configuration format with human approval on 2026-11-03.

Gate 5 selected a digest-pinned OCI image as the domain root filesystem, with
Ginux acting as the userspace kernel and runtime. The image is pulled only by
an explicit acquisition operation, cached outside Git, assembled into an
immutable indexed virtual root, and paired with a bounded in-memory write
overlay. The host continues to boot independently from its Rocky cloud QCOW2.
Human approval was recorded on 2026-11-03.

Gate 5 selected native host ELF execution under continuous ptrace supervision
as its dynamic userspace path with human approval on 2026-11-03. Experimentation
showed that approved passthrough syscalls require syscall-entry/exit supervision;
`PTRACE_SYSEMU` skips the syscall even when followed by `PTRACE_SYSCALL`. The
host execution surface is an explicit limitation. Unicorn Engine remains a
deferred alternative backend, not a discarded design.

Gate 5 selected the balanced execution profile with human approval on
2026-11-03. Native tracees may hold kernel descriptors only for immutable image
content and an assigned bootstrap console within a private domain filesystem
view. Security-sensitive interfaces remain virtualized, proxied, or denied.
