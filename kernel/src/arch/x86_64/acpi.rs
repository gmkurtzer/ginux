use core::ptr::read_unaligned;

const SDT_HEADER_SIZE: usize = 36;
const MADT_FIXED_SIZE: usize = 44;
const MAX_TABLE_SIZE: usize = 1024 * 1024;
const MAX_ROOT_ENTRIES: usize = 4096;
const MADT_ENABLED: u32 = 1;
const MADT_ONLINE_CAPABLE: u32 = 1 << 1;

#[derive(Clone, Copy)]
pub(crate) struct PlatformInfo {
    pub(crate) enabled_processors: usize,
    pub(crate) io_apics: usize,
    pub(crate) local_apic_address: u64,
    pub(crate) first_io_apic_address: u32,
    pub(crate) first_io_apic_gsi: u32,
}

pub(crate) fn discover(rsdp: *const u8, mapped_limit: usize) -> Option<PlatformInfo> {
    let revision = unsafe { rsdp.add(15).read() };
    let (root_address, entry_size, signature) = if revision >= 2 {
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

    for index in 0..payload_size / entry_size {
        let entry = root_address.checked_add(SDT_HEADER_SIZE + index * entry_size)?;
        let table_address = unsafe {
            if entry_size == 8 {
                usize::try_from(read_unaligned((entry as *const u8).cast::<u64>())).ok()?
            } else {
                read_unaligned((entry as *const u8).cast::<u32>()) as usize
            }
        };
        if signature_at(table_address, b"APIC", mapped_limit) {
            return parse_madt(table_address, mapped_limit);
        }
    }
    None
}

struct ValidatedSdt {
    length: usize,
}

fn validate_sdt(address: usize, signature: &[u8; 4], mapped_limit: usize) -> Option<ValidatedSdt> {
    if !range_is_mapped(address, SDT_HEADER_SIZE, mapped_limit) {
        return None;
    }
    for (offset, expected) in signature.iter().enumerate() {
        if unsafe { (address as *const u8).add(offset).read() } != *expected {
            return None;
        }
    }
    let length = unsafe { read_unaligned((address as *const u8).add(4).cast::<u32>()) } as usize;
    if !(SDT_HEADER_SIZE..=MAX_TABLE_SIZE).contains(&length)
        || !range_is_mapped(address, length, mapped_limit)
        || unsafe { checksum(address as *const u8, length) } != 0
    {
        return None;
    }
    Some(ValidatedSdt { length })
}

fn signature_at(address: usize, signature: &[u8; 4], mapped_limit: usize) -> bool {
    range_is_mapped(address, signature.len(), mapped_limit)
        && signature
            .iter()
            .enumerate()
            .all(|(offset, expected)| unsafe {
                (address as *const u8).add(offset).read() == *expected
            })
}

fn parse_madt(address: usize, mapped_limit: usize) -> Option<PlatformInfo> {
    let table = validate_sdt(address, b"APIC", mapped_limit)?;
    if table.length < MADT_FIXED_SIZE {
        return None;
    }
    let mut info = PlatformInfo {
        enabled_processors: 0,
        io_apics: 0,
        local_apic_address: unsafe {
            u64::from(read_unaligned((address as *const u8).add(36).cast::<u32>()))
        },
        first_io_apic_address: 0,
        first_io_apic_gsi: 0,
    };

    let mut offset = MADT_FIXED_SIZE;
    while offset < table.length {
        if table.length - offset < 2 {
            return None;
        }
        let entry = (address + offset) as *const u8;
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

fn range_is_mapped(address: usize, length: usize, mapped_limit: usize) -> bool {
    length != 0
        && address < mapped_limit
        && address
            .checked_add(length - 1)
            .is_some_and(|end| end < mapped_limit)
}

unsafe fn checksum(bytes: *const u8, length: usize) -> u8 {
    let mut sum = 0_u8;
    for offset in 0..length {
        sum = sum.wrapping_add(bytes.add(offset).read());
    }
    sum
}
