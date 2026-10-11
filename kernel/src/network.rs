use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

static LINK_UP: AtomicBool = AtomicBool::new(false);
static IPV4_CONFIGURATION: AtomicU64 = AtomicU64::new(0);
static IPV4_GATEWAY: AtomicU64 = AtomicU64::new(0);
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
