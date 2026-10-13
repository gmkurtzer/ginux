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

Gate: automated serial-console test exercises every command and clean shutdown. Human verification boots it and interacts with the console. No container starts yet.

## C2: Configured container autostart

**Implementation status:** Complete; awaiting human verification.

Deliverables:

- Authenticated local configuration names the `default` pinned minimal fixture,
  `/sbin/init`, bounded CPU/memory grants, and autostart policy.
- The controller starts, observes, stops, and restarts the container.
- A container or controller failure cannot panic the kernel and leaves no stale authority.

Gate: boot, readiness, stop, restart, injected failure, cleanup, and configuration-rejection tests. Human verification exercises lifecycle commands.

## C3: Rocky systemd early boot

Deliverables:

- A reproducible, digest-pinned Rocky Linux 10 x86-64 container image recipe.
- Rocky's `/sbin/init` executes as container PID 1.
- Early systemd reaches a named target or stops at a precisely documented compatibility boundary with machine-readable missing-interface results.

Gate: deterministic console transcript and compatibility matrix. Human verification observes Rocky/systemd boot rather than a custom host userspace.

## C4: Operational Rocky

Deliverables:

- Required process, mount, user, IPC, cgroup, time, hostname, device, and network object views are container-local.
- Required `/proc`, `/sys`, `/dev`, credentials, clocks, signals, futexes, dynamic linking, and console behavior work.
- Root inside Rocky has no implicit host authority.

Gate: systemd reaches the selected operational target and a console session can inspect and manage the container normally. Negative authority tests pass.

## C5: Networked and persistent Rocky

Deliverables:

- Rocky receives its own loopback, interface, IP address, routes, sockets, and network policy.
- A persistent volume survives host and container restart.
- SSH runs inside Rocky; it is not a native-host service.

Gate: login over SSH, network-isolation tests, storage crash/recovery tests, and reboot persistence. Human verification performs the login and persistence check.

## C6: Two isolated systems

Deliverables:

- Two Rocky system containers boot concurrently with independent init, process IDs, mounts, hostnames, credentials, cgroups, clocks, devices, IP addresses, routes, and persistent state.
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
