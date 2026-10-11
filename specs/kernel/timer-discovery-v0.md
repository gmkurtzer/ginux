# x86-64 Timer Discovery v0

**Status:** Read-only capability discovery.

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
