use crate::capability::{Handle, Rights, Table as CapabilityTable};

const MAX_DOMAINS: usize = 4;
const MAX_BOOT_GRANTS: usize = 8;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DomainId {
    slot: u16,
    generation: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Role {
    ControlPlane,
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
}

pub(crate) struct Manager {
    domains: [Option<Domain>; MAX_DOMAINS],
    generations: [u32; MAX_DOMAINS],
    control_plane_created: bool,
}

impl Manager {
    pub(crate) const fn new() -> Self {
        Self {
            domains: [const { None }; MAX_DOMAINS],
            generations: [0; MAX_DOMAINS],
            control_plane_created: false,
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
        });
        self.control_plane_created = true;
        Some(id)
    }

    fn domain(&self, id: DomainId) -> Option<&Domain> {
        self.domains
            .get(usize::from(id.slot))?
            .as_ref()
            .filter(|domain| domain.id == id)
    }
}

pub(crate) fn create_and_verify_control_plane() -> Option<u64> {
    const HOST_CONTROL: u64 = 1;
    const CONSOLE: u64 = 2;
    let grants = [
        BootGrant::new(
            HOST_CONTROL,
            Rights::READ.union(Rights::WRITE).union(Rights::MANAGE),
        ),
        BootGrant::new(CONSOLE, Rights::WRITE),
    ];
    let mut manager = Manager::new();
    let id = manager.create_control_plane(&grants)?;
    if manager.create_control_plane(&grants).is_some() {
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
    Some((u64::from(id.generation) << 16) | u64::from(id.slot))
}
