# Gate 6: x86-64 Decomposition

## Goal

Decompose the 44,324-line x86-64 CPU module along validated ownership and
mechanism boundaries without changing behavior.

## Approved scope

- Extract entry/return, descriptor tables, exceptions, per-CPU/APIC, user
  access, execution, signals, syscall dispatch, time, polling, futex, and
  diagnostic responsibilities.
- Separate architecture mechanisms from Linux personality policy.
- Document unsafe and assembly invariants.

## Non-goals

- A big-bang rewrite.
- Combining refactors with new SMP behavior.

## Completed work

None. Depends on Gate 5.

## Remaining work

- Approve the module boundary and dependency graph.
- Extract one boundary at a time while preserving tests and performance.

## Acceptance criteria

- Every extraction preserves the complete pre-extraction result.
- Dependencies follow a documented direction without circular ownership.
- Architecture modules do not own unrelated Linux subsystem policy.
- Compatibility and benchmark baselines stay within accepted tolerances.

## Evidence

Before/after dependency maps and line counts, per-extraction regressions,
unsafe-boundary documentation, and benchmark comparisons.

## Decisions and limitations

Module size alone is not the success metric; ownership and auditability are.

## Delivery strategy

Planned commits: dependency contract, followed by one behavior-preserving
module extraction per commit.

## Review policy and outcome

Checkpoint required. Stop for architecture review before SMP activation.
Outcome: pending.
