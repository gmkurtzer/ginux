use core::arch::{asm, x86_64::__cpuid};
use core::cell::UnsafeCell;
use core::hint::spin_loop;
use core::marker::PhantomData;
use core::mem::ManuallyDrop;
use core::ops::{Deref, DerefMut};
use core::sync::atomic::{AtomicU16, AtomicU32, Ordering};

const INTERRUPT_FLAG: u64 = 1 << 9;
const CPU_SLOT_LIMIT: usize = 256;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct LockRank(u16);

impl LockRank {
    pub(crate) const PRIMITIVE_TEST_LOW: Self = Self(1);
    pub(crate) const PRIMITIVE_TEST_HIGH: Self = Self(2);
    pub(crate) const ARCHITECTURE: Self = Self(10);
    pub(crate) const PHYSICAL_MEMORY: Self = Self(20);
    pub(crate) const ADDRESS_SPACE: Self = Self(30);
    pub(crate) const SCHEDULER: Self = Self(40);
    pub(crate) const DOMAIN_PROCESS: Self = Self(50);
    pub(crate) const FILESYSTEM: Self = Self(60);
    pub(crate) const IPC_SIGNAL: Self = Self(70);
    pub(crate) const DEVICE_NETWORK: Self = Self(80);
    pub(crate) const DIAGNOSTICS: Self = Self(90);
}

static HELD_RANKS: [AtomicU16; CPU_SLOT_LIMIT] = [const { AtomicU16::new(0) }; CPU_SLOT_LIMIT];
static KERNEL_ENTRY_LOCK: KernelEntryLock = KernelEntryLock::new();

pub(crate) struct SpinLock<T> {
    next: AtomicU32,
    owner: AtomicU32,
    rank: LockRank,
    value: UnsafeCell<T>,
}

// SAFETY: the ticket lock grants one guard at a time and a guard exists only
// while the protected value is exclusively borrowed.
unsafe impl<T: Send> Sync for SpinLock<T> {}

impl<T> SpinLock<T> {
    pub(crate) const fn new(rank: LockRank, value: T) -> Self {
        Self {
            next: AtomicU32::new(0),
            owner: AtomicU32::new(0),
            rank,
            value: UnsafeCell::new(value),
        }
    }

    pub(crate) fn lock(&self) -> SpinLockGuard<'_, T> {
        let previous_rank = enter_rank(self.rank);
        let ticket = self.next.fetch_add(1, Ordering::Relaxed);
        while self.owner.load(Ordering::Acquire) != ticket {
            spin_loop();
        }
        SpinLockGuard {
            lock: self,
            previous_rank,
            _not_send: PhantomData,
        }
    }

    pub(crate) fn try_lock(&self) -> Option<SpinLockGuard<'_, T>> {
        let owner = self.owner.load(Ordering::Acquire);
        if self
            .next
            .compare_exchange(
                owner,
                owner.wrapping_add(1),
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_err()
        {
            return None;
        }
        let previous_rank = enter_rank(self.rank);
        Some(SpinLockGuard {
            lock: self,
            previous_rank,
            _not_send: PhantomData,
        })
    }
}

pub(crate) struct SpinLockGuard<'a, T> {
    lock: &'a SpinLock<T>,
    previous_rank: u16,
    _not_send: PhantomData<*mut ()>,
}

impl<T> Deref for SpinLockGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        // SAFETY: the guard owns the current ticket.
        unsafe { &*self.lock.value.get() }
    }
}

impl<T> DerefMut for SpinLockGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        // SAFETY: the guard owns the current ticket exclusively.
        unsafe { &mut *self.lock.value.get() }
    }
}

impl<T> Drop for SpinLockGuard<'_, T> {
    fn drop(&mut self) {
        leave_rank(self.lock.rank, self.previous_rank);
        self.lock.owner.fetch_add(1, Ordering::Release);
    }
}

pub(crate) struct IrqSpinLock<T>(SpinLock<T>);

unsafe impl<T: Send> Sync for IrqSpinLock<T> {}

impl<T> IrqSpinLock<T> {
    pub(crate) const fn new(rank: LockRank, value: T) -> Self {
        Self(SpinLock::new(rank, value))
    }

    pub(crate) fn lock(&self) -> IrqSpinLockGuard<'_, T> {
        let interrupt_state = disable_interrupts();
        let guard = self.0.lock();
        IrqSpinLockGuard {
            guard: ManuallyDrop::new(guard),
            interrupt_state,
        }
    }

    pub(crate) fn try_lock(&self) -> Option<IrqSpinLockGuard<'_, T>> {
        let interrupt_state = disable_interrupts();
        match self.0.try_lock() {
            Some(guard) => Some(IrqSpinLockGuard {
                guard: ManuallyDrop::new(guard),
                interrupt_state,
            }),
            None => {
                restore_interrupts(interrupt_state);
                None
            }
        }
    }
}

pub(crate) struct IrqSpinLockGuard<'a, T> {
    guard: ManuallyDrop<SpinLockGuard<'a, T>>,
    interrupt_state: InterruptState,
}

impl<T> Deref for IrqSpinLockGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.guard
    }
}

impl<T> DerefMut for IrqSpinLockGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.guard
    }
}

impl<T> Drop for IrqSpinLockGuard<'_, T> {
    fn drop(&mut self) {
        // Release the shared lock before reenabling local interrupts.
        unsafe { ManuallyDrop::drop(&mut self.guard) };
        restore_interrupts(self.interrupt_state);
    }
}

pub(crate) fn kernel_entry_lock() -> KernelEntryGuard {
    KERNEL_ENTRY_LOCK.lock()
}

struct KernelEntryLock {
    next: AtomicU32,
    owner: AtomicU32,
    cpu_owner: AtomicU16,
    depths: [AtomicU32; CPU_SLOT_LIMIT],
}

impl KernelEntryLock {
    const fn new() -> Self {
        Self {
            next: AtomicU32::new(0),
            owner: AtomicU32::new(0),
            cpu_owner: AtomicU16::new(0),
            depths: [const { AtomicU32::new(0) }; CPU_SLOT_LIMIT],
        }
    }

    fn lock(&'static self) -> KernelEntryGuard {
        let interrupt_state = disable_interrupts();
        let cpu = cpu_slot();
        let encoded_cpu = cpu as u16 + 1;
        if self.cpu_owner.load(Ordering::Acquire) == encoded_cpu {
            let previous = self.depths[cpu].fetch_add(1, Ordering::AcqRel);
            if previous == 0 {
                crate::kernel_fail("ginux: invalid recursive kernel-entry depth\r\n");
            }
            return KernelEntryGuard {
                lock: self,
                cpu,
                interrupt_state,
                outermost: false,
                previous_rank: 0,
            };
        }

        let previous_rank = enter_rank(LockRank::ARCHITECTURE);
        let ticket = self.next.fetch_add(1, Ordering::Relaxed);
        while self.owner.load(Ordering::Acquire) != ticket {
            spin_loop();
        }
        self.depths[cpu].store(1, Ordering::Release);
        self.cpu_owner.store(encoded_cpu, Ordering::Release);
        KernelEntryGuard {
            lock: self,
            cpu,
            interrupt_state,
            outermost: true,
            previous_rank,
        }
    }
}

pub(crate) struct KernelEntryGuard {
    lock: &'static KernelEntryLock,
    cpu: usize,
    interrupt_state: InterruptState,
    outermost: bool,
    previous_rank: u16,
}

impl Drop for KernelEntryGuard {
    fn drop(&mut self) {
        let previous = self.lock.depths[self.cpu].fetch_sub(1, Ordering::AcqRel);
        if previous == 0 || self.outermost != (previous == 1) {
            crate::kernel_fail("ginux: invalid kernel-entry release depth\r\n");
        }
        if self.outermost {
            self.lock.cpu_owner.store(0, Ordering::Release);
            leave_rank(LockRank::ARCHITECTURE, self.previous_rank);
            self.lock.owner.fetch_add(1, Ordering::Release);
        }
        restore_interrupts(self.interrupt_state);
    }
}

#[derive(Clone, Copy)]
struct InterruptState {
    enabled: bool,
}

fn disable_interrupts() -> InterruptState {
    let flags: u64;
    // SAFETY: reading RFLAGS and clearing IF are privileged, local-CPU
    // operations. The matching guard restores the prior IF state.
    unsafe {
        asm!("pushfq", "pop {}", out(reg) flags, options(preserves_flags));
        asm!("cli", options(nomem, nostack));
    }
    InterruptState {
        enabled: flags & INTERRUPT_FLAG != 0,
    }
}

fn restore_interrupts(state: InterruptState) {
    if state.enabled {
        // SAFETY: this restores the IF state captured by disable_interrupts
        // after the shared lock has been released.
        unsafe { asm!("sti", options(nomem, nostack)) };
    }
}

fn cpu_slot() -> usize {
    usize::from((__cpuid(1).ebx >> 24) as u8)
}

fn enter_rank(rank: LockRank) -> u16 {
    let held = &HELD_RANKS[cpu_slot()];
    let previous = held.load(Ordering::Acquire);
    if previous >= rank.0 {
        crate::kernel_fail("ginux: synchronization lock-order violation\r\n");
    }
    held.store(rank.0, Ordering::Release);
    previous
}

fn leave_rank(rank: LockRank, previous: u16) {
    let held = &HELD_RANKS[cpu_slot()];
    if held.load(Ordering::Acquire) != rank.0 {
        crate::kernel_fail("ginux: synchronization unlock-order violation\r\n");
    }
    held.store(previous, Ordering::Release);
}

pub(crate) fn verify_primitives() -> bool {
    static LOW: IrqSpinLock<u64> = IrqSpinLock::new(LockRank::PRIMITIVE_TEST_LOW, 0);
    static HIGH: IrqSpinLock<u64> = IrqSpinLock::new(LockRank::PRIMITIVE_TEST_HIGH, 0);
    static PLAIN: SpinLock<u64> = SpinLock::new(LockRank::ARCHITECTURE, 0);

    let ranks = [
        LockRank::ARCHITECTURE,
        LockRank::PHYSICAL_MEMORY,
        LockRank::ADDRESS_SPACE,
        LockRank::SCHEDULER,
        LockRank::DOMAIN_PROCESS,
        LockRank::FILESYSTEM,
        LockRank::IPC_SIGNAL,
        LockRank::DEVICE_NETWORK,
        LockRank::DIAGNOSTICS,
    ];
    if !ranks.windows(2).all(|pair| pair[0] < pair[1]) {
        return false;
    }

    {
        let mut low = LOW.lock();
        *low = 1;
        let mut high = HIGH.lock();
        *high = *low + 1;
    }
    {
        let Some(mut plain) = PLAIN.try_lock() else {
            return false;
        };
        *plain = 3;
    }
    let Some(low) = LOW.try_lock() else {
        return false;
    };
    *low == 1 && *HIGH.lock() == 2 && *PLAIN.lock() == 3
}
