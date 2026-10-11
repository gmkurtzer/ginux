pub(crate) const VERSION: u16 = 1;
pub(crate) const FEATURES: u16 = 0x0fff;
pub(crate) const DISCOVERY_WORD: u64 = ((VERSION as u64) << 16) | FEATURES as u64;
pub(crate) const MAX_BUFFERED_CONSOLE_BYTES: u32 = 64;
pub(crate) const DESCRIPTOR_SIZE: usize = 16;
pub(crate) const FAULT_DESCRIPTOR_SIZE: usize = 48;

pub(crate) fn descriptor() -> [u8; DESCRIPTOR_SIZE] {
    let mut bytes = [0_u8; DESCRIPTOR_SIZE];
    bytes[0..4].copy_from_slice(&(DESCRIPTOR_SIZE as u32).to_le_bytes());
    bytes[4..6].copy_from_slice(&VERSION.to_le_bytes());
    bytes[6..8].copy_from_slice(&FEATURES.to_le_bytes());
    bytes[8..12].copy_from_slice(&MAX_BUFFERED_CONSOLE_BYTES.to_le_bytes());
    bytes
}

pub(crate) fn fault_descriptor(
    vector: u64,
    domain: u64,
    error: u64,
    address: u64,
    instruction: u64,
) -> Option<[u8; FAULT_DESCRIPTOR_SIZE]> {
    let mut bytes = [0_u8; FAULT_DESCRIPTOR_SIZE];
    bytes[0..4].copy_from_slice(&(FAULT_DESCRIPTOR_SIZE as u32).to_le_bytes());
    bytes[4..6].copy_from_slice(&VERSION.to_le_bytes());
    bytes[6..8].copy_from_slice(&u16::try_from(vector).ok()?.to_le_bytes());
    bytes[8..16].copy_from_slice(&domain.to_le_bytes());
    bytes[16..24].copy_from_slice(&error.to_le_bytes());
    bytes[24..32].copy_from_slice(&address.to_le_bytes());
    bytes[32..40].copy_from_slice(&instruction.to_le_bytes());
    Some(bytes)
}
