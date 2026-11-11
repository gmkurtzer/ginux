# Ginux

**Ginux is a configurable userspace kernel for isolated Linux compatibility
instances.** It runs as an ordinary Linux process and places an explicit,
versioned contract between managed workloads and the host kernel.

> [!IMPORTANT]
> Ginux began as an independent Rust kernel that booted directly through UEFI
> and ran a Rocky Linux 10 userspace. That research phase is complete. Its
> final source is permanently preserved on the
> [`bootable-kernel`](https://github.com/gmkurtzer/ginux/tree/bootable-kernel)
> branch and at the
> [`bootable-kernel-final`](https://github.com/gmkurtzer/ginux/tree/bootable-kernel-final)
> tag. Active development now continues here as a userspace kernel.

## Why the project changed direction

The standalone-kernel experiment proved that an AI coding agent, directed by
an experienced human through architecture, gates, and executable tests, could
construct a new kernel capable of running meaningful Linux userspace. It also
made the limits of that result obvious.

Linux is much more than source code. It is a stable interface, a development
process, a global community, vendor and hardware alignment, and decades of
compatibility and operational experience. Reimplementing kernel mechanisms
does not reproduce that ecosystem, and competing with Linux was never the
goal.

The more useful result is the boundary the experiment exposed. Linux must
preserve a broad userspace ABI; as Linus Torvalds put it, **“And we simply do
not break user space.”** The
[kernel regression documentation](https://docs.kernel.org/process/handling-regressions.html)
records that rule. Ginux can sit above Linux and deliberately offer a smaller
contract to each managed instance. Before a managed syscall has a host effect,
Ginux decides whether to emulate it, perform a validated mediated operation,
or deny it.

Put bluntly: **Linux must not break userspace; Ginux is designed to break the
full Linux ABI guarantee deliberately and selectively.** A instance receives
only the capabilities and interface behavior its approved workload requires.
Every departure is intended to be explicit, versioned, reviewable, and tested,
rather than accidental incompatibility.

This pivot is also a result of the development method. AI made it practical to
build disposable but executable prototype code, test an architectural idea,
learn from the running system, and carry the valuable parts into a better
design. The prototype was evidence, not an obligation to turn every experiment
into a product.

## What Ginux is now

```text
Host Linux kernel
  `- host systemd (real PID 1)
       `- ginux (ordinary trusted host process)
            `- instance contract
                 |- virtual processes and identities
                 |- virtual descriptors and kernel views
                 |- assigned devices and resources
                 `- managed Rocky Linux userspace
                      `- instance systemd (virtual PID 1)
```

Ginux supervises native x86-64 programs and intercepts managed syscalls before
their host effect. Its central dispatcher resolves the instance contract and
chooses one of three outcomes:

- **Emulate:** answer entirely from Ginux-owned state.
- **Mediate:** validate and translate the request, then perform a narrow
  Ginux-owned host operation.
- **Deny:** return a configured Linux-compatible error or terminate the task.

The current implementation includes:

- a security-focused x86-64 interpreter and a balanced native ptrace backend;
- strict, versioned instance configuration and lifecycle;
- a digest-pinned OCI root, immutable filesystem index, and bounded overlay;
- Ginux-owned process, credential, signal, synchronization, timer, and event
  models;
- virtual devices with explicit assignment, ownership, and revocation; and
- bounded virtual `/proc`, `/sys`, cgroup v2, mount, namespace, machine, and
  boot-identity views;
- a host-shared network backend that remains syscall-mediated; and
- a pinned Rocky Linux 10 systemd baseline that reaches `multi-user.target`,
  authenticates a managed console login, updates through Rocky's enabled
  repositories, and shuts the instance down cleanly.

This is a named compatibility baseline, not a claim of complete Linux or
systemd compatibility. Its exact image, readiness marker, tests, evidence, and
limitations are recorded in
[`hdae/gates-todo/9-systemd-baseline.md`](hdae/gates-todo/9-systemd-baseline.md).

### Agent containment research

Ginux is also being tested as a containment boundary for AI agents and other
automated software that can execute commands, modify files, open network
connections, and start additional processes. Instead of giving an agent the
host's complete Linux ABI, an operator can place its process tree inside a
instance whose kernel contract exposes only the operations and virtual resources
required for the task.

The research goal is a reviewable boundary spanning syscalls, ioctls,
filesystems, devices, networking, process authority, and virtual `/proc` and
`/sys` state. For example, a contract may allow an agent to read time, inspect
its own processes, and modify one assigned workspace while denying clock
changes, host filesystem access, unapproved network listeners, and unrelated
devices. Ginux can then audit the contract decisions made while the agent and
its child processes run.

This is an active verification target, not a claim that Ginux is currently a
production-grade agent sandbox. The contract inventory, configurable policy,
adversarial tests, performance costs, and escape boundaries are tracked through
the HDAE gates and must be demonstrated before stronger containment claims are
made.

## Relationship to other systems

The userspace-kernel idea is established research and engineering territory.
Ginux explores a particular point in that design space:

| Approach | Primary boundary | Difference in emphasis |
|---|---|---|
| Linux [seccomp](https://docs.kernel.org/userspace-api/seccomp_filter.html) | Filters host syscalls | Restricts the existing ABI rather than defining a complete virtual contract |
| [User-Mode Linux](https://kernel.org/doc/html/latest/virt/uml/user_mode_linux.html) | Runs Linux as a host process | Reuses a complete Linux kernel over virtual hardware |
| [gVisor](https://gvisor.dev/docs/) | Implements an application kernel in userspace | Closely related model with different compatibility and performance goals |
| [Gramine](https://gramine.readthedocs.io/en/stable/) | Packages applications with a library OS | Uses manifests and a library-OS boundary |
| [Proxos](https://www.usenix.org/legacy/events/osdi06/tech/ta-min.html) | Routes calls between private and commodity OSes | Makes trust placement configurable across the syscall interface |
| **Ginux** | Gives each instance a versioned kernel contract | Treats the declared contract, virtual state, devices, resources, and lifecycle as one reviewable boundary |

Ginux does **not** promise the complete Linux ABI. Compatibility claims apply
only to named contracts, workloads, and executable evidence.

## Development and testing

The reference development environment is macOS arm64. QEMU TCG boots a pinned
Rocky Linux 10 x86-64 host where Ginux runs as an ordinary unprivileged
process. The host systemd remains real PID 1.

Check prerequisites:

```sh
make doctor
```

Prepare the pinned assets with explicit network operations:

```sh
make host-image-fetch
make image-helper-fetch
make instance-image-fetch
make systemd-image-fetch
```

Build and run the deterministic suite offline after the caches are populated:

```sh
make build
make check
make test
```

Inspect the latest completed capability directly:

```sh
make test-systemd       # cached image, external networking disabled
make bench-prepare      # one-time networked setup for the pinned benchmark image
make bench              # matched native/Ginux matrix plus mandatory budget checks
make bench-modes-prepare # one-time setup for the three-way dispatch comparison
make bench-modes        # native/selective/paranoid tables and graphs
make demo-gate-15       # bounded reproducible three-way comparison
make test-systemd-live  # shared network plus real upstream dnf -y update
make host-run           # general interactive Rocky host with Ginux installed
make host-reset         # discard that host's cache, instances, and writable state
make demo-gate-9        # Gate 9 checkpoint through the same host workflow
```

Instance recipes select syscall control under `[contract.syscalls]`:

```toml
[contract.syscalls]
control = "selective" # default; use "paranoid" for all-syscall mediation
default = "deny"
allow = ["*"]
deny = []
```

`selective` is the default and permits direct host execution only
when the instance contract allows the operation and Ginux's versioned catalog
independently classifies it as direct-safe. Denials, virtual state, inspection,
translation, emulation, pointer or descriptor validation, and modified results
remain Ginux-dispatched in both modes. `paranoid` is an explicit opt-in that
sends every syscall through the Ginux userspace dispatcher before completion.

`make bench-compare` checks every row against the 4% native budget and, after an
accepted baseline is published under `benchmarks/gate11/`, against that Ginux
baseline as well. Environment mismatches are reported separately from
performance regressions. `make demo-gate-11` runs a bounded human-inspectable
comparison and returns nonzero when a mandatory row misses its budget.

Gate 15 provides a separate characterization workflow across native Rocky,
selective Ginux, and paranoid Ginux. Native is the no-mediation control, not a
Ginux instance mode. The checked-in templates under
[`benchmarks/gate15/`](benchmarks/gate15/) fix the complete test protocol and
may be copied without editing scripts. `make bench-modes` produces paired raw
samples, metadata, a Markdown table, and an SVG graph under `build/gate15/`.
See [Reproducible performance measurement](docs/performance.md) for the exact
protocol, customization workflow, output schema, and interpretation limits.

`make host-run` boots the QEMU Rocky host and logs you into its ordinary shell.
Ginux is installed in `PATH`. The first `create` or `import` downloads and verifies
the pinned Rocky OCI image with live per-layer progress; later operations reuse the
cache. The writable Rocky host overlay persists across `make host-run`, ordinary
cleanup, builds, and tests, so cached images, instance definitions, roots, and
stopped status remain available. `make host-reset` is the explicit destructive
operation that stops the host and removes that overlay; the next `make host-run`
starts from the pinned clean Rocky image.
A complete first session is:

```sh
ginux create rocky
ginux list
ginux status rocky
ginux start rocky --attach
```

`create` starts from the documented default recipe. Use
`ginux import NAME FILE` to create or atomically replace an instance from a
self-contained recipe, `ginux export NAME` to print that recipe, and
`ginux edit NAME` to validate and atomically apply an editor session.

Instances are contract-locked by default. For an explicit maintenance window,
stop the instance, run `ginux unlock NAME`, perform the required maintenance,
stop it again, and run `ginux lock NAME`. `ginux list` and `ginux status NAME`
show the current state, and starting an unlocked instance prints a warning.
Unlocking relaxes configurable recipe restrictions only; Ginux still owns the
virtual kernel boundary, rejects unsupported authority, reduces host identity,
and applies `NoNewPrivs`. Lock state is host deployment state and is never
written into an exported portable recipe.

Syscall auditing is disabled by default with `[audit] enabled = false`. Add
`--audit` to `ginux start` to enable it for that run, or set `enabled = true`
to enable it for every start. Enabled audit records always go to the instance's
`audit.log`; they are kept separate from the attached console.
Existing recipes using `mode = "stderr"` or `mode = "disabled"` remain
read-compatible and normalize to the new boolean form when rewritten.

The instance specification supports typed recipe overrides, ingress and egress
contract policy, volumes, virtual identity, and ordered pre-boot hooks. Host
network attachment and translated ports are deployment settings, supplied as
repeatable flags when the instance is created or changed:

```sh
ginux import web /etc/ginux/examples/rocky-systemd.toml \
    --network eth0:1 \
    --forward protocol=tcp,external_port=8080,instance_port=80 \
    --forward protocol=tcp,external_port=8443,instance_port=443
ginux set web \
    --network default \
    --forward none
```

Omitting `--network` uses the host's default network. Multiple flags are
validated and persisted atomically; an invalid combination changes nothing.
Hooks run only as
mediated instance processes; Ginux does not support arbitrary host-context hooks.
See [Gate 9.75 instance specification](docs/gate-9.75-instance-specification.md)
for the schema, precedence rules, examples, and verification targets.

The same instance file contains a self-contained hierarchical contract under
`[contract]` and its `syscalls`, `filesystem`, `network`, `devices`,
`privileges`, `process`, and `kernel` subsections. Syscall names accept bounded
globs compiled against the pinned catalog; explicit denies always win. The
contract is part of the instance rather than a separate operator object.
Validate and inspect the complete instance recipe before launch:

```sh
ginux validate /etc/ginux/examples/rocky-systemd.toml
ginux inspect /etc/ginux/examples/rocky-systemd.toml
ginux explain /etc/ginux/examples/rocky-systemd.toml filesystem.path.write
```

See [Self-contained userspace contract](docs/configurable-contract.md) for the
syntax, precedence, migration behavior, and complete CLI workflow.

The attached terminal becomes the Ginux instance console. Log in as `root` with
password `ginux`, explore normally, press `Ctrl-]` to detach while leaving the
instance running, and run `kill -RTMIN+4 1` as instance root to
shut down systemd and return to the host. Detached operation uses
`ginux start rocky`, `ginux status rocky`, and
`ginux stop rocky`. Exit the Rocky host shell to stop QEMU cleanly.
This stops the VM without deleting its persistent overlay; `make clean` resets
that host state. `make demo-gate-9` uses the same operator workflow;
`make run-systemd` remains as a compatibility alias, and
`make test-systemd-interactive` is the automated regression.

The live test and interactive demonstration enable shared external networking.
An upstream DNF update can take many minutes under QEMU TCG because every
managed syscall is intercepted and audited. The offline systemd test uses only
the prepared, digest-pinned cache.

Individual component and gate targets are documented in the `Makefile` and the
corresponding files under [`docs/`](docs/) and [`hdae/`](hdae/).

## Security model and limitations

The primary adversary controls a process inside an instance. The host kernel,
hardware, Ginux executable and approved configuration, boot chain, and
operator are currently trusted. The balanced native backend still relies on
the host for ELF loading, address spaces, page faults, signals, scheduling,
and ptrace; those remain host-kernel attack surface.

Ginux is research software. It has not received independent security
certification, does not provide complete Linux compatibility, and is not ready
for production workloads. Unknown contract operations fail closed, but current
coverage and guarantees are limited to the tested gates.

The precise supported claims, retained supervisor authority, trusted
components, exclusions, compatibility gaps, performance limitation, and
reproducible boundary tests are documented in
[Security boundary and limitations](docs/security-boundary.md).

For agent containment, the protected object is the host and any authority not
explicitly assigned to the instance. Ginux constrains the Linux processes in
which an agent and its tools execute; it does not inspect model reasoning,
secure an external model provider, or make untrusted code safe outside the
enforced instance boundary.

The full intent, threat model, invariants, and gate plan live in:

- [`hdae/PROJECT.md`](hdae/PROJECT.md)
- [`hdae/ARCHITECTURE.md`](hdae/ARCHITECTURE.md)
- [`hdae/PLAN.md`](hdae/PLAN.md)
- [`hdae/STATUS.md`](hdae/STATUS.md)

## Development method

Ginux is developed with
[Human-Directed Agentic Engineering (HDAE)](https://github.com/gmkurtzer/hdae):
human-owned product intent, architecture, constraints, and acceptance
boundaries combined with agent-executed implementation, diagnosis, and test
loops. Each approved gate records its scope, non-goals, acceptance criteria,
verification, evidence, and stopping point in `hdae/`.

## Historical standalone kernel

The independent bootable kernel remains available for research and historical
reference:

- [final branch: `bootable-kernel`](https://github.com/gmkurtzer/ginux/tree/bootable-kernel)
- [final tag: `bootable-kernel-final`](https://github.com/gmkurtzer/ginux/tree/bootable-kernel-final)
- [technical report and DOI](https://doi.org/10.5281/zenodo.23682548)

It is no longer the active development line. The code remains available under
the repository's existing terms for anyone who independently chooses to study
or fork it.

The name remains a joke in the same family as Linux: **Ginux** combines Linux,
GPT, and Greg. To keep the author's ego within its configured resource limit,
the `G` officially stands for GPT.

## License

Ginux is available under the Apache License 2.0. See [`LICENSE`](LICENSE) and
[`NOTICE`](NOTICE).

## Configured native launch

On x86-64 Linux, `ginux launch <config> <root> [--result-file <fresh host path>]`
executes `[instance].init` with the configured arguments and environment inside
the supplied host root directory. The bootstrap requires chroot and credential
change privileges, and a kernel with `close_range` (Linux 5.9 or later).
It drops to UID/GID 1000 before executing the workload. The configuration must
select `profile = "balanced"` and `backend = "ptrace-native"`; storage and device
assignments are not implemented by this command and are rejected.

`[instance].workdir` is optional and defaults to `/work`; `/` is also supported.
The bootstrap creates the working directory if its parent exists, then enters
it after dropping privileges. Init and working-directory paths are limited to
128 bytes and must be absolute without `..` path components. Up to seven
arguments of 128 bytes each and eight `NAME=value` environment entries of
256 bytes each are accepted. Invalid or oversized input fails before launch.

Workload stdout stays separate from supervisor diagnostics on stderr. Only
stdin, stdout, and stderr are inherited by the workload. The launcher returns
the workload exit code, or `128 + signal` for signal termination. Startup
failure returns 2; supervision or result-writing failure returns 1. Native
fork, clone, and subsequent exec are unsupported; fork returns `ENOSYS` until
child supervision is implemented. Non-x86-64 syscall ABIs and job-control
group stops fail explicitly. Audit output is capped at 64 KiB plus a truncation
notice. This path does not implement resource quotas from the instance metadata.

When requested, the result file is reserved exclusively before launch, with
mode 0600 and a maximum path length of 512 bytes. Existing files are never
overwritten. After completion, it contains one synced JSON object:

```json
{"version":1,"outcome":"exited","code":0,"signal":null,"generation":null}
```

`outcome` is `exited`, `signaled`, or `supervisor-error`; only the relevant
`code` or `signal` is populated. Storage generations are not implemented, so
`generation` is null. A missing, empty, or malformed result is a protocol failure.
A failure to write, sync, or close the result returns 1 even if the workload
succeeded. Consumers should wait for launcher completion before reading it.

The focused Linux integration scenario consumes existing binaries:

```sh
make build
make build-launch-fixture
make build-gate1-fixture
sudo make test-gate1-setup
sudo make test-launch
sudo make test-native-supervision
```

## Native process scenarios

The separate `process-native <config> <executable>` command exercises native
fork, clone, vfork, exec, threads, waits, termination signals, and pipes. It
executes a trusted host fixture as the caller, without chroot; it does not
require root on a host that permits tracing child processes. Configured
`launch` remains a separate entry point and does not yet use this process loop.

Process creation returns virtual identities, including clone TID cells.
Untraced, reparenting, namespace, and unsupported clone flags are denied.
Thread slots are reclaimed on exit. `CLONE_FILES` shares one descriptor table;
ordinary fork copies it, and exec unshares it before closing CLOEXEC entries.
Worker-thread exec preserves the group's process identity. Inherited host
files above stderr are closed before execution. PI futex operations and queries
for another task's robust list are unsupported.

Build the fixtures explicitly, then run their independent scenarios:

```sh
make build build-process-fixtures
make test-process-native
make test-process-native SCENARIO=shared_pipe
make test-process-native SCENARIO=thread_exec
```

The scenarios consume existing binaries under `.artifacts/process`, including
checks for syscall ABI dispatch, clone restrictions, descriptor inheritance,
thread reuse, successful and failed worker exec, signals, wait-status writes,
nonblocking waits, and vfork. The existing native fork fixture also runs. These
checks do not claim complete libc, job-control, or systemd compatibility.

Native configured clone3 captures the Linux 6.12 argument structure before
execution and gives the host a private read-only snapshot. Caller structures
may be read-only; Ginux does not rewrite them and restores syscall argument
registers. Normal kernel-declared outputs, such as the pidfd output, still apply.
The existing virtual-cgroup adaptation accepts only an owned virtual cgroup
directory and starts the child in the parent's host cgroup; it does not
implement host cgroup placement. Unknown nonzero extension bytes fail closed.

The native runtime reserves `0x600000000000` through `0x600000080000` for
transient clone snapshots (one 4 KiB page per active task slot, within existing
memory and task limits). Workloads cannot map, discard, move or change
permissions in this range. Parent and child remove their snapshots before
returning to workload code. A pre-existing mapping is never overwritten;
allocation errors propagate. Death during the private protocol fails the
instance closed so a shared address space cannot retain a stale snapshot.
Snapshot capture requires host `process_vm_readv`; it fails closed if the host
cannot provide it.
