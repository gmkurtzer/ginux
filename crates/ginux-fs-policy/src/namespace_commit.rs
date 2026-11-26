//! Per-instance filesystem namespace commit barrier.
//!
//! The policy resolver and the tracee's confined kernel syscall observe the
//! same namespace generation only while every namespace-changing operation is
//! excluded between them. Read leases may overlap (including blocking FIFO
//! opens); namespace and filesystem-context mutations are exclusive.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Lease {
    None,
    Read,
    Write,
}

impl Lease {
    pub const fn is_waiting(self) -> bool {
        !matches!(self, Self::None)
    }
}

#[derive(Clone, Copy)]
pub struct Barrier {
    writer: usize,
    readers: [usize; 128],
}

impl Default for Barrier {
    fn default() -> Self {
        Self::new()
    }
}

impl Barrier {
    pub const fn new() -> Self {
        Self {
            writer: 0,
            readers: [0; 128],
        }
    }

    pub fn can_acquire(&self, slot: usize, host_pid: usize, lease: Lease) -> bool {
        match lease {
            Lease::None => true,
            Lease::Read => self.writer == 0 || self.writer == host_pid,
            Lease::Write => {
                (self.writer == 0 || self.writer == host_pid)
                    && (self.readers.iter().all(|reader| *reader == 0)
                        || self.reader(slot) == host_pid)
            }
        }
    }

    pub fn acquire(&mut self, slot: usize, host_pid: usize, lease: Lease) -> bool {
        if !self.can_acquire(slot, host_pid, lease) {
            return false;
        }
        match lease {
            Lease::None => {}
            Lease::Read => self.readers[slot] = host_pid,
            Lease::Write => self.writer = host_pid,
        }
        true
    }

    pub fn release(&mut self, slot: usize, host_pid: usize, lease: Lease) -> bool {
        match lease {
            Lease::None => true,
            Lease::Read if self.reader(slot) == host_pid => {
                self.readers[slot] = 0;
                true
            }
            Lease::Write if self.writer == host_pid => {
                self.writer = 0;
                true
            }
            _ => false,
        }
    }

    fn reader(&self, slot: usize) -> usize {
        self.readers.get(slot).copied().unwrap_or(0)
    }
}

/// Namespace observations and opens cannot redirect another operation outside
/// the instance, so they share a read generation. Calls that can replace a
/// component, alter policy-relevant metadata, or change a shared cwd require
/// the exclusive generation. `fchdir` is descriptor-owned but changes the
/// same filesystem context.
pub fn lease_for(syscall: u64) -> Lease {
    if syscall == 81 {
        return Lease::Write;
    }
    let Some(operation) = crate::operation(syscall) else {
        return Lease::None;
    };
    if operation.owner != crate::Owner::FilesystemLane {
        return Lease::None;
    }
    match crate::operation_family(operation) {
        crate::OperationFamily::Observation | crate::OperationFamily::SafeOpen => Lease::Read,
        crate::OperationFamily::SinglePathMutation | crate::OperationFamily::TwoPathMutation => {
            Lease::Write
        }
        crate::OperationFamily::DescriptorIo | crate::OperationFamily::Special => Lease::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn readers_overlap_but_exchange_waits_for_commit() {
        let mut barrier = Barrier::new();
        assert!(barrier.acquire(0, 101, Lease::Read));
        assert!(barrier.acquire(1, 102, Lease::Read));
        assert!(!barrier.acquire(2, 103, Lease::Write));
        assert!(barrier.release(0, 101, Lease::Read));
        assert!(!barrier.acquire(2, 103, Lease::Write));
        assert!(barrier.release(1, 102, Lease::Read));
        assert!(barrier.acquire(2, 103, Lease::Write));
    }

    #[test]
    fn writer_excludes_resolution_until_syscall_exit() {
        let mut barrier = Barrier::new();
        assert!(barrier.acquire(0, 101, Lease::Write));
        assert!(!barrier.acquire(1, 102, Lease::Read));
        assert!(!barrier.acquire(2, 103, Lease::Write));
        assert!(barrier.release(0, 101, Lease::Write));
        assert!(barrier.acquire(1, 102, Lease::Read));
    }

    #[test]
    fn stale_owner_cannot_release_reused_slot() {
        let mut barrier = Barrier::new();
        assert!(barrier.acquire(7, 101, Lease::Write));
        assert!(!barrier.release(7, 202, Lease::Write));
        assert!(!barrier.acquire(8, 202, Lease::Read));
        assert!(barrier.release(7, 101, Lease::Write));
        assert!(barrier.acquire(8, 202, Lease::Read));
        assert!(!barrier.release(8, 303, Lease::Read));
        assert!(barrier.release(8, 202, Lease::Read));
    }

    #[test]
    fn operation_families_have_explicit_conflict_modes() {
        for operation in crate::OPERATIONS {
            if operation.owner == crate::Owner::FilesystemLane {
                assert_ne!(
                    lease_for(operation.syscall),
                    Lease::None,
                    "{}",
                    operation.name
                );
            }
        }
        assert_eq!(lease_for(81), Lease::Write);
        assert_eq!(lease_for(0), Lease::None);
    }
}
