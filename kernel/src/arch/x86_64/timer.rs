use core::arch::x86_64::__cpuid;

const CPUID_BASIC_MAX: u32 = 0;
const CPUID_FEATURES: u32 = 1;
const CPUID_TSC_CRYSTAL: u32 = 0x15;
const CPUID_PROCESSOR_FREQUENCY: u32 = 0x16;
const CPUID_EXTENDED_MAX: u32 = 0x8000_0000;
const CPUID_ADVANCED_POWER: u32 = 0x8000_0007;
const TSC_DEADLINE: u32 = 1 << 24;
const INVARIANT_TSC: u32 = 1 << 8;
const HERTZ_PER_MHZ: u64 = 1_000_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FrequencySource {
    ArchitecturalRatio,
    NominalProcessorFrequency,
    Unavailable,
}

pub(crate) struct Capabilities {
    pub(crate) invariant_tsc: bool,
    pub(crate) tsc_deadline: bool,
    pub(crate) frequency_hz: Option<u64>,
    pub(crate) frequency_source: FrequencySource,
}

/// Reads architectural timer capabilities without programming a timer.
pub(crate) fn discover() -> Capabilities {
    let maximum_basic = __cpuid(CPUID_BASIC_MAX).eax;
    let features = __cpuid(CPUID_FEATURES);
    let maximum_extended = __cpuid(CPUID_EXTENDED_MAX).eax;
    let invariant_tsc = maximum_extended >= CPUID_ADVANCED_POWER
        && __cpuid(CPUID_ADVANCED_POWER).edx & INVARIANT_TSC != 0;

    let architectural_frequency = (maximum_basic >= CPUID_TSC_CRYSTAL)
        .then(|| __cpuid(CPUID_TSC_CRYSTAL))
        .and_then(|ratio| {
            let denominator = u64::from(ratio.eax);
            let numerator = u64::from(ratio.ebx);
            let crystal_hz = u64::from(ratio.ecx);
            if denominator == 0 || numerator == 0 || crystal_hz == 0 {
                return None;
            }
            crystal_hz
                .checked_mul(numerator)
                .map(|frequency| frequency / denominator)
                .filter(|frequency| *frequency != 0)
        });

    let nominal_frequency = (maximum_basic >= CPUID_PROCESSOR_FREQUENCY)
        .then(|| __cpuid(CPUID_PROCESSOR_FREQUENCY))
        .and_then(|frequency| {
            (frequency.eax != 0)
                .then(|| u64::from(frequency.eax).checked_mul(HERTZ_PER_MHZ))
                .flatten()
        });

    let (frequency_hz, frequency_source) = if let Some(frequency) = architectural_frequency {
        (Some(frequency), FrequencySource::ArchitecturalRatio)
    } else if let Some(frequency) = nominal_frequency {
        (Some(frequency), FrequencySource::NominalProcessorFrequency)
    } else {
        (None, FrequencySource::Unavailable)
    };

    Capabilities {
        invariant_tsc,
        tsc_deadline: features.ecx & TSC_DEADLINE != 0,
        frequency_hz,
        frequency_source,
    }
}
