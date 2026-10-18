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

Status: **Copy-on-write/lifecycle checkpoint complete; awaiting operator review**

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

Remaining before Stage 2 is complete:

- Replace the nested child-first executor with a general process dispatcher.
- Suspend and resume a parent that performs a blocking wait while its child is
  still running, instead of reaching the current internal unsupported path.
- Bind stopped/continued state transitions to scheduler removal/admission;
  signal-triggered job control itself remains Stage 3 work.
- Stress concurrent cleanup and limit enforcement through the live dispatcher.

Gate passed for this checkpoint: `make test`. The next review cycle should
complete the dispatcher and blocking-wait portion of Stage 2, then stop again.

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

## Stage 9: Persistence and production architecture

Proposed scope: persistent domain configuration, image cache and writable
layers, controller-managed volumes and migration, domain QoS enforcement,
stable-kABI qualification, external driver/module architecture, hardware and
GPU support, security hardening, performance, cryptography, and attestation.
