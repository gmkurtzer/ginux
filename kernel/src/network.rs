use core::sync::atomic::{AtomicBool, AtomicU16, AtomicU64, AtomicU8, Ordering};

pub(crate) const NETWORK_NAME_SIZE: usize = 16;
const NETWORK_CAPACITY: usize = 64;
const VIRTUAL_NETWORK_CAPACITY: usize = NETWORK_CAPACITY - 1;
const ATTACHMENT_CAPACITY: usize = crate::control_abi::DOMAIN_LIMIT * 8;

static VIRTUAL_NETWORK_NAME_SIZES: [AtomicU8; VIRTUAL_NETWORK_CAPACITY] =
    [const { AtomicU8::new(0) }; VIRTUAL_NETWORK_CAPACITY];
static VIRTUAL_NETWORK_NAMES: [[AtomicU8; NETWORK_NAME_SIZE]; VIRTUAL_NETWORK_CAPACITY] =
    [const { [const { AtomicU8::new(0) }; NETWORK_NAME_SIZE] }; VIRTUAL_NETWORK_CAPACITY];
static VIRTUAL_NETWORK_UPLINKS: [AtomicU8; VIRTUAL_NETWORK_CAPACITY] =
    [const { AtomicU8::new(0) }; VIRTUAL_NETWORK_CAPACITY];
// Zero is unused. The upper byte is domain slot + 1 and the lower byte is
// network slot + 1. Attachments are resource-bounded kernel objects, not a
// singleton field on a domain.
static NETWORK_ATTACHMENTS: [AtomicU16; ATTACHMENT_CAPACITY] =
    [const { AtomicU16::new(0) }; ATTACHMENT_CAPACITY];

pub(crate) struct InventoryRecord {
    pub(crate) next_cursor: u64,
    pub(crate) kind: u16,
    pub(crate) name: [u8; NETWORK_NAME_SIZE],
    pub(crate) name_size: usize,
    pub(crate) uplink: [u8; NETWORK_NAME_SIZE],
    pub(crate) uplink_size: usize,
}

pub(crate) fn create_virtual(name: &[u8], uplink: &[u8]) -> bool {
    if !valid_network_name(name) || matches!(name, b"lo" | b"eth0") || network_index(name).is_some()
    {
        return false;
    }
    let Some(uplink_index) = network_index(uplink) else {
        return false;
    };
    let Some(slot) = VIRTUAL_NETWORK_NAME_SIZES
        .iter()
        .position(|size| size.load(Ordering::Acquire) == 0)
    else {
        return false;
    };
    for (index, byte) in VIRTUAL_NETWORK_NAMES[slot].iter().enumerate() {
        byte.store(name.get(index).copied().unwrap_or(0), Ordering::Relaxed);
    }
    VIRTUAL_NETWORK_UPLINKS[slot].store((uplink_index + 1) as u8, Ordering::Relaxed);
    VIRTUAL_NETWORK_NAME_SIZES[slot].store(name.len() as u8, Ordering::Release);
    true
}

pub(crate) fn inventory(cursor: usize) -> Option<InventoryRecord> {
    let (network_index, name, name_size, kind, uplink_index) = if cursor == 0 {
        let mut name = [0_u8; NETWORK_NAME_SIZE];
        name[..4].copy_from_slice(b"eth0");
        (0, name, 4, 1, None)
    } else {
        let start = cursor.saturating_sub(1);
        let slot = (start..VIRTUAL_NETWORK_CAPACITY)
            .find(|slot| VIRTUAL_NETWORK_NAME_SIZES[*slot].load(Ordering::Acquire) != 0)?;
        let name_size = usize::from(VIRTUAL_NETWORK_NAME_SIZES[slot].load(Ordering::Acquire));
        let mut name = [0_u8; NETWORK_NAME_SIZE];
        for (output, stored) in name[..name_size]
            .iter_mut()
            .zip(&VIRTUAL_NETWORK_NAMES[slot][..name_size])
        {
            *output = stored.load(Ordering::Acquire);
        }
        let uplink = usize::from(VIRTUAL_NETWORK_UPLINKS[slot].load(Ordering::Acquire));
        (slot + 1, name, name_size, 2, uplink.checked_sub(1))
    };
    let mut uplink = [0_u8; NETWORK_NAME_SIZE];
    let uplink_size = uplink_index
        .and_then(|index| copy_network_name(index, &mut uplink))
        .unwrap_or(0);
    let next_cursor = next_network_index(network_index + 1)
        .map(|index| index as u64)
        .unwrap_or(u64::MAX);
    Some(InventoryRecord {
        next_cursor,
        kind,
        name,
        name_size,
        uplink,
        uplink_size,
    })
}

pub(crate) fn attach(domain_slot: usize, name: &[u8]) -> bool {
    if domain_slot >= crate::control_abi::DOMAIN_LIMIT {
        return false;
    }
    let Some(network_slot) = network_index(name) else {
        return false;
    };
    let encoded = (((domain_slot + 1) as u16) << 8) | (network_slot + 1) as u16;
    if NETWORK_ATTACHMENTS
        .iter()
        .any(|attachment| attachment.load(Ordering::Acquire) == encoded)
    {
        return true;
    }
    NETWORK_ATTACHMENTS.iter().any(|attachment| {
        attachment
            .compare_exchange(0, encoded, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    })
}

pub(crate) fn attachment(domain_slot: usize, cursor: usize) -> Option<(u64, [u8; 16], usize)> {
    if domain_slot >= crate::control_abi::DOMAIN_LIMIT {
        return None;
    }
    let domain = (domain_slot + 1) as u16;
    let index = (cursor..ATTACHMENT_CAPACITY)
        .find(|index| NETWORK_ATTACHMENTS[*index].load(Ordering::Acquire) >> 8 == domain)?;
    let encoded = NETWORK_ATTACHMENTS[index].load(Ordering::Acquire);
    let network_slot = usize::from((encoded & 0xff).saturating_sub(1));
    let mut name = [0_u8; NETWORK_NAME_SIZE];
    let name_size = copy_network_name(network_slot, &mut name)?;
    let next = ((index + 1)..ATTACHMENT_CAPACITY)
        .find(|next| NETWORK_ATTACHMENTS[*next].load(Ordering::Acquire) >> 8 == domain)
        .map(|next| next as u64)
        .unwrap_or(u64::MAX);
    Some((next, name, name_size))
}

fn next_network_index(start: usize) -> Option<usize> {
    if start == 0 {
        return Some(0);
    }
    ((start - 1)..VIRTUAL_NETWORK_CAPACITY)
        .find(|slot| VIRTUAL_NETWORK_NAME_SIZES[*slot].load(Ordering::Acquire) != 0)
        .map(|slot| slot + 1)
}

fn network_index(name: &[u8]) -> Option<usize> {
    if name == b"eth0" {
        return Some(0);
    }
    (0..VIRTUAL_NETWORK_CAPACITY).find_map(|slot| {
        let size = usize::from(VIRTUAL_NETWORK_NAME_SIZES[slot].load(Ordering::Acquire));
        (size == name.len()
            && VIRTUAL_NETWORK_NAMES[slot][..size]
                .iter()
                .zip(name)
                .all(|(stored, supplied)| stored.load(Ordering::Acquire) == *supplied))
        .then_some(slot + 1)
    })
}

fn copy_network_name(index: usize, output: &mut [u8; NETWORK_NAME_SIZE]) -> Option<usize> {
    if index == 0 {
        output[..4].copy_from_slice(b"eth0");
        return Some(4);
    }
    let slot = index.checked_sub(1)?;
    let size = usize::from(
        VIRTUAL_NETWORK_NAME_SIZES
            .get(slot)?
            .load(Ordering::Acquire),
    );
    if size == 0 {
        return None;
    }
    for (destination, stored) in output[..size]
        .iter_mut()
        .zip(&VIRTUAL_NETWORK_NAMES[slot][..size])
    {
        *destination = stored.load(Ordering::Acquire);
    }
    Some(size)
}

fn valid_network_name(name: &[u8]) -> bool {
    !name.is_empty()
        && name.len() <= NETWORK_NAME_SIZE
        && name
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

static LINK_UP: AtomicBool = AtomicBool::new(false);
static IPV4_CONFIGURATION: AtomicU64 = AtomicU64::new(0);
static IPV4_GATEWAY: AtomicU64 = AtomicU64::new(0);
static IPV4_DNS: AtomicU64 = AtomicU64::new(0);
static CONFIGURATION_GENERATION: AtomicU64 = AtomicU64::new(0);
static ROUTE_DESTINATION: AtomicU64 = AtomicU64::new(0);
static ROUTE_GATEWAY: AtomicU64 = AtomicU64::new(0);

pub(crate) struct Configuration {
    pub(crate) link: u64,
    pub(crate) ipv4: u64,
    pub(crate) gateway: u64,
    pub(crate) route_destination: u64,
    pub(crate) route_gateway: u64,
}

pub(crate) enum ApplyError {
    Invalid,
    Stale,
}

pub(crate) fn set_link(value: u64) -> bool {
    if value > 1 {
        return false;
    }
    if LINK_UP.swap(value != 0, Ordering::AcqRel) != (value != 0) {
        record_change();
    }
    true
}

pub(crate) fn link_is_up() -> bool {
    LINK_UP.load(Ordering::Acquire)
}

pub(crate) fn set_ipv4(configuration: u64) -> bool {
    if !valid_ipv4_configuration(configuration) {
        return false;
    }
    if IPV4_CONFIGURATION.load(Ordering::Acquire) == configuration {
        return true;
    }
    IPV4_CONFIGURATION.store(configuration, Ordering::Release);
    let gateway = IPV4_GATEWAY.load(Ordering::Acquire) as u32;
    if gateway != 0 && !valid_gateway_for_configuration(gateway, configuration) {
        IPV4_GATEWAY.store(0, Ordering::Release);
    }
    let route_gateway = ROUTE_GATEWAY.load(Ordering::Acquire) as u32;
    if route_gateway != 0 && !valid_gateway_for_configuration(route_gateway, configuration) {
        ROUTE_DESTINATION.store(0, Ordering::Release);
        ROUTE_GATEWAY.store(0, Ordering::Release);
    }
    record_change();
    true
}

pub(crate) fn ipv4_configuration() -> u64 {
    IPV4_CONFIGURATION.load(Ordering::Acquire)
}

pub(crate) fn set_gateway(gateway: u64) -> bool {
    if gateway >> 32 != 0
        || (gateway != 0
            && !valid_gateway_for_configuration(
                gateway as u32,
                IPV4_CONFIGURATION.load(Ordering::Acquire),
            ))
    {
        return false;
    }
    if IPV4_GATEWAY.swap(gateway, Ordering::AcqRel) != gateway {
        record_change();
    }
    true
}

pub(crate) fn ipv4_gateway() -> u64 {
    IPV4_GATEWAY.load(Ordering::Acquire)
}

pub(crate) fn set_dns(address: u64) -> bool {
    if address >> 32 != 0 {
        return false;
    }
    if IPV4_DNS.swap(address, Ordering::AcqRel) != address {
        record_change();
    }
    true
}

pub(crate) fn dns() -> u64 {
    IPV4_DNS.load(Ordering::Acquire)
}

pub(crate) fn initialize_runtime() -> bool {
    let cleared = apply(
        generation(),
        Configuration {
            link: 0,
            ipv4: 0,
            gateway: 0,
            route_destination: 0,
            route_gateway: 0,
        },
    )
    .is_ok();
    cleared && set_dns(0)
}

pub(crate) fn generation() -> u64 {
    CONFIGURATION_GENERATION.load(Ordering::Acquire)
}

pub(crate) fn set_route(destination: u32, prefix: u8, gateway: u32) -> bool {
    if !valid_route_for_configuration(
        destination,
        prefix,
        gateway,
        IPV4_CONFIGURATION.load(Ordering::Acquire),
    ) {
        return false;
    }
    let packed_destination = u64::from(destination) | (u64::from(prefix) << 32);
    let changed = ROUTE_DESTINATION.load(Ordering::Acquire) != packed_destination
        || ROUTE_GATEWAY.load(Ordering::Acquire) != u64::from(gateway);
    if changed {
        ROUTE_DESTINATION.store(packed_destination, Ordering::Release);
        ROUTE_GATEWAY.store(u64::from(gateway), Ordering::Release);
        record_change();
    }
    true
}

pub(crate) fn route() -> (u64, u64) {
    (
        ROUTE_DESTINATION.load(Ordering::Acquire),
        ROUTE_GATEWAY.load(Ordering::Acquire),
    )
}

pub(crate) fn snapshot() -> [u64; 6] {
    let (route_destination, route_gateway) = route();
    [
        generation(),
        u64::from(link_is_up()),
        ipv4_configuration(),
        ipv4_gateway(),
        route_destination,
        route_gateway,
    ]
}

pub(crate) fn apply(
    expected_generation: u64,
    configuration: Configuration,
) -> Result<u64, ApplyError> {
    let current_generation = generation();
    if expected_generation != current_generation {
        return Err(ApplyError::Stale);
    }
    if configuration.link > 1
        || !valid_ipv4_configuration(configuration.ipv4)
        || configuration.gateway >> 32 != 0
        || (configuration.gateway != 0
            && !valid_gateway_for_configuration(configuration.gateway as u32, configuration.ipv4))
        || configuration.route_destination >> 40 != 0
        || configuration.route_gateway >> 32 != 0
    {
        return Err(ApplyError::Invalid);
    }
    let destination = configuration.route_destination as u32;
    let prefix = (configuration.route_destination >> 32) as u8;
    if !valid_route_for_configuration(
        destination,
        prefix,
        configuration.route_gateway as u32,
        configuration.ipv4,
    ) {
        return Err(ApplyError::Invalid);
    }
    let changed = LINK_UP.load(Ordering::Acquire) != (configuration.link != 0)
        || IPV4_CONFIGURATION.load(Ordering::Acquire) != configuration.ipv4
        || IPV4_GATEWAY.load(Ordering::Acquire) != configuration.gateway
        || ROUTE_DESTINATION.load(Ordering::Acquire) != configuration.route_destination
        || ROUTE_GATEWAY.load(Ordering::Acquire) != configuration.route_gateway;
    if changed {
        LINK_UP.store(configuration.link != 0, Ordering::Release);
        IPV4_CONFIGURATION.store(configuration.ipv4, Ordering::Release);
        IPV4_GATEWAY.store(configuration.gateway, Ordering::Release);
        ROUTE_DESTINATION.store(configuration.route_destination, Ordering::Release);
        ROUTE_GATEWAY.store(configuration.route_gateway, Ordering::Release);
        record_change();
    }
    Ok(generation())
}

fn record_change() {
    let _ =
        CONFIGURATION_GENERATION.try_update(Ordering::AcqRel, Ordering::Acquire, |generation| {
            Some(generation.saturating_add(1))
        });
}

fn valid_ipv4_configuration(configuration: u64) -> bool {
    let address = configuration as u32;
    let prefix = (configuration >> 32) as u8;
    configuration >> 40 == 0 && prefix <= 32 && (address != 0 || prefix == 0)
}

fn valid_gateway_for_configuration(gateway: u32, configuration: u64) -> bool {
    let address = configuration as u32;
    let prefix = (configuration >> 32) as u8;
    if address == 0 || prefix == 0 {
        return false;
    }
    let mask = u32::MAX << (32 - u32::from(prefix));
    let network = address & mask;
    let broadcast = network | !mask;
    gateway & mask == network && gateway != address && gateway != network && gateway != broadcast
}

fn valid_route_for_configuration(
    destination: u32,
    prefix: u8,
    gateway: u32,
    configuration: u64,
) -> bool {
    if destination == 0 && prefix == 0 && gateway == 0 {
        return true;
    }
    if prefix == 0 || prefix > 32 || gateway == 0 {
        return false;
    }
    let mask = u32::MAX << (32 - u32::from(prefix));
    destination & mask == destination && valid_gateway_for_configuration(gateway, configuration)
}
