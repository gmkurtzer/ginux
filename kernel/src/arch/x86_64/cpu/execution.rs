use core::{
    arch::{
        asm,
        x86_64::{__cpuid, __cpuid_count},
    },
    sync::atomic::Ordering,
};

use super::{
    active_container_slot, current_container_slot, current_linux_thread_slot, CPUID_AVX,
    CPUID_EXTENDED_STATE, CPUID_SSE, CPUID_XSAVE, CPU_ARCHITECTURE, CR0_EM, CR0_MP, CR0_NE,
    CR4_OSFXSR, CR4_OSXMMEXCPT, CR4_OSXSAVE, LINUX_PROCESS_RUNTIME_INSTANCE_LIMIT,
    LINUX_THREAD_LIMIT, X86_MUTABLE_RUNTIME, X86_RUNTIME,
};
use crate::{control_abi, linux_process};

pub(super) const EXTENDED_STATE_SIZE: usize = 1024;

#[derive(Clone, Copy)]
#[repr(C, align(64))]
pub(super) struct ExtendedState(pub(super) [u8; EXTENDED_STATE_SIZE]);

pub(crate) fn enable_extended_user_state() -> bool {
    let features = __cpuid(1);
    if features.edx & CPUID_SSE == 0
        || features.ecx & (CPUID_XSAVE | CPUID_AVX) != CPUID_XSAVE | CPUID_AVX
        || __cpuid(0).eax < CPUID_EXTENDED_STATE
        || __cpuid_count(CPUID_EXTENDED_STATE, 0).eax & 7 != 7
    {
        return false;
    }
    let mut cr0: u64;
    let mut cr4: u64;
    unsafe {
        asm!("mov {}, cr0", out(reg) cr0, options(nomem, nostack, preserves_flags));
        cr0 = (cr0 | CR0_MP | CR0_NE) & !CR0_EM;
        asm!("mov cr0, {}", in(reg) cr0, options(nomem, nostack, preserves_flags));
        asm!("mov {}, cr4", out(reg) cr4, options(nomem, nostack, preserves_flags));
        cr4 |= CR4_OSFXSR | CR4_OSXMMEXCPT | CR4_OSXSAVE;
        asm!("mov cr4, {}", in(reg) cr4, options(nomem, nostack, preserves_flags));
        asm!(
            "xsetbv",
            in("ecx") 0_u32,
            in("eax") 7_u32,
            in("edx") 0_u32,
            options(nostack, preserves_flags)
        );
    }
    let state = __cpuid_count(CPUID_EXTENDED_STATE, 0);
    let enabled = state.ebx as usize <= EXTENDED_STATE_SIZE && reset_extended_user_state();
    CPU_ARCHITECTURE
        .extended_user_state_enabled
        .store(enabled, Ordering::Release);
    enabled
}

pub(super) fn reset_extended_user_state() -> bool {
    let default_mxcsr = 0x1f80_u32;
    unsafe {
        asm!("fninit", options(nostack));
        asm!("ldmxcsr [{}]", in(reg) &default_mxcsr, options(nostack));
        asm!("vzeroall", options(nostack));
    }
    true
}

pub(super) fn restore_container_extended_state() -> bool {
    if !CPU_ARCHITECTURE
        .extended_user_state_enabled
        .load(Ordering::Acquire)
    {
        return true;
    }
    let Some(slot) = active_container_slot() else {
        return false;
    };
    if !X86_RUNTIME.CONTAINER_EXTENDED_STATE_VALID[slot].load(Ordering::Acquire) {
        return reset_extended_user_state();
    }
    let state = unsafe {
        X86_MUTABLE_RUNTIME
            .container_extended_states
            .get()
            .cast_const()
            .cast::<ExtendedState>()
            .add(slot)
    };
    unsafe {
        asm!(
            "xrstor64 [{}]",
            in(reg) state,
            in("eax") 7_u32,
            in("edx") 0_u32,
            options(nostack)
        );
    }
    true
}

pub(super) fn save_container_extended_state() -> bool {
    if !CPU_ARCHITECTURE
        .extended_user_state_enabled
        .load(Ordering::Acquire)
    {
        return true;
    }
    let Some(slot) = active_container_slot() else {
        return false;
    };
    let state = unsafe {
        X86_MUTABLE_RUNTIME
            .container_extended_states
            .get()
            .cast::<ExtendedState>()
            .add(slot)
    };
    unsafe {
        asm!(
            "xsave64 [{}]",
            in(reg) state,
            in("eax") 7_u32,
            in("edx") 0_u32,
            options(nostack)
        );
    }
    X86_RUNTIME.CONTAINER_EXTENDED_STATE_VALID[slot].store(true, Ordering::Release);
    reset_extended_user_state()
}

pub(super) fn discard_container_extended_state() -> bool {
    if !CPU_ARCHITECTURE
        .extended_user_state_enabled
        .load(Ordering::Acquire)
    {
        return true;
    }
    let Some(slot) = active_container_slot() else {
        return false;
    };
    X86_RUNTIME.CONTAINER_EXTENDED_STATE_VALID[slot].store(false, Ordering::Release);
    reset_extended_user_state()
}

fn linux_process_extended_state_index(pid: u32) -> Option<(usize, usize)> {
    let instance = current_container_slot().unwrap_or(control_abi::DOMAIN_LIMIT);
    let slot = linux_process::live_process_runtime_slot(pid)?;
    (instance < LINUX_PROCESS_RUNTIME_INSTANCE_LIMIT && slot < linux_process::PROCESS_RUNTIME_LIMIT)
        .then_some((instance, slot))
}

fn linux_thread_extended_state_index(pid: u32, thread: usize) -> Option<(usize, usize, usize)> {
    let (instance, process) = linux_process_extended_state_index(pid)?;
    (thread < LINUX_THREAD_LIMIT).then_some((instance, process, thread))
}

fn save_linux_thread_extended_state(pid: u32, thread: usize) -> bool {
    if !CPU_ARCHITECTURE
        .extended_user_state_enabled
        .load(Ordering::Acquire)
    {
        return true;
    }
    let Some((instance, process, thread)) = linux_thread_extended_state_index(pid, thread) else {
        return false;
    };
    let state = unsafe {
        &mut (*X86_MUTABLE_RUNTIME.process_extended_states.get())[instance][process][thread]
    };
    unsafe {
        asm!(
            "xsave64 [{}]",
            in(reg) state,
            in("eax") 7_u32,
            in("edx") 0_u32,
            options(nostack)
        );
        (*X86_MUTABLE_RUNTIME.process_extended_state_valid.get())[instance][process][thread] = true;
    }
    true
}

pub(super) fn restore_linux_thread_extended_state(pid: u32, thread: usize) -> bool {
    if !CPU_ARCHITECTURE
        .extended_user_state_enabled
        .load(Ordering::Acquire)
    {
        return true;
    }
    let Some((instance, process, thread)) = linux_thread_extended_state_index(pid, thread) else {
        return false;
    };
    if !unsafe {
        (*X86_MUTABLE_RUNTIME.process_extended_state_valid.get())[instance][process][thread]
    } {
        return reset_extended_user_state();
    }
    let state =
        unsafe { &(*X86_MUTABLE_RUNTIME.process_extended_states.get())[instance][process][thread] };
    unsafe {
        asm!(
            "xrstor64 [{}]",
            in(reg) state,
            in("eax") 7_u32,
            in("edx") 0_u32,
            options(nostack)
        );
    }
    true
}

pub(super) fn save_linux_process_extended_state(pid: u32) -> bool {
    if !CPU_ARCHITECTURE
        .extended_user_state_enabled
        .load(Ordering::Acquire)
    {
        return true;
    }
    let Some(thread) = current_linux_thread_slot() else {
        // A process that has retired its final thread has no live architectural
        // state to preserve during terminal lifecycle cleanup.
        return true;
    };
    save_linux_thread_extended_state(pid, thread)
}

pub(super) fn restore_linux_process_extended_state(pid: u32) -> bool {
    if !CPU_ARCHITECTURE
        .extended_user_state_enabled
        .load(Ordering::Acquire)
    {
        return true;
    }
    let Some(thread) = current_linux_thread_slot() else {
        return false;
    };
    restore_linux_thread_extended_state(pid, thread)
}

pub(super) fn switch_linux_thread_extended_state(from: usize, to: usize) -> bool {
    let pid = super::current_linux_process_pid();
    save_linux_thread_extended_state(pid, from) && restore_linux_thread_extended_state(pid, to)
}

pub(super) fn initialize_linux_thread_extended_state(thread: usize) -> bool {
    let Some(parent) = current_linux_thread_slot() else {
        return false;
    };
    let pid = super::current_linux_process_pid();
    if !save_linux_thread_extended_state(pid, parent) {
        return false;
    }
    let (Some((instance, process, parent)), Some((_, _, thread))) = (
        linux_thread_extended_state_index(pid, parent),
        linux_thread_extended_state_index(pid, thread),
    ) else {
        return false;
    };
    unsafe {
        (*X86_MUTABLE_RUNTIME.process_extended_states.get())[instance][process][thread] =
            (*X86_MUTABLE_RUNTIME.process_extended_states.get())[instance][process][parent];
        (*X86_MUTABLE_RUNTIME.process_extended_state_valid.get())[instance][process][thread] =
            (*X86_MUTABLE_RUNTIME.process_extended_state_valid.get())[instance][process][parent];
    }
    true
}

pub(super) fn discard_linux_thread_extended_state(thread: usize) {
    let pid = super::current_linux_process_pid();
    if let Some((instance, process, thread)) = linux_thread_extended_state_index(pid, thread) {
        unsafe {
            (*X86_MUTABLE_RUNTIME.process_extended_state_valid.get())[instance][process][thread] =
                false;
        }
    }
}

pub(super) fn clone_linux_process_extended_state(parent: u32, child: u32) -> bool {
    let (Some((parent_instance, parent_slot)), Some((child_instance, child_slot))) = (
        linux_process_extended_state_index(parent),
        linux_process_extended_state_index(child),
    ) else {
        return false;
    };
    if parent_instance != child_instance {
        return false;
    }
    unsafe {
        (*X86_MUTABLE_RUNTIME.process_extended_states.get())[child_instance][child_slot] =
            (*X86_MUTABLE_RUNTIME.process_extended_states.get())[parent_instance][parent_slot];
        (*X86_MUTABLE_RUNTIME.process_extended_state_valid.get())[child_instance][child_slot] =
            (*X86_MUTABLE_RUNTIME.process_extended_state_valid.get())[parent_instance][parent_slot];
    }
    true
}

pub(super) fn discard_linux_process_extended_state(pid: u32) {
    if let Some((instance, slot)) = linux_process_extended_state_index(pid) {
        unsafe {
            (*X86_MUTABLE_RUNTIME.process_extended_state_valid.get())[instance][slot] =
                [false; LINUX_THREAD_LIMIT]
        };
    }
}
