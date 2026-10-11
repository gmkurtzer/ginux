const ELF_HEADER_SIZE: usize = 64;
pub(crate) const PROGRAM_HEADER_SIZE: usize = 56;
const LOAD_SEGMENT_COUNT: usize = 2;
const MAX_PROGRAM_HEADER_COUNT: usize = 3;
const PT_LOAD: u32 = 1;
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
const FIXTURE_DATA_OFFSET: usize = 0x800;
const FIXTURE_SIZE: usize = FIXTURE_DATA_OFFSET + 8;
const PROBE_CODE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/linux-probe.bin"));

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
    pub(crate) segments: [Segment<'a>; LOAD_SEGMENT_COUNT],
}

#[derive(Clone, Copy)]
pub(crate) struct PlannedSegment<'a> {
    pub(crate) bytes: &'a [u8],
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
    pub(crate) segments: [PlannedSegment<'a>; LOAD_SEGMENT_COUNT],
}

pub(crate) fn parse(bytes: &[u8]) -> Option<Image<'_>> {
    if bytes.len() < ELF_HEADER_SIZE + PROGRAM_HEADER_SIZE * LOAD_SEGMENT_COUNT
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
        || read_u64(bytes, 40)? != 0
        || read_u32(bytes, 48)? != 0
        || read_u16(bytes, 52)? != ELF_HEADER_SIZE as u16
        || read_u16(bytes, 54)? != PROGRAM_HEADER_SIZE as u16
        || read_u16(bytes, 58)? != 0
        || read_u16(bytes, 60)? != 0
        || read_u16(bytes, 62)? != 0
    {
        return None;
    }
    let program_header_count = usize::from(read_u16(bytes, 56)?);
    if !(LOAD_SEGMENT_COUNT..=MAX_PROGRAM_HEADER_COUNT).contains(&program_header_count) {
        return None;
    }
    let program_table_end =
        ELF_HEADER_SIZE.checked_add(PROGRAM_HEADER_SIZE.checked_mul(program_header_count)?)?;
    if program_table_end > bytes.len() {
        return None;
    }
    let entry = read_u64(bytes, 24)?;
    let first = parse_segment(bytes, ELF_HEADER_SIZE)?;
    let second = parse_segment(bytes, ELF_HEADER_SIZE + PROGRAM_HEADER_SIZE)?;
    if program_header_count == MAX_PROGRAM_HEADER_COUNT
        && !valid_gnu_stack(bytes, ELF_HEADER_SIZE + 2 * PROGRAM_HEADER_SIZE)
    {
        return None;
    }
    let first_end = first.virtual_address.checked_add(first.memory_size)?;
    let second_end = second.virtual_address.checked_add(second.memory_size)?;
    if first.virtual_address < second_end && second.virtual_address < first_end {
        return None;
    }
    let segments = [first, second];
    let executable_count = segments
        .iter()
        .filter(|segment| segment.flags & PF_EXECUTE != 0)
        .count();
    let writable_count = segments
        .iter()
        .filter(|segment| segment.flags & PF_WRITE != 0)
        .count();
    if executable_count != 1
        || writable_count != 1
        || !segments.iter().any(|segment| {
            segment.flags & PF_EXECUTE != 0
                && entry >= segment.virtual_address
                && entry < segment.virtual_address + segment.memory_size
        })
    {
        return None;
    }
    let program_headers = segments.iter().find_map(|segment| {
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
    Some(Image {
        entry,
        program_headers,
        program_header_count: program_header_count as u16,
        segments,
    })
}

pub(crate) fn plan<'a>(image: &Image<'a>) -> Option<LoadPlan<'a>> {
    let segments = image.segments.map(|segment| {
        let memory_size = usize::try_from(segment.memory_size).ok()?;
        let page_count = memory_size.checked_add(PAGE_SIZE - 1)? / PAGE_SIZE;
        (segment.virtual_address.is_multiple_of(PAGE_SIZE as u64)
            && (1..=MAX_SEGMENT_PAGES).contains(&page_count)
            && segment.bytes.len() <= memory_size)
            .then_some(PlannedSegment {
                bytes: segment.bytes,
                virtual_page: segment.virtual_address,
                memory_size,
                page_count,
                writable: segment.flags & PF_WRITE != 0,
                executable: segment.flags & PF_EXECUTE != 0,
            })
    });
    Some(LoadPlan {
        entry: image.entry,
        program_headers: image.program_headers,
        program_header_count: image.program_header_count,
        segments: [segments[0]?, segments[1]?],
    })
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
    let Some(segment_offset) = page_index.checked_mul(PAGE_SIZE) else {
        return false;
    };
    if segment_offset < segment.bytes.len() {
        let file_end = segment
            .bytes
            .len()
            .min(segment_offset.saturating_add(PAGE_SIZE));
        let file_bytes = &segment.bytes[segment_offset..file_end];
        page[..file_bytes.len()].copy_from_slice(file_bytes);
    }
    true
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
        || physical_address != 0
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
        && read_u64(bytes, header + 48) == Some(16)
}

pub(crate) fn verify_parser() -> bool {
    let image = fixture();
    let mut without_stack = image;
    write_u16(&mut without_stack, 56, LOAD_SEGMENT_COUNT as u16);
    let Some(parsed) = parse(&image) else {
        return false;
    };
    if parsed.entry != 0x40_0100
        || parsed.program_headers != 0x40_0040
        || parsed.program_header_count != 3
        || parsed.segments[0].bytes.get(0x100..) != Some(PROBE_CODE)
        || parsed.segments[0].virtual_address != 0x40_0000
        || parsed.segments[0].memory_size != (0x100 + PROBE_CODE.len()) as u64
        || parsed.segments[0].flags != PF_READ | PF_EXECUTE
        || parsed.segments[1].bytes != [1, 2, 3, 4, 5, 6, 7, 8]
        || parsed.segments[1].virtual_address != 0x40_1000
        || parsed.segments[1].memory_size != (PAGE_SIZE + 512) as u64
        || parsed.segments[1].flags != PF_READ | PF_WRITE
    {
        return false;
    }
    parse(&without_stack).is_some()
        && rejects(&image, 0, b'X')
        && rejects(&image, 4, 1)
        && rejects_u16(&image, 16, 3)
        && rejects_u16(&image, 18, 3)
        && rejects_u64(&image, 24, 0x50_0000)
        && rejects_u32(&image, ELF_HEADER_SIZE + 4, PF_READ | PF_WRITE | PF_EXECUTE)
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
    materialize(current_plan.segments[0], &mut executable_page)
        && materialize(current_plan.segments[1], &mut writable_page)
        && materialize_page(multi_page_plan.segments[1], 1, &mut writable_tail_page)
        && current_plan.entry == 0x40_0100
        && current_plan.program_headers == 0x40_0040
        && current_plan.program_header_count == 3
        && current_plan.segments[0].virtual_page == 0x40_0000
        && current_plan.segments[0].page_count == 1
        && current_plan.segments[0].executable
        && !current_plan.segments[0].writable
        && executable_page[..4] == *b"\x7fELF"
        && &executable_page[0x100..0x100 + PROBE_CODE.len()] == PROBE_CODE
        && executable_page[0x100 + PROBE_CODE.len()..]
            .iter()
            .all(|byte| *byte == 0)
        && current_plan.segments[1].virtual_page == 0x40_1000
        && current_plan.segments[1].page_count == 2
        && current_plan.segments[1].writable
        && !current_plan.segments[1].executable
        && writable_page[..8] == [1, 2, 3, 4, 5, 6, 7, 8]
        && writable_page[8..].iter().all(|byte| *byte == 0)
        && multi_page_plan.segments[1].page_count == 2
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
    write_u16(&mut bytes, 56, MAX_PROGRAM_HEADER_COUNT as u16);
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
    write_u32(&mut bytes, stack_header, PT_GNU_STACK);
    write_u32(&mut bytes, stack_header + 4, PF_READ | PF_WRITE);
    write_u64(&mut bytes, stack_header + 48, 16);
    bytes[0x100..0x100 + PROBE_CODE.len()].copy_from_slice(PROBE_CODE);
    bytes[FIXTURE_DATA_OFFSET..FIXTURE_SIZE].copy_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8]);
    bytes
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
