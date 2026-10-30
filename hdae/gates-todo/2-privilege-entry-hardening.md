# Gate 2: x86-64 Privilege-Entry Hardening

## Goal

Ensure every privilege entry reaches trusted, guarded kernel storage before it
can use a stack or execute Rust code.

## Approved scope

- Minimal `SYSCALL` assembly entry and safe return path.
- Per-CPU entry state and guarded entry stacks.
- Distinct guarded IST stacks for NMI, double fault, and machine check.
- Defined nested-NMI behavior and transition to the current task stack.

## Non-goals

- General SMP enablement or unrelated syscall behavior changes.
- Broad x86-64 module decomposition.

## Completed work

- [x] Replaced singleton syscall scratch globals with CPU-indexable entry state
  selected through `IA32_GS_BASE`.
- [x] Switched from the userspace RSP to trusted entry storage before the first
  push, call, Rust prologue, or compiler-generated stack use.
- [x] Added distinct NMI, double-fault, and machine-check IST assignments with
  compact 64-KiB stacks and lower/upper guard pages.
- [x] Added canonical RIP/RSP validation before `SYSRET`; invalid state unwinds
  on the trusted kernel stack.
- [x] Added a static privilege-entry architecture check and updated the original
  claim reproducer to distinguish fixed claims from remaining user-copy work.

## Remaining work

- Specify and test nested-NMI behavior at each instrumented entry stage.
- Add adversarial stack and exception-injection tests.
- Diagnose the existing late interpreted-thread invalid return context and make
  the full verification boot pass without weakening its assertions.

## Acceptance criteria

- No push, call, Rust code, or compiler prologue uses a userspace-controlled
  stack after `SYSCALL` entry.
- NMI, double fault, and machine check have distinct nonzero ISTs with lower
  and upper guard pages.
- Invalid, unmapped, noncanonical, and guard-page user stacks cannot panic or
  corrupt the kernel.
- Injected NMI at every instrumented entry stage is contained.
- Existing Linux ABI and Rocky boot gates pass.

## Evidence

- `bash tools/architecture/check-privilege-entry.sh` -> passed; no stack use
  precedes the GS-based trusted stack switch, and all three guarded IST policies
  plus canonical-return validation are present.
- `bash tools/architecture/reproduce-hardening-feedback.sh` -> SYSCALL user-RSP,
  missing NMI IST, and missing machine-check IST claims now report
  `resolved-or-not-reproduced`; the Gate 3 user-copy claim remains confirmed.
- `make check` -> passed after the mechanism and verifier changes.
- `make test` -> failed after completing PID 1 and reaching the later
  interpreted workload: an existing invalid user return RIP `0x404d87` caused
  vector 6, followed by a kernel page fault. The harness terminated QEMU.
- An experimental all-IRET syscall return was rejected: it stalled the same
  later interpreted workload instead of preserving Linux syscall behavior.

## Decisions and limitations

The approved per-CPU layout uses a CPU-indexable entry-state array and GS-based
selection. Only the bootstrap CPU slot is active until Gate 9 starts APs.
Nested-NMI injection and the full regression remain incomplete, so this gate is
partial and open.

## Delivery strategy

- Working branch: `codex/feedback-hardening`.
- Intended base: `main`.
- Pull request: the shared ten-gate pull request.
- Planned commits: reproductions retained from Gate 1, entry-state/IST
  mechanism, syscall entry transition, and integration evidence.
- Actual commits: pending.

## Review policy and outcome

Delegated closure authorized after all criteria pass with fresh assembly,
fault-injection, and regression evidence. Outcome: pending.
