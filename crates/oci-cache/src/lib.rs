#![no_std]

pub const ENTRY_LIMIT: usize = 32;
pub const REFERENCE_CAPACITY: usize = 128;
pub const LAYER_LIMIT: usize = ginux_oci::OCI_LAYER_LIMIT;
pub const OBJECT_START_SECTOR: u64 = 256;

const MAGIC: &[u8; 8] = b"GOCICAT1";
const VERSION: u32 = 1;
const HEADER_SIZE: usize = 16;
const LAYER_SIZE: usize = 48;
const ENTRY_HEADER_SIZE: usize = 136;
const ENTRY_SIZE: usize = ENTRY_HEADER_SIZE + LAYER_LIMIT * LAYER_SIZE;
pub const ENCODED_SIZE: usize = HEADER_SIZE + ENTRY_LIMIT * ENTRY_SIZE;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Platform {
    LinuxAmd64 = 1,
}

impl Platform {
    fn decode(value: u8) -> Option<Self> {
        (value == Self::LinuxAmd64 as u8).then_some(Self::LinuxAmd64)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Layer {
    pub digest: [u8; 32],
    pub start_sector: u64,
    pub length: u64,
}

impl Layer {
    pub const EMPTY: Self = Self {
        digest: [0; 32],
        start_sector: 0,
        length: 0,
    };

    pub fn sector_count(self) -> Option<u64> {
        self.length.checked_add(511).map(|length| length / 512)
    }
}

#[derive(Clone, Copy)]
pub struct Entry {
    live: bool,
    platform: Platform,
    reference_length: u8,
    layer_count: u8,
    reference: [u8; REFERENCE_CAPACITY],
    layers: [Layer; LAYER_LIMIT],
}

impl Entry {
    const EMPTY: Self = Self {
        live: false,
        platform: Platform::LinuxAmd64,
        reference_length: 0,
        layer_count: 0,
        reference: [0; REFERENCE_CAPACITY],
        layers: [Layer::EMPTY; LAYER_LIMIT],
    };

    pub fn reference(&self) -> &[u8] {
        &self.reference[..usize::from(self.reference_length)]
    }

    pub fn platform(&self) -> Platform {
        self.platform
    }

    pub fn layers(&self) -> &[Layer] {
        &self.layers[..usize::from(self.layer_count)]
    }
}

pub struct Catalog {
    entries: [Entry; ENTRY_LIMIT],
}

impl Catalog {
    pub const fn new() -> Self {
        Self {
            entries: [Entry::EMPTY; ENTRY_LIMIT],
        }
    }

    pub fn decode(input: &[u8]) -> Option<Self> {
        if input.len() != ENCODED_SIZE
            || &input[..8] != MAGIC
            || u32::from_le_bytes(input[8..12].try_into().ok()?) != VERSION
            || input[12..HEADER_SIZE].iter().any(|byte| *byte != 0)
        {
            return None;
        }
        let mut catalog = Self::new();
        for index in 0..ENTRY_LIMIT {
            let start = HEADER_SIZE + index * ENTRY_SIZE;
            let input = &input[start..start + ENTRY_SIZE];
            if input[0] == 0 {
                if input.iter().any(|byte| *byte != 0) {
                    return None;
                }
                continue;
            }
            if input[0] != 1 || input[4..8].iter().any(|byte| *byte != 0) {
                return None;
            }
            let platform = Platform::decode(input[1])?;
            let reference_length = usize::from(input[2]);
            let layer_count = usize::from(input[3]);
            if reference_length == 0
                || reference_length > REFERENCE_CAPACITY
                || layer_count == 0
                || layer_count > LAYER_LIMIT
                || input[8 + reference_length..ENTRY_HEADER_SIZE]
                    .iter()
                    .any(|byte| *byte != 0)
            {
                return None;
            }
            let mut entry = Entry::EMPTY;
            entry.live = true;
            entry.platform = platform;
            entry.reference_length = reference_length as u8;
            entry.layer_count = layer_count as u8;
            entry.reference[..reference_length].copy_from_slice(&input[8..8 + reference_length]);
            for layer_index in 0..LAYER_LIMIT {
                let offset = ENTRY_HEADER_SIZE + layer_index * LAYER_SIZE;
                let encoded = &input[offset..offset + LAYER_SIZE];
                if layer_index >= layer_count {
                    if encoded.iter().any(|byte| *byte != 0) {
                        return None;
                    }
                    continue;
                }
                let mut digest = [0; 32];
                digest.copy_from_slice(&encoded[..32]);
                let start_sector = u64::from_le_bytes(encoded[32..40].try_into().ok()?);
                let length = u64::from_le_bytes(encoded[40..48].try_into().ok()?);
                let layer = Layer {
                    digest,
                    start_sector,
                    length,
                };
                if digest == [0; 32]
                    || start_sector < OBJECT_START_SECTOR
                    || length == 0
                    || layer.sector_count()?.checked_add(start_sector).is_none()
                {
                    return None;
                }
                entry.layers[layer_index] = layer;
            }
            if catalog.find(entry.reference(), platform).is_some() {
                return None;
            }
            catalog.entries[index] = entry;
        }
        Some(catalog)
    }

    pub fn encode(&self, output: &mut [u8]) -> bool {
        if output.len() != ENCODED_SIZE {
            return false;
        }
        output.fill(0);
        output[..8].copy_from_slice(MAGIC);
        output[8..12].copy_from_slice(&VERSION.to_le_bytes());
        for (index, entry) in self
            .entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| entry.live)
        {
            let start = HEADER_SIZE + index * ENTRY_SIZE;
            let output = &mut output[start..start + ENTRY_SIZE];
            output[0] = 1;
            output[1] = entry.platform as u8;
            output[2] = entry.reference_length;
            output[3] = entry.layer_count;
            output[8..8 + entry.reference().len()].copy_from_slice(entry.reference());
            for (layer_index, layer) in entry.layers().iter().enumerate() {
                let offset = ENTRY_HEADER_SIZE + layer_index * LAYER_SIZE;
                output[offset..offset + 32].copy_from_slice(&layer.digest);
                output[offset + 32..offset + 40].copy_from_slice(&layer.start_sector.to_le_bytes());
                output[offset + 40..offset + 48].copy_from_slice(&layer.length.to_le_bytes());
            }
        }
        true
    }

    pub fn find(&self, reference: &[u8], platform: Platform) -> Option<&Entry> {
        self.entries.iter().find(|entry| {
            entry.live && entry.platform == platform && entry.reference() == reference
        })
    }

    pub fn find_layer(&self, digest: [u8; 32], length: u64) -> Option<Layer> {
        self.entries
            .iter()
            .filter(|entry| entry.live)
            .flat_map(Entry::layers)
            .copied()
            .find(|layer| layer.digest == digest && layer.length == length)
    }

    pub fn next_object_sector(&self) -> Option<u64> {
        self.entries
            .iter()
            .filter(|entry| entry.live)
            .flat_map(Entry::layers)
            .try_fold(OBJECT_START_SECTOR, |next, layer| {
                Some(next.max(layer.start_sector.checked_add(layer.sector_count()?)?))
            })
    }

    pub fn upsert(&mut self, reference: &[u8], platform: Platform, layers: &[Layer]) -> bool {
        if reference.is_empty()
            || reference.len() > REFERENCE_CAPACITY
            || layers.is_empty()
            || layers.len() > LAYER_LIMIT
        {
            return false;
        }
        let slot = self
            .entries
            .iter()
            .position(|entry| {
                entry.live && entry.platform == platform && entry.reference() == reference
            })
            .or_else(|| self.entries.iter().position(|entry| !entry.live));
        let Some(slot) = slot else { return false };
        let mut entry = Entry::EMPTY;
        entry.live = true;
        entry.platform = platform;
        entry.reference_length = reference.len() as u8;
        entry.layer_count = layers.len() as u8;
        entry.reference[..reference.len()].copy_from_slice(reference);
        entry.layers[..layers.len()].copy_from_slice(layers);
        self.entries[slot] = entry;
        true
    }
}

impl Default for Catalog {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;

    #[test]
    fn round_trip_preserves_multiple_images_and_shared_layers() {
        let first = Layer {
            digest: [1; 32],
            start_sector: 256,
            length: 513,
        };
        let second = Layer {
            digest: [2; 32],
            start_sector: 258,
            length: 1024,
        };
        let mut catalog = Catalog::new();
        assert!(catalog.upsert(
            b"docker.io/library/alpine:latest",
            Platform::LinuxAmd64,
            &[first]
        ));
        assert!(catalog.upsert(
            b"docker.io/rockylinux/rockylinux:10",
            Platform::LinuxAmd64,
            &[first, second]
        ));
        let mut bytes = [0; ENCODED_SIZE];
        assert!(catalog.encode(&mut bytes));
        let recovered = Catalog::decode(&bytes).expect("catalog");
        assert_eq!(
            recovered
                .find(b"docker.io/library/alpine:latest", Platform::LinuxAmd64)
                .unwrap()
                .layers(),
            &[first]
        );
        assert_eq!(recovered.find_layer([1; 32], 513), Some(first));
        assert_eq!(recovered.next_object_sector(), Some(260));
    }

    #[test]
    fn corruption_is_rejected() {
        let mut bytes = [0; ENCODED_SIZE];
        Catalog::new().encode(&mut bytes);
        bytes[12] = 1;
        assert!(Catalog::decode(&bytes).is_none());
    }
}
