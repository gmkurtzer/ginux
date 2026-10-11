use crate::capability::{Handle, Rights, Table as CapabilityTable};
use crate::manifest::Manifest;
use core::mem::MaybeUninit;
use core::sync::atomic::{AtomicBool, Ordering};

const MAX_DOMAINS: usize = 4;
const MAX_BOOT_GRANTS: usize = 8;
const CONTROL_CALL_BUDGET: u16 = 11;
const SERVICE_CALL_BUDGET: u16 = 3;
const BOOTSTRAP_THREAD_LIMIT: u16 = 1;
const EVENT_CAPACITY: usize = 4;

static INITIALIZED: AtomicBool = AtomicBool::new(false);
static mut GLOBAL_MANAGER: MaybeUninit<Manager> = MaybeUninit::uninit();

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DomainId {
    slot: u16,
    generation: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Role {
    ControlPlane,
    Service,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum State {
    Ready,
}

#[derive(Clone, Copy)]
pub(crate) struct BootGrant {
    object: u64,
    rights: Rights,
}

impl BootGrant {
    pub(crate) const fn new(object: u64, rights: Rights) -> Self {
        Self { object, rights }
    }
}

struct Domain {
    id: DomainId,
    role: Role,
    state: State,
    capabilities: CapabilityTable,
    boot_roots: [Option<Handle>; MAX_BOOT_GRANTS],
    boot_root_count: usize,
    remaining_calls: u16,
    active_threads: u16,
    thread_limit: u16,
    parent: Option<DomainId>,
    delegated_roots: [Option<DelegatedRoot>; MAX_BOOT_GRANTS],
}

#[derive(Clone, Copy)]
struct DelegatedRoot {
    local: Handle,
    domain: DomainId,
    source: Handle,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct LifecycleEvent {
    kind: u8,
    domain: DomainId,
}

struct EventQueue {
    events: [Option<LifecycleEvent>; EVENT_CAPACITY],
    head: usize,
    len: usize,
}

impl EventQueue {
    const fn new() -> Self {
        Self {
            events: [None; EVENT_CAPACITY],
            head: 0,
            len: 0,
        }
    }

    fn push(&mut self, event: LifecycleEvent) -> bool {
        if self.len == EVENT_CAPACITY {
            return false;
        }
        let tail = (self.head + self.len) % EVENT_CAPACITY;
        self.events[tail] = Some(event);
        self.len += 1;
        true
    }

    fn pop(&mut self) -> Option<LifecycleEvent> {
        if self.len == 0 {
            return None;
        }
        let event = self.events[self.head].take()?;
        self.head = (self.head + 1) % EVENT_CAPACITY;
        self.len -= 1;
        Some(event)
    }

    const fn is_empty(&self) -> bool {
        self.len == 0
    }
}

pub(crate) struct Manager {
    domains: [Option<Domain>; MAX_DOMAINS],
    generations: [u32; MAX_DOMAINS],
    control_plane_created: bool,
    control_plane_id: Option<DomainId>,
    service_id: Option<DomainId>,
    last_retired_service_id: Option<DomainId>,
    lifecycle_events: EventQueue,
    last_delivered_service_fault: Option<DomainId>,
    total_calls: u16,
    total_thread_attaches: u16,
}

impl Manager {
    pub(crate) const fn new() -> Self {
        Self {
            domains: [const { None }; MAX_DOMAINS],
            generations: [0; MAX_DOMAINS],
            control_plane_created: false,
            control_plane_id: None,
            service_id: None,
            last_retired_service_id: None,
            lifecycle_events: EventQueue::new(),
            last_delivered_service_fault: None,
            total_calls: 0,
            total_thread_attaches: 0,
        }
    }

    fn create_control_plane(&mut self, grants: &[BootGrant]) -> Option<DomainId> {
        if self.control_plane_created || grants.is_empty() || grants.len() > MAX_BOOT_GRANTS {
            return None;
        }
        let slot = self.domains.iter().position(Option::is_none)?;
        let generation = self.generations[slot].checked_add(1)?;
        let id = DomainId {
            slot: slot as u16,
            generation,
        };
        let mut capabilities = CapabilityTable::new();
        let mut boot_roots = [None; MAX_BOOT_GRANTS];
        for (index, grant) in grants.iter().enumerate() {
            boot_roots[index] = Some(capabilities.authorize_boot(grant.object, grant.rights)?);
        }
        capabilities.seal_boot_authority();

        self.generations[slot] = generation;
        self.domains[slot] = Some(Domain {
            id,
            role: Role::ControlPlane,
            state: State::Ready,
            capabilities,
            boot_roots,
            boot_root_count: grants.len(),
            remaining_calls: CONTROL_CALL_BUDGET,
            active_threads: 0,
            thread_limit: BOOTSTRAP_THREAD_LIMIT,
            parent: None,
            delegated_roots: [None; MAX_BOOT_GRANTS],
        });
        self.control_plane_created = true;
        self.control_plane_id = Some(id);
        Some(id)
    }

    fn domain(&self, id: DomainId) -> Option<&Domain> {
        self.domains
            .get(usize::from(id.slot))?
            .as_ref()
            .filter(|domain| domain.id == id)
    }

    fn create_service(&mut self) -> Option<DomainId> {
        const IPC_OBJECT: u64 = 3;
        let rights = Rights::READ.union(Rights::WRITE);
        let control_id = self.control_plane_id?;
        let control = self.domain(control_id)?;
        let source = control.boot_roots.get(2).copied().flatten()?;
        if !control.capabilities.permits(source, IPC_OBJECT, rights) || self.service_id.is_some() {
            return None;
        }

        let slot = self.domains.iter().position(Option::is_none)?;
        let generation = self.generations[slot].checked_add(1)?;
        let id = DomainId {
            slot: slot as u16,
            generation,
        };
        let mut capabilities = CapabilityTable::new();
        let root = capabilities.accept_delegated(IPC_OBJECT, rights)?;
        capabilities.seal_boot_authority();
        let mut boot_roots = [None; MAX_BOOT_GRANTS];
        boot_roots[0] = Some(root);
        let mut delegated_roots = [None; MAX_BOOT_GRANTS];
        delegated_roots[0] = Some(DelegatedRoot {
            local: root,
            domain: control_id,
            source,
        });
        self.generations[slot] = generation;
        self.domains[slot] = Some(Domain {
            id,
            role: Role::Service,
            state: State::Ready,
            capabilities,
            boot_roots,
            boot_root_count: 1,
            remaining_calls: SERVICE_CALL_BUDGET,
            active_threads: 0,
            thread_limit: BOOTSTRAP_THREAD_LIMIT,
            parent: Some(control_id),
            delegated_roots,
        });
        self.service_id = Some(id);
        Some(id)
    }

    fn permits(&self, id: DomainId, handle: Handle, object: u64, rights: Rights) -> bool {
        self.permits_at_depth(id, handle, object, rights, 0)
    }

    fn permits_at_depth(
        &self,
        id: DomainId,
        handle: Handle,
        object: u64,
        rights: Rights,
        depth: usize,
    ) -> bool {
        if depth >= MAX_DOMAINS {
            return false;
        }
        let Some(domain) = self.domain(id) else {
            return false;
        };
        if !domain.capabilities.permits(handle, object, rights) {
            return false;
        }
        let Some(root) = domain.capabilities.authority_root(handle) else {
            return false;
        };
        if let Some(parent) = domain
            .delegated_roots
            .iter()
            .flatten()
            .find(|delegation| delegation.local == root)
        {
            return self.permits_at_depth(parent.domain, parent.source, object, rights, depth + 1);
        }
        self.control_plane_id == Some(id)
            && domain.boot_roots[..domain.boot_root_count].contains(&Some(root))
    }

    fn is_descendant(&self, ancestor: DomainId, child: DomainId) -> bool {
        if ancestor == child || self.domain(ancestor).is_none() {
            return false;
        }
        let mut current = child;
        for _ in 0..MAX_DOMAINS {
            let Some(parent) = self.domain(current).and_then(|domain| domain.parent) else {
                return false;
            };
            if parent == ancestor {
                return true;
            }
            current = parent;
        }
        false
    }

    fn retire_service(&mut self, id: DomainId) -> bool {
        if self.service_id != Some(id)
            || self
                .domain(id)
                .is_none_or(|domain| domain.role != Role::Service || domain.active_threads != 0)
        {
            return false;
        }
        self.domains[usize::from(id.slot)] = None;
        self.service_id = None;
        self.last_retired_service_id = Some(id);
        true
    }
}

pub(crate) fn initialize_control_plane(manifest: &Manifest) -> Option<u64> {
    if INITIALIZED.load(Ordering::Acquire) {
        return None;
    }
    const HOST_CONTROL: u64 = 1;
    const CONSOLE: u64 = 2;
    let mut grants = [BootGrant::new(0, Rights::READ); MAX_BOOT_GRANTS];
    for (index, grant) in manifest.grants().iter().enumerate() {
        grants[index] = BootGrant::new(grant.object, Rights::from_bits(grant.rights)?);
    }
    let grants = &grants[..manifest.grant_count()];
    let mut manager = Manager::new();
    let id = manager.create_control_plane(grants)?;
    if manager.create_control_plane(grants).is_some() {
        return None;
    }
    let domain = manager.domain(id)?;
    if domain.role != Role::ControlPlane
        || domain.state != State::Ready
        || !domain.capabilities.boot_authority_is_sealed()
        || domain.boot_root_count != grants.len()
        || !domain
            .capabilities
            .permits(domain.boot_roots[0]?, HOST_CONTROL, Rights::MANAGE)
        || domain
            .capabilities
            .permits(domain.boot_roots[1]?, CONSOLE, Rights::READ)
    {
        return None;
    }
    let encoded = encode_id(id);
    unsafe {
        core::ptr::addr_of_mut!(GLOBAL_MANAGER).write(MaybeUninit::new(manager));
    }
    INITIALIZED.store(true, Ordering::Release);
    control_plane_permits(0, HOST_CONTROL, Rights::MANAGE).then_some(encoded)
}

pub(crate) fn control_plane_permits(root_index: usize, object: u64, rights: Rights) -> bool {
    if !INITIALIZED.load(Ordering::Acquire) {
        return false;
    }
    unsafe {
        let manager = &*core::ptr::addr_of!(GLOBAL_MANAGER).cast::<Manager>();
        let Some(id) = manager.control_plane_id else {
            return false;
        };
        let Some(domain) = manager.domain(id) else {
            return false;
        };
        let Some(handle) = domain.boot_roots.get(root_index).copied().flatten() else {
            return false;
        };
        domain.capabilities.permits(handle, object, rights)
    }
}

pub(crate) fn control_plane_boot_handle(root_index: usize) -> Option<u64> {
    let manager = global_manager()?;
    let domain = manager.domain(manager.control_plane_id?)?;
    Some(
        domain
            .boot_roots
            .get(root_index)
            .copied()
            .flatten()?
            .to_raw(),
    )
}

pub(crate) fn initialize_boot_service() -> Option<u64> {
    let manager = global_manager_mut()?;
    let id = manager.create_service()?;
    let service = manager.domain(id)?;
    (service.role == Role::Service
        && service.state == State::Ready
        && service.boot_root_count == 1
        && service.capabilities.boot_authority_is_sealed())
    .then_some(encode_id(id))
}

pub(crate) fn service_boot_handle() -> Option<u64> {
    let manager = global_manager()?;
    let service = manager.domain(manager.service_id?)?;
    Some(service.boot_roots[0]?.to_raw())
}

pub(crate) fn control_plane_id() -> Option<u64> {
    Some(encode_id(global_manager()?.control_plane_id?))
}

pub(crate) fn handle_permits(domain_raw: u64, raw: u64, object: u64, rights: Rights) -> bool {
    let Some(domain_id) = decode_id(domain_raw) else {
        return false;
    };
    let Some(handle) = Handle::from_raw(raw) else {
        return false;
    };
    let Some(manager) = global_manager() else {
        return false;
    };
    manager.permits(domain_id, handle, object, rights)
}

pub(crate) fn verify_cross_domain_revocation_invariants() -> bool {
    verify_cross_domain_revocation_invariants_inner().unwrap_or(false)
}

fn verify_cross_domain_revocation_invariants_inner() -> Option<bool> {
    let grants = [
        BootGrant::new(1, Rights::READ.union(Rights::MANAGE)),
        BootGrant::new(2, Rights::WRITE),
        BootGrant::new(3, Rights::READ.union(Rights::WRITE)),
    ];
    let mut manager = Manager::new();
    let control = manager.create_control_plane(&grants)?;
    let service = manager.create_service()?;
    let service_handle = manager.domain(service)?.boot_roots[0]?;
    let parent_handle = manager.domain(control)?.boot_roots[2]?;
    let rights = Rights::READ.union(Rights::WRITE);
    let service_read = manager.domains[usize::from(service.slot)]
        .as_mut()?
        .capabilities
        .derive(service_handle, Rights::READ)?;
    if !manager.permits(service, service_handle, 3, rights)
        || !manager.permits(service, service_read, 3, Rights::READ)
        || manager.permits(service, service_read, 3, Rights::WRITE)
    {
        return Some(false);
    }
    manager.domains[usize::from(control.slot)]
        .as_mut()?
        .capabilities
        .revoke(parent_handle);
    Some(
        manager
            .domain(service)?
            .capabilities
            .permits(service_handle, 3, rights)
            && !manager.permits(service, service_handle, 3, rights)
            && !manager.permits(service, service_read, 3, Rights::READ),
    )
}

pub(crate) fn verify_domain_ancestry_invariants() -> bool {
    verify_domain_ancestry_invariants_inner().unwrap_or(false)
}

fn verify_domain_ancestry_invariants_inner() -> Option<bool> {
    let grants = [
        BootGrant::new(1, Rights::READ.union(Rights::MANAGE)),
        BootGrant::new(2, Rights::WRITE),
        BootGrant::new(3, Rights::READ.union(Rights::WRITE)),
    ];
    let mut manager = Manager::new();
    let control = manager.create_control_plane(&grants)?;
    let service = manager.create_service()?;
    let service_ipc = manager.domain(service)?.boot_roots[0]?;
    if !manager.is_descendant(control, service)
        || manager.is_descendant(service, control)
        || manager.is_descendant(control, control)
        || manager.permits(service, service_ipc, 1, Rights::READ)
        || !manager.retire_service(service)
        || manager.is_descendant(control, service)
    {
        return Some(false);
    }
    let replacement = manager.create_service()?;
    Some(replacement != service && manager.is_descendant(control, replacement))
}

pub(crate) fn control_owns_service(control_raw: u64, service_raw: u64) -> bool {
    let (Some(control), Some(service), Some(manager)) = (
        decode_id(control_raw),
        decode_id(service_raw),
        global_manager(),
    ) else {
        return false;
    };
    manager.is_descendant(control, service)
}

pub(crate) fn charge_call(domain_raw: u64) -> bool {
    let Some(id) = decode_id(domain_raw) else {
        return false;
    };
    let Some(manager) = global_manager_mut() else {
        return false;
    };
    let Some(total_calls) = manager.total_calls.checked_add(1) else {
        return false;
    };
    let Some(domain) = manager
        .domains
        .get_mut(usize::from(id.slot))
        .and_then(Option::as_mut)
        .filter(|domain| domain.id == id)
    else {
        return false;
    };
    let Some(remaining_calls) = domain.remaining_calls.checked_sub(1) else {
        return false;
    };
    domain.remaining_calls = remaining_calls;
    manager.total_calls = total_calls;
    true
}

pub(crate) fn call_budget_proof_complete() -> bool {
    let Some(manager) = global_manager() else {
        return false;
    };
    let Some(control) = manager.control_plane_id.and_then(|id| manager.domain(id)) else {
        return false;
    };
    control.remaining_calls == 0 && manager.total_calls == 16
}

pub(crate) fn attach_thread(domain_raw: u64) -> bool {
    let Some(id) = decode_id(domain_raw) else {
        return false;
    };
    let Some(manager) = global_manager_mut() else {
        return false;
    };
    let Some(total) = manager.total_thread_attaches.checked_add(1) else {
        return false;
    };
    let Some(domain) = manager
        .domains
        .get_mut(usize::from(id.slot))
        .and_then(Option::as_mut)
        .filter(|domain| domain.id == id)
    else {
        return false;
    };
    if domain.active_threads >= domain.thread_limit {
        return false;
    }
    domain.active_threads += 1;
    manager.total_thread_attaches = total;
    true
}

pub(crate) fn detach_thread(domain_raw: u64) -> bool {
    let Some(id) = decode_id(domain_raw) else {
        return false;
    };
    let Some(manager) = global_manager_mut() else {
        return false;
    };
    let Some(domain) = manager
        .domains
        .get_mut(usize::from(id.slot))
        .and_then(Option::as_mut)
        .filter(|domain| domain.id == id)
    else {
        return false;
    };
    let Some(active) = domain.active_threads.checked_sub(1) else {
        return false;
    };
    domain.active_threads = active;
    true
}

pub(crate) fn thread_budget_proof_complete() -> bool {
    let Some(manager) = global_manager() else {
        return false;
    };
    let Some(control) = manager.control_plane_id.and_then(|id| manager.domain(id)) else {
        return false;
    };
    control.active_threads == 0
        && control.thread_limit == BOOTSTRAP_THREAD_LIMIT
        && manager.total_thread_attaches == 3
}

pub(crate) fn is_service_domain(domain_raw: u64) -> bool {
    let Some(id) = decode_id(domain_raw) else {
        return false;
    };
    global_manager()
        .and_then(|manager| manager.domain(id))
        .is_some_and(|domain| domain.role == Role::Service)
}

pub(crate) fn retire_service(domain_raw: u64) -> bool {
    let Some(id) = decode_id(domain_raw) else {
        return false;
    };
    let Some(manager) = global_manager_mut() else {
        return false;
    };
    manager.retire_service(id)
}

pub(crate) fn record_service_fault(domain_raw: u64) -> bool {
    let Some(id) = decode_id(domain_raw) else {
        return false;
    };
    let Some(manager) = global_manager_mut() else {
        return false;
    };
    if manager.service_id != Some(id)
        || manager
            .domain(id)
            .is_none_or(|domain| domain.role != Role::Service)
    {
        return false;
    }
    manager.lifecycle_events.push(LifecycleEvent {
        kind: 1,
        domain: id,
    })
}

pub(crate) fn take_service_fault() -> Option<u64> {
    let manager = global_manager_mut()?;
    let event = manager.lifecycle_events.pop()?;
    manager.last_delivered_service_fault = Some(event.domain);
    Some((encode_id(event.domain) << 8) | u64::from(event.kind))
}

pub(crate) fn service_fault_was_delivered(domain_raw: u64) -> bool {
    let Some(id) = decode_id(domain_raw) else {
        return false;
    };
    global_manager().is_some_and(|manager| {
        manager.lifecycle_events.is_empty() && manager.last_delivered_service_fault == Some(id)
    })
}

pub(crate) fn verify_lifecycle_event_invariants() -> bool {
    let first = LifecycleEvent {
        kind: 1,
        domain: DomainId {
            slot: 1,
            generation: 7,
        },
    };
    let mut queue = EventQueue::new();
    queue.pop().is_none()
        && queue.push(first)
        && queue.push(LifecycleEvent {
            kind: 2,
            domain: first.domain,
        })
        && queue.push(LifecycleEvent {
            kind: 3,
            domain: first.domain,
        })
        && queue.push(LifecycleEvent {
            kind: 4,
            domain: first.domain,
        })
        && !queue.push(LifecycleEvent {
            kind: 5,
            domain: first.domain,
        })
        && queue.pop() == Some(first)
        && queue.push(LifecycleEvent {
            kind: 5,
            domain: first.domain,
        })
        && queue.pop().is_some_and(|event| event.kind == 2)
        && queue.pop().is_some_and(|event| event.kind == 3)
        && queue.pop().is_some_and(|event| event.kind == 4)
        && queue.pop().is_some_and(|event| event.kind == 5)
        && queue.pop().is_none()
        && queue.is_empty()
        && queue.events.iter().all(Option::is_none)
}

pub(crate) fn domain_is_retired(domain_raw: u64) -> bool {
    let Some(id) = decode_id(domain_raw) else {
        return false;
    };
    global_manager().is_some_and(|manager| manager.domain(id).is_none())
}

pub(crate) fn last_retired_service_domain() -> Option<u64> {
    Some(encode_id(global_manager()?.last_retired_service_id?))
}

fn global_manager() -> Option<&'static Manager> {
    if !INITIALIZED.load(Ordering::Acquire) {
        return None;
    }
    unsafe { Some(&*core::ptr::addr_of!(GLOBAL_MANAGER).cast::<Manager>()) }
}

fn global_manager_mut() -> Option<&'static mut Manager> {
    if !INITIALIZED.load(Ordering::Acquire) {
        return None;
    }
    unsafe { Some(&mut *core::ptr::addr_of_mut!(GLOBAL_MANAGER).cast::<Manager>()) }
}

const fn encode_id(id: DomainId) -> u64 {
    (id.generation as u64) << 16 | id.slot as u64
}

const fn decode_id(raw: u64) -> Option<DomainId> {
    if raw >> 48 != 0 {
        return None;
    }
    let id = DomainId {
        slot: raw as u16,
        generation: (raw >> 16) as u32,
    };
    if id.generation == 0 || id.slot as usize >= MAX_DOMAINS {
        return None;
    }
    Some(id)
}
