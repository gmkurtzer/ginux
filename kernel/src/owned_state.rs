use core::cell::UnsafeCell;
use core::mem::MaybeUninit;
use core::sync::atomic::{AtomicU8, Ordering};

const EMPTY: u8 = 0;
const CLAIMED: u8 = 1;
const READY: u8 = 2;

/// Lifecycle owner for one long-lived kernel object.
///
/// This type owns initialization state and storage together so callers cannot
/// publish a readiness flag independently from the value it guards. It does
/// not provide mutual exclusion after publication; Gate 8 adds the locking
/// required when these roots become reachable from multiple CPUs.
pub(crate) struct OwnedState<T> {
    state: AtomicU8,
    value: UnsafeCell<MaybeUninit<T>>,
}

// SAFETY: OwnedState controls publication of its storage. Access after
// publication still relies on the subsystem's documented serialization rule;
// this wrapper intentionally does not claim to be a lock.
unsafe impl<T> Sync for OwnedState<T> {}

impl<T> OwnedState<T> {
    pub(crate) const fn empty() -> Self {
        Self {
            state: AtomicU8::new(EMPTY),
            value: UnsafeCell::new(MaybeUninit::uninit()),
        }
    }

    pub(crate) fn try_claim(&self) -> Option<OwnedStateClaim<'_, T>> {
        self.state
            .compare_exchange(EMPTY, CLAIMED, Ordering::AcqRel, Ordering::Acquire)
            .ok()
            .map(|_| OwnedStateClaim {
                owner: self,
                published: false,
            })
    }

    pub(crate) fn try_install(&self, value: T) -> Result<(), T> {
        let Some(claim) = self.try_claim() else {
            return Err(value);
        };
        claim.publish(value);
        Ok(())
    }

    pub(crate) fn get(&self) -> Option<&T> {
        if self.state.load(Ordering::Acquire) != READY {
            return None;
        }
        // SAFETY: READY is stored with release ordering only after the value
        // has been completely initialized.
        Some(unsafe { (&*self.value.get()).assume_init_ref() })
    }

    pub(crate) fn get_mut_ptr(&self) -> Option<*mut T> {
        if self.state.load(Ordering::Acquire) != READY {
            return None;
        }
        Some(self.value.get().cast::<T>())
    }

    pub(crate) fn reset(&self) -> bool {
        if self
            .state
            .compare_exchange(READY, CLAIMED, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return false;
        }
        // SAFETY: The READY -> CLAIMED transition gives the lifecycle owner
        // exclusive teardown access to one initialized value.
        unsafe { (&mut *self.value.get()).assume_init_drop() };
        self.state.store(EMPTY, Ordering::Release);
        true
    }
}

pub(crate) struct OwnedStateClaim<'a, T> {
    owner: &'a OwnedState<T>,
    published: bool,
}

impl<T> OwnedStateClaim<'_, T> {
    pub(crate) fn as_mut_ptr(&self) -> *mut T {
        self.owner.value.get().cast::<T>()
    }

    pub(crate) fn publish(mut self, value: T) {
        // SAFETY: A live claim is the sole initializer for this owner.
        unsafe { self.as_mut_ptr().write(value) };
        self.owner.state.store(READY, Ordering::Release);
        self.published = true;
    }

    pub(crate) fn publish_initialized(mut self) {
        self.owner.state.store(READY, Ordering::Release);
        self.published = true;
    }
}

impl<T> Drop for OwnedStateClaim<'_, T> {
    fn drop(&mut self) {
        if !self.published {
            self.owner.state.store(EMPTY, Ordering::Release);
        }
    }
}
