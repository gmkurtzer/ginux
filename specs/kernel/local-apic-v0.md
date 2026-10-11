# x86-64 Local APIC Discovery v0

**Status:** Read-only bootstrap-processor discovery.

The kernel requires an architectural local APIC on x86-64. After installing
its exception tables, it:

- verifies local-APIC support with CPUID leaf 1;
- reads `IA32_APIC_BASE` and requires the global-enable bit;
- records the legacy APIC ID from CPUID;
- distinguishes active xAPIC and x2APIC modes; and
- records the physical APIC base address.

This revision does not enable or reconfigure the APIC, access its MMIO page,
route external interrupts, configure its timer, or execute `STI`. Interrupts
remain disabled throughout the boot test. Timer setup must first establish a
clock source and a deterministic calibration contract.
