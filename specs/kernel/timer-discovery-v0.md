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
not read the TSC, program an APIC timer, enable interrupts, expose time to
userspace, or make scheduling decisions.
