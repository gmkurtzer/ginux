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
The workflow also runs the full deterministic `make test` suite on macOS/QEMU
TCG. The ordinary PR `qemu` check runs a smoke test, so its green result alone
must not be described as full integration passage.

The historical measurements below are retained as original contributor evidence.
Current acceptance is tracked by the public `scheduler` workflow and its
downloadable `scheduler-tcg-1cpu`, `scheduler-tcg-2cpu`, and
`scheduler-tcg-4cpu` artifacts. Each artifact identifies the tested merge commit,
EFI, compiler and payloads; the PR head and merge commit are different identities.

Cross-domain computation overlap uses `CLOCK_REALTIME`, whose epoch is shared
by the domains. Ginux's `CLOCK_MONOTONIC` epoch is per domain and cannot establish
such overlap. Initial acceptance run `41801727049` passed its register, CPU-mask,
signal, lifecycle and oracle-control checks, but its overlap calculation used the
wrong clock. That overlap result is superseded by the corrected-clock reruns.

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

## Acceptance follow-up, 2026-11-04

The source-built KVM matrix passes all twelve cases after integration with
`c8d12653`: cooperative/spinner/context on 1/2/4 CPUs, four siblings per domain
on one CPU, signal/timed waits, and continuous four-worker stop/restart.
The context and sibling cases were rerun with the shared-clock correction:
observed computation overlap was 16.98 / 20.34 / 23.63 seconds on 1/2/4 CPUs and
76.32 seconds for the sibling case. CPU masks were 1/3/15 as required.
The continuous-worker test additionally passes on TCG and requires the workload
to remain active after each of its three restarts.

Compact contributor evidence, with input identities and sentinel reports, is in
[`evidence/acceptance-20261104.json`](evidence/acceptance-20261104.json).
It is supplementary evidence from a source export, not an independent CI result.
The source-built pre-fix `7fdc880e` control passes its cooperative case and
reproduces spinner starvation on both the contributor TCG host and public CI.

The public [scheduler workflow](https://github.com/gmkurtzer/ginux/actions/workflows/scheduler.yml)
runs the corrected fixtures from the checked-out PR merge tree. Its status and
artifacts for the current PR head, together with `full-integration`, are the
maintainer acceptance evidence. Old successful runs do not certify later code.

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

On the current integration with `c8d12653`, contributor Linux/QEMU 8.2 TCG
`make test` failed during post-ABI-rejection native-volume recovery. A fresh
unmodified `c8d12653` run on the same host also failed in native-volume lifecycle,
but at replacement recovery on the recovery boot. The failure stages differ;
this comparison does not prove identical causes. The dedicated public
`full-integration` job is the independent full-suite acceptance path. Neither
local failure is counted as a pass, and no native-volume fix is included here.

The historical candidate passed targeted 1/2/4-CPU runs, two-domain context tests,
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
