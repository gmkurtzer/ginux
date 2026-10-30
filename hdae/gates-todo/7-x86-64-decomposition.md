# Gate 7: x86-64 Decomposition

## Goal

Decompose the 44,324-line x86-64 CPU module along the ownership and mechanism
boundaries established by earlier gates without changing behavior.

## Approved scope

- Extract entry/return, descriptor tables, exceptions, per-CPU/APIC, user
  access, execution, signals, syscall dispatch, time, polling, futex, and
  diagnostic responsibilities.
- Separate architecture mechanisms from Linux personality policy.
- Document unsafe and assembly invariants and dependency direction.

## Non-goals

- A big-bang rewrite or arbitrary line-count target.
- Combining module moves with new SMP behavior.

## Completed work

None. Depends on Gate 6.

## Remaining work

- Approve the module boundary and dependency graph.
- Extract one responsibility at a time with fresh regression evidence.

## Acceptance criteria

- Every extraction preserves the complete pre-extraction result.
- Dependencies follow a documented direction without circular ownership.
- Architecture modules do not own unrelated Linux subsystem policy.
- Unsafe and assembly boundaries state their required invariants.
- Compatibility and benchmark baselines stay within accepted tolerances.

## Evidence

Before/after dependency maps and line counts, per-extraction regressions,
unsafe-boundary documentation, and benchmark comparisons.

## Decisions and limitations

Auditability, cohesive responsibility, and ownership are the success measures;
file size alone is not.

## Delivery strategy

- Working branch: `codex/feedback-hardening`.
- Intended base: `main`.
- Pull request: the shared ten-gate pull request.
- Planned commits: dependency contract followed by one behavior-preserving
  responsibility extraction per commit.
- Actual commits: pending.

## Review policy and outcome

Checkpoint required. Stop for architecture review before synchronization and
SMP work. Outcome: pending.
