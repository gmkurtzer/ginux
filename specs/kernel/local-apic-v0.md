# x86-64 Local APIC Discovery v0

**Status:** Bootstrap discovery and one-shot timer operation verified in QEMU.

The kernel requires an architectural local APIC on x86-64. After installing
its exception tables, it:

- verifies local-APIC support with CPUID leaf 1;
- reads `IA32_APIC_BASE` and requires the global-enable bit;
- records the legacy APIC ID from CPUID;
- distinguishes active xAPIC and x2APIC modes; and
- records the physical APIC base address.

In xAPIC mode, the complete register window must translate through the checked
supervisor-only higher-half direct map. Volatile register reads, writes, timer
calibration, interrupt acknowledgement, and saved timer-state reconstruction
use that high alias rather than the physical identity address. x2APIC register
access continues to use MSRs.

The QEMU gate calibrates a masked local-APIC timer against PIT channel 2,
delivers and acknowledges a one-shot interrupt, and later uses one-shots for
ring-3 scheduler preemption. This revision does not route external interrupts,
program an I/O APIC, initialize secondary processors, or define a general timer
service.
