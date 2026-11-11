# Ginux Architecture

## System boundary

```text
Host Linux kernel
  `- Rocky Linux host systemd: real host PID 1
       `- ginux: trusted ordinary host process and userspace kernel
            `- instance
                 |- versioned contract and normalized configuration
                 |- virtual CPU contexts and address spaces
                 |- virtual processes, descriptors, devices, and kernel views
                 `- Rocky Linux systemd: virtual instance PID 1
                      `- managed services and processes
```

The host boots and operates normally without Ginux replacing its init. Ginux
remains resident after an operator or host service starts it; it does not
install restrictions and then disappear. Instance systemd and its descendants
execute as managed code and see a Linux-compatible interface implemented by
Ginux.

## Security invariants

1. Every managed syscall is governed by the immutable instance contract and
   selected dispatch mode before any host effect occurs.
2. Paranoid dispatch stops every managed syscall before execution and enters
   the central Ginux userspace dispatcher. Selective dispatch may directly
   execute only operations that are both explicitly allowed by the effective
   contract and cataloged by Ginux as semantically transparent and direct-safe.
3. Denied operations and operations requiring inspection, translation,
   emulation, virtual state, pointer or descriptor validation, or modified
   results always stop before execution and enter the central Ginux dispatcher.
   Ginux returns configured denials without invoking the requested host syscall.
4. Managed pointers, structures, strings, and ioctl payloads are copied through
   the active backend's controlled memory-access path and validated before use.
5. Security-profile descriptor numbers resolve only to Ginux objects. The
   balanced native profile may use kernel descriptors solely for immutable OCI
   image content and an explicitly assigned bootstrap console inside a private
   instance filesystem view. Their use remains syscall-mediated; supervisor or
   unrelated host descriptors and host paths are never disclosed.
6. `/dev`, `/proc`, `/sys`, cgroups, mounts, process identities, and machine
   identity are derived from Ginux state.
7. Device assignment is explicit and exclusive by default. Instance teardown
   revokes its authority and releases its resources.
8. Direct device memory, MMIO, DMA, and unknown ioctls are unavailable unless a
   later contract explicitly introduces a separately reviewed safe mechanism.
9. Invalid contracts and unsupported security-critical configuration fail
   closed.
10. Benchmark optimizations may expand the reviewed catalog of direct-safe
    operations, but may not infer direct execution merely from an allow rule or
    weaken paranoid dispatch, denial, or complex-operation mediation.

## Major components

### Host integration

Ginux is an ordinary Rocky host executable. It validates host prerequisites
and instance configuration, initializes logging and recovery behavior, opens
only the host resources needed by configured backends, and starts the instance.
Host systemd may supervise Ginux as a service, but manual invocation is the
Gate 1 baseline. Ginux failure stops its instances without replacing or shutting
down the host init system.

### Execution platform

Development and verification run on a macOS arm64 host through QEMU TCG
emulating a normally booted Rocky Linux 10 x86-64 host. Host systemd remains
real PID 1 and Ginux runs as an ordinary process inside that environment.
Ginux acts as the application kernel for its managed userspace rather than
booting another guest Linux kernel beneath the instance. Gate 5 uses native
x86-64 execution supervised by ptrace syscall-entry and syscall-exit stops: the
host kernel loads the ELF and its interpreter and the host CPU executes
instructions. Gate 11 adds configurable dispatch. Paranoid mode retains a
pre-effect Ginux userspace stop for every syscall. Selective mode compiles the
effective contract together with Ginux's versioned direct-safety
classification: only explicitly allowed, transparent operations execute
directly; denied and complex operations still stop in Ginux before their host
implementation. Losing required supervision terminates the tracee and fails
the instance closed. Gate 1's
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
  bootstrap-console operations may continue inside a private instance view.
  Writable storage, devices, networking, ioctls, credentials, and virtual
  kernel state are emulated, proxied through Ginux objects, or denied.
- **Performance:** native execution may use a broader validated passthrough set,
  but only after explicit configuration, benchmarks, and security review.

Dispatch mode is an independent per-instance choice:

- **Paranoid:** every syscall enters the Ginux userspace dispatcher before it
  can complete. This maximizes inspection and sanitization opportunities and
  preserves the original all-syscall mediation model at its measured cost.
- **Selective:** a syscall executes directly only when the effective contract
  allows it and the versioned Ginux catalog marks it direct-safe. Denials and
  operations requiring inspection, translation, emulation, or virtual state
  remain userspace-dispatched. An allow rule alone never creates a direct path.

Selective is the default to minimize mediation overhead. Paranoid must be
selected explicitly when the operator requires every syscall to enter Ginux
userspace before completion.

Gate 5 implements the balanced profile. Backend selection may later be made per
process, but mixed interpreter/native process semantics are not part of this
gate.

### Instance memory and task model

Ginux owns the instance's virtual identities, descriptors, devices, contract, and
lifecycle. With native ptrace supervision, host tracees provide address spaces
and tasks under continuous Ginux supervision; their host identities and
addresses are never exposed as instance authority. Ginux copies and validates
tracee memory before mediation. In interpreter backends, managed virtual
addresses instead index only Ginux-owned memory and permission metadata.

### Contract dispatcher

Every syscall and ioctl is governed by a versioned instance contract. The
compiled dispatch plan first chooses direct-safe execution or Ginux userspace
dispatch according to the configured dispatch mode. A userspace-dispatched
decision has one of these effects:

- **Emulate:** answer entirely from Ginux state.
- **Mediate:** validate and translate the request, then use a narrow
  Ginux-owned host operation.
- **Deny:** return the configured Linux-compatible error or terminate the
  managed task when policy requires it.
- **Audit:** record the decision, relevant normalized arguments, result, instance
  identity, and contract version without leaking secrets.

There is no inferred direct-continue action. Selective direct execution requires
both an explicit contract allow and a versioned catalog classification proving
that the operation needs no argument inspection, authority validation,
translation, emulation, virtual result, or synchronous Ginux audit. Paranoid
mode disables the direct path. Denials are made and recorded by Ginux without
executing the requested host syscall.

### Virtual VFS and image backend

The initial instance root is a digest-pinned Rocky Linux 10 OCI image. An explicit
acquisition operation pulls and verifies its manifest, configuration, and
layers into a host-owned cache outside Git. Ginux deterministically assembles
the layers into an immutable indexed virtual root and adds a bounded,
instance-local writable overlay. The OCI configuration supplies launch metadata,
but Ginux—not `runc`, `crun`, `containerd`, or another OCI runtime—constructs
the execution view and supervises native loading through ptrace syscall stops.
Managed code
receives neither host paths nor host descriptors. Ordinary instance starts and
tests require no network access.

### Virtual kernel filesystems

- `/dev` resolves names to Ginux device objects.
- `/proc` and `/sys` are Ginux-owned, in-memory object models. Lookups,
  directory entries, metadata, links, and file contents are generated on
  demand from current instance state; no virtual kernel nodes are staged in
  the instance root or persisted between starts.
- `/proc` describes only live Ginux-managed processes and instance state.
- `/sys` describes only contract-visible virtual devices and attributes.
- `/sys/fs/cgroup` provides the cgroup v2 behavior required by the validated
  systemd baseline.

Host trees are not bind-mounted into the instance.

### Device model

An instance configuration assigns device frontends to Ginux-controlled backends.
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
socket namespace. With no network configuration, sockets owned by an instance behave like
other host applications: outbound and inbound operations are available, and
normal bind conflicts apply, so the first successful bind owns an address and
port. Ginux still intercepts, validates, mediates, and audits every managed
network operation; managed code does not receive unchecked supervisor
descriptors or an unmediated syscall path.

Gate 9.75 adds explicit selection of a host interface or source address,
ingress and egress policy, and port translation such as host
`device-or-address:8080` to instance port `80`. Selecting an attachment such as
`eth0:1` gives the instance the normal shared use of that complete host network
attachment; it does not reserve the attachment or create an isolated network.
Two instances attached to the same host network therefore behave like two host
applications, including first-successful-bind ownership. Translation is the
explicit exception: Ginux owns the selected external address and port and
routes it to the configured instance port. These controls use the same network
frontend rather than creating a separate compatibility path.

### Configuration and lifecycle

Each instance has a versioned configuration containing identity, root image,
init, environment, resource limits, execution profile, dispatch mode, contract
profile, storage, devices, networking, audit behavior, and failure policy. The
execution profile makes the performance/security tradeoff and backend trust
boundary explicit. Ginux validates and normalizes the complete configuration
before starting managed code. The effective configuration is immutable
evidence for that instance; mutable runtime controls, if later added,
require explicit authorization and audit semantics.

Gate 9.75 expands this into the comprehensive instance specification. It includes
hostname, access, volumes and host mappings, network attachment and port
policy, resources, devices, contract and audit behavior, failure policy, and
pre-boot hooks with an explicitly defined execution context. Command-line
arguments may override file values through one typed normalization and
validation path. Precedence is built-in defaults, then the configuration file,
then command-line overrides. The normalized effective configuration is
recorded before instance startup.

Gate 9.75 pre-boot hooks execute only as managed instance processes after the
instance root and declarative resources have been prepared but before the instance
init process begins. Hooks use the same mediated runtime, effective contract,
virtual identity, filesystem view, devices, and network policy as the instance;
they receive no host descriptors, paths, credentials, or helper authority.
Host-side setup remains declarative and is performed only by reviewed Ginux
backends. Arbitrary host-context hooks are unsupported because they would be
root-authority extensions to the trusted computing base rather than protection-domain
configuration.

Gate 10.5 separates virtual authority from the host execution backstop. Ginux
continues to own instance-visible UIDs, GIDs, groups, and permission decisions.
The native tracee itself always sets `PR_SET_NO_NEW_PRIVS`; when its runtime was
started as host root it drops to host UID/GID 65534, and otherwise it retains
the non-root caller's host UID/GID. Host credentials therefore cannot grant
instance authority and provide a second containment boundary if mediation is
imperfect.

Gate 10.5 also gives trusted pre-boot commands a distinct, bounded bootstrap
phase. They execute through the managed syscall path inside the instance root,
with no host paths, descriptors, credentials, or unmediated operations. Final
workload path restrictions activate only after all hooks complete, allowing the
recipe to prepare the state that it subsequently makes read-only. One denied
absolute directory path covers that directory and its descendants using
component-boundary matching and confined path resolution.

Gate 12 separates portable policy from deployment-specific network state. The
instance recipe owns a hierarchical `[contract]` describing syscall,
filesystem, network, device, privilege, process, and synthetic-kernel-view
authority. Host interface selection and external-to-instance port translation
are not recipe fields; repeatable top-level `create`, `import`, and `set` flags
persist them as operational instance state. The instance remains the managed
object, but the CLI does not add a redundant `instance` command group. With no
explicit attachment, the instance uses the host default network. `/proc` and
`/sys` remain synthetic and Ginux-owned regardless of policy; policy controls
visibility and operations, not passthrough to host kernel trees.

Gate 12.1 makes that synthetic boundary concrete in the native runtime. The
instance root contains only empty `proc` and `sys` mountpoint directories.
Every visible directory entry, link, metadata record, descriptor, offset, and
file payload is a Ginux-owned in-memory object generated from current virtual
instance state when accessed. No host procfs or sysfs path is opened to satisfy
an instance operation, and unsupported nodes fail closed. Host procfs may be
sampled internally only for statistics about a managed host tracee; raw host
paths and host identities are never projected into the instance.

Gate 12.5 separates the portable contract from an operational maintenance
lock. Instances default to `locked`; a stopped instance may be explicitly
`unlocked` so Ginux materializes a temporary broad contract for supported
maintenance operations without changing the recipe. Unlocking never removes
Ginux virtualization, promotes unsupported catalog operations, exposes host
kernel trees, grants host credentials, or removes `NoNewPrivs`. Lock state is
persisted beside host network deployment state, shown in lifecycle output, and
must be restored before normal operation.

### Observability and benchmarking

Audit events expose contract decisions and instance lifecycle without exposing
host authority. Benchmarks compare identical workloads natively, in a
conventional container, and under Ginux. The suite covers CPU, memory, syscall
latency, process operations, filesystem and block I/O, networking, boot time,
and idle overhead.

Gate 15 adds a reproducible dispatch matrix without changing execution
semantics. Ordinary Rocky execution is the no-Ginux control. Selective and
paranoid measurements each carry a native control collected in the same host
session with identical workload, affinity, allocation, memory, warmup, and
repetition settings. Combined reports retain both native controls and normalize
each Ginux mode only to its paired samples. Checked-in templates define the
profiles and suite; validation rejects incomplete rows or incompatible
environments before generating tables or graphs.

## Important open decisions

- Numeric performance budgets, set from reproducible baseline evidence before
  Gate 11 begins.

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

Gate 4 selected strict, versioned TOML as the operator-facing instance
configuration format with human approval on 2026-11-03.

Gate 5 selected a digest-pinned OCI image as the instance root filesystem, with
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
content and an assigned bootstrap console within a private instance filesystem
view. Security-sensitive interfaces remain virtualized, proxied, or denied.
