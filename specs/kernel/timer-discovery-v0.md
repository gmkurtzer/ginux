# x86-64 Timer Discovery v0

**Status:** Calibrated one-shot interrupt and ring-3 preemption verified.

The bootstrap processor records the following CPUID timer properties:

- whether the invariant TSC property is advertised;
- whether TSC-deadline local-APIC timers are supported; and
- a reported TSC frequency, when architectural data is available.

The preferred frequency source is the crystal-clock ratio from CPUID leaf
`0x15`. The base frequency from leaf `0x16` is retained only as a nominal
fallback. Absence of either source is valid and requires later calibration
against an independent clock before the TSC can measure wall-clock time.

Discovery does not imply that a clock source is trusted. This revision does
not read the TSC, enable interrupts, expose time to userspace, or make
scheduling decisions.

## Bootstrap local-APIC calibration

The kernel calibrates the bootstrap processor's local-APIC timer counter using
an approximately 10 ms, mode-zero PIT channel 2 interval. The local-APIC timer
is masked throughout, uses divide-by-16 mode, and is stopped after measurement.
The legacy speaker remains disabled and its control byte is restored. A bounded
poll prevents missing or non-functional PIT hardware from hanging boot.

The resulting frequency describes the APIC counter at the recorded divider. It
is sufficient for bring-up tests but is not yet a production clocksource:
future hardware support must prefer ACPI-described timers, quantify calibration
error, and define behavior where the PIT is absent.

## One-shot interrupt verification

After calibration, the kernel installs owned IDT gates for vector 32 and the
local-APIC spurious vector 255. It enables the local APIC in software, programs
a masked-by-default one-shot for approximately 10 ms, briefly enables CPU
interrupt delivery, acknowledges the timer with an APIC EOI, then disables
interrupts and masks/stops the timer again. A bounded poll turns a missing
interrupt into a boot failure instead of an indefinite wait.

## One-shot scheduler preemption

After both bootstrap domains and contexts exist, the kernel programs a second
one-shot for approximately 1 ms while interrupts remain disabled. The initial
`IRETQ` enables interrupts only in ring 3. Vector 32 saves every general-purpose
register plus the hardware return frame, acknowledges the APIC, records the
interrupted control context, selects the ready service context, reloads CR3,
and returns directly to that service. The control context later resumes at its
interrupted instruction and completes normally. The timer is masked again
before either address space is reclaimed.

This proves one involuntary user-context switch, not a periodic scheduler.
There is no tick policy, quantum accounting, timer queue, deadline API, or
preemption while kernel code runs. External interrupt routing remains disabled
and no other interrupt source is intentionally unmasked.
