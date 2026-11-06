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
                      `- domain systemd (virtual PID 1)
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
  boot-identity views;
- a host-shared network backend that remains syscall-mediated; and
- a pinned Rocky Linux 10 systemd baseline that reaches `multi-user.target`,
  authenticates a managed console login, updates through Rocky's enabled
  repositories, and shuts the domain down cleanly.

This is a named compatibility baseline, not a claim of complete Linux or
systemd compatibility. Its exact image, readiness marker, tests, evidence, and
limitations are recorded in
[`hdae/gates-todo/9-systemd-baseline.md`](hdae/gates-todo/9-systemd-baseline.md).

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
make bench              # systemd timing and resource observations
make test-systemd-live  # shared network plus real upstream dnf -y update
make run-host           # general interactive Rocky host with Ginux installed
make demo-gate-9        # Gate 9 checkpoint through the same host workflow
```

`make run-host` boots the QEMU Rocky host and logs you into its ordinary shell.
Ginux is installed in `PATH`, its pinned Rocky domain image is cached, and no
domain has been created or started. A complete first session is:

```sh
ginux domain create rocky --config /etc/ginux/examples/rocky-systemd.toml
ginux domain list
ginux domain status rocky
ginux domain start rocky --attach
```

The attached terminal becomes the Ginux domain console. Log in as `root` with
password `ginux`, explore normally, and run `kill -RTMIN+4 1` as domain root to
shut down systemd and return to the host. Detached operation uses
`ginux domain start rocky`, `ginux domain status rocky`, and
`ginux domain stop rocky`. Exit the Rocky host shell to stop QEMU and clean up.
`make demo-gate-9` uses the same operator workflow; `make run-systemd` remains
as a compatibility alias, and `make test-systemd-interactive` is the automated
regression.

The live test and interactive demonstration enable shared external networking.
An upstream DNF update can take many minutes under QEMU TCG because every
managed syscall is intercepted and audited. The offline systemd test uses only
the prepared, digest-pinned cache.

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
