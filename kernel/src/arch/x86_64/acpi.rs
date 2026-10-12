use core::ptr::read_unaligned;

use super::paging;

const SDT_HEADER_SIZE: usize = 36;
const MADT_FIXED_SIZE: usize = 44;
const DMAR_FIXED_SIZE: usize = 48;
const DMAR_STRUCTURE_HEADER_SIZE: usize = 4;
const DMAR_DRHD_SIZE: usize = 16;
const DMAR_DEVICE_SCOPE_MIN_SIZE: usize = 8;
const MAX_DMAR_ENDPOINT_SCOPES: usize = 64;
const MAX_TABLE_SIZE: usize = 1024 * 1024;
const MAX_ROOT_ENTRIES: usize = 4096;
const MADT_ENABLED: u32 = 1;
const MADT_ONLINE_CAPABLE: u32 = 1 << 1;

#[derive(Clone, Copy)]
struct DmarDeviceScopes {
    endpoint_source_ids: [u16; MAX_DMAR_ENDPOINT_SCOPES],
    endpoint_count: usize,
    scope_count: usize,
}

impl DmarDeviceScopes {
    const EMPTY: Self = Self {
        endpoint_source_ids: [0; MAX_DMAR_ENDPOINT_SCOPES],
        endpoint_count: 0,
        scope_count: 0,
    };

    fn contains_endpoint(self, bus: u8, device: u8, function: u8) -> bool {
        let Some(source_id) = pci_source_id(bus, device, function) else {
            return false;
        };
        self.endpoint_source_ids[..self.endpoint_count].contains(&source_id)
    }
}

#[derive(Clone, Copy)]
pub(crate) struct PlatformInfo {
    pub(crate) enabled_processors: usize,
    pub(crate) io_apics: usize,
    pub(crate) local_apic_address: u64,
    pub(crate) first_io_apic_address: u32,
    pub(crate) first_io_apic_gsi: u32,
    pub(crate) iommu_units: usize,
    pub(crate) iommu_host_address_width: u8,
    pub(crate) first_iommu_register_base: u64,
    pub(crate) first_iommu_include_all: bool,
    first_iommu_scopes: DmarDeviceScopes,
}

impl PlatformInfo {
    pub(crate) fn first_iommu_scope_count(self) -> usize {
        self.first_iommu_scopes.scope_count
    }

    pub(crate) fn first_iommu_covers_pci(self, bus: u8, device: u8, function: u8) -> bool {
        pci_source_id(bus, device, function).is_some()
            && (self.first_iommu_include_all
                || self
                    .first_iommu_scopes
                    .contains_endpoint(bus, device, function))
    }
}

pub(crate) fn discover(rsdp: *const u8, mapped_limit: usize) -> Option<PlatformInfo> {
    let rsdp_physical = rsdp as usize;
    let rsdp = mapped_bytes(rsdp_physical, 20, mapped_limit)?;
    let revision = unsafe { rsdp.add(15).read() };
    let (root_address, entry_size, signature) = if revision >= 2 {
        let rsdp = mapped_bytes(rsdp_physical, 36, mapped_limit)?;
        let xsdt = unsafe { read_unaligned(rsdp.add(24).cast::<u64>()) };
        if xsdt != 0 {
            (usize::try_from(xsdt).ok()?, 8, b"XSDT")
        } else {
            let rsdt = unsafe { read_unaligned(rsdp.add(16).cast::<u32>()) };
            (rsdt as usize, 4, b"RSDT")
        }
    } else {
        let rsdt = unsafe { read_unaligned(rsdp.add(16).cast::<u32>()) };
        (rsdt as usize, 4, b"RSDT")
    };

    let root = validate_sdt(root_address, signature, mapped_limit)?;
    let payload_size = root.length.checked_sub(SDT_HEADER_SIZE)?;
    if !payload_size.is_multiple_of(entry_size) || payload_size / entry_size > MAX_ROOT_ENTRIES {
        return None;
    }

    let mut platform = None;
    let mut iommu = None;
    for index in 0..payload_size / entry_size {
        let entry = root_address.checked_add(SDT_HEADER_SIZE + index * entry_size)?;
        let entry = mapped_bytes(entry, entry_size, mapped_limit)?;
        let table_address = unsafe {
            if entry_size == 8 {
                usize::try_from(read_unaligned(entry.cast::<u64>())).ok()?
            } else {
                read_unaligned(entry.cast::<u32>()) as usize
            }
        };
        if signature_at(table_address, b"APIC", mapped_limit) {
            if platform.is_some() {
                return None;
            }
            platform = parse_madt(table_address, mapped_limit);
        } else if signature_at(table_address, b"DMAR", mapped_limit) {
            if iommu.is_some() {
                return None;
            }
            iommu = parse_dmar(table_address, mapped_limit);
        }
    }
    let (Some(mut platform), Some(iommu)) = (platform, iommu) else {
        return None;
    };
    platform.iommu_units = iommu.units;
    platform.iommu_host_address_width = iommu.host_address_width;
    platform.first_iommu_register_base = iommu.first_register_base;
    platform.first_iommu_include_all = iommu.first_include_all;
    platform.first_iommu_scopes = iommu.first_scopes;
    Some(platform)
}

struct ValidatedSdt {
    length: usize,
    bytes: *const u8,
}

fn validate_sdt(address: usize, signature: &[u8; 4], mapped_limit: usize) -> Option<ValidatedSdt> {
    let header = mapped_bytes(address, SDT_HEADER_SIZE, mapped_limit)?;
    for (offset, expected) in signature.iter().enumerate() {
        if unsafe { header.add(offset).read() } != *expected {
            return None;
        }
    }
    let length = unsafe { read_unaligned(header.add(4).cast::<u32>()) } as usize;
    if !(SDT_HEADER_SIZE..=MAX_TABLE_SIZE).contains(&length) {
        return None;
    }
    let bytes = mapped_bytes(address, length, mapped_limit)?;
    if unsafe { checksum(bytes, length) } != 0 {
        return None;
    }
    Some(ValidatedSdt { length, bytes })
}

fn signature_at(address: usize, signature: &[u8; 4], mapped_limit: usize) -> bool {
    let Some(bytes) = mapped_bytes(address, signature.len(), mapped_limit) else {
        return false;
    };
    signature
        .iter()
        .enumerate()
        .all(|(offset, expected)| unsafe { bytes.add(offset).read() == *expected })
}

fn parse_madt(address: usize, mapped_limit: usize) -> Option<PlatformInfo> {
    let table = validate_sdt(address, b"APIC", mapped_limit)?;
    if table.length < MADT_FIXED_SIZE {
        return None;
    }
    let mut info = PlatformInfo {
        enabled_processors: 0,
        io_apics: 0,
        local_apic_address: unsafe { u64::from(read_unaligned(table.bytes.add(36).cast::<u32>())) },
        first_io_apic_address: 0,
        first_io_apic_gsi: 0,
        iommu_units: 0,
        iommu_host_address_width: 0,
        first_iommu_register_base: 0,
        first_iommu_include_all: false,
        first_iommu_scopes: DmarDeviceScopes::EMPTY,
    };

    let mut offset = MADT_FIXED_SIZE;
    while offset < table.length {
        if table.length - offset < 2 {
            return None;
        }
        let entry = unsafe { table.bytes.add(offset) };
        let entry_type = unsafe { entry.read() };
        let entry_length = usize::from(unsafe { entry.add(1).read() });
        if entry_length < 2 || entry_length > table.length - offset {
            return None;
        }

        match entry_type {
            0 if entry_length >= 8 => {
                let flags = unsafe { read_unaligned(entry.add(4).cast::<u32>()) };
                if flags & (MADT_ENABLED | MADT_ONLINE_CAPABLE) != 0 {
                    info.enabled_processors = info.enabled_processors.checked_add(1)?;
                }
            }
            1 if entry_length >= 12 => {
                let io_apic_address = unsafe { read_unaligned(entry.add(4).cast::<u32>()) };
                let gsi = unsafe { read_unaligned(entry.add(8).cast::<u32>()) };
                if info.io_apics == 0 {
                    info.first_io_apic_address = io_apic_address;
                    info.first_io_apic_gsi = gsi;
                }
                info.io_apics = info.io_apics.checked_add(1)?;
            }
            5 if entry_length >= 12 => {
                info.local_apic_address = unsafe { read_unaligned(entry.add(4).cast::<u64>()) };
            }
            9 if entry_length >= 16 => {
                let flags = unsafe { read_unaligned(entry.add(4).cast::<u32>()) };
                if flags & (MADT_ENABLED | MADT_ONLINE_CAPABLE) != 0 {
                    info.enabled_processors = info.enabled_processors.checked_add(1)?;
                }
            }
            _ => {}
        }
        offset += entry_length;
    }

    (info.enabled_processors != 0 && info.io_apics != 0 && info.local_apic_address != 0)
        .then_some(info)
}

#[derive(Clone, Copy)]
struct IommuInfo {
    units: usize,
    host_address_width: u8,
    first_register_base: u64,
    first_include_all: bool,
    first_scopes: DmarDeviceScopes,
}

fn parse_dmar(address: usize, mapped_limit: usize) -> Option<IommuInfo> {
    let table = validate_sdt(address, b"DMAR", mapped_limit)?;
    if table.length < DMAR_FIXED_SIZE {
        return None;
    }
    let host_address_width = unsafe { table.bytes.add(36).read().checked_add(1)? };
    let flags = unsafe { table.bytes.add(37).read() };
    if !(32..=64).contains(&host_address_width)
        || flags & !0x07 != 0
        || (38..DMAR_FIXED_SIZE).any(|offset| unsafe { table.bytes.add(offset).read() != 0 })
    {
        return None;
    }
    let structures = unsafe { table.bytes.add(DMAR_FIXED_SIZE) };
    parse_dmar_structures(
        structures,
        table.length - DMAR_FIXED_SIZE,
        host_address_width,
        mapped_limit,
    )
}

fn parse_dmar_structures(
    bytes: *const u8,
    length: usize,
    host_address_width: u8,
    mapped_limit: usize,
) -> Option<IommuInfo> {
    let mut offset = 0;
    let mut units = 0_usize;
    let mut first_register_base = 0_u64;
    let mut first_include_all = false;
    let mut first_scopes = DmarDeviceScopes::EMPTY;
    while offset < length {
        if length - offset < DMAR_STRUCTURE_HEADER_SIZE {
            return None;
        }
        let structure = unsafe { bytes.add(offset) };
        let kind = unsafe { read_unaligned(structure.cast::<u16>()) };
        let structure_length =
            usize::from(unsafe { read_unaligned(structure.add(2).cast::<u16>()) });
        if structure_length < DMAR_STRUCTURE_HEADER_SIZE || structure_length > length - offset {
            return None;
        }
        if kind == 0 {
            if structure_length < DMAR_DRHD_SIZE {
                return None;
            }
            let flags = unsafe { structure.add(4).read() };
            let reserved = unsafe { structure.add(5).read() };
            let segment = unsafe { read_unaligned(structure.add(6).cast::<u16>()) };
            let register_base = unsafe { read_unaligned(structure.add(8).cast::<u64>()) };
            let scopes = validate_dmar_device_scopes(
                unsafe { structure.add(DMAR_DRHD_SIZE) },
                structure_length - DMAR_DRHD_SIZE,
            )?;
            if flags & !1 != 0
                || reserved != 0
                || segment != 0
                || register_base == 0
                || !register_base.is_multiple_of(4096)
                || !range_is_mapped(usize::try_from(register_base).ok()?, 4096, mapped_limit)
            {
                return None;
            }
            units = units.checked_add(1)?;
            if units == 1 {
                first_register_base = register_base;
                first_include_all = flags & 1 != 0;
                first_scopes = scopes;
            }
        }
        offset += structure_length;
    }
    (offset == length && units != 0 && first_register_base != 0).then_some(IommuInfo {
        units,
        host_address_width,
        first_register_base,
        first_include_all,
        first_scopes,
    })
}

fn validate_dmar_device_scopes(bytes: *const u8, length: usize) -> Option<DmarDeviceScopes> {
    let mut scopes = DmarDeviceScopes::EMPTY;
    let mut offset = 0;
    while offset < length {
        if length - offset < 2 {
            return None;
        }
        let scope_type = unsafe { bytes.add(offset).read() };
        let scope_length = usize::from(unsafe { bytes.add(offset + 1).read() });
        if scope_length < DMAR_DEVICE_SCOPE_MIN_SIZE
            || scope_length > length - offset
            || !(scope_length - 6).is_multiple_of(2)
            || !(1..=5).contains(&scope_type)
            || unsafe { read_unaligned(bytes.add(offset + 2).cast::<u16>()) } != 0
        {
            return None;
        }
        let enumeration_id = unsafe { bytes.add(offset + 4).read() };
        let start_bus = unsafe { bytes.add(offset + 5).read() };
        if matches!(scope_type, 1 | 2) && enumeration_id != 0 {
            return None;
        }
        let path_entries = (scope_length - 6) / 2;
        for path in 0..path_entries {
            let device = unsafe { bytes.add(offset + 6 + path * 2).read() };
            let function = unsafe { bytes.add(offset + 7 + path * 2).read() };
            if device >= 32 || function >= 8 {
                return None;
            }
        }
        scopes.scope_count = scopes.scope_count.checked_add(1)?;
        if scope_type == 1 && path_entries == 1 {
            let device = unsafe { bytes.add(offset + 6).read() };
            let function = unsafe { bytes.add(offset + 7).read() };
            let source_id = pci_source_id(start_bus, device, function)?;
            if scopes.endpoint_count == MAX_DMAR_ENDPOINT_SCOPES
                || scopes.endpoint_source_ids[..scopes.endpoint_count].contains(&source_id)
            {
                return None;
            }
            scopes.endpoint_source_ids[scopes.endpoint_count] = source_id;
            scopes.endpoint_count += 1;
        }
        offset += scope_length;
    }
    (offset == length).then_some(scopes)
}

fn pci_source_id(bus: u8, device: u8, function: u8) -> Option<u16> {
    (device < 32 && function < 8)
        .then_some((u16::from(bus) << 8) | (u16::from(device) << 3) | u16::from(function))
}

pub(crate) fn verify_malformed_dmar_rejected(mapped_limit: usize) -> bool {
    let mut valid = [0_u8; DMAR_DRHD_SIZE];
    valid[2..4].copy_from_slice(&(DMAR_DRHD_SIZE as u16).to_le_bytes());
    valid[4] = 1;
    valid[8..16].copy_from_slice(&0xfed9_0000_u64.to_le_bytes());
    if parse_dmar_structures(valid.as_ptr(), valid.len(), 48, mapped_limit).is_none() {
        return false;
    }
    let mut zero_length = valid;
    zero_length[2..4].copy_from_slice(&0_u16.to_le_bytes());
    let mut short_drhd = valid;
    short_drhd[2..4].copy_from_slice(&8_u16.to_le_bytes());
    let mut zero_base = valid;
    zero_base[8..16].fill(0);
    let mut unknown_flags = valid;
    unknown_flags[4] = 0x80;
    let mut trailing = [0_u8; DMAR_DRHD_SIZE + 1];
    trailing[..DMAR_DRHD_SIZE].copy_from_slice(&valid);
    let mut bad_scope = [0_u8; DMAR_DRHD_SIZE + DMAR_DEVICE_SCOPE_MIN_SIZE];
    bad_scope[..DMAR_DRHD_SIZE].copy_from_slice(&valid);
    let bad_scope_length = bad_scope.len() as u16;
    bad_scope[2..4].copy_from_slice(&bad_scope_length.to_le_bytes());
    bad_scope[4] = 0;
    bad_scope[DMAR_DRHD_SIZE + 1] = 7;
    let rejected = [
        zero_length.as_slice(),
        short_drhd.as_slice(),
        zero_base.as_slice(),
        unknown_flags.as_slice(),
        trailing.as_slice(),
        bad_scope.as_slice(),
    ]
    .into_iter()
    .all(|bytes| parse_dmar_structures(bytes.as_ptr(), bytes.len(), 48, mapped_limit).is_none());
    rejected
}

pub(crate) fn verify_dmar_scope_resolution(mapped_limit: usize) -> bool {
    let mut endpoint = [0_u8; DMAR_DRHD_SIZE + DMAR_DEVICE_SCOPE_MIN_SIZE];
    let endpoint_length = endpoint.len() as u16;
    endpoint[2..4].copy_from_slice(&endpoint_length.to_le_bytes());
    endpoint[8..16].copy_from_slice(&0xfed9_0000_u64.to_le_bytes());
    endpoint[DMAR_DRHD_SIZE] = 1;
    endpoint[DMAR_DRHD_SIZE + 1] = DMAR_DEVICE_SCOPE_MIN_SIZE as u8;
    endpoint[DMAR_DRHD_SIZE + 5] = 0;
    endpoint[DMAR_DRHD_SIZE + 6] = 2;
    endpoint[DMAR_DRHD_SIZE + 7] = 0;
    let Some(info) = parse_dmar_structures(endpoint.as_ptr(), endpoint.len(), 48, mapped_limit)
    else {
        return false;
    };
    if info.first_include_all
        || !info.first_scopes.contains_endpoint(0, 2, 0)
        || info.first_scopes.contains_endpoint(0, 3, 0)
        || info.first_scopes.contains_endpoint(0, 2, 1)
    {
        return false;
    }

    let mut duplicate = [0_u8; DMAR_DRHD_SIZE + 2 * DMAR_DEVICE_SCOPE_MIN_SIZE];
    duplicate[..endpoint.len()].copy_from_slice(&endpoint);
    let duplicate_length = duplicate.len() as u16;
    duplicate[2..4].copy_from_slice(&duplicate_length.to_le_bytes());
    duplicate[endpoint.len()..].copy_from_slice(&endpoint[DMAR_DRHD_SIZE..]);
    let mut reserved = endpoint;
    reserved[DMAR_DRHD_SIZE + 2] = 1;
    let mut enumeration = endpoint;
    enumeration[DMAR_DRHD_SIZE + 4] = 1;
    let mut invalid_device = endpoint;
    invalid_device[DMAR_DRHD_SIZE + 6] = 32;
    let mut invalid_function = endpoint;
    invalid_function[DMAR_DRHD_SIZE + 7] = 8;
    let mut unknown_type = endpoint;
    unknown_type[DMAR_DRHD_SIZE] = 6;
    let rejected = [
        duplicate.as_slice(),
        reserved.as_slice(),
        enumeration.as_slice(),
        invalid_device.as_slice(),
        invalid_function.as_slice(),
        unknown_type.as_slice(),
    ]
    .into_iter()
    .all(|bytes| parse_dmar_structures(bytes.as_ptr(), bytes.len(), 48, mapped_limit).is_none());
    rejected
}

fn range_is_mapped(address: usize, length: usize, mapped_limit: usize) -> bool {
    length != 0
        && address < mapped_limit
        && address
            .checked_add(length - 1)
            .is_some_and(|end| end < mapped_limit)
}

fn mapped_bytes(address: usize, length: usize, mapped_limit: usize) -> Option<*const u8> {
    if !range_is_mapped(address, length, mapped_limit) {
        return None;
    }
    paging::direct_map_range_address(address as u64, length).map(|mapped| mapped as *const u8)
}

unsafe fn checksum(bytes: *const u8, length: usize) -> u8 {
    let mut sum = 0_u8;
    for offset in 0..length {
        sum = sum.wrapping_add(bytes.add(offset).read());
    }
    sum
}
