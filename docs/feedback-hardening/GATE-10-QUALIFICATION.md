# Gate 10 Qualification and Comparison

Gate 10 qualifies commit `94db6ccc` against the published Ginux research
baseline. Every command in the frozen matrix passed. The complete evidence set
is indexed in [`gate-10-results/README.md`](gate-10-results/README.md).

## Functional qualification

- Static checks, architecture invariants, and the compatibility ledger passed.
- Both reported hardening findings remained resolved under the reproduction
  script; tested invalid userspace pointers returned errors without panicking.
- The deterministic runtime and Linux ABI suites passed. The controller report
  covered 23 interface groups and 375 syscall identifiers.
- The 1/2/4-vCPU topology matrix and two repeated 2-vCPU stress passes passed.
- Persistent storage recovery/reset, OCI cold-pull and reboot cache reuse,
  Rocky systemd boot/login, detached controller responsiveness, `dnf update`,
  and concurrent Rocky domains passed.
- Exact-volume checks found no QEMU process retaining the tested volumes after
  the harnesses exited.

## Published single-CPU comparison

The final boot median was 14.702 seconds versus the published 12.860-second
Ginux baseline, a 14.32% increase within the frozen 25% limit. All four
sysbench medians remained within the frozen 20% tolerance: CPU was 3.94% lower,
memory 2.74% lower, threads 11.36% lower, and mutex time 2.39% higher. The
matched Rocky-on-Linux measurements remain context, not a gate threshold.

## SMP scaling

With the same userspace, binary, QEMU model, and memory, moving from one
vCPU/one thread to two vCPUs/two threads raised CPU throughput from 273.550 to
768.730 events/s (2.810x) and memory throughput from 604.960 to 1422.990 MiB/s
(2.352x). Both exceed the frozen 1.500x and 1.250x requirements.

## Defects found during qualification

Qualification exposed and retained regression coverage for runtime process
lifecycle cleanup, Linux exec/service startup, AP thread ownership, fair
scheduling, materialized address-space capacity, and bounded `poll` without a
scheduler peer. The final poll behavior now follows Linux timeout semantics and
does not perturb ordinary scheduler selection.

## Limits

This evidence covers the tested x86-64 QEMU TCG environment and workloads. It
does not establish production security, broad hardware support, complete Linux
compatibility, or general performance parity. Storage and networking were
qualified through their frozen functional integration tests; no separate
numerical storage or networking baseline was part of the frozen matrix. The
floating upstream Alpine integration image remains a known separate follow-up.
