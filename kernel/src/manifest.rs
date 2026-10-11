use crate::crypto;

const MAGIC: &[u8; 8] = b"GINXMNF\0";
const VERSION: u16 = 1;
const HEADER_SIZE: usize = 32;
const GRANT_SIZE: usize = 16;
const MAX_GRANTS: usize = 8;
const ALLOWED_RIGHTS: u64 = 0x0f;

#[derive(Clone, Copy)]
pub(crate) struct Grant {
    pub(crate) object: u64,
    pub(crate) rights: u64,
}

const EMPTY_GRANT: Grant = Grant {
    object: 0,
    rights: 0,
};

pub(crate) struct Manifest {
    sequence: u64,
    grants: [Grant; MAX_GRANTS],
    grant_count: usize,
}

impl Manifest {
    pub(crate) fn parse_and_measure(
        pointer: *const u8,
        size: usize,
        expected_digest: &[u8; 32],
        mapped_limit: usize,
    ) -> Option<Self> {
        if pointer.is_null()
            || !(HEADER_SIZE..=HEADER_SIZE + MAX_GRANTS * GRANT_SIZE).contains(&size)
        {
            return None;
        }
        let address = pointer as usize;
        if address >= mapped_limit || address.checked_add(size - 1)? >= mapped_limit {
            return None;
        }
        // SAFETY: BootInfo provides an immutable buffer with `size` bytes. Its
        // complete mapped range is validated before page-table activation.
        let bytes = unsafe { core::slice::from_raw_parts(pointer, size) };
        if crypto::sha256(bytes) != *expected_digest || &bytes[..8] != MAGIC {
            return None;
        }
        let version = read_u16(bytes, 8)?;
        let header_size = usize::from(read_u16(bytes, 10)?);
        let total_size = usize::try_from(read_u32(bytes, 12)?).ok()?;
        let sequence = read_u64(bytes, 16)?;
        let grant_count = usize::from(read_u16(bytes, 24)?);
        let flags = read_u16(bytes, 26)?;
        let reserved = read_u32(bytes, 28)?;
        if version != VERSION
            || header_size != HEADER_SIZE
            || total_size != size
            || sequence == 0
            || grant_count == 0
            || grant_count > MAX_GRANTS
            || flags != 0
            || reserved != 0
            || HEADER_SIZE.checked_add(grant_count.checked_mul(GRANT_SIZE)?)? != size
        {
            return None;
        }

        let mut grants = [EMPTY_GRANT; MAX_GRANTS];
        for index in 0..grant_count {
            let offset = HEADER_SIZE + index * GRANT_SIZE;
            let grant = Grant {
                object: read_u64(bytes, offset)?,
                rights: read_u64(bytes, offset + 8)?,
            };
            if grant.object == 0
                || grant.rights == 0
                || grant.rights & !ALLOWED_RIGHTS != 0
                || grants[..index]
                    .iter()
                    .any(|existing| existing.object == grant.object)
            {
                return None;
            }
            grants[index] = grant;
        }
        Some(Self {
            sequence,
            grants,
            grant_count,
        })
    }

    pub(crate) const fn sequence(&self) -> u64 {
        self.sequence
    }

    pub(crate) const fn grant_count(&self) -> usize {
        self.grant_count
    }

    pub(crate) fn grants(&self) -> &[Grant] {
        &self.grants[..self.grant_count]
    }
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
