# x86-64 Timer Discovery v0

**Status:** Calibrated monotonic time, one-shot interrupts, and ring-3
preemption verified.

The bootstrap processor records the following CPUID timer properties:

- whether the invariant TSC property is advertised;
- whether TSC-deadline local-APIC timers are supported; and
- a reported TSC frequency, when architectural data is available.

The preferred frequency source is the crystal-clock ratio from CPUID leaf
`0x15`. The base frequency from leaf `0x16` is retained only as a nominal
fallback. Absence of either source is valid. The bootstrap calibration below
measures the TSC against an independent clock before it is used for elapsed
time. Discovery alone does not imply that a clock source is trusted.

## Bootstrap local-APIC calibration

The kernel calibrates the bootstrap processor's local-APIC timer counter using
an approximately 10 ms, mode-zero PIT channel 2 interval. The local-APIC timer
is masked throughout, uses divide-by-16 mode, and is stopped after measurement.
The legacy speaker remains disabled and its control byte is restored. A bounded
poll prevents missing or non-functional PIT hardware from hanging boot.

The same interval measures the ordered TSC before and after the PIT gate. When
CPUID does not report a TSC frequency, this calibrated value supplies the
bounded Linux monotonic clock. The resulting APIC and TSC frequencies are
sufficient for bring-up tests but are not yet production clocksources: future
hardware support must prefer ACPI-described timers, quantify calibration
error, reconcile per-CPU counters, and define behavior where the PIT is absent.

## Bounded Linux monotonic time

The static Linux proof snapshots an ordered TSC epoch on the bootstrap
processor and retains either the CPUID-reported frequency or the PIT-calibrated
fallback for its lifetime. Linux `clock_gettime` accepts `CLOCK_MONOTONIC` and
`CLOCK_MONOTONIC_RAW`; both return elapsed time from that private epoch as a
16-byte x86-64 `timespec`. Nanoseconds are always below one billion, successive
reads cannot move backward, and the complete output range is validated before
copy-out. Unsupported clock IDs return `-EINVAL` and invalid destinations
return `-EFAULT` without a partial write.
`clock_getres` reports the same source's one-tick duration, rounded up to a
normalized `timespec`; a null output pointer is accepted as a support query.

This does not provide realtime, boot-time, process/thread CPU clocks, clock
adjustment, sleeping, vDSO acceleration, cross-CPU synchronization, or a
stable epoch shared between containers.

## One-shot interrupt verification

After calibration, the kernel installs owned IDT gates for vector 32 and the
local-APIC spurious vector 255. It enables the local APIC in software, programs
a masked-by-default one-shot for approximately 10 ms, briefly enables CPU
interrupt delivery, acknowledges the timer with an APIC EOI, then disables
interrupts and masks/stops the timer again. A bounded poll turns a missing
interrupt into a boot failure instead of an indefinite wait.

## One-shot scheduler preemption

After both bootstrap domains and contexts exist, the kernel programs a
scheduler one-shot for approximately 1 ms while interrupts remain disabled. The initial
`IRETQ` enables interrupts only in ring 3. Vector 32 saves every general-purpose
register plus the hardware return frame, acknowledges the APIC, records the
interrupted control context, selects the ready service context, reloads CR3,
and returns directly to that service. The control context later resumes at its
interrupted instruction. After the service faults, the kernel retires its
domain, queues a replacement generation, and rearms the same one-shot. A second
interrupt involuntarily dispatches that replacement, which exits cleanly and
returns to the still-runnable control context. The timer is masked again before
either address space is reclaimed.

This proves bounded repeated involuntary user-context switching across domain
retirement and generation replacement, not a periodic scheduler.
There is no tick policy, quantum accounting, timer queue, deadline API, or
preemption while kernel code runs. External interrupt routing remains disabled
and no other interrupt source is intentionally unmasked.
