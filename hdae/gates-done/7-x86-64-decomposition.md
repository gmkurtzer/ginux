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
- [x] Extracted the Linux syscall entry dispatcher into
  `cpu/linux_syscall.rs`, leaving x86 entry assembly in the architecture
  module and Linux subsystem implementations behind the dispatcher boundary.
- [x] Extracted Linux clock, elapsed/realtime accounting, sleep, interval
  timer, rusage, times, sysinfo, and related system-information mechanisms
  into `cpu/linux_time.rs`.
- [x] Extracted Linux futex wait, wake, wait-vector, and requeue mechanisms into
  `cpu/linux_futex.rs`.
- [x] Extracted Linux select, poll, ppoll, wait completion, scheduling-yield,
  and epoll creation mechanisms into `cpu/linux_poll.rs`.
- [x] Extracted Linux signal actions, masks, pending queues, timed waits,
  delivery, signal frames, return, process signaling, and default dispositions
  into `cpu/linux_signal.rs`.

## Remaining work

- None for this gate. Further Linux subsystem decomposition can continue only
  behind the same per-extraction regression rule.

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
- The Linux syscall-dispatch extraction passed the same complete regression
  sequence and preserved the 450-global ownership inventory.
- The Linux time-mechanism extraction passed the same complete regression
  sequence, including live timekeeping validation in the controller runtime.
- The Linux futex extraction passed the same complete regression sequence.
- The Linux polling extraction passed the same complete regression sequence.
- The Linux signal extraction passed formatting, architecture checks,
  `make check`, and the full deterministic QEMU verification boot, including
  signal delivery, Linux ELF/runtime compatibility, clean shutdown,
  `GINUX_KERNEL_OK`, and `Boot test passed`.
- `make benchmark-compare` confirmed the pinned single-vCPU comparison inputs
  and preserved the recorded medians: CPU 1.152x faster, memory 1.095x slower,
  threads 1.012x faster, and mutex 1.089x faster than the matched Rocky/Linux
  data.

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

The x86 `SYSCALL`, exception-entry, and return assembly remains in `cpu.rs` so
its local labels, fixup ranges, and Rust entry symbols stay in one compilation
unit. APIC, timer, paging, PCI, and IOMMU hardware mechanisms were already
separate architecture modules. Verification probes that directly exercise
private Linux runtime state remain with that state; moving them alone would
broaden visibility without creating a new ownership boundary.

## Delivery strategy

- Working branch: `codex/feedback-hardening`.
- Intended base: `main`.
- Pull request: the shared ten-gate pull request.
- Planned commits: dependency contract followed by one behavior-preserving
  responsibility extraction per commit.
- Actual commits: `1adacc47`, `072d1339`, `c88cc875`, `c807aba4`, `bb20c71b`,
  `839961f5`, `d1ad424d`, `d7d1eafb`, `6d0971b6`, `c25d0f28`, and `e692ddd4`.

## Review policy and outcome

Delegated closure authorized after every extraction passes the full regression,
dependency, invariant, and performance criteria. Outcome: passed and closed
under the approved batch delegation.
