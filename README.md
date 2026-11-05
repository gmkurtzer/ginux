# Ginux

**Ginux is a configurable userspace kernel for isolated Linux compatibility
domains.** It runs as an ordinary Linux process and places an explicit,
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
contract to each managed domain. Before a managed syscall has a host effect,
Ginux decides whether to emulate it, perform a validated mediated operation,
or deny it.

Put bluntly: **Linux must not break userspace; Ginux is designed to break the
full Linux ABI guarantee deliberately and selectively.** A domain receives
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
            `- domain contract
                 |- virtual processes and identities
                 |- virtual descriptors and kernel views
                 |- assigned devices and resources
                 `- managed Rocky Linux userspace
                      `- domain systemd (virtual PID 1, planned Gate 9)
```

Ginux supervises native x86-64 programs and intercepts managed syscalls before
their host effect. Its central dispatcher resolves the domain contract and
chooses one of three outcomes:

- **Emulate:** answer entirely from Ginux-owned state.
- **Mediate:** validate and translate the request, then perform a narrow
  Ginux-owned host operation.
- **Deny:** return a configured Linux-compatible error or terminate the task.

The current implementation includes:

- a security-focused x86-64 interpreter and a balanced native ptrace backend;
- strict, versioned domain configuration and lifecycle;
- a digest-pinned OCI root, immutable filesystem index, and bounded overlay;
- Ginux-owned process, credential, signal, synchronization, timer, and event
  models;
- virtual devices with explicit assignment, ownership, and revocation; and
- bounded virtual `/proc`, `/sys`, cgroup v2, mount, namespace, machine, and
  boot-identity views.

Gate 8 is complete. Booting Rocky Linux systemd as virtual PID 1 is the next
planned compatibility gate; it is not yet a current claim.

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
| **Ginux** | Gives each domain a versioned kernel contract | Treats the declared contract, virtual state, devices, resources, and lifecycle as one reviewable boundary |

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
make domain-image-fetch
```

Build and run the deterministic suite offline after the caches are populated:

```sh
make build
make check
make test
```

Inspect the latest completed capability directly:

```sh
make demo-gate-8
```

Individual component and gate targets are documented in the `Makefile` and the
corresponding files under [`docs/`](docs/) and [`hdae/`](hdae/).

## Security model and limitations

The primary adversary controls a process inside a domain. The host kernel,
hardware, Ginux executable and approved configuration, boot chain, and
operator are currently trusted. The balanced native backend still relies on
the host for ELF loading, address spaces, page faults, signals, scheduling,
and ptrace; those remain host-kernel attack surface.

Ginux is research software. It has not received independent security
certification, does not provide complete Linux compatibility, and is not ready
for production workloads. Unknown contract operations fail closed, but current
coverage and guarantees are limited to the tested gates.

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
executes `[domain].init` with the configured arguments and environment inside
the supplied host root directory. The bootstrap requires chroot and credential
change privileges, and a kernel with `close_range` (Linux 5.9 or later).
It drops to UID/GID 1000 before executing the workload. The configuration must
select `profile = "balanced"` and `backend = "ptrace-native"`; storage and device
assignments are not implemented by this command and are rejected.

`[domain].workdir` is optional and defaults to `/work`; `/` is also supported.
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
notice. This path does not implement resource quotas from the domain metadata.

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
