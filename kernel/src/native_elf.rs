const ELF_HEADER_SIZE: usize = 64;
const PROGRAM_HEADER_SIZE: usize = 56;
const DYNAMIC_ENTRY_SIZE: usize = 16;
const RELA_ENTRY_SIZE: usize = 24;
const MAX_FILE_SIZE: usize = 64 * 1024;
const MAX_IMAGE_SIZE: u64 = 64 * 1024;
const MAX_PROGRAM_HEADERS: usize = 8;
const MAX_LOAD_SEGMENTS: usize = 3;
const MAX_RELOCATIONS: usize = 128;
const MAX_SERVICE_RELOCATIONS: usize = 8;
const MAX_CONTROLLER_RELOCATIONS: usize = 128;
const PAGE_SIZE: u64 = 4096;

const ET_DYN: u16 = 3;
const EM_X86_64: u16 = 62;
const PT_LOAD: u32 = 1;
const PT_DYNAMIC: u32 = 2;
const PT_PHDR: u32 = 6;
const PT_GNU_STACK: u32 = 0x6474_e551;
const PT_GNU_RELRO: u32 = 0x6474_e552;
const PF_EXECUTE: u32 = 1;
const PF_WRITE: u32 = 2;
const PF_READ: u32 = 4;

const DT_NULL: u64 = 0;
const DT_HASH: u64 = 4;
const DT_STRTAB: u64 = 5;
const DT_SYMTAB: u64 = 6;
const DT_RELA: u64 = 7;
const DT_RELASZ: u64 = 8;
const DT_RELAENT: u64 = 9;
const DT_STRSZ: u64 = 10;
const DT_SYMENT: u64 = 11;
const DT_DEBUG: u64 = 21;
const DT_FLAGS: u64 = 30;
const DT_GNU_HASH: u64 = 0x6fff_fef5;
const DT_FLAGS_1: u64 = 0x6fff_fffb;
const DT_RELACOUNT: u64 = 0x6fff_fff9;
const DF_BIND_NOW: u64 = 8;
const DF_1_NOW_PIE: u64 = 0x0800_0001;
const R_X86_64_RELATIVE: u64 = 8;

#[derive(Clone, Copy)]
pub(crate) struct LoadSegment {
    pub(crate) file_offset: u64,
    pub(crate) virtual_address: u64,
    pub(crate) file_size: u64,
    pub(crate) memory_size: u64,
    pub(crate) flags: u32,
}

#[derive(Clone, Copy)]
pub(crate) struct RelativeRelocation {
    pub(crate) target: u64,
    pub(crate) addend: u64,
}

pub(crate) struct NativeImage<'a> {
    pub(crate) bytes: &'a [u8],
    pub(crate) entry: u64,
    pub(crate) segments: [Option<LoadSegment>; MAX_LOAD_SEGMENTS],
    pub(crate) relocations: [Option<RelativeRelocation>; MAX_RELOCATIONS],
    pub(crate) relocation_count: usize,
}

impl NativeImage<'_> {
    pub(crate) fn page_count(&self) -> Option<usize> {
        let mut end = 0;
        for segment in self.segments.iter().flatten() {
            end = end.max(segment.virtual_address.checked_add(segment.memory_size)?);
        }
        usize::try_from(end.div_ceil(PAGE_SIZE)).ok()
    }

    pub(crate) fn byte_at_virtual(&self, address: u64) -> Option<u8> {
        let offset = virtual_to_file(&self.segments, address, 1)?;
        self.bytes.get(offset).copied()
    }

    pub(crate) fn page_is_executable(&self, page: usize) -> bool {
        let address = (page as u64).saturating_mul(PAGE_SIZE);
        self.segments.iter().flatten().any(|segment| {
            segment.flags & PF_EXECUTE != 0
                && address >= segment.virtual_address / PAGE_SIZE * PAGE_SIZE
                && address
                    < segment
                        .virtual_address
                        .saturating_add(segment.memory_size)
                        .div_ceil(PAGE_SIZE)
                        * PAGE_SIZE
        })
    }

    pub(crate) fn address_is_executable(&self, address: u64) -> bool {
        self.segments
            .iter()
            .flatten()
            .any(|segment| segment.flags & PF_EXECUTE != 0 && contains_memory(*segment, address, 1))
    }

    pub(crate) fn materialize_page(&self, page: usize, output: &mut [u8; 4096]) -> bool {
        output.fill(0);
        let page_start = (page as u64).saturating_mul(PAGE_SIZE);
        let page_end = page_start.saturating_add(PAGE_SIZE);
        for segment in self.segments.iter().flatten() {
            let segment_file_end = segment.virtual_address.saturating_add(segment.file_size);
            let copy_start = page_start.max(segment.virtual_address);
            let copy_end = page_end.min(segment_file_end);
            if copy_start >= copy_end {
                continue;
            }
            let Some(source_start) = copy_start
                .checked_sub(segment.virtual_address)
                .and_then(|delta| segment.file_offset.checked_add(delta))
                .and_then(|offset| usize::try_from(offset).ok())
            else {
                return false;
            };
            let Some(size) = copy_end
                .checked_sub(copy_start)
                .and_then(|size| usize::try_from(size).ok())
            else {
                return false;
            };
            let Some(destination_start) = copy_start
                .checked_sub(page_start)
                .and_then(|offset| usize::try_from(offset).ok())
            else {
                return false;
            };
            let Some(source) = self
                .bytes
                .get(source_start..source_start.saturating_add(size))
            else {
                return false;
            };
            let Some(destination) =
                output.get_mut(destination_start..destination_start.saturating_add(size))
            else {
                return false;
            };
            destination.copy_from_slice(source);
        }
        true
    }
}

pub(crate) fn inspect(bytes: &[u8]) -> Option<NativeImage<'_>> {
    inspect_with_relocation_limit(bytes, MAX_SERVICE_RELOCATIONS)
}

pub(crate) fn inspect_controller(bytes: &[u8]) -> Option<NativeImage<'_>> {
    inspect_with_relocation_limit(bytes, MAX_CONTROLLER_RELOCATIONS)
}

fn inspect_with_relocation_limit(
    bytes: &[u8],
    maximum_relocations: usize,
) -> Option<NativeImage<'_>> {
    if !(ELF_HEADER_SIZE..=MAX_FILE_SIZE).contains(&bytes.len())
        || bytes.get(..16)? != b"\x7fELF\x02\x01\x01\0\0\0\0\0\0\0\0\0"
        || read_u16(bytes, 16)? != ET_DYN
        || read_u16(bytes, 18)? != EM_X86_64
        || read_u32(bytes, 20)? != 1
        || read_u32(bytes, 48)? != 0
        || usize::from(read_u16(bytes, 52)?) != ELF_HEADER_SIZE
        || usize::from(read_u16(bytes, 54)?) != PROGRAM_HEADER_SIZE
    {
        return None;
    }
    let entry = read_u64(bytes, 24)?;
    let header_offset = usize::try_from(read_u64(bytes, 32)?).ok()?;
    let header_count = usize::from(read_u16(bytes, 56)?);
    let header_bytes = header_count.checked_mul(PROGRAM_HEADER_SIZE)?;
    if !(1..=MAX_PROGRAM_HEADERS).contains(&header_count)
        || header_offset.checked_add(header_bytes)? > bytes.len()
    {
        return None;
    }

    let mut segments: [Option<LoadSegment>; MAX_LOAD_SEGMENTS] = [None; MAX_LOAD_SEGMENTS];
    let mut segment_count = 0;
    let mut dynamic = None;
    let mut stack_seen = false;
    let mut relro = None;
    let mut phdr_seen = false;
    for index in 0..header_count {
        let offset = header_offset.checked_add(index.checked_mul(PROGRAM_HEADER_SIZE)?)?;
        let kind = read_u32(bytes, offset)?;
        let flags = read_u32(bytes, offset + 4)?;
        let file_offset = read_u64(bytes, offset + 8)?;
        let virtual_address = read_u64(bytes, offset + 16)?;
        let physical_address = read_u64(bytes, offset + 24)?;
        let file_size = read_u64(bytes, offset + 32)?;
        let memory_size = read_u64(bytes, offset + 40)?;
        let alignment = read_u64(bytes, offset + 48)?;
        let segment = LoadSegment {
            file_offset,
            virtual_address,
            file_size,
            memory_size,
            flags,
        };
        match kind {
            PT_LOAD => {
                if segment_count == MAX_LOAD_SEGMENTS
                    || physical_address != virtual_address
                    || file_size == 0
                    || file_size > memory_size
                    || alignment != PAGE_SIZE
                    || file_offset % PAGE_SIZE != virtual_address % PAGE_SIZE
                    || flags & !(PF_READ | PF_WRITE | PF_EXECUTE) != 0
                    || flags & PF_READ == 0
                    || flags & (PF_WRITE | PF_EXECUTE) == (PF_WRITE | PF_EXECUTE)
                    || !file_range_valid(bytes, segment)
                    || virtual_address.checked_add(memory_size)? > MAX_IMAGE_SIZE
                {
                    return None;
                }
                let start_page = virtual_address / PAGE_SIZE;
                let end_page = virtual_address
                    .checked_add(memory_size)?
                    .div_ceil(PAGE_SIZE);
                if segments[..segment_count].iter().flatten().any(|prior| {
                    let prior_start = prior.virtual_address / PAGE_SIZE;
                    let prior_end = prior
                        .virtual_address
                        .saturating_add(prior.memory_size)
                        .div_ceil(PAGE_SIZE);
                    start_page < prior_end && prior_start < end_page
                }) {
                    return None;
                }
                segments[segment_count] = Some(segment);
                segment_count += 1;
            }
            PT_DYNAMIC => {
                if dynamic.replace(segment).is_some()
                    || flags != (PF_READ | PF_WRITE)
                    || file_size == 0
                    || file_size != memory_size
                    || file_size % DYNAMIC_ENTRY_SIZE as u64 != 0
                    || !file_range_valid(bytes, segment)
                {
                    return None;
                }
            }
            PT_PHDR => {
                if phdr_seen
                    || flags != PF_READ
                    || file_offset != header_offset as u64
                    || virtual_address != file_offset
                    || physical_address != virtual_address
                    || file_size != header_bytes as u64
                    || memory_size != file_size
                {
                    return None;
                }
                phdr_seen = true;
            }
            PT_GNU_STACK => {
                if stack_seen || flags != (PF_READ | PF_WRITE) || file_size != 0 || memory_size != 0
                {
                    return None;
                }
                stack_seen = true;
            }
            PT_GNU_RELRO => {
                if relro.replace(segment).is_some() || flags != PF_READ || file_size > memory_size {
                    return None;
                }
            }
            _ => return None,
        }
    }
    if segment_count != MAX_LOAD_SEGMENTS || !phdr_seen || !stack_seen {
        return None;
    }
    let read_only = exactly_one_segment(&segments, PF_READ)?;
    let executable = exactly_one_segment(&segments, PF_READ | PF_EXECUTE)?;
    let writable = exactly_one_segment(&segments, PF_READ | PF_WRITE)?;
    if read_only.virtual_address != 0
        || read_only.file_offset != 0
        || !contains_memory(executable, entry, 1)
    {
        return None;
    }
    let dynamic = dynamic?;
    let relro = relro?;
    if !contains_memory(writable, dynamic.virtual_address, dynamic.memory_size)
        || relro.virtual_address != writable.virtual_address
        || relro.memory_size != writable.memory_size
        || relro.file_offset != writable.file_offset
        || relro.file_size != writable.file_size
    {
        return None;
    }
    let (relocations, relocation_count) =
        parse_dynamic(bytes, &segments, dynamic, writable, maximum_relocations)?;
    Some(NativeImage {
        bytes,
        entry,
        segments,
        relocations,
        relocation_count,
    })
}

fn parse_dynamic(
    bytes: &[u8],
    segments: &[Option<LoadSegment>; MAX_LOAD_SEGMENTS],
    dynamic: LoadSegment,
    writable: LoadSegment,
    maximum_relocations: usize,
) -> Option<([Option<RelativeRelocation>; MAX_RELOCATIONS], usize)> {
    let start = usize::try_from(dynamic.file_offset).ok()?;
    let end = start.checked_add(usize::try_from(dynamic.file_size).ok()?)?;
    let mut rela_address = None;
    let mut rela_size = None;
    let mut rela_entry = None;
    let mut rela_count = None;
    let mut bind_now = false;
    let mut pie = false;
    let mut null_seen = false;
    for offset in (start..end).step_by(DYNAMIC_ENTRY_SIZE) {
        let tag = read_u64(bytes, offset)?;
        let value = read_u64(bytes, offset + 8)?;
        if null_seen && (tag != 0 || value != 0) {
            return None;
        }
        match tag {
            DT_NULL => null_seen = true,
            DT_RELA => set_once(&mut rela_address, value)?,
            DT_RELASZ => set_once(&mut rela_size, value)?,
            DT_RELAENT if value == RELA_ENTRY_SIZE as u64 => set_once(&mut rela_entry, value)?,
            DT_RELACOUNT => set_once(&mut rela_count, value)?,
            DT_FLAGS if value == DF_BIND_NOW => bind_now = true,
            DT_FLAGS_1 if value == DF_1_NOW_PIE => pie = true,
            DT_SYMENT if value == 24 => {}
            DT_STRSZ if value == 1 => {}
            DT_HASH | DT_STRTAB | DT_SYMTAB | DT_DEBUG | DT_GNU_HASH => {}
            _ => return None,
        }
    }
    let rela_size = rela_size?;
    let count = usize::try_from(rela_count?).ok()?;
    if !null_seen
        || !bind_now
        || !pie
        || rela_entry.is_none()
        || count == 0
        || count > maximum_relocations
        || rela_size != (count * RELA_ENTRY_SIZE) as u64
    {
        return None;
    }
    let rela_file = virtual_to_file(segments, rela_address?, rela_size)?;
    let mut relocations = [None; MAX_RELOCATIONS];
    for (index, slot) in relocations[..count].iter_mut().enumerate() {
        let offset = rela_file.checked_add(index.checked_mul(RELA_ENTRY_SIZE)?)?;
        let target = read_u64(bytes, offset)?;
        let info = read_u64(bytes, offset + 8)?;
        let addend = read_i64(bytes, offset + 16)?;
        if info != R_X86_64_RELATIVE
            || target % 8 != 0
            || addend < 0
            || !contains_memory(writable, target, 8)
            || !segments
                .iter()
                .flatten()
                .any(|segment| contains_memory(*segment, addend as u64, 1))
        {
            return None;
        }
        *slot = Some(RelativeRelocation {
            target,
            addend: addend as u64,
        });
    }
    Some((relocations, count))
}

fn exactly_one_segment(
    segments: &[Option<LoadSegment>; MAX_LOAD_SEGMENTS],
    flags: u32,
) -> Option<LoadSegment> {
    let mut matches = segments
        .iter()
        .flatten()
        .filter(|segment| segment.flags == flags);
    let result = *matches.next()?;
    matches.next().is_none().then_some(result)
}

fn virtual_to_file(
    segments: &[Option<LoadSegment>; MAX_LOAD_SEGMENTS],
    address: u64,
    size: u64,
) -> Option<usize> {
    let segment = segments
        .iter()
        .flatten()
        .find(|segment| contains_file(**segment, address, size))?;
    let delta = address.checked_sub(segment.virtual_address)?;
    usize::try_from(segment.file_offset.checked_add(delta)?).ok()
}

fn contains_file(segment: LoadSegment, address: u64, size: u64) -> bool {
    address >= segment.virtual_address
        && address
            .checked_add(size)
            .zip(segment.virtual_address.checked_add(segment.file_size))
            .is_some_and(|(end, segment_end)| end <= segment_end)
}

fn contains_memory(segment: LoadSegment, address: u64, size: u64) -> bool {
    address >= segment.virtual_address
        && address
            .checked_add(size)
            .zip(segment.virtual_address.checked_add(segment.memory_size))
            .is_some_and(|(end, segment_end)| end <= segment_end)
}

fn file_range_valid(bytes: &[u8], segment: LoadSegment) -> bool {
    usize::try_from(segment.file_offset)
        .ok()
        .zip(usize::try_from(segment.file_size).ok())
        .and_then(|(start, size)| start.checked_add(size))
        .is_some_and(|end| end <= bytes.len())
}

fn set_once(slot: &mut Option<u64>, value: u64) -> Option<()> {
    if slot.replace(value).is_some() {
        return None;
    }
    Some(())
}

fn read_u16(bytes: &[u8], offset: usize) -> Option<u16> {
    Some(u16::from_le_bytes(
        bytes.get(offset..offset + 2)?.try_into().ok()?,
    ))
}

fn read_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        bytes.get(offset..offset + 4)?.try_into().ok()?,
    ))
}

fn read_u64(bytes: &[u8], offset: usize) -> Option<u64> {
    Some(u64::from_le_bytes(
        bytes.get(offset..offset + 8)?.try_into().ok()?,
    ))
}

fn read_i64(bytes: &[u8], offset: usize) -> Option<i64> {
    Some(i64::from_le_bytes(
        bytes.get(offset..offset + 8)?.try_into().ok()?,
    ))
}
