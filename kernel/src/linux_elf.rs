const ELF_HEADER_SIZE: usize = 64;
pub(crate) const PROGRAM_HEADER_SIZE: usize = 56;
const MIN_LOAD_SEGMENT_COUNT: usize = 2;
const MAX_LOAD_SEGMENT_COUNT: usize = 3;
const MAX_PROGRAM_HEADER_COUNT: usize = 6;
const PT_LOAD: u32 = 1;
const PT_INTERP: u32 = 3;
const PT_PHDR: u32 = 6;
const PT_GNU_STACK: u32 = 0x6474_e551;
const PT_GNU_RELRO: u32 = 0x6474_e552;
const ET_EXEC: u16 = 2;
const ET_DYN: u16 = 3;
const EM_X86_64: u16 = 62;
const PF_EXECUTE: u32 = 1;
const PF_WRITE: u32 = 2;
const PF_READ: u32 = 4;
const MIN_USER_ADDRESS: u64 = 0x1_0000;
const MAX_USER_ADDRESS: u64 = 1 << 47;
const PAGE_SIZE: usize = 4096;
const MAX_EXECUTABLE_SEGMENT_PAGES: usize = 4;
const MAX_OTHER_SEGMENT_PAGES: usize = 2;
const MAX_INTERPRETER_PATH_SIZE: usize = 256;
const FIXTURE_EXECUTABLE_MEMORY_SIZE: u64 = (4 * PAGE_SIZE) as u64;
const FIXTURE_DATA_OFFSET: usize = 0x4000;
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
    pub(crate) kind: ImageKind,
    pub(crate) entry: u64,
    pub(crate) program_headers: u64,
    pub(crate) program_header_count: u16,
    pub(crate) executable: Segment<'a>,
    pub(crate) read_only: Option<Segment<'a>>,
    pub(crate) writable: Segment<'a>,
    pub(crate) relro: Option<Relro>,
    pub(crate) interpreter: Option<&'a [u8]>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ImageKind {
    Executable,
    PositionIndependent,
}

#[derive(Clone, Copy)]
pub(crate) struct Relro {
    pub(crate) virtual_address: u64,
    pub(crate) memory_size: u64,
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
    pub(crate) relro: Option<Relro>,
}

pub(crate) struct InterpretedLoadPlan<'main, 'interpreter> {
    pub(crate) initial_entry: u64,
    pub(crate) main_entry: u64,
    pub(crate) interpreter_base: u64,
    pub(crate) main: LoadPlan<'main>,
    pub(crate) interpreter: LoadPlan<'interpreter>,
}

#[derive(Clone, Copy)]
pub(crate) struct ProcessStartup {
    pub(crate) initial_entry: u64,
    pub(crate) program_headers: u64,
    pub(crate) program_header_count: u16,
    pub(crate) interpreter_base: u64,
    pub(crate) main_entry: u64,
}

pub(crate) const AUXILIARY_VECTOR_WORDS: usize = 36;

pub(crate) fn parse(bytes: &[u8]) -> Option<Image<'_>> {
    if bytes.len() < ELF_HEADER_SIZE + PROGRAM_HEADER_SIZE * MIN_LOAD_SEGMENT_COUNT
        || bytes.get(0..4)? != b"\x7fELF"
        || bytes[4] != 2
        || bytes[5] != 1
        || bytes[6] != 1
        || !matches!(bytes[7], 0 | 3)
        || bytes[8] != 0
        || bytes[9..16].iter().any(|byte| *byte != 0)
        || read_u16(bytes, 18)? != EM_X86_64
        || read_u32(bytes, 20)? != 1
        || read_u64(bytes, 32)? != ELF_HEADER_SIZE as u64
        || read_u32(bytes, 48)? != 0
        || read_u16(bytes, 52)? != ELF_HEADER_SIZE as u16
        || read_u16(bytes, 54)? != PROGRAM_HEADER_SIZE as u16
    {
        return None;
    }
    let kind = match read_u16(bytes, 16)? {
        ET_EXEC => ImageKind::Executable,
        ET_DYN => ImageKind::PositionIndependent,
        _ => return None,
    };
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
    let mut relro = None;
    let mut interpreter = None;
    for index in 0..program_header_count {
        let header = ELF_HEADER_SIZE + index * PROGRAM_HEADER_SIZE;
        match read_u32(bytes, header)? {
            PT_LOAD if load_count < MAX_LOAD_SEGMENT_COUNT => {
                loads[load_count] = Some(parse_segment(
                    bytes,
                    header,
                    kind == ImageKind::PositionIndependent,
                )?);
                load_count += 1;
            }
            PT_GNU_STACK if !gnu_stack_seen && valid_gnu_stack(bytes, header) => {
                gnu_stack_seen = true;
            }
            PT_PHDR if phdr_address.is_none() => {
                phdr_address = Some(parse_phdr(bytes, header, program_header_count)?);
            }
            PT_GNU_RELRO if relro.is_none() => {
                relro = Some(parse_relro(bytes, header)?);
            }
            PT_INTERP if interpreter.is_none() => {
                interpreter = Some(parse_interpreter(bytes, header)?);
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
    if relro.is_some_and(|range| {
        range.virtual_address < writable.virtual_address
            || range.virtual_address % PAGE_SIZE as u64 != 0
            || range.memory_size > PAGE_SIZE as u64
            || range.virtual_address.checked_add(range.memory_size)
                != writable.virtual_address.checked_add(writable.memory_size)
    }) {
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
    let mapped_program_table_end = program_headers
        .checked_add(u64::try_from(program_header_count.checked_mul(PROGRAM_HEADER_SIZE)?).ok()?)?;
    if (program_headers..mapped_program_table_end).contains(&entry) {
        return None;
    }
    Some(Image {
        kind,
        entry,
        program_headers,
        program_header_count: program_header_count as u16,
        executable,
        read_only,
        writable,
        relro,
        interpreter,
    })
}

pub(crate) fn plan<'a>(image: &Image<'a>) -> Option<LoadPlan<'a>> {
    if image.interpreter.is_some() {
        return None;
    }
    plan_executable(image)
}

fn plan_executable<'a>(image: &Image<'a>) -> Option<LoadPlan<'a>> {
    if image.kind != ImageKind::Executable {
        return None;
    }
    let plan = LoadPlan {
        entry: image.entry,
        program_headers: image.program_headers,
        program_header_count: image.program_header_count,
        executable: plan_segment(image.executable)?,
        read_only: match image.read_only {
            Some(segment) => Some(plan_segment(segment)?),
            None => None,
        },
        writable: plan_segment(image.writable)?,
        relro: image.relro,
    };
    (!plan_has_internal_overlap(&plan)).then_some(plan)
}

pub(crate) fn plan_interpreted<'main, 'interpreter>(
    main: &Image<'main>,
    resolved_path: &[u8],
    interpreter: &Image<'interpreter>,
    interpreter_base: u64,
) -> Option<InterpretedLoadPlan<'main, 'interpreter>> {
    if main.kind != ImageKind::Executable || main.interpreter != Some(resolved_path) {
        return None;
    }
    let main = plan_executable(main)?;
    let interpreter = plan_position_independent(interpreter, interpreter_base)?;
    if plans_overlap(&main, &interpreter) {
        return None;
    }
    Some(InterpretedLoadPlan {
        initial_entry: interpreter.entry,
        main_entry: main.entry,
        interpreter_base,
        main,
        interpreter,
    })
}

pub(crate) fn static_startup(plan: &LoadPlan<'_>) -> ProcessStartup {
    ProcessStartup {
        initial_entry: plan.entry,
        program_headers: plan.program_headers,
        program_header_count: plan.program_header_count,
        interpreter_base: 0,
        main_entry: plan.entry,
    }
}

pub(crate) fn interpreted_startup(plan: &InterpretedLoadPlan<'_, '_>) -> ProcessStartup {
    ProcessStartup {
        initial_entry: plan.initial_entry,
        program_headers: plan.main.program_headers,
        program_header_count: plan.main.program_header_count,
        interpreter_base: plan.interpreter_base,
        main_entry: plan.main_entry,
    }
}

pub(crate) fn auxiliary_vector(
    startup: ProcessStartup,
    executable_name: u64,
    platform: u64,
) -> [u64; AUXILIARY_VECTOR_WORDS] {
    [
        3,
        startup.program_headers,
        4,
        PROGRAM_HEADER_SIZE as u64,
        5,
        u64::from(startup.program_header_count),
        6,
        PAGE_SIZE as u64,
        7,
        startup.interpreter_base,
        8,
        0,
        11,
        0,
        12,
        0,
        13,
        0,
        14,
        0,
        23,
        0,
        31,
        executable_name,
        9,
        startup.main_entry,
        17,
        100,
        15,
        platform,
        16,
        0,
        26,
        0,
        0,
        0,
    ]
}

fn plans_overlap(first: &LoadPlan<'_>, second: &LoadPlan<'_>) -> bool {
    planned_segments(first).into_iter().flatten().any(|first| {
        planned_segments(second)
            .into_iter()
            .flatten()
            .any(|second| planned_segments_overlap(first, second))
    })
}

fn plan_has_internal_overlap(plan: &LoadPlan<'_>) -> bool {
    let segments = planned_segments(plan);
    segments
        .iter()
        .enumerate()
        .filter_map(|(index, segment)| segment.map(|segment| (index, segment)))
        .any(|(index, first)| {
            segments[index + 1..]
                .iter()
                .flatten()
                .any(|second| planned_segments_overlap(first, *second))
        })
}

fn planned_segments<'a>(plan: &LoadPlan<'a>) -> [Option<PlannedSegment<'a>>; 3] {
    [Some(plan.executable), plan.read_only, Some(plan.writable)]
}

fn planned_segments_overlap(first: PlannedSegment<'_>, second: PlannedSegment<'_>) -> bool {
    let first_end = first
        .virtual_page
        .checked_add(first.page_count as u64 * PAGE_SIZE as u64);
    let second_end = second
        .virtual_page
        .checked_add(second.page_count as u64 * PAGE_SIZE as u64);
    first_end
        .zip(second_end)
        .is_none_or(|(first_end, second_end)| {
            first.virtual_page < second_end && second.virtual_page < first_end
        })
}

pub(crate) fn plan_position_independent<'a>(
    image: &Image<'a>,
    load_bias: u64,
) -> Option<LoadPlan<'a>> {
    if image.kind != ImageKind::PositionIndependent
        || image.interpreter.is_some()
        || load_bias < MIN_USER_ADDRESS
        || !load_bias.is_multiple_of(PAGE_SIZE as u64)
    {
        return None;
    }
    let executable = relocate_segment(image.executable, load_bias)?;
    let writable = relocate_segment(image.writable, load_bias)?;
    let read_only = match image.read_only {
        Some(segment) => Some(relocate_segment(segment, load_bias)?),
        None => None,
    };
    let relro = match image.relro {
        Some(range) => Some(Relro {
            virtual_address: range.virtual_address.checked_add(load_bias)?,
            memory_size: range.memory_size,
        }),
        None => None,
    };
    let plan = LoadPlan {
        entry: image.entry.checked_add(load_bias)?,
        program_headers: image.program_headers.checked_add(load_bias)?,
        program_header_count: image.program_header_count,
        executable: plan_segment(executable)?,
        read_only: match read_only {
            Some(segment) => Some(plan_segment(segment)?),
            None => None,
        },
        writable: plan_segment(writable)?,
        relro,
    };
    (!plan_has_internal_overlap(&plan)).then_some(plan)
}

fn relocate_segment(segment: Segment<'_>, load_bias: u64) -> Option<Segment<'_>> {
    let virtual_address = segment.virtual_address.checked_add(load_bias)?;
    let end = virtual_address.checked_add(segment.memory_size)?;
    (virtual_address >= MIN_USER_ADDRESS && end <= MAX_USER_ADDRESS).then_some(Segment {
        virtual_address,
        ..segment
    })
}

fn plan_segment(segment: Segment<'_>) -> Option<PlannedSegment<'_>> {
    let memory_size = usize::try_from(segment.memory_size).ok()?;
    let virtual_page = segment.virtual_address & !(PAGE_SIZE as u64 - 1);
    let page_offset = usize::try_from(segment.virtual_address - virtual_page).ok()?;
    let mapped_size = page_offset.checked_add(memory_size)?;
    let page_count = mapped_size.checked_add(PAGE_SIZE - 1)? / PAGE_SIZE;
    let maximum_pages = if segment.flags & PF_EXECUTE != 0 {
        MAX_EXECUTABLE_SEGMENT_PAGES
    } else {
        MAX_OTHER_SEGMENT_PAGES
    };
    ((1..=maximum_pages).contains(&page_count) && segment.bytes.len() <= memory_size).then_some(
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

fn parse_segment(bytes: &[u8], header: usize, position_independent: bool) -> Option<Segment<'_>> {
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
        || !position_independent && virtual_address < MIN_USER_ADDRESS
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

fn parse_relro(bytes: &[u8], header: usize) -> Option<Relro> {
    let file_offset = read_u64(bytes, header + 8)?;
    let virtual_address = read_u64(bytes, header + 16)?;
    let physical_address = read_u64(bytes, header + 24)?;
    let file_size = read_u64(bytes, header + 32)?;
    let memory_size = read_u64(bytes, header + 40)?;
    let alignment = read_u64(bytes, header + 48)?;
    (read_u32(bytes, header + 4)? == PF_READ
        && (physical_address == 0 || physical_address == virtual_address)
        && memory_size != 0
        && file_size <= memory_size
        && alignment != 0
        && alignment.is_power_of_two()
        && file_offset % alignment == virtual_address % alignment)
        .then_some(Relro {
            virtual_address,
            memory_size,
        })
}

fn parse_interpreter(bytes: &[u8], header: usize) -> Option<&[u8]> {
    if read_u32(bytes, header)? != PT_INTERP
        || read_u32(bytes, header + 4)? != PF_READ
        || read_u64(bytes, header + 16)? != 0
        || read_u64(bytes, header + 24)? != 0
        || read_u64(bytes, header + 48)? != 1
    {
        return None;
    }
    let offset = usize::try_from(read_u64(bytes, header + 8)?).ok()?;
    let file_size = usize::try_from(read_u64(bytes, header + 32)?).ok()?;
    if !(2..=MAX_INTERPRETER_PATH_SIZE).contains(&file_size)
        || read_u64(bytes, header + 40)? != file_size as u64
    {
        return None;
    }
    let path = bytes.get(offset..offset.checked_add(file_size)?)?;
    (path[0] == b'/' && path[file_size - 1] == 0 && !path[..file_size - 1].contains(&0))
        .then_some(&path[..file_size - 1])
}

pub(crate) fn verify_parser() -> bool {
    let image = fixture();
    let Some(parsed) = parse(&image) else {
        return false;
    };
    if parsed.kind != ImageKind::Executable
        || parsed.entry != 0x40_0100
        || parsed.program_headers != 0x40_0040
        || parsed.program_header_count != 3
        || parsed.executable.bytes.get(0x100..) != Some(PROBE_CODE)
        || parsed.executable.virtual_address != 0x40_0000
        || parsed.executable.memory_size != FIXTURE_EXECUTABLE_MEMORY_SIZE
        || parsed.executable.flags != PF_READ | PF_EXECUTE
        || parsed.read_only.is_some()
        || parsed.interpreter.is_some()
        || parsed.writable.bytes != *b"PROBE!!\n"
        || parsed.writable.virtual_address != 0x40_4000
        || parsed.writable.memory_size != (PAGE_SIZE + 512) as u64
        || parsed.writable.flags != PF_READ | PF_WRITE
        || parsed
            .relro
            .is_none_or(|range| range.virtual_address != 0x40_5000 || range.memory_size != 512)
    {
        return false;
    }
    if !verify_metadata_parser(&image)
        || !verify_relro_parser()
        || !verify_interpreter_parser(&image)
    {
        return false;
    }
    let rejections = rejects(&image, 0, b'X')
        && rejects(&image, 4, 1)
        && rejects_u16(&image, 16, 1)
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
        && rejects_entry_in_program_headers(&image)
        && rejects_u16(&image, 56, (MAX_PROGRAM_HEADER_COUNT + 1) as u16)
        && parse(&image[..image.len() - 1]).is_none();
    rejections
}

fn rejects_entry_in_program_headers(image: &[u8; FIXTURE_SIZE]) -> bool {
    let mut candidate = *image;
    write_u16(&mut candidate, 56, 4);
    write_gnu_stack(&mut candidate, ELF_HEADER_SIZE + 3 * PROGRAM_HEADER_SIZE);
    parse(&candidate).is_none()
}

#[inline(never)]
fn verify_metadata_parser(image: &[u8; FIXTURE_SIZE]) -> bool {
    let stack_header = ELF_HEADER_SIZE + 2 * PROGRAM_HEADER_SIZE;
    let extended_phdr_header = ELF_HEADER_SIZE + 4 * PROGRAM_HEADER_SIZE;
    let mut candidate = *image;

    write_u16(&mut candidate, 56, MIN_LOAD_SEGMENT_COUNT as u16);
    if parse(&candidate).is_none() {
        return false;
    }

    candidate = *image;
    write_phdr(&mut candidate, stack_header, 3);
    if parse(&candidate).is_none_or(|image| image.program_headers != 0x40_0040) {
        return false;
    }

    candidate = *image;
    configure_extended_metadata(&mut candidate);
    if parse(&candidate).is_none() {
        return false;
    }

    swap_program_headers(&mut candidate, ELF_HEADER_SIZE, extended_phdr_header);
    if parse(&candidate).is_none_or(|image| {
        image.executable.flags != PF_READ | PF_EXECUTE || image.writable.flags != PF_READ | PF_WRITE
    }) {
        return false;
    }

    candidate = *image;
    configure_extended_metadata(&mut candidate);
    write_gnu_stack(&mut candidate, extended_phdr_header);
    if parse(&candidate).is_some() {
        return false;
    }

    candidate = *image;
    configure_extended_metadata(&mut candidate);
    write_phdr(&mut candidate, stack_header, 5);
    if parse(&candidate).is_some() {
        return false;
    }

    candidate = *image;
    write_phdr(&mut candidate, stack_header, 3);
    write_u64(&mut candidate, stack_header + 16, 0x40_0048);
    if parse(&candidate).is_some() {
        return false;
    }

    candidate = *image;
    write_phdr(&mut candidate, stack_header, 3);
    write_u64(&mut candidate, stack_header + 32, 1);
    if parse(&candidate).is_some() {
        return false;
    }

    candidate = *image;
    configure_extended_metadata(&mut candidate);
    write_u64(&mut candidate, ELF_HEADER_SIZE + 24, 0x40_0000);
    write_u64(
        &mut candidate,
        ELF_HEADER_SIZE + PROGRAM_HEADER_SIZE + 24,
        0x40_4000,
    );
    write_u64(&mut candidate, extended_phdr_header + 24, 0x40_0040);
    if parse(&candidate).is_none() {
        return false;
    }

    candidate = *image;
    write_gnu_stack(&mut candidate, stack_header);
    write_u64(&mut candidate, stack_header + 48, 0);
    parse(&candidate).is_some()
}

fn configure_extended_metadata(bytes: &mut [u8]) {
    let phdr_header = ELF_HEADER_SIZE + 3 * PROGRAM_HEADER_SIZE;
    let extended_phdr_header = ELF_HEADER_SIZE + 4 * PROGRAM_HEADER_SIZE;
    write_u16(bytes, 56, 5);
    write_u64(bytes, 24, 0x40_0200);
    write_gnu_stack(bytes, phdr_header);
    write_phdr(bytes, extended_phdr_header, 5);
}

#[inline(never)]
fn verify_interpreter_parser(image: &[u8; FIXTURE_SIZE]) -> bool {
    const PATH: &[u8] = b"/lib64/ld-linux-x86-64.so.2\0";
    const PATH_OFFSET: usize = 0x3f80;
    const SECOND_PATH_OFFSET: usize = 0x3fc0;
    let header = ELF_HEADER_SIZE + 3 * PROGRAM_HEADER_SIZE;
    let duplicate_header = ELF_HEADER_SIZE + 4 * PROGRAM_HEADER_SIZE;
    let mut candidate = *image;
    write_u16(&mut candidate, 56, 4);
    write_u64(&mut candidate, 24, 0x40_0120);
    write_interpreter(&mut candidate, header, PATH_OFFSET, PATH);
    let valid = parse(&candidate).is_some_and(|image| {
        image.interpreter == Some(&PATH[..PATH.len() - 1]) && plan(&image).is_none()
    });

    candidate = *image;
    write_u16(&mut candidate, 56, 5);
    write_interpreter(&mut candidate, header, PATH_OFFSET, PATH);
    write_interpreter(&mut candidate, duplicate_header, SECOND_PATH_OFFSET, PATH);
    let duplicate_rejected = parse(&candidate).is_none();

    candidate = *image;
    write_u16(&mut candidate, 56, 4);
    write_interpreter(&mut candidate, header, PATH_OFFSET, PATH);
    candidate[PATH_OFFSET + PATH.len() - 1] = b'X';
    let unterminated_rejected = parse(&candidate).is_none();

    candidate = *image;
    write_u16(&mut candidate, 56, 4);
    write_interpreter(&mut candidate, header, PATH_OFFSET, PATH);
    candidate[PATH_OFFSET] = b'l';
    let relative_rejected = parse(&candidate).is_none();

    candidate = *image;
    write_u16(&mut candidate, 56, 4);
    write_interpreter(&mut candidate, header, PATH_OFFSET, PATH);
    write_u32(&mut candidate, header + 4, PF_READ | PF_WRITE);
    valid
        && duplicate_rejected
        && unterminated_rejected
        && relative_rejected
        && parse(&candidate).is_none()
}

#[inline(never)]
fn verify_relro_parser() -> bool {
    let stack_header = ELF_HEADER_SIZE + 2 * PROGRAM_HEADER_SIZE;
    let phdr_header = ELF_HEADER_SIZE + 3 * PROGRAM_HEADER_SIZE;
    let mut candidate = fixture();
    write_u16(&mut candidate, 56, 3);
    write_relro(&mut candidate, stack_header, 0x40_5000, 512);
    let valid = parse(&candidate).is_some_and(|image| {
        image
            .relro
            .is_some_and(|range| range.virtual_address == 0x40_5000 && range.memory_size == 512)
    });
    candidate = fixture();
    write_u16(&mut candidate, 56, 3);
    write_relro(&mut candidate, stack_header, 0x40_5000, 512);
    write_u16(&mut candidate, 56, 4);
    write_relro(&mut candidate, phdr_header, 0x40_5000, 512);
    let duplicate_rejected = parse(&candidate).is_none();
    candidate = fixture();
    write_u16(&mut candidate, 56, 3);
    write_relro(&mut candidate, stack_header, 0x40_4001, 512);
    let unaligned_rejected = parse(&candidate).is_none();
    candidate = fixture();
    write_u16(&mut candidate, 56, 3);
    write_relro(
        &mut candidate,
        stack_header,
        0x40_5000,
        PAGE_SIZE as u64 + 1,
    );
    let oversized_rejected = parse(&candidate).is_none();
    candidate = fixture();
    write_u16(&mut candidate, 56, 3);
    write_relro(&mut candidate, stack_header, 0x40_4000, 512);
    valid
        && duplicate_rejected
        && unaligned_rejected
        && oversized_rejected
        && parse(&candidate).is_none()
}

pub(crate) fn verify_load_plan() -> bool {
    let image = fixture();
    let Some(parsed) = parse(&image) else {
        return false;
    };
    let Some(current_plan) = plan(&parsed) else {
        return false;
    };
    if current_plan.entry != 0x40_0100 {
        return false;
    }
    if current_plan.program_headers != 0x40_0040
        || current_plan.program_header_count != 3
        || current_plan.executable.virtual_page != 0x40_0000
        || current_plan.executable.virtual_address != 0x40_0000
        || current_plan.executable.page_count != 4
        || !current_plan.executable.executable
        || current_plan.executable.writable
        || current_plan.read_only.is_some()
        || !current_plan
            .relro
            .is_some_and(|range| range.virtual_address == 0x40_5000 && range.memory_size == 512)
        || current_plan.writable.virtual_page != 0x40_4000
        || current_plan.writable.virtual_address != 0x40_4000
        || current_plan.writable.page_count != 2
        || !current_plan.writable.writable
        || current_plan.writable.executable
    {
        return false;
    }

    let mut page = [0xa5_u8; PAGE_SIZE];
    let first_page_code = PAGE_SIZE - 0x100;
    let second_page_code = (PROBE_CODE.len() - first_page_code).min(PAGE_SIZE);
    let third_page_offset = first_page_code + second_page_code;
    let third_page_code = (PROBE_CODE.len() - third_page_offset).min(PAGE_SIZE);
    let fourth_page_offset = third_page_offset + third_page_code;
    let fourth_page_code = PROBE_CODE.len() - fourth_page_offset;
    if !materialize(current_plan.executable, &mut page)
        || page[..4] != *b"\x7fELF"
        || page[0x100..] != PROBE_CODE[..first_page_code]
        || !materialize_page(current_plan.executable, 1, &mut page)
        || page[..second_page_code]
            != PROBE_CODE[first_page_code..first_page_code + second_page_code]
        || page[second_page_code..].iter().any(|byte| *byte != 0)
        || !materialize_page(current_plan.executable, 2, &mut page)
        || page[..third_page_code]
            != PROBE_CODE[third_page_offset..third_page_offset + third_page_code]
        || page[third_page_code..].iter().any(|byte| *byte != 0)
        || !materialize_page(current_plan.executable, 3, &mut page)
        || page[..fourth_page_code] != PROBE_CODE[fourth_page_offset..]
        || page[fourth_page_code..].iter().any(|byte| *byte != 0)
        || !materialize(current_plan.writable, &mut page)
        || page[..8] != *b"PROBE!!\n"
        || page[8..].iter().any(|byte| *byte != 0)
        || !materialize_page(current_plan.writable, 1, &mut page)
        || page.iter().any(|byte| *byte != 0)
    {
        return false;
    }
    verify_oversized_writable_plan_rejected()
        && verify_page_aliasing_plan_rejected()
        && verify_position_independent_plan()
        && verify_interpreted_plan()
}

#[inline(never)]
fn verify_position_independent_plan() -> bool {
    let mut image = fixture();
    write_u16(&mut image, 16, ET_DYN);
    write_u64(&mut image, 24, 0x100);
    write_u64(&mut image, ELF_HEADER_SIZE + 16, 0);
    write_u64(
        &mut image,
        ELF_HEADER_SIZE + PROGRAM_HEADER_SIZE + 16,
        0x4000,
    );
    let relro_header = ELF_HEADER_SIZE + 2 * PROGRAM_HEADER_SIZE;
    write_u64(&mut image, relro_header + 16, 0x5000);
    write_u64(&mut image, relro_header + 24, 0x5000);
    let Some(parsed) = parse(&image) else {
        return false;
    };
    let Some(relocated) = plan_position_independent(&parsed, 0x60_0000) else {
        return false;
    };
    parsed.kind == ImageKind::PositionIndependent
        && parsed.program_headers == 0x40
        && plan(&parsed).is_none()
        && relocated.entry == 0x60_0100
        && relocated.program_headers == 0x60_0040
        && relocated.executable.virtual_address == 0x60_0000
        && relocated.writable.virtual_address == 0x60_4000
        && relocated
            .relro
            .is_some_and(|range| range.virtual_address == 0x60_5000)
        && plan_position_independent(&parsed, 0).is_none()
        && plan_position_independent(&parsed, 0x60_0001).is_none()
        && plan_position_independent(&parsed, MAX_USER_ADDRESS).is_none()
}

#[inline(never)]
fn verify_interpreted_plan() -> bool {
    const PATH: &[u8] = b"/lib64/ld-ginux.so";
    let main_bytes = interpreted_main_fixture();
    let interpreter_bytes = compact_position_independent_fixture();
    let (Some(main), Some(interpreter)) = (parse(&main_bytes), parse(&interpreter_bytes)) else {
        return false;
    };
    let Some(combined) = plan_interpreted(&main, PATH, &interpreter, 0x60_0000) else {
        return false;
    };
    let startup = interpreted_startup(&combined);
    let auxiliary = auxiliary_vector(startup, 0x70_1000, 0x70_2000);
    startup.initial_entry == 0x60_00f0
        && startup.program_headers == 0x3f_f040
        && startup.program_header_count == 5
        && startup.interpreter_base == 0x60_0000
        && startup.main_entry == 0x40_0100
        && auxiliary[1] == 0x3f_f040
        && auxiliary[5] == 5
        && auxiliary[9] == 0x60_0000
        && auxiliary[23] == 0x70_1000
        && auxiliary[25] == 0x40_0100
        && auxiliary[29] == 0x70_2000
        && auxiliary[34..] == [0, 0]
        && combined.initial_entry == 0x60_00f0
        && combined.main_entry == 0x40_0100
        && combined.interpreter_base == 0x60_0000
        && combined.main.program_headers == 0x3f_f040
        && combined
            .main
            .read_only
            .is_some_and(|segment| segment.virtual_address == 0x3f_f000)
        && combined.interpreter.program_headers == 0x60_0040
        && combined.interpreter.executable.virtual_page == 0x60_0000
        && combined.interpreter.writable.virtual_page == 0x60_3000
        && plan_interpreted(&main, b"/lib64/other.so", &interpreter, 0x60_0000).is_none()
        && plan_interpreted(&main, PATH, &interpreter, 0x40_0000).is_none()
}

#[inline(never)]
fn verify_oversized_writable_plan_rejected() -> bool {
    let mut oversized = fixture();
    write_u64(
        &mut oversized,
        ELF_HEADER_SIZE + PROGRAM_HEADER_SIZE + 40,
        (MAX_OTHER_SEGMENT_PAGES * PAGE_SIZE + 1) as u64,
    );
    write_u16(&mut oversized, 56, MIN_LOAD_SEGMENT_COUNT as u16);
    parse(&oversized).is_some_and(|image| plan(&image).is_none())
}

#[inline(never)]
fn verify_page_aliasing_plan_rejected() -> bool {
    let static_bytes = fixture();
    let Some(mut static_image) = parse(&static_bytes) else {
        return false;
    };
    static_image.read_only = Some(Segment {
        bytes: &static_bytes[..16],
        file_offset: 0,
        virtual_address: 0x40_5200,
        memory_size: 16,
        flags: PF_READ,
    });
    let static_rejected = plan(&static_image).is_none();

    let dynamic_bytes = compact_position_independent_fixture();
    let Some(mut dynamic_image) = parse(&dynamic_bytes) else {
        return false;
    };
    dynamic_image.read_only = Some(Segment {
        bytes: &dynamic_bytes[..16],
        file_offset: 0,
        virtual_address: 0x3100,
        memory_size: 16,
        flags: PF_READ,
    });
    static_rejected && plan_position_independent(&dynamic_image, 0x60_0000).is_none()
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
    let mut executable_tail_page = [0xa5_u8; PAGE_SIZE];
    let mut executable_third_page = [0xa5_u8; PAGE_SIZE];
    let mut executable_fourth_page = [0xa5_u8; PAGE_SIZE];
    let first_page_code = PAGE_SIZE - 0x100;
    let second_page_code = (PROBE_CODE.len() - first_page_code).min(PAGE_SIZE);
    let third_page_offset = first_page_code + second_page_code;
    let third_page_code = (PROBE_CODE.len() - third_page_offset).min(PAGE_SIZE);
    let fourth_page_offset = third_page_offset + third_page_code;
    let fourth_page_code = PROBE_CODE.len() - fourth_page_offset;
    let read_only_valid = materialize(read_only, &mut page)
        && page[..4] == *b"\x7fELF"
        && materialized_page_matches(read_only, 0, &page);
    let executable_valid = materialize(plan.executable, &mut page)
        && page[..0x100].iter().all(|byte| *byte == 0)
        && page[0x100..] == PROBE_CODE[..first_page_code]
        && materialized_page_matches(plan.executable, 0, &page)
        && materialize_page(plan.executable, 1, &mut executable_tail_page)
        && executable_tail_page[..second_page_code]
            == PROBE_CODE[first_page_code..first_page_code + second_page_code]
        && executable_tail_page[second_page_code..]
            .iter()
            .all(|byte| *byte == 0)
        && materialized_page_matches(plan.executable, 1, &executable_tail_page);
    let executable_valid = executable_valid
        && materialize_page(plan.executable, 2, &mut executable_third_page)
        && executable_third_page[..third_page_code]
            == PROBE_CODE[third_page_offset..third_page_offset + third_page_code]
        && executable_third_page[third_page_code..]
            .iter()
            .all(|byte| *byte == 0)
        && materialized_page_matches(plan.executable, 2, &executable_third_page);
    let executable_valid = executable_valid
        && materialize_page(plan.executable, 3, &mut executable_fourth_page)
        && executable_fourth_page[..fourth_page_code] == PROBE_CODE[fourth_page_offset..]
        && executable_fourth_page[fourth_page_code..]
            .iter()
            .all(|byte| *byte == 0)
        && materialized_page_matches(plan.executable, 3, &executable_fourth_page);
    let writable_valid = materialize(plan.writable, &mut page)
        && page[..8] == *b"PROBE!!\n"
        && page[8..].iter().all(|byte| *byte == 0)
        && materialize_page(plan.writable, 1, &mut page)
        && page.iter().all(|byte| *byte == 0);
    image.entry == 0x40_0100
        && image.program_headers == 0x3f_f040
        && image.program_header_count == 6
        && image.executable.virtual_address == 0x40_0100
        && image.executable.bytes.starts_with(PROBE_CODE)
        && image.executable.bytes[PROBE_CODE.len()..]
            .iter()
            .all(|byte| *byte == 0)
        && image
            .read_only
            .is_some_and(|segment| segment.virtual_address == 0x3f_f000)
        && image.writable.virtual_address == 0x40_4000
        && image.writable.bytes.len() == 4608
        && image.writable.bytes[..8] == *b"PROBE!!\n"
        && image.writable.bytes[8..].iter().all(|byte| *byte == 0)
        && image.writable.memory_size == 4608
        && image
            .relro
            .is_some_and(|range| range.virtual_address == 0x40_5000 && range.memory_size == 512)
        && plan
            .relro
            .is_some_and(|range| range.virtual_address == 0x40_5000 && range.memory_size == 512)
        && plan.executable.virtual_page == 0x40_0000
        && plan.executable.virtual_address == 0x40_0100
        && plan.executable.page_count == 4
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

pub(crate) fn verify_mapped_interpreted_plan(protected_start: u64, protected_size: usize) -> bool {
    const PATH: &[u8] = b"/lib64/ld-ginux.so";
    const PATH_WITH_NUL: &[u8] = b"/lib64/ld-ginux.so\0";
    let mut main_bytes = fixture();
    write_u16(&mut main_bytes, 56, 4);
    write_u64(&mut main_bytes, 24, 0x40_0120);
    write_interpreter(
        &mut main_bytes,
        ELF_HEADER_SIZE + 3 * PROGRAM_HEADER_SIZE,
        0x3f80,
        PATH_WITH_NUL,
    );
    let interpreter_bytes = compact_position_independent_fixture();
    let (Some(main), Some(interpreter)) = (parse(&main_bytes), parse(&interpreter_bytes)) else {
        return false;
    };
    let Some(plan) = plan_interpreted(&main, PATH, &interpreter, 0x60_0000) else {
        return false;
    };
    crate::address_space::verify_interpreted_elf_mapping(&plan, protected_start, protected_size)
}

fn interpreted_main_fixture() -> [u8; FIXTURE_SIZE] {
    const PATH: &[u8] = b"/lib64/ld-ginux.so\0";
    const EXECUTABLE_OFFSET: usize = 0x200;
    const PATH_OFFSET: usize = 0x3f80;
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
    write_u16(&mut bytes, 56, 5);
    let program_table_size = (ELF_HEADER_SIZE + 5 * PROGRAM_HEADER_SIZE) as u64;
    write_segment(
        &mut bytes,
        ELF_HEADER_SIZE,
        0,
        0x3f_f000,
        program_table_size,
        program_table_size,
        PF_READ,
    );
    write_segment(
        &mut bytes,
        ELF_HEADER_SIZE + PROGRAM_HEADER_SIZE,
        EXECUTABLE_OFFSET as u64,
        0x40_0100,
        PROBE_CODE.len() as u64,
        (4 * PAGE_SIZE - 0x100) as u64,
        PF_READ | PF_EXECUTE,
    );
    write_segment(
        &mut bytes,
        ELF_HEADER_SIZE + 2 * PROGRAM_HEADER_SIZE,
        FIXTURE_DATA_OFFSET as u64,
        0x40_4000,
        8,
        (PAGE_SIZE + 512) as u64,
        PF_READ | PF_WRITE,
    );
    write_relro(
        &mut bytes,
        ELF_HEADER_SIZE + 3 * PROGRAM_HEADER_SIZE,
        0x40_5000,
        512,
    );
    write_interpreter(
        &mut bytes,
        ELF_HEADER_SIZE + 4 * PROGRAM_HEADER_SIZE,
        PATH_OFFSET,
        PATH,
    );
    bytes[EXECUTABLE_OFFSET..EXECUTABLE_OFFSET + PROBE_CODE.len()].copy_from_slice(PROBE_CODE);
    bytes[FIXTURE_DATA_OFFSET..FIXTURE_SIZE].copy_from_slice(b"PROBE!!\n");
    bytes
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
        FIXTURE_EXECUTABLE_MEMORY_SIZE,
        PF_READ | PF_EXECUTE,
    );
    write_segment(
        &mut bytes,
        ELF_HEADER_SIZE + PROGRAM_HEADER_SIZE,
        FIXTURE_DATA_OFFSET as u64,
        0x40_4000,
        8,
        (PAGE_SIZE + 512) as u64,
        PF_READ | PF_WRITE,
    );
    let stack_header = ELF_HEADER_SIZE + 2 * PROGRAM_HEADER_SIZE;
    write_relro(&mut bytes, stack_header, 0x40_5000, 512);
    bytes[0x100..0x100 + PROBE_CODE.len()].copy_from_slice(PROBE_CODE);
    bytes[FIXTURE_DATA_OFFSET..FIXTURE_SIZE].copy_from_slice(b"PROBE!!\n");
    bytes
}

fn compact_position_independent_fixture() -> [u8; 512] {
    let mut bytes = [0_u8; 512];
    bytes[0..4].copy_from_slice(b"\x7fELF");
    bytes[4] = 2;
    bytes[5] = 1;
    bytes[6] = 1;
    write_u16(&mut bytes, 16, ET_DYN);
    write_u16(&mut bytes, 18, EM_X86_64);
    write_u32(&mut bytes, 20, 1);
    write_u64(&mut bytes, 24, 0xf0);
    write_u64(&mut bytes, 32, ELF_HEADER_SIZE as u64);
    write_u16(&mut bytes, 52, ELF_HEADER_SIZE as u16);
    write_u16(&mut bytes, 54, PROGRAM_HEADER_SIZE as u16);
    write_u16(&mut bytes, 56, MIN_LOAD_SEGMENT_COUNT as u16);
    write_segment(
        &mut bytes,
        ELF_HEADER_SIZE,
        0,
        0,
        0x100,
        0x100,
        PF_READ | PF_EXECUTE,
    );
    write_segment(
        &mut bytes,
        ELF_HEADER_SIZE + PROGRAM_HEADER_SIZE,
        0x100,
        0x3000,
        8,
        0x100,
        PF_READ | PF_WRITE,
    );
    bytes[0xf0] = 0xc3;
    bytes[0x100..0x108].copy_from_slice(b"DYNAMIC!");
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

fn write_interpreter(bytes: &mut [u8], header: usize, offset: usize, path: &[u8]) {
    bytes[header..header + PROGRAM_HEADER_SIZE].fill(0);
    write_u32(bytes, header, PT_INTERP);
    write_u32(bytes, header + 4, PF_READ);
    write_u64(bytes, header + 8, offset as u64);
    write_u64(bytes, header + 32, path.len() as u64);
    write_u64(bytes, header + 40, path.len() as u64);
    write_u64(bytes, header + 48, 1);
    bytes[offset..offset + path.len()].copy_from_slice(path);
}

fn write_relro(bytes: &mut [u8], header: usize, virtual_address: u64, memory_size: u64) {
    bytes[header..header + PROGRAM_HEADER_SIZE].fill(0);
    write_u32(bytes, header, PT_GNU_RELRO);
    write_u32(bytes, header + 4, PF_READ);
    write_u64(bytes, header + 16, virtual_address);
    write_u64(bytes, header + 24, virtual_address);
    write_u64(bytes, header + 40, memory_size);
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
