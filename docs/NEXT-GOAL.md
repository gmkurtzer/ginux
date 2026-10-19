# Next Goal: Linux Compatibility to Operational Rocky

This goal is deliberately divided into small reviewable stages. Only one stage
is implemented per review cycle. At the end of every stage, the implementation
is tested and committed, this document and the compatibility ledger are
updated, and work stops for operator evaluation. The following stage does not
begin until the operator accepts the completed stage.

## Operator-prioritized checkpoint: persistent host volume foundation

Status: **Configuration and verified OCI layer persistence complete**

The QEMU development host now uses a sparse, self-initializing persistent
volume with a versioned, geometry-bound header committed through the existing
two-slot transactional format. Reopening the same volume recovers its committed
generation without rewriting it. Deleting it and running Ginux again creates a
clean volume. `make test-storage` verifies all three boots and guarantees QEMU
cleanup. `make reset-storage` performs the explicit development reset.

The local configuration record now persists domain definitions, resource
settings, images, init paths, personalities, autostart flags, virtual-network
topology and attachments, and host static/DHCP network policy. Boot recreates
configured kernel domain and network objects; deleting the volume resets them.
Running process state is never resumed across a host reboot.

The indexed, digest-addressed OCI layer cache is now implemented. Registry
transport is fully quiesced before verified layers are written; layer bytes
are flushed before the independent catalog transaction commits. A reboot
restores and re-verifies cached layers before deciding whether registry access
is necessary. `make test-oci-cache` proves a real cold Alpine pull followed by
a warm boot with no registry authentication or layer transfer.

The next storage checkpoint is deliberately not started. Remaining work is
cache refresh/garbage collection, writable domain layers, explicit volumes,
and logs.

## Stage 1: Shared open-file state

Status: **Complete; accepted**

Delivered:

- The current file position belongs to the Linux open-file description.
- Descriptors created by `dup` and inherited by `fork` share that position.
- Independently opened files retain independent identities and positions.
- Saved fork peers receive cursor changes before their context is restored.
- Cross-process verification covers identity and shared-position propagation.

Gate: `make check` and the complete QEMU verification boot pass. Commit
`b7c8067` contains the implementation and initial roadmap.

## Stage 2: Copy-on-write and concurrent scheduling

Status: **Lifecycle checkpoint complete; runtime COW paused after regression review**

Delivered in this checkpoint:

- Writable private fork pages use generation-checked copy-on-write; read-only
  mappings are shared without becoming writable.
- User and supervisor writes materialize private frames, while exit cleanup
  retains frames still referenced by another process.
- Execution-active, completion, exit, and termination state is saved per
  process instead of leaking through global state during a process switch.
- Stopped and continued child events support Linux `wait4`/`waitid` encodings,
  option filtering, one-shot consumption, and procfs stopped state.
- Multiple admitted processes remain runnable, and nice values now influence
  ready-process priority selection. Cleanup, process-limit, round-robin, and
  priority-selection invariants are covered by the verification boot.
- Linux personality forks temporarily use eager private copies. Rocky's repeated
  fork/exec/exit path found incomplete COW re-fork bookkeeping; COW remains in
  the process layer but must not return to the runtime path until that sequence
  has its own passing regression gate.

Remaining before Stage 2 is complete:

- Replace the nested child-first executor with a general process dispatcher.
- Suspend and resume a parent that performs a blocking wait while its child is
  still running, instead of reaching the current internal unsupported path.
- Bind stopped/continued state transitions to scheduler removal/admission;
  signal-triggered job control itself remains Stage 3 work.
- Stress concurrent cleanup and limit enforcement through the live dispatcher.

Gate passed for this checkpoint: `make test`. The next review cycle should
complete the dispatcher and blocking-wait portion of Stage 2, then stop again.

### Completed checkpoint: Rocky systemd operational target and console

Rocky Linux 10 UBI-init launches `/sbin/init` as domain PID 1, reaches
`multi-user.target`, and starts a trusted controller-attach root console. The
gate executes a command in that shell, so a printed prompt alone cannot pass.
Cold and cached warm runs are covered by `make test-rocky-systemd`.

The init image is `rockylinux/rockylinux:10-ubi-init`. The standard `:10` image
declares `/bin/bash` and contains no systemd executable, so it cannot satisfy an
explicit `/sbin/init` configuration. Init planning now distinguishes a missing
file, permissions, interpreter, loader, executable format, and load-layout
failure instead of reporting one opaque load-plan error.

The systemd generator path exceeded an obsolete 64-byte `execve` pathname
buffer; Linux exec now uses the filesystem path capacity. Production runtime
builds also omit the verification syscall ring trace, removing three atomic
trace operations from every syscall and suppressing full trace dumps during
ordinary boot. Verification images retain that diagnostic coverage.

The console uses Bash's direct blocking input mode until serial readiness can
wake a readline `pselect` wait. Current nonfatal gaps include os-release
propagation, netlink, journal file I/O, named FIFOs, cgroup IDs, parts of
inotify/epoll, timezone-change events, and some
bind-remount behavior. Long-running PID 1 also confirms why general control
must come from an independent exec path rather than its attached console.

On the Apple Silicon development host, x86-64 QEMU provides TCG only. The
measured cached run reaches the verified shell in about 25 seconds, with about
18 seconds spent after Linux init. Kernel boot-to-controller is under 200 ms
and cached OCI restore plus root publication is about 6.5 seconds. Native KVM
timing remains the required performance gate; the current TCG result does not
satisfy the eventual couple-second target.

### Next bounded checkpoint: domain process dispatch and exec

`domain exec` is not a controller-side shell shortcut. It must create a new
Linux process inside an already-running domain while PID 1 remains alive. That
requires the general process dispatcher above and is the next implementation
checkpoint.

The interface contract for that checkpoint is:

- `domain exec NAME COMMAND [ARG...]` runs a non-interactive process, streams
  its standard output and error to the controller, waits for it, and returns
  its exit status without changing domain state.
- `domain exec --interactive NAME COMMAND [ARG...]` allocates a fresh PTY,
  makes it the process's controlling terminal, attaches the controller to it,
  and supports the same detach sequence as `domain attach`.
- Each exec process receives the target domain's Linux personality, root and
  mount namespace, PID namespace, credentials, environment baseline, resource
  policy, and network view. It does not inherit host/control-domain authority.
- Exec failure terminates only the exec process. PID 1 and the domain remain
  running. Stopping PID 1 still stops the domain and all of its exec processes.
- The control ABI carries an argument vector and explicit interactive flag;
  command text is never reparsed by the kernel as a shell command.

Acceptance gate: boot an Alpine `/sbin/init`, execute `/bin/echo` both ways,
detach and reattach an interactive `/bin/sh`, verify independent PIDs in the
domain's `/proc`, and confirm a failed exec leaves PID 1 running. Test and
commit this checkpoint, then stop for operator review.

## Stage 3: Complete signal semantics

Proposed scope: arbitrary eligible-thread selection, job-control stop/resume,
interrupted-call restart, nested delivery, and capability- and
user-namespace-aware permissions.

## Stage 4: Complete the C3 domain runtime

Proposed scope: reliable concurrent domains, isolated per-domain networking and
multiple NIC attachment, DHCP and network lifecycle, hardened
attach/detach/stop/restart cleanup, expanded per-domain procfs, and enforced CPU
and memory limits.

## Stage 5: Filesystem and memory compatibility

Proposed scope: general coherent writable file-backed `MAP_SHARED`, remaining
`mremap` behavior, complete `memfd_secret`, named FIFOs and filesystem socket
nodes, broader overlay behavior, useful sysfs and cgroup filesystems, and
vDSO/vvar.

## Stage 6: Network compatibility

Proposed scope: IPv6, interruptible blocking socket operations, general listen
backlogs, socket options and errors, ancillary messages, and complete isolated
domain dataplanes.

## Stage 7: ELF and libc compatibility

Proposed scope: static PIE without `PT_INTERP`, broader dynamic relocations and
dependencies, NSS, locale, resolver, vDSO, and independent musl/glibc
regression coverage.

## Stage 8: Operational Rocky with systemd

Proposed scope: systemd as domain PID 1, mounts, devices, credentials, cgroups,
clocks, timers, process supervision, sockets, service activation, console
login, and `dnf update`.

Current probe: Rocky 10 UBI-init is downloaded, published, and `/sbin/init`
reaches `multi-user.target`; the attached root shell executes a verification
command. The remaining API-filesystem, netlink, cgroup, journal, event, and
console-readiness warnings are the next diagnostic and compatibility gate.

## Stage 9: Persistence and production architecture

Proposed scope: persistent domain configuration, image cache and writable
layers, controller-managed volumes and migration, domain QoS enforcement,
stable-kABI qualification, external driver/module architecture, hardware and
GPU support, security hardening, performance, cryptography, and attestation.
