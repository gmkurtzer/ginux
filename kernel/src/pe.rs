const DOS_MAGIC: u16 = 0x5a4d;
const PE_SIGNATURE: u32 = 0x0000_4550;
const AMD64_MACHINE: u16 = 0x8664;
const PE32_PLUS_MAGIC: u16 = 0x020b;
const COFF_HEADER_SIZE: usize = 20;
const SECTION_HEADER_SIZE: usize = 40;
const MAX_SECTIONS: usize = 96;
const SECTION_EXECUTE: u32 = 0x2000_0000;
const SECTION_WRITE: u32 = 0x8000_0000;
const PAGE_SIZE: usize = 4096;

pub(crate) struct ImageSummary {
    pub(crate) sections: usize,
    pub(crate) executable_sections: usize,
    pub(crate) writable_sections: usize,
    executable_ranges: [(usize, usize); MAX_SECTIONS],
    executable_range_count: usize,
}

impl ImageSummary {
    pub(crate) fn executable_ranges(&self) -> &[(usize, usize)] {
        &self.executable_ranges[..self.executable_range_count]
    }
}

pub(crate) fn inspect(base: *const u8, mapped_size: usize) -> Option<ImageSummary> {
    if base.is_null() || mapped_size < 64 || read_u16(base, mapped_size, 0)? != DOS_MAGIC {
        return None;
    }
    let pe_offset = read_u32(base, mapped_size, 0x3c)? as usize;
    if read_u32(base, mapped_size, pe_offset)? != PE_SIGNATURE {
        return None;
    }
    let coff = pe_offset.checked_add(4)?;
    if read_u16(base, mapped_size, coff)? != AMD64_MACHINE {
        return None;
    }
    let sections = read_u16(base, mapped_size, coff + 2)? as usize;
    let optional_size = read_u16(base, mapped_size, coff + 16)? as usize;
    if sections == 0 || sections > MAX_SECTIONS || optional_size < 64 {
        return None;
    }
    let optional = coff.checked_add(COFF_HEADER_SIZE)?;
    if read_u16(base, mapped_size, optional)? != PE32_PLUS_MAGIC {
        return None;
    }
    let section_alignment = read_u32(base, mapped_size, optional + 32)? as usize;
    let image_size = read_u32(base, mapped_size, optional + 56)? as usize;
    if section_alignment < 4096
        || !section_alignment.is_power_of_two()
        || !(base as usize).is_multiple_of(PAGE_SIZE)
        || image_size == 0
        || image_size > mapped_size
    {
        return None;
    }
    let section_table = optional.checked_add(optional_size)?;
    let section_bytes = sections.checked_mul(SECTION_HEADER_SIZE)?;
    section_table
        .checked_add(section_bytes)
        .filter(|end| *end <= mapped_size)?;

    let mut executable_sections = 0_usize;
    let mut writable_sections = 0_usize;
    let mut executable_ranges = [(0_usize, 0_usize); MAX_SECTIONS];
    let mut executable_range_count = 0_usize;
    for index in 0..sections {
        let section = section_table.checked_add(index.checked_mul(SECTION_HEADER_SIZE)?)?;
        let virtual_size = read_u32(base, mapped_size, section + 8)? as usize;
        let virtual_address = read_u32(base, mapped_size, section + 12)? as usize;
        let characteristics = read_u32(base, mapped_size, section + 36)?;
        if virtual_size != 0
            && (!virtual_address.is_multiple_of(section_alignment)
                || virtual_address
                    .checked_add(virtual_size)
                    .is_none_or(|end| end > image_size))
        {
            return None;
        }
        let executable = characteristics & SECTION_EXECUTE != 0;
        let writable = characteristics & SECTION_WRITE != 0;
        if executable && writable {
            return None;
        }
        if executable && virtual_size != 0 {
            let start = (base as usize).checked_add(virtual_address)?;
            let raw_end = start.checked_add(virtual_size)?;
            let end = raw_end.checked_add(PAGE_SIZE - 1)? / PAGE_SIZE * PAGE_SIZE;
            if end > (base as usize).checked_add(mapped_size)? {
                return None;
            }
            executable_ranges[executable_range_count] = (start, end);
            executable_range_count += 1;
        }
        executable_sections += usize::from(executable);
        writable_sections += usize::from(writable);
    }
    (executable_range_count != 0).then_some(ImageSummary {
        sections,
        executable_sections,
        writable_sections,
        executable_ranges,
        executable_range_count,
    })
}

fn read_u16(base: *const u8, size: usize, offset: usize) -> Option<u16> {
    let end = offset.checked_add(2)?;
    (end <= size).then(|| unsafe { base.add(offset).cast::<u16>().read_unaligned() })
}

fn read_u32(base: *const u8, size: usize, offset: usize) -> Option<u32> {
    let end = offset.checked_add(4)?;
    (end <= size).then(|| unsafe { base.add(offset).cast::<u32>().read_unaligned() })
}
