pub(crate) const VERSION: u16 = 1;
pub(crate) const FEATURES: u16 = 0x07ff;
pub(crate) const DISCOVERY_WORD: u64 = ((VERSION as u64) << 16) | FEATURES as u64;
pub(crate) const MAX_BUFFERED_CONSOLE_BYTES: u32 = 64;
pub(crate) const DESCRIPTOR_SIZE: usize = 16;

pub(crate) fn descriptor() -> [u8; DESCRIPTOR_SIZE] {
    let mut bytes = [0_u8; DESCRIPTOR_SIZE];
    bytes[0..4].copy_from_slice(&(DESCRIPTOR_SIZE as u32).to_le_bytes());
    bytes[4..6].copy_from_slice(&VERSION.to_le_bytes());
    bytes[6..8].copy_from_slice(&FEATURES.to_le_bytes());
    bytes[8..12].copy_from_slice(&MAX_BUFFERED_CONSOLE_BYTES.to_le_bytes());
    bytes
}
