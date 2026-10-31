# HDAE Status

## Recovery checkpoint

- Active skill: `.agents/skills/hdae-development/SKILL.md`
- Reload required after compaction, reset, resume, or handoff: yes
- Active project: feedback-driven kernel hardening
- Branch: `codex/feedback-hardening`
- Baseline tag: `research-v1-baseline`
- Active gate: Gate 11.6, SMP Linux thread-context integrity, technically
  passed and awaiting checkpoint acceptance
- Last completed action: exact cross-thread IRET restoration passed the
  deterministic 1/2/4-vCPU verification matrix, 18 live thread-yield samples,
  full static checks, futex regression, and shared-page regression
- Next action: obtain human acceptance; only then close Gate 11.6 and resume
  Gate 11

## Current repository state

Local and remote `main` identify commit `1852455`. The first ten
feedback-hardening gates passed and closed under delegated review on
`codex/feedback-hardening`; Gate 11 adds measurement-only qualification to the
same branch and PR.
The ordinary QEMU path defaults to two vCPUs; the complete one/two/four-vCPU
matrix and repeated stress pass. Gate 11.6's exact register-return correction
is commit `5e3393e3`; its report and raw evidence locations are under
`docs/feedback-hardening/`. No push or merge was performed.

## Open decisions

- Gate 11.6 passed technically and requires human acceptance. Gate 11 remains
  paused until that checkpoint is accepted.

## Active diagnosis

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
