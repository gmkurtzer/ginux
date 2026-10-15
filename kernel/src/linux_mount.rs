use core::{
    mem::MaybeUninit,
    sync::atomic::{AtomicBool, AtomicU32, AtomicU8, Ordering},
};

use crate::{
    arch::x86_64::cpu,
    control_abi, domain,
    initramfs::{self, PublishedRootHandle},
    oci_image,
};

const NAMESPACE_LIMIT: usize = control_abi::CONTAINER_LIMIT + 4;
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
    owner_domain: u64,
    root: NamespaceRoot,
    root_flags: u32,
    domain_root: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum NamespaceRoot {
    Embedded(PublishedRootHandle),
    Oci { container_slot: u8 },
}

struct MountNamespaceTable {
    namespaces: [Option<MountNamespace>; NAMESPACE_LIMIT],
    generations: [u32; NAMESPACE_LIMIT],
    next_id: u32,
    boot_initial: Option<MountNamespaceHandle>,
}

impl MountNamespaceTable {
    const fn new() -> Self {
        Self {
            namespaces: [None; NAMESPACE_LIMIT],
            generations: [0; NAMESPACE_LIMIT],
            next_id: 1,
            boot_initial: None,
        }
    }

    fn create(
        &mut self,
        owner_domain: u64,
        root: NamespaceRoot,
        root_flags: u32,
        domain_root: bool,
    ) -> Option<MountNamespaceHandle> {
        if !root.is_valid()
            || root_flags != ROOT_FLAGS
            || domain_root
                && self.namespaces.iter().flatten().any(|namespace| {
                    namespace.owner_domain == owner_domain && namespace.domain_root
                })
        {
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
            owner_domain,
            root,
            root_flags,
            domain_root,
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
        if namespace.domain_root || namespace.handle != handle {
            return false;
        }
        self.namespaces[usize::from(handle.slot)] = None;
        true
    }
}

impl NamespaceRoot {
    fn is_valid(self) -> bool {
        match self {
            Self::Embedded(root) => root.generation != 0,
            Self::Oci { container_slot } => {
                usize::from(container_slot) < control_abi::CONTAINER_LIMIT
            }
        }
    }

    fn is_live(self) -> bool {
        match self {
            Self::Embedded(root) => initramfs::published_root_handle_is_live(root),
            Self::Oci { container_slot } => oci_image::is_installed(usize::from(container_slot)),
        }
    }
}

pub(crate) fn verify_contract() -> bool {
    let root = NamespaceRoot::Embedded(PublishedRootHandle { generation: 7 });
    let mut table = MountNamespaceTable::new();
    if table.create(7, root, ROOT_READ_ONLY, true).is_some() {
        return false;
    }
    let Some(first) = table.create(7, root, ROOT_FLAGS, true) else {
        return false;
    };
    table.boot_initial = Some(first);
    let stale = MountNamespaceHandle {
        generation: first.generation.saturating_add(1),
        ..first
    };
    if first.id != 1
        || table.namespace(first).is_none_or(|namespace| {
            namespace.root != root
                || namespace.root_flags != ROOT_FLAGS
                || namespace.owner_domain != 7
                || !namespace.domain_root
        })
        || table.namespace(stale).is_some()
        || table.retire(first)
    {
        return false;
    }
    let mut disposable = [None; NAMESPACE_LIMIT - 1];
    for slot in &mut disposable {
        *slot = table.create(7, root, ROOT_FLAGS, false);
        if slot.is_none() {
            return false;
        }
    }
    if table.create(7, root, ROOT_FLAGS, false).is_some() {
        return false;
    }
    let retired = disposable[0].is_some_and(|handle| table.retire(handle));
    let Some(replacement) = table.create(7, root, ROOT_FLAGS, false) else {
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
    let Some(root) = initramfs::published_root_handle().map(NamespaceRoot::Embedded) else {
        return false;
    };
    let table = core::ptr::addr_of_mut!(LIVE_TABLE).cast::<MountNamespaceTable>();
    unsafe { table.write(MountNamespaceTable::new()) };
    let table = unsafe { &mut *table };
    let Some(handle) = table.create(0, root, ROOT_FLAGS, true) else {
        return false;
    };
    table.boot_initial = Some(handle);
    TABLE_INSTALLED.store(true, Ordering::Release);
    live_namespace(handle).is_some()
}

pub(crate) fn activate_initial() -> bool {
    let (owner_domain, root) = match cpu::current_container_slot() {
        Some(container_slot) => {
            let Some(owner_domain) = cpu::container_domain_id(container_slot) else {
                return false;
            };
            if !cpu::container_uses_installed_oci_image(container_slot) {
                let Some(root) = initramfs::published_root_handle().map(NamespaceRoot::Embedded)
                else {
                    return false;
                };
                return activate_domain_root(owner_domain, root);
            }
            let Ok(container_slot) = u8::try_from(container_slot) else {
                return false;
            };
            (owner_domain, NamespaceRoot::Oci { container_slot })
        }
        None => {
            let Some(root) = initramfs::published_root_handle().map(NamespaceRoot::Embedded) else {
                return false;
            };
            (0, root)
        }
    };
    activate_domain_root(owner_domain, root)
}

fn activate_domain_root(owner_domain: u64, root: NamespaceRoot) -> bool {
    let Some(table) = live_table_mut() else {
        return false;
    };
    let handle = table
        .namespaces
        .iter()
        .flatten()
        .find(|namespace| namespace.owner_domain == owner_domain && namespace.domain_root)
        .map(|namespace| namespace.handle)
        .or_else(|| (owner_domain == 0).then_some(table.boot_initial).flatten())
        .or_else(|| table.create(owner_domain, root, ROOT_FLAGS, true));
    let Some(handle) = handle else { return false };
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
    if live_namespace(active).is_none_or(|namespace| !namespace.domain_root) {
        return false;
    }
    CURRENT_ID.store(0, Ordering::Release);
    CURRENT_GENERATION.store(0, Ordering::Release);
    CURRENT_SLOT.store(EMPTY_SLOT, Ordering::Release);
    current().is_none()
}

pub(crate) fn current_root_is_read_only() -> bool {
    let expected_owner = cpu::current_container_slot().and_then(cpu::container_domain_id);
    current().and_then(live_namespace).is_some_and(|namespace| {
        expected_owner.map_or_else(
            || {
                namespace.owner_domain == 0
                    || domain::container_domain_limits(namespace.owner_domain).is_some()
            },
            |expected| namespace.owner_domain == expected,
        ) && namespace.domain_root
            && namespace.root_flags == ROOT_FLAGS
            && namespace.root.is_live()
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

fn live_table_mut() -> Option<&'static mut MountNamespaceTable> {
    if !TABLE_INSTALLED.load(Ordering::Acquire) {
        return None;
    }
    Some(unsafe { &mut *core::ptr::addr_of_mut!(LIVE_TABLE).cast::<MountNamespaceTable>() })
}
