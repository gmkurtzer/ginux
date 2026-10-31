# Gate 11.6 SMP Thread-Context Integrity

Gate 11.6 corrects a timing-sensitive Linux register corruption exposed by the
scaled four-vCPU sysbench thread-yield workload.

## Diagnosis

The failing instruction was `lock incq (%r11)` in sysbench's histogram update.
At the fault, live `r11` was `0x10206`, a flags value and invalid low pointer.
The active thread slot at the same RIP still contained saved
`r11=0x8000ebb028`, the valid shared-counter pointer. CPU, slot, generation,
state, and the remaining live and saved registers agreed.

The save path was therefore correct. The mismatch occurred when a syscall on
one thread selected a target that had previously been preempted by an
interrupt. The common SYSRET path must set `rcx` to the return RIP and `r11` to
RFLAGS, which is correct for returning from the current syscall. It is not
correct for an asynchronously saved target, where both registers contain live
application state.

## Correction

Commit `5e3393e3` retains SYSRET for ordinary same-thread syscall returns. Every
scheduler-backed cross-thread syscall switch now validates the target handle
and full saved context, publishes exact `rax`, `rcx`, and `r11`, and uses the
existing IRET epilogue. Remote runtime-CPU dispatch consumes the same exact
15-register image instead of reconstructing syscall-clobbered registers.

The deterministic verification path fills all 15 general registers with
distinct sentinel values, saves an interrupt context, prepares the exact IRET
return, and checks the syscall frame, RIP, RSP, RFLAGS, TLS, thread identity,
slot generation, and exact `rax`/`rcx`/`r11`. It then restores the original
context and verifies stale handles remain rejected.

## Evidence

- `make test-smp` passed 1/2/4-vCPU verification boots and both repeated
  two-vCPU stress passes. The same suite retains low, absent, read-only-write,
  supervisor-only, and NX-execute rejection coverage.
- `make test-rocky-smp-thread-context` passed nine 2-vCPU/2-thread and nine
  4-vCPU/4-thread sysbench thread-yield samples. Every workload marker was zero;
  no user fault, panic, register mismatch, stale generation, or hang appeared.
- `make check` passed formatting, lint, unit, compatibility, writable-global,
  runtime-owner, synchronization, privilege-entry, user-access, and architecture
  checks.
- `make test-rocky-smp-futex` passed the private-futex regression.
- `make test-rocky-smp-shared-page` passed all 2/4-vCPU shared writable-page
  regressions.
- Each QEMU harness reaped its child. Exact-volume `lsof` inspection found no
  remaining owner of the benchmark volume.

Raw live logs are generated under
`build/benchmark/rocky-smp-thread-context/regression/`; build output is not
committed. This evidence covers the tested x86-64 QEMU environment and does not
claim broader hardware qualification.
