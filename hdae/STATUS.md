# HDAE Status

## Recovery checkpoint

- Active skill: `.agents/skills/hdae-development/SKILL.md`
- Reload required after compaction, reset, resume, or handoff: yes
- Active project: feedback-driven kernel hardening
- Branch: `codex/feedback-hardening`
- Baseline tag: `research-v1-baseline`
- Active gate: Gate 9, SMP operation and 2-vCPU test default, in progress
- Last completed action: Gate 8 closed after adding ranked locks, synchronizing
  physical allocation and scrubbing, enforcing a recursive cross-CPU-exclusive
  kernel-entry boundary, auditing direct interrupt masking, and passing the
  complete deterministic QEMU suite
- Next action: extend the proven simultaneous execution into cross-CPU signal,
  futex, pipe, socket, process-exit, and mapping-change tests

## Current repository state

Local and remote `main` identify commit `1852455`. Gates 1 through 8 passed and
closed under delegated review. Gate 9 is active on the shared hardening branch.
The discovered AP set starts through a W^X low-memory trampoline. Kernel-only
IPIs, remote invalidation, concurrent processor execution, AP Linux execution,
simultaneous Linux address spaces across the BSP and CPU 1, and per-process
domain-bounded affinity pass the full two-vCPU deterministic QEMU suite.
Linux `/proc/cpuinfo` and `/proc/stat` are generated from the domain-visible
online processor set. Cross-CPU lifecycle and IPC semantics, domain
placement/accounting, topology stress, and the two-vCPU harness default remain
open Gate 9 work.

## Open decisions

- None. Per-CPU entry storage and distinct guarded NMI, double-fault, and
  machine-check ISTs are already approved architectural requirements.

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
