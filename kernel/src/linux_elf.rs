const ELF_HEADER_SIZE: usize = 64;
const PROGRAM_HEADER_SIZE: usize = 56;
const PROGRAM_HEADER_COUNT: usize = 2;
const PROGRAM_TABLE_END: usize = ELF_HEADER_SIZE + PROGRAM_HEADER_SIZE * PROGRAM_HEADER_COUNT;
const PT_LOAD: u32 = 1;
const ET_EXEC: u16 = 2;
const EM_X86_64: u16 = 62;
const PF_EXECUTE: u32 = 1;
const PF_WRITE: u32 = 2;
const PF_READ: u32 = 4;
const MIN_USER_ADDRESS: u64 = 0x1_0000;
const MAX_USER_ADDRESS: u64 = 1 << 47;
const PAGE_SIZE: usize = 4096;

#[derive(Clone, Copy)]
pub(crate) struct Segment<'a> {
    pub(crate) bytes: &'a [u8],
    pub(crate) virtual_address: u64,
    pub(crate) memory_size: u64,
    pub(crate) flags: u32,
}

pub(crate) struct Image<'a> {
    pub(crate) entry: u64,
    pub(crate) segments: [Segment<'a>; PROGRAM_HEADER_COUNT],
}

#[derive(Clone, Copy)]
pub(crate) struct PlannedSegment<'a> {
    pub(crate) bytes: &'a [u8],
    pub(crate) virtual_page: u64,
    pub(crate) memory_size: usize,
    pub(crate) writable: bool,
    pub(crate) executable: bool,
}

pub(crate) struct LoadPlan<'a> {
    pub(crate) entry: u64,
    pub(crate) segments: [PlannedSegment<'a>; PROGRAM_HEADER_COUNT],
}

pub(crate) fn parse(bytes: &[u8]) -> Option<Image<'_>> {
    if bytes.len() < PROGRAM_TABLE_END
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
        || read_u16(bytes, 56)? != PROGRAM_HEADER_COUNT as u16
        || read_u16(bytes, 58)? != 0
        || read_u16(bytes, 60)? != 0
        || read_u16(bytes, 62)? != 0
    {
        return None;
    }
    let entry = read_u64(bytes, 24)?;
    let first = parse_segment(bytes, ELF_HEADER_SIZE)?;
    let second = parse_segment(bytes, ELF_HEADER_SIZE + PROGRAM_HEADER_SIZE)?;
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
    Some(Image { entry, segments })
}

pub(crate) fn plan<'a>(image: &Image<'a>) -> Option<LoadPlan<'a>> {
    let segments = image.segments.map(|segment| {
        let memory_size = usize::try_from(segment.memory_size).ok()?;
        (segment.virtual_address.is_multiple_of(PAGE_SIZE as u64)
            && memory_size <= PAGE_SIZE
            && segment.bytes.len() <= memory_size)
            .then_some(PlannedSegment {
                bytes: segment.bytes,
                virtual_page: segment.virtual_address,
                memory_size,
                writable: segment.flags & PF_WRITE != 0,
                executable: segment.flags & PF_EXECUTE != 0,
            })
    });
    Some(LoadPlan {
        entry: image.entry,
        segments: [segments[0]?, segments[1]?],
    })
}

pub(crate) fn materialize(segment: PlannedSegment<'_>, page: &mut [u8; PAGE_SIZE]) -> bool {
    if segment.memory_size > page.len() || segment.bytes.len() > segment.memory_size {
        return false;
    }
    page.fill(0);
    page[..segment.bytes.len()].copy_from_slice(segment.bytes);
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
        || offset < PROGRAM_TABLE_END
        || file_end > bytes.len()
        || alignment == 0
        || !alignment.is_power_of_two()
        || offset as u64 % alignment != virtual_address % alignment
    {
        return None;
    }
    Some(Segment {
        bytes: bytes.get(offset..file_end)?,
        virtual_address,
        memory_size,
        flags,
    })
}

pub(crate) fn verify_parser() -> bool {
    let image = fixture();
    let Some(parsed) = parse(&image) else {
        return false;
    };
    if parsed.entry != 0x40_0000
        || parsed.segments[0].bytes != [0xc3]
        || parsed.segments[0].virtual_address != 0x40_0000
        || parsed.segments[0].memory_size != 1
        || parsed.segments[0].flags != PF_READ | PF_EXECUTE
        || parsed.segments[1].bytes != [1, 2, 3, 4, 5, 6, 7, 8]
        || parsed.segments[1].virtual_address != 0x40_1000
        || parsed.segments[1].memory_size != 16
        || parsed.segments[1].flags != PF_READ | PF_WRITE
    {
        return false;
    }
    rejects(&image, 0, b'X')
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
        && parse(&image[..image.len() - 1]).is_none()
}

pub(crate) fn verify_load_plan() -> bool {
    let image = fixture();
    let Some(parsed) = parse(&image) else {
        return false;
    };
    let Some(plan) = plan(&parsed) else {
        return false;
    };
    let mut executable_page = [0xa5_u8; PAGE_SIZE];
    let mut writable_page = [0xa5_u8; PAGE_SIZE];
    materialize(plan.segments[0], &mut executable_page)
        && materialize(plan.segments[1], &mut writable_page)
        && plan.entry == 0x40_0000
        && plan.segments[0].virtual_page == 0x40_0000
        && plan.segments[0].executable
        && !plan.segments[0].writable
        && executable_page[0] == 0xc3
        && executable_page[1..].iter().all(|byte| *byte == 0)
        && plan.segments[1].virtual_page == 0x40_1000
        && plan.segments[1].writable
        && !plan.segments[1].executable
        && writable_page[..8] == [1, 2, 3, 4, 5, 6, 7, 8]
        && writable_page[8..].iter().all(|byte| *byte == 0)
}

fn fixture() -> [u8; 272] {
    let mut bytes = [0_u8; 272];
    bytes[0..4].copy_from_slice(b"\x7fELF");
    bytes[4] = 2;
    bytes[5] = 1;
    bytes[6] = 1;
    write_u16(&mut bytes, 16, ET_EXEC);
    write_u16(&mut bytes, 18, EM_X86_64);
    write_u32(&mut bytes, 20, 1);
    write_u64(&mut bytes, 24, 0x40_0000);
    write_u64(&mut bytes, 32, ELF_HEADER_SIZE as u64);
    write_u16(&mut bytes, 52, ELF_HEADER_SIZE as u16);
    write_u16(&mut bytes, 54, PROGRAM_HEADER_SIZE as u16);
    write_u16(&mut bytes, 56, PROGRAM_HEADER_COUNT as u16);
    write_segment(
        &mut bytes,
        ELF_HEADER_SIZE,
        0x100,
        0x40_0000,
        1,
        1,
        PF_READ | PF_EXECUTE,
    );
    write_segment(
        &mut bytes,
        ELF_HEADER_SIZE + PROGRAM_HEADER_SIZE,
        0x108,
        0x40_1000,
        8,
        16,
        PF_READ | PF_WRITE,
    );
    bytes[0x100] = 0xc3;
    bytes[0x108..0x110].copy_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8]);
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
