#![no_std]

use ginux_volume_format::{
    decode, encode, HostVolumeHeader, PAYLOAD_CAPACITY, SECTOR_SIZE, SLOT_COUNT,
};
use sha2::{Digest as _, Sha256};

pub use ginux_volume_format::Record;

pub trait BlockDevice {
    fn read_sector(&mut self, sector: usize, output: &mut [u8; SECTOR_SIZE]) -> bool;
    fn write_sector(&mut self, sector: usize, input: &[u8; SECTOR_SIZE]) -> bool;
    fn flush(&mut self) -> bool;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommitError {
    Corrupt,
    Device,
    GenerationExhausted,
    PayloadTooLarge,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommitStage {
    BeforeWrite,
    PreparedWrite,
    PreparedFlush,
    CommitWrite,
    FinalFlush,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScanError {
    Ambiguous,
    Corrupt,
    Device,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostVolumeDisposition {
    Initialized,
    Migrated,
    Recovered,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HostVolumeState {
    pub header: HostVolumeHeader,
    pub generation: u64,
    pub disposition: HostVolumeDisposition,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostVolumeError {
    Corrupt,
    Device,
    GenerationExhausted,
    Incompatible,
    TooSmall,
}

pub const LEGACY_HOST_VOLUME_PAYLOAD: &[u8] = b"ginux-reboot-persistent\n";
pub const LOCAL_CONFIG_SLOT_SECTORS: usize = 32;
pub const LOCAL_CONFIG_SLOT_STARTS: [usize; 2] = [8, 40];
pub const LOCAL_CONFIG_HEADER_SIZE: usize = 80;
pub const LOCAL_CONFIG_PAYLOAD_CAPACITY: usize =
    LOCAL_CONFIG_SLOT_SECTORS * SECTOR_SIZE - LOCAL_CONFIG_HEADER_SIZE;
pub const OCI_CATALOG_SLOT_SECTORS: usize = 64;
pub const OCI_CATALOG_SLOT_STARTS: [usize; 2] = [72, 136];
pub const OCI_CATALOG_PAYLOAD_CAPACITY: usize =
    OCI_CATALOG_SLOT_SECTORS * SECTOR_SIZE - LOCAL_CONFIG_HEADER_SIZE;
pub const OCI_OBJECT_START_SECTOR: usize = 256;

const LOCAL_CONFIG_MAGIC: &[u8; 8] = b"GINUXLC1";
const LOCAL_CONFIG_VERSION: u32 = 1;
const LOCAL_CONFIG_COMMIT_MARKER: u64 = 0x3147_4643_4c58_5547;
const OCI_CATALOG_MAGIC: &[u8; 8] = b"GINUXOC1";
const OCI_CATALOG_COMMIT_MARKER: u64 = 0x3147_434f_5855_4e47;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LocalConfigRecord {
    pub generation: u64,
    pub length: usize,
}

pub type OciCatalogRecord = LocalConfigRecord;

#[derive(Clone, Copy)]
struct ExtentLayout {
    magic: &'static [u8; 8],
    marker: u64,
    slot_sectors: usize,
    slot_starts: [usize; 2],
    payload_capacity: usize,
}

const LOCAL_CONFIG_LAYOUT: ExtentLayout = ExtentLayout {
    magic: LOCAL_CONFIG_MAGIC,
    marker: LOCAL_CONFIG_COMMIT_MARKER,
    slot_sectors: LOCAL_CONFIG_SLOT_SECTORS,
    slot_starts: LOCAL_CONFIG_SLOT_STARTS,
    payload_capacity: LOCAL_CONFIG_PAYLOAD_CAPACITY,
};

const OCI_CATALOG_LAYOUT: ExtentLayout = ExtentLayout {
    magic: OCI_CATALOG_MAGIC,
    marker: OCI_CATALOG_COMMIT_MARKER,
    slot_sectors: OCI_CATALOG_SLOT_SECTORS,
    slot_starts: OCI_CATALOG_SLOT_STARTS,
    payload_capacity: OCI_CATALOG_PAYLOAD_CAPACITY,
};

pub fn open_host_volume<D: BlockDevice>(
    device: &mut D,
    volume_sectors: u64,
) -> Result<HostVolumeState, HostVolumeError> {
    let header = HostVolumeHeader::new(volume_sectors).ok_or(HostVolumeError::TooSmall)?;
    let current = recover_with_slot(device).map_err(host_scan_error)?;
    let (record, disposition) = match current {
        None => (
            commit(device, &header.encode()).map_err(host_commit_error)?,
            HostVolumeDisposition::Initialized,
        ),
        Some((_, record)) if record.payload() == LEGACY_HOST_VOLUME_PAYLOAD => (
            commit(device, &header.encode()).map_err(host_commit_error)?,
            HostVolumeDisposition::Migrated,
        ),
        Some((_, record)) => {
            let Some(recovered) = HostVolumeHeader::decode(record.payload()) else {
                return Err(HostVolumeError::Incompatible);
            };
            if recovered != header {
                return Err(HostVolumeError::Incompatible);
            }
            (record, HostVolumeDisposition::Recovered)
        }
    };
    Ok(HostVolumeState {
        header,
        generation: record.generation,
        disposition,
    })
}

pub fn recover_local_config<D: BlockDevice>(
    device: &mut D,
    output: &mut [u8],
) -> Result<Option<LocalConfigRecord>, ScanError> {
    recover_extent(device, output, LOCAL_CONFIG_LAYOUT)
}

pub fn recover_oci_catalog<D: BlockDevice>(
    device: &mut D,
    output: &mut [u8],
) -> Result<Option<OciCatalogRecord>, ScanError> {
    recover_extent(device, output, OCI_CATALOG_LAYOUT)
}

fn recover_extent<D: BlockDevice>(
    device: &mut D,
    output: &mut [u8],
    layout: ExtentLayout,
) -> Result<Option<LocalConfigRecord>, ScanError> {
    let mut selected: Option<(usize, LocalConfigRecord)> = None;
    let mut corrupt = false;
    for (slot, start) in layout.slot_starts.iter().copied().enumerate() {
        match inspect_extent_slot(device, start, output, layout)? {
            LocalConfigSlot::Empty | LocalConfigSlot::Prepared => {}
            LocalConfigSlot::Corrupt => corrupt = true,
            LocalConfigSlot::Committed(record) => match selected {
                None => selected = Some((slot, record)),
                Some((_, current)) if record.generation > current.generation => {
                    selected = Some((slot, record));
                }
                Some((_, current)) if record.generation == current.generation => {
                    return Err(ScanError::Ambiguous);
                }
                Some(_) => {}
            },
        }
    }
    if let Some((slot, record)) = selected {
        read_extent_payload(
            device,
            layout.slot_starts[slot],
            record.length,
            output,
            layout,
        )?;
        Ok(Some(record))
    } else if corrupt {
        Err(ScanError::Corrupt)
    } else {
        Ok(None)
    }
}

pub fn commit_local_config<D: BlockDevice>(
    device: &mut D,
    payload: &[u8],
) -> Result<LocalConfigRecord, CommitError> {
    let mut recovery = [0_u8; LOCAL_CONFIG_PAYLOAD_CAPACITY];
    commit_extent(device, payload, &mut recovery, LOCAL_CONFIG_LAYOUT)
}

pub fn commit_oci_catalog<D: BlockDevice>(
    device: &mut D,
    payload: &[u8],
) -> Result<OciCatalogRecord, CommitError> {
    let mut recovery = [0_u8; OCI_CATALOG_PAYLOAD_CAPACITY];
    commit_extent(device, payload, &mut recovery, OCI_CATALOG_LAYOUT)
}

fn commit_extent<D: BlockDevice>(
    device: &mut D,
    payload: &[u8],
    recovery: &mut [u8],
    layout: ExtentLayout,
) -> Result<LocalConfigRecord, CommitError> {
    if payload.is_empty() || payload.len() > layout.payload_capacity {
        return Err(CommitError::PayloadTooLarge);
    }
    let current = recover_extent(device, recovery, layout).map_err(scan_error)?;
    if let Some(record) = current {
        if &recovery[..record.length] == payload {
            return Ok(record);
        }
    }
    let generation = current
        .map_or(Some(1), |record| record.generation.checked_add(1))
        .ok_or(CommitError::GenerationExhausted)?;
    let target = current.map_or(0, |record| (record.generation as usize) & 1);
    let start = layout.slot_starts[target];
    let prepared = extent_header(generation, payload, false, layout);
    let committed = extent_header(generation, payload, true, layout);
    write_extent_slot(device, start, payload, &prepared, layout)?;
    if !device.flush() || !device.write_sector(start, &committed) || !device.flush() {
        return Err(CommitError::Device);
    }
    Ok(LocalConfigRecord {
        generation,
        length: payload.len(),
    })
}

enum LocalConfigSlot {
    Empty,
    Prepared,
    Committed(LocalConfigRecord),
    Corrupt,
}

fn inspect_extent_slot<D: BlockDevice>(
    device: &mut D,
    start: usize,
    scratch: &mut [u8],
    layout: ExtentLayout,
) -> Result<LocalConfigSlot, ScanError> {
    let mut header = [0_u8; SECTOR_SIZE];
    if !device.read_sector(start, &mut header) {
        return Err(ScanError::Device);
    }
    if header.iter().all(|byte| *byte == 0) {
        return Ok(LocalConfigSlot::Empty);
    }
    if &header[..8] != layout.magic
        || u32::from_le_bytes(header[8..12].try_into().unwrap_or([0; 4])) != LOCAL_CONFIG_VERSION
        || usize::from(u16::from_le_bytes(
            header[12..14].try_into().unwrap_or([0; 2]),
        )) != LOCAL_CONFIG_HEADER_SIZE
        || header[72..LOCAL_CONFIG_HEADER_SIZE]
            .iter()
            .any(|byte| *byte != 0)
    {
        return Ok(LocalConfigSlot::Corrupt);
    }
    let length = u32::from_le_bytes(header[16..20].try_into().unwrap_or([0; 4])) as usize;
    let generation = u64::from_le_bytes(header[24..32].try_into().unwrap_or([0; 8]));
    let marker = u64::from_le_bytes(header[32..40].try_into().unwrap_or([0; 8]));
    if length == 0 || length > layout.payload_capacity || length > scratch.len() || generation == 0
    {
        return Ok(LocalConfigSlot::Corrupt);
    }
    if marker == 0 {
        return Ok(LocalConfigSlot::Prepared);
    }
    if marker != layout.marker {
        return Ok(LocalConfigSlot::Corrupt);
    }
    read_extent_payload(device, start, length, scratch, layout)?;
    let expected: [u8; 32] = Sha256::digest(&scratch[..length]).into();
    if header[40..72] != expected {
        return Ok(LocalConfigSlot::Corrupt);
    }
    Ok(LocalConfigSlot::Committed(LocalConfigRecord {
        generation,
        length,
    }))
}

fn extent_header(
    generation: u64,
    payload: &[u8],
    committed: bool,
    layout: ExtentLayout,
) -> [u8; SECTOR_SIZE] {
    let mut header = [0_u8; SECTOR_SIZE];
    header[..8].copy_from_slice(layout.magic);
    header[8..12].copy_from_slice(&LOCAL_CONFIG_VERSION.to_le_bytes());
    header[12..14].copy_from_slice(&(LOCAL_CONFIG_HEADER_SIZE as u16).to_le_bytes());
    header[16..20].copy_from_slice(&(payload.len() as u32).to_le_bytes());
    header[24..32].copy_from_slice(&generation.to_le_bytes());
    if committed {
        header[32..40].copy_from_slice(&layout.marker.to_le_bytes());
    }
    let digest: [u8; 32] = Sha256::digest(payload).into();
    header[40..72].copy_from_slice(&digest);
    let first = payload.len().min(SECTOR_SIZE - LOCAL_CONFIG_HEADER_SIZE);
    header[LOCAL_CONFIG_HEADER_SIZE..LOCAL_CONFIG_HEADER_SIZE + first]
        .copy_from_slice(&payload[..first]);
    header
}

fn write_extent_slot<D: BlockDevice>(
    device: &mut D,
    start: usize,
    payload: &[u8],
    header: &[u8; SECTOR_SIZE],
    layout: ExtentLayout,
) -> Result<(), CommitError> {
    if !device.write_sector(start, header) {
        return Err(CommitError::Device);
    }
    let first = payload.len().min(SECTOR_SIZE - LOCAL_CONFIG_HEADER_SIZE);
    let mut offset = first;
    for sector in 1..layout.slot_sectors {
        let mut bytes = [0_u8; SECTOR_SIZE];
        let length = (payload.len() - offset).min(SECTOR_SIZE);
        bytes[..length].copy_from_slice(&payload[offset..offset + length]);
        if !device.write_sector(start + sector, &bytes) {
            return Err(CommitError::Device);
        }
        offset += length;
    }
    Ok(())
}

fn read_extent_payload<D: BlockDevice>(
    device: &mut D,
    start: usize,
    length: usize,
    output: &mut [u8],
    layout: ExtentLayout,
) -> Result<(), ScanError> {
    if length > output.len() {
        return Err(ScanError::Corrupt);
    }
    let mut sector = [0_u8; SECTOR_SIZE];
    if !device.read_sector(start, &mut sector) {
        return Err(ScanError::Device);
    }
    let first = length.min(SECTOR_SIZE - LOCAL_CONFIG_HEADER_SIZE);
    output[..first]
        .copy_from_slice(&sector[LOCAL_CONFIG_HEADER_SIZE..LOCAL_CONFIG_HEADER_SIZE + first]);
    let mut offset = first;
    for index in 1..layout.slot_sectors {
        if offset == length {
            break;
        }
        if !device.read_sector(start + index, &mut sector) {
            return Err(ScanError::Device);
        }
        let copied = (length - offset).min(SECTOR_SIZE);
        output[offset..offset + copied].copy_from_slice(&sector[..copied]);
        offset += copied;
    }
    Ok(())
}

pub fn commit<D: BlockDevice>(device: &mut D, payload: &[u8]) -> Result<Record, CommitError> {
    commit_through(device, payload, CommitStage::FinalFlush)?;
    recover_with_slot(device)
        .map_err(scan_error)?
        .map(|(_, record)| record)
        .ok_or(CommitError::Device)
}

pub fn recover<D: BlockDevice>(device: &mut D) -> Option<Record> {
    recover_with_slot(device)
        .ok()
        .flatten()
        .map(|(_, record)| record)
}

pub fn commit_through<D: BlockDevice>(
    device: &mut D,
    payload: &[u8],
    through: CommitStage,
) -> Result<(), CommitError> {
    if payload.len() > PAYLOAD_CAPACITY {
        return Err(CommitError::PayloadTooLarge);
    }
    let current = recover_with_slot(device).map_err(scan_error)?;
    let generation = current
        .map_or(Some(1), |(_, record)| record.generation.checked_add(1))
        .ok_or(CommitError::GenerationExhausted)?;
    let target = current.map_or(0, |(slot, _)| slot ^ 1);
    if through == CommitStage::BeforeWrite {
        return Ok(());
    }

    let (prepared, committed) = encode(generation, payload).ok_or(CommitError::PayloadTooLarge)?;
    if !device.write_sector(target, &prepared) {
        return Err(CommitError::Device);
    }
    if through == CommitStage::PreparedWrite {
        return Ok(());
    }
    if !device.flush() {
        return Err(CommitError::Device);
    }
    if through == CommitStage::PreparedFlush {
        return Ok(());
    }
    if !device.write_sector(target, &committed) {
        return Err(CommitError::Device);
    }
    if through == CommitStage::CommitWrite {
        return Ok(());
    }
    if !device.flush() {
        return Err(CommitError::Device);
    }
    Ok(())
}

pub fn recover_with_slot<D: BlockDevice>(
    device: &mut D,
) -> Result<Option<(usize, Record)>, ScanError> {
    let mut sector = [0_u8; SECTOR_SIZE];
    let mut selected: Option<(usize, Record)> = None;
    let mut corrupt = false;
    for slot in 0..SLOT_COUNT {
        if !device.read_sector(slot, &mut sector) {
            return Err(ScanError::Device);
        }
        let Some(record) = decode(&sector) else {
            corrupt |= sector.iter().any(|byte| *byte != 0);
            continue;
        };
        match selected {
            None => selected = Some((slot, record)),
            Some((_, current)) if record.generation > current.generation => {
                selected = Some((slot, record));
            }
            Some((_, current)) if record.generation == current.generation => {
                return Err(ScanError::Ambiguous);
            }
            Some(_) => {}
        }
    }
    if selected.is_none() && corrupt {
        Err(ScanError::Corrupt)
    } else {
        Ok(selected)
    }
}

fn scan_error(error: ScanError) -> CommitError {
    match error {
        ScanError::Ambiguous | ScanError::Corrupt => CommitError::Corrupt,
        ScanError::Device => CommitError::Device,
    }
}

fn host_scan_error(error: ScanError) -> HostVolumeError {
    match error {
        ScanError::Ambiguous | ScanError::Corrupt => HostVolumeError::Corrupt,
        ScanError::Device => HostVolumeError::Device,
    }
}

fn host_commit_error(error: CommitError) -> HostVolumeError {
    match error {
        CommitError::Corrupt => HostVolumeError::Corrupt,
        CommitError::Device => HostVolumeError::Device,
        CommitError::GenerationExhausted => HostVolumeError::GenerationExhausted,
        CommitError::PayloadTooLarge => HostVolumeError::Incompatible,
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use std::vec;
    use std::vec::Vec;

    #[derive(Clone, Copy)]
    struct MemoryDevice {
        durable: [[u8; SECTOR_SIZE]; SLOT_COUNT],
        pending: [[u8; SECTOR_SIZE]; SLOT_COUNT],
    }

    impl MemoryDevice {
        const fn new() -> Self {
            Self {
                durable: [[0; SECTOR_SIZE]; SLOT_COUNT],
                pending: [[0; SECTOR_SIZE]; SLOT_COUNT],
            }
        }

        fn crash(&mut self) {
            self.pending = self.durable;
        }
    }

    impl BlockDevice for MemoryDevice {
        fn read_sector(&mut self, sector: usize, output: &mut [u8; SECTOR_SIZE]) -> bool {
            self.pending.get(sector).is_some_and(|input| {
                *output = *input;
                true
            })
        }

        fn write_sector(&mut self, sector: usize, input: &[u8; SECTOR_SIZE]) -> bool {
            self.pending.get_mut(sector).is_some_and(|output| {
                *output = *input;
                true
            })
        }

        fn flush(&mut self) -> bool {
            self.durable = self.pending;
            true
        }
    }

    #[derive(Clone)]
    struct LargeMemoryDevice {
        durable: Vec<[u8; SECTOR_SIZE]>,
        pending: Vec<[u8; SECTOR_SIZE]>,
    }

    impl LargeMemoryDevice {
        fn new() -> Self {
            Self {
                durable: vec![[0; SECTOR_SIZE]; 200],
                pending: vec![[0; SECTOR_SIZE]; 200],
            }
        }
    }

    impl BlockDevice for LargeMemoryDevice {
        fn read_sector(&mut self, sector: usize, output: &mut [u8; SECTOR_SIZE]) -> bool {
            self.pending.get(sector).is_some_and(|input| {
                *output = *input;
                true
            })
        }

        fn write_sector(&mut self, sector: usize, input: &[u8; SECTOR_SIZE]) -> bool {
            self.pending.get_mut(sector).is_some_and(|output| {
                *output = *input;
                true
            })
        }

        fn flush(&mut self) -> bool {
            self.durable.clone_from(&self.pending);
            true
        }
    }

    #[test]
    fn final_flush_is_the_commit_point() {
        let mut baseline = MemoryDevice::new();
        let first = commit(&mut baseline, b"one").expect("first commit");
        assert_eq!(first.generation, 1);
        for stage in [
            CommitStage::BeforeWrite,
            CommitStage::PreparedWrite,
            CommitStage::PreparedFlush,
            CommitStage::CommitWrite,
            CommitStage::FinalFlush,
        ] {
            let mut device = baseline;
            commit_through(&mut device, b"two", stage).expect("staged commit");
            device.crash();
            let record = recover(&mut device).expect("recover old or new");
            assert_eq!(
                record.generation,
                u64::from(stage == CommitStage::FinalFlush) + 1
            );
        }
    }

    #[test]
    fn foreign_nonzero_media_fails_closed() {
        let mut device = MemoryDevice::new();
        device.pending[0][0] = 1;
        assert!(device.flush());
        assert_eq!(recover_with_slot(&mut device), Err(ScanError::Corrupt));
        assert_eq!(commit(&mut device, b"no"), Err(CommitError::Corrupt));
    }

    #[test]
    fn host_volume_initializes_recovers_and_migrates_transactionally() {
        let mut fresh = MemoryDevice::new();
        let initialized = open_host_volume(&mut fresh, 131_072).expect("initialize host volume");
        assert_eq!(initialized.disposition, HostVolumeDisposition::Initialized);
        assert_eq!(initialized.generation, 1);
        let recovered = open_host_volume(&mut fresh, 131_072).expect("recover host volume");
        assert_eq!(recovered.disposition, HostVolumeDisposition::Recovered);
        assert_eq!(recovered.generation, 1);
        assert_eq!(recovered.header, initialized.header);

        let mut legacy = MemoryDevice::new();
        commit(&mut legacy, LEGACY_HOST_VOLUME_PAYLOAD).expect("legacy marker");
        let migrated = open_host_volume(&mut legacy, 131_072).expect("migrate host volume");
        assert_eq!(migrated.disposition, HostVolumeDisposition::Migrated);
        assert_eq!(migrated.generation, 2);
        assert_eq!(
            HostVolumeHeader::decode(migrated.header.encode().as_slice()),
            Some(migrated.header)
        );
    }

    #[test]
    fn host_volume_rejects_geometry_changes_and_foreign_records() {
        let mut device = MemoryDevice::new();
        open_host_volume(&mut device, 131_072).expect("initialize host volume");
        assert_eq!(
            open_host_volume(&mut device, 65_536),
            Err(HostVolumeError::Incompatible)
        );

        let mut foreign = MemoryDevice::new();
        commit(&mut foreign, b"not-a-host-volume").expect("foreign record");
        assert_eq!(
            open_host_volume(&mut foreign, 131_072),
            Err(HostVolumeError::Incompatible)
        );
    }

    #[test]
    fn local_config_commits_large_payloads_and_recovers_newest_generation() {
        let mut device = LargeMemoryDevice::new();
        let first = vec![0x5a; 10_511];
        let record = commit_local_config(&mut device, &first).expect("first config commit");
        assert_eq!(record.generation, 1);
        let mut output = vec![0; LOCAL_CONFIG_PAYLOAD_CAPACITY];
        let recovered = recover_local_config(&mut device, &mut output)
            .expect("scan first config")
            .expect("first config");
        assert_eq!(recovered, record);
        assert_eq!(&output[..recovered.length], first.as_slice());

        let second = vec![0xa5; 12_345];
        let record = commit_local_config(&mut device, &second).expect("second config commit");
        assert_eq!(record.generation, 2);
        output.fill(0);
        let recovered = recover_local_config(&mut device, &mut output)
            .expect("scan second config")
            .expect("second config");
        assert_eq!(recovered, record);
        assert_eq!(&output[..recovered.length], second.as_slice());
    }

    #[test]
    fn corrupt_local_config_is_rejected_without_modification() {
        let mut device = LargeMemoryDevice::new();
        commit_local_config(&mut device, b"persistent-settings").expect("config commit");
        device.pending[LOCAL_CONFIG_SLOT_STARTS[0]][LOCAL_CONFIG_HEADER_SIZE] ^= 1;
        device.durable.clone_from(&device.pending);
        let before = device.pending.clone();
        let mut output = vec![0; LOCAL_CONFIG_PAYLOAD_CAPACITY];
        assert_eq!(
            recover_local_config(&mut device, &mut output),
            Err(ScanError::Corrupt)
        );
        assert_eq!(device.pending, before);
    }

    #[test]
    fn oci_catalog_is_independent_and_transactional() {
        let mut device = LargeMemoryDevice::new();
        open_host_volume(&mut device, 131_072).expect("host volume");
        commit_local_config(&mut device, b"domain configuration").expect("local config");
        let catalog = [0x5a; 28_944];
        let committed = commit_oci_catalog(&mut device, &catalog).expect("catalog commit");
        assert_eq!(committed.generation, 1);
        let mut recovered = [0; OCI_CATALOG_PAYLOAD_CAPACITY];
        let record = recover_oci_catalog(&mut device, &mut recovered)
            .expect("catalog recovery")
            .expect("catalog record");
        assert_eq!(&recovered[..record.length], &catalog);
        let mut local = [0; LOCAL_CONFIG_PAYLOAD_CAPACITY];
        let local_record = recover_local_config(&mut device, &mut local)
            .expect("local recovery")
            .expect("local record");
        assert_eq!(&local[..local_record.length], b"domain configuration");
    }
}
