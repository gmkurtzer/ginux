use crate::crypto;
use ed25519_dalek::{Signature, VerifyingKey};

const MAGIC: &[u8; 8] = b"GINXMNF\0";
const VERSION: u16 = 3;
const HEADER_SIZE: usize = 112;
const GRANT_SIZE: usize = 16;
const MAX_GRANTS: usize = 8;
const MAX_ROOT_REJECTION_PROOF_SIZE: usize = 4096;
const ALLOWED_RIGHTS: u64 = 0x0f;
const DEVELOPMENT_PUBLIC_KEY: [u8; 32] = [
    0xd7, 0x5a, 0x98, 0x01, 0x82, 0xb1, 0x0a, 0xb7, 0xd5, 0x4b, 0xfe, 0xd3, 0xc9, 0x64, 0x07, 0x3a,
    0x0e, 0xe1, 0x72, 0xf3, 0xda, 0xa6, 0x23, 0x25, 0xaf, 0x02, 0x1a, 0x68, 0xf7, 0x07, 0x51, 0x1a,
];

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
    bundle_size: usize,
    bundle_sha256: [u8; 32],
    root_size: usize,
    root_sha256: [u8; 32],
}

impl Manifest {
    pub(crate) fn parse_and_verify(
        pointer: *const u8,
        size: usize,
        expected_digest: &[u8; 32],
        signature: &[u8; 64],
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
        if crypto::sha256(bytes) != *expected_digest
            || !verify_development_signature(bytes, signature)
        {
            return None;
        }
        parse_canonical(bytes)
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

    pub(crate) fn bundle_matches(
        &self,
        pointer: *const u8,
        size: usize,
        mapped_limit: usize,
    ) -> bool {
        if pointer.is_null() || size != self.bundle_size {
            return false;
        }
        let address = pointer as usize;
        if address >= mapped_limit
            || address
                .checked_add(size.saturating_sub(1))
                .is_none_or(|end| end >= mapped_limit)
        {
            return false;
        }
        let bytes = unsafe { core::slice::from_raw_parts(pointer, size) };
        crypto::sha256(bytes) == self.bundle_sha256
    }

    pub(crate) fn root_matches(
        &self,
        pointer: *const u8,
        size: usize,
        mapped_limit: usize,
    ) -> bool {
        if pointer.is_null() || size != self.root_size {
            return false;
        }
        let address = pointer as usize;
        if address >= mapped_limit
            || address
                .checked_add(size.saturating_sub(1))
                .is_none_or(|end| end >= mapped_limit)
        {
            return false;
        }
        let bytes = unsafe { core::slice::from_raw_parts(pointer, size) };
        crypto::sha256(bytes) == self.root_sha256
    }

    pub(crate) fn verify_root_rejection(
        &self,
        pointer: *const u8,
        size: usize,
        mapped_limit: usize,
    ) -> bool {
        if !self.root_matches(pointer, size, mapped_limit)
            || size == 0
            || size > MAX_ROOT_REJECTION_PROOF_SIZE
        {
            return false;
        }
        let source = unsafe { core::slice::from_raw_parts(pointer, size) };
        let mut candidate = [0_u8; MAX_ROOT_REJECTION_PROOF_SIZE];
        candidate[..size].copy_from_slice(source);
        candidate[0] ^= 1;
        !self.root_matches(candidate.as_ptr(), size, mapped_limit)
    }
}

fn parse_canonical(bytes: &[u8]) -> Option<Manifest> {
    if !(HEADER_SIZE..=HEADER_SIZE + MAX_GRANTS * GRANT_SIZE).contains(&bytes.len())
        || &bytes[..8] != MAGIC
    {
        return None;
    }
    let version = read_u16(bytes, 8)?;
    let header_size = usize::from(read_u16(bytes, 10)?);
    let total_size = usize::try_from(read_u32(bytes, 12)?).ok()?;
    let sequence = read_u64(bytes, 16)?;
    let grant_count = usize::from(read_u16(bytes, 24)?);
    let flags = read_u16(bytes, 26)?;
    let reserved = read_u32(bytes, 28)?;
    let bundle_size = usize::try_from(read_u64(bytes, 32)?).ok()?;
    let bundle_sha256: [u8; 32] = bytes.get(40..72)?.try_into().ok()?;
    let root_size = usize::try_from(read_u64(bytes, 72)?).ok()?;
    let root_sha256: [u8; 32] = bytes.get(80..112)?.try_into().ok()?;
    if version != VERSION
        || header_size != HEADER_SIZE
        || total_size != bytes.len()
        || sequence == 0
        || grant_count == 0
        || grant_count > MAX_GRANTS
        || flags != 0
        || reserved != 0
        || bundle_size == 0
        || root_size == 0
        || HEADER_SIZE.checked_add(grant_count.checked_mul(GRANT_SIZE)?)? != bytes.len()
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
    Some(Manifest {
        sequence,
        grants,
        grant_count,
        bundle_size,
        bundle_sha256,
        root_size,
        root_sha256,
    })
}

pub(crate) fn verify_parser_rejections(pointer: *const u8, size: usize) -> bool {
    const MAX_SIZE: usize = HEADER_SIZE + MAX_GRANTS * GRANT_SIZE;
    if pointer.is_null() || !(HEADER_SIZE + 2 * GRANT_SIZE..=MAX_SIZE).contains(&size) {
        return false;
    }
    let original = unsafe { core::slice::from_raw_parts(pointer, size) };
    if parse_canonical(original).is_none() || parse_canonical(&original[..size - 1]).is_some() {
        return false;
    }

    let mut candidate = [0_u8; MAX_SIZE];
    candidate[..size].copy_from_slice(original);
    candidate[28] = 1;
    if parse_canonical(&candidate[..size]).is_some() {
        return false;
    }
    candidate[..size].copy_from_slice(original);
    candidate[72..80].copy_from_slice(&0_u64.to_le_bytes());
    if parse_canonical(&candidate[..size]).is_some() {
        return false;
    }
    candidate[..size].copy_from_slice(original);
    candidate[HEADER_SIZE + 8] = 0x80;
    if parse_canonical(&candidate[..size]).is_some() {
        return false;
    }
    candidate[..size].copy_from_slice(original);
    let mut first_object = [0_u8; 8];
    first_object.copy_from_slice(&candidate[HEADER_SIZE..HEADER_SIZE + 8]);
    candidate[HEADER_SIZE + GRANT_SIZE..HEADER_SIZE + GRANT_SIZE + 8]
        .copy_from_slice(&first_object);
    parse_canonical(&candidate[..size]).is_none()
}

pub(crate) fn verify_signature_rejection(
    pointer: *const u8,
    size: usize,
    signature: &[u8; 64],
) -> bool {
    if pointer.is_null() || size == 0 {
        return false;
    }
    let bytes = unsafe { core::slice::from_raw_parts(pointer, size) };
    let mut invalid = *signature;
    invalid[0] ^= 1;
    !verify_development_signature(bytes, &invalid)
}

fn verify_development_signature(message: &[u8], signature: &[u8; 64]) -> bool {
    let Ok(key) = VerifyingKey::from_bytes(&DEVELOPMENT_PUBLIC_KEY) else {
        return false;
    };
    key.verify_strict(message, &Signature::from_bytes(signature))
        .is_ok()
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
