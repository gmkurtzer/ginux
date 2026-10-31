use super::*;

pub(super) fn wake_linux_poll_waits() -> u32 {
    let mut woken = 0;
    for slot in 0..LINUX_THREAD_LIMIT {
        woken += u32::from(complete_linux_poll_wait(slot, false));
    }
    woken
}

#[derive(Clone, Copy)]
pub(super) struct LinuxSelectInput {
    pub(super) descriptor_count: u64,
    pub(super) read_address: u64,
    pub(super) write_address: u64,
    pub(super) exception_address: u64,
    pub(super) timeout_address: u64,
    pub(super) signal_mask_address: u64,
}

#[derive(Clone, Copy)]
pub(super) struct LinuxSelectSnapshot {
    pub(super) input: LinuxSelectInput,
    pub(super) requested_read: [u64; LINUX_SELECT_WORD_LIMIT],
    pub(super) requested_write: [u64; LINUX_SELECT_WORD_LIMIT],
    pub(super) requested_exception: [u64; LINUX_SELECT_WORD_LIMIT],
}

pub(super) struct LinuxParsedSelect {
    pub(super) snapshot: LinuxSelectSnapshot,
    pub(super) duration: Option<u64>,
    pub(super) temporary_signal_mask: Option<u64>,
}

pub(super) fn linux_select_parse(
    mut input: LinuxSelectInput,
    pselect: bool,
    copy_from: fn(u64, &mut [u8]) -> bool,
) -> Result<LinuxParsedSelect, u64> {
    let descriptor_count = input.descriptor_count as i32;
    if descriptor_count < 0 {
        return Err(LINUX_EINVAL);
    }
    input.descriptor_count = descriptor_count as u64;
    if input.descriptor_count > LINUX_AUX_DESCRIPTOR_LIMIT {
        return Err(LINUX_EINVAL);
    }
    let duration = if input.timeout_address == 0 {
        None
    } else {
        let mut timeout = [0_u8; 16];
        if !copy_from(input.timeout_address, &mut timeout) {
            return Err(LINUX_EFAULT);
        }
        let seconds = i64::from_ne_bytes(timeout[..8].try_into().unwrap());
        let fraction = i64::from_ne_bytes(timeout[8..].try_into().unwrap());
        let (fraction_limit, scale) = if pselect {
            (NANOSECONDS_PER_SECOND as i64, 1)
        } else {
            (MICROSECONDS_PER_SECOND as i64, 1_000)
        };
        if seconds < 0 || !(0..fraction_limit).contains(&fraction) {
            return Err(LINUX_EINVAL);
        }
        let Some(duration) = (seconds as u64)
            .checked_mul(NANOSECONDS_PER_SECOND)
            .and_then(|seconds| seconds.checked_add((fraction as u64) * scale))
        else {
            return Err(LINUX_EINVAL);
        };
        Some(duration)
    };
    let temporary_signal_mask = if pselect && input.signal_mask_address != 0 {
        let mut argument = [0_u8; 16];
        if !copy_from(input.signal_mask_address, &mut argument) {
            return Err(LINUX_EFAULT);
        }
        let mask_address = u64::from_ne_bytes(argument[..8].try_into().unwrap());
        let mask_size = u64::from_ne_bytes(argument[8..].try_into().unwrap());
        if mask_size != LINUX_SIGNAL_SET_SIZE {
            return Err(LINUX_EINVAL);
        }
        if mask_address == 0 {
            None
        } else {
            let mut mask = [0_u8; LINUX_SIGNAL_SET_SIZE as usize];
            if !copy_from(mask_address, &mut mask) {
                return Err(LINUX_EFAULT);
            }
            Some(u64::from_ne_bytes(mask) & !LINUX_UNBLOCKABLE_SIGNALS)
        }
    } else {
        None
    };
    let word_count = input.descriptor_count.div_ceil(u64::BITS as u64) as usize;
    let byte_count = word_count * size_of::<u64>();
    let mut requested = [[0_u64; LINUX_SELECT_WORD_LIMIT]; 3];
    for (address, output) in [
        input.read_address,
        input.write_address,
        input.exception_address,
    ]
    .into_iter()
    .zip(requested.iter_mut())
    {
        if address != 0 && byte_count != 0 {
            let mut bytes = [0_u8; LINUX_SELECT_WORD_LIMIT * size_of::<u64>()];
            if !copy_from(address, &mut bytes[..byte_count]) {
                return Err(LINUX_EFAULT);
            }
            for (word, bytes) in output[..word_count]
                .iter_mut()
                .zip(bytes[..byte_count].as_chunks::<8>().0)
            {
                *word = u64::from_ne_bytes(*bytes);
            }
        }
    }
    if word_count != 0 && !input.descriptor_count.is_multiple_of(u64::BITS as u64) {
        let valid_mask = (1_u64 << (input.descriptor_count % u64::BITS as u64)) - 1;
        for set in &mut requested {
            set[word_count - 1] &= valid_mask;
        }
    }
    Ok(LinuxParsedSelect {
        snapshot: LinuxSelectSnapshot {
            input,
            requested_read: requested[0],
            requested_write: requested[1],
            requested_exception: requested[2],
        },
        duration,
        temporary_signal_mask,
    })
}

pub(super) fn linux_select_ready(
    snapshot: LinuxSelectSnapshot,
) -> Result<
    (
        [u64; LINUX_SELECT_WORD_LIMIT],
        [u64; LINUX_SELECT_WORD_LIMIT],
        u64,
    ),
    u64,
> {
    let mut ready_read = [0_u64; LINUX_SELECT_WORD_LIMIT];
    let mut ready_write = [0_u64; LINUX_SELECT_WORD_LIMIT];
    let mut ready_count = 0_u64;
    for descriptor in 0..snapshot.input.descriptor_count {
        let word = descriptor as usize / u64::BITS as usize;
        let bit = 1_u64 << (descriptor % u64::BITS as u64);
        if (snapshot.requested_read[word]
            | snapshot.requested_write[word]
            | snapshot.requested_exception[word])
            & bit
            != 0
            && !linux_descriptor_open(descriptor)
        {
            return Err(LINUX_EBADF);
        }
        let mut descriptor_ready = false;
        if snapshot.requested_read[word] & bit != 0
            && linux_descriptor_poll_events(descriptor as i32, LINUX_POLLIN) & LINUX_POLLIN != 0
        {
            ready_read[word] |= bit;
            descriptor_ready = true;
        }
        if snapshot.requested_write[word] & bit != 0
            && linux_descriptor_poll_events(descriptor as i32, LINUX_POLLOUT) & LINUX_POLLOUT != 0
        {
            ready_write[word] |= bit;
            descriptor_ready = true;
        }
        if descriptor_ready {
            ready_count += 1;
        }
    }
    Ok((ready_read, ready_write, ready_count))
}

pub(super) fn linux_select_waits_for_attached_console(snapshot: LinuxSelectSnapshot) -> bool {
    let Some(slot) = active_container_slot() else {
        return false;
    };
    if !X86_RUNTIME.CONTAINER_CONSOLE_ATTACHED[slot].load(Ordering::Acquire) {
        return false;
    }
    (0..snapshot.input.descriptor_count).any(|descriptor| {
        let word = descriptor as usize / u64::BITS as usize;
        let bit = 1_u64 << (descriptor % u64::BITS as u64);
        snapshot.requested_read[word] & bit != 0
            && (u16::from(linux_console_descriptor(descriptor)) & LINUX_DESCRIPTOR_INPUT != 0
                || linux_device_descriptor(descriptor) & LINUX_DEVICE_CONSOLE != 0)
    })
}

pub(super) fn linux_select_copy_result(
    snapshot: LinuxSelectSnapshot,
    ready_read: [u64; LINUX_SELECT_WORD_LIMIT],
    ready_write: [u64; LINUX_SELECT_WORD_LIMIT],
    copy_to: fn(u64, &[u8]) -> bool,
) -> u64 {
    let word_count = snapshot.input.descriptor_count.div_ceil(u64::BITS as u64) as usize;
    let byte_count = word_count * size_of::<u64>();
    let empty = [0_u64; LINUX_SELECT_WORD_LIMIT];
    for (address, values) in [
        (snapshot.input.read_address, ready_read),
        (snapshot.input.write_address, ready_write),
        (snapshot.input.exception_address, empty),
    ] {
        if address != 0 && byte_count != 0 {
            let mut bytes = [0_u8; LINUX_SELECT_WORD_LIMIT * size_of::<u64>()];
            for (value, bytes) in values[..word_count]
                .iter()
                .zip(bytes[..byte_count].as_chunks_mut::<8>().0)
            {
                bytes.copy_from_slice(&value.to_ne_bytes());
            }
            if !copy_to(address, &bytes[..byte_count]) {
                return LINUX_EFAULT;
            }
        }
    }
    (0..snapshot.input.descriptor_count)
        .filter(|descriptor| {
            let word = *descriptor as usize / u64::BITS as usize;
            let bit = 1_u64 << (*descriptor % u64::BITS as u64);
            (ready_read[word] | ready_write[word]) & bit != 0
        })
        .count() as u64
}

pub(super) fn linux_select_with_copy(
    input: LinuxSelectInput,
    pselect: bool,
    copy_from: fn(u64, &mut [u8]) -> bool,
    copy_to: fn(u64, &[u8]) -> bool,
) -> u64 {
    let parsed = match linux_select_parse(input, pselect, copy_from) {
        Ok(parsed) => parsed,
        Err(error) => return error,
    };
    let (ready_read, ready_write, _) = match linux_select_ready(parsed.snapshot) {
        Ok(ready) => ready,
        Err(error) => return error,
    };
    linux_select_copy_result(parsed.snapshot, ready_read, ready_write, copy_to)
}

pub(super) fn store_linux_select_snapshot(slot: usize, snapshot: LinuxSelectSnapshot) {
    X86_RUNTIME.LINUX_THREAD_SELECT_READ_ADDRESSES[slot]
        .store(snapshot.input.read_address, Ordering::SeqCst);
    X86_RUNTIME.LINUX_THREAD_SELECT_WRITE_ADDRESSES[slot]
        .store(snapshot.input.write_address, Ordering::SeqCst);
    X86_RUNTIME.LINUX_THREAD_SELECT_EXCEPTION_ADDRESSES[slot]
        .store(snapshot.input.exception_address, Ordering::SeqCst);
    X86_RUNTIME.LINUX_THREAD_SELECT_TIMEOUT_ADDRESSES[slot]
        .store(snapshot.input.timeout_address, Ordering::SeqCst);
    for index in 0..LINUX_SELECT_WORD_LIMIT {
        X86_RUNTIME.LINUX_THREAD_SELECT_READ_SETS[slot][index]
            .store(snapshot.requested_read[index], Ordering::SeqCst);
        X86_RUNTIME.LINUX_THREAD_SELECT_WRITE_SETS[slot][index]
            .store(snapshot.requested_write[index], Ordering::SeqCst);
        X86_RUNTIME.LINUX_THREAD_SELECT_EXCEPTION_SETS[slot][index]
            .store(snapshot.requested_exception[index], Ordering::SeqCst);
    }
}

pub(super) fn load_linux_select_snapshot(
    slot: usize,
    descriptor_count: u64,
) -> LinuxSelectSnapshot {
    LinuxSelectSnapshot {
        input: LinuxSelectInput {
            descriptor_count,
            read_address: X86_RUNTIME.LINUX_THREAD_SELECT_READ_ADDRESSES[slot]
                .load(Ordering::SeqCst),
            write_address: X86_RUNTIME.LINUX_THREAD_SELECT_WRITE_ADDRESSES[slot]
                .load(Ordering::SeqCst),
            exception_address: X86_RUNTIME.LINUX_THREAD_SELECT_EXCEPTION_ADDRESSES[slot]
                .load(Ordering::SeqCst),
            timeout_address: X86_RUNTIME.LINUX_THREAD_SELECT_TIMEOUT_ADDRESSES[slot]
                .load(Ordering::SeqCst),
            signal_mask_address: 0,
        },
        requested_read: core::array::from_fn(|index| {
            X86_RUNTIME.LINUX_THREAD_SELECT_READ_SETS[slot][index].load(Ordering::SeqCst)
        }),
        requested_write: core::array::from_fn(|index| {
            X86_RUNTIME.LINUX_THREAD_SELECT_WRITE_SETS[slot][index].load(Ordering::SeqCst)
        }),
        requested_exception: core::array::from_fn(|index| {
            X86_RUNTIME.LINUX_THREAD_SELECT_EXCEPTION_SETS[slot][index].load(Ordering::SeqCst)
        }),
    }
}

pub(super) fn linux_select_write_remaining(snapshot: LinuxSelectSnapshot, deadline: u64) -> bool {
    if snapshot.input.timeout_address == 0 {
        return true;
    }
    let Some(now) = linux_elapsed_nanoseconds() else {
        return false;
    };
    let remaining = deadline.saturating_sub(now);
    let mut timeval = [0_u8; 16];
    timeval[..8].copy_from_slice(&(remaining / NANOSECONDS_PER_SECOND).to_ne_bytes());
    timeval[8..].copy_from_slice(&(remaining % NANOSECONDS_PER_SECOND / 1_000).to_ne_bytes());
    copy_to_linux_user(snapshot.input.timeout_address, &timeval)
}

pub(super) fn complete_linux_select_wait(slot: usize, timed_out: bool) -> bool {
    let kind = if slot < LINUX_THREAD_LIMIT {
        X86_RUNTIME.LINUX_THREAD_WAIT_KINDS[slot].load(Ordering::SeqCst)
    } else {
        LINUX_THREAD_WAIT_NONE
    };
    if slot >= LINUX_THREAD_LIMIT
        || X86_RUNTIME.LINUX_THREAD_STATES[slot].load(Ordering::SeqCst) != LINUX_THREAD_BLOCKED
        || !matches!(kind, LINUX_THREAD_WAIT_SELECT | LINUX_THREAD_WAIT_PSELECT)
    {
        return false;
    }
    let descriptor_count = X86_RUNTIME.LINUX_THREAD_WAIT_OUTPUTS[slot].load(Ordering::SeqCst);
    if descriptor_count > LINUX_AUX_DESCRIPTOR_LIMIT {
        return false;
    }
    let snapshot = load_linux_select_snapshot(slot, descriptor_count);
    let readiness = linux_select_ready(snapshot);
    if readiness.is_ok_and(|(_, _, count)| count == 0) && !timed_out {
        return false;
    }
    let deadline = X86_RUNTIME.LINUX_THREAD_WAIT_DEADLINES[slot].load(Ordering::SeqCst);
    let result = match readiness {
        Ok((ready_read, ready_write, _)) => {
            let result = linux_select_copy_result(
                snapshot,
                if timed_out {
                    [0; LINUX_SELECT_WORD_LIMIT]
                } else {
                    ready_read
                },
                if timed_out {
                    [0; LINUX_SELECT_WORD_LIMIT]
                } else {
                    ready_write
                },
                copy_to_linux_user,
            );
            if result != LINUX_EFAULT
                && kind == LINUX_THREAD_WAIT_SELECT
                && !linux_select_write_remaining(snapshot, deadline)
            {
                LINUX_EFAULT
            } else {
                result
            }
        }
        Err(error) => error,
    };
    X86_RUNTIME.LINUX_THREAD_RAXS[slot].store(result, Ordering::SeqCst);
    X86_RUNTIME.LINUX_THREAD_WAIT_KINDS[slot].store(LINUX_THREAD_WAIT_NONE, Ordering::SeqCst);
    X86_RUNTIME.LINUX_THREAD_WAIT_KEYS[slot].store(0, Ordering::SeqCst);
    X86_RUNTIME.LINUX_THREAD_WAIT_OUTPUTS[slot].store(0, Ordering::SeqCst);
    X86_RUNTIME.LINUX_THREAD_WAIT_DEADLINES[slot].store(0, Ordering::SeqCst);
    if kind == LINUX_THREAD_WAIT_PSELECT {
        X86_RUNTIME.LINUX_THREAD_SIGNAL_MASKS[slot].store(
            X86_RUNTIME.LINUX_THREAD_WAIT_PRIOR_SIGNAL_MASKS[slot].swap(0, Ordering::SeqCst),
            Ordering::SeqCst,
        );
    }
    clear_linux_select_snapshot(slot);
    X86_RUNTIME.LINUX_THREAD_STATES[slot].store(LINUX_THREAD_READY, Ordering::SeqCst);
    true
}

pub(super) fn wake_linux_select_waits() -> u32 {
    let mut woken = 0;
    for slot in 0..LINUX_THREAD_LIMIT {
        woken += u32::from(complete_linux_select_wait(slot, false));
    }
    woken
}

pub(super) fn linux_select_live_return(
    output: *mut LinuxSyscallFrame,
    caller: LinuxUserContext,
    input: LinuxSelectInput,
    pselect: bool,
) -> u64 {
    let parsed = match linux_select_parse(input, pselect, copy_from_linux_user) {
        Ok(parsed) => parsed,
        Err(error) => return error,
    };
    let set_size = parsed
        .snapshot
        .input
        .descriptor_count
        .div_ceil(u64::BITS as u64) as usize
        * size_of::<u64>();
    for address in [
        parsed.snapshot.input.read_address,
        parsed.snapshot.input.write_address,
        parsed.snapshot.input.exception_address,
    ] {
        if address != 0 && set_size != 0 && !linux_writable_user_range_contains(address, set_size) {
            return LINUX_EFAULT;
        }
    }
    if !pselect
        && parsed.snapshot.input.timeout_address != 0
        && !linux_writable_user_range_contains(parsed.snapshot.input.timeout_address, 16)
    {
        return LINUX_EFAULT;
    }
    let (ready_read, ready_write, ready) = match linux_select_ready(parsed.snapshot) {
        Ok(ready) => ready,
        Err(error) => return error,
    };
    if ready != 0 || parsed.duration == Some(0) {
        let result =
            linux_select_copy_result(parsed.snapshot, ready_read, ready_write, copy_to_linux_user);
        if result == LINUX_EFAULT || pselect || parsed.snapshot.input.timeout_address == 0 {
            return result;
        }
        let duration = parsed.duration.unwrap_or(0);
        let Some(deadline) = linux_elapsed_nanoseconds().and_then(|now| now.checked_add(duration))
        else {
            return LINUX_EINVAL;
        };
        return if linux_select_write_remaining(parsed.snapshot, deadline) {
            result
        } else {
            LINUX_EFAULT
        };
    }
    // An attached domain console is serviced directly by the serial device.
    // There may be no second runnable Linux thread while an exec child owns
    // that console, so waiting through the Linux thread scheduler would return
    // ENOSYS. Wait on the device here and let the following read consume the
    // byte (including the controller detach escape).
    if linux_select_waits_for_attached_console(parsed.snapshot) {
        let deadline = parsed.duration.and_then(|duration| {
            linux_elapsed_nanoseconds().and_then(|now| now.checked_add(duration))
        });
        while !crate::serial_input_ready() {
            if deadline.is_some_and(|deadline| {
                linux_elapsed_nanoseconds().is_some_and(|now| now >= deadline)
            }) {
                return linux_select_copy_result(
                    parsed.snapshot,
                    [0; LINUX_SELECT_WORD_LIMIT],
                    [0; LINUX_SELECT_WORD_LIMIT],
                    copy_to_linux_user,
                );
            }
            core::hint::spin_loop();
        }
        let (ready_read, ready_write, _) = match linux_select_ready(parsed.snapshot) {
            Ok(ready) => ready,
            Err(error) => return error,
        };
        return linux_select_copy_result(
            parsed.snapshot,
            ready_read,
            ready_write,
            copy_to_linux_user,
        );
    }
    let Some(current) = current_linux_thread_handle() else {
        return LINUX_ESRCH;
    };
    let Some(target) = ready_linux_thread_handle() else {
        return LINUX_ENOSYS;
    };
    let Some(tls) = linux_thread_context(current) else {
        return LINUX_ESRCH;
    };
    let deadline = if let Some(duration) = parsed.duration {
        let Some(deadline) = linux_elapsed_nanoseconds().and_then(|now| now.checked_add(duration))
        else {
            return LINUX_EINVAL;
        };
        deadline
    } else {
        0
    };
    let slot = usize::from(current.slot);
    store_linux_select_snapshot(slot, parsed.snapshot);
    let wait_kind = if pselect {
        LINUX_THREAD_WAIT_PSELECT
    } else {
        LINUX_THREAD_WAIT_SELECT
    };
    if !save_linux_thread_return_context(current, caller)
        || !block_current_linux_thread(
            wait_kind,
            1,
            parsed.snapshot.input.descriptor_count,
            deadline,
        )
    {
        clear_linux_select_snapshot(slot);
        return LINUX_EINVAL;
    }
    let prior_signal_mask = X86_RUNTIME.ELF_PROBE_SIGNAL_MASK.load(Ordering::SeqCst);
    if pselect {
        X86_RUNTIME.LINUX_THREAD_WAIT_PRIOR_SIGNAL_MASKS[slot]
            .store(prior_signal_mask, Ordering::SeqCst);
        X86_RUNTIME.LINUX_THREAD_SIGNAL_MASKS[slot].store(
            parsed.temporary_signal_mask.unwrap_or(prior_signal_mask),
            Ordering::SeqCst,
        );
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
        X86_RUNTIME.LINUX_THREAD_WAIT_KINDS[slot].store(LINUX_THREAD_WAIT_NONE, Ordering::SeqCst);
        X86_RUNTIME.LINUX_THREAD_WAIT_KEYS[slot].store(0, Ordering::SeqCst);
        X86_RUNTIME.LINUX_THREAD_WAIT_OUTPUTS[slot].store(0, Ordering::SeqCst);
        X86_RUNTIME.LINUX_THREAD_WAIT_DEADLINES[slot].store(0, Ordering::SeqCst);
        X86_RUNTIME.LINUX_THREAD_WAIT_PRIOR_SIGNAL_MASKS[slot].store(0, Ordering::SeqCst);
        clear_linux_select_snapshot(slot);
        X86_RUNTIME.LINUX_THREAD_STATES[slot].store(LINUX_THREAD_RUNNING, Ordering::SeqCst);
        X86_RUNTIME
            .LINUX_CURRENT_THREAD_SLOT
            .store(current.slot, Ordering::SeqCst);
        let _ = install_linux_tls_context(tls);
        if pselect {
            store_current_linux_signal_mask(prior_signal_mask);
        }
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
    LINUX_RETURN_FROM_IRET
}

pub(super) fn linux_poll_live_return(
    output: *mut LinuxSyscallFrame,
    caller: LinuxUserContext,
    address: u64,
    count: u64,
    timeout: u64,
) -> u64 {
    let timeout = timeout as u32 as i32;
    let duration = if timeout < 0 {
        None
    } else {
        let Some(duration) = (timeout as u64).checked_mul(1_000_000) else {
            return LINUX_EINVAL;
        };
        Some(duration)
    };
    linux_poll_wait_live_return(
        output,
        caller,
        address,
        count,
        duration,
        LINUX_THREAD_WAIT_POLL,
        None,
    )
}

pub(super) fn linux_signal_suspend_live_return(
    output: *mut LinuxSyscallFrame,
    caller: LinuxUserContext,
    wait_kind: u8,
    temporary_signal_mask: Option<u64>,
) -> u64 {
    // The poll wait engine already provides the required atomic
    // save/block/schedule transition. A nonzero private key satisfies its
    // generation-safe wait record without exposing a descriptor or pointer.
    linux_poll_wait_live_return(output, caller, 1, 0, None, wait_kind, temporary_signal_mask)
}

pub(super) fn yield_current_linux_process(mut caller: LinuxUserContext, result: u64) -> bool {
    if !X86_RUNTIME.LINUX_RUNTIME_MODE.load(Ordering::SeqCst) || current_linux_process_pid() == 1 {
        return false;
    }
    caller.rax = result;
    let pid = current_linux_process_pid();
    if !refresh_live_linux_process_registry(caller) {
        return false;
    }
    X86_RUNTIME
        .LINUX_PROCESS_CHILD_YIELD_PID
        .store(pid, Ordering::SeqCst);
    X86_RUNTIME
        .LINUX_PROCESS_CHILD_YIELDED
        .store(true, Ordering::SeqCst);
    true
}

pub(super) fn restart_current_linux_process_syscall(
    mut caller: LinuxUserContext,
    syscall: u64,
) -> bool {
    let Some(instruction) = caller.rip.checked_sub(2) else {
        return false;
    };
    caller.rip = instruction;
    caller.rax = syscall;
    yield_current_linux_process(caller, syscall)
}

pub(super) fn yield_current_linux_process_wait(caller: LinuxUserContext) -> bool {
    yield_current_linux_process(caller, LINUX_EINTR)
}

fn linux_poll_synchronously(address: u64, active: &mut [u8], duration: Option<u64>) -> u64 {
    let deadline = duration
        .and_then(|duration| linux_elapsed_nanoseconds().and_then(|now| now.checked_add(duration)));
    loop {
        let mut ready = 0;
        for pollfd in active.as_chunks_mut::<8>().0 {
            let descriptor = i32::from_ne_bytes(pollfd[..4].try_into().unwrap());
            let events = u16::from_ne_bytes(pollfd[4..6].try_into().unwrap());
            let returned = linux_descriptor_poll_events(descriptor, events);
            pollfd[6..8].copy_from_slice(&returned.to_ne_bytes());
            ready += u64::from(returned != 0);
        }
        if ready != 0 {
            return if active.is_empty() || copy_to_linux_user(address, active) {
                ready
            } else {
                LINUX_EFAULT
            };
        }
        if deadline
            .is_some_and(|deadline| linux_elapsed_nanoseconds().is_some_and(|now| now >= deadline))
        {
            return if active.is_empty() || copy_to_linux_user(address, active) {
                0
            } else {
                LINUX_EFAULT
            };
        }
        core::hint::spin_loop();
    }
}

pub(super) fn linux_poll_wait_live_return(
    output: *mut LinuxSyscallFrame,
    caller: LinuxUserContext,
    address: u64,
    count: u64,
    duration: Option<u64>,
    wait_kind: u8,
    temporary_signal_mask: Option<u64>,
) -> u64 {
    let Ok(descriptor_count) = usize::try_from(count) else {
        return LINUX_EINVAL;
    };
    if descriptor_count > LINUX_POLLFD_LIMIT {
        return LINUX_EINVAL;
    }
    let mut pollfds = [0_u8; LINUX_POLLFD_LIMIT * 8];
    let active = &mut pollfds[..descriptor_count * 8];
    if descriptor_count != 0
        && (!copy_from_linux_user(address, active)
            || !linux_writable_user_range_contains(address, active.len()))
    {
        return LINUX_EFAULT;
    }
    let mut ready = 0_u64;
    for pollfd in active.as_chunks_mut::<8>().0 {
        let descriptor = i32::from_ne_bytes(pollfd[..4].try_into().unwrap());
        let events = u16::from_ne_bytes(pollfd[4..6].try_into().unwrap());
        let returned = linux_descriptor_poll_events(descriptor, events);
        pollfd[6..8].copy_from_slice(&returned.to_ne_bytes());
        ready += u64::from(returned != 0);
    }
    if descriptor_count == 0 && duration == Some(0) {
        return 0;
    }
    if ready != 0 || duration == Some(0) {
        return if copy_to_linux_user(address, active) {
            ready
        } else {
            LINUX_EFAULT
        };
    }
    // The current nested executor suspends a parent while its child runs. If
    // that child blocks on the parent's Unix-socket endpoint, spinning until
    // timeout prevents the only process that can make the descriptor ready
    // from running. Yield this narrow dependency back to the parent.
    let current_pid = current_linux_process_pid();
    let parent_pid = linux_process::live_process_parent_pid(current_pid);
    let waits_for_parent_unix_peer = parent_pid.is_some_and(|parent_pid| {
        active.as_chunks::<8>().0.iter().any(|pollfd| {
            let descriptor = i32::from_ne_bytes(pollfd[..4].try_into().unwrap());
            let Some(peer) = (descriptor >= 0)
                .then(|| linux_socket_slot(descriptor as u64))
                .flatten()
                .filter(|slot| {
                    crate::linux_socket::kind(*slot) == Some(crate::linux_socket::Kind::Unix)
                })
                .and_then(crate::linux_socket::local_peer_slot)
            else {
                return false;
            };
            linux_process_runtime_state(parent_pid)
                .filter(|state| state.valid)
                .is_some_and(|state| {
                    state
                        .descriptors
                        .stdio_sockets
                        .iter()
                        .chain(&state.descriptors.sockets)
                        .any(|stored| *stored == peer + 1)
                })
        })
    });
    let polls_socket = active.as_chunks::<8>().0.iter().any(|pollfd| {
        let descriptor = i32::from_ne_bytes(pollfd[..4].try_into().unwrap());
        descriptor >= 0 && linux_socket_slot(descriptor as u64).is_some()
    });
    if !waits_for_parent_unix_peer && polls_socket {
        return linux_poll_synchronously(address, active, duration);
    }
    let Some(current) = current_linux_thread_handle() else {
        return LINUX_ESRCH;
    };
    let Some(target) = ready_linux_thread_handle() else {
        // A lone Linux process has no peer thread to schedule while a bounded
        // poll sleeps. Drive descriptor readiness until its deadline instead
        // of leaking the scheduler's lack of a target as ENOSYS to userspace.
        if !waits_for_parent_unix_peer && duration.is_some() {
            return linux_poll_synchronously(address, active, duration);
        }
        if waits_for_parent_unix_peer
            && X86_RUNTIME
                .LINUX_PROCESS_SCHEDULER_NESTING
                .load(Ordering::SeqCst)
                != 0
            && yield_current_linux_process(caller, 0)
        {
            return LINUX_RETURN_TO_KERNEL;
        }
        if matches!(
            wait_kind,
            LINUX_THREAD_WAIT_PAUSE | LINUX_THREAD_WAIT_SIGSUSPEND
        ) && yield_current_linux_process_wait(caller)
        {
            return LINUX_RETURN_TO_KERNEL;
        }
        return LINUX_ENOSYS;
    };
    let Some(tls) = linux_thread_context(current) else {
        return LINUX_ESRCH;
    };
    let deadline = if let Some(duration) = duration {
        let Some(deadline) = linux_elapsed_nanoseconds().and_then(|now| now.checked_add(duration))
        else {
            return LINUX_EINVAL;
        };
        deadline
    } else {
        0
    };
    let slot = usize::from(current.slot);
    for (index, pollfd) in active.as_chunks::<8>().0.iter().enumerate() {
        X86_RUNTIME.LINUX_THREAD_POLL_DESCRIPTORS[slot][index].store(
            u32::from_ne_bytes(pollfd[..4].try_into().unwrap()),
            Ordering::SeqCst,
        );
        X86_RUNTIME.LINUX_THREAD_POLL_EVENTS[slot][index].store(
            u16::from_ne_bytes(pollfd[4..6].try_into().unwrap()),
            Ordering::SeqCst,
        );
    }
    if !save_linux_thread_return_context(current, caller)
        || !block_current_linux_thread(wait_kind, address, descriptor_count as u64, deadline)
    {
        clear_linux_poll_snapshot(slot);
        return LINUX_EINVAL;
    }
    let prior_signal_mask = X86_RUNTIME.ELF_PROBE_SIGNAL_MASK.load(Ordering::SeqCst);
    if let Some(mask) = temporary_signal_mask {
        X86_RUNTIME.LINUX_THREAD_WAIT_PRIOR_SIGNAL_MASKS[slot]
            .store(prior_signal_mask, Ordering::SeqCst);
        X86_RUNTIME.LINUX_THREAD_SIGNAL_MASKS[slot].store(mask, Ordering::SeqCst);
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
        X86_RUNTIME.LINUX_THREAD_WAIT_KINDS[slot].store(LINUX_THREAD_WAIT_NONE, Ordering::SeqCst);
        X86_RUNTIME.LINUX_THREAD_WAIT_KEYS[slot].store(0, Ordering::SeqCst);
        X86_RUNTIME.LINUX_THREAD_WAIT_OUTPUTS[slot].store(0, Ordering::SeqCst);
        X86_RUNTIME.LINUX_THREAD_WAIT_DEADLINES[slot].store(0, Ordering::SeqCst);
        X86_RUNTIME.LINUX_THREAD_WAIT_PRIOR_SIGNAL_MASKS[slot].store(0, Ordering::SeqCst);
        clear_linux_poll_snapshot(slot);
        X86_RUNTIME.LINUX_THREAD_STATES[slot].store(LINUX_THREAD_RUNNING, Ordering::SeqCst);
        X86_RUNTIME
            .LINUX_CURRENT_THREAD_SLOT
            .store(current.slot, Ordering::SeqCst);
        let _ = install_linux_tls_context(tls);
        if temporary_signal_mask.is_some() {
            store_current_linux_signal_mask(prior_signal_mask);
        }
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
    match wait_kind {
        LINUX_THREAD_WAIT_PPOLL => {
            X86_RUNTIME
                .LINUX_PPOLL_WAIT_BLOCK_COUNT
                .fetch_add(1, Ordering::SeqCst);
        }
        LINUX_THREAD_WAIT_PAUSE | LINUX_THREAD_WAIT_SIGSUSPEND => {}
        _ => {
            X86_RUNTIME
                .LINUX_POLL_WAIT_BLOCK_COUNT
                .fetch_add(1, Ordering::SeqCst);
        }
    }
    LINUX_RETURN_FROM_IRET
}

pub(super) fn linux_ppoll_live_return(
    output: *mut LinuxSyscallFrame,
    caller: LinuxUserContext,
    address: u64,
    count: u64,
    timeout_address: u64,
    signal_mask_address: u64,
    signal_set_size: u64,
) -> u64 {
    let duration = if timeout_address == 0 {
        None
    } else {
        let mut timeout = [0_u8; 16];
        if !copy_from_linux_user(timeout_address, &mut timeout) {
            return LINUX_EFAULT;
        }
        let seconds = i64::from_ne_bytes(timeout[..8].try_into().unwrap());
        let nanoseconds = i64::from_ne_bytes(timeout[8..].try_into().unwrap());
        if seconds < 0 || !(0..NANOSECONDS_PER_SECOND as i64).contains(&nanoseconds) {
            return LINUX_EINVAL;
        }
        let Some(duration) = (seconds as u64)
            .checked_mul(NANOSECONDS_PER_SECOND)
            .and_then(|seconds| seconds.checked_add(nanoseconds as u64))
        else {
            return LINUX_EINVAL;
        };
        Some(duration)
    };
    let temporary_signal_mask = match linux_temporary_signal_mask(LinuxSignalMaskInput {
        address: signal_mask_address,
        size: signal_set_size,
    }) {
        Ok(mask) => mask,
        Err(error) => return error,
    };
    linux_poll_wait_live_return(
        output,
        caller,
        address,
        count,
        duration,
        LINUX_THREAD_WAIT_PPOLL,
        Some(temporary_signal_mask),
    )
}

pub(super) fn linux_epoll_create1(flags: u64) -> u64 {
    let flags = u64::from(flags as u32);
    if flags & !LINUX_O_CLOEXEC != 0 {
        return LINUX_EINVAL;
    }
    let runtime = X86_RUNTIME.LINUX_RUNTIME_MODE.load(Ordering::SeqCst);
    if !runtime
        && X86_RUNTIME
            .ELF_PROBE_EPOLL_DESCRIPTOR
            .load(Ordering::SeqCst)
            != 0
    {
        return LINUX_EMFILE;
    }
    let descriptor_start = if runtime {
        LINUX_AUX_DESCRIPTOR_FIRST
    } else {
        0
    };
    let Some(descriptor) = (descriptor_start..LINUX_AUX_DESCRIPTOR_LIMIT)
        .find(|descriptor| !linux_descriptor_open(*descriptor))
    else {
        return LINUX_EMFILE;
    };
    if runtime {
        let Some(active) = descriptor
            .checked_sub(LINUX_AUX_DESCRIPTOR_FIRST)
            .and_then(|index| X86_RUNTIME.LINUX_RUNTIME_EPOLL_ACTIVE.get(index as usize))
        else {
            return LINUX_EMFILE;
        };
        active.store(1, Ordering::SeqCst);
        linux_epoll_forget_owner(descriptor);
    } else {
        X86_RUNTIME
            .ELF_PROBE_EPOLL_DESCRIPTOR
            .store(descriptor as u16 + 1, Ordering::SeqCst);
        for target in 0..LINUX_DESCRIPTOR_TABLE_LIMIT as usize {
            X86_RUNTIME.ELF_PROBE_EPOLL_EVENTS[target].store(0, Ordering::SeqCst);
            X86_RUNTIME.ELF_PROBE_EPOLL_DATA[target].store(0, Ordering::SeqCst);
        }
    }
    set_linux_descriptor_close_on_exec(descriptor, flags & LINUX_O_CLOEXEC != 0);
    descriptor
}
