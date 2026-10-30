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

None. Depends on Gate 1.

## Remaining work

- Specify entry/return register, stack, interrupt, and SMAP invariants.
- Implement and instrument the minimal assembly path.
- Add adversarial stack and exception-injection tests.

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

Assembly audit, IDT/TSS assertions, fault-injection matrix, guard-page tests,
and full regression transcript.

## Decisions and limitations

Per-CPU layout and nested-NMI semantics require human architectural review.

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
