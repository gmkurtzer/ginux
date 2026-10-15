# Ginux Verification Roadmap

This roadmap converts the current architecture into user-verifiable stopping points. Each checkpoint is implemented, tested, committed with sign-off, and then stopped for review. Work on the following checkpoint does not begin until the current checkpoint is accepted.

## C0: Architecture alignment

Deliverables:

- One consistent architecture for a unified `GINUX.EFI`, embedded ring-3 controller, kernel/module device boundary, Rocky Linux 10 benchmark, stable kABI, and non-precluded future `.ko` compatibility.
- ADR 0002 records the new boot and driver boundary.
- An automated documentation consistency check.

Gate: `make architecture-check` and a signed commit. No runtime behavior changes in this checkpoint.

## C1: Interactive unified boot

Deliverables:

- `GINUX.EFI` contains an inspectable component catalog and the controller payload.
- The controller starts in ring 3 and owns the interactive console.
- Commands include `help`, `status`, `components`, measured-root `ls`, global
  host `/proc` inspection, `ps`, and `shutdown`.
- `make run` provides the intuitive Mac/QEMU path; documentation covers optional UTM and later KVM acceleration on x86-64 Linux.

Gate: automated serial-console test exercises every command and clean shutdown. Human verification boots it and interacts with the console. No workload domain starts yet.

## C2: Runtime domain lifecycle

**Implementation status:** Complete; awaiting human verification.

Deliverables:

- No workload domain name or entity exists implicitly at boot.
- The controller explicitly creates a named runtime record and sets the pinned
  minimal fixture, `/sbin/init`, bounded CPU/memory grants, and autostart policy.
- The controller starts, observes, stops, and restarts the domain.
- `domain list` reports the complete runtime inventory, personality, and configuration.
- The controller remains resident as host PID 1 across every lifecycle transition.
- A workload-domain or controller failure cannot panic the kernel and leaves no stale authority.

Gate: absent-at-boot, explicit creation/configuration, readiness, stop, restart,
injected failure, cleanup, and predefined-profile rejection tests. Human
verification exercises configuration and lifecycle commands.

## C3: Alpine shell container

**Implementation status:** In progress. Explicit image configuration, runtime
registry resolution and download, verified rootfs ingestion, scalable
four-segment PIE mapping, ELF-declared dynamic-loader startup, and an
interactive BusyBox `/bin/sh` work for a single runtime. C3 remains open:
detach/reattach must use schedulable domain contexts, and Alpine and Rocky must
run concurrently without shared Linux ABI state. Multi-layer filesystem
merging and container networking also remain.

Deliverables:

- Runtime selection of the OCI manifest matching the kernel's native OS,
  architecture, and variant without a build-embedded image. The selected
  manifest and image configuration are digest-verified before layer ingestion.
- Operator-configurable control-domain networking can reach an OCI registry
  and resolve the native-platform manifest. `domain start NAME` automatically pulls
  a missing configured image, verifies every digest while streaming it into an
  available-memory-backed store, and atomically publishes the immutable object.
- Container configuration, cached image objects, writable overlays, and
  temporary data are boot-local and disappear on host reboot. Image storage has
  no configured byte ceiling and fails cleanly if backing resources are exhausted.
- Physical and virtual network devices are independently inventoried kernel
  objects. A domain may attach multiple devices; omission selects host-network
  sharing, and each attached device keeps one unchanged name across the domain
  boundary.
- Each attached interface has explicit static or DHCP configuration and DNS
  resolver state independent from the control domain.
- Alpine's `/bin/sh` executes as container PID 1 with an attached console.
- `Ctrl-]` returns to the controller without stopping the container, and boot
  logs can be inspected after detach.
- The image loader accepts normal four-segment PIE executables and scales past
  the deliberately tiny C2 proof-image limits without weakening W^X.

Gate: deterministic Alpine and Rocky shell transcripts, concurrent detached
runtimes, independent attach/detach/stop/restart, and negative digest/path
tests. Human verification interacts with both unmodified shells rather than a
custom host userspace. See `docs/STABILITY-AUDIT.md` for the complete sequence.

## C4: Rocky systemd early boot

Deliverables:

- Rocky's `/sbin/init` executes as container PID 1 after configurable pre-init
  policy applies the required service enablement and masking changes.
- Required process, mount, user, IPC, cgroup, time, hostname, device, and network object views are container-local.
- Required `/proc`, `/sys`, `/dev`, credentials, clocks, signals, futexes, dynamic linking, and console behavior work.
- Root inside Rocky has no implicit host authority.

Gate: systemd reaches the selected operational target and a console session can inspect and manage the container normally. Negative authority tests pass.

## C5: Networked and persistent Rocky

Deliverables:

- Rocky receives its own loopback, interface, IP address, routes, sockets, and network policy.
- A persistent volume survives host and container restart.
- The controller can initialize a selected local volume, copy and verify
  boot-local state into it, stage it for activation, and reboot into the
  persistent configuration without allowing mutable storage to replace the
  authenticated kernel or embedded controller.
- SSH runs inside Rocky; it is not a native-host service.

Gate: login over SSH, network-isolation tests, storage crash/recovery tests, and reboot persistence. Human verification performs the login and persistence check.

## C6: Two isolated systems

Deliverables:

- Scale the concurrency already required by C3 to multiple Rocky system
  containers with independent init, process IDs, mounts, hostnames,
  credentials, cgroups, clocks, devices, IP addresses, routes, and persistent
  state.
- Failure or root compromise in one does not grant observation or control of the other or the host.

Gate: automated cross-container negative tests and injected failure/restart. Human verification compares both consoles and network identities.

## C7: Remote authenticated management

Deliverables:

- Mutually authenticated host-to-host trust enrollment, including a desktop client profile.
- Remote configuration and lifecycle operations are authorized by local policy and cannot expand the sealed authority envelope.
- Trust rotation and revocation are auditable and fail closed.

Gate: enrollment, allowed/denied operation, replay, rotation, revocation, and unavailable-controller tests. Human verification manages a host remotely and revokes access.

## Work after C7

Broader hardware support, stable-kABI qualification, selected binary `.ko` research, NVIDIA/CUDA, performance parity or improvement, formal verification, attestation, and cryptographic validation each receive explicit milestones and gates before implementation begins.
