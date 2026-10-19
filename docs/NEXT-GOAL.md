# Next Goal: Linux Compatibility to Operational Rocky

This goal is deliberately divided into small reviewable stages. Only one stage
is implemented per review cycle. At the end of every stage, the implementation
is tested and committed, this document and the compatibility ledger are
updated, and work stops for operator evaluation. The following stage does not
begin until the operator accepts the completed stage.

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

### Completed checkpoint: Rocky systemd launch

Rocky Linux 10 UBI-init now launches `/sbin/init` as domain PID 1. Systemd
detects QEMU and x86-64, executes its generator fork/exec paths, and remains
alive through the smoke-test stability window. The gate is
`make probe-rocky-systemd`.

This is a launch checkpoint, not a complete system boot. Current nonfatal gaps
include os-release propagation, netlink, sysctl files, timezone-change events,
and some bind-remount behavior. Long-running PID 1 also confirms why control
must come from an independent exec path rather than its attached console.

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
launches as systemd PID 1. Generator fork/exec succeeds and PID 1 survives the
stability window. After `domain exec` exists, a non-interactive systemd status
command and the remaining API-filesystem warnings become the next diagnostic
and compatibility gate; launching PID 1 alone is not a complete system boot.

## Stage 9: Persistence and production architecture

Proposed scope: persistent domain configuration, image cache and writable
layers, controller-managed volumes and migration, domain QoS enforcement,
stable-kABI qualification, external driver/module architecture, hardware and
GPU support, security hardening, performance, cryptography, and attestation.
