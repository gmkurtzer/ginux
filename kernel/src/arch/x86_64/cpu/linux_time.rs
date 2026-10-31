use super::*;

pub(super) fn linux_clock_gettime(clock: u64, output_address: u64) -> u64 {
    let clock = u64::from(clock as u32);
    if !linux_clock_supported(clock) {
        return LINUX_EINVAL;
    }
    let realtime = matches!(
        clock,
        LINUX_CLOCK_REALTIME
            | LINUX_CLOCK_REALTIME_COARSE
            | LINUX_CLOCK_REALTIME_ALARM
            | LINUX_CLOCK_TAI
    );
    let Some((seconds, mut nanoseconds)) = (if realtime {
        linux_realtime_time()
    } else {
        linux_elapsed_time()
    }) else {
        return LINUX_ENOSYS;
    };
    if matches!(
        clock,
        LINUX_CLOCK_REALTIME_COARSE | LINUX_CLOCK_MONOTONIC_COARSE
    ) {
        let resolution = NANOSECONDS_PER_SECOND / LINUX_CLOCK_TICKS_PER_SECOND;
        nanoseconds -= nanoseconds % resolution;
    }
    let mut timespec = [0_u8; 16];
    timespec[..8].copy_from_slice(&seconds.to_ne_bytes());
    timespec[8..].copy_from_slice(&nanoseconds.to_ne_bytes());
    if !address_space::copy_to_current_linux_user_fast(output_address, &timespec) {
        return LINUX_EFAULT;
    }
    0
}

pub(super) fn linux_gettimeofday(timeval_address: u64, timezone_address: u64) -> u64 {
    if timeval_address == 0 && timezone_address == 0 {
        return 0;
    }
    let Some((seconds, nanoseconds)) = linux_realtime_time() else {
        return LINUX_ENOSYS;
    };
    if timeval_address != 0
        && (!copy_to_linux_user(timeval_address, &seconds.to_ne_bytes())
            || !copy_to_linux_user(
                timeval_address.saturating_add(8),
                &(nanoseconds / 1_000).to_ne_bytes(),
            ))
    {
        return LINUX_EFAULT;
    }
    if timezone_address != 0 && !copy_to_linux_user(timezone_address, &[0; 8]) {
        return LINUX_EFAULT;
    }
    0
}

pub(super) fn linux_time(output_address: u64) -> u64 {
    if output_address != 0 && !linux_writable_user_range_contains(output_address, 8) {
        return LINUX_EFAULT;
    }
    let Some((seconds, _)) = linux_realtime_time() else {
        return LINUX_ENOSYS;
    };
    if output_address != 0 && !copy_to_linux_user(output_address, &seconds.to_ne_bytes()) {
        return LINUX_EFAULT;
    }
    seconds
}

pub(super) fn linux_uname_with_copy(address: u64, copy: impl FnOnce(u64, &[u8]) -> bool) -> u64 {
    let mut name = [0_u8; LINUX_UTSNAME_SIZE];
    name[..6].copy_from_slice(b"Ginux\0");
    name[65..71].copy_from_slice(b"ginux\0");
    let release = if X86_RUNTIME.ELF_PROBE_PERSONALITY.load(Ordering::SeqCst)
        & LINUX_PERSONALITY_UNAME26
        != 0
    {
        b"2.6.52.0-ginux.0.1\0".as_slice()
    } else {
        b"6.12.0-ginux.0.1\0".as_slice()
    };
    name[130..130 + release.len()].copy_from_slice(release);
    name[195..198].copy_from_slice(b"#1\0");
    name[260..267].copy_from_slice(b"x86_64\0");
    name[325..337].copy_from_slice(b"localdomain\0");
    if copy(address, &name) {
        0
    } else {
        LINUX_EFAULT
    }
}

pub(super) fn linux_elapsed_time() -> Option<(u64, u64)> {
    let frequency = X86_RUNTIME.ELF_PROBE_CLOCK_FREQUENCY.load(Ordering::SeqCst);
    let epoch = X86_RUNTIME.ELF_PROBE_CLOCK_EPOCH.load(Ordering::SeqCst);
    if frequency == 0 || epoch == 0 {
        return None;
    }
    let ticks = timer::timestamp_counter().saturating_sub(epoch);
    linux_ticks_to_time(ticks, frequency)
}

pub(crate) fn refresh_linux_vvar() -> bool {
    if !X86_RUNTIME.LINUX_RUNTIME_MODE.load(Ordering::Acquire)
        || !X86_RUNTIME.ELF_PROBE_ACTIVE.load(Ordering::Acquire)
    {
        return true;
    }
    let frequency = X86_RUNTIME
        .ELF_PROBE_CLOCK_FREQUENCY
        .load(Ordering::Relaxed);
    let monotonic_epoch = X86_RUNTIME.ELF_PROBE_CLOCK_EPOCH.load(Ordering::Relaxed);
    let now = timer::timestamp_counter();
    let multiplier = (NANOSECONDS_PER_SECOND << 32).checked_div(frequency);
    let monotonic = now
        .checked_sub(monotonic_epoch)
        .and_then(|ticks| linux_ticks_to_time(ticks, frequency));
    let realtime = crate::linux_realtime_epoch().and_then(|(seconds, epoch)| {
        now.checked_sub(epoch)
            .and_then(|ticks| linux_ticks_to_time(ticks, frequency))
            .and_then(|(elapsed, nanoseconds)| {
                seconds
                    .checked_add(elapsed)
                    .map(|seconds| (seconds, nanoseconds))
            })
    });
    let frame = (0..usize::from(
        X86_RUNTIME
            .ELF_PROBE_ACTIVE_IMAGE_COUNT
            .load(Ordering::Acquire),
    ))
        .find(|index| {
            X86_RUNTIME.ELF_PROBE_ACTIVE_IMAGE_ADDRESSES[*index].load(Ordering::Acquire)
                == crate::linux_elf::VVAR_ADDRESS
        })
        .and_then(|index| {
            memory::FrameLease::from_parts(
                X86_RUNTIME.ELF_PROBE_ACTIVE_IMAGE_FRAMES[index].load(Ordering::Acquire),
                X86_RUNTIME.ELF_PROBE_ACTIVE_IMAGE_GENERATIONS[index].load(Ordering::Acquire),
            )
        });
    multiplier
        .zip(monotonic)
        .zip(realtime)
        .zip(frame)
        .is_some_and(
            |(
                (
                    (multiplier, (monotonic_seconds, monotonic_nanoseconds)),
                    (realtime_seconds, realtime_nanoseconds),
                ),
                frame,
            )| {
                address_space::update_linux_vvar(
                    frame,
                    [
                        multiplier,
                        now,
                        monotonic_seconds,
                        monotonic_nanoseconds,
                        realtime_seconds,
                        realtime_nanoseconds,
                    ],
                )
            },
        )
}

pub(super) fn linux_ticks_to_time(ticks: u64, frequency: u64) -> Option<(u64, u64)> {
    if frequency == 0 {
        return None;
    }
    let seconds = ticks / frequency;
    let remainder = ticks % frequency;
    let nanoseconds = if frequency == NANOSECONDS_PER_SECOND {
        remainder
    } else {
        remainder
            .checked_mul(NANOSECONDS_PER_SECOND)?
            .checked_div(frequency)?
    };
    Some((seconds, nanoseconds))
}

pub(super) fn linux_realtime_time() -> Option<(u64, u64)> {
    let frequency = X86_RUNTIME.ELF_PROBE_CLOCK_FREQUENCY.load(Ordering::SeqCst);
    let (epoch_seconds, epoch_counter) = crate::linux_realtime_epoch()?;
    let ticks = timer::timestamp_counter().checked_sub(epoch_counter)?;
    let (elapsed_seconds, nanoseconds) = linux_ticks_to_time(ticks, frequency)?;
    Some((epoch_seconds.checked_add(elapsed_seconds)?, nanoseconds))
}

pub(super) fn linux_host_realtime_time() -> Option<(u64, u64)> {
    let frequency = timer::monotonic_frequency()?;
    let (epoch_seconds, epoch_counter) = crate::linux_realtime_epoch()?;
    let ticks = timer::timestamp_counter().checked_sub(epoch_counter)?;
    let elapsed_seconds = ticks.checked_div(frequency)?;
    let remainder = ticks.checked_rem(frequency)?;
    let nanoseconds = remainder
        .checked_mul(NANOSECONDS_PER_SECOND)?
        .checked_div(frequency)?;
    Some((epoch_seconds.checked_add(elapsed_seconds)?, nanoseconds))
}

pub(super) fn linux_elapsed_nanoseconds() -> Option<u64> {
    let (seconds, nanoseconds) = linux_elapsed_time()?;
    seconds
        .checked_mul(NANOSECONDS_PER_SECOND)
        .and_then(|value| value.checked_add(nanoseconds))
}

pub(super) fn linux_system_elapsed_time() -> Option<(u64, u64)> {
    let frequency = timer::monotonic_frequency()?;
    let (_, epoch) = crate::linux_realtime_epoch()?;
    let elapsed = timer::timestamp_counter().saturating_sub(epoch);
    Some((
        elapsed / frequency,
        elapsed % frequency * NANOSECONDS_PER_SECOND / frequency,
    ))
}

pub(super) fn linux_clock_supported(clock: u64) -> bool {
    let clock = u64::from(clock as u32);
    matches!(
        clock,
        LINUX_CLOCK_REALTIME
            | LINUX_CLOCK_MONOTONIC
            | LINUX_CLOCK_PROCESS_CPUTIME_ID
            | LINUX_CLOCK_THREAD_CPUTIME_ID
            | LINUX_CLOCK_MONOTONIC_RAW
            | LINUX_CLOCK_REALTIME_COARSE
            | LINUX_CLOCK_MONOTONIC_COARSE
            | LINUX_CLOCK_BOOTTIME
            | LINUX_CLOCK_REALTIME_ALARM
            | LINUX_CLOCK_BOOTTIME_ALARM
            | LINUX_CLOCK_TAI
    )
}

pub(super) fn linux_times(output_address: u64) -> u64 {
    let frequency = X86_RUNTIME.ELF_PROBE_CLOCK_FREQUENCY.load(Ordering::SeqCst);
    let epoch = X86_RUNTIME.ELF_PROBE_CLOCK_EPOCH.load(Ordering::SeqCst);
    if frequency == 0 || epoch == 0 {
        return LINUX_ENOSYS;
    }
    let elapsed = timer::timestamp_counter().saturating_sub(epoch);
    let ticks = elapsed / frequency * LINUX_CLOCK_TICKS_PER_SECOND
        + elapsed % frequency * LINUX_CLOCK_TICKS_PER_SECOND / frequency;
    if output_address != 0 {
        let mut accounting = [0_u8; 32];
        accounting[..8].copy_from_slice(&ticks.to_ne_bytes());
        if !copy_to_linux_user(output_address, &accounting) {
            return LINUX_EFAULT;
        }
    }
    ticks
}

pub(super) fn linux_sysinfo(output_address: u64) -> u64 {
    let Some(processes) = linux_process::live_process_count() else {
        return LINUX_ESRCH;
    };
    let Some(uptime) = linux_elapsed_time() else {
        return LINUX_ENOSYS;
    };
    let information = match linux_sysinfo_information(processes, uptime) {
        Ok(information) => information,
        Err(error) => return error,
    };
    if copy_to_linux_user(output_address, &information) {
        0
    } else {
        LINUX_EFAULT
    }
}

pub(super) fn linux_sysinfo_information(
    processes: u16,
    (uptime, _): (u64, u64),
) -> Result<[u8; 112], u64> {
    let Some((total_frames, free_frames)) = memory::frame_counts() else {
        return Err(LINUX_ENOSYS);
    };
    let Some(total_memory) = total_frames.checked_mul(PAGE_SIZE as u64) else {
        return Err(LINUX_EOVERFLOW);
    };
    let Some(free_memory) = free_frames.checked_mul(PAGE_SIZE as u64) else {
        return Err(LINUX_EOVERFLOW);
    };
    let mut information = [0_u8; 112];
    information[..8].copy_from_slice(&uptime.to_ne_bytes());
    information[32..40].copy_from_slice(&total_memory.to_ne_bytes());
    information[40..48].copy_from_slice(&free_memory.to_ne_bytes());
    information[80..82].copy_from_slice(&processes.to_ne_bytes());
    information[104..108].copy_from_slice(&1_u32.to_ne_bytes());
    Ok(information)
}

pub(super) fn linux_getrusage(who: u64, output_address: u64) -> u64 {
    let who = who as i32;
    let children = who == -1;
    if !matches!(who, -1..=1) {
        return LINUX_EINVAL;
    }
    let mut usage = [0_u8; 144];
    if !children {
        let Some((seconds, nanoseconds)) = linux_elapsed_time() else {
            return LINUX_ENOSYS;
        };
        usage[..8].copy_from_slice(&seconds.to_ne_bytes());
        usage[8..16].copy_from_slice(&(nanoseconds / 1_000).to_ne_bytes());
        let resident_kib = address_space::current_linux_user_page_count()
            .and_then(|pages| pages.checked_mul(PAGE_SIZE / 1024))
            .and_then(|kib| u64::try_from(kib).ok())
            .unwrap_or(0);
        usage[32..40].copy_from_slice(&resident_kib.to_ne_bytes());
    }
    if !copy_to_linux_user(output_address, &usage) {
        return LINUX_EFAULT;
    }
    0
}

pub(super) fn linux_getitimer(which: u64, output_address: u64) -> u64 {
    let which = u64::from(which as u32);
    if which > 2 {
        return LINUX_EINVAL;
    }
    let specification = if which == 0 {
        let Some(now) = linux_elapsed_nanoseconds() else {
            return LINUX_ENOSYS;
        };
        linux_interval_timer_specification(
            X86_RUNTIME
                .ELF_PROBE_REAL_TIMER_INTERVAL
                .load(Ordering::SeqCst),
            X86_RUNTIME
                .ELF_PROBE_REAL_TIMER_DEADLINE
                .load(Ordering::SeqCst),
            now,
        )
    } else {
        [0_u8; 32]
    };
    if !copy_to_linux_user(output_address, &specification) {
        return LINUX_EFAULT;
    }
    0
}

pub(super) fn linux_alarm(seconds: u64) -> u64 {
    let seconds = u64::from(seconds as u32);
    let Some(now) = linux_elapsed_nanoseconds() else {
        return 0;
    };
    let old_deadline = X86_RUNTIME
        .ELF_PROBE_REAL_TIMER_DEADLINE
        .load(Ordering::SeqCst);
    let remaining = old_deadline
        .saturating_sub(now)
        .div_ceil(NANOSECONDS_PER_SECOND);
    let deadline = if seconds == 0 {
        0
    } else {
        now.saturating_add(seconds.saturating_mul(NANOSECONDS_PER_SECOND))
    };
    X86_RUNTIME
        .ELF_PROBE_REAL_TIMER_INTERVAL
        .store(0, Ordering::SeqCst);
    X86_RUNTIME
        .ELF_PROBE_REAL_TIMER_DEADLINE
        .store(deadline, Ordering::SeqCst);
    remaining.min(u64::from(u32::MAX))
}

pub(super) fn refresh_linux_real_timer() -> bool {
    let deadline = X86_RUNTIME
        .ELF_PROBE_REAL_TIMER_DEADLINE
        .load(Ordering::SeqCst);
    if deadline == 0 {
        return false;
    }
    let Some(now) = linux_elapsed_nanoseconds() else {
        return false;
    };
    if now < deadline {
        return false;
    }
    let interval = X86_RUNTIME
        .ELF_PROBE_REAL_TIMER_INTERVAL
        .load(Ordering::SeqCst);
    let next = now
        .saturating_sub(deadline)
        .checked_div(interval)
        .map_or(0, |elapsed| {
            deadline.saturating_add((elapsed + 1).saturating_mul(interval))
        });
    X86_RUNTIME
        .ELF_PROBE_REAL_TIMER_DEADLINE
        .store(next, Ordering::SeqCst);
    let index = LINUX_SIGALRM as usize - 1;
    X86_RUNTIME.ELF_PROBE_PENDING_SIGNAL_CODES[index]
        .store(LINUX_SI_KERNEL as u32, Ordering::SeqCst);
    X86_RUNTIME.ELF_PROBE_PENDING_SIGNAL_VALUES[index].store(0, Ordering::SeqCst);
    X86_RUNTIME
        .ELF_PROBE_PENDING_SIGNALS
        .fetch_or(1 << index, Ordering::SeqCst);
    true
}

pub(super) fn linux_nanosleep_live_return(
    output: *mut LinuxSyscallFrame,
    caller: LinuxUserContext,
    request: u64,
    remaining: u64,
) -> u64 {
    linux_clock_nanosleep_live_return(output, caller, LINUX_CLOCK_MONOTONIC, 0, request, remaining)
}

pub(super) fn linux_clock_nanosleep_live_return(
    output: *mut LinuxSyscallFrame,
    caller: LinuxUserContext,
    clock: u64,
    flags: u64,
    request: u64,
    remaining: u64,
) -> u64 {
    let clock = u64::from(clock as u32);
    let flags = u64::from(flags as u32);
    if !matches!(clock, LINUX_CLOCK_REALTIME | LINUX_CLOCK_MONOTONIC)
        || flags & !LINUX_TIMER_ABSTIME != 0
    {
        return LINUX_EINVAL;
    }
    let mut request_bytes = [0_u8; 16];
    if !copy_from_linux_user(request, &mut request_bytes) {
        return LINUX_EFAULT;
    }
    let Some(requested) = linux_timespec_nanoseconds(&request_bytes) else {
        return LINUX_EINVAL;
    };
    let Some(monotonic_now) = linux_elapsed_nanoseconds() else {
        return LINUX_ENOSYS;
    };
    let realtime_now = if clock == LINUX_CLOCK_REALTIME {
        let Some((seconds, nanoseconds)) = linux_realtime_time() else {
            return LINUX_ENOSYS;
        };
        let Some(now) = seconds
            .checked_mul(1_000_000_000)
            .and_then(|seconds| seconds.checked_add(nanoseconds))
        else {
            return LINUX_EINVAL;
        };
        now
    } else {
        monotonic_now
    };
    let absolute = flags & LINUX_TIMER_ABSTIME != 0;
    let deadline = if absolute && clock == LINUX_CLOCK_REALTIME {
        monotonic_now.saturating_add(requested.saturating_sub(realtime_now))
    } else if absolute {
        requested
    } else {
        let Some(deadline) = monotonic_now.checked_add(requested) else {
            return LINUX_EINVAL;
        };
        deadline
    };
    if deadline <= monotonic_now {
        return 0;
    }
    let remaining = if absolute { 0 } else { remaining };
    if remaining != 0 && !linux_writable_user_range_contains(remaining, 16) {
        return LINUX_EFAULT;
    }
    let Some(current) = current_linux_thread_handle() else {
        return LINUX_ESRCH;
    };
    let Some(target) = ready_linux_thread_handle() else {
        // With no runnable peer, the current process is the domain's idle
        // task. Sleep in the kernel until the deadline instead of exposing an
        // implementation detail as ENOSYS. Device/timer interrupts still wake
        // the CPU and are handled normally between checks.
        while linux_elapsed_nanoseconds().is_some_and(|now| now < deadline) {
            unsafe { asm!("sti", "hlt", "cli", options(nomem, nostack)) };
        }
        return 0;
    };
    let Some(tls) = linux_thread_context(current) else {
        return LINUX_ESRCH;
    };
    if !save_linux_thread_return_context(current, caller)
        || !block_current_linux_thread(LINUX_THREAD_WAIT_SLEEP, 1, remaining, deadline)
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
        .LINUX_SLEEP_BLOCK_COUNT
        .fetch_add(1, Ordering::SeqCst);
    LINUX_RETURN_FROM_IRET
}

pub(super) fn linux_setitimer(which: u64, input_address: u64, old_output_address: u64) -> u64 {
    let which = u64::from(which as u32);
    if which > 2 {
        return LINUX_EINVAL;
    }
    if old_output_address != 0 && !linux_writable_user_range_contains(old_output_address, 32) {
        return LINUX_EFAULT;
    }
    let mut requested = [0_u8; 32];
    if input_address != 0 && !copy_from_linux_user(input_address, &mut requested) {
        return LINUX_EFAULT;
    }
    let Some(interval) = linux_timeval_nanoseconds(&requested[..16]) else {
        return LINUX_EINVAL;
    };
    let Some(value) = linux_timeval_nanoseconds(&requested[16..]) else {
        return LINUX_EINVAL;
    };
    if which != 0 && (interval != 0 || value != 0) {
        return LINUX_ENOSYS;
    }
    let Some(now) = linux_elapsed_nanoseconds() else {
        return LINUX_ENOSYS;
    };
    let old = if which == 0 {
        linux_interval_timer_specification(
            X86_RUNTIME
                .ELF_PROBE_REAL_TIMER_INTERVAL
                .load(Ordering::SeqCst),
            X86_RUNTIME
                .ELF_PROBE_REAL_TIMER_DEADLINE
                .load(Ordering::SeqCst),
            now,
        )
    } else {
        [0_u8; 32]
    };
    let deadline = if value == 0 {
        0
    } else {
        let Some(deadline) = now.checked_add(value) else {
            return LINUX_EINVAL;
        };
        deadline
    };
    if old_output_address != 0 && !copy_to_linux_user(old_output_address, &old) {
        return LINUX_EFAULT;
    }
    if which == 0 {
        X86_RUNTIME
            .ELF_PROBE_REAL_TIMER_INTERVAL
            .store(interval, Ordering::SeqCst);
        X86_RUNTIME
            .ELF_PROBE_REAL_TIMER_DEADLINE
            .store(deadline, Ordering::SeqCst);
    }
    0
}

pub(super) fn linux_timeval_nanoseconds(value: &[u8]) -> Option<u64> {
    let seconds = i64::from_ne_bytes(value.get(..8)?.try_into().ok()?);
    let microseconds = i64::from_ne_bytes(value.get(8..16)?.try_into().ok()?);
    if seconds < 0 || !(0..1_000_000).contains(&microseconds) {
        return None;
    }
    (seconds as u64)
        .checked_mul(NANOSECONDS_PER_SECOND)
        .and_then(|seconds| seconds.checked_add((microseconds as u64) * 1_000))
}

pub(super) fn linux_interval_timer_specification(
    interval: u64,
    deadline: u64,
    now: u64,
) -> [u8; 32] {
    let remaining = deadline.saturating_sub(now);
    let mut specification = [0_u8; 32];
    for (offset, value) in [(0, interval), (16, remaining)] {
        specification[offset..offset + 8]
            .copy_from_slice(&(value / NANOSECONDS_PER_SECOND).to_ne_bytes());
        specification[offset + 8..offset + 16]
            .copy_from_slice(&((value % NANOSECONDS_PER_SECOND) / 1_000).to_ne_bytes());
    }
    specification
}
