# HDAE Status

## Recovery checkpoint

- Active skill: `.agents/skills/hdae-development/SKILL.md`
- Reload required after compaction, reset, resume, or handoff: yes
- Active project: feedback-driven kernel hardening
- Branch: `codex/feedback-hardening`
- Baseline tag: `research-v1-baseline`
- Active gate: Gate 3, fault-recovering userspace access, ready to begin
- Last completed action: Gate 3 added exception-fixup user-copy loops, partial
  cross-page and unmapped runtime tests, and passed `make check` plus the full
  `make test` QEMU boot suite
- Next action: classify every remaining direct SMAP-open userspace access,
  migrate untrusted syscall pointers to audited primitives, and enforce the
  boundary with an architecture check

## Current repository state

Local and remote `main` identify commit `1852455`. Gate 2 passed and closed
under delegated review. Gate 3 is partial: its fault-fixup mechanism and
adversarial runtime proof pass, while the direct-access migration and audit
remain open.

## Open decisions

- None. Per-CPU entry storage and distinct guarded NMI, double-fault, and
  machine-check ISTs are already approved architectural requirements.

## Durable evidence

The detailed rationale is in
[`docs/FEEDBACK-HARDENING-RESEARCH.md`](../docs/FEEDBACK-HARDENING-RESEARCH.md).
Gate 1 evidence is in
[`docs/feedback-hardening/GATE-1-BASELINE.md`](../docs/feedback-hardening/GATE-1-BASELINE.md).
The authoritative execution state and gate outcomes are the files under this
`hdae/` directory.
