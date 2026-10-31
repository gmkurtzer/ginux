use super::*;

pub(crate) fn linux_startup_random() -> Option<[u8; 16]> {
    if core::arch::x86_64::__cpuid(1).ecx & (1 << 30) == 0 {
        return None;
    }
    let mut output = [0_u8; 16];
    for chunk in output.as_chunks_mut::<8>().0 {
        let mut generated = None;
        for _ in 0..16 {
            let mut value: u64;
            let mut ready: u8;
            unsafe {
                asm!(
                    "rdrand {value}",
                    "setc {ready}",
                    value = out(reg) value,
                    ready = out(reg_byte) ready,
                    options(nostack, preserves_flags)
                );
            }
            if ready != 0 {
                generated = Some(value);
                break;
            }
        }
        chunk.copy_from_slice(&generated?.to_ne_bytes());
    }
    Some(output)
}

pub(crate) struct LinuxBootIdCell(pub(crate) UnsafeCell<[u8; 37]>);

unsafe impl Sync for LinuxBootIdCell {}

pub(crate) fn linux_boot_id() -> &'static [u8] {
    if X86_RUNTIME.LINUX_BOOT_ID_STATE.load(Ordering::Acquire) != 2
        && X86_RUNTIME
            .LINUX_BOOT_ID_STATE
            .compare_exchange(0, 1, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    {
        let mut random = linux_startup_random().unwrap_or([0x47; 16]);
        random[6] = random[6] & 0x0f | 0x40;
        random[8] = random[8] & 0x3f | 0x80;
        let output = unsafe { &mut *X86_RUNTIME.LINUX_BOOT_ID.0 .0.get() };
        let hex = b"0123456789abcdef";
        let mut target = 0;
        for (index, byte) in random.into_iter().enumerate() {
            if matches!(index, 4 | 6 | 8 | 10) {
                output[target] = b'-';
                target += 1;
            }
            output[target] = hex[usize::from(byte >> 4)];
            output[target + 1] = hex[usize::from(byte & 0x0f)];
            target += 2;
        }
        output[36] = b'\n';
        X86_RUNTIME.LINUX_BOOT_ID_STATE.store(2, Ordering::Release);
    }
    while X86_RUNTIME.LINUX_BOOT_ID_STATE.load(Ordering::Acquire) != 2 {
        core::hint::spin_loop();
    }
    unsafe { &*X86_RUNTIME.LINUX_BOOT_ID.0 .0.get() }
}

pub(crate) fn linux_getrandom(address: u64, length: u64, flags: u64) -> u64 {
    linux_getrandom_with_copy(address, length, flags, copy_to_linux_user)
}

pub(crate) fn linux_getrandom_with_copy(
    address: u64,
    length: u64,
    flags: u64,
    copy: fn(u64, &[u8]) -> bool,
) -> u64 {
    const LINUX_GRND_NONBLOCK: u64 = 1;
    const LINUX_GRND_RANDOM: u64 = 2;
    const LINUX_GRND_INSECURE: u64 = 4;
    const LINUX_GRND_SUPPORTED: u64 = LINUX_GRND_NONBLOCK | LINUX_GRND_RANDOM | LINUX_GRND_INSECURE;
    if flags & !LINUX_GRND_SUPPORTED != 0
        || flags & (LINUX_GRND_RANDOM | LINUX_GRND_INSECURE)
            == LINUX_GRND_RANDOM | LINUX_GRND_INSECURE
    {
        return LINUX_EINVAL;
    }
    let Ok(length) = usize::try_from(length) else {
        return LINUX_EFAULT;
    };
    let mut written = 0_usize;
    while written < length {
        let Some(random) = linux_startup_random() else {
            return if written == 0 {
                if flags & LINUX_GRND_NONBLOCK != 0 {
                    LINUX_EAGAIN
                } else {
                    LINUX_EIO
                }
            } else {
                written as u64
            };
        };
        let count = (length - written).min(random.len());
        let Some(destination) = address.checked_add(written as u64) else {
            return if written == 0 {
                LINUX_EFAULT
            } else {
                written as u64
            };
        };
        if !copy(destination, &random[..count]) {
            return if written == 0 {
                LINUX_EFAULT
            } else {
                written as u64
            };
        }
        written += count;
    }
    written as u64
}
