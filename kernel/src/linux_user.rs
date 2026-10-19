use crate::{arch::x86_64::cpu, control_abi, domain};
use core::sync::atomic::{AtomicBool, Ordering};

const NAMESPACE_LIMIT: usize = 4;
const ROOT_INSIDE_ID: u32 = 0;
const ROOT_OUTSIDE_ID: u32 = 100_000;
const ROOT_ID_COUNT: u32 = 1;
const HOST_INSTANCE: usize = control_abi::DOMAIN_LIMIT;
const INSTANCE_LIMIT: usize = control_abi::DOMAIN_LIMIT + 1;

pub(crate) const UID_MAP: &[u8] = b"0 100000 1\n";
pub(crate) const GID_MAP: &[u8] = b"0 100000 1\n";
pub(crate) const SETGROUPS: &[u8] = b"deny\n";

static TABLE_INSTALLED: AtomicBool = AtomicBool::new(false);
static mut LIVE_TABLES: [UserNamespaceTable; INSTANCE_LIMIT] =
    [const { UserNamespaceTable::new() }; INSTANCE_LIMIT];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct UserNamespaceHandle {
    owner_domain: u64,
    slot: u8,
    generation: u32,
    id: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Credentials {
    pub(crate) uid: u32,
    pub(crate) gid: u32,
    pub(crate) euid: u32,
    pub(crate) egid: u32,
    pub(crate) suid: u32,
    pub(crate) sgid: u32,
    pub(crate) supplementary_gid: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PrivilegedOperation {
    ChangeIdentity,
    ChangeRoot,
    ManageMounts,
    ManageKernel,
    ManageNamespaces,
    RawIo,
    LoadBpf,
    ObserveKernelLog,
    ManageClock,
    ProcessAccounting,
    ManageTerminal,
    ManageStorageQuota,
    TraceProcesses,
}

impl PrivilegedOperation {
    const fn bit(self) -> u32 {
        1 << self as u32
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct IdMap {
    inside: u32,
    outside: u32,
    count: u32,
}

impl IdMap {
    const ROOT: Self = Self {
        inside: ROOT_INSIDE_ID,
        outside: ROOT_OUTSIDE_ID,
        count: ROOT_ID_COUNT,
    };

    fn outside(self, inside: u32) -> Option<u32> {
        let offset = inside.checked_sub(self.inside)?;
        (offset < self.count).then(|| self.outside + offset)
    }

    fn inside(self, outside: u32) -> Option<u32> {
        let offset = outside.checked_sub(self.outside)?;
        (offset < self.count).then(|| self.inside + offset)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct UserNamespace {
    handle: UserNamespaceHandle,
    uid_map: IdMap,
    gid_map: IdMap,
    setgroups_allowed: bool,
    linux_capabilities: u64,
    privileged_operations: u32,
}

struct UserNamespaceTable {
    owner_domain: u64,
    namespaces: [Option<UserNamespace>; NAMESPACE_LIMIT],
    generations: [u32; NAMESPACE_LIMIT],
    next_id: u32,
    initial: Option<UserNamespaceHandle>,
}

impl UserNamespaceTable {
    const fn new() -> Self {
        Self {
            owner_domain: 0,
            namespaces: [None; NAMESPACE_LIMIT],
            generations: [0; NAMESPACE_LIMIT],
            next_id: 1,
            initial: None,
        }
    }

    fn create(&mut self, uid_map: IdMap, gid_map: IdMap) -> Option<UserNamespaceHandle> {
        if uid_map != IdMap::ROOT || gid_map != IdMap::ROOT {
            return None;
        }
        let slot = self.namespaces.iter().position(Option::is_none)?;
        let generation = self.generations[slot].checked_add(1)?;
        let id = self.next_id;
        self.next_id = self.next_id.checked_add(1)?;
        let handle = UserNamespaceHandle {
            owner_domain: self.owner_domain,
            slot: u8::try_from(slot).ok()?,
            generation,
            id,
        };
        self.generations[slot] = generation;
        self.namespaces[slot] = Some(UserNamespace {
            handle,
            uid_map,
            gid_map,
            setgroups_allowed: false,
            linux_capabilities: 0,
            privileged_operations: 0,
        });
        Some(handle)
    }

    fn namespace(&self, handle: UserNamespaceHandle) -> Option<&UserNamespace> {
        self.namespaces
            .get(usize::from(handle.slot))?
            .as_ref()
            .filter(|namespace| namespace.handle == handle)
    }

    fn retire(&mut self, handle: UserNamespaceHandle) -> bool {
        if self.initial == Some(handle) || self.namespace(handle).is_none() {
            return false;
        }
        self.namespaces[usize::from(handle.slot)] = None;
        true
    }
}

pub(crate) fn verify_contract() -> bool {
    let mut table = UserNamespaceTable::new();
    let invalid = IdMap {
        inside: 1,
        ..IdMap::ROOT
    };
    if table.create(invalid, IdMap::ROOT).is_some() {
        return false;
    }
    let Some(first) = table.create(IdMap::ROOT, IdMap::ROOT) else {
        return false;
    };
    table.initial = Some(first);
    let stale = UserNamespaceHandle {
        generation: first.generation.saturating_add(1),
        ..first
    };
    let Some(namespace) = table.namespace(first) else {
        return false;
    };
    if first.id != 1
        || namespace.uid_map.outside(0) != Some(ROOT_OUTSIDE_ID)
        || namespace.uid_map.inside(ROOT_OUTSIDE_ID) != Some(0)
        || namespace.uid_map.outside(1).is_some()
        || namespace.gid_map.inside(ROOT_OUTSIDE_ID + 1).is_some()
        || namespace.setgroups_allowed
        || namespace.linux_capabilities != 0
        || [
            PrivilegedOperation::ChangeIdentity,
            PrivilegedOperation::ChangeRoot,
            PrivilegedOperation::ManageMounts,
            PrivilegedOperation::ManageKernel,
            PrivilegedOperation::ManageNamespaces,
            PrivilegedOperation::RawIo,
            PrivilegedOperation::LoadBpf,
            PrivilegedOperation::ObserveKernelLog,
            PrivilegedOperation::ManageClock,
            PrivilegedOperation::ProcessAccounting,
            PrivilegedOperation::ManageTerminal,
            PrivilegedOperation::ManageStorageQuota,
            PrivilegedOperation::TraceProcesses,
        ]
        .into_iter()
        .any(|operation| !privileged_operation_denied_in(namespace, operation))
        || table.namespace(stale).is_some()
        || table.retire(first)
    {
        return false;
    }
    let mut disposable = [None; NAMESPACE_LIMIT - 1];
    for handle in &mut disposable {
        *handle = table.create(IdMap::ROOT, IdMap::ROOT);
        if handle.is_none() {
            return false;
        }
    }
    if table.create(IdMap::ROOT, IdMap::ROOT).is_some() {
        return false;
    }
    let retired = disposable[0].is_some_and(|handle| table.retire(handle));
    let Some(replacement) = table.create(IdMap::ROOT, IdMap::ROOT) else {
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
    let mut index = 0;
    while index < INSTANCE_LIMIT {
        unsafe {
            core::ptr::addr_of_mut!(LIVE_TABLES[index]).write(UserNamespaceTable::new());
        }
        index += 1;
    }
    let Some(table) = live_table_mut_at(HOST_INSTANCE) else {
        return false;
    };
    let Some(handle) = table.create(IdMap::ROOT, IdMap::ROOT) else {
        return false;
    };
    table.initial = Some(handle);
    TABLE_INSTALLED.store(true, Ordering::Release);
    credentials(handle).is_some()
}

pub(crate) fn initial() -> Option<UserNamespaceHandle> {
    let (index, owner_domain) = current_instance_and_owner()?;
    let table = live_table_mut_at(index)?;
    if table.initial.is_none() {
        table.owner_domain = owner_domain;
        let handle = table.create(IdMap::ROOT, IdMap::ROOT)?;
        table.initial = Some(handle);
    }
    (table.owner_domain == owner_domain)
        .then_some(table.initial)
        .flatten()
}

pub(crate) fn credentials(handle: UserNamespaceHandle) -> Option<Credentials> {
    let namespace = live_table_for_handle(handle)?.namespace(handle)?;
    if namespace.uid_map.outside(0) != Some(ROOT_OUTSIDE_ID)
        || namespace.gid_map.outside(0) != Some(ROOT_OUTSIDE_ID)
        || namespace.setgroups_allowed
        || namespace.linux_capabilities != 0
    {
        return None;
    }
    Some(Credentials {
        uid: 0,
        gid: 0,
        euid: 0,
        egid: 0,
        suid: 0,
        sgid: 0,
        supplementary_gid: 0,
    })
}

pub(crate) fn has_no_linux_capabilities(handle: UserNamespaceHandle) -> bool {
    live_table_for_handle(handle)
        .and_then(|table| table.namespace(handle))
        .is_some_and(|namespace| namespace.linux_capabilities == 0)
}

pub(crate) fn setgroups_allowed(handle: UserNamespaceHandle) -> bool {
    live_table_for_handle(handle)
        .and_then(|table| table.namespace(handle))
        .is_some_and(|namespace| namespace.setgroups_allowed)
}

pub(crate) fn privileged_operation_denied(
    handle: UserNamespaceHandle,
    operation: PrivilegedOperation,
) -> bool {
    // A domain's root user may administer only that domain's mount namespace.
    // The syscall implementation independently requires a live domain context,
    // so this authority can never mutate the control domain or another domain.
    if operation == PrivilegedOperation::ManageMounts && handle.owner_domain != 0 {
        return false;
    }
    live_table_for_handle(handle)
        .and_then(|table| table.namespace(handle))
        .is_some_and(|namespace| privileged_operation_denied_in(namespace, operation))
}

fn privileged_operation_denied_in(
    namespace: &UserNamespace,
    operation: PrivilegedOperation,
) -> bool {
    namespace.linux_capabilities == 0 && namespace.privileged_operations & operation.bit() == 0
}

fn live_table_for_handle(handle: UserNamespaceHandle) -> Option<&'static UserNamespaceTable> {
    let index = if handle.owner_domain == 0 {
        HOST_INSTANCE
    } else {
        domain::container_slot_for_domain(handle.owner_domain)?
    };
    let table = live_table_at(index)?;
    (table.owner_domain == handle.owner_domain).then_some(table)
}

fn live_table_at(index: usize) -> Option<&'static UserNamespaceTable> {
    if !TABLE_INSTALLED.load(Ordering::Acquire) {
        return None;
    }
    let table = unsafe {
        core::ptr::addr_of!(LIVE_TABLES)
            .cast::<UserNamespaceTable>()
            .add(index)
    };
    Some(unsafe { &*table })
}

fn live_table_mut_at(index: usize) -> Option<&'static mut UserNamespaceTable> {
    if !TABLE_INSTALLED.load(Ordering::Acquire) && index != HOST_INSTANCE {
        return None;
    }
    let table = unsafe {
        core::ptr::addr_of_mut!(LIVE_TABLES)
            .cast::<UserNamespaceTable>()
            .add(index)
    };
    Some(unsafe { &mut *table })
}

fn current_instance_and_owner() -> Option<(usize, u64)> {
    let Some(slot) = cpu::current_container_slot() else {
        return Some((HOST_INSTANCE, 0));
    };
    Some((slot, cpu::container_domain_id(slot)?))
}
