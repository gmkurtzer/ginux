const MAX_CAPABILITIES: usize = 64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Rights(u64);

impl Rights {
    pub(crate) const READ: Self = Self(1 << 0);
    pub(crate) const WRITE: Self = Self(1 << 1);
    pub(crate) const MANAGE: Self = Self(1 << 2);
    pub(crate) const EXECUTE: Self = Self(1 << 3);

    pub(crate) const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    const fn is_empty(self) -> bool {
        self.0 == 0
    }

    const fn contains(self, requested: Self) -> bool {
        self.0 & requested.0 == requested.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Handle {
    slot: u16,
    generation: u32,
}

#[derive(Clone, Copy)]
struct Entry {
    object: u64,
    rights: Rights,
    generation: u32,
    parent: Option<Handle>,
}

pub(crate) struct Table {
    entries: [Option<Entry>; MAX_CAPABILITIES],
    generations: [u32; MAX_CAPABILITIES],
    boot_authority_sealed: bool,
}

impl Table {
    pub(crate) const fn new() -> Self {
        Self {
            entries: [None; MAX_CAPABILITIES],
            generations: [0; MAX_CAPABILITIES],
            boot_authority_sealed: false,
        }
    }

    /// Introduces authority from the immutable boot envelope.
    ///
    /// This is the only operation that does not require a parent capability.
    pub(crate) fn authorize_boot(&mut self, object: u64, rights: Rights) -> Option<Handle> {
        if self.boot_authority_sealed || rights.is_empty() {
            return None;
        }
        self.insert(object, rights, None)
    }

    pub(crate) fn seal_boot_authority(&mut self) {
        self.boot_authority_sealed = true;
    }

    /// Produces strictly less authority over the same object.
    pub(crate) fn derive(&mut self, parent: Handle, rights: Rights) -> Option<Handle> {
        let parent_entry = self.entry(parent)?;
        if rights.is_empty()
            || rights == parent_entry.rights
            || !parent_entry.rights.contains(rights)
        {
            return None;
        }
        self.insert(parent_entry.object, rights, Some(parent))
    }

    pub(crate) fn permits(&self, handle: Handle, object: u64, rights: Rights) -> bool {
        !rights.is_empty()
            && self
                .entry(handle)
                .is_some_and(|entry| entry.object == object && entry.rights.contains(rights))
    }

    /// Revokes a capability and every capability derived from it.
    pub(crate) fn revoke(&mut self, root: Handle) -> bool {
        if self.entry(root).is_none() {
            return false;
        }

        let mut revoked = [false; MAX_CAPABILITIES];
        revoked[usize::from(root.slot)] = true;
        loop {
            let mut changed = false;
            for (slot, entry) in self.entries.iter().enumerate() {
                let Some(parent) = entry.and_then(|value| value.parent) else {
                    continue;
                };
                if revoked[usize::from(parent.slot)] && !revoked[slot] {
                    revoked[slot] = true;
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        for (slot, should_revoke) in revoked.iter().enumerate() {
            if *should_revoke {
                self.entries[slot] = None;
            }
        }
        true
    }

    fn insert(&mut self, object: u64, rights: Rights, parent: Option<Handle>) -> Option<Handle> {
        let slot = self.entries.iter().position(Option::is_none)?;
        let generation = self.generations[slot].checked_add(1)?;
        self.generations[slot] = generation;
        self.entries[slot] = Some(Entry {
            object,
            rights,
            generation,
            parent,
        });
        Some(Handle {
            slot: slot as u16,
            generation,
        })
    }

    fn entry(&self, handle: Handle) -> Option<Entry> {
        self.entries
            .get(usize::from(handle.slot))
            .copied()
            .flatten()
            .filter(|entry| entry.generation == handle.generation)
    }
}

pub(crate) fn verify_authority_invariants() -> bool {
    const OBJECT: u64 = 7;
    let root_rights = Rights::READ.union(Rights::WRITE).union(Rights::MANAGE);
    let mut table = Table::new();
    let Some(root) = table.authorize_boot(OBJECT, root_rights) else {
        return false;
    };
    let Some(second_root) = table.authorize_boot(8, Rights::READ.union(Rights::WRITE)) else {
        return false;
    };
    table.seal_boot_authority();
    if table.authorize_boot(9, Rights::READ).is_some() {
        return false;
    }
    if table.derive(root, root_rights).is_some()
        || table
            .derive(root, Rights::READ.union(Rights::EXECUTE))
            .is_some()
    {
        return false;
    }
    let Some(read_only) = table.derive(root, Rights::READ) else {
        return false;
    };
    if !table.permits(read_only, OBJECT, Rights::READ)
        || table.permits(read_only, OBJECT, Rights::WRITE)
        || !table.revoke(root)
        || table.permits(read_only, OBJECT, Rights::READ)
    {
        return false;
    }

    let Some(replacement) = table.derive(second_root, Rights::READ) else {
        return false;
    };
    replacement != root
        && table.permits(replacement, 8, Rights::READ)
        && !table.permits(root, OBJECT, Rights::READ)
}
