const MAGIC: &[u8; 8] = b"GINXBIN\0";
const VERSION: u16 = 1;
const HEADER_SIZE: usize = 32;
const MAX_CODE_SIZE: usize = 4096;
const MAX_SIZE: usize = HEADER_SIZE + MAX_CODE_SIZE;

pub(crate) struct Image<'a> {
    pub(crate) code: &'a [u8],
    pub(crate) entry_offset: usize,
}

pub(crate) fn parse(bytes: &[u8]) -> Option<Image<'_>> {
    if !(HEADER_SIZE + 1..=MAX_SIZE).contains(&bytes.len()) || &bytes[..8] != MAGIC {
        return None;
    }
    let version = read_u16(bytes, 8)?;
    let header_size = usize::from(read_u16(bytes, 10)?);
    let total_size = usize::try_from(read_u32(bytes, 12)?).ok()?;
    let entry_offset = usize::try_from(read_u32(bytes, 16)?).ok()?;
    let code_size = usize::try_from(read_u32(bytes, 20)?).ok()?;
    let flags = read_u32(bytes, 24)?;
    let reserved = read_u32(bytes, 28)?;
    if version != VERSION
        || header_size != HEADER_SIZE
        || total_size != bytes.len()
        || code_size == 0
        || code_size > MAX_CODE_SIZE
        || entry_offset >= code_size
        || HEADER_SIZE.checked_add(code_size)? != bytes.len()
        || flags != 0
        || reserved != 0
    {
        return None;
    }
    Some(Image {
        code: bytes.get(HEADER_SIZE..)?,
        entry_offset,
    })
}

pub(crate) fn verify_parser_rejections(bytes: &[u8]) -> bool {
    if parse(bytes).is_none() || bytes.len() > MAX_SIZE {
        return false;
    }
    let mut candidate = [0_u8; MAX_SIZE];
    candidate[..bytes.len()].copy_from_slice(bytes);
    candidate[0] ^= 1;
    if parse(&candidate[..bytes.len()]).is_some() {
        return false;
    }
    candidate[..bytes.len()].copy_from_slice(bytes);
    candidate[8] = 2;
    if parse(&candidate[..bytes.len()]).is_some() {
        return false;
    }
    candidate[..bytes.len()].copy_from_slice(bytes);
    candidate[10] = 0;
    if parse(&candidate[..bytes.len()]).is_some() {
        return false;
    }
    candidate[..bytes.len()].copy_from_slice(bytes);
    candidate[12..16].copy_from_slice(&0_u32.to_le_bytes());
    if parse(&candidate[..bytes.len()]).is_some() {
        return false;
    }
    candidate[..bytes.len()].copy_from_slice(bytes);
    candidate[20..24].copy_from_slice(&0_u32.to_le_bytes());
    if parse(&candidate[..bytes.len()]).is_some() {
        return false;
    }
    candidate[..bytes.len()].copy_from_slice(bytes);
    candidate[24] = 1;
    if parse(&candidate[..bytes.len()]).is_some() {
        return false;
    }
    candidate[..bytes.len()].copy_from_slice(bytes);
    candidate[28] = 1;
    if parse(&candidate[..bytes.len()]).is_some() {
        return false;
    }
    candidate[..bytes.len()].copy_from_slice(bytes);
    candidate[16..20].copy_from_slice(&u32::MAX.to_le_bytes());
    parse(&candidate[..bytes.len()]).is_none() && parse(&bytes[..bytes.len() - 1]).is_none()
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
