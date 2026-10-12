use core::{
    mem::MaybeUninit,
    sync::atomic::{AtomicBool, AtomicU32, AtomicU8, Ordering},
};

use crate::initramfs::{self, PublishedRootHandle};

const NAMESPACE_LIMIT: usize = 4;
const EMPTY_SLOT: u8 = u8::MAX;
pub(crate) const ROOT_READ_ONLY: u32 = 1 << 0;
pub(crate) const ROOT_NOSUID: u32 = 1 << 1;
pub(crate) const ROOT_NODEV: u32 = 1 << 2;
const ROOT_FLAGS: u32 = ROOT_READ_ONLY | ROOT_NOSUID | ROOT_NODEV;
pub(crate) const MOUNTINFO: &[u8] = b"1 0 0:1 / / ro,nosuid,nodev - ginux-root rootfs ro\n";

static TABLE_INSTALLED: AtomicBool = AtomicBool::new(false);
static CURRENT_SLOT: AtomicU8 = AtomicU8::new(EMPTY_SLOT);
static CURRENT_GENERATION: AtomicU32 = AtomicU32::new(0);
static CURRENT_ID: AtomicU32 = AtomicU32::new(0);
static mut LIVE_TABLE: MaybeUninit<MountNamespaceTable> = MaybeUninit::uninit();

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MountNamespaceHandle {
    slot: u8,
    generation: u32,
    id: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct MountNamespace {
    handle: MountNamespaceHandle,
    root: PublishedRootHandle,
    root_flags: u32,
}

struct MountNamespaceTable {
    namespaces: [Option<MountNamespace>; NAMESPACE_LIMIT],
    generations: [u32; NAMESPACE_LIMIT],
    next_id: u32,
    initial: Option<MountNamespaceHandle>,
}

impl MountNamespaceTable {
    const fn new() -> Self {
        Self {
            namespaces: [None; NAMESPACE_LIMIT],
            generations: [0; NAMESPACE_LIMIT],
            next_id: 1,
            initial: None,
        }
    }

    fn create(
        &mut self,
        root: PublishedRootHandle,
        root_flags: u32,
    ) -> Option<MountNamespaceHandle> {
        if root.generation == 0 || root_flags != ROOT_FLAGS {
            return None;
        }
        let slot = self.namespaces.iter().position(Option::is_none)?;
        let generation = self.generations[slot].checked_add(1)?;
        let id = self.next_id;
        self.next_id = self.next_id.checked_add(1)?;
        let handle = MountNamespaceHandle {
            slot: u8::try_from(slot).ok()?,
            generation,
            id,
        };
        self.generations[slot] = generation;
        self.namespaces[slot] = Some(MountNamespace {
            handle,
            root,
            root_flags,
        });
        Some(handle)
    }

    fn namespace(&self, handle: MountNamespaceHandle) -> Option<&MountNamespace> {
        self.namespaces
            .get(usize::from(handle.slot))?
            .as_ref()
            .filter(|namespace| namespace.handle == handle)
    }

    fn retire(&mut self, handle: MountNamespaceHandle) -> bool {
        let Some(namespace) = self.namespace(handle) else {
            return false;
        };
        if self.initial == Some(handle) || namespace.handle != handle {
            return false;
        }
        self.namespaces[usize::from(handle.slot)] = None;
        true
    }
}

pub(crate) fn verify_contract() -> bool {
    let root = PublishedRootHandle { generation: 7 };
    let mut table = MountNamespaceTable::new();
    if table.create(root, ROOT_READ_ONLY).is_some() {
        return false;
    }
    let Some(first) = table.create(root, ROOT_FLAGS) else {
        return false;
    };
    table.initial = Some(first);
    let stale = MountNamespaceHandle {
        generation: first.generation.saturating_add(1),
        ..first
    };
    if first.id != 1
        || table
            .namespace(first)
            .is_none_or(|namespace| namespace.root != root || namespace.root_flags != ROOT_FLAGS)
        || table.namespace(stale).is_some()
        || table.retire(first)
    {
        return false;
    }
    let mut disposable = [None; NAMESPACE_LIMIT - 1];
    for slot in &mut disposable {
        *slot = table.create(root, ROOT_FLAGS);
        if slot.is_none() {
            return false;
        }
    }
    if table.create(root, ROOT_FLAGS).is_some() {
        return false;
    }
    let retired = disposable[0].is_some_and(|handle| table.retire(handle));
    let Some(replacement) = table.create(root, ROOT_FLAGS) else {
        return false;
    };
    retired
        && replacement.slot == disposable[0].unwrap().slot
        && replacement.generation > disposable[0].unwrap().generation
        && replacement.id > disposable[0].unwrap().id
        && table.namespace(disposable[0].unwrap()).is_none()
}

pub(crate) fn install_initial() -> bool {
    if TABLE_INSTALLED.load(Ordering::Acquire) {
        return false;
    }
    let Some(root) = initramfs::published_root_handle() else {
        return false;
    };
    let table = core::ptr::addr_of_mut!(LIVE_TABLE).cast::<MountNamespaceTable>();
    unsafe { table.write(MountNamespaceTable::new()) };
    let table = unsafe { &mut *table };
    let Some(handle) = table.create(root, ROOT_FLAGS) else {
        return false;
    };
    table.initial = Some(handle);
    TABLE_INSTALLED.store(true, Ordering::Release);
    live_namespace(handle).is_some()
}

pub(crate) fn activate_initial() -> bool {
    let Some(table) = live_table() else {
        return false;
    };
    let Some(handle) = table.initial else {
        return false;
    };
    if CURRENT_SLOT
        .compare_exchange(EMPTY_SLOT, handle.slot, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return false;
    }
    CURRENT_GENERATION.store(handle.generation, Ordering::Release);
    CURRENT_ID.store(handle.id, Ordering::Release);
    current().is_some()
}

pub(crate) fn deactivate_initial() -> bool {
    let Some(active) = current() else {
        return false;
    };
    let initial = live_table().and_then(|table| table.initial);
    if initial != Some(active) {
        return false;
    }
    CURRENT_ID.store(0, Ordering::Release);
    CURRENT_GENERATION.store(0, Ordering::Release);
    CURRENT_SLOT.store(EMPTY_SLOT, Ordering::Release);
    current().is_none()
}

pub(crate) fn current_root_is_read_only() -> bool {
    current().and_then(live_namespace).is_some_and(|namespace| {
        namespace.root_flags == ROOT_FLAGS
            && initramfs::published_root_handle_is_live(namespace.root)
    })
}

pub(crate) fn current_mountinfo() -> Option<&'static [u8]> {
    current_root_is_read_only().then_some(MOUNTINFO)
}

fn current() -> Option<MountNamespaceHandle> {
    let slot = CURRENT_SLOT.load(Ordering::Acquire);
    let generation = CURRENT_GENERATION.load(Ordering::Acquire);
    let id = CURRENT_ID.load(Ordering::Acquire);
    if slot == EMPTY_SLOT || generation == 0 || id == 0 {
        return None;
    }
    let handle = MountNamespaceHandle {
        slot,
        generation,
        id,
    };
    live_namespace(handle).map(|_| handle)
}

fn live_namespace(handle: MountNamespaceHandle) -> Option<&'static MountNamespace> {
    live_table()?.namespace(handle)
}

fn live_table() -> Option<&'static MountNamespaceTable> {
    if !TABLE_INSTALLED.load(Ordering::Acquire) {
        return None;
    }
    Some(unsafe { &*core::ptr::addr_of!(LIVE_TABLE).cast::<MountNamespaceTable>() })
}
