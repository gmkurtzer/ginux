use core::arch::{asm, x86_64::__cpuid};

const CPUID_FEATURES: u32 = 1;
const CPUID_APIC: u32 = 1 << 9;
const CPUID_X2APIC: u32 = 1 << 21;
const IA32_APIC_BASE: u32 = 0x1b;
const APIC_GLOBAL_ENABLE: u64 = 1 << 11;
const APIC_X2_MODE: u64 = 1 << 10;
const APIC_BASE_MASK: u64 = 0x000f_ffff_ffff_f000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Mode {
    XApic,
    X2Apic,
}

pub(crate) struct LocalApic {
    pub(crate) id: u8,
    pub(crate) mode: Mode,
    pub(crate) physical_base: u64,
}

impl LocalApic {
    /// Reads a local-APIC register.
    ///
    /// # Safety
    ///
    /// In xAPIC mode, the APIC MMIO page must be mapped uncached at the
    /// discovered physical base. The register must be aligned and below
    /// offset `0x400`.
    pub(crate) unsafe fn read(&self, register: u32) -> u32 {
        debug_assert!(register < 0x400 && register.is_multiple_of(16));
        match self.mode {
            Mode::XApic => {
                ((self.physical_base as usize + register as usize) as *const u32).read_volatile()
            }
            Mode::X2Apic => read_msr(0x800 + register / 16) as u32,
        }
    }

    /// Writes a local-APIC register under the mapping requirements of `read`.
    pub(crate) unsafe fn write(&self, register: u32, value: u32) {
        debug_assert!(register < 0x400 && register.is_multiple_of(16));
        match self.mode {
            Mode::XApic => ((self.physical_base as usize + register as usize) as *mut u32)
                .write_volatile(value),
            Mode::X2Apic => write_msr(0x800 + register / 16, u64::from(value)),
        }
    }
}

/// Discovers the bootstrap processor's already-enabled local APIC.
///
/// This is deliberately read-only. Interrupt delivery, MMIO access, timer
/// configuration, and global interrupt enable remain outside this milestone.
pub(crate) fn discover() -> Option<LocalApic> {
    // CPUID leaf 1 is mandatory on x86-64 and has no side effects.
    let features = __cpuid(CPUID_FEATURES);
    if features.edx & CPUID_APIC == 0 {
        return None;
    }

    // SAFETY: CPUID reports an architectural APIC, for which IA32_APIC_BASE is
    // defined. The kernel runs at CPL0 with exceptions installed.
    let base_msr = unsafe { read_msr(IA32_APIC_BASE) };
    if base_msr & APIC_GLOBAL_ENABLE == 0 {
        return None;
    }

    let mode = if features.ecx & CPUID_X2APIC != 0 && base_msr & APIC_X2_MODE != 0 {
        Mode::X2Apic
    } else {
        Mode::XApic
    };
    let physical_base = base_msr & APIC_BASE_MASK;
    if physical_base == 0 {
        return None;
    }

    Some(LocalApic {
        id: (features.ebx >> 24) as u8,
        mode,
        physical_base,
    })
}

unsafe fn read_msr(msr: u32) -> u64 {
    let low: u32;
    let high: u32;
    asm!(
        "rdmsr",
        in("ecx") msr,
        out("eax") low,
        out("edx") high,
        options(nomem, nostack, preserves_flags)
    );
    (u64::from(high) << 32) | u64::from(low)
}

unsafe fn write_msr(msr: u32, value: u64) {
    asm!(
        "wrmsr",
        in("ecx") msr,
        in("eax") value as u32,
        in("edx") (value >> 32) as u32,
        options(nomem, nostack, preserves_flags)
    );
}
