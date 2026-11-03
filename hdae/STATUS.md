# HDAE Status

## Recovery checkpoint

- Active skill: `.agents/skills/hdae-development/SKILL.md`
- Reload required after compaction, reset, resume, or handoff: yes
- Active project: remaining reporter remediation batch
- Branch: `hdae/issue-25-mprotect-shootdown`
- Baseline tag: `research-v1-baseline`
- Active gate: Gate 40 mprotect SMP shootdown, delivered as PR #49 for issue #25
- Last completed action: merged current `main` into PR #49 and resolved the
  durable HDAE checkpoint conflict
- Next action: run fresh `make check` and `make test`, then push the new exact
  head for GitHub CI and a later unchanged-head review

## Current repository state

Remote `main` is at `5dd58ca4`. The first
eleven feedback-hardening gates and inserted correctness gates are complete.
Gate 12 is complete on `codex/performance-parity`. Gates 13 through 19 and 21
through 23 and Gates 32 through 33 remain in `gates-todo/` for maintainer
review after delivery through merged pull requests. Gate 34 was delivered
through merged PR #44 and issue #17 is closed; Gate 35 was delivered through
merged PR #45 and issue #18 is closed; Gate 36 was delivered through merged PR
#46 and issue #19 is closed. Gate 37 was delivered through merged PR #59 and
issue #57 is closed. PR #49 is being synchronized for fresh validation; PRs
#50 through #54 remain open and conflicted pending the same merge-train process.
The ordinary QEMU path defaults to two vCPUs; the complete one/two/four-vCPU
matrix and repeated stress pass. Gate 11's comparison and reduced evidence are
under `docs/feedback-hardening/`. No direct push to `main` was performed.

## Open decisions

- None. The architecture defines a domain as the unit of private payload memory
  ownership and accounting; Gate 37 must preserve that invariant. The
  user delegated issues #17, #18, #19, #20, #22, #24, #25, #27, #28, and #29
  as one automatic batch with final review after all PRs.

## Active diagnosis

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
