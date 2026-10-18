# Next Goal: Linux Compatibility to Operational Rocky

This goal is deliberately divided into reviewable stages. Only one stage is
implemented per review cycle. At the end of every stage, the implementation is
tested and committed, this document and the compatibility ledger are updated,
and work stops for operator evaluation. The following stage does not begin
until the operator accepts the completed stage.

## Stage 1: Core process semantics

Status: **In progress**

Deliverables:

- Linux open-file descriptions share their current file position across
  `dup` and `fork`, while independently opened files retain independent
  positions.
- Forked memory uses copy-on-write rather than eager private copies.
- Processes and threads can remain runnable concurrently, with blocking wait
  and stopped/continued child-state reporting.
- Signal target selection, job-control stop/resume, interrupted-call restart,
  nesting, and namespace-aware permission behavior match the pinned Linux
  contract covered by the test suite.
- Scheduler and priority behavior has positive, negative, lifecycle, and
  cleanup coverage.

Gate: focused process, descriptor, memory, scheduling, wait, and signal tests;
the complete QEMU boot test; `make check`; signed commit; then stop for human
review.

## Stage 2: Complete the C3 domain runtime

Proposed scope (confirmed only after Stage 1 review): reliable concurrent
domains, isolated per-domain networking and multiple NIC attachment, DHCP and
network lifecycle, hardened attach/detach/stop/restart cleanup, expanded
per-domain procfs, and enforced CPU and memory limits.

## Stage 3: Filesystem and memory compatibility

Proposed scope: general coherent writable file-backed `MAP_SHARED`, remaining
`mremap` behavior, complete `memfd_secret`, named FIFOs and filesystem socket
nodes, broader overlay behavior, useful sysfs and cgroup filesystems, and
vDSO/vvar.

## Stage 4: Network compatibility

Proposed scope: IPv6, interruptible blocking socket operations, general listen
backlogs, socket options and errors, ancillary messages, and complete isolated
domain dataplanes.

## Stage 5: ELF and libc compatibility

Proposed scope: static PIE without `PT_INTERP`, broader dynamic relocations and
dependencies, NSS, locale, resolver, vDSO, and independent musl/glibc
regression coverage.

## Stage 6: Operational Rocky with systemd

Proposed scope: systemd as domain PID 1, mounts, devices, credentials, cgroups,
clocks, timers, process supervision, sockets, service activation, console
login, and `dnf update`.

## Stage 7: Persistence and production architecture

Proposed scope: persistent domain configuration, image cache and writable
layers, controller-managed volumes and migration, domain QoS enforcement,
stable-kABI qualification, external driver/module architecture, hardware and
GPU support, security hardening, performance, cryptography, and attestation.

