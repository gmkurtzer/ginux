use core::arch::{asm, global_asm, x86_64::__cpuid};
use core::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering};

use super::{
    apic::{LocalApic, Mode},
    cpu,
};

const CPUID_BASIC_MAX: u32 = 0;
const CPUID_FEATURES: u32 = 1;
const CPUID_TSC_CRYSTAL: u32 = 0x15;
const CPUID_PROCESSOR_FREQUENCY: u32 = 0x16;
const CPUID_EXTENDED_MAX: u32 = 0x8000_0000;
const CPUID_ADVANCED_POWER: u32 = 0x8000_0007;
const TSC_DEADLINE: u32 = 1 << 24;
const INVARIANT_TSC: u32 = 1 << 8;
const HERTZ_PER_MHZ: u64 = 1_000_000;
const APIC_LVT_TIMER: u32 = 0x320;
const APIC_INITIAL_COUNT: u32 = 0x380;
const APIC_CURRENT_COUNT: u32 = 0x390;
const APIC_DIVIDE_CONFIGURATION: u32 = 0x3e0;
const APIC_TIMER_MASKED: u32 = 1 << 16;
const APIC_DIVIDE_BY_16: u32 = 0x3;
const PIT_FREQUENCY_HZ: u64 = 1_193_182;
const PIT_CALIBRATION_TICKS: u16 = 11_932;
const PIT_COMMAND: u16 = 0x43;
const PIT_CHANNEL_2: u16 = 0x42;
const PIT_SPEAKER_CONTROL: u16 = 0x61;
const PIT_CHANNEL_2_MODE_0: u8 = 0xb0;
const PIT_GATE_2: u8 = 1 << 0;
const PIT_SPEAKER_ENABLE: u8 = 1 << 1;
const PIT_OUTPUT_2: u8 = 1 << 5;
const PIT_POLL_LIMIT: usize = 100_000_000;
const APIC_EOI: u32 = 0xb0;
const APIC_SPURIOUS_VECTOR: u32 = 0xf0;
const APIC_SOFTWARE_ENABLE: u32 = 1 << 8;
const TIMER_VECTOR: u8 = 32;
const SPURIOUS_VECTOR: u8 = 255;
const INTERRUPT_POLL_LIMIT: usize = 100_000_000;

static TIMER_FIRED: AtomicBool = AtomicBool::new(false);
static INTERRUPT_APIC_BASE: AtomicU64 = AtomicU64::new(0);
static INTERRUPT_APIC_MODE: AtomicU8 = AtomicU8::new(0);

global_asm!(
    r#"
    .text
    .global ginux_apic_timer_entry
ginux_apic_timer_entry:
    cld
    push rax
    push rbx
    push rcx
    push rdx
    push rsi
    push rdi
    push rbp
    push r8
    push r9
    push r10
    push r11
    push r12
    push r13
    push r14
    push r15
    mov r12, rsp
    and rsp, -16
    call {timer_handler}
    mov rsp, r12
    pop r15
    pop r14
    pop r13
    pop r12
    pop r11
    pop r10
    pop r9
    pop r8
    pop rbp
    pop rdi
    pop rsi
    pop rdx
    pop rcx
    pop rbx
    pop rax
    iretq

    .global ginux_apic_spurious_entry
ginux_apic_spurious_entry:
    iretq
"#,
    timer_handler = sym timer_interrupt_handler,
);

extern "C" {
    fn ginux_apic_timer_entry();
    fn ginux_apic_spurious_entry();
}

pub(crate) struct LocalApicCalibration {
    pub(crate) counter_hz: u64,
    pub(crate) divider: u32,
}

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

/// Calibrates the masked local-APIC timer against a one-shot PIT interval.
///
/// # Safety
///
/// The caller must own the legacy PIT and local APIC, keep interrupts
/// disabled, and provide the exclusive mapped limit for xAPIC MMIO.
pub(crate) unsafe fn calibrate_local_apic(
    apic: &LocalApic,
    mapped_limit: usize,
) -> Option<LocalApicCalibration> {
    if apic.mode == Mode::XApic && apic.physical_base.checked_add(0x400)? > mapped_limit as u64 {
        return None;
    }

    apic.write(APIC_LVT_TIMER, APIC_TIMER_MASKED);
    apic.write(APIC_DIVIDE_CONFIGURATION, APIC_DIVIDE_BY_16);
    apic.write(APIC_INITIAL_COUNT, u32::MAX);

    let original_control = in_u8(PIT_SPEAKER_CONTROL);
    out_u8(PIT_SPEAKER_CONTROL, original_control & !PIT_GATE_2);
    out_u8(PIT_COMMAND, PIT_CHANNEL_2_MODE_0);
    out_u8(PIT_CHANNEL_2, PIT_CALIBRATION_TICKS as u8);
    out_u8(PIT_CHANNEL_2, (PIT_CALIBRATION_TICKS >> 8) as u8);
    out_u8(
        PIT_SPEAKER_CONTROL,
        (original_control & !PIT_SPEAKER_ENABLE) | PIT_GATE_2,
    );

    let mut completed = false;
    for _ in 0..PIT_POLL_LIMIT {
        if in_u8(PIT_SPEAKER_CONTROL) & PIT_OUTPUT_2 != 0 {
            completed = true;
            break;
        }
        core::hint::spin_loop();
    }

    let current = apic.read(APIC_CURRENT_COUNT);
    apic.write(APIC_INITIAL_COUNT, 0);
    out_u8(PIT_SPEAKER_CONTROL, original_control);
    if !completed {
        return None;
    }

    let elapsed = u64::from(u32::MAX - current);
    let counter_hz = elapsed
        .checked_mul(PIT_FREQUENCY_HZ)?
        .checked_div(u64::from(PIT_CALIBRATION_TICKS))?;
    (counter_hz != 0).then_some(LocalApicCalibration {
        counter_hz,
        divider: 16,
    })
}

/// Delivers and acknowledges one local-APIC timer interrupt.
///
/// Interrupts are enabled only while waiting for the owned one-shot source and
/// are disabled before this function returns.
pub(crate) unsafe fn verify_local_apic_interrupt(
    apic: &LocalApic,
    calibration: &LocalApicCalibration,
) -> bool {
    if !cpu::install_interrupt_gate(TIMER_VECTOR, ginux_apic_timer_entry as *const () as usize)
        || !cpu::install_interrupt_gate(
            SPURIOUS_VECTOR,
            ginux_apic_spurious_entry as *const () as usize,
        )
    {
        return false;
    }

    INTERRUPT_APIC_BASE.store(apic.physical_base, Ordering::SeqCst);
    INTERRUPT_APIC_MODE.store(
        if apic.mode == Mode::X2Apic { 1 } else { 0 },
        Ordering::SeqCst,
    );
    TIMER_FIRED.store(false, Ordering::SeqCst);

    let spurious = apic.read(APIC_SPURIOUS_VECTOR);
    apic.write(
        APIC_SPURIOUS_VECTOR,
        (spurious & !0xff) | u32::from(SPURIOUS_VECTOR) | APIC_SOFTWARE_ENABLE,
    );
    apic.write(APIC_DIVIDE_CONFIGURATION, APIC_DIVIDE_BY_16);
    apic.write(APIC_LVT_TIMER, u32::from(TIMER_VECTOR));
    let initial_count = (calibration.counter_hz / 100).clamp(1, u64::from(u32::MAX)) as u32;
    apic.write(APIC_INITIAL_COUNT, initial_count);

    asm!("sti", options(nomem, nostack));
    for _ in 0..INTERRUPT_POLL_LIMIT {
        if TIMER_FIRED.load(Ordering::SeqCst) {
            break;
        }
        core::hint::spin_loop();
    }
    asm!("cli", options(nomem, nostack));
    apic.write(APIC_LVT_TIMER, APIC_TIMER_MASKED | u32::from(TIMER_VECTOR));
    apic.write(APIC_INITIAL_COUNT, 0);
    TIMER_FIRED.load(Ordering::SeqCst)
}

extern "sysv64" fn timer_interrupt_handler() {
    TIMER_FIRED.store(true, Ordering::SeqCst);
    let apic = LocalApic {
        id: 0,
        mode: if INTERRUPT_APIC_MODE.load(Ordering::SeqCst) == 1 {
            Mode::X2Apic
        } else {
            Mode::XApic
        },
        physical_base: INTERRUPT_APIC_BASE.load(Ordering::SeqCst),
    };
    // SAFETY: The verifier initializes these values before unmasking the timer,
    // and the handler runs with exclusive bootstrap-processor APIC ownership.
    unsafe { apic.write(APIC_EOI, 0) };
}

unsafe fn in_u8(port: u16) -> u8 {
    let value: u8;
    asm!(
        "in al, dx",
        in("dx") port,
        out("al") value,
        options(nomem, nostack, preserves_flags)
    );
    value
}

unsafe fn out_u8(port: u16, value: u8) {
    asm!(
        "out dx, al",
        in("dx") port,
        in("al") value,
        options(nomem, nostack, preserves_flags)
    );
}
