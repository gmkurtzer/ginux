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

- [x] Recorded the initial 48,510-line module size, dependency direction,
  planned extraction boundaries, and unsafe/assembly invariants.
- [x] Extracted per-CPU entry state, GDT/TSS/IDT structures, guarded stacks,
  and their explicit owner into `cpu/architecture.rs`.
- [x] Extracted Linux syscall identifiers, ABI constants, and clone-flag policy
  into `cpu/linux_abi.rs`, leaving hardware constants in the x86 mechanism
  module and preserving the public compatibility-limit exports.
- [x] Moved entry-state installation, descriptor-table construction,
  interrupt-gate management, and guarded-stack accessors into
  `cpu/architecture.rs`; updated the privilege-entry architecture check to
  inspect the extracted mechanism at its authoritative location.
- [x] Extracted extended user-state feature enablement and container/process
  save, restore, clone, reset, and discard mechanisms into `cpu/execution.rs`.

## Remaining work

- Extract one responsibility at a time with fresh regression evidence.

## Acceptance criteria

- Every extraction preserves the complete pre-extraction result.
- Dependencies follow a documented direction without circular ownership.
- Architecture modules do not own unrelated Linux subsystem policy.
- Unsafe and assembly boundaries state their required invariants.
- Compatibility and benchmark baselines stay within accepted tolerances.

## Evidence

- [`docs/feedback-hardening/X86-DECOMPOSITION.md`](../../docs/feedback-hardening/X86-DECOMPOSITION.md)
  records the pre-extraction size, dependency direction, module boundaries,
  unsafe invariants, and per-extraction verification rule.
- The first extraction passed `make architecture-check`, `make check`, and
  `make build-verification && bash tests/qemu/boot.sh`; the writable-global
  inventory remained stable at 450 and QEMU shut down cleanly.
- The Linux ABI extraction passed the same architecture, repository, and full
  deterministic QEMU verification sequence without changing the inventory.
- The descriptor and entry-state extraction passed formatting,
  `make architecture-check`, `make check`, and
  `make build-verification && bash tests/qemu/boot.sh`; the inventory remained
  at 450 classified globals and QEMU reached `GINUX_KERNEL_OK` and shut down.
- The extended execution-state extraction passed the same complete regression
  sequence, including Linux ELF/runtime compatibility and a clean QEMU boot.

## Decisions and limitations

Auditability, cohesive responsibility, and ownership are the success measures;
file size alone is not.

An attempted extraction of the raw user-copy wrappers, verifier, SMAP helper,
and copy-loop assembly compiled and passed static and unit checks but failed the
full deterministic QEMU boot twice with widespread Linux verification errors
and `GINUX_KERNEL_PANIC`. Keeping the assembly co-located with exception entry
did not restore behavior while the Rust wrappers lived in a child module. The
attempt was reverted completely. User-copy and exception-fixup code therefore
remain together pending a later change with explicit emitted-code comparison;
this gate will not trade verified behavior for file movement.

## Delivery strategy

- Working branch: `codex/feedback-hardening`.
- Intended base: `main`.
- Pull request: the shared ten-gate pull request.
- Planned commits: dependency contract followed by one behavior-preserving
  responsibility extraction per commit.
- Actual commits: `1adacc47` defines the dependency and invariant contract;
  `072d1339` extracts architecture ownership; `c88cc875` extracts Linux ABI
  definitions; descriptor and entry-state mechanism extraction pending commit.

## Review policy and outcome

Delegated closure authorized after every extraction passes the full regression,
dependency, invariant, and performance criteria. Outcome: pending.
