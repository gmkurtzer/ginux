use core::sync::atomic::{AtomicBool, AtomicU16, AtomicU64, AtomicU8, Ordering};

pub(crate) const NETWORK_NAME_SIZE: usize = 16;
const NETWORK_CAPACITY: usize = 64;
const VIRTUAL_NETWORK_CAPACITY: usize = NETWORK_CAPACITY - 1;
const ATTACHMENT_CAPACITY: usize = crate::control_abi::DOMAIN_LIMIT * 8;
pub(crate) const PERSISTENT_NETWORK_CONFIG_SIZE: usize = 2048;
const PERSISTENT_VIRTUAL_OFFSET: usize = 80;
const PERSISTENT_VIRTUAL_RECORD_SIZE: usize = 20;
const PERSISTENT_ATTACHMENT_OFFSET: usize = 1344;

// Zero is unused. The upper byte is domain slot + 1 and the lower byte is
// network slot + 1. Attachments are resource-bounded kernel objects, not a
// singleton field on a domain.
struct NetworkOwner {
    virtual_name_sizes: [AtomicU8; VIRTUAL_NETWORK_CAPACITY],
    virtual_names: [[AtomicU8; NETWORK_NAME_SIZE]; VIRTUAL_NETWORK_CAPACITY],
    virtual_uplinks: [AtomicU8; VIRTUAL_NETWORK_CAPACITY],
    virtual_links: [AtomicBool; VIRTUAL_NETWORK_CAPACITY],
    attachments: [AtomicU16; ATTACHMENT_CAPACITY],
    link_up: AtomicBool,
    ipv4_configuration: AtomicU64,
    ipv4_gateway: AtomicU64,
    ipv4_dns: AtomicU64,
    dhcp_configuration: AtomicBool,
    configuration_generation: AtomicU64,
    route_destination: AtomicU64,
    route_gateway: AtomicU64,
}

static NETWORKS: NetworkOwner = NetworkOwner {
    virtual_name_sizes: [const { AtomicU8::new(0) }; VIRTUAL_NETWORK_CAPACITY],
    virtual_names: [const { [const { AtomicU8::new(0) }; NETWORK_NAME_SIZE] };
        VIRTUAL_NETWORK_CAPACITY],
    virtual_uplinks: [const { AtomicU8::new(0) }; VIRTUAL_NETWORK_CAPACITY],
    virtual_links: [const { AtomicBool::new(false) }; VIRTUAL_NETWORK_CAPACITY],
    attachments: [const { AtomicU16::new(0) }; ATTACHMENT_CAPACITY],
    link_up: AtomicBool::new(false),
    ipv4_configuration: AtomicU64::new(0),
    ipv4_gateway: AtomicU64::new(0),
    ipv4_dns: AtomicU64::new(0),
    dhcp_configuration: AtomicBool::new(false),
    configuration_generation: AtomicU64::new(0),
    route_destination: AtomicU64::new(0),
    route_gateway: AtomicU64::new(0),
};

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
    let Some(slot) = NETWORKS
        .virtual_name_sizes
        .iter()
        .position(|size| size.load(Ordering::Acquire) == 0)
    else {
        return false;
    };
    for (index, byte) in NETWORKS.virtual_names[slot].iter().enumerate() {
        byte.store(name.get(index).copied().unwrap_or(0), Ordering::Relaxed);
    }
    NETWORKS.virtual_uplinks[slot].store((uplink_index + 1) as u8, Ordering::Relaxed);
    NETWORKS.virtual_links[slot].store(true, Ordering::Relaxed);
    NETWORKS.virtual_name_sizes[slot].store(name.len() as u8, Ordering::Release);
    true
}

pub(crate) fn set_virtual_link(name: &[u8], up: bool) -> bool {
    let Some(slot) = network_index(name).and_then(|index| index.checked_sub(1)) else {
        return false;
    };
    NETWORKS.virtual_links[slot].store(up, Ordering::Release);
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
            .find(|slot| NETWORKS.virtual_name_sizes[*slot].load(Ordering::Acquire) != 0)?;
        let name_size = usize::from(NETWORKS.virtual_name_sizes[slot].load(Ordering::Acquire));
        let mut name = [0_u8; NETWORK_NAME_SIZE];
        for (output, stored) in name[..name_size]
            .iter_mut()
            .zip(&NETWORKS.virtual_names[slot][..name_size])
        {
            *output = stored.load(Ordering::Acquire);
        }
        let uplink = usize::from(NETWORKS.virtual_uplinks[slot].load(Ordering::Acquire));
        let state = if NETWORKS.virtual_links[slot].load(Ordering::Acquire) {
            ginux_native_abi::NATIVE_NETWORK_FLAG_UP
        } else {
            0
        };
        (
            slot + 1,
            name,
            name_size,
            ginux_native_abi::NATIVE_NETWORK_KIND_VIRTUAL | state,
            uplink.checked_sub(1),
        )
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
    if NETWORKS
        .attachments
        .iter()
        .any(|attachment| attachment.load(Ordering::Acquire) == encoded)
    {
        return true;
    }
    NETWORKS.attachments.iter().any(|attachment| {
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
        .find(|index| NETWORKS.attachments[*index].load(Ordering::Acquire) >> 8 == domain)?;
    let encoded = NETWORKS.attachments[index].load(Ordering::Acquire);
    let network_slot = usize::from((encoded & 0xff).saturating_sub(1));
    let mut name = [0_u8; NETWORK_NAME_SIZE];
    let name_size = copy_network_name(network_slot, &mut name)?;
    let next = ((index + 1)..ATTACHMENT_CAPACITY)
        .find(|next| NETWORKS.attachments[*next].load(Ordering::Acquire) >> 8 == domain)
        .map(|next| next as u64)
        .unwrap_or(u64::MAX);
    Some((next, name, name_size))
}

fn next_network_index(start: usize) -> Option<usize> {
    if start == 0 {
        return Some(0);
    }
    ((start - 1)..VIRTUAL_NETWORK_CAPACITY)
        .find(|slot| NETWORKS.virtual_name_sizes[*slot].load(Ordering::Acquire) != 0)
        .map(|slot| slot + 1)
}

fn network_index(name: &[u8]) -> Option<usize> {
    if name == b"eth0" {
        return Some(0);
    }
    (0..VIRTUAL_NETWORK_CAPACITY).find_map(|slot| {
        let size = usize::from(NETWORKS.virtual_name_sizes[slot].load(Ordering::Acquire));
        (size == name.len()
            && NETWORKS.virtual_names[slot][..size]
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
        NETWORKS
            .virtual_name_sizes
            .get(slot)?
            .load(Ordering::Acquire),
    );
    if size == 0 {
        return None;
    }
    for (destination, stored) in output[..size]
        .iter_mut()
        .zip(&NETWORKS.virtual_names[slot][..size])
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
    if NETWORKS.link_up.swap(value != 0, Ordering::AcqRel) != (value != 0) {
        record_change();
    }
    true
}

pub(crate) fn link_is_up() -> bool {
    NETWORKS.link_up.load(Ordering::Acquire)
}

pub(crate) fn reset_configuration() {
    NETWORKS.link_up.store(false, Ordering::Release);
    NETWORKS.ipv4_configuration.store(0, Ordering::Release);
    NETWORKS.ipv4_gateway.store(0, Ordering::Release);
    NETWORKS.ipv4_dns.store(0, Ordering::Release);
    NETWORKS.dhcp_configuration.store(false, Ordering::Release);
    NETWORKS
        .configuration_generation
        .store(0, Ordering::Release);
    NETWORKS.route_destination.store(0, Ordering::Release);
    NETWORKS.route_gateway.store(0, Ordering::Release);
}

pub(crate) fn set_ipv4(configuration: u64) -> bool {
    if !valid_ipv4_configuration(configuration) {
        return false;
    }
    if NETWORKS.ipv4_configuration.load(Ordering::Acquire) == configuration {
        return true;
    }
    NETWORKS
        .ipv4_configuration
        .store(configuration, Ordering::Release);
    let gateway = NETWORKS.ipv4_gateway.load(Ordering::Acquire) as u32;
    if gateway != 0 && !valid_gateway_for_configuration(gateway, configuration) {
        NETWORKS.ipv4_gateway.store(0, Ordering::Release);
    }
    let route_gateway = NETWORKS.route_gateway.load(Ordering::Acquire) as u32;
    if route_gateway != 0 && !valid_gateway_for_configuration(route_gateway, configuration) {
        NETWORKS.route_destination.store(0, Ordering::Release);
        NETWORKS.route_gateway.store(0, Ordering::Release);
    }
    record_change();
    true
}

pub(crate) fn ipv4_configuration() -> u64 {
    NETWORKS.ipv4_configuration.load(Ordering::Acquire)
}

pub(crate) fn set_gateway(gateway: u64) -> bool {
    if gateway >> 32 != 0
        || (gateway != 0
            && !valid_gateway_for_configuration(
                gateway as u32,
                NETWORKS.ipv4_configuration.load(Ordering::Acquire),
            ))
    {
        return false;
    }
    if NETWORKS.ipv4_gateway.swap(gateway, Ordering::AcqRel) != gateway {
        record_change();
    }
    true
}

pub(crate) fn ipv4_gateway() -> u64 {
    NETWORKS.ipv4_gateway.load(Ordering::Acquire)
}

pub(crate) fn set_dns(address: u64) -> bool {
    if address >> 32 != 0 {
        return false;
    }
    if NETWORKS.ipv4_dns.swap(address, Ordering::AcqRel) != address {
        record_change();
    }
    true
}

pub(crate) fn dns() -> u64 {
    NETWORKS.ipv4_dns.load(Ordering::Acquire)
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

pub(crate) fn set_dhcp_configuration(enabled: bool) {
    NETWORKS
        .dhcp_configuration
        .store(enabled, Ordering::Release);
}

pub(crate) fn export_persistent_configuration(output: &mut [u8; PERSISTENT_NETWORK_CONFIG_SIZE]) {
    output.fill(0);
    output[..8].copy_from_slice(b"GINUXNW1");
    output[8..12].copy_from_slice(&1_u32.to_le_bytes());
    output[12..14].copy_from_slice(&(VIRTUAL_NETWORK_CAPACITY as u16).to_le_bytes());
    output[16] = u8::from(NETWORKS.dhcp_configuration.load(Ordering::Acquire));
    let snapshot = snapshot();
    for (index, value) in snapshot.iter().enumerate() {
        let offset = 24 + index * 8;
        output[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
    }
    output[72..80].copy_from_slice(&dns().to_le_bytes());
    for slot in 0..VIRTUAL_NETWORK_CAPACITY {
        let offset = PERSISTENT_VIRTUAL_OFFSET + slot * PERSISTENT_VIRTUAL_RECORD_SIZE;
        let name_size = NETWORKS.virtual_name_sizes[slot].load(Ordering::Acquire);
        output[offset] = name_size;
        output[offset + 1] = NETWORKS.virtual_uplinks[slot].load(Ordering::Acquire);
        output[offset + 2] = u8::from(NETWORKS.virtual_links[slot].load(Ordering::Acquire));
        for (destination, source) in output[offset + 4..offset + 20]
            .iter_mut()
            .zip(&NETWORKS.virtual_names[slot])
        {
            *destination = source.load(Ordering::Acquire);
        }
    }
    for (index, attachment) in NETWORKS.attachments.iter().enumerate() {
        let offset = PERSISTENT_ATTACHMENT_OFFSET + index * 2;
        output[offset..offset + 2]
            .copy_from_slice(&attachment.load(Ordering::Acquire).to_le_bytes());
    }
}

pub(crate) fn import_persistent_configuration(
    input: &[u8; PERSISTENT_NETWORK_CONFIG_SIZE],
) -> bool {
    if &input[..8] != b"GINUXNW1"
        || u32::from_le_bytes(input[8..12].try_into().unwrap_or([0; 4])) != 1
        || usize::from(u16::from_le_bytes(
            input[12..14].try_into().unwrap_or([0; 2]),
        )) != VIRTUAL_NETWORK_CAPACITY
        || input[14..16].iter().any(|byte| *byte != 0)
        || input[16] > 1
        || input[17..24].iter().any(|byte| *byte != 0)
        || NETWORKS
            .virtual_name_sizes
            .iter()
            .any(|size| size.load(Ordering::Acquire) != 0)
        || NETWORKS
            .attachments
            .iter()
            .any(|attachment| attachment.load(Ordering::Acquire) != 0)
    {
        return false;
    }
    let mut virtual_present = [false; VIRTUAL_NETWORK_CAPACITY];
    for (slot, present) in virtual_present.iter_mut().enumerate() {
        let offset = PERSISTENT_VIRTUAL_OFFSET + slot * PERSISTENT_VIRTUAL_RECORD_SIZE;
        let size = usize::from(input[offset]);
        let uplink = usize::from(input[offset + 1]);
        let link = input[offset + 2];
        let name = &input[offset + 4..offset + 20];
        if size == 0 {
            if uplink != 0 || link != 0 || name.iter().any(|byte| *byte != 0) {
                return false;
            }
            continue;
        }
        if size > NETWORK_NAME_SIZE
            || link > 1
            || !(1..=NETWORK_CAPACITY).contains(&uplink)
            || !valid_network_name(&name[..size])
            || matches!(&name[..size], b"lo" | b"eth0")
            || name[size..].iter().any(|byte| *byte != 0)
        {
            return false;
        }
        *present = true;
    }
    for (slot, present) in virtual_present.iter().enumerate() {
        if !*present {
            continue;
        }
        let offset = PERSISTENT_VIRTUAL_OFFSET + slot * PERSISTENT_VIRTUAL_RECORD_SIZE;
        let uplink = usize::from(input[offset + 1]);
        if uplink > 1 && !virtual_present[uplink - 2] {
            return false;
        }
        for (other, other_present) in virtual_present.iter().enumerate().take(slot) {
            if *other_present {
                let other_offset =
                    PERSISTENT_VIRTUAL_OFFSET + other * PERSISTENT_VIRTUAL_RECORD_SIZE;
                if input[other_offset] == input[offset]
                    && input[other_offset + 4..other_offset + 20] == input[offset + 4..offset + 20]
                {
                    return false;
                }
            }
        }
    }
    for index in 0..ATTACHMENT_CAPACITY {
        let offset = PERSISTENT_ATTACHMENT_OFFSET + index * 2;
        let encoded = u16::from_le_bytes(input[offset..offset + 2].try_into().unwrap_or([0; 2]));
        if encoded == 0 {
            continue;
        }
        let domain = usize::from(encoded >> 8);
        let network = usize::from(encoded & 0xff);
        if !(1..=crate::control_abi::DOMAIN_LIMIT).contains(&domain)
            || !(1..=NETWORK_CAPACITY).contains(&network)
            || (network > 1 && !virtual_present[network - 2])
        {
            return false;
        }
    }
    reset_configuration();
    let dhcp = input[16] == 1;
    if !dhcp {
        let value = |index: usize| {
            let offset = 24 + index * 8;
            u64::from_le_bytes(input[offset..offset + 8].try_into().unwrap_or([0; 8]))
        };
        if !set_ipv4(value(2))
            || !set_gateway(value(3))
            || !set_dns(u64::from_le_bytes(
                input[72..80].try_into().unwrap_or([0; 8]),
            ))
            || !set_route(value(4) as u32, (value(4) >> 32) as u8, value(5) as u32)
            || !set_link(value(1))
        {
            reset_configuration();
            return false;
        }
    }
    NETWORKS.dhcp_configuration.store(dhcp, Ordering::Release);
    for (slot, present) in virtual_present.iter().enumerate() {
        if !*present {
            continue;
        }
        let offset = PERSISTENT_VIRTUAL_OFFSET + slot * PERSISTENT_VIRTUAL_RECORD_SIZE;
        let size = input[offset];
        NETWORKS.virtual_name_sizes[slot].store(size, Ordering::Release);
        NETWORKS.virtual_uplinks[slot].store(input[offset + 1], Ordering::Release);
        NETWORKS.virtual_links[slot].store(input[offset + 2] == 1, Ordering::Release);
        for (destination, source) in NETWORKS.virtual_names[slot]
            .iter()
            .zip(&input[offset + 4..offset + 20])
        {
            destination.store(*source, Ordering::Release);
        }
    }
    for (index, attachment) in NETWORKS.attachments.iter().enumerate() {
        let offset = PERSISTENT_ATTACHMENT_OFFSET + index * 2;
        attachment.store(
            u16::from_le_bytes(input[offset..offset + 2].try_into().unwrap_or([0; 2])),
            Ordering::Release,
        );
    }
    true
}

pub(crate) fn generation() -> u64 {
    NETWORKS.configuration_generation.load(Ordering::Acquire)
}

pub(crate) fn set_route(destination: u32, prefix: u8, gateway: u32) -> bool {
    if !valid_route_for_configuration(
        destination,
        prefix,
        gateway,
        NETWORKS.ipv4_configuration.load(Ordering::Acquire),
    ) {
        return false;
    }
    let packed_destination = u64::from(destination) | (u64::from(prefix) << 32);
    let changed = NETWORKS.route_destination.load(Ordering::Acquire) != packed_destination
        || NETWORKS.route_gateway.load(Ordering::Acquire) != u64::from(gateway);
    if changed {
        NETWORKS
            .route_destination
            .store(packed_destination, Ordering::Release);
        NETWORKS
            .route_gateway
            .store(u64::from(gateway), Ordering::Release);
        record_change();
    }
    true
}

pub(crate) fn route() -> (u64, u64) {
    (
        NETWORKS.route_destination.load(Ordering::Acquire),
        NETWORKS.route_gateway.load(Ordering::Acquire),
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
    NETWORKS.dhcp_configuration.store(false, Ordering::Release);
    let changed = NETWORKS.link_up.load(Ordering::Acquire) != (configuration.link != 0)
        || NETWORKS.ipv4_configuration.load(Ordering::Acquire) != configuration.ipv4
        || NETWORKS.ipv4_gateway.load(Ordering::Acquire) != configuration.gateway
        || NETWORKS.route_destination.load(Ordering::Acquire) != configuration.route_destination
        || NETWORKS.route_gateway.load(Ordering::Acquire) != configuration.route_gateway;
    if changed {
        NETWORKS
            .link_up
            .store(configuration.link != 0, Ordering::Release);
        NETWORKS
            .ipv4_configuration
            .store(configuration.ipv4, Ordering::Release);
        NETWORKS
            .ipv4_gateway
            .store(configuration.gateway, Ordering::Release);
        NETWORKS
            .route_destination
            .store(configuration.route_destination, Ordering::Release);
        NETWORKS
            .route_gateway
            .store(configuration.route_gateway, Ordering::Release);
        record_change();
    }
    Ok(generation())
}

fn record_change() {
    let _ = NETWORKS.configuration_generation.try_update(
        Ordering::AcqRel,
        Ordering::Acquire,
        |generation| Some(generation.saturating_add(1)),
    );
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
