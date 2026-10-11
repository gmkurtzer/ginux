use crate::capability::{Handle, Rights, Table as CapabilityTable};
use crate::manifest::Manifest;
use core::mem::MaybeUninit;
use core::sync::atomic::{AtomicBool, Ordering};

const MAX_DOMAINS: usize = 4;
const MAX_BOOT_GRANTS: usize = 8;
const CONTROL_CALL_BUDGET: u16 = 22;
const SERVICE_CALL_BUDGET: u16 = 3;
const BOOTSTRAP_THREAD_LIMIT: u16 = 1;
const BOOTSTRAP_OPERATION_LIMIT: u16 = 1;
const SERVICE_RESTART_BUDGET: u16 = 1;
const EVENT_CAPACITY: usize = 4;
const MAX_AUXILIARY_SERVICES: usize = MAX_DOMAINS - 2;
const MAX_SERVICE_IDENTITIES: usize = 1 + MAX_AUXILIARY_SERVICES;

static INITIALIZED: AtomicBool = AtomicBool::new(false);
static mut GLOBAL_MANAGER: MaybeUninit<Manager> = MaybeUninit::uninit();

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DomainId {
    slot: u16,
    generation: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ServiceId {
    slot: u16,
    generation: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Role {
    ControlPlane,
    SupervisedService,
    AuxiliaryService,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum State {
    Ready,
    Stopping,
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
    service: Option<ServiceId>,
    role: Role,
    state: State,
    capabilities: CapabilityTable,
    boot_roots: [Option<Handle>; MAX_BOOT_GRANTS],
    boot_root_count: usize,
    remaining_calls: u16,
    active_threads: u16,
    thread_limit: u16,
    active_operations: u16,
    operation_limit: u16,
    parent: Option<DomainId>,
    delegated_roots: [Option<DelegatedRoot>; MAX_BOOT_GRANTS],
}

#[derive(Clone, Copy)]
struct DelegatedRoot {
    local: Handle,
    domain: DomainId,
    source: Handle,
}

#[derive(Clone, Copy)]
struct DelegationGrant {
    source: Handle,
    object: u64,
    rights: Rights,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct LifecycleEvent {
    kind: u8,
    domain: DomainId,
    service: Option<ServiceId>,
    detail: u8,
    fault: Option<FaultMetadata>,
    sequence: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct FaultMetadata {
    pub(crate) vector: u64,
    pub(crate) error: u64,
    pub(crate) address: u64,
    pub(crate) instruction: u64,
}

#[derive(Clone, Copy)]
struct EventOverflow {
    first_domain: DomainId,
    first_service: Option<ServiceId>,
    first_sequence: u64,
    dropped: u8,
}

struct EventQueue {
    events: [Option<LifecycleEvent>; EVENT_CAPACITY],
    head: usize,
    len: usize,
    overflow: Option<EventOverflow>,
    next_sequence: u64,
    last_delivered_sequence: u64,
}

impl EventQueue {
    const fn new() -> Self {
        Self {
            events: [None; EVENT_CAPACITY],
            head: 0,
            len: 0,
            overflow: None,
            next_sequence: 1,
            last_delivered_sequence: 0,
        }
    }

    fn push(&mut self, mut event: LifecycleEvent) -> bool {
        let Some(next) = self.next_sequence.checked_add(1) else {
            return false;
        };
        event.sequence = self.next_sequence;
        if let Some(overflow) = &mut self.overflow {
            overflow.dropped = overflow.dropped.saturating_add(1);
            self.next_sequence = next;
            return true;
        }
        if self.len == EVENT_CAPACITY {
            self.overflow = Some(EventOverflow {
                first_domain: event.domain,
                first_service: event.service,
                first_sequence: event.sequence,
                dropped: 1,
            });
            self.next_sequence = next;
            return true;
        }
        let tail = (self.head + self.len) % EVENT_CAPACITY;
        self.events[tail] = Some(event);
        self.len += 1;
        self.next_sequence = next;
        true
    }

    fn pop(&mut self) -> Option<LifecycleEvent> {
        let event = if self.len == 0 {
            let overflow = self.overflow.take()?;
            LifecycleEvent {
                kind: crate::control_abi::LIFECYCLE_OVERFLOW,
                domain: overflow.first_domain,
                service: overflow.first_service,
                detail: overflow.dropped,
                fault: None,
                sequence: overflow.first_sequence,
            }
        } else {
            let event = self.events[self.head].take()?;
            self.head = (self.head + 1) % EVENT_CAPACITY;
            self.len -= 1;
            event
        };
        if event.sequence == 0 || event.sequence <= self.last_delivered_sequence {
            return None;
        }
        self.last_delivered_sequence = event.sequence;
        Some(event)
    }

    const fn is_empty(&self) -> bool {
        self.len == 0 && self.overflow.is_none()
    }
}

pub(crate) struct Manager {
    domains: [Option<Domain>; MAX_DOMAINS],
    generations: [u32; MAX_DOMAINS],
    service_generations: [u32; MAX_SERVICE_IDENTITIES],
    control_plane_created: bool,
    control_plane_id: Option<DomainId>,
    service_id: Option<DomainId>,
    primary_service_identity: Option<ServiceId>,
    auxiliary_service_ids: [Option<DomainId>; MAX_AUXILIARY_SERVICES],
    last_retired_service_id: Option<DomainId>,
    lifecycle_events: EventQueue,
    last_delivered_lifecycle_event: Option<LifecycleEvent>,
    total_calls: u16,
    total_thread_attaches: u16,
    service_started: bool,
    remaining_service_restarts: u16,
    total_service_restarts: u16,
    quarantined_frame_owner: Option<DomainId>,
}

impl Manager {
    pub(crate) const fn new() -> Self {
        Self {
            domains: [const { None }; MAX_DOMAINS],
            generations: [0; MAX_DOMAINS],
            service_generations: [0; MAX_SERVICE_IDENTITIES],
            control_plane_created: false,
            control_plane_id: None,
            service_id: None,
            primary_service_identity: None,
            auxiliary_service_ids: [None; MAX_AUXILIARY_SERVICES],
            last_retired_service_id: None,
            lifecycle_events: EventQueue::new(),
            last_delivered_lifecycle_event: None,
            total_calls: 0,
            total_thread_attaches: 0,
            service_started: false,
            remaining_service_restarts: SERVICE_RESTART_BUDGET,
            total_service_restarts: 0,
            quarantined_frame_owner: None,
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
            service: None,
            role: Role::ControlPlane,
            state: State::Ready,
            capabilities,
            boot_roots,
            boot_root_count: grants.len(),
            remaining_calls: CONTROL_CALL_BUDGET,
            active_threads: 0,
            thread_limit: BOOTSTRAP_THREAD_LIMIT,
            active_operations: 0,
            operation_limit: BOOTSTRAP_OPERATION_LIMIT,
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

    fn next_service_identity(&self, slot: usize) -> Option<ServiceId> {
        let generation = self.service_generations.get(slot)?.checked_add(1)?;
        Some(ServiceId {
            slot: u16::try_from(slot).ok()?,
            generation,
        })
    }

    fn create_service(&mut self) -> Option<DomainId> {
        const IPC_OBJECT: u64 = 3;
        let rights = Rights::READ.union(Rights::WRITE);
        let control_id = self.control_plane_id?;
        let control = self.domain(control_id)?;
        let source = control.boot_roots.get(2).copied().flatten()?;
        self.create_service_with_delegations(&[DelegationGrant {
            source,
            object: IPC_OBJECT,
            rights,
        }])
    }

    fn create_service_with_delegations(&mut self, grants: &[DelegationGrant]) -> Option<DomainId> {
        if grants.is_empty() || grants.len() > MAX_BOOT_GRANTS || self.service_id.is_some() {
            return None;
        }
        let new_identity = self.primary_service_identity.is_none();
        let identity = if let Some(identity) = self.primary_service_identity {
            identity
        } else {
            self.next_service_identity(0)?
        };
        let id = self.construct_service_domain(grants, Role::SupervisedService, identity)?;
        if new_identity {
            self.service_generations[0] = identity.generation;
            self.primary_service_identity = Some(identity);
        }
        self.service_id = Some(id);
        Some(id)
    }

    fn create_auxiliary_service_with_delegations(
        &mut self,
        grants: &[DelegationGrant],
    ) -> Option<DomainId> {
        let registry_slot = self
            .auxiliary_service_ids
            .iter()
            .position(Option::is_none)?;
        let identity_slot = registry_slot.checked_add(1)?;
        let identity = self.next_service_identity(identity_slot)?;
        let id = self.construct_service_domain(grants, Role::AuxiliaryService, identity)?;
        self.service_generations[identity_slot] = identity.generation;
        self.auxiliary_service_ids[registry_slot] = Some(id);
        Some(id)
    }

    fn construct_service_domain(
        &mut self,
        grants: &[DelegationGrant],
        role: Role,
        identity: ServiceId,
    ) -> Option<DomainId> {
        if grants.is_empty() || grants.len() > MAX_BOOT_GRANTS {
            return None;
        }
        if !matches!(role, Role::SupervisedService | Role::AuxiliaryService) {
            return None;
        }
        let control_id = self.control_plane_id?;
        let control = self.domain(control_id)?;
        if grants.iter().any(|grant| {
            grant.object == 0
                || !control
                    .capabilities
                    .permits(grant.source, grant.object, grant.rights)
        }) {
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
        let mut delegated_roots = [None; MAX_BOOT_GRANTS];
        for (index, grant) in grants.iter().enumerate() {
            let root = capabilities.accept_delegated(grant.object, grant.rights)?;
            boot_roots[index] = Some(root);
            delegated_roots[index] = Some(DelegatedRoot {
                local: root,
                domain: control_id,
                source: grant.source,
            });
        }
        capabilities.seal_boot_authority();
        self.generations[slot] = generation;
        self.domains[slot] = Some(Domain {
            id,
            service: Some(identity),
            role,
            state: State::Ready,
            capabilities,
            boot_roots,
            boot_root_count: grants.len(),
            remaining_calls: SERVICE_CALL_BUDGET,
            active_threads: 0,
            thread_limit: if role == Role::SupervisedService {
                BOOTSTRAP_THREAD_LIMIT
            } else {
                0
            },
            active_operations: 0,
            operation_limit: BOOTSTRAP_OPERATION_LIMIT,
            parent: Some(control_id),
            delegated_roots,
        });
        Some(id)
    }

    fn create_initial_service(&mut self) -> Option<DomainId> {
        if self.service_started {
            return None;
        }
        let id = self.create_service()?;
        self.service_started = true;
        Some(id)
    }

    fn restart_service(&mut self) -> Option<DomainId> {
        if !self.service_started {
            return None;
        }
        let remaining = self.remaining_service_restarts.checked_sub(1)?;
        let total = self.total_service_restarts.checked_add(1)?;
        let id = self.create_service()?;
        self.remaining_service_restarts = remaining;
        self.total_service_restarts = total;
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

    fn begin_operation(&mut self, id: DomainId) -> bool {
        let Some(domain) = self
            .domains
            .get_mut(usize::from(id.slot))
            .and_then(Option::as_mut)
            .filter(|domain| domain.id == id)
        else {
            return false;
        };
        if domain.state != State::Ready || domain.active_operations >= domain.operation_limit {
            return false;
        }
        domain.active_operations += 1;
        true
    }

    fn end_operation(&mut self, id: DomainId) -> bool {
        let Some(domain) = self
            .domains
            .get_mut(usize::from(id.slot))
            .and_then(Option::as_mut)
            .filter(|domain| domain.id == id)
        else {
            return false;
        };
        let Some(active) = domain.active_operations.checked_sub(1) else {
            return false;
        };
        domain.active_operations = active;
        true
    }

    fn attach_thread(&mut self, id: DomainId) -> bool {
        let Some(total) = self.total_thread_attaches.checked_add(1) else {
            return false;
        };
        let Some(domain) = self
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
        self.total_thread_attaches = total;
        true
    }

    fn detach_thread(&mut self, id: DomainId) -> bool {
        let Some(domain) = self
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

    fn retire_service(&mut self, id: DomainId) -> bool {
        if self.service_id != Some(id)
            || self.domain(id).is_none_or(|domain| {
                domain.role != Role::SupervisedService
                    || domain.active_threads != 0
                    || domain.active_operations != 0
            })
        {
            return false;
        }
        self.domains[usize::from(id.slot)] = None;
        self.service_id = None;
        self.last_retired_service_id = Some(id);
        true
    }

    fn discard_unstarted_service(&mut self, id: DomainId) -> bool {
        if self.service_id != Some(id)
            || self.domain(id).is_none_or(|domain| {
                domain.role != Role::SupervisedService
                    || domain.active_threads != 0
                    || domain.active_operations != 0
            })
        {
            return false;
        }
        self.domains[usize::from(id.slot)] = None;
        self.service_id = None;
        true
    }

    fn retire_auxiliary_service(&mut self, id: DomainId) -> bool {
        let Some(registry_slot) = self
            .auxiliary_service_ids
            .iter()
            .position(|registered| *registered == Some(id))
        else {
            return false;
        };
        let Some(service) = self
            .domain(id)
            .filter(|domain| {
                domain.role == Role::AuxiliaryService
                    && domain.state == State::Stopping
                    && domain.active_threads == 0
                    && domain.active_operations == 0
            })
            .and_then(|domain| domain.service)
        else {
            return false;
        };
        if !self.lifecycle_events.push(LifecycleEvent {
            kind: crate::control_abi::LIFECYCLE_AUXILIARY_STOPPED,
            domain: id,
            service: Some(service),
            detail: 0,
            fault: None,
            sequence: 0,
        }) {
            return false;
        }
        self.domains[usize::from(id.slot)] = None;
        self.auxiliary_service_ids[registry_slot] = None;
        true
    }

    fn request_auxiliary_stop(
        &mut self,
        supervisor: DomainId,
        capability: Handle,
        id: DomainId,
    ) -> bool {
        const HOST_CONTROL: u64 = 1;
        if self.control_plane_id != Some(supervisor)
            || !self.permits(supervisor, capability, HOST_CONTROL, Rights::MANAGE)
        {
            return false;
        }
        if !self.auxiliary_service_ids.contains(&Some(id)) {
            return false;
        }
        let Some(domain) = self
            .domains
            .get_mut(usize::from(id.slot))
            .and_then(Option::as_mut)
            .filter(|domain| domain.id == id)
        else {
            return false;
        };
        if domain.role != Role::AuxiliaryService || domain.state != State::Ready {
            return false;
        }
        if domain.boot_root_count == 0
            || domain.boot_roots[..domain.boot_root_count]
                .iter()
                .any(|root| {
                    root.is_none_or(|root| domain.capabilities.authority_root(root) != Some(root))
                })
        {
            return false;
        }
        for root in domain.boot_roots[..domain.boot_root_count].iter().flatten() {
            if !domain.capabilities.revoke(*root) {
                return false;
            }
        }
        domain.state = State::Stopping;
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
    let id = manager.create_initial_service()?;
    validate_boot_service(manager, id)
}

pub(crate) fn initialize_boot_service_restart() -> Option<u64> {
    let manager = global_manager_mut()?;
    let quarantined = manager.quarantined_frame_owner?;
    let owner = crate::memory::FrameOwner::domain(encode_id(quarantined))?;
    if crate::memory::owner_live_frame_count(owner) != Some(7) {
        return None;
    }
    let id = manager.restart_service()?;
    validate_boot_service(manager, id)
}

fn validate_boot_service(manager: &Manager, id: DomainId) -> Option<u64> {
    let service = manager.domain(id)?;
    (service.role == Role::SupervisedService
        && service.service == manager.primary_service_identity
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

pub(crate) fn primary_service_identity() -> Option<u64> {
    Some(encode_service_id(
        global_manager()?.primary_service_identity?,
    ))
}

pub(crate) fn domain_service_identity(domain_raw: u64) -> Option<u64> {
    let id = decode_id(domain_raw)?;
    Some(encode_service_id(global_manager()?.domain(id)?.service?))
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
    let parent_handle = manager.domain(control)?.boot_roots[2]?;
    let unrelated_source = manager.domain(control)?.boot_roots[0]?;
    let console_source = manager.domain(control)?.boot_roots[1]?;
    let rights = Rights::READ.union(Rights::WRITE);
    if manager
        .create_service_with_delegations(&[
            DelegationGrant {
                source: parent_handle,
                object: 3,
                rights,
            },
            DelegationGrant {
                source: console_source,
                object: 2,
                rights: Rights::READ,
            },
        ])
        .is_some()
        || manager.service_id.is_some()
    {
        return Some(false);
    }
    let service = manager.create_service_with_delegations(&[
        DelegationGrant {
            source: parent_handle,
            object: 3,
            rights,
        },
        DelegationGrant {
            source: unrelated_source,
            object: 1,
            rights: Rights::READ,
        },
    ])?;
    let service_handle = manager.domain(service)?.boot_roots[0]?;
    let auxiliary_handle = manager.domain(service)?.boot_roots[1]?;
    let service_read = manager.domains[usize::from(service.slot)]
        .as_mut()?
        .capabilities
        .derive(service_handle, Rights::READ)?;
    if !manager.permits(service, service_handle, 3, rights)
        || !manager.permits(service, service_read, 3, Rights::READ)
        || manager.permits(service, service_read, 3, Rights::WRITE)
        || !manager.permits(service, auxiliary_handle, 1, Rights::READ)
        || manager.permits(service, auxiliary_handle, 1, Rights::MANAGE)
    {
        return Some(false);
    }
    for request_bits in 1..=0x0f {
        let requested = Rights::from_bits(request_bits)?;
        let root_expected = request_bits & 0x03 == request_bits;
        let read_expected = request_bits == 0x01;
        if manager.permits(service, service_handle, 3, requested) != root_expected
            || manager.permits(service, service_read, 3, requested) != read_expected
            || manager.permits(service, service_handle, 2, requested)
        {
            return Some(false);
        }
    }
    manager.domains[usize::from(service.slot)]
        .as_mut()?
        .delegated_roots[0]
        .as_mut()?
        .source = unrelated_source;
    if manager.permits(service, service_handle, 3, Rights::READ) {
        return Some(false);
    }
    manager.domains[usize::from(service.slot)]
        .as_mut()?
        .delegated_roots[0]
        .as_mut()?
        .source = parent_handle;
    manager.domains[usize::from(control.slot)]
        .as_mut()?
        .capabilities
        .revoke(parent_handle);
    let replacement_source = manager.domains[usize::from(control.slot)]
        .as_mut()?
        .capabilities
        .derive(unrelated_source, Rights::READ)?;
    Some(
        replacement_source != parent_handle
            && manager.permits(control, replacement_source, 1, Rights::READ)
            && manager
                .domain(service)?
                .capabilities
                .permits(service_handle, 3, rights)
            && !manager.permits(service, service_handle, 3, rights)
            && !manager.permits(service, service_read, 3, Rights::READ)
            && manager.permits(service, auxiliary_handle, 1, Rights::READ)
            && !manager.permits(service, auxiliary_handle, 1, Rights::MANAGE),
    )
}

pub(crate) fn verify_domain_ancestry_invariants() -> bool {
    verify_domain_ancestry_invariants_inner().unwrap_or(false)
}

pub(crate) fn verify_auxiliary_service_registry_invariants() -> bool {
    verify_auxiliary_service_registry_invariants_inner().unwrap_or(false)
}

pub(crate) fn verify_auxiliary_retirement_overflow_invariants() -> bool {
    verify_auxiliary_retirement_overflow_invariants_inner().unwrap_or(false)
}

pub(crate) fn verify_auxiliary_stop_revocation_invariants() -> bool {
    verify_auxiliary_stop_revocation_invariants_inner().unwrap_or(false)
}

fn verify_auxiliary_stop_revocation_invariants_inner() -> Option<bool> {
    let grants = [
        BootGrant::new(1, Rights::READ.union(Rights::MANAGE)),
        BootGrant::new(2, Rights::WRITE),
        BootGrant::new(3, Rights::READ.union(Rights::WRITE)),
    ];
    let mut manager = Manager::new();
    let control = manager.create_control_plane(&grants)?;
    let host_source = manager.domain(control)?.boot_roots[0]?;
    let console_source = manager.domain(control)?.boot_roots[1]?;
    let auxiliary = manager.create_auxiliary_service_with_delegations(&[
        DelegationGrant {
            source: console_source,
            object: 2,
            rights: Rights::WRITE,
        },
        DelegationGrant {
            source: host_source,
            object: 1,
            rights: Rights::READ,
        },
    ])?;
    let console = manager.domain(auxiliary)?.boot_roots[0]?;
    let host = manager.domain(auxiliary)?.boot_roots[1]?;
    if !manager.permits(auxiliary, console, 2, Rights::WRITE)
        || !manager.permits(auxiliary, host, 1, Rights::READ)
        || !manager.request_auxiliary_stop(control, host_source, auxiliary)
    {
        return Some(false);
    }
    Some(
        !manager.permits(auxiliary, console, 2, Rights::WRITE)
            && !manager.permits(auxiliary, host, 1, Rights::READ)
            && !manager.begin_operation(auxiliary)
            && manager.domain(auxiliary)?.state == State::Stopping
            && manager.retire_auxiliary_service(auxiliary),
    )
}

fn verify_auxiliary_retirement_overflow_invariants_inner() -> Option<bool> {
    let grants = [
        BootGrant::new(1, Rights::READ.union(Rights::MANAGE)),
        BootGrant::new(2, Rights::WRITE),
        BootGrant::new(3, Rights::READ.union(Rights::WRITE)),
    ];
    let mut manager = Manager::new();
    let control = manager.create_control_plane(&grants)?;
    let host_source = manager.domain(control)?.boot_roots[0]?;
    let auxiliary = manager.create_auxiliary_service_with_delegations(&[DelegationGrant {
        source: host_source,
        object: 1,
        rights: Rights::READ,
    }])?;
    let auxiliary_identity = manager.domain(auxiliary)?.service?;
    for kind in 10..14 {
        if !manager.lifecycle_events.push(LifecycleEvent {
            kind,
            domain: control,
            service: None,
            detail: 0,
            fault: None,
            sequence: 0,
        }) {
            return Some(false);
        }
    }
    if !manager.request_auxiliary_stop(control, host_source, auxiliary)
        || !manager.retire_auxiliary_service(auxiliary)
        || manager.domain(auxiliary).is_some()
    {
        return Some(false);
    }
    for sequence in 1..=4 {
        let event = manager.lifecycle_events.pop()?;
        if event.domain != control || event.sequence != sequence {
            return Some(false);
        }
    }
    let overflow = manager.lifecycle_events.pop()?;
    Some(
        overflow.kind == crate::control_abi::LIFECYCLE_OVERFLOW
            && overflow.domain == auxiliary
            && overflow.service == Some(auxiliary_identity)
            && overflow.detail == 1
            && overflow.fault.is_none()
            && overflow.sequence == 5
            && manager.lifecycle_events.is_empty(),
    )
}

fn verify_auxiliary_service_registry_invariants_inner() -> Option<bool> {
    let grants = [
        BootGrant::new(1, Rights::READ.union(Rights::MANAGE)),
        BootGrant::new(2, Rights::WRITE),
        BootGrant::new(3, Rights::READ.union(Rights::WRITE)),
    ];
    let mut manager = Manager::new();
    let control = manager.create_control_plane(&grants)?;
    let primary = manager.create_service()?;
    let console_source = manager.domain(control)?.boot_roots[1]?;
    let host_source = manager.domain(control)?.boot_roots[0]?;
    let console_grant = DelegationGrant {
        source: console_source,
        object: 2,
        rights: Rights::WRITE,
    };
    let console_service = manager.create_auxiliary_service_with_delegations(&[console_grant])?;
    let host_service = manager.create_auxiliary_service_with_delegations(&[DelegationGrant {
        source: host_source,
        object: 1,
        rights: Rights::READ,
    }])?;
    let primary_identity = manager.domain(primary)?.service?;
    let console_identity = manager.domain(console_service)?.service?;
    let host_identity = manager.domain(host_service)?.service?;
    let console_handle = manager.domain(console_service)?.boot_roots[0]?;
    let host_handle = manager.domain(host_service)?.boot_roots[0]?;
    if manager.service_id != Some(primary)
        || manager.domain(primary)?.role != Role::SupervisedService
        || Some(primary_identity) != manager.primary_service_identity
        || manager.domain(primary)?.thread_limit != BOOTSTRAP_THREAD_LIMIT
        || manager.domain(console_service)?.role != Role::AuxiliaryService
        || console_identity.slot == host_identity.slot
        || manager.domain(console_service)?.thread_limit != 0
        || manager.domain(host_service)?.role != Role::AuxiliaryService
        || manager.domain(host_service)?.thread_limit != 0
        || manager.attach_thread(console_service)
        || manager
            .create_auxiliary_service_with_delegations(&[console_grant])
            .is_some()
        || manager.auxiliary_service_ids != [Some(console_service), Some(host_service)]
        || !manager.is_descendant(control, console_service)
        || !manager.is_descendant(control, host_service)
        || !manager.permits(console_service, console_handle, 2, Rights::WRITE)
        || manager.permits(console_service, console_handle, 2, Rights::READ)
        || !manager.permits(host_service, host_handle, 1, Rights::READ)
        || manager.permits(host_service, host_handle, 1, Rights::MANAGE)
        || !manager.begin_operation(console_service)
        || manager.retire_auxiliary_service(console_service)
        || manager.request_auxiliary_stop(console_service, console_handle, console_service)
        || manager.request_auxiliary_stop(control, console_source, console_service)
        || !manager.request_auxiliary_stop(control, host_source, console_service)
        || manager.request_auxiliary_stop(control, host_source, console_service)
        || manager.begin_operation(console_service)
        || manager.retire_auxiliary_service(console_service)
        || !manager.end_operation(console_service)
        || manager.retire_service(console_service)
        || !manager.retire_auxiliary_service(console_service)
        || manager.domain(console_service).is_some()
        || manager.request_auxiliary_stop(control, host_source, console_service)
        || manager.is_descendant(control, console_service)
        || manager.service_id != Some(primary)
        || !manager.permits(host_service, host_handle, 1, Rights::READ)
    {
        return Some(false);
    }
    let replacement = manager.create_auxiliary_service_with_delegations(&[console_grant])?;
    let replacement_handle = manager.domain(replacement)?.boot_roots[0]?;
    let replacement_identity = manager.domain(replacement)?.service?;
    if replacement == console_service
        || replacement.slot != console_service.slot
        || replacement_identity.slot != console_identity.slot
        || replacement_identity.generation <= console_identity.generation
        || manager.auxiliary_service_ids != [Some(replacement), Some(host_service)]
        || !manager.permits(replacement, replacement_handle, 2, Rights::WRITE)
        || manager.permits(console_service, console_handle, 2, Rights::WRITE)
        || manager.retire_auxiliary_service(replacement)
        || !manager.request_auxiliary_stop(control, host_source, replacement)
        || !manager.retire_auxiliary_service(replacement)
        || !manager.permits(
            primary,
            manager.domain(primary)?.boot_roots[0]?,
            3,
            Rights::READ,
        )
    {
        return Some(false);
    }
    let first_retirement = manager.lifecycle_events.pop()?;
    let second_retirement = manager.lifecycle_events.pop()?;
    if first_retirement.kind != crate::control_abi::LIFECYCLE_AUXILIARY_STOPPED
        || first_retirement.domain != console_service
        || first_retirement.service != Some(console_identity)
        || first_retirement.detail != 0
        || first_retirement.fault.is_some()
        || first_retirement.sequence != 1
        || second_retirement.kind != crate::control_abi::LIFECYCLE_AUXILIARY_STOPPED
        || second_retirement.domain != replacement
        || second_retirement.service != Some(replacement_identity)
        || second_retirement.detail != 0
        || second_retirement.fault.is_some()
        || second_retirement.sequence != 2
        || !manager.lifecycle_events.is_empty()
        || !manager.request_auxiliary_stop(control, host_source, host_service)
    {
        return Some(false);
    }
    manager.lifecycle_events.next_sequence = u64::MAX;
    Some(
        !manager.retire_auxiliary_service(host_service)
            && manager.auxiliary_service_ids == [None, Some(host_service)]
            && manager.domain(host_service)?.state == State::Stopping
            && manager.lifecycle_events.is_empty()
            && manager.lifecycle_events.next_sequence == u64::MAX,
    )
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
        || !manager.begin_operation(service)
        || manager.begin_operation(service)
        || manager.retire_service(service)
        || !manager.end_operation(service)
        || !manager.retire_service(service)
        || manager.is_descendant(control, service)
    {
        return Some(false);
    }
    let replacement = manager.create_service()?;
    Some(replacement != service && manager.is_descendant(control, replacement))
}

pub(crate) fn verify_service_restart_budget_invariants() -> bool {
    verify_service_restart_budget_invariants_inner().unwrap_or(false)
}

fn verify_service_restart_budget_invariants_inner() -> Option<bool> {
    let grants = [
        BootGrant::new(1, Rights::READ.union(Rights::MANAGE)),
        BootGrant::new(2, Rights::WRITE),
        BootGrant::new(3, Rights::READ.union(Rights::WRITE)),
    ];
    let mut manager = Manager::new();
    manager.create_control_plane(&grants)?;
    let service = manager.create_initial_service()?;
    let service_identity = manager.domain(service)?.service?;
    if manager.create_initial_service().is_some() || manager.restart_service().is_some() {
        return Some(false);
    }
    if !manager.retire_service(service) {
        return Some(false);
    }
    let replacement = manager.restart_service()?;
    let replacement_identity = manager.domain(replacement)?.service?;
    if replacement == service
        || replacement_identity != service_identity
        || !manager.discard_unstarted_service(replacement)
    {
        return Some(false);
    }
    Some(
        manager.restart_service().is_none()
            && manager.create_initial_service().is_none()
            && manager.domain(replacement).is_none()
            && manager.last_retired_service_id == Some(service)
            && manager.primary_service_identity == Some(service_identity)
            && manager.remaining_service_restarts == 0
            && manager.total_service_restarts == SERVICE_RESTART_BUDGET,
    )
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

pub(crate) fn begin_operation(domain_raw: u64) -> bool {
    let Some(id) = decode_id(domain_raw) else {
        return false;
    };
    global_manager_mut().is_some_and(|manager| manager.begin_operation(id))
}

pub(crate) fn end_operation(domain_raw: u64) -> bool {
    let Some(id) = decode_id(domain_raw) else {
        return false;
    };
    global_manager_mut().is_some_and(|manager| manager.end_operation(id))
}

pub(crate) fn operation_accounting_proof_complete() -> bool {
    global_manager().is_some_and(|manager| {
        manager
            .domains
            .iter()
            .flatten()
            .all(|domain| domain.active_operations == 0)
    })
}

pub(crate) fn call_budget_proof_complete() -> bool {
    let Some(manager) = global_manager() else {
        return false;
    };
    let Some(control) = manager.control_plane_id.and_then(|id| manager.domain(id)) else {
        return false;
    };
    control.remaining_calls == 0 && manager.total_calls == 27
}

pub(crate) fn attach_thread(domain_raw: u64) -> bool {
    let Some(id) = decode_id(domain_raw) else {
        return false;
    };
    global_manager_mut().is_some_and(|manager| manager.attach_thread(id))
}

pub(crate) fn detach_thread(domain_raw: u64) -> bool {
    let Some(id) = decode_id(domain_raw) else {
        return false;
    };
    global_manager_mut().is_some_and(|manager| manager.detach_thread(id))
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

pub(crate) fn service_restart_budget_proof_complete() -> bool {
    global_manager().is_some_and(|manager| {
        manager.service_started
            && manager.remaining_service_restarts == 0
            && manager.total_service_restarts == SERVICE_RESTART_BUDGET
    })
}

pub(crate) fn is_supervised_service_domain(domain_raw: u64) -> bool {
    let Some(id) = decode_id(domain_raw) else {
        return false;
    };
    global_manager()
        .and_then(|manager| manager.domain(id))
        .is_some_and(|domain| domain.role == Role::SupervisedService)
}

pub(crate) fn retire_service(domain_raw: u64) -> bool {
    let Some(id) = decode_id(domain_raw) else {
        return false;
    };
    let Some(owner) = crate::memory::FrameOwner::domain(domain_raw) else {
        return false;
    };
    if crate::memory::owner_live_frame_count(owner) != Some(7) {
        return false;
    }
    let Some(manager) = global_manager_mut() else {
        return false;
    };
    if manager.quarantined_frame_owner.is_some() || !manager.retire_service(id) {
        return false;
    }
    manager.quarantined_frame_owner = Some(id);
    true
}

pub(crate) fn adopt_quarantined_service_frames(retired_raw: u64, replacement_raw: u64) -> bool {
    let (Some(retired), Some(replacement), Some(retired_owner), Some(replacement_owner)) = (
        decode_id(retired_raw),
        decode_id(replacement_raw),
        crate::memory::FrameOwner::domain(retired_raw),
        crate::memory::FrameOwner::domain(replacement_raw),
    ) else {
        return false;
    };
    let Some(manager) = global_manager_mut() else {
        return false;
    };
    if manager.quarantined_frame_owner != Some(retired)
        || manager.service_id != Some(replacement)
        || crate::memory::owner_live_frame_count(retired_owner) != Some(0)
        || crate::memory::owner_live_frame_count(replacement_owner) != Some(7)
    {
        return false;
    }
    manager.quarantined_frame_owner = None;
    true
}

pub(crate) fn restore_quarantined_service_frames(retired_raw: u64, replacement_raw: u64) -> bool {
    let (Some(retired), Some(replacement), Some(retired_owner), Some(replacement_owner)) = (
        decode_id(retired_raw),
        decode_id(replacement_raw),
        crate::memory::FrameOwner::domain(retired_raw),
        crate::memory::FrameOwner::domain(replacement_raw),
    ) else {
        return false;
    };
    let Some(manager) = global_manager_mut() else {
        return false;
    };
    if manager.quarantined_frame_owner.is_some()
        || manager.service_id != Some(replacement)
        || crate::memory::owner_live_frame_count(retired_owner) != Some(7)
        || crate::memory::owner_live_frame_count(replacement_owner) != Some(0)
    {
        return false;
    }
    manager.quarantined_frame_owner = Some(retired);
    true
}

pub(crate) fn complete_service_frame_quarantine(retired_raw: u64) -> bool {
    let (Some(retired), Some(owner)) = (
        decode_id(retired_raw),
        crate::memory::FrameOwner::domain(retired_raw),
    ) else {
        return false;
    };
    let Some(manager) = global_manager_mut() else {
        return false;
    };
    if manager.quarantined_frame_owner != Some(retired)
        || crate::memory::owner_live_frame_count(owner) != Some(0)
    {
        return false;
    }
    manager.quarantined_frame_owner = None;
    true
}

pub(crate) fn service_frame_quarantine_is_clear() -> bool {
    global_manager().is_some_and(|manager| manager.quarantined_frame_owner.is_none())
}

pub(crate) fn discard_unstarted_service(domain_raw: u64) -> bool {
    let Some(id) = decode_id(domain_raw) else {
        return false;
    };
    let Some(owner) = crate::memory::FrameOwner::domain(domain_raw) else {
        return false;
    };
    if crate::memory::owner_live_frame_count(owner) != Some(0) {
        return false;
    }
    let Some(manager) = global_manager_mut() else {
        return false;
    };
    manager.discard_unstarted_service(id)
}

pub(crate) fn record_service_fault(domain_raw: u64, fault: FaultMetadata) -> bool {
    let Some(id) = decode_id(domain_raw) else {
        return false;
    };
    let Some(manager) = global_manager_mut() else {
        return false;
    };
    let Some(service) = manager
        .domain(id)
        .filter(|domain| domain.role == Role::SupervisedService)
        .and_then(|domain| domain.service)
    else {
        return false;
    };
    if manager.service_id != Some(id) {
        return false;
    }
    manager.lifecycle_events.push(LifecycleEvent {
        kind: crate::control_abi::LIFECYCLE_SERVICE_FAULT,
        domain: id,
        service: Some(service),
        detail: 0,
        fault: Some(fault),
        sequence: 0,
    })
}

pub(crate) fn take_lifecycle_event() -> Option<u64> {
    let manager = global_manager_mut()?;
    let event = manager.lifecycle_events.pop()?;
    manager.last_delivered_lifecycle_event = Some(event);
    Some((u64::from(event.detail) << 56) | (encode_id(event.domain) << 8) | u64::from(event.kind))
}

pub(crate) fn service_fault_was_delivered(domain_raw: u64) -> bool {
    let Some(id) = decode_id(domain_raw) else {
        return false;
    };
    global_manager().is_some_and(|manager| {
        manager.lifecycle_events.is_empty()
            && manager.last_delivered_lifecycle_event.is_some_and(|event| {
                event.kind == crate::control_abi::LIFECYCLE_SERVICE_FAULT
                    && event.domain == id
                    && event.service == manager.primary_service_identity
                    && event.fault.is_some()
            })
    })
}

pub(crate) fn delivered_service_fault_matches(domain_raw: u64, expected: FaultMetadata) -> bool {
    let Some(id) = decode_id(domain_raw) else {
        return false;
    };
    global_manager().is_some_and(|manager| {
        manager.last_delivered_lifecycle_event.is_some_and(|event| {
            event.kind == crate::control_abi::LIFECYCLE_SERVICE_FAULT
                && event.domain == id
                && event.service == manager.primary_service_identity
                && event.detail == 0
                && event.fault == Some(expected)
        })
    })
}

pub(crate) fn delivered_service_fault_descriptor(
) -> Option<[u8; crate::control_abi::FAULT_DESCRIPTOR_SIZE]> {
    let event = global_manager()?.last_delivered_lifecycle_event?;
    if event.kind != crate::control_abi::LIFECYCLE_SERVICE_FAULT || event.detail != 0 {
        return None;
    }
    let fault = event.fault?;
    crate::control_abi::fault_descriptor(
        fault.vector,
        encode_id(event.domain),
        fault.error,
        fault.address,
        fault.instruction,
    )
}

pub(crate) fn last_lifecycle_event_audit() -> Option<(u8, u64, u64)> {
    let event = global_manager()?.last_delivered_lifecycle_event?;
    Some((event.kind, encode_id(event.domain), event.sequence))
}

pub(crate) fn last_lifecycle_service_identity() -> Option<u64> {
    Some(encode_service_id(
        global_manager()?.last_delivered_lifecycle_event?.service?,
    ))
}

pub(crate) fn delivered_lifecycle_event_descriptor(
) -> Option<[u8; crate::control_abi::LIFECYCLE_DESCRIPTOR_SIZE]> {
    let event = global_manager()?.last_delivered_lifecycle_event?;
    Some(crate::control_abi::lifecycle_descriptor(
        event.kind,
        event.detail,
        encode_id(event.domain),
        event.sequence,
    ))
}

pub(crate) fn verify_lifecycle_event_invariants() -> bool {
    let first = LifecycleEvent {
        kind: 1,
        domain: DomainId {
            slot: 1,
            generation: 7,
        },
        service: Some(ServiceId {
            slot: 0,
            generation: 3,
        }),
        detail: 0,
        fault: Some(FaultMetadata {
            vector: 14,
            error: 6,
            address: 0,
            instruction: 0x1000,
        }),
        sequence: 0,
    };
    let mut zero_sequence = EventQueue::new();
    if !zero_sequence.push(first) {
        return false;
    }
    let Some(zero_event) = zero_sequence.events[0].as_mut() else {
        return false;
    };
    zero_event.sequence = 0;
    let zero_sequence_rejected = zero_sequence.pop().is_none();
    let mut stale_sequence = EventQueue::new();
    let stale_sequence_rejected = stale_sequence.push(first) && {
        stale_sequence.last_delivered_sequence = 1;
        stale_sequence.pop().is_none()
    };
    let mut exhausted = EventQueue::new();
    exhausted.next_sequence = u64::MAX;
    let exhausted_sequence_rejected = !exhausted.push(first) && exhausted.is_empty();
    let mut queue = EventQueue::new();
    zero_sequence_rejected
        && stale_sequence_rejected
        && exhausted_sequence_rejected
        && queue.pop().is_none()
        && queue.push(first)
        && queue.push(LifecycleEvent {
            kind: 2,
            domain: first.domain,
            service: first.service,
            detail: 0,
            fault: None,
            sequence: 0,
        })
        && queue.push(LifecycleEvent {
            kind: 3,
            domain: first.domain,
            service: first.service,
            detail: 0,
            fault: None,
            sequence: 0,
        })
        && queue.push(LifecycleEvent {
            kind: 4,
            domain: first.domain,
            service: first.service,
            detail: 0,
            fault: None,
            sequence: 0,
        })
        && queue.push(LifecycleEvent {
            kind: 5,
            domain: first.domain,
            service: first.service,
            detail: 0,
            fault: None,
            sequence: 0,
        })
        && queue.push(LifecycleEvent {
            kind: 6,
            domain: DomainId {
                slot: 2,
                generation: 9,
            },
            service: Some(ServiceId {
                slot: 2,
                generation: 11,
            }),
            detail: 0,
            fault: None,
            sequence: 0,
        })
        && queue.pop().is_some_and(|event| {
            event
                == LifecycleEvent {
                    sequence: 1,
                    ..first
                }
        })
        && queue.push(LifecycleEvent {
            kind: 7,
            domain: first.domain,
            service: first.service,
            detail: 0,
            fault: None,
            sequence: 0,
        })
        && queue
            .pop()
            .is_some_and(|event| event.kind == 2 && event.sequence == 2)
        && queue
            .pop()
            .is_some_and(|event| event.kind == 3 && event.sequence == 3)
        && queue
            .pop()
            .is_some_and(|event| event.kind == 4 && event.sequence == 4)
        && queue.pop().is_some_and(|event| {
            event.kind == 2
                && event.domain == first.domain
                && event.detail == 3
                && event.sequence == 5
        })
        && queue.pop().is_none()
        && queue.is_empty()
        && queue.events.iter().all(Option::is_none)
        && queue.next_sequence == 8
        && queue.last_delivered_sequence == 5
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

const fn encode_service_id(id: ServiceId) -> u64 {
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
