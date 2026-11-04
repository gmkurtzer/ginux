# HDAE Status

## Recovery checkpoint

- Active skill: `.agents/skills/hdae-development/SKILL.md`
- Reload required after compaction, reset, resume, or handoff: yes
- Active project: remaining reporter remediation batch
- Branch: `hdae/issue-27-transactional-rootfs-exec`
- Baseline tag: `research-v1-baseline`
- Active gate: Gate 41, transactional rootfs exec for issue #27
- Last completed action: merged current `main` at `c8d12653` into PR #53's
  branch; `make check` and complete `make test` pass, while the Rocky retry
  again failed on registry DNS before download, unpack, or rootfs `exec`
- Next action: push the conflict-free branch and keep PR #53 and issue #27 open
  until a reachable registry or populated cache permits the mandatory
  rootfs-specific integration run

## Current repository state

Remote `main` is at `c8d12653`. The first
eleven feedback-hardening gates and inserted correctness gates are complete.
Gate 12 is complete on `codex/performance-parity`. PR #52 is merged and issue
#20 is closed. PRs #54, #61, and #81 are merged and issues #43, #58, and #78
are closed. Gate 41 is implemented on PR #53; its last synchronized static and
complete QEMU evidence pass, but rootfs-specific Rocky integration remains
blocked before image download by registry DNS failure. The branch is now
synchronized with current `main`, including the merged native controller
  desktop. Fresh post-merge static and complete deterministic QEMU validation
  pass; the mandatory Rocky integration remains externally blocked.

## Open decisions

- None for Gate 41. The human selected staged address-space publication. The
  domain remains the unit of private payload memory ownership and accounting.

## Active diagnosis

- Gate 43 now distinguishes ordinary inactive activation failure from failed
  rollback. Ordinary failure permits table retirement only after rollback
  proves translation disabled, caches invalidated, the root cleared, command
  completion, and final inactive state. Ambiguous rollback leaves all six
  table leases live and kernel-owned until reboot.
- `make check` and complete `make test` pass; the deterministic QEMU marker is
  `ginux: failed IOMMU activation rollback quarantined`.

- Root cause: Gate 36 correctly allocated private payload frames to the domain,
  but several leaf lookup, fork/exec, RELRO, and cleanup paths still assumed
  every payload frame was kernel-owned. Allocation succeeded, then validation
  treated the live domain-owned frame as absent.
- Wayland is not involved. PR #56's clock change does not resolve the failure.
- Payload leaves now accept exact live leases owned by the domain or kernel,
  plus immutable runtime extents; hierarchy frames remain kernel-only.
- `make check`, `make test`, and `make test-alpine` pass. Alpine completes its
  shell, restart, and failure-containment probes under the 128 MiB budget.

- Configurable allocator owner limits stop exactly at the container budget,
  preserve independent owner capacity, and restore capacity after release.
- Linux private mmap, exec, stack, copy-on-write, process, tmpfs, and SysV
  frames use the generation-safe container owner; shared immutable file-cache
  and page-table hierarchy frames remain kernel-owned by design.
- `make check` passes with the unsafe inventory unchanged at 1,033 blocks.
  `make test` passes the full two-vCPU QEMU and Linux runtime suite, including
  atomic mmap-style and independent SysV quota-exhaustion markers.

- Four-vCPU runtime work now dispatches independently to CPUs 1 through 3.
  A targeted nine-sample four-thread run produced a 2,924 events/s median,
  versus the matched Linux baseline of 1,254 events/s. The prior baseline was
  0.014x Linux.
- Coordinated fast futex waits and arbitrary-count wakes preserve an atomic
  wait-publication/wake boundary. Armed interval timers no longer disable safe
  fast syscalls before a signal is actually pending.
- Profiling found that glibc issued about 699,000 valid
  `FUTEX_WAIT_BITSET|FUTEX_CLOCK_REALTIME` calls during a four-thread memory
  sample. Ginux had returned `EINVAL` for every call. Full bitset operation,
  mask, and absolute-timeout handling now preserves Linux semantics; the
  coordinator uses Linux-permitted spurious wakeups while remote workers run.
- A focused three-sample four-vCPU run produced medians of 2,540 MiB/s for
  memory and 1.061 seconds for mutex, versus matched Linux medians of 2,496
  MiB/s and 1.096 seconds. Thread yield remained above the Linux baseline.
- The final exact-code matrix contains nine matched samples per cell. Every
  cell reaches at least `0.970x`; eighteen of twenty unique cells beat Linux.
  Fixed 2-vCPU CPU throughput is `0.982x`, and scaled 4-vCPU mutex is `0.970x`.
- `make check`, `make architecture-check`, `make test-smp`, `make
  test-rocky-smp-futex`, `make test-rocky-smp-shared-page`, and `make
  test-rocky-smp-thread-context` pass.

## Prior correctness diagnosis

- Fault diagnostics proved the saved thread slot retained the valid shared
  counter in `r11`, while a syscall-based switch resumed it with RFLAGS in
  `r11`. The SYSRET epilogue was correct for an ordinary syscall but incorrect
  for a target asynchronously saved by an interrupt.
- Scheduler-backed cross-thread syscall switches now use the exact-register
  IRET epilogue; ordinary same-thread syscalls retain SYSRET.
- `make check`, `make test-smp`, `make test-rocky-smp-futex`, `make
  test-rocky-smp-shared-page`, and `make test-rocky-smp-thread-context` passed.
  The last command completed nine samples at each of 2 and 4 vCPUs. No tested
  volume owner remained.
- The exact original 4-vCPU/4-thread `cpu memory threads` sequence passed three
  batches and 27 total workload markers, including the formerly failing second
  batch.
- The full matched topology matrix then completed. Every sysbench cell contains
  nine Linux and nine Ginux samples with the same binary hash; every boot cell
  contains three samples. Four-thread Ginux scheduling remains a measured
  performance limitation rather than a correctness failure.

## Limitations

- The floating upstream Alpine integration image currently stalls at Linux
  user entry on both Gate 5 and pre-Gate-5 commits. Deterministic Linux ABI and
  verification-boot suites pass; external Alpine compatibility is separate
  follow-up work.
- A Gate 7 user-access module extraction passed static and unit checks but
  failed deterministic QEMU twice and was reverted. Its assembly and Rust
  wrappers remain co-located with exception entry until emitted-code behavior
  can be compared without weakening the gate.

## Durable evidence

The detailed rationale is in
[`docs/FEEDBACK-HARDENING-RESEARCH.md`](../docs/FEEDBACK-HARDENING-RESEARCH.md).
Gate 1 evidence is in
[`docs/feedback-hardening/GATE-1-BASELINE.md`](../docs/feedback-hardening/GATE-1-BASELINE.md).
The authoritative execution state and gate outcomes are the files under this
`hdae/` directory.
Gate 10 evidence and final limits are in
[`docs/feedback-hardening/GATE-10-QUALIFICATION.md`](../docs/feedback-hardening/GATE-10-QUALIFICATION.md).
Gate 11 evidence and final limits are in
[`docs/feedback-hardening/GATE-11-TOPOLOGY-BENCHMARKS.md`](../docs/feedback-hardening/GATE-11-TOPOLOGY-BENCHMARKS.md).
