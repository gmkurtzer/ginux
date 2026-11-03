use super::*;

const BACKGROUND_DOMAIN_QUANTUM_TICKS: u32 = 50;
const BACKGROUND_INPUT_POLL_TICKS: u32 = 5;

pub(super) fn start_linux_domain_slice() {
    X86_RUNTIME
        .LINUX_RUNTIME_BACKGROUND_TICKS
        .store(0, Ordering::Release);
    X86_RUNTIME
        .LINUX_RUNTIME_BACKGROUND_YIELD_REQUESTED
        .store(false, Ordering::Release);
    crate::arch::x86_64::smp::resume_linux_runtime_dispatch();
}

pub(super) fn finish_suspended_linux_state() -> bool {
    // The syscall/interrupt entry guard has already been dropped. Workers can
    // finish their entry, save their context and return to their AP dispatcher.
    crate::arch::x86_64::smp::wait_linux_runtime_quiescence();
    let saved = if X86_RUNTIME
        .LINUX_RUNTIME_INTERRUPT_SUSPENDED
        .swap(false, Ordering::AcqRel)
    {
        reset_extended_user_state()
    } else {
        save_container_extended_state()
    };
    saved && execution::preserve_suspended_thread_extended_state()
}

pub(super) fn suspend_background_linux_interrupt(
    frame: &UserInterruptFrame,
    extended: Option<&ExtendedState>,
) -> bool {
    if !X86_RUNTIME.LINUX_RUNTIME_MODE.load(Ordering::Acquire)
        || current_linux_runtime_cpu_index() != 0
        || !active_container_slot().is_some_and(|slot| {
            matches!(
                container_state(slot),
                control_abi::DOMAIN_STATE_BOOTING | control_abi::DOMAIN_STATE_RUNNING
            ) && !X86_RUNTIME.CONTAINER_CONSOLE_ATTACHED[slot].load(Ordering::Acquire)
        })
    {
        return false;
    }
    let ticks = X86_RUNTIME
        .LINUX_RUNTIME_BACKGROUND_TICKS
        .fetch_add(1, Ordering::AcqRel)
        .saturating_add(1);
    let due = ticks >= BACKGROUND_DOMAIN_QUANTUM_TICKS
        || ticks >= BACKGROUND_INPUT_POLL_TICKS && crate::serial_input_ready();
    if !due {
        return false;
    }
    X86_RUNTIME
        .LINUX_RUNTIME_BACKGROUND_YIELD_REQUESTED
        .store(true, Ordering::Release);
    let (Some(interrupted), Some(current), Some(extended)) = (
        frame.user_context(USER_CODE_SELECTOR, USER_DATA_SELECTOR),
        current_linux_thread_handle(),
        extended,
    ) else {
        return false;
    };
    if !save_linux_interrupt_context(current, interrupted, active_fs_base()) {
        return false;
    }
    let Some(context) = linux_thread_return_context(current) else {
        return false;
    };
    if !save_suspended_linux_runtime(context, 0, 0, true) {
        return false;
    }
    X86_RUNTIME.LINUX_RUNTIME_SUSPEND_REGISTERS[LINUX_SYSCALL_FRAME_REGISTERS]
        .store(interrupted.rcx, Ordering::Release);
    X86_RUNTIME.LINUX_RUNTIME_SUSPEND_REGISTERS[LINUX_SYSCALL_FRAME_REGISTERS + 1]
        .store(interrupted.r11, Ordering::Release);
    if !execution::save_interrupt_container_state(extended) {
        crate::kernel_fail("ginux: interrupted domain state unavailable\r\n");
    }
    X86_RUNTIME
        .LINUX_RUNTIME_INTERRUPT_SUSPENDED
        .store(true, Ordering::Release);
    true
}

pub(crate) fn quiesce_linux_runtime_interrupt(
    frame: &UserInterruptFrame,
    extended: Option<&ExtendedState>,
) -> bool {
    let _entry = crate::sync::kernel_entry_lock();
    if !crate::arch::x86_64::smp::linux_runtime_quiescence_requested()
        || current_linux_runtime_cpu_index() == 0
    {
        return false;
    }
    let (Some(interrupted), Some(current), Some(extended)) = (
        frame.user_context(USER_CODE_SELECTOR, USER_DATA_SELECTOR),
        current_linux_thread_handle(),
        extended,
    ) else {
        return false;
    };
    if !save_linux_interrupt_context(current, interrupted, active_fs_base())
        || !execution::save_interrupt_thread_state(usize::from(current.slot), extended)
    {
        crate::kernel_fail("ginux: interrupted AP context unavailable\r\n");
    }
    let slot = usize::from(current.slot);
    X86_RUNTIME.LINUX_THREAD_SIGNAL_MASKS[slot].store(
        X86_RUNTIME.ELF_PROBE_SIGNAL_MASK.load(Ordering::Acquire),
        Ordering::Release,
    );
    X86_RUNTIME.LINUX_THREAD_STATES[slot].store(LINUX_THREAD_READY, Ordering::Release);
    X86_RUNTIME
        .LINUX_CURRENT_THREAD_SLOT
        .store(LINUX_THREAD_SLOT_NONE, Ordering::Release);
    true
}

pub(super) fn dispatch_ready_linux_runtime_threads(root: memory::FrameLease) {
    let _entry = crate::sync::kernel_entry_lock();
    if cfg!(feature = "verification-boot") {
        return;
    }
    for slot in 0..LINUX_THREAD_LIMIT {
        if X86_RUNTIME.LINUX_THREAD_STATES[slot].load(Ordering::Acquire) != LINUX_THREAD_READY {
            continue;
        }
        let Some(handle) = linux_thread_handle(slot) else {
            continue;
        };
        let Some(context) = linux_thread_return_context(handle) else {
            continue;
        };
        let Some(general) = linux_thread_resume_general(handle, context) else {
            continue;
        };
        if !crate::arch::x86_64::smp::begin_linux_runtime_thread(
            root,
            handle.slot,
            handle.generation,
            handle.tid,
            context.rip,
            context.rsp,
            context.fs_base,
            general,
            context.rflags,
        ) {
            break;
        }
    }
}
