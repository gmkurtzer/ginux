use core::{
    cell::UnsafeCell,
    mem::MaybeUninit,
    sync::atomic::{AtomicBool, AtomicU32, AtomicU8, Ordering},
};

use crate::{
    arch::x86_64::cpu,
    control_abi, domain,
    initramfs::{self, PublishedRootHandle},
    published_rootfs,
};

const NAMESPACE_LIMIT: usize = control_abi::DOMAIN_LIMIT + 4;
const EMPTY_SLOT: u8 = u8::MAX;
pub(crate) const ROOT_READ_ONLY: u32 = 1 << 0;
pub(crate) const ROOT_NOSUID: u32 = 1 << 1;
pub(crate) const ROOT_NODEV: u32 = 1 << 2;
const ROOT_FLAGS: u32 = ROOT_READ_ONLY | ROOT_NOSUID | ROOT_NODEV;
pub(crate) const MOUNTINFO: &[u8] = b"1 0 0:1 / / ro,nosuid,nodev - ginux-root rootfs ro\n2 1 0:2 / /proc rw,nosuid,nodev,noexec,relatime - proc proc rw\n3 1 0:3 / /sys/fs/cgroup rw,nosuid,nodev,noexec,relatime - cgroup2 cgroup2 rw\n";

struct MountRuntimeOwner {
    installed: AtomicBool,
    current_slot: AtomicU8,
    current_generation: AtomicU32,
    current_id: AtomicU32,
    table: UnsafeCell<MaybeUninit<MountNamespaceTable>>,
}

unsafe impl Sync for MountRuntimeOwner {}

static MOUNT_RUNTIME: MountRuntimeOwner = MountRuntimeOwner {
    installed: AtomicBool::new(false),
    current_slot: AtomicU8::new(EMPTY_SLOT),
    current_generation: AtomicU32::new(0),
    current_id: AtomicU32::new(0),
    table: UnsafeCell::new(MaybeUninit::uninit()),
};

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
    Published { domain_slot: u8 },
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
            Self::Published { domain_slot } => usize::from(domain_slot) < control_abi::DOMAIN_LIMIT,
        }
    }

    fn is_live(self) -> bool {
        match self {
            Self::Embedded(root) => initramfs::published_root_handle_is_live(root),
            Self::Published { domain_slot } => {
                published_rootfs::is_installed(usize::from(domain_slot))
            }
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
    if MOUNT_RUNTIME.installed.load(Ordering::Acquire) {
        return false;
    }
    let Some(root) = initramfs::published_root_handle().map(NamespaceRoot::Embedded) else {
        return false;
    };
    let table = MOUNT_RUNTIME.table.get().cast::<MountNamespaceTable>();
    unsafe { table.write(MountNamespaceTable::new()) };
    let table = unsafe { &mut *table };
    let Some(handle) = table.create(0, root, ROOT_FLAGS, true) else {
        return false;
    };
    table.boot_initial = Some(handle);
    MOUNT_RUNTIME.installed.store(true, Ordering::Release);
    live_namespace(handle).is_some()
}

pub(crate) fn activate_initial() -> bool {
    let (owner_domain, root) = match cpu::current_container_slot() {
        Some(container_slot) => {
            let Some(owner_domain) = cpu::container_domain_id(container_slot) else {
                return false;
            };
            if !cpu::domain_uses_published_rootfs(container_slot) {
                let Some(root) = initramfs::published_root_handle().map(NamespaceRoot::Embedded)
                else {
                    return false;
                };
                return activate_domain_root(owner_domain, root);
            }
            let Ok(domain_slot) = u8::try_from(container_slot) else {
                return false;
            };
            (owner_domain, NamespaceRoot::Published { domain_slot })
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
    if MOUNT_RUNTIME
        .current_slot
        .compare_exchange(EMPTY_SLOT, handle.slot, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return false;
    }
    MOUNT_RUNTIME
        .current_generation
        .store(handle.generation, Ordering::Release);
    MOUNT_RUNTIME.current_id.store(handle.id, Ordering::Release);
    current().is_some()
}

pub(crate) fn deactivate_initial() -> bool {
    let Some(active) = current() else {
        return false;
    };
    if live_namespace(active).is_none_or(|namespace| !namespace.domain_root) {
        return false;
    }
    MOUNT_RUNTIME.current_id.store(0, Ordering::Release);
    MOUNT_RUNTIME.current_generation.store(0, Ordering::Release);
    MOUNT_RUNTIME
        .current_slot
        .store(EMPTY_SLOT, Ordering::Release);
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
        ) && namespace.root_flags == ROOT_FLAGS
            && namespace.root.is_live()
    })
}

pub(crate) fn current_handle() -> Option<MountNamespaceHandle> {
    current().filter(|handle| live_namespace(*handle).is_some())
}

pub(crate) fn clone_current() -> Option<MountNamespaceHandle> {
    let active = current()?;
    let namespace = *live_namespace(active)?;
    live_table_mut()?.create(
        namespace.owner_domain,
        namespace.root,
        namespace.root_flags,
        false,
    )
}

pub(crate) fn switch_to(handle: MountNamespaceHandle) -> bool {
    if live_namespace(handle).is_none() {
        return false;
    }
    MOUNT_RUNTIME
        .current_slot
        .store(handle.slot, Ordering::Release);
    MOUNT_RUNTIME
        .current_generation
        .store(handle.generation, Ordering::Release);
    MOUNT_RUNTIME.current_id.store(handle.id, Ordering::Release);
    current() == Some(handle)
}

pub(crate) fn retire(handle: MountNamespaceHandle) -> bool {
    if current() == Some(handle) {
        return false;
    }
    live_table_mut().is_some_and(|table| table.retire(handle))
}

pub(crate) fn current_mountinfo() -> Option<&'static [u8]> {
    current_root_is_read_only().then_some(MOUNTINFO)
}

pub(crate) fn current_namespace_id() -> Option<u64> {
    current()
        .filter(|_| current_root_is_read_only())
        .map(|handle| u64::from(handle.id))
}

pub(crate) fn current_unique_mount_id(local_id: u64) -> Option<u64> {
    if !matches!(local_id, 1 | 2) {
        return None;
    }
    current_namespace_id()?
        .checked_shl(32)
        .and_then(|namespace| namespace.checked_add(local_id))
}

fn current() -> Option<MountNamespaceHandle> {
    let slot = MOUNT_RUNTIME.current_slot.load(Ordering::Acquire);
    let generation = MOUNT_RUNTIME.current_generation.load(Ordering::Acquire);
    let id = MOUNT_RUNTIME.current_id.load(Ordering::Acquire);
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
    if !MOUNT_RUNTIME.installed.load(Ordering::Acquire) {
        return None;
    }
    Some(unsafe { &*MOUNT_RUNTIME.table.get().cast::<MountNamespaceTable>() })
}

fn live_table_mut() -> Option<&'static mut MountNamespaceTable> {
    if !MOUNT_RUNTIME.installed.load(Ordering::Acquire) {
        return None;
    }
    Some(unsafe { &mut *MOUNT_RUNTIME.table.get().cast::<MountNamespaceTable>() })
}
