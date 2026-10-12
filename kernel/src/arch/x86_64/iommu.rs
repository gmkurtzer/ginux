use core::ptr::read_volatile;

use super::paging::MmioMapping;

const VERSION_OFFSET: u64 = 0x00;
const CAPABILITY_OFFSET: u64 = 0x08;
const EXTENDED_CAPABILITY_OFFSET: u64 = 0x10;
const GLOBAL_STATUS_OFFSET: u64 = 0x1c;
const ROOT_TABLE_ADDRESS_OFFSET: u64 = 0x20;
const TRANSLATION_ENABLE_STATUS: u32 = 1 << 31;
const SUPPORTED_ADDRESS_WIDTHS_SHIFT: u64 = 8;
const SUPPORTED_ADDRESS_WIDTHS_MASK: u64 = 0x1f;

pub(crate) struct Capabilities {
    pub(crate) major_version: u8,
    pub(crate) minor_version: u8,
    pub(crate) capability: u64,
    pub(crate) extended_capability: u64,
}

pub(crate) fn probe(register_base: u64, mapping: MmioMapping) -> Option<Capabilities> {
    let version = read_u32(mapping, register_base.checked_add(VERSION_OFFSET)?)?;
    let capability = read_u64(mapping, register_base.checked_add(CAPABILITY_OFFSET)?)?;
    let extended_capability = read_u64(
        mapping,
        register_base.checked_add(EXTENDED_CAPABILITY_OFFSET)?,
    )?;
    let global_status = read_u32(mapping, register_base.checked_add(GLOBAL_STATUS_OFFSET)?)?;
    let root_table_address = read_u64(
        mapping,
        register_base.checked_add(ROOT_TABLE_ADDRESS_OFFSET)?,
    )?;
    let major_version = ((version >> 4) & 0x0f) as u8;
    let minor_version = (version & 0x0f) as u8;
    if major_version == 0
        || capability == 0
        || capability >> SUPPORTED_ADDRESS_WIDTHS_SHIFT & SUPPORTED_ADDRESS_WIDTHS_MASK == 0
        || global_status & TRANSLATION_ENABLE_STATUS != 0
        || root_table_address != 0
    {
        return None;
    }
    Some(Capabilities {
        major_version,
        minor_version,
        capability,
        extended_capability,
    })
}

fn read_u32(mapping: MmioMapping, physical: u64) -> Option<u32> {
    let address = mapping.address(physical, size_of::<u32>())?;
    Some(u32::from_le(unsafe {
        read_volatile(address as *const u32)
    }))
}

fn read_u64(mapping: MmioMapping, physical: u64) -> Option<u64> {
    let address = mapping.address(physical, size_of::<u64>())?;
    Some(u64::from_le(unsafe {
        read_volatile(address as *const u64)
    }))
}
