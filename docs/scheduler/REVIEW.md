# Detached-domain preemption: validation notes

This change addresses #83. PR #84, the native scheduler atomicity fix for #82,
is now merged. The initial publication base was `4c832ea6`; the acceptance
follow-up integrates main `c8d12653`, including the controller desktop. Attached-console
preemption and switching separate processes inside one domain are outside scope.

## Mechanism and invariants

A 50-tick domain quantum and rotating domain selection use the existing Linux
domain suspend/resume lifecycle. Pending controller input permits an earlier
handoff after five ticks. Sibling thread switches do not reset the domain budget.

Timer entry captures GPRs and aligned XSAVE state before Rust, installs kernel
GS, and exits only from a saved ring-3 context after dropping entry guards.
Resume restores exact RCX/R11, RFLAGS, stack, FS/TLS and x87/SSE/AVX state.
The final XRSTOR is in the assembly resume path.

A separate maskable IPI quiesces AP workers before the BSP changes the global
domain bank. Waiting happens outside the kernel-entry lock. Workers remain
runnable/blocked and are dispatched again after domain activation. NMI remains
TLB-only. A quiescence timeout fails closed.

The patch also banks signal-return state, retains the live domain bank on
console resume, returns EAGAIN after background work to the desktop controller's
nonblocking read loop, saturates lifecycle
statistics and permits stopping an init that has never forked. Native operation
counters retain the checked-overflow policy of the separate atomicity fix.

## Reproduction

Run `make test-scheduler` as documented in `tests/scheduler/README.md`.
The acceptance follow-up compiles static payloads, constructs a fresh offline
64 MiB cache with repository tooling, and supports QEMU TCG as well as KVM.
The public `scheduler` workflow runs 1/2/4 CPU cells and retains logs, hashes and
JSON evidence. It needs no private seed volume or contributor-host access.

The revised context oracle also requires overlapping computation intervals;
the stop/restart probe uses a continuous four-worker payload. Every case waits
for its own complete status record rather than an earlier command's prompt.
The historical measurements below are retained as original contributor evidence;
new acceptance results are recorded separately after fresh validation.

The one-CPU pre-fix control responds to three queries when the workload calls
`sched_yield()`, but a no-syscall spinner receives no controller response within
ten seconds. Its normal EFI SHA256 is
`2f4b14991c4109d8f153b05d8be7d2d17f64960694564c823e4e97d20a9920c3`.

Context fixtures use different seeds per domain and worker and test all GPRs,
CF/DF, stack/red zone, FS/TLS, x87 value/control, MXCSR and YMM0/7/15. Hardware
CPUID masks prove AP execution; the existing getcpu stub is not used as evidence.
The harness requires both computation reports and clean console-read/exit.
Signal fixtures compute inside SIGUSR1, return through sigreturn and complete
five timed sleeps. Stop/start reuses the runtime slot; there is no delete action
in the current domain-control ABI.

## Historical publication validation

Code commit: `2659ba595fe8d5880f69816c56c29df4aff872ac` (documentation/evidence
commits do not change the tested executable). QEMU/KVM, q35, 1 GiB RAM,
`host,+smep,+smap,+umip`, 1/2/4 vCPUs, Xeon E-2236 test host. The existing rig
uses a 48-bit IOMMU address setting and OVMF 4M firmware.

Normal EFI SHA256:
`46d13567ec18a53cc99bd60df83a552d71bf393a6d18aef952b885535a2bba61`.

Pinned gzip layer SHA256 values:

| Fixture | SHA256 |
|---|---|
| Spinner/cooperative | `3fd6326c4b61c2a4ce3647dbcba67e04ce88411decb47ee101fde13e7ca773b4` |
| Context 1/2/4 workers | `5b317996f86234858442b69ec4b78d60eb42c30b37c97a19126a0e863931881e` |
| Signal/timed wait | `5e0e1a4abea66bdb022f6ac50dac9c7e11411a220fabbd9ad1b7f2b6596a6df0` |

- Normal build and `make check`: PASS.
- Native scheduler host checks: 11/11 PASS (includes 1024 lifecycle handoffs).
- Kernel policy unit tests: 5/5 PASS.
- No-syscall spinner on 1/2/4 CPUs: PASS, including three stop/start cycles per
  topology and a 30-query one-CPU soak.
- Two-domain context on 1/2/4 CPUs: PASS, hardware CPU masks 1/3/15 and clean exits.
- Two-domain signal/timed waits and active four-worker stop/start: PASS.
- Four siblings per domain on one CPU: PASS, both domains finish after a
  75-second interval without input and exit cleanly.
- `make test`: FAIL with the same invalid-opcode/-EINTR signature as the earlier
  baseline. The exact publication run excerpt is in `evidence/full-boot-failure.txt`.

Machine-readable targeted results are under `evidence/`. Their elapsed response
measurements have 50 ms polling resolution; the first query includes startup.
They are not interrupt-latency microbenchmarks.

## Qualification limits

The earlier candidate passed targeted 1/2/4-CPU runs, two-domain context tests,
four siblings per domain on one CPU, signal/timed-wait tests and stop/start.
Its full boot suite and the unmodified local base both failed at Linux probe
RIP 4214196, vector 6, RAX=-EINTR (-4), followed by a missing native-volume IOMMU
marker. This failure was not fixed as part of the scheduler changes.

The rig has no cached pinned Rocky/sysbench assets. Existing Rocky multi-domain,
detached, futex, shared-page and thread-context suites have not been qualified;
focused static fixtures do not replace those compatibility suites. Runtime
extended-state setup requires XSAVE/AVX; other hardware is not qualified.

Three earlier one-domain, one-worker timing pairs used the same binary and
100 million assembly iterations on a shared Xeon E-2236 host. Median elapsed time
was 4,675,567 us before and 4,664,076 us after (ratio 0.99754), within run-to-run
variation. The baseline computation passed but its later console cleanup failed;
its harness result remained ERROR. This is neither an acceleration claim nor a
measurement of per-handoff overhead, and it is not a benchmark of the rebased
publication build.
