use super::*;

pub(super) fn linux_rt_sigaction(signal: u64, action: u64, old_action: u64, set_size: u64) -> u64 {
    let signal = u64::from(signal as u32);
    if set_size != LINUX_SIGNAL_SET_SIZE || !(1..=LINUX_SIGNAL_COUNT as u64).contains(&signal) {
        return LINUX_EINVAL;
    }
    if action != 0 && matches!(signal, 9 | 19) {
        return LINUX_EINVAL;
    }
    if old_action != 0 && !linux_writable_user_range_contains(old_action, LINUX_SIGNAL_ACTION_SIZE)
    {
        return LINUX_EFAULT;
    }
    let parse_word = |bytes: &[u8; LINUX_SIGNAL_ACTION_SIZE], offset: usize| {
        let mut word = [0_u8; 8];
        word.copy_from_slice(&bytes[offset..offset + 8]);
        u64::from_ne_bytes(word)
    };
    let new_action = if action == 0 {
        None
    } else {
        let mut bytes = [0_u8; LINUX_SIGNAL_ACTION_SIZE];
        if !copy_from_linux_user(action, &mut bytes) {
            return LINUX_EFAULT;
        }
        Some([
            parse_word(&bytes, 0),
            parse_word(&bytes, 8),
            parse_word(&bytes, 16),
            parse_word(&bytes, 24) & !LINUX_UNBLOCKABLE_SIGNALS,
        ])
    };
    let index = signal as usize - 1;
    let current = [
        X86_RUNTIME.ELF_PROBE_SIGNAL_HANDLERS[index].load(Ordering::SeqCst),
        X86_RUNTIME.ELF_PROBE_SIGNAL_FLAGS[index].load(Ordering::SeqCst),
        X86_RUNTIME.ELF_PROBE_SIGNAL_RESTORERS[index].load(Ordering::SeqCst),
        X86_RUNTIME.ELF_PROBE_SIGNAL_ACTION_MASKS[index].load(Ordering::SeqCst),
    ];
    if old_action != 0 {
        let mut bytes = [0_u8; LINUX_SIGNAL_ACTION_SIZE];
        for (offset, word) in current.into_iter().enumerate() {
            bytes[offset * 8..offset * 8 + 8].copy_from_slice(&word.to_ne_bytes());
        }
        if !copy_to_linux_user(old_action, &bytes) {
            return LINUX_EFAULT;
        }
    }
    if let Some([handler, flags, restorer, mask]) = new_action {
        X86_RUNTIME.ELF_PROBE_SIGNAL_HANDLERS[index].store(handler, Ordering::SeqCst);
        X86_RUNTIME.ELF_PROBE_SIGNAL_FLAGS[index].store(flags, Ordering::SeqCst);
        X86_RUNTIME.ELF_PROBE_SIGNAL_RESTORERS[index].store(restorer, Ordering::SeqCst);
        X86_RUNTIME.ELF_PROBE_SIGNAL_ACTION_MASKS[index].store(mask, Ordering::SeqCst);
    }
    0
}

pub(super) fn linux_sigaltstack(new_stack: u64, old_stack: u64) -> u64 {
    if old_stack != 0 && !linux_writable_user_range_contains(old_stack, LINUX_ALT_STACK_SIZE) {
        return LINUX_EFAULT;
    }
    if new_stack != 0
        && X86_RUNTIME.ELF_PROBE_ALT_STACK_FLAGS.load(Ordering::SeqCst) == LINUX_SS_ONSTACK
    {
        return LINUX_EPERM;
    }
    let replacement = if new_stack == 0 {
        None
    } else {
        let mut bytes = [0_u8; LINUX_ALT_STACK_SIZE];
        if !copy_from_linux_user(new_stack, &mut bytes) {
            return LINUX_EFAULT;
        }
        let mut pointer_bytes = [0_u8; 8];
        let mut flags_bytes = [0_u8; 4];
        let mut size_bytes = [0_u8; 8];
        pointer_bytes.copy_from_slice(&bytes[..8]);
        flags_bytes.copy_from_slice(&bytes[8..12]);
        size_bytes.copy_from_slice(&bytes[16..24]);
        let pointer = u64::from_ne_bytes(pointer_bytes);
        let flags = u32::from_ne_bytes(flags_bytes);
        let size = u64::from_ne_bytes(size_bytes);
        match flags {
            LINUX_SS_DISABLE => Some((0, LINUX_SS_DISABLE, 0)),
            0 | LINUX_SS_AUTODISARM if size < LINUX_MIN_ALT_STACK_SIZE => return LINUX_ENOMEM,
            0 | LINUX_SS_AUTODISARM => {
                let Ok(length) = usize::try_from(size) else {
                    return LINUX_EFAULT;
                };
                if !linux_writable_user_range_contains(pointer, length) {
                    return LINUX_EFAULT;
                }
                Some((pointer, flags, size))
            }
            _ => return LINUX_EINVAL,
        }
    };
    if old_stack != 0 {
        let mut bytes = [0_u8; LINUX_ALT_STACK_SIZE];
        bytes[..8].copy_from_slice(
            &X86_RUNTIME
                .ELF_PROBE_ALT_STACK_POINTER
                .load(Ordering::SeqCst)
                .to_ne_bytes(),
        );
        bytes[8..12].copy_from_slice(
            &X86_RUNTIME
                .ELF_PROBE_ALT_STACK_FLAGS
                .load(Ordering::SeqCst)
                .to_ne_bytes(),
        );
        bytes[16..24].copy_from_slice(
            &X86_RUNTIME
                .ELF_PROBE_ALT_STACK_SIZE
                .load(Ordering::SeqCst)
                .to_ne_bytes(),
        );
        if !copy_to_linux_user(old_stack, &bytes) {
            return LINUX_EFAULT;
        }
    }
    if let Some((pointer, flags, size)) = replacement {
        X86_RUNTIME
            .ELF_PROBE_ALT_STACK_POINTER
            .store(pointer, Ordering::SeqCst);
        X86_RUNTIME
            .ELF_PROBE_ALT_STACK_FLAGS
            .store(flags, Ordering::SeqCst);
        X86_RUNTIME
            .ELF_PROBE_ALT_STACK_SIZE
            .store(size, Ordering::SeqCst);
    }
    0
}

pub(super) fn linux_rt_sigprocmask(how: u64, set: u64, old_set: u64, set_size: u64) -> u64 {
    let how = u64::from(how as u32);
    if set_size != LINUX_SIGNAL_SET_SIZE {
        return LINUX_EINVAL;
    }
    if old_set != 0 && !linux_writable_user_range_contains(old_set, LINUX_SIGNAL_SET_SIZE as usize)
    {
        return LINUX_EFAULT;
    }
    let requested = if set == 0 {
        None
    } else {
        let mut bytes = [0_u8; LINUX_SIGNAL_SET_SIZE as usize];
        if !copy_from_linux_user(set, &mut bytes) {
            return LINUX_EFAULT;
        }
        Some(u64::from_ne_bytes(bytes) & !LINUX_UNBLOCKABLE_SIGNALS)
    };
    let current = X86_RUNTIME.ELF_PROBE_SIGNAL_MASK.load(Ordering::SeqCst);
    let next = match requested {
        None => current,
        Some(mask) if how == LINUX_SIG_BLOCK => current | mask,
        Some(mask) if how == LINUX_SIG_UNBLOCK => current & !mask,
        Some(mask) if how == LINUX_SIG_SETMASK => mask,
        Some(_) => return LINUX_EINVAL,
    };
    if old_set != 0 && !copy_to_linux_user(old_set, &current.to_ne_bytes()) {
        return LINUX_EFAULT;
    }
    store_current_linux_signal_mask(next);
    0
}

pub(super) fn linux_rt_sigpending(set: u64, set_size: u64) -> u64 {
    if set_size != LINUX_SIGNAL_SET_SIZE {
        return LINUX_EINVAL;
    }
    let dynamic = linux_dynamic_pending_masks().map_or(0, |(process, thread)| process | thread);
    let pending = (X86_RUNTIME.ELF_PROBE_PENDING_SIGNALS.load(Ordering::SeqCst) | dynamic)
        & X86_RUNTIME.ELF_PROBE_SIGNAL_MASK.load(Ordering::SeqCst);
    if copy_to_linux_user(set, &pending.to_ne_bytes()) {
        0
    } else {
        LINUX_EFAULT
    }
}

pub(super) fn linux_rt_sigqueueinfo(
    process: u64,
    thread: Option<u64>,
    signal: u64,
    information: u64,
    output: *mut LinuxSyscallFrame,
    interrupted: LinuxUserContext,
    return_registers: (u64, u64),
) -> u64 {
    let process = u64::from(process as u32);
    let thread = thread.map(|thread| u64::from(thread as u32));
    let signal = u64::from(signal as u32);
    let current_process = u64::from(current_linux_process_pid());
    let resolved_thread = if let Some(thread) = thread {
        match linux_tgkill_target(process, thread) {
            Ok(target) => Some(target),
            Err(error) => return error,
        }
    } else {
        None
    };
    let target_thread = resolved_thread.and_then(|target| match target {
        LinuxThreadSignalTarget::Active(target) => Some(target),
        LinuxThreadSignalTarget::Saved { .. } => None,
    });
    if signal == 0 {
        let target_process = resolved_thread.map_or(process as u32, |target| match target {
            LinuxThreadSignalTarget::Active(_) => current_linux_process_pid(),
            LinuxThreadSignalTarget::Saved { pid, .. } => pid,
        });
        if target_process != current_linux_process_pid() {
            if linux_process::live_process_slot(target_process).is_none() {
                return LINUX_ESRCH;
            }
            if !linux_signal_permitted(target_process, signal) {
                return LINUX_EPERM;
            }
        }
        return 0;
    }
    if !(1..=LINUX_SIGNAL_COUNT as u64).contains(&signal) {
        return LINUX_EINVAL;
    }
    let mut bytes = [0_u8; size_of::<LinuxSignalInformation>()];
    if !copy_from_linux_user(information, &mut bytes) {
        return LINUX_EFAULT;
    }
    let code = i32::from_ne_bytes(bytes[8..12].try_into().unwrap());
    if code >= 0 {
        return LINUX_EPERM;
    }
    let value = u64::from_ne_bytes(bytes[24..32].try_into().unwrap());
    let (sender_pid, sender_uid) = linux_signal_sender();
    if let Some(LinuxThreadSignalTarget::Saved {
        pid,
        process_slot,
        thread_slot,
    }) = resolved_thread
    {
        if !linux_signal_permitted(pid, signal) {
            return LINUX_EPERM;
        }
        return linux_queue_saved_thread_signal(
            pid,
            process_slot,
            thread_slot,
            signal,
            code,
            Some(value),
        );
    }
    if process != current_process {
        let process = process as u32;
        if linux_process::live_process_slot(process).is_none() {
            return LINUX_ESRCH;
        }
        if !linux_signal_permitted(process, signal) {
            return LINUX_EPERM;
        }
        return linux_queue_process_signal(process, signal, code, Some(value));
    }
    if wake_linux_signal_wait_target(
        target_thread,
        signal,
        code,
        Some(value),
        Some((sender_pid, sender_uid)),
    ) {
        return 0;
    }
    let handler = X86_RUNTIME.ELF_PROBE_SIGNAL_HANDLERS[signal as usize - 1].load(Ordering::SeqCst);
    if handler == 1 || handler == 0 && linux_default_signal_ignored(signal) {
        return 0;
    }
    let instance = linux_signal_instance();
    let process_slot = match linux_process::live_process_slot(process as u32) {
        Some(slot) => slot,
        None => return LINUX_ESRCH,
    };
    let (scope, slot, target_is_current, target_mask) = if let Some(target) = target_thread {
        let thread_slot = usize::from(target.slot);
        (
            crate::linux_signal::Scope::Thread,
            linux_thread_signal_queue_slot(process_slot, thread_slot),
            current_linux_thread_handle() == Some(target),
            X86_RUNTIME.LINUX_THREAD_SIGNAL_MASKS[thread_slot].load(Ordering::SeqCst),
        )
    } else {
        (
            crate::linux_signal::Scope::Process,
            process_slot,
            true,
            X86_RUNTIME.ELF_PROBE_SIGNAL_MASK.load(Ordering::SeqCst),
        )
    };
    if target_is_current && target_mask & (1 << (signal - 1)) == 0 {
        return linux_kill(
            u64::from(current_linux_process_pid()),
            (signal, code, Some(value)),
            output,
            interrupted,
            return_registers,
        );
    }
    match crate::linux_signal::enqueue(
        instance,
        scope,
        slot,
        crate::linux_signal::Pending {
            signal: signal as u8,
            code,
            value,
            sender_pid,
            sender_uid,
        },
        signal >= 32,
    ) {
        Ok(_) => 0,
        Err(crate::linux_signal::Error::NoMemory) => LINUX_EAGAIN,
        Err(crate::linux_signal::Error::Invalid) => LINUX_EINVAL,
    }
}

pub(super) fn linux_signal_instance() -> usize {
    current_container_slot().unwrap_or(control_abi::DOMAIN_LIMIT)
}

pub(super) fn linux_signal_sender() -> (u32, u32) {
    (
        current_linux_process_pid(),
        current_linux_credentials().map_or(0, |credentials| credentials.uid),
    )
}

pub(super) fn linux_thread_signal_queue_slot(process: usize, thread: usize) -> usize {
    process * crate::linux_signal::THREAD_LIMIT + thread
}

pub(super) fn clear_linux_signal_queues_for_process(instance: usize, process: usize) -> bool {
    crate::linux_signal::clear(instance, crate::linux_signal::Scope::Process, process).is_ok()
        && (0..crate::linux_signal::THREAD_LIMIT).all(|thread| {
            crate::linux_signal::clear(
                instance,
                crate::linux_signal::Scope::Thread,
                linux_thread_signal_queue_slot(process, thread),
            )
            .is_ok()
        })
}

pub(super) fn clear_current_linux_process_signal_queues() -> bool {
    linux_process::live_process_slot(current_linux_process_pid()).is_some_and(|process| {
        clear_linux_signal_queues_for_process(linux_signal_instance(), process)
    })
}

pub(super) fn linux_dynamic_pending_masks() -> Option<(u64, u64)> {
    let instance = linux_signal_instance();
    let process = linux_process::live_process_slot(current_linux_process_pid())?;
    let thread = current_linux_thread_slot()?;
    Some((
        crate::linux_signal::pending_mask(instance, crate::linux_signal::Scope::Process, process)
            .ok()?,
        crate::linux_signal::pending_mask(
            instance,
            crate::linux_signal::Scope::Thread,
            linux_thread_signal_queue_slot(process, thread),
        )
        .ok()?,
    ))
}

pub(super) fn linux_peek_dynamic_signal(
    requested: u64,
) -> Option<(crate::linux_signal::Scope, crate::linux_signal::Pending)> {
    let instance = linux_signal_instance();
    let process = linux_process::live_process_slot(current_linux_process_pid())?;
    let thread = current_linux_thread_slot()?;
    if let Some(pending) = crate::linux_signal::peek(
        instance,
        crate::linux_signal::Scope::Thread,
        linux_thread_signal_queue_slot(process, thread),
        requested,
    )
    .ok()?
    {
        return Some((crate::linux_signal::Scope::Thread, pending));
    }
    crate::linux_signal::peek(
        instance,
        crate::linux_signal::Scope::Process,
        process,
        requested,
    )
    .ok()?
    .map(|pending| (crate::linux_signal::Scope::Process, pending))
}

pub(super) fn linux_dequeue_dynamic_signal(
    scope: crate::linux_signal::Scope,
    requested: u64,
) -> Option<crate::linux_signal::Pending> {
    let instance = linux_signal_instance();
    let slot = match scope {
        crate::linux_signal::Scope::Process => {
            linux_process::live_process_slot(current_linux_process_pid())?
        }
        crate::linux_signal::Scope::Thread => linux_thread_signal_queue_slot(
            linux_process::live_process_slot(current_linux_process_pid())?,
            current_linux_thread_slot()?,
        ),
    };
    crate::linux_signal::dequeue(instance, scope, slot, requested)
        .ok()
        .flatten()
}

pub(super) fn linux_rt_sigtimedwait(
    set: u64,
    information: u64,
    timeout: u64,
    set_size: u64,
) -> u64 {
    if set_size != LINUX_SIGNAL_SET_SIZE {
        return LINUX_EINVAL;
    }
    let mut set_bytes = [0_u8; LINUX_SIGNAL_SET_SIZE as usize];
    if !copy_from_linux_user(set, &mut set_bytes) {
        return LINUX_EFAULT;
    }
    let requested = u64::from_ne_bytes(set_bytes) & !LINUX_UNBLOCKABLE_SIGNALS;
    let polls = if timeout == 0 {
        false
    } else {
        let mut timeout_bytes = [0_u8; 16];
        if !copy_from_linux_user(timeout, &mut timeout_bytes) {
            return LINUX_EFAULT;
        }
        let seconds = i64::from_ne_bytes(timeout_bytes[..8].try_into().unwrap());
        let nanoseconds = i64::from_ne_bytes(timeout_bytes[8..].try_into().unwrap());
        if seconds < 0 || !(0..NANOSECONDS_PER_SECOND as i64).contains(&nanoseconds) {
            return LINUX_EINVAL;
        }
        seconds == 0 && nanoseconds == 0
    };
    let dynamic = linux_dynamic_pending_masks().map_or(0, |(process, thread)| process | thread);
    let matching =
        (X86_RUNTIME.ELF_PROBE_PENDING_SIGNALS.load(Ordering::SeqCst) | dynamic) & requested;
    if matching == 0 {
        return if polls { LINUX_EAGAIN } else { LINUX_ENOSYS };
    }
    if information != 0
        && !linux_writable_user_range_contains(information, size_of::<LinuxSignalInformation>())
    {
        return LINUX_EFAULT;
    }
    let dynamic_candidate = linux_peek_dynamic_signal(requested);
    let legacy_signal = u64::from(matching.trailing_zeros()) + 1;
    let use_dynamic = dynamic_candidate.is_some_and(|(scope, pending)| {
        scope == crate::linux_signal::Scope::Thread || u64::from(pending.signal) <= legacy_signal
    });
    let dynamic_signal = use_dynamic.then_some(dynamic_candidate).flatten();
    let signal =
        dynamic_signal.map_or_else(|| legacy_signal, |(_, pending)| u64::from(pending.signal));
    let index = signal as usize - 1;
    let signal_bit = 1 << (signal - 1);
    let (sender_pid, sender_uid) = dynamic_signal
        .map_or_else(linux_signal_sender, |(_, pending)| {
            (pending.sender_pid, pending.sender_uid)
        });
    let signal_information = LinuxSignalInformation::queued_from(
        signal,
        dynamic_signal.map_or_else(
            || X86_RUNTIME.ELF_PROBE_PENDING_SIGNAL_CODES[index].load(Ordering::SeqCst) as i32,
            |(_, pending)| pending.code,
        ),
        dynamic_signal.map_or_else(
            || X86_RUNTIME.ELF_PROBE_PENDING_SIGNAL_VALUES[index].load(Ordering::SeqCst),
            |(_, pending)| pending.value,
        ),
        sender_pid,
        sender_uid,
    );
    if information != 0 {
        // SAFETY: every byte in this repr(C) value is initialized, including
        // padding and the complete signal-specific payload.
        let bytes = unsafe {
            core::slice::from_raw_parts(
                core::ptr::from_ref(&signal_information).cast::<u8>(),
                size_of::<LinuxSignalInformation>(),
            )
        };
        if !copy_to_linux_user(information, bytes) {
            return LINUX_EFAULT;
        }
    }
    if let Some((scope, _)) = dynamic_signal {
        let _ = linux_dequeue_dynamic_signal(scope, signal_bit);
    } else {
        X86_RUNTIME
            .ELF_PROBE_PENDING_SIGNALS
            .fetch_and(!signal_bit, Ordering::SeqCst);
        X86_RUNTIME.ELF_PROBE_PENDING_SIGNAL_CODES[index].store(0, Ordering::SeqCst);
        X86_RUNTIME.ELF_PROBE_PENDING_SIGNAL_VALUES[index].store(0, Ordering::SeqCst);
    }
    signal
}

pub(super) fn linux_rt_sigtimedwait_live_return(
    output: *mut LinuxSyscallFrame,
    caller: LinuxUserContext,
    set: u64,
    information: u64,
    timeout: u64,
    set_size: u64,
) -> u64 {
    let immediate = linux_rt_sigtimedwait(set, information, timeout, set_size);
    if immediate != LINUX_ENOSYS {
        return immediate;
    }
    let deadline = if timeout == 0 {
        0
    } else {
        let mut timeout_bytes = [0_u8; 16];
        if !copy_from_linux_user(timeout, &mut timeout_bytes) {
            return LINUX_EFAULT;
        }
        let Some(duration) = linux_timespec_nanoseconds(&timeout_bytes).filter(|value| *value != 0)
        else {
            return LINUX_EINVAL;
        };
        let Some(deadline) = linux_elapsed_nanoseconds().and_then(|now| now.checked_add(duration))
        else {
            return LINUX_EINVAL;
        };
        deadline
    };
    let mut set_bytes = [0_u8; LINUX_SIGNAL_SET_SIZE as usize];
    if !copy_from_linux_user(set, &mut set_bytes) {
        return LINUX_EFAULT;
    }
    let requested = u64::from_ne_bytes(set_bytes) & !LINUX_UNBLOCKABLE_SIGNALS;
    if requested == 0 {
        return LINUX_ENOSYS;
    }
    if information != 0
        && !linux_writable_user_range_contains(information, size_of::<LinuxSignalInformation>())
    {
        return LINUX_EFAULT;
    }
    let Some(current) = current_linux_thread_handle() else {
        return LINUX_ESRCH;
    };
    if X86_RUNTIME.ELF_PROBE_SYSTEM_INIT.load(Ordering::SeqCst)
        && current_linux_process_pid() == 1
        && current_linux_tid() == Some(1)
        && requested == 1 << (LINUX_SIGTERM - 1)
        && timeout == 0
        && information != 0
        && X86_RUNTIME.ELF_PROBE_SIGNAL_MASK.load(Ordering::SeqCst) & requested == requested
    {
        if X86_RUNTIME
            .LINUX_SYSTEM_SUPERVISOR_WAIT_COUNT
            .load(Ordering::SeqCst)
            != 0
            || !refresh_live_linux_process_registry(caller)
            || !save_linux_thread_return_context(current, caller)
            || !block_current_linux_thread(LINUX_THREAD_WAIT_SIGNAL, requested, information, 0)
        {
            return LINUX_EINVAL;
        }
        X86_RUNTIME
            .LINUX_SIGNAL_WAIT_BLOCK_COUNT
            .fetch_add(1, Ordering::SeqCst);
        X86_RUNTIME
            .LINUX_SYSTEM_SUPERVISOR_WAIT_COUNT
            .fetch_add(1, Ordering::SeqCst);
        return LINUX_RETURN_TO_KERNEL;
    }
    let Some(target) = ready_linux_thread_handle() else {
        return LINUX_ENOSYS;
    };
    let Some(tls) = linux_thread_context(current) else {
        return LINUX_ESRCH;
    };
    if !save_linux_thread_return_context(current, caller)
        || !block_current_linux_thread(LINUX_THREAD_WAIT_SIGNAL, requested, information, deadline)
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
    if !write_linux_live_return(output, selected) {
        return LINUX_EINVAL;
    }
    X86_RUNTIME
        .LINUX_LIVE_RETURN_VALIDATED
        .store(true, Ordering::SeqCst);
    X86_RUNTIME
        .LINUX_SIGNAL_WAIT_BLOCK_COUNT
        .fetch_add(1, Ordering::SeqCst);
    selected.rax
}

pub(super) fn wake_linux_signal_wait(
    signal: u64,
    signal_code: i32,
    signal_value: Option<u64>,
) -> bool {
    wake_linux_signal_wait_target(None, signal, signal_code, signal_value, None)
}

pub(super) fn wake_linux_signal_wait_target(
    target: Option<LinuxThreadHandle>,
    signal: u64,
    signal_code: i32,
    signal_value: Option<u64>,
    sender: Option<(u32, u32)>,
) -> bool {
    let signal_bit = 1_u64 << (signal - 1);
    for slot in 0..LINUX_THREAD_LIMIT {
        if target.is_some_and(|target| usize::from(target.slot) != slot) {
            continue;
        }
        if X86_RUNTIME.LINUX_THREAD_STATES[slot].load(Ordering::SeqCst) != LINUX_THREAD_BLOCKED
            || X86_RUNTIME.LINUX_THREAD_WAIT_KINDS[slot].load(Ordering::SeqCst)
                != LINUX_THREAD_WAIT_SIGNAL
            || X86_RUNTIME.LINUX_THREAD_WAIT_KEYS[slot].load(Ordering::SeqCst) & signal_bit == 0
        {
            continue;
        }
        let information = X86_RUNTIME.LINUX_THREAD_WAIT_OUTPUTS[slot].load(Ordering::SeqCst);
        if information != 0 {
            let value = sender.map_or_else(
                || LinuxSignalInformation::queued(signal, signal_code, signal_value.unwrap_or(0)),
                |(pid, uid)| {
                    LinuxSignalInformation::queued_from(
                        signal,
                        signal_code,
                        signal_value.unwrap_or(0),
                        pid,
                        uid,
                    )
                },
            );
            // SAFETY: LinuxSignalInformation is repr(C), and every byte of
            // this generated value, including padding, is initialized.
            let bytes = unsafe {
                core::slice::from_raw_parts(
                    core::ptr::from_ref(&value).cast::<u8>(),
                    size_of::<LinuxSignalInformation>(),
                )
            };
            if !copy_to_linux_user(information, bytes) {
                return false;
            }
        }
        X86_RUNTIME.LINUX_THREAD_RAXS[slot].store(signal, Ordering::SeqCst);
        X86_RUNTIME.LINUX_THREAD_WAIT_KINDS[slot].store(LINUX_THREAD_WAIT_NONE, Ordering::SeqCst);
        X86_RUNTIME.LINUX_THREAD_WAIT_KEYS[slot].store(0, Ordering::SeqCst);
        X86_RUNTIME.LINUX_THREAD_WAIT_OUTPUTS[slot].store(0, Ordering::SeqCst);
        X86_RUNTIME.LINUX_THREAD_WAIT_DEADLINES[slot].store(0, Ordering::SeqCst);
        X86_RUNTIME.LINUX_THREAD_STATES[slot].store(LINUX_THREAD_READY, Ordering::SeqCst);
        X86_RUNTIME
            .LINUX_SIGNAL_WAIT_WAKE_COUNT
            .fetch_add(1, Ordering::SeqCst);
        return true;
    }
    false
}

pub(super) fn expire_linux_waits() -> u32 {
    if !X86_RUNTIME.LINUX_TIMED_WAITS_ACTIVE.load(Ordering::Acquire) {
        return 0;
    }
    let Some(now) = linux_elapsed_nanoseconds() else {
        return 0;
    };
    let mut expired = 0;
    for slot in 0..LINUX_THREAD_LIMIT {
        let deadline = X86_RUNTIME.LINUX_THREAD_WAIT_DEADLINES[slot].load(Ordering::SeqCst);
        if deadline == 0
            || now < deadline
            || X86_RUNTIME.LINUX_THREAD_STATES[slot].load(Ordering::SeqCst) != LINUX_THREAD_BLOCKED
        {
            continue;
        }
        if matches!(
            X86_RUNTIME.LINUX_THREAD_WAIT_KINDS[slot].load(Ordering::SeqCst),
            LINUX_THREAD_WAIT_POLL | LINUX_THREAD_WAIT_PPOLL
        ) {
            if complete_linux_poll_wait(slot, true) {
                expired += 1;
            }
            continue;
        }
        if matches!(
            X86_RUNTIME.LINUX_THREAD_WAIT_KINDS[slot].load(Ordering::SeqCst),
            LINUX_THREAD_WAIT_EPOLL | LINUX_THREAD_WAIT_EPOLL_PWAIT
        ) {
            if complete_linux_epoll_wait(slot, true) {
                expired += 1;
            }
            continue;
        }
        if matches!(
            X86_RUNTIME.LINUX_THREAD_WAIT_KINDS[slot].load(Ordering::SeqCst),
            LINUX_THREAD_WAIT_SELECT | LINUX_THREAD_WAIT_PSELECT
        ) {
            if complete_linux_select_wait(slot, true) {
                expired += 1;
            }
            continue;
        }
        if X86_RUNTIME.LINUX_THREAD_WAIT_KINDS[slot].load(Ordering::SeqCst)
            == LINUX_THREAD_WAIT_TIMERFD_READ
        {
            if complete_linux_timerfd_read_wait(slot) {
                expired += 1;
            }
            continue;
        }
        let (result, counter) =
            match X86_RUNTIME.LINUX_THREAD_WAIT_KINDS[slot].load(Ordering::SeqCst) {
                LINUX_THREAD_WAIT_FUTEX => {
                    (LINUX_ETIMEDOUT, &X86_RUNTIME.LINUX_FUTEX_WAIT_TIMEOUT_COUNT)
                }
                LINUX_THREAD_WAIT_SIGNAL => {
                    (LINUX_EAGAIN, &X86_RUNTIME.LINUX_SIGNAL_WAIT_TIMEOUT_COUNT)
                }
                LINUX_THREAD_WAIT_SLEEP => (0, &X86_RUNTIME.LINUX_SLEEP_WAKE_COUNT),
                _ => continue,
            };
        X86_RUNTIME.LINUX_THREAD_RAXS[slot].store(result, Ordering::SeqCst);
        X86_RUNTIME.LINUX_THREAD_WAIT_KINDS[slot].store(LINUX_THREAD_WAIT_NONE, Ordering::SeqCst);
        X86_RUNTIME.LINUX_THREAD_WAIT_KEYS[slot].store(0, Ordering::SeqCst);
        X86_RUNTIME.LINUX_THREAD_WAIT_OUTPUTS[slot].store(0, Ordering::SeqCst);
        X86_RUNTIME.LINUX_THREAD_WAIT_DEADLINES[slot].store(0, Ordering::SeqCst);
        X86_RUNTIME.LINUX_THREAD_STATES[slot].store(LINUX_THREAD_READY, Ordering::SeqCst);
        counter.fetch_add(1, Ordering::SeqCst);
        expired += 1;
    }
    let active = (0..LINUX_THREAD_LIMIT).any(|slot| {
        X86_RUNTIME.LINUX_THREAD_WAIT_DEADLINES[slot].load(Ordering::SeqCst) != 0
            && X86_RUNTIME.LINUX_THREAD_STATES[slot].load(Ordering::SeqCst) == LINUX_THREAD_BLOCKED
    });
    X86_RUNTIME
        .LINUX_TIMED_WAITS_ACTIVE
        .store(active, Ordering::Release);
    expired
}

pub(super) fn save_linux_interrupt_context(
    handle: LinuxThreadHandle,
    context: UserFaultContext,
    fs_base: u64,
) -> bool {
    let saved = LinuxUserContext {
        registers: LinuxSyscallFrame {
            r15: context.r15,
            r14: context.r14,
            r13: context.r13,
            r12: context.r12,
            rbp: context.rbp,
            rbx: context.rbx,
            r10: context.r10,
            r9: context.r9,
            r8: context.r8,
            rdx: context.rdx,
            rsi: context.rsi,
            rdi: context.rdi,
        },
        rip: context.rip,
        rsp: context.rsp,
        rflags: context.rflags,
        rax: context.rax,
        fs_base,
    };
    if !save_linux_thread_return_context(handle, saved) {
        return false;
    }
    let slot = usize::from(handle.slot);
    X86_RUNTIME.LINUX_THREAD_RCXS[slot].store(context.rcx, Ordering::SeqCst);
    X86_RUNTIME.LINUX_THREAD_R11S[slot].store(context.r11, Ordering::SeqCst);
    true
}

pub(super) fn linux_interrupt_context(handle: LinuxThreadHandle) -> Option<UserFaultContext> {
    let slot = usize::from(handle.slot);
    let saved = linux_thread_return_context(handle)?;
    Some(UserFaultContext {
        r15: saved.registers.r15,
        r14: saved.registers.r14,
        r13: saved.registers.r13,
        r12: saved.registers.r12,
        r11: X86_RUNTIME.LINUX_THREAD_R11S[slot].load(Ordering::SeqCst),
        r10: saved.registers.r10,
        r9: saved.registers.r9,
        r8: saved.registers.r8,
        rbp: saved.registers.rbp,
        rdi: saved.registers.rdi,
        rsi: saved.registers.rsi,
        rdx: saved.registers.rdx,
        rcx: X86_RUNTIME.LINUX_THREAD_RCXS[slot].load(Ordering::SeqCst),
        rbx: saved.registers.rbx,
        rax: saved.rax,
        rip: saved.rip,
        rflags: saved.rflags,
        rsp: saved.rsp,
    })
}

pub(crate) fn expire_linux_waits_from_timer(frame: &mut UserInterruptFrame) {
    if X86_RUNTIME.LINUX_RUNTIME_MODE.load(Ordering::SeqCst)
        && !X86_RUNTIME
            .LINUX_RUNTIME_STALLED_DIAGNOSTIC_WRITTEN
            .load(Ordering::SeqCst)
        && linux_elapsed_nanoseconds().is_some_and(|elapsed| elapsed >= 60_000_000_000)
        && !X86_RUNTIME
            .LINUX_RUNTIME_STALLED_DIAGNOSTIC_WRITTEN
            .swap(true, Ordering::SeqCst)
    {
        serial_write("ginux: Linux runtime exceeded sixty seconds; execution snapshot:");
        serial_write(" pid=");
        serial_write_u64(u64::from(current_linux_process_pid()));
        serial_write(" tid=");
        serial_write_u64(current_linux_tid().map_or(0, u64::from));
        serial_write(" rip=");
        serial_write_u64(
            frame
                .user_context(USER_CODE_SELECTOR, USER_DATA_SELECTOR)
                .map_or(0, |context| context.rip),
        );
        serial_write(" rsp=");
        serial_write_u64(
            frame
                .user_context(USER_CODE_SELECTOR, USER_DATA_SELECTOR)
                .map_or(0, |context| context.rsp),
        );
        serial_write(" syscalls=");
        serial_write_u64(
            X86_RUNTIME
                .LINUX_RUNTIME_SYSCALL_TRACE_COUNT
                .load(Ordering::SeqCst),
        );
        serial_write(" epoll-fd/events=");
        serial_write_u64(
            X86_RUNTIME
                .LINUX_EPOLL_LAST_READY_DESCRIPTOR
                .load(Ordering::SeqCst),
        );
        serial_write("/");
        serial_write_u64(
            X86_RUNTIME
                .LINUX_EPOLL_LAST_READY_EVENTS
                .load(Ordering::SeqCst),
        );
        serial_write(" name=");
        let mut name = [0_u8; 16];
        name[..8].copy_from_slice(
            &X86_RUNTIME
                .ELF_PROBE_PROCESS_NAME_LOW
                .load(Ordering::SeqCst)
                .to_ne_bytes(),
        );
        name[8..].copy_from_slice(
            &X86_RUNTIME
                .ELF_PROBE_PROCESS_NAME_HIGH
                .load(Ordering::SeqCst)
                .to_ne_bytes(),
        );
        let length = name
            .iter()
            .position(|byte| *byte == 0)
            .unwrap_or(name.len());
        crate::serial_write_bytes(&name[..length]);
        serial_write("\r\n");
        write_linux_runtime_syscall_trace();
    }
    let descriptor_wakes = wake_linux_descriptor_waits();
    let expired = expire_linux_waits();
    // Detached domains keep the CPU until the operator has input. Polling at
    // 200 Hz bounds console latency without serializing an idle runtime on
    // every timer interrupt.
    if X86_RUNTIME.LINUX_RUNTIME_MODE.load(Ordering::Acquire)
        && active_container_slot().is_some_and(|slot| {
            matches!(
                container_state(slot),
                control_abi::DOMAIN_STATE_BOOTING | control_abi::DOMAIN_STATE_RUNNING
            ) && !X86_RUNTIME.CONTAINER_CONSOLE_ATTACHED[slot].load(Ordering::Acquire)
        })
        && X86_RUNTIME
            .LINUX_RUNTIME_BACKGROUND_TICKS
            .fetch_add(1, Ordering::AcqRel)
            + 1
            >= 5
    {
        X86_RUNTIME
            .LINUX_RUNTIME_BACKGROUND_TICKS
            .store(0, Ordering::Release);
        if crate::serial_input_ready() {
            X86_RUNTIME
                .LINUX_RUNTIME_BACKGROUND_YIELD_REQUESTED
                .store(true, Ordering::Release);
        }
    }
    // A Linux runtime thread can wait in a userspace spin lock without making
    // another syscall. Give a runnable sibling a bounded time slice so such a
    // lock cannot deadlock merely because the current thread never yields.
    let runtime_quantum = if X86_RUNTIME.LINUX_RUNTIME_MODE.load(Ordering::SeqCst) {
        let ticks = X86_RUNTIME
            .LINUX_RUNTIME_PREEMPT_TICKS
            .fetch_add(1, Ordering::SeqCst)
            + 1;
        if ticks >= 10 {
            X86_RUNTIME
                .LINUX_RUNTIME_PREEMPT_TICKS
                .store(0, Ordering::SeqCst);
            ready_linux_thread_handle().is_some()
        } else {
            false
        }
    } else {
        false
    };
    if descriptor_wakes == 0 && expired == 0 && !runtime_quantum {
        return;
    }
    X86_RUNTIME
        .LINUX_TIMER_WAIT_EXPIRY_COUNT
        .fetch_add(expired, Ordering::SeqCst);
    let Some(interrupted) = frame.user_context(USER_CODE_SELECTOR, USER_DATA_SELECTOR) else {
        return;
    };
    if let (Some(current), Some(target)) =
        (current_linux_thread_handle(), ready_linux_thread_handle())
    {
        let Some(target_context) = linux_interrupt_context(target) else {
            return;
        };
        let mut restored = *frame;
        if !restored.restore_user_context(target_context, USER_CODE_SELECTOR, USER_DATA_SELECTOR)
            || !save_linux_interrupt_context(current, interrupted, active_fs_base())
            || !switch_linux_thread(target)
        {
            return;
        }
        *frame = restored;
        X86_RUNTIME
            .LINUX_LAST_PREEMPTED_THREAD_SLOT
            .store(current.slot, Ordering::Release);
        X86_RUNTIME
            .LINUX_TIMER_WAIT_PREEMPTION_COUNT
            .fetch_add(1, Ordering::SeqCst);
    }
}

pub(super) fn linux_executable_user_address(address: u64) -> bool {
    let code = X86_RUNTIME.ELF_PROBE_CODE_BASE.load(Ordering::SeqCst);
    let code_end = code.saturating_add(X86_RUNTIME.ELF_PROBE_CODE_SIZE.load(Ordering::SeqCst));
    let loader = X86_RUNTIME
        .ELF_PROBE_LOADER_CODE_BASE
        .load(Ordering::SeqCst);
    let loader_end = loader.saturating_add(
        X86_RUNTIME
            .ELF_PROBE_LOADER_CODE_SIZE
            .load(Ordering::SeqCst),
    );
    (code..code_end).contains(&address) || (loader..loader_end).contains(&address)
}

pub(super) fn linux_signal_action_deliverable(handler: u64, flags: u64, restorer: u64) -> bool {
    let supported = LINUX_SA_SIGINFO
        | LINUX_SA_RESTORER
        | LINUX_SA_ONSTACK
        | LINUX_SA_NODEFER
        | LINUX_SA_RESETHAND;
    handler > 1
        && flags & LINUX_SA_RESTORER != 0
        && flags & !supported == 0
        && linux_executable_user_address(handler)
        && linux_executable_user_address(restorer)
}

pub(super) fn linux_signal_stack(stack_top: u64, use_alt_stack: bool) -> Option<u64> {
    let frame_top = if use_alt_stack {
        stack_top
    } else {
        stack_top.checked_sub(LINUX_SIGNAL_RED_ZONE_SIZE)?
    };
    let signal_stack = frame_top
        .checked_sub(LINUX_SIGNAL_FRAME_SIZE as u64)?
        .checked_div(16)?
        .checked_mul(16)?
        .checked_sub(8)?;
    if use_alt_stack
        && signal_stack
            < X86_RUNTIME
                .ELF_PROBE_ALT_STACK_POINTER
                .load(Ordering::SeqCst)
    {
        None
    } else {
        Some(signal_stack)
    }
}

pub(super) fn linux_rt_signal_frame(
    context: LinuxSignalContext,
    flags: u64,
    information: LinuxSignalInformation,
    trap: u64,
    error: u64,
    cr2: u64,
) -> LinuxRtSignalFrame {
    let registers = context.registers;
    let alt_stack_flags = context.alt_stack_flags;
    let alt_stack_disabled = alt_stack_flags == LINUX_SS_DISABLE;
    LinuxRtSignalFrame {
        restorer: context.signal_restorer,
        context: LinuxSignalUserContext {
            flags: LINUX_UC_SIGCONTEXT_SS | LINUX_UC_STRICT_RESTORE_SS,
            link: 0,
            stack: LinuxSignalStack {
                pointer: if alt_stack_disabled {
                    0
                } else {
                    context.alt_stack_pointer
                },
                flags: alt_stack_flags,
                padding: 0,
                size: if alt_stack_disabled {
                    0
                } else {
                    context.alt_stack_size
                },
            },
            machine: LinuxSignalMachineContext {
                r8: registers.r8,
                r9: registers.r9,
                r10: registers.r10,
                r11: context.r11,
                r12: registers.r12,
                r13: registers.r13,
                r14: registers.r14,
                r15: registers.r15,
                rdi: registers.rdi,
                rsi: registers.rsi,
                rbp: registers.rbp,
                rbx: registers.rbx,
                rdx: registers.rdx,
                rax: context.rax,
                rcx: context.rcx,
                rsp: context.rsp,
                rip: context.rip,
                rflags: context.rflags,
                cs: USER_CODE_SELECTOR,
                gs: 0,
                fs: 0,
                ss: USER_DATA_SELECTOR,
                error,
                trap,
                old_mask: context.signal_mask,
                cr2,
                fpstate: 0,
                reserved: [0; 8],
            },
            signal_mask: context.signal_mask,
        },
        information: if flags & LINUX_SA_SIGINFO != 0 {
            information
        } else {
            LinuxSignalInformation::ZERO
        },
    }
}

pub(super) fn copy_linux_signal_frame(address: u64, frame: &LinuxRtSignalFrame) -> bool {
    // SAFETY: every field, including ABI padding and reserved bytes, is
    // initialized above and the resulting byte view is used only for copy-out.
    let bytes = unsafe {
        core::slice::from_raw_parts(
            core::ptr::from_ref(frame).cast::<u8>(),
            size_of::<LinuxRtSignalFrame>(),
        )
    };
    copy_to_linux_user(address, bytes)
}

pub(super) fn copy_linux_signal_frame_from_user(
    address: u64,
    frame: &mut LinuxRtSignalFrame,
) -> bool {
    // SAFETY: the destination is a fully initialized, repr(C) frame and the
    // mutable byte view spans exactly that object for bounded copy-in.
    let bytes = unsafe {
        core::slice::from_raw_parts_mut(
            core::ptr::from_mut(frame).cast::<u8>(),
            size_of::<LinuxRtSignalFrame>(),
        )
    };
    copy_from_linux_user(address, bytes)
}

pub(super) fn linux_restored_signal_context(
    saved: LinuxSignalContext,
    returned: &LinuxRtSignalFrame,
) -> Option<LinuxRestoredSignalContext> {
    let expected = linux_rt_signal_frame(saved, 0, LinuxSignalInformation::ZERO, 0, 0, 0);
    let context = &returned.context;
    let machine = &context.machine;
    if returned.restorer != saved.signal_restorer
        || context.flags != LINUX_UC_SIGCONTEXT_SS | LINUX_UC_STRICT_RESTORE_SS
        || context.link != 0
        || context.stack != expected.context.stack
        || machine.cs != USER_CODE_SELECTOR
        || machine.gs != 0
        || machine.fs != 0
        || machine.ss != USER_DATA_SELECTOR
        || machine.fpstate != 0
        || machine.reserved != [0; 8]
    {
        return None;
    }
    let restored = LinuxUserContext {
        registers: LinuxSyscallFrame {
            r15: machine.r15,
            r14: machine.r14,
            r13: machine.r13,
            r12: machine.r12,
            rbp: machine.rbp,
            rbx: machine.rbx,
            r10: machine.r10,
            r9: machine.r9,
            r8: machine.r8,
            rdx: machine.rdx,
            rsi: machine.rsi,
            rdi: machine.rdi,
        },
        rip: machine.rip,
        rsp: machine.rsp,
        rflags: (saved.rflags & !LINUX_SIGNAL_RESTORABLE_RFLAGS)
            | (machine.rflags & LINUX_SIGNAL_RESTORABLE_RFLAGS),
        rax: machine.rax,
        fs_base: active_fs_base(),
    };
    restored.is_valid().then_some(LinuxRestoredSignalContext {
        user: restored,
        signal_mask: context.signal_mask & !LINUX_UNBLOCKABLE_SIGNALS,
        rcx: machine.rcx,
        r11: machine.r11,
    })
}

pub(super) fn linux_apply_delivered_signal_action(
    index: usize,
    signal_bit: u64,
    old_mask: u64,
    flags: u64,
) {
    let action_mask = X86_RUNTIME.ELF_PROBE_SIGNAL_ACTION_MASKS[index].load(Ordering::SeqCst);
    let automatic_mask = if flags & LINUX_SA_NODEFER == 0 {
        signal_bit
    } else {
        0
    };
    store_current_linux_signal_mask(old_mask | action_mask | automatic_mask);
    if flags & LINUX_SA_RESETHAND != 0 {
        X86_RUNTIME.ELF_PROBE_SIGNAL_HANDLERS[index].store(0, Ordering::SeqCst);
        X86_RUNTIME.ELF_PROBE_SIGNAL_FLAGS[index].store(0, Ordering::SeqCst);
        X86_RUNTIME.ELF_PROBE_SIGNAL_RESTORERS[index].store(0, Ordering::SeqCst);
        X86_RUNTIME.ELF_PROBE_SIGNAL_ACTION_MASKS[index].store(0, Ordering::SeqCst);
    }
}

pub(super) fn linux_default_signal_terminates(signal: u64) -> bool {
    matches!(signal, 1..=16 | 24..=27 | 29..=64)
}

pub(super) fn linux_default_signal_ignored(signal: u64) -> bool {
    matches!(signal, LINUX_SIGCHLD | 18 | 23 | 28)
}

pub(super) fn verify_linux_default_signal_dispositions() -> bool {
    (1..=LINUX_SIGNAL_COUNT as u64).all(|signal| {
        let terminates = matches!(signal, 1..=16 | 24..=27 | 29..=64);
        let ignored = matches!(signal, 17 | 18 | 23 | 28);
        linux_default_signal_terminates(signal) == terminates
            && linux_default_signal_ignored(signal) == ignored
            && !(linux_default_signal_terminates(signal) && linux_default_signal_ignored(signal))
    })
}

pub(super) fn linux_terminate_by_default_signal(signal: u64) -> u64 {
    if X86_RUNTIME.LINUX_RUNTIME_MODE.load(Ordering::SeqCst) && current_linux_process_pid() != 1 {
        let pid = current_linux_process_pid();
        if !clear_current_linux_process_signal_queues() {
            return LINUX_EIO;
        }
        X86_RUNTIME
            .LINUX_PROCESS_CHILD_STATUS
            .store((128 + signal.min(127)) as u8, Ordering::SeqCst);
        X86_RUNTIME
            .LINUX_PROCESS_CHILD_EXIT_PID
            .store(pid, Ordering::SeqCst);
        X86_RUNTIME
            .LINUX_PROCESS_CHILD_EXITED
            .store(true, Ordering::SeqCst);
        return LINUX_RETURN_TO_KERNEL;
    }
    if !record_live_linux_process_snapshot() {
        return LINUX_ESRCH;
    }
    if !linux_close_all_descriptors() {
        return LINUX_EIO;
    }
    if !crate::linux_socket::shutdown()
        || !linux_tmpfs::teardown()
        || !crate::linux_overlay::teardown()
    {
        return LINUX_EIO;
    }
    if !clear_current_linux_process_signal_queues() {
        return LINUX_EIO;
    }
    retire_all_linux_threads();
    X86_RUNTIME.ELF_PROBE_SIGNAL_MASK.store(0, Ordering::SeqCst);
    X86_RUNTIME
        .ELF_PROBE_PENDING_SIGNALS
        .store(0, Ordering::SeqCst);
    X86_RUNTIME
        .ELF_PROBE_EXEC_IGNORED_SIGNALS
        .store(0, Ordering::SeqCst);
    for index in 0..LINUX_SIGNAL_COUNT {
        X86_RUNTIME.ELF_PROBE_PENDING_SIGNAL_CODES[index].store(0, Ordering::SeqCst);
        X86_RUNTIME.ELF_PROBE_PENDING_SIGNAL_VALUES[index].store(0, Ordering::SeqCst);
        X86_RUNTIME.ELF_PROBE_SIGNAL_HANDLERS[index].store(0, Ordering::SeqCst);
        X86_RUNTIME.ELF_PROBE_SIGNAL_FLAGS[index].store(0, Ordering::SeqCst);
        X86_RUNTIME.ELF_PROBE_SIGNAL_RESTORERS[index].store(0, Ordering::SeqCst);
        X86_RUNTIME.ELF_PROBE_SIGNAL_ACTION_MASKS[index].store(0, Ordering::SeqCst);
    }
    X86_RUNTIME
        .ELF_PROBE_ALT_STACK_POINTER
        .store(0, Ordering::SeqCst);
    X86_RUNTIME
        .ELF_PROBE_ALT_STACK_SIZE
        .store(0, Ordering::SeqCst);
    X86_RUNTIME
        .ELF_PROBE_ALT_STACK_FLAGS
        .store(LINUX_SS_DISABLE, Ordering::SeqCst);
    X86_RUNTIME
        .ELF_PROBE_TERMINATION_SIGNAL
        .store(signal as u32, Ordering::SeqCst);
    X86_RUNTIME
        .ELF_PROBE_COMPLETED
        .store(true, Ordering::SeqCst);
    X86_RUNTIME.ELF_PROBE_ACTIVE.store(false, Ordering::SeqCst);
    LINUX_RETURN_TO_KERNEL
}

pub(super) fn linux_signal_targets(
    selector: i32,
) -> Result<([u32; linux_process::PROCESS_LIMIT], usize), u64> {
    let current = current_linux_process_pid();
    let group = match selector {
        0 => linux_process::live_process_group(current),
        value if value < -1 => Some(value.unsigned_abs()),
        _ => None,
    };
    let mut targets = [0_u32; linux_process::PROCESS_LIMIT];
    let mut count = 0;
    for slot in 0..linux_process::PROCESS_LIMIT {
        let Some(pid) = linux_process::live_process_pid_at(slot) else {
            continue;
        };
        let selected = match selector {
            value if value > 0 => pid == value as u32,
            0 => group.is_some_and(|group| linux_process::live_process_group(pid) == Some(group)),
            -1 => pid != 1,
            _ => group.is_some_and(|group| linux_process::live_process_group(pid) == Some(group)),
        };
        if selected {
            targets[count] = pid;
            count += 1;
        }
    }
    (count != 0).then_some((targets, count)).ok_or(LINUX_ESRCH)
}

pub(super) fn linux_signal_permitted(target: u32, signal: u64) -> bool {
    if target == current_linux_process_pid() {
        return true;
    }
    if signal == LINUX_SIGCONT
        && linux_process::live_process_session(target)
            == linux_process::live_process_session(current_linux_process_pid())
    {
        return true;
    }
    let Some(caller_namespace) = current_linux_user_namespace() else {
        return false;
    };
    let Some(target_namespace) = linux_process::live_process_user_namespace(target) else {
        return false;
    };
    if !linux_user::may_signal(caller_namespace, target_namespace) {
        return false;
    }
    let (Some(caller), Some(target)) = (
        linux_user::credentials(caller_namespace),
        linux_user::credentials(target_namespace),
    ) else {
        return false;
    };
    [caller.uid, caller.euid]
        .into_iter()
        .any(|sender| sender == target.uid || sender == target.suid)
}

pub(super) fn linux_target_signal_ignored(target: u32, signal: u64) -> bool {
    let index = signal as usize - 1;
    let (handler, mask) = if target == current_linux_process_pid() {
        (
            X86_RUNTIME.ELF_PROBE_SIGNAL_HANDLERS[index].load(Ordering::SeqCst),
            X86_RUNTIME.ELF_PROBE_SIGNAL_MASK.load(Ordering::SeqCst),
        )
    } else {
        linux_process_runtime_state(target)
            .filter(|state| state.valid)
            .map_or((0, 0), |state| {
                (state.signal_handlers[index], state.thread_signal_mask)
            })
    };
    handler == 1
        || handler == 0
            && signal != LINUX_SIGCHLD
            && linux_default_signal_ignored(signal)
            && mask & (1 << index) == 0
}

pub(super) fn linux_queue_process_signal(
    target: u32,
    signal: u64,
    signal_code: i32,
    signal_value: Option<u64>,
) -> u64 {
    if linux_target_signal_ignored(target, signal)
        || linux_process::live_process_status(target)
            .is_some_and(|status| status.state == linux_process::LiveProcessState::Zombie)
    {
        return 0;
    }
    let Some(process) = linux_process::live_process_slot(target) else {
        return LINUX_ESRCH;
    };
    let (sender_pid, sender_uid) = linux_signal_sender();
    match crate::linux_signal::enqueue(
        linux_signal_instance(),
        crate::linux_signal::Scope::Process,
        process,
        crate::linux_signal::Pending {
            signal: signal as u8,
            code: signal_code,
            value: signal_value.unwrap_or(0),
            sender_pid,
            sender_uid,
        },
        signal >= 32,
    ) {
        Ok(_) => 0,
        Err(crate::linux_signal::Error::NoMemory) => LINUX_EAGAIN,
        Err(crate::linux_signal::Error::Invalid) => LINUX_EINVAL,
    }
}

pub(super) fn linux_kill(
    pid: u64,
    generated: (u64, i32, Option<u64>),
    output: *mut LinuxSyscallFrame,
    interrupted: LinuxUserContext,
    return_registers: (u64, u64),
) -> u64 {
    let (signal, signal_code, signal_value) = generated;
    let signal = u64::from(signal as u32);
    if !(1..=LINUX_SIGNAL_COUNT as u64).contains(&signal) && signal != 0 {
        return LINUX_EINVAL;
    }
    let (targets, count) = match linux_signal_targets(pid as u32 as i32) {
        Ok(targets) => targets,
        Err(error) => return error,
    };
    let mut permitted = false;
    let mut current = false;
    for target in targets[..count].iter().copied() {
        if !linux_signal_permitted(target, signal) {
            continue;
        }
        permitted = true;
        if signal == 0 {
            continue;
        }
        if target == current_linux_process_pid() {
            current = true;
            continue;
        }
        let result = linux_queue_process_signal(target, signal, signal_code, signal_value);
        if result != 0 {
            return result;
        }
    }
    if !permitted {
        return LINUX_EPERM;
    }
    if signal == 0 || !current {
        return 0;
    }
    linux_kill_current(
        (signal, signal_code, signal_value),
        Some(linux_signal_sender()),
        output,
        interrupted,
        return_registers,
    )
}

pub(super) fn linux_kill_current(
    generated: (u64, i32, Option<u64>),
    sender: Option<(u32, u32)>,
    output: *mut LinuxSyscallFrame,
    interrupted: LinuxUserContext,
    return_registers: (u64, u64),
) -> u64 {
    let (signal, signal_code, signal_value) = generated;
    let index = signal as usize - 1;
    let signal_bit = 1 << (signal - 1);
    if wake_linux_signal_wait_target(None, signal, signal_code, signal_value, sender) {
        return 0;
    }
    let handler = X86_RUNTIME.ELF_PROBE_SIGNAL_HANDLERS[index].load(Ordering::SeqCst);
    if handler == 1 || handler == 0 && linux_default_signal_ignored(signal) {
        return 0;
    }
    let old_mask = X86_RUNTIME.ELF_PROBE_SIGNAL_MASK.load(Ordering::SeqCst);
    if old_mask & signal_bit != 0 {
        if signal >= 32 {
            let Some(process) = linux_process::live_process_slot(current_linux_process_pid())
            else {
                return LINUX_ESRCH;
            };
            let (sender_pid, sender_uid) = linux_signal_sender();
            return match crate::linux_signal::enqueue(
                linux_signal_instance(),
                crate::linux_signal::Scope::Process,
                process,
                crate::linux_signal::Pending {
                    signal: signal as u8,
                    code: signal_code,
                    value: signal_value.unwrap_or(0),
                    sender_pid,
                    sender_uid,
                },
                true,
            ) {
                Ok(_) => 0,
                Err(crate::linux_signal::Error::NoMemory) => LINUX_EAGAIN,
                Err(crate::linux_signal::Error::Invalid) => LINUX_EINVAL,
            };
        }
        if X86_RUNTIME.ELF_PROBE_PENDING_SIGNALS.load(Ordering::SeqCst) & signal_bit == 0 {
            X86_RUNTIME.ELF_PROBE_PENDING_SIGNAL_CODES[index]
                .store(signal_code as u32, Ordering::SeqCst);
            X86_RUNTIME.ELF_PROBE_PENDING_SIGNAL_VALUES[index]
                .store(signal_value.unwrap_or(0), Ordering::SeqCst);
        }
        X86_RUNTIME
            .ELF_PROBE_PENDING_SIGNALS
            .fetch_or(signal_bit, Ordering::SeqCst);
        return 0;
    }
    if handler == 0 {
        return if linux_default_signal_terminates(signal)
            && !X86_RUNTIME.LINUX_SIGNAL_ACTIVE.load(Ordering::SeqCst)
        {
            linux_terminate_by_default_signal(signal)
        } else {
            // SIGSTOP, SIGTSTP, SIGTTIN, and SIGTTOU require the pending
            // scheduler-visible stopped-process model.
            LINUX_ENOSYS
        };
    }
    if output.is_null()
        || current_linux_thread_handle().is_none()
        || X86_RUNTIME.LINUX_SIGNAL_ACTIVE.load(Ordering::SeqCst)
    {
        return LINUX_ENOSYS;
    }
    let flags = X86_RUNTIME.ELF_PROBE_SIGNAL_FLAGS[index].load(Ordering::SeqCst);
    let restorer = X86_RUNTIME.ELF_PROBE_SIGNAL_RESTORERS[index].load(Ordering::SeqCst);
    if !linux_signal_action_deliverable(handler, flags, restorer) {
        return LINUX_ENOSYS;
    }
    let old_alt_stack_flags = X86_RUNTIME.ELF_PROBE_ALT_STACK_FLAGS.load(Ordering::SeqCst);
    let use_alt_stack = flags & LINUX_SA_ONSTACK != 0
        && old_alt_stack_flags & (LINUX_SS_DISABLE | LINUX_SS_ONSTACK) == 0;
    let stack_top = if use_alt_stack {
        let pointer = X86_RUNTIME
            .ELF_PROBE_ALT_STACK_POINTER
            .load(Ordering::SeqCst);
        let size = X86_RUNTIME.ELF_PROBE_ALT_STACK_SIZE.load(Ordering::SeqCst);
        let Some(top) = pointer.checked_add(size) else {
            return LINUX_EFAULT;
        };
        top
    } else {
        interrupted.rsp
    };
    let Some(signal_stack) = linux_signal_stack(stack_top, use_alt_stack) else {
        return LINUX_EFAULT;
    };
    let context = LinuxSignalContext::from_syscall(
        interrupted,
        return_registers,
        (signal_stack, restorer),
        old_mask,
        old_alt_stack_flags,
    );
    if !context.user_context().is_valid() {
        return LINUX_EINVAL;
    }
    let information = sender.map_or_else(
        || LinuxSignalInformation::queued(signal, signal_code, signal_value.unwrap_or(0)),
        |(pid, uid)| {
            LinuxSignalInformation::queued_from(
                signal,
                signal_code,
                signal_value.unwrap_or(0),
                pid,
                uid,
            )
        },
    );
    let signal_frame = linux_rt_signal_frame(context, flags, information, 0, 0, 0);
    if !copy_linux_signal_frame(signal_stack, &signal_frame) {
        return LINUX_EFAULT;
    }
    let mut handler_registers = interrupted.registers;
    handler_registers.rdi = signal;
    handler_registers.rsi = signal_stack + LINUX_SIGNAL_FRAME_INFO_OFFSET;
    handler_registers.rdx = signal_stack + LINUX_SIGNAL_FRAME_CONTEXT_OFFSET;
    let handler_context = LinuxUserContext {
        registers: handler_registers,
        rip: handler,
        rsp: signal_stack,
        rflags: interrupted.rflags & !((1 << 8) | (1 << 10) | (1 << 16)),
        rax: 0,
        fs_base: active_fs_base(),
    };
    if !handler_context.is_valid() || !write_linux_live_return(output, handler_context) {
        return LINUX_EINVAL;
    }
    // SAFETY: syscall entry is serialized with interrupts disabled until its
    // return context enters the handler.
    unsafe { X86_MUTABLE_RUNTIME.signal_context.get().write(context) };
    linux_apply_delivered_signal_action(index, signal_bit, old_mask, flags);
    if use_alt_stack {
        if old_alt_stack_flags & LINUX_SS_AUTODISARM != 0 {
            X86_RUNTIME
                .ELF_PROBE_ALT_STACK_POINTER
                .store(0, Ordering::SeqCst);
            X86_RUNTIME
                .ELF_PROBE_ALT_STACK_SIZE
                .store(0, Ordering::SeqCst);
            X86_RUNTIME
                .ELF_PROBE_ALT_STACK_FLAGS
                .store(LINUX_SS_DISABLE, Ordering::SeqCst);
        } else {
            X86_RUNTIME
                .ELF_PROBE_ALT_STACK_FLAGS
                .store(LINUX_SS_ONSTACK, Ordering::SeqCst);
        }
    }
    X86_RUNTIME
        .LINUX_SIGNAL_ACTIVE
        .store(true, Ordering::SeqCst);
    X86_RUNTIME
        .LINUX_SIGNAL_DELIVERY_COUNT
        .fetch_add(1, Ordering::SeqCst);
    0
}

pub(super) fn linux_interrupt_blocked_wait(
    target: LinuxThreadHandle,
    generated: (u64, i32),
    output: *mut LinuxSyscallFrame,
    sender_return: LinuxUserContext,
) -> u64 {
    linux_deliver_signal_to_blocked_wait(
        target,
        generated,
        output,
        sender_return,
        LINUX_EINTR,
        false,
    )
}

pub(super) fn linux_deliver_signal_to_blocked_wait(
    target: LinuxThreadHandle,
    generated: (u64, i32),
    output: *mut LinuxSyscallFrame,
    sender_return: LinuxUserContext,
    interrupted_result: u64,
    completed: bool,
) -> u64 {
    let (signal, signal_code) = generated;
    if signal == 0 {
        return 0;
    }
    if !(1..=LINUX_SIGNAL_COUNT as u64).contains(&signal) {
        return LINUX_EINVAL;
    }
    let Some(sender) = current_linux_thread_handle() else {
        return LINUX_ESRCH;
    };
    let Some(sender_tls) = linux_thread_context(sender) else {
        return LINUX_ESRCH;
    };
    let saved_sender = linux_thread_return_context(sender);
    let slot = usize::from(target.slot);
    let wait_kind = X86_RUNTIME.LINUX_THREAD_WAIT_KINDS[slot].load(Ordering::SeqCst);
    if sender == target
        || X86_RUNTIME.LINUX_THREAD_STATES[slot].load(Ordering::SeqCst) != LINUX_THREAD_BLOCKED
        || !matches!(
            wait_kind,
            LINUX_THREAD_WAIT_SIGNAL
                | LINUX_THREAD_WAIT_SLEEP
                | LINUX_THREAD_WAIT_POLL
                | LINUX_THREAD_WAIT_EPOLL
                | LINUX_THREAD_WAIT_EVENTFD_READ
                | LINUX_THREAD_WAIT_PIPE_READ
                | LINUX_THREAD_WAIT_TIMERFD_READ
                | LINUX_THREAD_WAIT_PIPE_WRITE
                | LINUX_THREAD_WAIT_EVENTFD_WRITE
                | LINUX_THREAD_WAIT_PAUSE
                | LINUX_THREAD_WAIT_SIGSUSPEND
                | LINUX_THREAD_WAIT_PPOLL
                | LINUX_THREAD_WAIT_EPOLL_PWAIT
                | LINUX_THREAD_WAIT_SELECT
                | LINUX_THREAD_WAIT_PSELECT
        )
        || output.is_null()
        || X86_RUNTIME.LINUX_SIGNAL_ACTIVE.load(Ordering::SeqCst)
    {
        return LINUX_ENOSYS;
    }
    let index = signal as usize - 1;
    let signal_bit = 1 << (signal - 1);
    let wait_mask = X86_RUNTIME.LINUX_THREAD_SIGNAL_MASKS[slot].load(Ordering::SeqCst);
    if wait_mask & signal_bit != 0
        || wait_kind == LINUX_THREAD_WAIT_SIGNAL
            && X86_RUNTIME.LINUX_THREAD_WAIT_KEYS[slot].load(Ordering::SeqCst) & signal_bit != 0
    {
        return LINUX_ENOSYS;
    }
    let handler = X86_RUNTIME.ELF_PROBE_SIGNAL_HANDLERS[index].load(Ordering::SeqCst);
    if handler == 1 {
        return 0;
    }
    let flags = X86_RUNTIME.ELF_PROBE_SIGNAL_FLAGS[index].load(Ordering::SeqCst);
    let restorer = X86_RUNTIME.ELF_PROBE_SIGNAL_RESTORERS[index].load(Ordering::SeqCst);
    if handler == 0 || !linux_signal_action_deliverable(handler, flags, restorer) {
        return LINUX_ENOSYS;
    }
    let Some(mut interrupted) = linux_thread_return_context(target) else {
        return LINUX_ESRCH;
    };
    let saved_interrupted = interrupted;
    interrupted.rax = interrupted_result;
    let masked_wait = matches!(
        wait_kind,
        LINUX_THREAD_WAIT_PPOLL
            | LINUX_THREAD_WAIT_EPOLL_PWAIT
            | LINUX_THREAD_WAIT_SIGSUSPEND
            | LINUX_THREAD_WAIT_PSELECT
    );
    let restored_mask = if masked_wait {
        X86_RUNTIME.LINUX_THREAD_WAIT_PRIOR_SIGNAL_MASKS[slot].load(Ordering::SeqCst)
    } else {
        wait_mask
    };
    let old_alt_stack_flags = X86_RUNTIME.ELF_PROBE_ALT_STACK_FLAGS.load(Ordering::SeqCst);
    let use_alt_stack = flags & LINUX_SA_ONSTACK != 0
        && old_alt_stack_flags & (LINUX_SS_DISABLE | LINUX_SS_ONSTACK) == 0;
    let stack_top = if use_alt_stack {
        let pointer = X86_RUNTIME
            .ELF_PROBE_ALT_STACK_POINTER
            .load(Ordering::SeqCst);
        let size = X86_RUNTIME.ELF_PROBE_ALT_STACK_SIZE.load(Ordering::SeqCst);
        let Some(top) = pointer.checked_add(size) else {
            return LINUX_EFAULT;
        };
        top
    } else {
        interrupted.rsp
    };
    let Some(signal_stack) = linux_signal_stack(stack_top, use_alt_stack) else {
        return LINUX_EFAULT;
    };
    let target_stack_valid = if use_alt_stack {
        linux_writable_user_range_contains(signal_stack, LINUX_SIGNAL_FRAME_SIZE)
    } else {
        linux_range_contains(
            X86_RUNTIME.LINUX_THREAD_STACK_BASES[slot].load(Ordering::SeqCst),
            X86_RUNTIME.LINUX_THREAD_STACK_SIZES[slot].load(Ordering::SeqCst),
            signal_stack,
            LINUX_SIGNAL_FRAME_SIZE,
        )
    };
    if !target_stack_valid {
        return LINUX_EFAULT;
    }
    let context = LinuxSignalContext::from_syscall(
        interrupted,
        (
            X86_RUNTIME.LINUX_THREAD_RCXS[slot].load(Ordering::SeqCst),
            X86_RUNTIME.LINUX_THREAD_R11S[slot].load(Ordering::SeqCst),
        ),
        (signal_stack, restorer),
        restored_mask,
        old_alt_stack_flags,
    );
    let signal_frame = linux_rt_signal_frame(
        context,
        flags,
        LinuxSignalInformation::generated(signal, signal_code),
        0,
        0,
        0,
    );
    let mut handler_registers = interrupted.registers;
    handler_registers.rdi = signal;
    handler_registers.rsi = signal_stack + LINUX_SIGNAL_FRAME_INFO_OFFSET;
    handler_registers.rdx = signal_stack + LINUX_SIGNAL_FRAME_CONTEXT_OFFSET;
    let handler_context = LinuxUserContext {
        registers: handler_registers,
        rip: handler,
        rsp: signal_stack,
        rflags: interrupted.rflags & !((1 << 8) | (1 << 10) | (1 << 16)),
        rax: 0,
        fs_base: interrupted.fs_base,
    };
    if !context.user_context().is_valid() || !handler_context.is_valid() {
        return LINUX_EINVAL;
    }
    let timeout_remaining = if wait_kind == LINUX_THREAD_WAIT_SLEEP {
        let address = X86_RUNTIME.LINUX_THREAD_WAIT_OUTPUTS[slot].load(Ordering::SeqCst);
        if address == 0 {
            None
        } else {
            let Some(now) = linux_elapsed_nanoseconds() else {
                return LINUX_ENOSYS;
            };
            let deadline = X86_RUNTIME.LINUX_THREAD_WAIT_DEADLINES[slot].load(Ordering::SeqCst);
            let mut replaced = [0_u8; 16];
            if deadline == 0 || !copy_from_linux_user(address, &mut replaced) {
                return LINUX_EFAULT;
            }
            let mut remaining = [0_u8; 16];
            linux_store_timespec(&mut remaining, deadline.saturating_sub(now));
            Some((address, replaced, remaining))
        }
    } else if wait_kind == LINUX_THREAD_WAIT_SELECT {
        let descriptor_count = X86_RUNTIME.LINUX_THREAD_WAIT_OUTPUTS[slot].load(Ordering::SeqCst);
        let snapshot = load_linux_select_snapshot(slot, descriptor_count);
        let address = snapshot.input.timeout_address;
        if address == 0 {
            None
        } else {
            let Some(now) = linux_elapsed_nanoseconds() else {
                return LINUX_ENOSYS;
            };
            let deadline = X86_RUNTIME.LINUX_THREAD_WAIT_DEADLINES[slot].load(Ordering::SeqCst);
            let mut replaced = [0_u8; 16];
            if deadline == 0 || !copy_from_linux_user(address, &mut replaced) {
                return LINUX_EFAULT;
            }
            let remaining_nanoseconds = deadline.saturating_sub(now);
            let mut remaining = [0_u8; 16];
            remaining[..8]
                .copy_from_slice(&(remaining_nanoseconds / NANOSECONDS_PER_SECOND).to_ne_bytes());
            remaining[8..].copy_from_slice(
                &(remaining_nanoseconds % NANOSECONDS_PER_SECOND / 1_000).to_ne_bytes(),
            );
            Some((address, replaced, remaining))
        }
    } else {
        None
    };
    let mut replaced = [0_u8; LINUX_SIGNAL_FRAME_SIZE];
    if !copy_from_linux_user(signal_stack, &mut replaced) {
        return LINUX_EFAULT;
    }
    if let Some((address, _, remaining)) = &timeout_remaining {
        if !copy_to_linux_user(*address, remaining) {
            return LINUX_EFAULT;
        }
    }
    if !copy_linux_signal_frame(signal_stack, &signal_frame) {
        let _ = copy_to_linux_user(signal_stack, &replaced);
        if let Some((address, original, _)) = &timeout_remaining {
            let _ = copy_to_linux_user(*address, original);
        }
        return LINUX_EFAULT;
    }
    if !save_linux_thread_return_context(sender, sender_return) {
        let _ = copy_to_linux_user(signal_stack, &replaced);
        if let Some((address, original, _)) = &timeout_remaining {
            let _ = copy_to_linux_user(*address, original);
        }
        return LINUX_EINVAL;
    }
    if !save_linux_thread_return_context(target, interrupted) {
        restore_linux_thread_return_context(sender, saved_sender);
        let _ = copy_to_linux_user(signal_stack, &replaced);
        if let Some((address, original, _)) = &timeout_remaining {
            let _ = copy_to_linux_user(*address, original);
        }
        return LINUX_EINVAL;
    }
    if !write_linux_live_return(output, handler_context) {
        let _ = save_linux_thread_return_context(target, saved_interrupted);
        restore_linux_thread_return_context(sender, saved_sender);
        let _ = copy_to_linux_user(signal_stack, &replaced);
        if let Some((address, original, _)) = &timeout_remaining {
            let _ = copy_to_linux_user(*address, original);
        }
        return LINUX_EINVAL;
    }
    X86_RUNTIME.LINUX_THREAD_SIGNAL_MASKS[slot].store(restored_mask, Ordering::SeqCst);
    X86_RUNTIME.LINUX_THREAD_STATES[slot].store(LINUX_THREAD_READY, Ordering::SeqCst);
    if !switch_linux_thread(target) {
        let _ = save_linux_thread_return_context(target, saved_interrupted);
        restore_linux_thread_return_context(sender, saved_sender);
        X86_RUNTIME.LINUX_THREAD_SIGNAL_MASKS[slot].store(wait_mask, Ordering::SeqCst);
        X86_RUNTIME.LINUX_THREAD_STATES[slot].store(LINUX_THREAD_BLOCKED, Ordering::SeqCst);
        let _ = install_linux_tls_context(sender_tls);
        let _ = write_linux_live_return(output, sender_return);
        let _ = copy_to_linux_user(signal_stack, &replaced);
        if let Some((address, original, _)) = &timeout_remaining {
            let _ = copy_to_linux_user(*address, original);
        }
        return LINUX_EINVAL;
    }
    X86_RUNTIME.LINUX_THREAD_WAIT_KINDS[slot].store(LINUX_THREAD_WAIT_NONE, Ordering::SeqCst);
    X86_RUNTIME.LINUX_THREAD_WAIT_KEYS[slot].store(0, Ordering::SeqCst);
    X86_RUNTIME.LINUX_THREAD_WAIT_OUTPUTS[slot].store(0, Ordering::SeqCst);
    X86_RUNTIME.LINUX_THREAD_WAIT_DEADLINES[slot].store(0, Ordering::SeqCst);
    if masked_wait {
        X86_RUNTIME.LINUX_THREAD_WAIT_PRIOR_SIGNAL_MASKS[slot].store(0, Ordering::SeqCst);
    }
    if matches!(wait_kind, LINUX_THREAD_WAIT_POLL | LINUX_THREAD_WAIT_PPOLL) {
        clear_linux_poll_snapshot(slot);
    }
    if matches!(
        wait_kind,
        LINUX_THREAD_WAIT_SELECT | LINUX_THREAD_WAIT_PSELECT
    ) {
        clear_linux_select_snapshot(slot);
    }
    if matches!(
        wait_kind,
        LINUX_THREAD_WAIT_PIPE_READ | LINUX_THREAD_WAIT_PIPE_WRITE
    ) {
        clear_linux_iov_snapshot(slot);
    }
    // SAFETY: the switch above made the target the sole running Linux thread,
    // and the live return already names its validated handler context.
    unsafe { X86_MUTABLE_RUNTIME.signal_context.get().write(context) };
    linux_apply_delivered_signal_action(index, signal_bit, restored_mask, flags);
    if use_alt_stack {
        if old_alt_stack_flags & LINUX_SS_AUTODISARM != 0 {
            X86_RUNTIME
                .ELF_PROBE_ALT_STACK_POINTER
                .store(0, Ordering::SeqCst);
            X86_RUNTIME
                .ELF_PROBE_ALT_STACK_SIZE
                .store(0, Ordering::SeqCst);
            X86_RUNTIME
                .ELF_PROBE_ALT_STACK_FLAGS
                .store(LINUX_SS_DISABLE, Ordering::SeqCst);
        } else {
            X86_RUNTIME
                .ELF_PROBE_ALT_STACK_FLAGS
                .store(LINUX_SS_ONSTACK, Ordering::SeqCst);
        }
    }
    X86_RUNTIME
        .LINUX_SIGNAL_ACTIVE
        .store(true, Ordering::SeqCst);
    X86_RUNTIME
        .LINUX_SIGNAL_DELIVERY_COUNT
        .fetch_add(1, Ordering::SeqCst);
    if completed && wait_kind == LINUX_THREAD_WAIT_PIPE_WRITE {
        X86_RUNTIME
            .LINUX_PIPE_WRITE_WAKE_COUNT
            .fetch_add(1, Ordering::SeqCst);
    } else if wait_kind == LINUX_THREAD_WAIT_SIGNAL {
        X86_RUNTIME
            .LINUX_SIGNAL_WAIT_INTERRUPT_COUNT
            .fetch_add(1, Ordering::SeqCst);
    } else if wait_kind == LINUX_THREAD_WAIT_SLEEP {
        X86_RUNTIME
            .LINUX_SLEEP_INTERRUPT_COUNT
            .fetch_add(1, Ordering::SeqCst);
    } else if wait_kind == LINUX_THREAD_WAIT_POLL {
        X86_RUNTIME
            .LINUX_POLL_WAIT_INTERRUPT_COUNT
            .fetch_add(1, Ordering::SeqCst);
    } else if wait_kind == LINUX_THREAD_WAIT_EPOLL {
        X86_RUNTIME
            .LINUX_EPOLL_WAIT_INTERRUPT_COUNT
            .fetch_add(1, Ordering::SeqCst);
    } else if wait_kind == LINUX_THREAD_WAIT_EVENTFD_READ {
        X86_RUNTIME
            .LINUX_EVENTFD_READ_INTERRUPT_COUNT
            .fetch_add(1, Ordering::SeqCst);
    } else if wait_kind == LINUX_THREAD_WAIT_PIPE_READ {
        X86_RUNTIME
            .LINUX_PIPE_READ_INTERRUPT_COUNT
            .fetch_add(1, Ordering::SeqCst);
    } else if wait_kind == LINUX_THREAD_WAIT_PIPE_WRITE {
        X86_RUNTIME
            .LINUX_PIPE_WRITE_INTERRUPT_COUNT
            .fetch_add(1, Ordering::SeqCst);
    } else if wait_kind == LINUX_THREAD_WAIT_EVENTFD_WRITE {
        X86_RUNTIME
            .LINUX_EVENTFD_WRITE_INTERRUPT_COUNT
            .fetch_add(1, Ordering::SeqCst);
    } else if wait_kind == LINUX_THREAD_WAIT_TIMERFD_READ {
        X86_RUNTIME
            .LINUX_TIMERFD_READ_INTERRUPT_COUNT
            .fetch_add(1, Ordering::SeqCst);
    } else {
        X86_RUNTIME
            .LINUX_MASKED_WAIT_INTERRUPT_COUNT
            .fetch_add(1, Ordering::SeqCst);
    }
    0
}

pub(super) fn linux_tgkill_identity(
    thread_group: u64,
    thread: u64,
) -> Result<LinuxThreadHandle, u64> {
    let thread_group = thread_group as i32;
    let thread = thread as i32;
    if thread_group <= 0 || thread <= 0 {
        return Err(LINUX_EINVAL);
    }
    if thread_group as u32 != current_linux_process_pid() {
        return Err(LINUX_ESRCH);
    }
    linux_thread_handle_by_tid(thread as u64).ok_or(LINUX_ESRCH)
}

pub(super) fn linux_tkill_identity(thread: u64) -> Result<LinuxThreadHandle, u64> {
    let thread = thread as i32;
    if thread <= 0 {
        return Err(LINUX_EINVAL);
    }
    linux_thread_handle_by_tid(thread as u64).ok_or(LINUX_ESRCH)
}

pub(super) fn linux_saved_thread_target(pid: u32, tid: u32) -> Option<LinuxThreadSignalTarget> {
    if pid == current_linux_process_pid()
        || linux_process::live_process_status(pid)?.state
            != linux_process::LiveProcessState::Running
    {
        return None;
    }
    let process_slot = linux_process::live_process_slot(pid)?;
    let state = linux_process_runtime_state(pid)?;
    let thread_slot = usize::from(state.thread_slot);
    (state.valid && state.tid == tid && thread_slot < LINUX_THREAD_LIMIT).then_some(
        LinuxThreadSignalTarget::Saved {
            pid,
            process_slot,
            thread_slot,
        },
    )
}

pub(super) fn linux_tgkill_target(
    thread_group: u64,
    thread: u64,
) -> Result<LinuxThreadSignalTarget, u64> {
    let thread_group = thread_group as i32;
    let thread = thread as i32;
    if thread_group <= 0 || thread <= 0 {
        return Err(LINUX_EINVAL);
    }
    let pid = thread_group as u32;
    let tid = thread as u32;
    if pid == current_linux_process_pid() {
        return linux_thread_handle_by_tid(u64::from(tid))
            .map(LinuxThreadSignalTarget::Active)
            .ok_or(LINUX_ESRCH);
    }
    linux_saved_thread_target(pid, tid).ok_or(LINUX_ESRCH)
}

pub(super) fn linux_tkill_target(thread: u64) -> Result<LinuxThreadSignalTarget, u64> {
    let thread = thread as i32;
    if thread <= 0 {
        return Err(LINUX_EINVAL);
    }
    let tid = thread as u32;
    if let Some(target) = linux_thread_handle_by_tid(u64::from(tid)) {
        return Ok(LinuxThreadSignalTarget::Active(target));
    }
    (0..linux_process::PROCESS_LIMIT)
        .filter_map(linux_process::live_process_pid_at)
        .find_map(|pid| linux_saved_thread_target(pid, tid))
        .ok_or(LINUX_ESRCH)
}

pub(super) fn linux_queue_saved_thread_signal(
    pid: u32,
    process_slot: usize,
    thread_slot: usize,
    signal: u64,
    code: i32,
    value: Option<u64>,
) -> u64 {
    if linux_target_signal_ignored(pid, signal) {
        return 0;
    }
    let (sender_pid, sender_uid) = linux_signal_sender();
    match crate::linux_signal::enqueue(
        linux_signal_instance(),
        crate::linux_signal::Scope::Thread,
        linux_thread_signal_queue_slot(process_slot, thread_slot),
        crate::linux_signal::Pending {
            signal: signal as u8,
            code,
            value: value.unwrap_or(0),
            sender_pid,
            sender_uid,
        },
        signal >= 32,
    ) {
        Ok(_) => 0,
        Err(crate::linux_signal::Error::NoMemory) => LINUX_EAGAIN,
        Err(crate::linux_signal::Error::Invalid) => LINUX_EINVAL,
    }
}

pub(super) fn linux_send_resolved_thread_signal(
    target: LinuxThreadSignalTarget,
    signal: u64,
    code: i32,
    output: *mut LinuxSyscallFrame,
    interrupted: LinuxUserContext,
    return_registers: (u64, u64),
) -> u64 {
    let signal = u64::from(signal as u32);
    if !(1..=LINUX_SIGNAL_COUNT as u64).contains(&signal) && signal != 0 {
        return LINUX_EINVAL;
    }
    match target {
        LinuxThreadSignalTarget::Active(target) => {
            linux_send_thread_signal(target, signal, code, output, interrupted, return_registers)
        }
        LinuxThreadSignalTarget::Saved {
            pid,
            process_slot,
            thread_slot,
        } => {
            if !linux_signal_permitted(pid, signal) {
                return LINUX_EPERM;
            }
            if signal == 0 {
                return 0;
            }
            linux_queue_saved_thread_signal(pid, process_slot, thread_slot, signal, code, None)
        }
    }
}

pub(super) fn linux_tgkill(
    thread_group: u64,
    thread: u64,
    signal: u64,
    output: *mut LinuxSyscallFrame,
    interrupted: LinuxUserContext,
    return_registers: (u64, u64),
) -> u64 {
    let result = match linux_tgkill_target(thread_group, thread) {
        Ok(target) => linux_send_resolved_thread_signal(
            target,
            signal,
            LINUX_SI_TKILL,
            output,
            interrupted,
            return_registers,
        ),
        Err(error) => error,
    };
    if X86_RUNTIME.LINUX_RUNTIME_MODE.load(Ordering::SeqCst)
        && thread_group as u32 != current_linux_process_pid()
        && result != 0
    {
        serial_write("ginux: cross-process tgkill rejected pid/tid/signal/error ");
        serial_write_u64(u64::from(thread_group as u32));
        serial_write(" ");
        serial_write_u64(u64::from(thread as u32));
        serial_write(" ");
        serial_write_u64(u64::from(signal as u32));
        serial_write(" ");
        serial_write_u64(result);
        serial_write("\r\n");
    }
    result
}

pub(super) fn linux_tkill(
    thread: u64,
    signal: u64,
    output: *mut LinuxSyscallFrame,
    interrupted: LinuxUserContext,
    return_registers: (u64, u64),
) -> u64 {
    match linux_tkill_target(thread) {
        Ok(target) => linux_send_resolved_thread_signal(
            target,
            signal,
            LINUX_SI_TKILL,
            output,
            interrupted,
            return_registers,
        ),
        Err(error) => error,
    }
}

pub(super) fn linux_send_thread_signal(
    target: LinuxThreadHandle,
    signal: u64,
    code: i32,
    output: *mut LinuxSyscallFrame,
    interrupted: LinuxUserContext,
    return_registers: (u64, u64),
) -> u64 {
    let signal = u64::from(signal as u32);
    if signal == 0 {
        return 0;
    }
    if !(1..=LINUX_SIGNAL_COUNT as u64).contains(&signal) {
        return LINUX_EINVAL;
    }
    let handler = X86_RUNTIME.ELF_PROBE_SIGNAL_HANDLERS[signal as usize - 1].load(Ordering::SeqCst);
    if handler == 1 || handler == 0 && linux_default_signal_ignored(signal) {
        return 0;
    }
    let slot = usize::from(target.slot);
    let target_is_current = current_linux_thread_handle() == Some(target);
    if target_is_current
        && X86_RUNTIME.LINUX_THREAD_SIGNAL_MASKS[slot].load(Ordering::SeqCst) & (1 << (signal - 1))
            == 0
    {
        return linux_kill(
            u64::from(current_linux_process_pid()),
            (signal, code, None),
            output,
            interrupted,
            return_registers,
        );
    }
    if !target_is_current {
        if wake_linux_signal_wait_target(
            Some(target),
            signal,
            code,
            None,
            Some(linux_signal_sender()),
        ) {
            return 0;
        }
        let result = linux_interrupt_blocked_wait(target, (signal, code), output, interrupted);
        if result != LINUX_ENOSYS {
            return result;
        }
    }
    let Some(process) = linux_process::live_process_slot(current_linux_process_pid()) else {
        return LINUX_ESRCH;
    };
    let (sender_pid, sender_uid) = linux_signal_sender();
    match crate::linux_signal::enqueue(
        linux_signal_instance(),
        crate::linux_signal::Scope::Thread,
        linux_thread_signal_queue_slot(process, slot),
        crate::linux_signal::Pending {
            signal: signal as u8,
            code,
            value: 0,
            sender_pid,
            sender_uid,
        },
        signal >= 32,
    ) {
        Ok(_) => 0,
        Err(crate::linux_signal::Error::NoMemory) => LINUX_EAGAIN,
        Err(crate::linux_signal::Error::Invalid) => LINUX_EINVAL,
    }
}

pub(super) fn linux_deliver_pending_signal(
    output: *mut LinuxSyscallFrame,
    interrupted: LinuxUserContext,
    return_registers: (u64, u64),
) -> u64 {
    if X86_RUNTIME.LINUX_SIGNAL_ACTIVE.load(Ordering::SeqCst) {
        return 0;
    }
    let requested = !X86_RUNTIME.ELF_PROBE_SIGNAL_MASK.load(Ordering::SeqCst);
    let deliverable = X86_RUNTIME.ELF_PROBE_PENDING_SIGNALS.load(Ordering::SeqCst) & requested;
    let dynamic = linux_peek_dynamic_signal(requested);
    if deliverable == 0 && dynamic.is_none() {
        return 0;
    }
    let legacy_signal = (deliverable != 0).then(|| u64::from(deliverable.trailing_zeros()) + 1);
    let use_dynamic = dynamic.is_some_and(|(scope, pending)| {
        scope == crate::linux_signal::Scope::Thread
            || legacy_signal.is_none_or(|legacy| u64::from(pending.signal) <= legacy)
    });
    let dynamic = use_dynamic.then_some(dynamic).flatten();
    let signal = dynamic.map_or_else(
        || legacy_signal.unwrap_or(0),
        |(_, pending)| u64::from(pending.signal),
    );
    let index = signal as usize - 1;
    let signal_bit = 1 << (signal - 1);
    let code = dynamic.map_or_else(
        || X86_RUNTIME.ELF_PROBE_PENDING_SIGNAL_CODES[index].load(Ordering::SeqCst) as i32,
        |(_, pending)| pending.code,
    );
    let value = dynamic.map_or_else(
        || X86_RUNTIME.ELF_PROBE_PENDING_SIGNAL_VALUES[index].load(Ordering::SeqCst),
        |(_, pending)| pending.value,
    );
    let result = linux_kill_current(
        (signal, code, Some(value)),
        dynamic.map(|(_, pending)| (pending.sender_pid, pending.sender_uid)),
        output,
        interrupted,
        return_registers,
    );
    if result == 0 {
        if let Some((scope, _)) = dynamic {
            let _ = linux_dequeue_dynamic_signal(scope, signal_bit);
        } else {
            X86_RUNTIME
                .ELF_PROBE_PENDING_SIGNALS
                .fetch_and(!signal_bit, Ordering::SeqCst);
            X86_RUNTIME.ELF_PROBE_PENDING_SIGNAL_CODES[index].store(0, Ordering::SeqCst);
            X86_RUNTIME.ELF_PROBE_PENDING_SIGNAL_VALUES[index].store(0, Ordering::SeqCst);
        }
    }
    result
}

pub(super) fn deliver_linux_sigsegv(
    frame: &mut UserFaultFrame,
    fault: domain::FaultMetadata,
) -> bool {
    if !frame.is_page_fault()
        || current_linux_thread_handle().is_none()
        || X86_RUNTIME.LINUX_SIGNAL_ACTIVE.load(Ordering::SeqCst)
    {
        return false;
    }
    let signal_bit = 1 << (LINUX_SIGSEGV - 1);
    let old_mask = X86_RUNTIME.ELF_PROBE_SIGNAL_MASK.load(Ordering::SeqCst);
    if old_mask & signal_bit != 0 {
        return false;
    }
    let index = LINUX_SIGSEGV as usize - 1;
    let handler = X86_RUNTIME.ELF_PROBE_SIGNAL_HANDLERS[index].load(Ordering::SeqCst);
    let flags = X86_RUNTIME.ELF_PROBE_SIGNAL_FLAGS[index].load(Ordering::SeqCst);
    let restorer = X86_RUNTIME.ELF_PROBE_SIGNAL_RESTORERS[index].load(Ordering::SeqCst);
    if !linux_signal_action_deliverable(handler, flags, restorer) {
        return false;
    }
    let interrupted = frame.context();
    let old_alt_stack_flags = X86_RUNTIME.ELF_PROBE_ALT_STACK_FLAGS.load(Ordering::SeqCst);
    let use_alt_stack = flags & LINUX_SA_ONSTACK != 0
        && old_alt_stack_flags & (LINUX_SS_DISABLE | LINUX_SS_ONSTACK) == 0;
    let stack_top = if use_alt_stack {
        let pointer = X86_RUNTIME
            .ELF_PROBE_ALT_STACK_POINTER
            .load(Ordering::SeqCst);
        let size = X86_RUNTIME.ELF_PROBE_ALT_STACK_SIZE.load(Ordering::SeqCst);
        let Some(top) = pointer.checked_add(size) else {
            return false;
        };
        top
    } else {
        interrupted.rsp
    };
    let Some(signal_stack) = linux_signal_stack(stack_top, use_alt_stack) else {
        return false;
    };
    let context = LinuxSignalContext::from_fault(
        interrupted,
        signal_stack,
        restorer,
        old_mask,
        old_alt_stack_flags,
    );
    if !context.user_context().is_valid() {
        return false;
    }
    let signal_frame = linux_rt_signal_frame(
        context,
        flags,
        LinuxSignalInformation::fault(
            LINUX_SIGSEGV,
            if fault.error & 1 == 0 {
                LINUX_SEGV_MAPERR
            } else {
                LINUX_SEGV_ACCERR
            },
            fault.address,
        ),
        fault.vector,
        fault.error,
        fault.address,
    );
    if !copy_linux_signal_frame(signal_stack, &signal_frame) {
        return false;
    }
    if !frame.enter_signal_handler(
        handler,
        signal_stack,
        LINUX_SIGSEGV,
        (
            signal_stack + LINUX_SIGNAL_FRAME_INFO_OFFSET,
            signal_stack + LINUX_SIGNAL_FRAME_CONTEXT_OFFSET,
        ),
        USER_CODE_SELECTOR,
        USER_DATA_SELECTOR,
    ) {
        return false;
    }
    // SAFETY: interrupts are disabled from exception entry through IRETQ, and
    // only one bounded Linux signal frame may be active at a time.
    unsafe { X86_MUTABLE_RUNTIME.signal_context.get().write(context) };
    linux_apply_delivered_signal_action(index, signal_bit, old_mask, flags);
    if use_alt_stack {
        if old_alt_stack_flags & LINUX_SS_AUTODISARM != 0 {
            X86_RUNTIME
                .ELF_PROBE_ALT_STACK_POINTER
                .store(0, Ordering::SeqCst);
            X86_RUNTIME
                .ELF_PROBE_ALT_STACK_SIZE
                .store(0, Ordering::SeqCst);
            X86_RUNTIME
                .ELF_PROBE_ALT_STACK_FLAGS
                .store(LINUX_SS_DISABLE, Ordering::SeqCst);
        } else {
            X86_RUNTIME
                .ELF_PROBE_ALT_STACK_FLAGS
                .store(LINUX_SS_ONSTACK, Ordering::SeqCst);
        }
    }
    X86_RUNTIME
        .LINUX_SIGNAL_ACTIVE
        .store(true, Ordering::SeqCst);
    X86_RUNTIME
        .LINUX_SIGNAL_DELIVERY_COUNT
        .fetch_add(1, Ordering::SeqCst);
    true
}

pub(super) fn linux_rt_sigreturn(output: *mut LinuxSyscallFrame, user_stack: u64) -> u64 {
    if output.is_null() || !X86_RUNTIME.LINUX_SIGNAL_ACTIVE.load(Ordering::SeqCst) {
        return LINUX_EINVAL;
    }
    // SAFETY: signal delivery serializes this context while interrupts remain
    // disabled; active is cleared before another delivery can publish state.
    let context = unsafe { X86_MUTABLE_RUNTIME.signal_context.get().cast_const().read() };
    if X86_RUNTIME.LINUX_RUNTIME_MODE.load(Ordering::SeqCst) {
        serial_write("ginux: signal return pid/stack/expected/rip/rsp: ");
        serial_write_u64(u64::from(current_linux_process_pid()));
        serial_write("/");
        serial_write_u64(user_stack);
        serial_write("/");
        serial_write_u64(context.signal_stack.saturating_add(8));
        serial_write("/");
        serial_write_u64(context.rip);
        serial_write("/");
        serial_write_u64(context.rsp);
        serial_write("\r\n");
    }
    if context.signal_stack.checked_add(8) != Some(user_stack) {
        return LINUX_EINVAL;
    }
    let mut returned = linux_rt_signal_frame(context, 0, LinuxSignalInformation::ZERO, 0, 0, 0);
    if !copy_linux_signal_frame_from_user(context.signal_stack, &mut returned) {
        return LINUX_EINVAL;
    }
    let Some(restored) = linux_restored_signal_context(context, &returned) else {
        return LINUX_EINVAL;
    };
    if !write_linux_live_return(output, restored.user) {
        return LINUX_EINVAL;
    }
    // SAFETY: this serialized return consumes the sole active record before
    // checking whether another unblocked pending signal should replace it.
    unsafe {
        X86_MUTABLE_RUNTIME
            .signal_context
            .get()
            .write(LinuxSignalContext::ZERO)
    };
    store_current_linux_signal_mask(restored.signal_mask);
    X86_RUNTIME
        .ELF_PROBE_ALT_STACK_POINTER
        .store(context.alt_stack_pointer, Ordering::SeqCst);
    X86_RUNTIME
        .ELF_PROBE_ALT_STACK_SIZE
        .store(context.alt_stack_size, Ordering::SeqCst);
    X86_RUNTIME
        .ELF_PROBE_ALT_STACK_FLAGS
        .store(context.alt_stack_flags, Ordering::SeqCst);
    X86_RUNTIME
        .LINUX_SIGNAL_ACTIVE
        .store(false, Ordering::SeqCst);
    X86_RUNTIME
        .LINUX_SIGNAL_RETURN_COUNT
        .fetch_add(1, Ordering::SeqCst);
    let pending_result =
        linux_deliver_pending_signal(output, restored.user, (restored.rcx, restored.r11));
    if pending_result == 0 && X86_RUNTIME.LINUX_SIGNAL_ACTIVE.load(Ordering::SeqCst) {
        return 0;
    }
    // SAFETY: the dedicated IRETQ epilogue consumes these values before this
    // serialized syscall entry can be reused.
    unsafe {
        let entry_state = &mut *current_entry_state();
        entry_state.signal_rax = restored.user.rax;
        entry_state.signal_rcx = restored.rcx;
        entry_state.signal_r11 = restored.r11;
    }
    LINUX_RETURN_FROM_SIGNAL
}

pub(super) fn abort_linux_signal() {
    if X86_RUNTIME
        .LINUX_SIGNAL_ACTIVE
        .swap(false, Ordering::SeqCst)
    {
        // SAFETY: the active context is exclusively owned by this serialized
        // Linux execution while interrupts remain disabled.
        let context = unsafe { X86_MUTABLE_RUNTIME.signal_context.get().cast_const().read() };
        store_current_linux_signal_mask(context.signal_mask);
        X86_RUNTIME
            .ELF_PROBE_ALT_STACK_POINTER
            .store(context.alt_stack_pointer, Ordering::SeqCst);
        X86_RUNTIME
            .ELF_PROBE_ALT_STACK_SIZE
            .store(context.alt_stack_size, Ordering::SeqCst);
        X86_RUNTIME
            .ELF_PROBE_ALT_STACK_FLAGS
            .store(context.alt_stack_flags, Ordering::SeqCst);
        unsafe {
            X86_MUTABLE_RUNTIME
                .signal_context
                .get()
                .write(LinuxSignalContext::ZERO)
        };
    }
}
