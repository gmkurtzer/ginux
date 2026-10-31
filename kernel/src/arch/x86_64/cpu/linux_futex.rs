use super::*;

pub(super) fn linux_futex_live_return(
    output: *mut LinuxSyscallFrame,
    caller: LinuxUserContext,
    address: u64,
    operation: u64,
    value: u64,
    timeout: u64,
) -> u64 {
    if !address.is_multiple_of(size_of::<u32>() as u64) {
        return LINUX_EINVAL;
    }
    let mut bytes = [0_u8; size_of::<u32>()];
    if !copy_from_linux_user(address, &mut bytes) {
        return LINUX_EFAULT;
    }
    match operation {
        0 | 128 => {
            if u64::from(u32::from_ne_bytes(bytes)) != value {
                return LINUX_EAGAIN;
            }
            let Some(current) = current_linux_thread_handle() else {
                return LINUX_ESRCH;
            };
            let target = ready_linux_thread_handle();
            if target.is_none() && current_linux_runtime_cpu_index() == 0 {
                if crate::arch::x86_64::smp::linux_runtime_thread_slot().is_some() {
                    // Linux futex callers must tolerate spurious wakeups and
                    // recheck the userspace word. Keep the main CPU on its
                    // normal SYSRET path while the remote worker runs;
                    // returning through the kernel runner would discard the
                    // live syscall frame.
                    return 0;
                }
                // Preserve the single-CPU verification runtime's existing
                // unsupported result when no local or remote peer can wake
                // this thread. The SMP compatibility path above is the only
                // behavior changed by this gate.
                return LINUX_ENOSYS;
            }
            let Some(tls) = linux_thread_context(current) else {
                return LINUX_ESRCH;
            };
            let deadline = if timeout == 0 {
                0
            } else {
                let mut timeout_bytes = [0_u8; 16];
                if !copy_from_linux_user(timeout, &mut timeout_bytes) {
                    return LINUX_EFAULT;
                }
                let Some(duration) = linux_timespec_nanoseconds(&timeout_bytes) else {
                    return LINUX_EINVAL;
                };
                if duration == 0 {
                    return LINUX_ETIMEDOUT;
                }
                let Some(deadline) =
                    linux_elapsed_nanoseconds().and_then(|now| now.checked_add(duration))
                else {
                    return LINUX_EINVAL;
                };
                deadline
            };
            if !save_linux_thread_return_context(current, caller)
                || !block_current_linux_thread(
                    LINUX_THREAD_WAIT_FUTEX,
                    address,
                    u64::from(u32::MAX),
                    deadline,
                )
            {
                return LINUX_EINVAL;
            }
            let Some(target) = target else {
                X86_RUNTIME
                    .LINUX_FUTEX_WAIT_BLOCK_COUNT
                    .fetch_add(1, Ordering::SeqCst);
                return LINUX_RETURN_TO_KERNEL;
            };
            let mut target_registers = LinuxSyscallFrame::ZERO;
            let mut target_rip = 0;
            let mut target_rsp = 0;
            let mut target_rflags = 0;
            let mut target_rax = 0;
            if !install_linux_thread_return(
                target,
                &mut target_registers,
                &mut target_rip,
                &mut target_rsp,
                &mut target_rflags,
                &mut target_rax,
            ) {
                let slot = usize::from(current.slot);
                X86_RUNTIME.LINUX_THREAD_WAIT_KINDS[slot]
                    .store(LINUX_THREAD_WAIT_NONE, Ordering::SeqCst);
                X86_RUNTIME.LINUX_THREAD_WAIT_KEYS[slot].store(0, Ordering::SeqCst);
                X86_RUNTIME.LINUX_THREAD_WAIT_OUTPUTS[slot].store(0, Ordering::SeqCst);
                X86_RUNTIME.LINUX_THREAD_WAIT_DEADLINES[slot].store(0, Ordering::SeqCst);
                X86_RUNTIME.LINUX_THREAD_STATES[slot].store(LINUX_THREAD_RUNNING, Ordering::SeqCst);
                X86_RUNTIME
                    .LINUX_CURRENT_THREAD_SLOT
                    .store(current.slot, Ordering::SeqCst);
                let _ = install_linux_tls_context(tls);
                return LINUX_EINVAL;
            }
            let selected = LinuxUserContext {
                registers: target_registers,
                rip: target_rip,
                rsp: target_rsp,
                rflags: target_rflags,
                rax: target_rax,
                fs_base: active_fs_base(),
            };
            if !write_linux_thread_live_return(output, target, selected) {
                return LINUX_EINVAL;
            }
            X86_RUNTIME
                .LINUX_LIVE_RETURN_VALIDATED
                .store(true, Ordering::SeqCst);
            X86_RUNTIME
                .LINUX_FUTEX_WAIT_BLOCK_COUNT
                .fetch_add(1, Ordering::SeqCst);
            LINUX_RETURN_FROM_IRET
        }
        1 | 129 => {
            let woken = wake_linux_futex(address, u32::try_from(value).unwrap_or(u32::MAX));
            if woken != 0 {
                X86_RUNTIME
                    .LINUX_FUTEX_EXPLICIT_WAKE_COUNT
                    .fetch_add(1, Ordering::SeqCst);
            }
            u64::from(woken)
        }
        _ => LINUX_EINVAL,
    }
}

pub(super) fn linux_futex2_flags_valid(flags: u64) -> bool {
    const FUTEX2_SIZE_U32: u64 = 2;
    const FUTEX2_PRIVATE: u64 = 128;
    flags & !(FUTEX2_SIZE_U32 | FUTEX2_PRIVATE) == 0 && flags & 3 == FUTEX2_SIZE_U32
}

pub(super) fn linux_futex_wake(address: u64, mask: u64, count: u64, flags: u64) -> u64 {
    if !linux_futex2_flags_valid(flags)
        || !address.is_multiple_of(size_of::<u32>() as u64)
        || mask == 0
        || mask > u64::from(u32::MAX)
        || count > i32::MAX as u64
    {
        return LINUX_EINVAL;
    }
    let mut bytes = [0_u8; size_of::<u32>()];
    if !copy_from_linux_user(address, &mut bytes) {
        return LINUX_EFAULT;
    }
    u64::from(wake_linux_futex_masked(address, mask as u32, count as u32))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn linux_futex_wait_live_return(
    output: *mut LinuxSyscallFrame,
    caller: LinuxUserContext,
    address: u64,
    value: u64,
    mask: u64,
    flags: u64,
    timeout: u64,
    clock: u64,
) -> u64 {
    if !linux_futex2_flags_valid(flags)
        || !address.is_multiple_of(size_of::<u32>() as u64)
        || value > u64::from(u32::MAX)
        || mask == 0
        || mask > u64::from(u32::MAX)
        || !matches!(clock, 0 | 1)
    {
        return LINUX_EINVAL;
    }
    let mut bytes = [0_u8; size_of::<u32>()];
    if !copy_from_linux_user(address, &mut bytes) {
        return LINUX_EFAULT;
    }
    if u64::from(u32::from_ne_bytes(bytes)) != value {
        return LINUX_EAGAIN;
    }
    let deadline = if timeout == 0 {
        0
    } else {
        let mut timeout_bytes = [0_u8; 16];
        if !copy_from_linux_user(timeout, &mut timeout_bytes) {
            return LINUX_EFAULT;
        }
        let Some(deadline) = linux_timespec_nanoseconds(&timeout_bytes) else {
            return LINUX_EINVAL;
        };
        let Some(monotonic_now) = linux_elapsed_nanoseconds() else {
            return LINUX_ENOSYS;
        };
        let deadline = if clock == LINUX_CLOCK_REALTIME {
            let Some((seconds, nanoseconds)) = linux_realtime_time() else {
                return LINUX_ENOSYS;
            };
            let Some(realtime_now) = seconds
                .checked_mul(NANOSECONDS_PER_SECOND)
                .and_then(|seconds| seconds.checked_add(nanoseconds))
            else {
                return LINUX_EINVAL;
            };
            monotonic_now.saturating_add(deadline.saturating_sub(realtime_now))
        } else {
            deadline
        };
        if monotonic_now >= deadline {
            return LINUX_ETIMEDOUT;
        }
        deadline
    };
    let Some(current) = current_linux_thread_handle() else {
        return LINUX_ESRCH;
    };
    let Some(target) = ready_linux_thread_handle() else {
        return LINUX_ENOSYS;
    };
    let Some(tls) = linux_thread_context(current) else {
        return LINUX_ESRCH;
    };
    if !save_linux_thread_return_context(current, caller)
        || !block_current_linux_thread(LINUX_THREAD_WAIT_FUTEX, address, mask, deadline)
    {
        return LINUX_EINVAL;
    }
    let mut target_registers = LinuxSyscallFrame::ZERO;
    let mut target_rip = 0;
    let mut target_rsp = 0;
    let mut target_rflags = 0;
    let mut target_rax = 0;
    if !install_linux_thread_return(
        target,
        &mut target_registers,
        &mut target_rip,
        &mut target_rsp,
        &mut target_rflags,
        &mut target_rax,
    ) {
        let slot = usize::from(current.slot);
        X86_RUNTIME.LINUX_THREAD_WAIT_KINDS[slot].store(LINUX_THREAD_WAIT_NONE, Ordering::SeqCst);
        X86_RUNTIME.LINUX_THREAD_WAIT_KEYS[slot].store(0, Ordering::SeqCst);
        X86_RUNTIME.LINUX_THREAD_WAIT_OUTPUTS[slot].store(0, Ordering::SeqCst);
        X86_RUNTIME.LINUX_THREAD_WAIT_DEADLINES[slot].store(0, Ordering::SeqCst);
        X86_RUNTIME.LINUX_THREAD_STATES[slot].store(LINUX_THREAD_RUNNING, Ordering::SeqCst);
        X86_RUNTIME
            .LINUX_CURRENT_THREAD_SLOT
            .store(current.slot, Ordering::SeqCst);
        let _ = install_linux_tls_context(tls);
        return LINUX_EINVAL;
    }
    let selected = LinuxUserContext {
        registers: target_registers,
        rip: target_rip,
        rsp: target_rsp,
        rflags: target_rflags,
        rax: target_rax,
        fs_base: active_fs_base(),
    };
    if !write_linux_thread_live_return(output, target, selected) {
        return LINUX_EINVAL;
    }
    X86_RUNTIME
        .LINUX_LIVE_RETURN_VALIDATED
        .store(true, Ordering::SeqCst);
    X86_RUNTIME
        .LINUX_FUTEX_WAIT_BLOCK_COUNT
        .fetch_add(1, Ordering::SeqCst);
    LINUX_RETURN_FROM_IRET
}

#[allow(clippy::too_many_arguments)]
pub(super) fn linux_futex_waitv_live_return(
    output: *mut LinuxSyscallFrame,
    caller: LinuxUserContext,
    waiters: u64,
    count: u64,
    flags: u64,
    timeout: u64,
    clock: u64,
) -> u64 {
    if flags != 0 || count == 0 || count > 128 {
        return LINUX_EINVAL;
    }
    if count != 1 {
        return LINUX_ENOSYS;
    }
    let mut waiter = [0_u8; 24];
    if !copy_from_linux_user(waiters, &mut waiter) {
        return LINUX_EFAULT;
    }
    let value = u64::from_ne_bytes(waiter[..8].try_into().unwrap());
    let address = u64::from_ne_bytes(waiter[8..16].try_into().unwrap());
    let waiter_flags = u32::from_ne_bytes(waiter[16..20].try_into().unwrap());
    let reserved = u32::from_ne_bytes(waiter[20..24].try_into().unwrap());
    if reserved != 0 {
        return LINUX_EINVAL;
    }
    linux_futex_wait_live_return(
        output,
        caller,
        address,
        value,
        u64::from(u32::MAX),
        u64::from(waiter_flags),
        timeout,
        clock,
    )
}

pub(super) fn linux_futex_requeue(
    waiters: u64,
    flags: u64,
    wake_count: u64,
    requeue_count: u64,
) -> u64 {
    if flags != 0 || waiters == 0 || wake_count > i32::MAX as u64 || requeue_count > i32::MAX as u64
    {
        return LINUX_EINVAL;
    }
    let mut entries = [0_u8; 48];
    if !copy_from_linux_user(waiters, &mut entries) {
        return LINUX_EFAULT;
    }
    let parse = |offset: usize| {
        (
            u64::from_ne_bytes(entries[offset..offset + 8].try_into().unwrap()),
            u64::from_ne_bytes(entries[offset + 8..offset + 16].try_into().unwrap()),
            u32::from_ne_bytes(entries[offset + 16..offset + 20].try_into().unwrap()),
            u32::from_ne_bytes(entries[offset + 20..offset + 24].try_into().unwrap()),
        )
    };
    let (expected, source, source_flags, source_reserved) = parse(0);
    let (_, target, target_flags, target_reserved) = parse(24);
    if source_reserved != 0
        || target_reserved != 0
        || source_flags != target_flags
        || !linux_futex2_flags_valid(u64::from(source_flags))
        || !source.is_multiple_of(size_of::<u32>() as u64)
        || !target.is_multiple_of(size_of::<u32>() as u64)
        || expected > u64::from(u32::MAX)
    {
        return LINUX_EINVAL;
    }
    let mut source_bytes = [0_u8; 4];
    let mut target_bytes = [0_u8; 4];
    if !copy_from_linux_user(source, &mut source_bytes)
        || !copy_from_linux_user(target, &mut target_bytes)
    {
        return LINUX_EFAULT;
    }
    if u64::from(u32::from_ne_bytes(source_bytes)) != expected {
        return LINUX_EAGAIN;
    }
    let woken = wake_linux_futex(source, wake_count as u32);
    let mut requeued = 0_u32;
    for slot in 0..LINUX_THREAD_LIMIT {
        if requeued == requeue_count as u32 {
            break;
        }
        if X86_RUNTIME.LINUX_THREAD_STATES[slot].load(Ordering::SeqCst) == LINUX_THREAD_BLOCKED
            && X86_RUNTIME.LINUX_THREAD_WAIT_KINDS[slot].load(Ordering::SeqCst)
                == LINUX_THREAD_WAIT_FUTEX
            && X86_RUNTIME.LINUX_THREAD_WAIT_KEYS[slot].load(Ordering::SeqCst) == source
        {
            X86_RUNTIME.LINUX_THREAD_WAIT_KEYS[slot].store(target, Ordering::SeqCst);
            requeued += 1;
        }
    }
    u64::from(woken + requeued)
}
