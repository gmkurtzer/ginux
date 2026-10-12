#![no_std]

use sha2::{Digest, Sha256};

pub const SECTOR_SIZE: usize = 512;
pub const SLOT_COUNT: usize = 2;
pub const MAGIC: &[u8; 8] = b"GINUXPV0";
pub const VERSION: u32 = 1;
pub const HEADER_SIZE: usize = 72;
pub const PAYLOAD_CAPACITY: usize = SECTOR_SIZE - HEADER_SIZE;
pub const COMMIT_MARKER: u64 = 0x3144_4554_5449_4d43;

const VERSION_OFFSET: usize = 8;
const HEADER_SIZE_OFFSET: usize = 12;
const PAYLOAD_LENGTH_OFFSET: usize = 14;
const GENERATION_OFFSET: usize = 16;
const COMMIT_OFFSET: usize = 24;
const DIGEST_OFFSET: usize = 32;
const DIGEST_SIZE: usize = 32;
const RESERVED_OFFSET: usize = 64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Record {
    pub generation: u64,
    length: u16,
    payload: [u8; PAYLOAD_CAPACITY],
}

impl Record {
    pub fn payload(&self) -> &[u8] {
        &self.payload[..usize::from(self.length)]
    }
}

pub fn encode(generation: u64, payload: &[u8]) -> Option<([u8; SECTOR_SIZE], [u8; SECTOR_SIZE])> {
    if generation == 0 || payload.len() > PAYLOAD_CAPACITY {
        return None;
    }
    let mut committed = [0_u8; SECTOR_SIZE];
    committed[..MAGIC.len()].copy_from_slice(MAGIC);
    write_u32(&mut committed, VERSION_OFFSET, VERSION);
    write_u16(&mut committed, HEADER_SIZE_OFFSET, HEADER_SIZE as u16);
    write_u16(
        &mut committed,
        PAYLOAD_LENGTH_OFFSET,
        u16::try_from(payload.len()).ok()?,
    );
    write_u64(&mut committed, GENERATION_OFFSET, generation);
    write_u64(&mut committed, COMMIT_OFFSET, COMMIT_MARKER);
    committed[HEADER_SIZE..HEADER_SIZE + payload.len()].copy_from_slice(payload);
    let digest = digest_sector(&committed);
    committed[DIGEST_OFFSET..DIGEST_OFFSET + DIGEST_SIZE].copy_from_slice(&digest);

    let mut prepared = committed;
    write_u64(&mut prepared, COMMIT_OFFSET, 0);
    Some((prepared, committed))
}

pub fn decode(sector: &[u8; SECTOR_SIZE]) -> Option<Record> {
    if &sector[..MAGIC.len()] != MAGIC
        || read_u32(sector, VERSION_OFFSET) != VERSION
        || usize::from(read_u16(sector, HEADER_SIZE_OFFSET)) != HEADER_SIZE
        || read_u64(sector, COMMIT_OFFSET) != COMMIT_MARKER
        || sector[RESERVED_OFFSET..HEADER_SIZE]
            .iter()
            .any(|byte| *byte != 0)
    {
        return None;
    }
    let generation = read_u64(sector, GENERATION_OFFSET);
    let length = read_u16(sector, PAYLOAD_LENGTH_OFFSET);
    let length_usize = usize::from(length);
    if generation == 0
        || length_usize > PAYLOAD_CAPACITY
        || sector[HEADER_SIZE + length_usize..]
            .iter()
            .any(|byte| *byte != 0)
        || sector[DIGEST_OFFSET..DIGEST_OFFSET + DIGEST_SIZE] != digest_sector(sector)
    {
        return None;
    }
    let mut payload = [0_u8; PAYLOAD_CAPACITY];
    payload[..length_usize].copy_from_slice(&sector[HEADER_SIZE..HEADER_SIZE + length_usize]);
    Some(Record {
        generation,
        length,
        payload,
    })
}

fn digest_sector(sector: &[u8; SECTOR_SIZE]) -> [u8; DIGEST_SIZE] {
    let mut canonical = *sector;
    canonical[DIGEST_OFFSET..DIGEST_OFFSET + DIGEST_SIZE].fill(0);
    Sha256::digest(canonical).into()
}

fn read_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap_or([0; 2]))
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap_or([0; 4]))
}

fn read_u64(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap_or([0; 8]))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prepared_is_not_committed_and_committed_round_trips() {
        let (prepared, committed) = encode(7, b"shared-codec").expect("valid record");
        assert_eq!(decode(&prepared), None);
        let record = decode(&committed).expect("committed record");
        assert_eq!(record.generation, 7);
        assert_eq!(record.payload(), b"shared-codec");
    }

    #[test]
    fn rejects_invalid_inputs_and_corruption() {
        assert!(encode(0, b"invalid").is_none());
        assert!(encode(1, &[0; PAYLOAD_CAPACITY + 1]).is_none());
        let (_, mut committed) = encode(1, b"digest").expect("valid record");
        committed[HEADER_SIZE] ^= 1;
        assert_eq!(decode(&committed), None);
    }
}
