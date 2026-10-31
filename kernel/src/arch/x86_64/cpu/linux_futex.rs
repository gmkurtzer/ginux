use super::*;

static LINUX_PROFILE_FIRST_FUTEX_OPERATION: AtomicU64 = AtomicU64::new(u64::MAX);
static LINUX_PROFILE_FUTEX_FAST_MISS_CPUS: AtomicU64 = AtomicU64::new(0);
static LINUX_PROFILE_FUTEX_OPERATION_COUNTS: [AtomicU64; 512] = [const { AtomicU64::new(0) }; 512];

fn profile_linux_futex_operation(operation: u64) {
    if !cfg!(feature = "linux-boot-profile") {
        return;
    }
    if LINUX_PROFILE_FIRST_FUTEX_OPERATION
        .compare_exchange(u64::MAX, operation, Ordering::Relaxed, Ordering::Relaxed)
        .is_ok()
    {
        serial_write("ginux: profile futex operation: ");
        serial_write_u64(operation);
        serial_write("\r\n");
    }
    if let Some(count) = usize::try_from(operation)
        .ok()
        .and_then(|operation| LINUX_PROFILE_FUTEX_OPERATION_COUNTS.get(operation))
    {
        count.fetch_add(1, Ordering::Relaxed);
    }
}

pub(super) fn write_linux_futex_profile() {
    if !cfg!(feature = "linux-boot-profile") {
        return;
    }
    for (operation, count) in LINUX_PROFILE_FUTEX_OPERATION_COUNTS.iter().enumerate() {
        let count = count.swap(0, Ordering::Relaxed);
        if count == 0 {
            continue;
        }
        serial_write("ginux: profile futex operation/count: ");
        serial_write_u64(operation as u64);
        serial_write("/");
        serial_write_u64(count);
        serial_write("\r\n");
    }
}

pub(super) fn linux_remote_futex_wait_fast(
    address: u64,
    operation: u64,
    value: u64,
    timeout: u64,
    bitset: u64,
) -> Option<u64> {
    const FUTEX_COMMAND_MASK: u64 = 0x7f;
    const FUTEX_WAIT_BITSET: u64 = 9;
    const FUTEX_PRIVATE_FLAG: u64 = 128;
    const FUTEX_CLOCK_REALTIME: u64 = 256;

    profile_linux_futex_operation(operation);
    if !address.is_multiple_of(size_of::<u32>() as u64) {
        return None;
    }
    if matches!(operation, 1 | 129) {
        let limit = u32::try_from(value).unwrap_or(u32::MAX);
        if limit == 0 {
            return Some(0);
        }
        let _coordination = LINUX_FUTEX_COORDINATION.lock();
        let mut woken = 0_u32;
        for slot in 0..LINUX_THREAD_LIMIT {
            if woken == limit {
                break;
            }
            if X86_RUNTIME.LINUX_THREAD_STATES[slot].load(Ordering::Acquire) != LINUX_THREAD_BLOCKED
                && X86_RUNTIME.LINUX_THREAD_WAIT_KEYS[slot].load(Ordering::Acquire) == address
                && X86_RUNTIME.LINUX_THREAD_WAIT_KINDS[slot]
                    .compare_exchange(
                        LINUX_THREAD_WAIT_FUTEX,
                        LINUX_THREAD_WAIT_NONE,
                        Ordering::AcqRel,
                        Ordering::Acquire,
                    )
                    .is_ok()
            {
                X86_RUNTIME.LINUX_THREAD_WAIT_KEYS[slot].store(0, Ordering::Release);
                X86_RUNTIME.LINUX_THREAD_WAIT_OUTPUTS[slot].store(0, Ordering::Release);
                woken += 1;
            }
        }
        if woken < limit {
            woken = woken.saturating_add(wake_linux_futex_masked_locked(
                address,
                u32::MAX,
                limit - woken,
            ));
        }
        return Some(u64::from(woken));
    }
    let command = operation & FUTEX_COMMAND_MASK;
    let flags = operation & !FUTEX_COMMAND_MASK;
    if command == FUTEX_WAIT_BITSET {
        if flags & !(FUTEX_PRIVATE_FLAG | FUTEX_CLOCK_REALTIME) != 0
            || bitset == 0
            || bitset > u64::from(u32::MAX)
        {
            return Some(LINUX_EINVAL);
        }
        let mut bytes = [0_u8; size_of::<u32>()];
        if !address_space::copy_from_current_linux_user_fast(address, &mut bytes) {
            return None;
        }
        if u64::from(u32::from_ne_bytes(bytes)) != value {
            return Some(LINUX_EAGAIN);
        }
        crate::arch::x86_64::smp::linux_runtime_thread_slot()?;
        if timeout != 0 {
            let mut timeout_bytes = [0_u8; 16];
            if !address_space::copy_from_current_linux_user_fast(timeout, &mut timeout_bytes) {
                return None;
            }
            let Some(requested) = linux_timespec_nanoseconds(&timeout_bytes) else {
                return Some(LINUX_EINVAL);
            };
            let expired = if flags & FUTEX_CLOCK_REALTIME == 0 {
                linux_elapsed_nanoseconds()? >= requested
            } else {
                let (seconds, nanoseconds) = linux_realtime_time()?;
                let Some(realtime_now) = seconds
                    .checked_mul(NANOSECONDS_PER_SECOND)
                    .and_then(|seconds| seconds.checked_add(nanoseconds))
                else {
                    return Some(LINUX_EINVAL);
                };
                realtime_now >= requested
            };
            if expired {
                return Some(LINUX_ETIMEDOUT);
            }
        }
        // A remote application CPU can satisfy the predicate concurrently.
        // Linux permits this wait to return spuriously, and returning directly
        // preserves the coordinator's exact live syscall frame.
        return Some(0);
    }
    if !matches!(operation, 0 | 128) || timeout != 0 {
        return None;
    }
    let mut bytes = [0_u8; size_of::<u32>()];
    if !address_space::copy_from_current_linux_user_fast(address, &mut bytes) {
        // The ordinary path can materialize a valid lazy Linux mapping before
        // deciding whether the address is actually invalid.
        return None;
    }
    if u64::from(u32::from_ne_bytes(bytes)) != value {
        return Some(LINUX_EAGAIN);
    }
    let remote = crate::arch::x86_64::smp::linux_runtime_thread_slot();
    if remote.is_none() {
        let cpu = current_linux_runtime_cpu_index();
        if cfg!(feature = "linux-boot-profile")
            && cpu < 64
            && LINUX_PROFILE_FUTEX_FAST_MISS_CPUS.fetch_or(1 << cpu, Ordering::Relaxed) & (1 << cpu)
                == 0
        {
            serial_write("ginux: profile futex fast miss cpu/remote: ");
            serial_write_u64(cpu as u64);
            serial_write("/0\r\n");
        }
        return None;
    }
    if current_linux_runtime_cpu_index() == 0
        && (0..LINUX_THREAD_LIMIT).any(|slot| {
            X86_RUNTIME.LINUX_THREAD_STATES[slot].load(Ordering::Acquire) == LINUX_THREAD_READY
                && !crate::arch::x86_64::smp::linux_runtime_thread_is_remote(slot)
        })
    {
        if cfg!(feature = "linux-boot-profile")
            && LINUX_PROFILE_FUTEX_FAST_MISS_CPUS.fetch_or(1 << 32, Ordering::Relaxed) & (1 << 32)
                == 0
        {
            serial_write("ginux: profile futex fast miss cpu/remote/local-ready: 0/1/1\r\n");
        }
        // Let the ordinary futex path save the coordinator and run the local
        // worker instead of consuming CPU 0 in a polling wait.
        return None;
    }
    let coordination = LINUX_FUTEX_COORDINATION.lock();
    if !address_space::copy_from_current_linux_user_fast(address, &mut bytes) {
        return Some(LINUX_EFAULT);
    }
    if u64::from(u32::from_ne_bytes(bytes)) != value {
        return Some(LINUX_EAGAIN);
    }
    let slot = current_linux_thread_slot()?;
    if X86_RUNTIME.LINUX_THREAD_WAIT_KINDS[slot].load(Ordering::Acquire) != LINUX_THREAD_WAIT_NONE {
        return None;
    }
    X86_RUNTIME.LINUX_THREAD_WAIT_KEYS[slot].store(address, Ordering::Relaxed);
    X86_RUNTIME.LINUX_THREAD_WAIT_OUTPUTS[slot].store(u64::from(u32::MAX), Ordering::Relaxed);
    if X86_RUNTIME.LINUX_THREAD_WAIT_KINDS[slot]
        .compare_exchange(
            LINUX_THREAD_WAIT_NONE,
            LINUX_THREAD_WAIT_FUTEX,
            Ordering::AcqRel,
            Ordering::Acquire,
        )
        .is_err()
    {
        X86_RUNTIME.LINUX_THREAD_WAIT_KEYS[slot].store(0, Ordering::Release);
        X86_RUNTIME.LINUX_THREAD_WAIT_OUTPUTS[slot].store(0, Ordering::Release);
        return None;
    }
    drop(coordination);

    let finish = |result| {
        let _coordination = LINUX_FUTEX_COORDINATION.lock();
        let _ = X86_RUNTIME.LINUX_THREAD_WAIT_KINDS[slot].compare_exchange(
            LINUX_THREAD_WAIT_FUTEX,
            LINUX_THREAD_WAIT_NONE,
            Ordering::AcqRel,
            Ordering::Acquire,
        );
        X86_RUNTIME.LINUX_THREAD_WAIT_KEYS[slot].store(0, Ordering::Release);
        X86_RUNTIME.LINUX_THREAD_WAIT_OUTPUTS[slot].store(0, Ordering::Release);
        Some(result)
    };

    // Keep the coordinating thread outside the coarse kernel-entry lock while
    // application CPUs run. The userspace futex word is authoritative; a
    // changed value completes the wait. Bound the polling interval so unusual
    // pthread handshakes retain Linux-permitted spurious-wakeup progress.
    for _ in 0..4096 {
        if X86_RUNTIME.LINUX_THREAD_WAIT_KINDS[slot].load(Ordering::Acquire)
            != LINUX_THREAD_WAIT_FUTEX
        {
            return finish(0);
        }
        if !address_space::copy_from_current_linux_user_fast(address, &mut bytes) {
            return finish(LINUX_EFAULT);
        }
        if u64::from(u32::from_ne_bytes(bytes)) != value {
            return finish(0);
        }
        core::hint::spin_loop();
    }
    finish(0)
}

pub(super) fn linux_futex_live_return(
    output: *mut LinuxSyscallFrame,
    caller: LinuxUserContext,
    address: u64,
    operation: u64,
    value: u64,
    timeout: u64,
    bitset: u64,
) -> u64 {
    const FUTEX_COMMAND_MASK: u64 = 0x7f;
    const FUTEX_WAIT: u64 = 0;
    const FUTEX_WAKE: u64 = 1;
    const FUTEX_WAIT_BITSET: u64 = 9;
    const FUTEX_WAKE_BITSET: u64 = 10;
    const FUTEX_PRIVATE_FLAG: u64 = 128;
    const FUTEX_CLOCK_REALTIME: u64 = 256;

    profile_linux_futex_operation(operation);
    let command = operation & FUTEX_COMMAND_MASK;
    let flags = operation & !FUTEX_COMMAND_MASK;
    if !address.is_multiple_of(size_of::<u32>() as u64)
        || flags & !(FUTEX_PRIVATE_FLAG | FUTEX_CLOCK_REALTIME) != 0
        || flags & FUTEX_CLOCK_REALTIME != 0 && command != FUTEX_WAIT_BITSET
    {
        return LINUX_EINVAL;
    }
    let mut bytes = [0_u8; size_of::<u32>()];
    if !copy_from_linux_user(address, &mut bytes) {
        return LINUX_EFAULT;
    }
    if matches!(command, FUTEX_WAKE | FUTEX_WAKE_BITSET) {
        let mask = if command == FUTEX_WAKE_BITSET {
            let Ok(mask) = u32::try_from(bitset) else {
                return LINUX_EINVAL;
            };
            if mask == 0 {
                return LINUX_EINVAL;
            }
            mask
        } else {
            u32::MAX
        };
        let woken =
            wake_linux_futex_masked(address, mask, u32::try_from(value).unwrap_or(u32::MAX));
        if woken != 0 {
            X86_RUNTIME
                .LINUX_FUTEX_EXPLICIT_WAKE_COUNT
                .fetch_add(1, Ordering::SeqCst);
        }
        return u64::from(woken);
    }
    if !matches!(command, FUTEX_WAIT | FUTEX_WAIT_BITSET)
        || command == FUTEX_WAIT_BITSET && (bitset == 0 || bitset > u64::from(u32::MAX))
    {
        return LINUX_EINVAL;
    }
    if u64::from(u32::from_ne_bytes(bytes)) != value {
        return LINUX_EAGAIN;
    }
    let Some(current) = current_linux_thread_handle() else {
        return LINUX_ESRCH;
    };
    let target = ready_linux_thread_handle();
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
        let Some(requested) = linux_timespec_nanoseconds(&timeout_bytes) else {
            return LINUX_EINVAL;
        };
        if command == FUTEX_WAIT {
            if requested == 0 {
                return LINUX_ETIMEDOUT;
            }
            let Some(deadline) =
                linux_elapsed_nanoseconds().and_then(|now| now.checked_add(requested))
            else {
                return LINUX_EINVAL;
            };
            deadline
        } else {
            let Some(monotonic_now) = linux_elapsed_nanoseconds() else {
                return LINUX_ENOSYS;
            };
            if flags & FUTEX_CLOCK_REALTIME == 0 {
                if monotonic_now >= requested {
                    return LINUX_ETIMEDOUT;
                }
                requested
            } else {
                let Some((seconds, nanoseconds)) = linux_realtime_time() else {
                    return LINUX_ENOSYS;
                };
                let Some(realtime_now) = seconds
                    .checked_mul(NANOSECONDS_PER_SECOND)
                    .and_then(|seconds| seconds.checked_add(nanoseconds))
                else {
                    return LINUX_EINVAL;
                };
                if realtime_now >= requested {
                    return LINUX_ETIMEDOUT;
                }
                monotonic_now.saturating_add(requested - realtime_now)
            }
        }
    };
    let wait_mask = if command == FUTEX_WAIT_BITSET {
        bitset
    } else {
        u64::from(u32::MAX)
    };
    if target.is_none() && current_linux_runtime_cpu_index() == 0 {
        if crate::arch::x86_64::smp::linux_runtime_thread_slot().is_some() {
            return 0;
        }
        return LINUX_ENOSYS;
    }
    if command == FUTEX_WAIT_BITSET
        && current_linux_runtime_cpu_index() == 0
        && crate::arch::x86_64::smp::linux_runtime_thread_slot().is_some()
    {
        // A remote application CPU can satisfy this wait concurrently. Linux
        // permits a futex wait to return spuriously, so keep the coordinator's
        // live syscall frame intact and let userspace recheck its predicate.
        // Scheduler-backed switching from this coordinator frame is reserved
        // for waits whose owner can be suspended and restored exactly.
        return 0;
    }
    let coordination = LINUX_FUTEX_COORDINATION.lock();
    if !copy_from_linux_user(address, &mut bytes) {
        return LINUX_EFAULT;
    }
    if u64::from(u32::from_ne_bytes(bytes)) != value {
        return LINUX_EAGAIN;
    }
    if !save_linux_thread_return_context(current, caller)
        || !block_current_linux_thread(LINUX_THREAD_WAIT_FUTEX, address, wait_mask, deadline)
    {
        return LINUX_EINVAL;
    }
    drop(coordination);
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
    let coordination = LINUX_FUTEX_COORDINATION.lock();
    if !copy_from_linux_user(address, &mut bytes) {
        return LINUX_EFAULT;
    }
    if u64::from(u32::from_ne_bytes(bytes)) != value {
        return LINUX_EAGAIN;
    }
    if !save_linux_thread_return_context(current, caller)
        || !block_current_linux_thread(LINUX_THREAD_WAIT_FUTEX, address, mask, deadline)
    {
        return LINUX_EINVAL;
    }
    drop(coordination);
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
