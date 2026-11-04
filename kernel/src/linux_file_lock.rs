//! Advisory file locks for the Linux personality: POSIX record locks, open
//! file description (OFD) locks and flock(2) locks, following fs/locks.c.
//!
//! The table is a fixed array and does no I/O, so it is also built as a host
//! test (`make check-file-lock`). Callers resolve descriptors to a `File` and
//! an `Owner` and decide how a blocked request waits.

/// Linux OFFSET_MAX: the end of a lock that extends to end of file.
pub(crate) const OFFSET_MAX: u64 = i64::MAX as u64;
pub(crate) const CAPACITY: usize = 256;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Kind {
    Read,
    Write,
}

/// POSIX locks belong to a process, OFD and flock locks to an open file
/// description. Process and Description locks share one list and conflict
/// with each other; flock locks are a separate list (fs/locks.c keeps
/// flc_posix and flc_flock apart).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Owner {
    Process(u32),
    Description(u64),
    Flock(u64),
}

impl Owner {
    const fn flock(self) -> bool {
        matches!(self, Self::Flock(_))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct File {
    pub(crate) instance: u16,
    pub(crate) device: u64,
    pub(crate) inode: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Lock {
    pub(crate) file: File,
    pub(crate) owner: Owner,
    pub(crate) kind: Kind,
    /// First and last byte, inclusive; `end == OFFSET_MAX` is "to EOF".
    pub(crate) start: u64,
    pub(crate) end: u64,
    /// The locking process, reported by F_GETLK for POSIX locks.
    pub(crate) pid: u32,
}

impl Lock {
    const fn overlaps(&self, start: u64, end: u64) -> bool {
        self.start <= end && start <= self.end
    }

    /// Overlapping or directly adjacent: same-kind locks of one owner merge.
    const fn touches(&self, start: u64, end: u64) -> bool {
        self.start <= end.saturating_add(1) && start <= self.end.saturating_add(1)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Error {
    /// EAGAIN, or a reason to wait: the lock that is in the way.
    Conflict(Lock),
    /// ENOLCK: the table cannot hold the result.
    NoSpace,
}

pub(crate) struct LockTable {
    locks: [Option<Lock>; CAPACITY],
}

impl LockTable {
    pub(crate) const fn new() -> Self {
        Self {
            locks: [None; CAPACITY],
        }
    }

    /// The conflicting lock with the lowest start, as F_GETLK reports it
    /// (posix_test_lock). `kind == None` is an OFD F_GETLK with F_UNLCK,
    /// which reports the caller's own overlapping lock instead.
    pub(crate) fn conflict(
        &self,
        file: File,
        owner: Owner,
        kind: Option<Kind>,
        start: u64,
        end: u64,
    ) -> Option<Lock> {
        self.locks
            .iter()
            .flatten()
            .filter(|lock| {
                lock.file == file
                    && lock.owner.flock() == owner.flock()
                    && lock.overlaps(start, end)
                    && match kind {
                        None => lock.owner == owner,
                        Some(kind) => {
                            lock.owner != owner && (kind == Kind::Write || lock.kind == Kind::Write)
                        }
                    }
            })
            .min_by_key(|lock| lock.start)
            .copied()
    }

    /// F_SETLK for POSIX and OFD locks (posix_lock_inode): `kind == None`
    /// unlocks. The owner's locks in the range are replaced, split at the
    /// edges, and same-kind neighbours are merged. Nothing changes on error.
    pub(crate) fn set_range(
        &mut self,
        file: File,
        owner: Owner,
        pid: u32,
        kind: Option<Kind>,
        start: u64,
        end: u64,
    ) -> Result<(), Error> {
        if kind.is_some() {
            if let Some(lock) = self.conflict(file, owner, kind, start, end) {
                return Err(Error::Conflict(lock));
            }
        }
        let own = |lock: &Lock| lock.file == file && lock.owner == owner;
        let merges = |lock: &Lock| Some(lock.kind) == kind && lock.touches(start, end);
        let (mut low, mut high) = (start, end);
        for lock in self
            .locks
            .iter()
            .flatten()
            .filter(|lock| own(lock) && merges(lock))
        {
            low = low.min(lock.start);
            high = high.max(lock.end);
        }
        // Own locks do not overlap each other, so at most one lock crosses
        // each edge of the new range and leaves a remainder.
        let mut remainders = [None; 2];
        let mut released = 0;
        for lock in self.locks.iter().flatten().filter(|lock| own(lock)) {
            if merges(lock) {
                released += 1;
            } else if lock.overlaps(low, high) {
                released += 1;
                if lock.start < low {
                    remainders[0] = Some(Lock {
                        end: low - 1,
                        ..*lock
                    });
                }
                if lock.end > high {
                    remainders[1] = Some(Lock {
                        start: high + 1,
                        ..*lock
                    });
                }
            }
        }
        let added = remainders.iter().flatten().count() + usize::from(kind.is_some());
        if added > self.free() + released {
            return Err(Error::NoSpace);
        }
        for slot in &mut self.locks {
            if slot.is_some_and(|lock| own(&lock) && (merges(&lock) || lock.overlaps(low, high))) {
                *slot = None;
            }
        }
        for lock in remainders.into_iter().flatten() {
            self.insert(lock);
        }
        if let Some(kind) = kind {
            self.insert(Lock {
                file,
                owner,
                kind,
                start: low,
                end: high,
                pid,
            });
        }
        Ok(())
    }

    /// flock(2) (flock_lock_inode): a description's existing lock of another
    /// kind is dropped before the conflict check, so a failed upgrade or
    /// downgrade leaves the description unlocked, as on Linux.
    pub(crate) fn set_whole(
        &mut self,
        file: File,
        owner: Owner,
        pid: u32,
        kind: Option<Kind>,
    ) -> Result<(), Error> {
        for slot in &mut self.locks {
            if let Some(lock) = slot.filter(|lock| lock.file == file && lock.owner == owner) {
                if Some(lock.kind) == kind {
                    return Ok(());
                }
                *slot = None;
            }
        }
        let Some(kind) = kind else {
            return Ok(());
        };
        if let Some(lock) = self.conflict(file, owner, Some(kind), 0, OFFSET_MAX) {
            return Err(Error::Conflict(lock));
        }
        if self.free() == 0 {
            return Err(Error::NoSpace);
        }
        self.insert(Lock {
            file,
            owner,
            kind,
            start: 0,
            end: OFFSET_MAX,
            pid,
        });
        Ok(())
    }

    pub(crate) fn lock_at(&self, index: usize) -> Option<Lock> {
        self.locks.get(index).copied().flatten()
    }

    /// Drop every lock for which `release` is true.
    pub(crate) fn release(&mut self, release: impl Fn(&Lock) -> bool) {
        for slot in &mut self.locks {
            if slot.is_some_and(|lock| release(&lock)) {
                *slot = None;
            }
        }
    }

    fn free(&self) -> usize {
        self.locks.iter().filter(|slot| slot.is_none()).count()
    }

    fn insert(&mut self, lock: Lock) {
        if let Some(slot) = self.locks.iter_mut().find(|slot| slot.is_none()) {
            *slot = Some(lock);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE: File = File {
        instance: 0,
        device: 1,
        inode: 7,
    };
    const A: Owner = Owner::Process(10);
    const B: Owner = Owner::Process(11);
    const R: Option<Kind> = Some(Kind::Read);
    const W: Option<Kind> = Some(Kind::Write);

    fn table() -> Box<LockTable> {
        Box::new(LockTable::new())
    }

    fn ranges(table: &LockTable, owner: Owner) -> Vec<(Kind, u64, u64)> {
        let mut found: Vec<_> = (0..CAPACITY)
            .filter_map(|index| table.lock_at(index))
            .filter(|lock| lock.owner == owner)
            .map(|lock| (lock.kind, lock.start, lock.end))
            .collect();
        found.sort_by_key(|range| range.1);
        found
    }

    fn conflict_start(result: Result<(), Error>) -> Option<u64> {
        match result {
            Err(Error::Conflict(lock)) => Some(lock.start),
            _ => None,
        }
    }

    #[test]
    fn overlap_conflicts_and_adjacent_does_not() {
        let mut t = table();
        t.set_range(FILE, A, 10, W, 0, 9).unwrap();
        assert_eq!(conflict_start(t.set_range(FILE, B, 11, W, 5, 14)), Some(0));
        assert_eq!(conflict_start(t.set_range(FILE, B, 11, R, 9, 9)), Some(0));
        assert_eq!(t.set_range(FILE, B, 11, W, 10, 19), Ok(()));
        let other = File { inode: 8, ..FILE };
        assert_eq!(t.set_range(other, B, 11, W, 0, 9), Ok(()));
    }

    #[test]
    fn read_locks_share() {
        let mut t = table();
        t.set_range(FILE, A, 10, R, 0, 9).unwrap();
        assert_eq!(t.set_range(FILE, B, 11, R, 0, 9), Ok(()));
        assert_eq!(conflict_start(t.set_range(FILE, B, 11, W, 0, 0)), Some(0));
    }

    #[test]
    fn unlock_middle_splits() {
        let mut t = table();
        t.set_range(FILE, A, 10, W, 0, 99).unwrap();
        t.set_range(FILE, A, 10, None, 40, 59).unwrap();
        assert_eq!(ranges(&t, A), [(Kind::Write, 0, 39), (Kind::Write, 60, 99)]);
        assert_eq!(t.set_range(FILE, B, 11, W, 40, 59), Ok(()));
    }

    #[test]
    fn same_kind_neighbours_merge() {
        let mut t = table();
        t.set_range(FILE, A, 10, W, 0, 9).unwrap();
        t.set_range(FILE, A, 10, W, 10, 19).unwrap();
        t.set_range(FILE, A, 10, W, 30, 39).unwrap();
        t.set_range(FILE, A, 10, W, 25, 34).unwrap();
        assert_eq!(ranges(&t, A), [(Kind::Write, 0, 19), (Kind::Write, 25, 39)]);
        t.set_range(FILE, A, 10, W, 20, 24).unwrap();
        assert_eq!(ranges(&t, A), [(Kind::Write, 0, 39)]);
    }

    #[test]
    fn other_kind_inside_splits_in_three() {
        let mut t = table();
        t.set_range(FILE, A, 10, W, 0, 19).unwrap();
        t.set_range(FILE, A, 10, R, 5, 9).unwrap();
        assert_eq!(
            ranges(&t, A),
            [
                (Kind::Write, 0, 4),
                (Kind::Read, 5, 9),
                (Kind::Write, 10, 19)
            ]
        );
        t.set_range(FILE, A, 10, W, 0, OFFSET_MAX).unwrap();
        assert_eq!(ranges(&t, A), [(Kind::Write, 0, OFFSET_MAX)]);
    }

    #[test]
    fn conflict_reports_lowest_start_and_unlock_query_reports_own() {
        let mut t = table();
        t.set_range(FILE, A, 10, W, 60, 69).unwrap();
        t.set_range(FILE, A, 10, R, 20, 29).unwrap();
        assert_eq!(
            t.conflict(FILE, B, W, 0, OFFSET_MAX).map(|l| l.start),
            Some(20)
        );
        assert_eq!(
            t.conflict(FILE, B, R, 0, OFFSET_MAX).map(|l| l.start),
            Some(60)
        );
        assert_eq!(t.conflict(FILE, A, W, 0, OFFSET_MAX), None);
        assert_eq!(t.conflict(FILE, A, None, 0, 25).map(|l| l.end), Some(29));
        assert_eq!(t.conflict(FILE, B, None, 0, 25), None);
    }

    #[test]
    fn description_and_process_locks_conflict_flock_is_separate() {
        let mut t = table();
        let ofd = Owner::Description(5);
        t.set_range(FILE, ofd, 10, W, 0, 9).unwrap();
        assert!(t.set_range(FILE, A, 10, W, 0, 9).is_err());
        assert!(t
            .set_range(FILE, Owner::Description(6), 10, W, 5, 5)
            .is_err());
        assert_eq!(t.set_whole(FILE, Owner::Flock(5), 10, W), Ok(()));
        assert!(t.set_whole(FILE, Owner::Flock(6), 10, R).is_err());
        t.release(|lock| lock.owner == ofd);
        assert_eq!(t.set_range(FILE, A, 10, W, 0, 9), Ok(()));
    }

    #[test]
    fn flock_conversion_drops_old_lock_first() {
        let mut t = table();
        let (a, b) = (Owner::Flock(1), Owner::Flock(2));
        t.set_whole(FILE, a, 10, R).unwrap();
        t.set_whole(FILE, b, 11, R).unwrap();
        assert!(t.set_whole(FILE, a, 10, W).is_err());
        assert_eq!(ranges(&t, a), []);
        assert_eq!(t.set_whole(FILE, b, 11, W), Ok(()));
        assert_eq!(t.set_whole(FILE, b, 11, W), Ok(()));
        assert_eq!(t.set_whole(FILE, b, 11, R), Ok(()));
        assert_eq!(ranges(&t, b), [(Kind::Read, 0, OFFSET_MAX)]);
        assert_eq!(t.set_whole(FILE, b, 11, None), Ok(()));
        assert_eq!(ranges(&t, b), []);
    }

    #[test]
    fn full_table_is_no_space_and_unchanged() {
        let mut t = table();
        for index in 0..CAPACITY as u64 {
            t.set_range(FILE, A, 10, W, index * 4, index * 4).unwrap();
        }
        assert_eq!(t.set_range(FILE, A, 10, W, 2000, 2000), Err(Error::NoSpace));
        // Merging 0 and 4 through 1..3 frees a slot; the second lock fills it.
        t.set_range(FILE, A, 10, W, 1, 3).unwrap();
        t.set_range(FILE, A, 10, W, 2000, 2000).unwrap();
        // Unlocking the middle of a lock needs one more slot.
        assert_eq!(t.set_range(FILE, A, 10, None, 1, 1), Err(Error::NoSpace));
        assert!(ranges(&t, A).contains(&(Kind::Write, 0, 4)));
        t.release(|lock| lock.start == 8);
        assert_eq!(t.set_range(FILE, A, 10, None, 1, 1), Ok(()));
        assert!(ranges(&t, A).contains(&(Kind::Write, 2, 4)));
    }
}
