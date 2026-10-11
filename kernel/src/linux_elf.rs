const ELF_HEADER_SIZE: usize = 64;
pub(crate) const PROGRAM_HEADER_SIZE: usize = 56;
const MIN_LOAD_SEGMENT_COUNT: usize = 2;
const MAX_LOAD_SEGMENT_COUNT: usize = 3;
const MAX_PROGRAM_HEADER_COUNT: usize = 5;
const PT_LOAD: u32 = 1;
const PT_PHDR: u32 = 6;
const PT_GNU_STACK: u32 = 0x6474_e551;
const ET_EXEC: u16 = 2;
const EM_X86_64: u16 = 62;
const PF_EXECUTE: u32 = 1;
const PF_WRITE: u32 = 2;
const PF_READ: u32 = 4;
const MIN_USER_ADDRESS: u64 = 0x1_0000;
const MAX_USER_ADDRESS: u64 = 1 << 47;
const PAGE_SIZE: usize = 4096;
const MAX_SEGMENT_PAGES: usize = 2;
const FIXTURE_DATA_OFFSET: usize = 0x1000;
const FIXTURE_SIZE: usize = FIXTURE_DATA_OFFSET + 8;
const PROBE_CODE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/linux-probe.bin"));
const LINKED_PROBE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/linux-probe.elf"));

#[derive(Clone, Copy)]
pub(crate) struct Segment<'a> {
    pub(crate) bytes: &'a [u8],
    pub(crate) file_offset: u64,
    pub(crate) virtual_address: u64,
    pub(crate) memory_size: u64,
    pub(crate) flags: u32,
}

pub(crate) struct Image<'a> {
    pub(crate) entry: u64,
    pub(crate) program_headers: u64,
    pub(crate) program_header_count: u16,
    pub(crate) executable: Segment<'a>,
    pub(crate) read_only: Option<Segment<'a>>,
    pub(crate) writable: Segment<'a>,
}

#[derive(Clone, Copy)]
pub(crate) struct PlannedSegment<'a> {
    pub(crate) bytes: &'a [u8],
    pub(crate) virtual_address: u64,
    pub(crate) virtual_page: u64,
    pub(crate) memory_size: usize,
    pub(crate) page_count: usize,
    pub(crate) writable: bool,
    pub(crate) executable: bool,
}

pub(crate) struct LoadPlan<'a> {
    pub(crate) entry: u64,
    pub(crate) program_headers: u64,
    pub(crate) program_header_count: u16,
    pub(crate) executable: PlannedSegment<'a>,
    pub(crate) read_only: Option<PlannedSegment<'a>>,
    pub(crate) writable: PlannedSegment<'a>,
}

pub(crate) fn parse(bytes: &[u8]) -> Option<Image<'_>> {
    if bytes.len() < ELF_HEADER_SIZE + PROGRAM_HEADER_SIZE * MIN_LOAD_SEGMENT_COUNT
        || bytes.get(0..4)? != b"\x7fELF"
        || bytes[4] != 2
        || bytes[5] != 1
        || bytes[6] != 1
        || !matches!(bytes[7], 0 | 3)
        || bytes[8] != 0
        || bytes[9..16].iter().any(|byte| *byte != 0)
        || read_u16(bytes, 16)? != ET_EXEC
        || read_u16(bytes, 18)? != EM_X86_64
        || read_u32(bytes, 20)? != 1
        || read_u64(bytes, 32)? != ELF_HEADER_SIZE as u64
        || read_u32(bytes, 48)? != 0
        || read_u16(bytes, 52)? != ELF_HEADER_SIZE as u16
        || read_u16(bytes, 54)? != PROGRAM_HEADER_SIZE as u16
    {
        return None;
    }
    let program_header_count = usize::from(read_u16(bytes, 56)?);
    if !(MIN_LOAD_SEGMENT_COUNT..=MAX_PROGRAM_HEADER_COUNT).contains(&program_header_count) {
        return None;
    }
    let program_table_end =
        ELF_HEADER_SIZE.checked_add(PROGRAM_HEADER_SIZE.checked_mul(program_header_count)?)?;
    if program_table_end > bytes.len() {
        return None;
    }
    let entry = read_u64(bytes, 24)?;
    let mut loads = [None; MAX_LOAD_SEGMENT_COUNT];
    let mut load_count = 0;
    let mut gnu_stack_seen = false;
    let mut phdr_address = None;
    for index in 0..program_header_count {
        let header = ELF_HEADER_SIZE + index * PROGRAM_HEADER_SIZE;
        match read_u32(bytes, header)? {
            PT_LOAD if load_count < MAX_LOAD_SEGMENT_COUNT => {
                loads[load_count] = Some(parse_segment(bytes, header)?);
                load_count += 1;
            }
            PT_GNU_STACK if !gnu_stack_seen && valid_gnu_stack(bytes, header) => {
                gnu_stack_seen = true;
            }
            PT_PHDR if phdr_address.is_none() => {
                phdr_address = Some(parse_phdr(bytes, header, program_header_count)?);
            }
            _ => return None,
        }
    }
    if !(MIN_LOAD_SEGMENT_COUNT..=MAX_LOAD_SEGMENT_COUNT).contains(&load_count) {
        return None;
    }
    let loads = &loads[..load_count];
    let executable = loads
        .iter()
        .flatten()
        .find(|segment| segment.flags & PF_EXECUTE != 0)
        .copied()?;
    let writable = loads
        .iter()
        .flatten()
        .find(|segment| segment.flags & PF_WRITE != 0)
        .copied()?;
    let executable_count = loads
        .iter()
        .flatten()
        .filter(|segment| segment.flags & PF_EXECUTE != 0)
        .count();
    let writable_count = loads
        .iter()
        .flatten()
        .filter(|segment| segment.flags & PF_WRITE != 0)
        .count();
    let read_only = loads
        .iter()
        .flatten()
        .find(|segment| segment.flags == PF_READ)
        .copied();
    if executable_count != 1
        || writable_count != 1
        || usize::from(read_only.is_some()) + 2 != load_count
        || entry < executable.virtual_address
        || entry
            >= executable
                .virtual_address
                .checked_add(executable.memory_size)?
    {
        return None;
    }
    for (index, first) in loads.iter().flatten().enumerate() {
        let first_end = first.virtual_address.checked_add(first.memory_size)?;
        for second in loads[index + 1..].iter().flatten() {
            let second_end = second.virtual_address.checked_add(second.memory_size)?;
            if first.virtual_address < second_end && second.virtual_address < first_end {
                return None;
            }
        }
    }
    let program_headers = loads.iter().flatten().find_map(|segment| {
        let table_offset = ELF_HEADER_SIZE as u64;
        let table_end = program_table_end as u64;
        (segment.file_offset <= table_offset
            && segment
                .file_offset
                .checked_add(segment.bytes.len() as u64)?
                >= table_end)
            .then(|| {
                segment
                    .virtual_address
                    .checked_add(table_offset - segment.file_offset)
            })?
    })?;
    if phdr_address.is_some_and(|address| address != program_headers) {
        return None;
    }
    Some(Image {
        entry,
        program_headers,
        program_header_count: program_header_count as u16,
        executable,
        read_only,
        writable,
    })
}

pub(crate) fn plan<'a>(image: &Image<'a>) -> Option<LoadPlan<'a>> {
    Some(LoadPlan {
        entry: image.entry,
        program_headers: image.program_headers,
        program_header_count: image.program_header_count,
        executable: plan_segment(image.executable)?,
        read_only: match image.read_only {
            Some(segment) => Some(plan_segment(segment)?),
            None => None,
        },
        writable: plan_segment(image.writable)?,
    })
}

fn plan_segment(segment: Segment<'_>) -> Option<PlannedSegment<'_>> {
    let memory_size = usize::try_from(segment.memory_size).ok()?;
    let virtual_page = segment.virtual_address & !(PAGE_SIZE as u64 - 1);
    let page_offset = usize::try_from(segment.virtual_address - virtual_page).ok()?;
    let mapped_size = page_offset.checked_add(memory_size)?;
    let page_count = mapped_size.checked_add(PAGE_SIZE - 1)? / PAGE_SIZE;
    ((1..=MAX_SEGMENT_PAGES).contains(&page_count) && segment.bytes.len() <= memory_size).then_some(
        PlannedSegment {
            bytes: segment.bytes,
            virtual_address: segment.virtual_address,
            virtual_page,
            memory_size,
            page_count,
            writable: segment.flags & PF_WRITE != 0,
            executable: segment.flags & PF_EXECUTE != 0,
        },
    )
}

pub(crate) fn materialize(segment: PlannedSegment<'_>, page: &mut [u8; PAGE_SIZE]) -> bool {
    materialize_page(segment, 0, page)
}

pub(crate) fn materialize_page(
    segment: PlannedSegment<'_>,
    page_index: usize,
    page: &mut [u8; PAGE_SIZE],
) -> bool {
    if page_index >= segment.page_count || segment.bytes.len() > segment.memory_size {
        return false;
    }
    page.fill(0);
    let Some(page_start) = segment
        .virtual_page
        .checked_add((page_index * PAGE_SIZE) as u64)
    else {
        return false;
    };
    let Some(file_end) = segment
        .virtual_address
        .checked_add(segment.bytes.len() as u64)
    else {
        return false;
    };
    let copy_start = page_start.max(segment.virtual_address);
    let copy_end = page_start.saturating_add(PAGE_SIZE as u64).min(file_end);
    if copy_start < copy_end {
        let source_start =
            usize::try_from(copy_start - segment.virtual_address).unwrap_or(usize::MAX);
        let destination_start = usize::try_from(copy_start - page_start).unwrap_or(usize::MAX);
        let length = usize::try_from(copy_end - copy_start).unwrap_or(usize::MAX);
        let Some(source_end) = source_start.checked_add(length) else {
            return false;
        };
        let Some(destination_end) = destination_start.checked_add(length) else {
            return false;
        };
        let Some(source) = segment.bytes.get(source_start..source_end) else {
            return false;
        };
        let Some(destination) = page.get_mut(destination_start..destination_end) else {
            return false;
        };
        destination.copy_from_slice(source);
    }
    true
}

pub(crate) fn materialized_page_matches(
    segment: PlannedSegment<'_>,
    page_index: usize,
    page: &[u8; PAGE_SIZE],
) -> bool {
    if page_index >= segment.page_count {
        return false;
    }
    let Some(page_start) = segment
        .virtual_page
        .checked_add((page_index * PAGE_SIZE) as u64)
    else {
        return false;
    };
    page.iter().enumerate().all(|(offset, byte)| {
        let address = page_start + offset as u64;
        let source = address
            .checked_sub(segment.virtual_address)
            .and_then(|offset| {
                usize::try_from(offset)
                    .ok()
                    .and_then(|offset| segment.bytes.get(offset))
            });
        *byte == source.copied().unwrap_or(0)
    })
}

fn parse_segment(bytes: &[u8], header: usize) -> Option<Segment<'_>> {
    if read_u32(bytes, header)? != PT_LOAD {
        return None;
    }
    let flags = read_u32(bytes, header + 4)?;
    let offset = usize::try_from(read_u64(bytes, header + 8)?).ok()?;
    let virtual_address = read_u64(bytes, header + 16)?;
    let physical_address = read_u64(bytes, header + 24)?;
    let file_size = usize::try_from(read_u64(bytes, header + 32)?).ok()?;
    let memory_size = read_u64(bytes, header + 40)?;
    let alignment = read_u64(bytes, header + 48)?;
    let end = virtual_address.checked_add(memory_size)?;
    let file_end = offset.checked_add(file_size)?;
    if flags & !(PF_READ | PF_WRITE | PF_EXECUTE) != 0
        || flags & PF_READ == 0
        || flags & (PF_WRITE | PF_EXECUTE) == (PF_WRITE | PF_EXECUTE)
        || physical_address != 0 && physical_address != virtual_address
        || file_size == 0
        || memory_size < file_size as u64
        || virtual_address < MIN_USER_ADDRESS
        || end > MAX_USER_ADDRESS
        || end <= virtual_address
        || file_end > bytes.len()
        || alignment == 0
        || !alignment.is_power_of_two()
        || offset as u64 % alignment != virtual_address % alignment
    {
        return None;
    }
    Some(Segment {
        bytes: bytes.get(offset..file_end)?,
        file_offset: offset as u64,
        virtual_address,
        memory_size,
        flags,
    })
}

fn valid_gnu_stack(bytes: &[u8], header: usize) -> bool {
    read_u32(bytes, header) == Some(PT_GNU_STACK)
        && read_u32(bytes, header + 4) == Some(PF_READ | PF_WRITE)
        && (8..48)
            .step_by(8)
            .all(|offset| read_u64(bytes, header + offset) == Some(0))
        && matches!(read_u64(bytes, header + 48), Some(0 | 16))
}

fn parse_phdr(bytes: &[u8], header: usize, program_header_count: usize) -> Option<u64> {
    let table_size = PROGRAM_HEADER_SIZE.checked_mul(program_header_count)? as u64;
    let virtual_address = read_u64(bytes, header + 16)?;
    let physical_address = read_u64(bytes, header + 24)?;
    (read_u32(bytes, header + 4)? == PF_READ
        && read_u64(bytes, header + 8)? == ELF_HEADER_SIZE as u64
        && (physical_address == 0 || physical_address == virtual_address)
        && read_u64(bytes, header + 32)? == table_size
        && read_u64(bytes, header + 40)? == table_size
        && read_u64(bytes, header + 48)? == 8)
        .then_some(virtual_address)
}

pub(crate) fn verify_parser() -> bool {
    let image = fixture();
    let mut without_stack = image;
    write_u16(&mut without_stack, 56, MIN_LOAD_SEGMENT_COUNT as u16);
    let stack_header = ELF_HEADER_SIZE + 2 * PROGRAM_HEADER_SIZE;
    let phdr_header = ELF_HEADER_SIZE + 3 * PROGRAM_HEADER_SIZE;
    let mut with_phdr = image;
    write_phdr(&mut with_phdr, stack_header, 3);
    let mut with_phdr_and_stack = image;
    write_u16(&mut with_phdr_and_stack, 56, 4);
    write_phdr(&mut with_phdr_and_stack, phdr_header, 4);
    let mut reordered = with_phdr_and_stack;
    swap_program_headers(&mut reordered, ELF_HEADER_SIZE, phdr_header);
    let mut duplicate_stack = with_phdr_and_stack;
    write_gnu_stack(&mut duplicate_stack, phdr_header);
    let mut duplicate_phdr = with_phdr_and_stack;
    write_phdr(&mut duplicate_phdr, stack_header, 4);
    let mut bad_phdr_address = with_phdr;
    write_u64(&mut bad_phdr_address, stack_header + 16, 0x40_0048);
    let mut bad_phdr_size = with_phdr;
    write_u64(&mut bad_phdr_size, stack_header + 32, 1);
    let mut conventional_addresses = with_phdr_and_stack;
    write_u64(&mut conventional_addresses, ELF_HEADER_SIZE + 24, 0x40_0000);
    write_u64(
        &mut conventional_addresses,
        ELF_HEADER_SIZE + PROGRAM_HEADER_SIZE + 24,
        0x40_1000,
    );
    write_u64(&mut conventional_addresses, phdr_header + 24, 0x40_0040);
    let mut zero_aligned_stack = image;
    write_u64(&mut zero_aligned_stack, stack_header + 48, 0);
    let mut three_loads = image;
    let read_only_header = stack_header;
    let moved_stack_header = phdr_header;
    let moved_phdr_header = ELF_HEADER_SIZE + 4 * PROGRAM_HEADER_SIZE;
    write_u16(&mut three_loads, 56, 5);
    write_segment(
        &mut three_loads,
        read_only_header,
        0x300,
        0x40_3000,
        8,
        8,
        PF_READ,
    );
    write_gnu_stack(&mut three_loads, moved_stack_header);
    write_phdr(&mut three_loads, moved_phdr_header, 5);
    let Some(parsed) = parse(&image) else {
        return false;
    };
    if parsed.entry != 0x40_0100
        || parsed.program_headers != 0x40_0040
        || parsed.program_header_count != 3
        || parsed.executable.bytes.get(0x100..) != Some(PROBE_CODE)
        || parsed.executable.virtual_address != 0x40_0000
        || parsed.executable.memory_size != (0x100 + PROBE_CODE.len()) as u64
        || parsed.executable.flags != PF_READ | PF_EXECUTE
        || parsed.read_only.is_some()
        || parsed.writable.bytes != [1, 2, 3, 4, 5, 6, 7, 8]
        || parsed.writable.virtual_address != 0x40_1000
        || parsed.writable.memory_size != (PAGE_SIZE + 512) as u64
        || parsed.writable.flags != PF_READ | PF_WRITE
    {
        return false;
    }
    parse(&without_stack).is_some()
        && parse(&with_phdr).is_some_and(|image| image.program_headers == 0x40_0040)
        && parse(&with_phdr_and_stack).is_some()
        && parse(&reordered).is_some_and(|image| {
            image.executable.flags == PF_READ | PF_EXECUTE
                && image.writable.flags == PF_READ | PF_WRITE
        })
        && parse(&duplicate_stack).is_none()
        && parse(&duplicate_phdr).is_none()
        && parse(&bad_phdr_address).is_none()
        && parse(&bad_phdr_size).is_none()
        && parse(&conventional_addresses).is_some()
        && parse(&zero_aligned_stack).is_some()
        && parse(&three_loads).is_some_and(|image| {
            image.read_only.is_some_and(|segment| {
                segment.flags == PF_READ && segment.virtual_address == 0x40_3000
            }) && plan(&image).is_some_and(|plan| {
                plan.read_only
                    .is_some_and(|segment| segment.virtual_page == 0x40_3000)
            })
        })
        && rejects(&image, 0, b'X')
        && rejects(&image, 4, 1)
        && rejects_u16(&image, 16, 3)
        && rejects_u16(&image, 18, 3)
        && rejects_u64(&image, 24, 0x50_0000)
        && rejects_u32(&image, ELF_HEADER_SIZE + 4, PF_READ | PF_WRITE | PF_EXECUTE)
        && rejects_u64(&image, ELF_HEADER_SIZE + 24, 1)
        && rejects_u64(&image, ELF_HEADER_SIZE + 32, u64::MAX)
        && rejects_u64(
            &image,
            ELF_HEADER_SIZE + PROGRAM_HEADER_SIZE + 16,
            0x40_0000,
        )
        && rejects_u32(&image, ELF_HEADER_SIZE + PROGRAM_HEADER_SIZE, 3)
        && rejects_u32(
            &image,
            ELF_HEADER_SIZE + 2 * PROGRAM_HEADER_SIZE + 4,
            PF_READ | PF_WRITE | PF_EXECUTE,
        )
        && rejects_u16(&image, 56, (MAX_PROGRAM_HEADER_COUNT + 1) as u16)
        && parse(&image[..image.len() - 1]).is_none()
}

pub(crate) fn verify_load_plan() -> bool {
    let image = fixture();
    let Some(parsed) = parse(&image) else {
        return false;
    };
    let Some(current_plan) = plan(&parsed) else {
        return false;
    };
    let mut executable_page = [0xa5_u8; PAGE_SIZE];
    let mut writable_page = [0xa5_u8; PAGE_SIZE];
    let multi_page_image = image;
    let Some(multi_page_plan) = parse(&multi_page_image).and_then(|image| plan(&image)) else {
        return false;
    };
    let mut writable_tail_page = [0xa5_u8; PAGE_SIZE];
    materialize(current_plan.executable, &mut executable_page)
        && materialize(current_plan.writable, &mut writable_page)
        && materialize_page(multi_page_plan.writable, 1, &mut writable_tail_page)
        && current_plan.entry == 0x40_0100
        && current_plan.program_headers == 0x40_0040
        && current_plan.program_header_count == 3
        && current_plan.executable.virtual_page == 0x40_0000
        && current_plan.executable.virtual_address == 0x40_0000
        && current_plan.executable.page_count == 1
        && current_plan.executable.executable
        && !current_plan.executable.writable
        && current_plan.read_only.is_none()
        && executable_page[..4] == *b"\x7fELF"
        && &executable_page[0x100..0x100 + PROBE_CODE.len()] == PROBE_CODE
        && executable_page[0x100 + PROBE_CODE.len()..]
            .iter()
            .all(|byte| *byte == 0)
        && current_plan.writable.virtual_page == 0x40_1000
        && current_plan.writable.virtual_address == 0x40_1000
        && current_plan.writable.page_count == 2
        && current_plan.writable.writable
        && !current_plan.writable.executable
        && writable_page[..8] == [1, 2, 3, 4, 5, 6, 7, 8]
        && writable_page[8..].iter().all(|byte| *byte == 0)
        && multi_page_plan.writable.page_count == 2
        && writable_tail_page.iter().all(|byte| *byte == 0)
        && {
            let mut oversized = multi_page_image;
            write_u64(
                &mut oversized,
                ELF_HEADER_SIZE + PROGRAM_HEADER_SIZE + 40,
                (MAX_SEGMENT_PAGES * PAGE_SIZE + 1) as u64,
            );
            parse(&oversized).is_some_and(|image| plan(&image).is_none())
        }
}

pub(crate) fn verify_linked_probe() -> bool {
    let Some(image) = parse(LINKED_PROBE) else {
        return false;
    };
    let Some(plan) = plan(&image) else {
        return false;
    };
    let Some(read_only) = plan.read_only else {
        return false;
    };
    let mut page = [0xa5_u8; PAGE_SIZE];
    let read_only_valid = materialize(read_only, &mut page)
        && page[..4] == *b"\x7fELF"
        && materialized_page_matches(read_only, 0, &page);
    let executable_valid = materialize(plan.executable, &mut page)
        && page[..0x100].iter().all(|byte| *byte == 0)
        && &page[0x100..0x100 + PROBE_CODE.len()] == PROBE_CODE
        && materialized_page_matches(plan.executable, 0, &page);
    let writable_valid = materialize(plan.writable, &mut page)
        && page[..8] == [1, 2, 3, 4, 5, 6, 7, 8]
        && page[8..].iter().all(|byte| *byte == 0)
        && materialize_page(plan.writable, 1, &mut page)
        && page.iter().all(|byte| *byte == 0);
    image.entry == 0x40_0100
        && image.program_headers == 0x3f_f040
        && image.program_header_count == 5
        && image.executable.virtual_address == 0x40_0100
        && image.executable.bytes == PROBE_CODE
        && image
            .read_only
            .is_some_and(|segment| segment.virtual_address == 0x3f_f000)
        && image.writable.virtual_address == 0x40_1000
        && image.writable.bytes == [1, 2, 3, 4, 5, 6, 7, 8]
        && image.writable.memory_size == 4608
        && plan.executable.virtual_page == 0x40_0000
        && plan.executable.virtual_address == 0x40_0100
        && plan.executable.page_count == 1
        && read_only.virtual_page == 0x3f_f000
        && plan.writable.page_count == 2
        && read_only_valid
        && executable_valid
        && writable_valid
        && crate::address_space::verify_linux_elf_mapping(&plan, 1, 1)
}

pub(crate) fn verify_mapped_load_plan(protected_start: u64, protected_size: usize) -> bool {
    let image = fixture();
    let Some(parsed) = parse(&image) else {
        return false;
    };
    let Some(plan) = plan(&parsed) else {
        return false;
    };
    crate::address_space::verify_linux_elf_mapping(&plan, protected_start, protected_size)
}

fn fixture() -> [u8; FIXTURE_SIZE] {
    let mut bytes = [0_u8; FIXTURE_SIZE];
    bytes[0..4].copy_from_slice(b"\x7fELF");
    bytes[4] = 2;
    bytes[5] = 1;
    bytes[6] = 1;
    write_u16(&mut bytes, 16, ET_EXEC);
    write_u16(&mut bytes, 18, EM_X86_64);
    write_u32(&mut bytes, 20, 1);
    write_u64(&mut bytes, 24, 0x40_0100);
    write_u64(&mut bytes, 32, ELF_HEADER_SIZE as u64);
    write_u16(&mut bytes, 52, ELF_HEADER_SIZE as u16);
    write_u16(&mut bytes, 54, PROGRAM_HEADER_SIZE as u16);
    write_u16(&mut bytes, 56, (MIN_LOAD_SEGMENT_COUNT + 1) as u16);
    write_segment(
        &mut bytes,
        ELF_HEADER_SIZE,
        0,
        0x40_0000,
        (0x100 + PROBE_CODE.len()) as u64,
        (0x100 + PROBE_CODE.len()) as u64,
        PF_READ | PF_EXECUTE,
    );
    write_segment(
        &mut bytes,
        ELF_HEADER_SIZE + PROGRAM_HEADER_SIZE,
        FIXTURE_DATA_OFFSET as u64,
        0x40_1000,
        8,
        (PAGE_SIZE + 512) as u64,
        PF_READ | PF_WRITE,
    );
    let stack_header = ELF_HEADER_SIZE + 2 * PROGRAM_HEADER_SIZE;
    write_gnu_stack(&mut bytes, stack_header);
    bytes[0x100..0x100 + PROBE_CODE.len()].copy_from_slice(PROBE_CODE);
    bytes[FIXTURE_DATA_OFFSET..FIXTURE_SIZE].copy_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8]);
    bytes
}

fn write_gnu_stack(bytes: &mut [u8], header: usize) {
    bytes[header..header + PROGRAM_HEADER_SIZE].fill(0);
    write_u32(bytes, header, PT_GNU_STACK);
    write_u32(bytes, header + 4, PF_READ | PF_WRITE);
    write_u64(bytes, header + 48, 16);
}

fn write_phdr(bytes: &mut [u8], header: usize, program_header_count: usize) {
    bytes[header..header + PROGRAM_HEADER_SIZE].fill(0);
    write_u32(bytes, header, PT_PHDR);
    write_u32(bytes, header + 4, PF_READ);
    write_u64(bytes, header + 8, ELF_HEADER_SIZE as u64);
    write_u64(bytes, header + 16, 0x40_0040);
    let table_size = (program_header_count * PROGRAM_HEADER_SIZE) as u64;
    write_u64(bytes, header + 32, table_size);
    write_u64(bytes, header + 40, table_size);
    write_u64(bytes, header + 48, 8);
}

fn swap_program_headers(bytes: &mut [u8], first: usize, second: usize) {
    let mut saved = [0_u8; PROGRAM_HEADER_SIZE];
    saved.copy_from_slice(&bytes[first..first + PROGRAM_HEADER_SIZE]);
    bytes.copy_within(second..second + PROGRAM_HEADER_SIZE, first);
    bytes[second..second + PROGRAM_HEADER_SIZE].copy_from_slice(&saved);
}

fn write_segment(
    bytes: &mut [u8],
    header: usize,
    offset: u64,
    virtual_address: u64,
    file_size: u64,
    memory_size: u64,
    flags: u32,
) {
    write_u32(bytes, header, PT_LOAD);
    write_u32(bytes, header + 4, flags);
    write_u64(bytes, header + 8, offset);
    write_u64(bytes, header + 16, virtual_address);
    write_u64(bytes, header + 32, file_size);
    write_u64(bytes, header + 40, memory_size);
    write_u64(bytes, header + 48, 8);
}

fn rejects(bytes: &[u8], offset: usize, value: u8) -> bool {
    let mut candidate = fixture();
    candidate[..bytes.len()].copy_from_slice(bytes);
    candidate[offset] = value;
    parse(&candidate).is_none()
}

fn rejects_u16(bytes: &[u8], offset: usize, value: u16) -> bool {
    let mut candidate = fixture();
    candidate[..bytes.len()].copy_from_slice(bytes);
    write_u16(&mut candidate, offset, value);
    parse(&candidate).is_none()
}

fn rejects_u32(bytes: &[u8], offset: usize, value: u32) -> bool {
    let mut candidate = fixture();
    candidate[..bytes.len()].copy_from_slice(bytes);
    write_u32(&mut candidate, offset, value);
    parse(&candidate).is_none()
}

fn rejects_u64(bytes: &[u8], offset: usize, value: u64) -> bool {
    let mut candidate = fixture();
    candidate[..bytes.len()].copy_from_slice(bytes);
    write_u64(&mut candidate, offset, value);
    parse(&candidate).is_none()
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

fn write_u16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn write_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn write_u64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}
