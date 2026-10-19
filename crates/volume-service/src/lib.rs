#![no_std]

use ginux_volume_format::{
    decode, encode, HostVolumeHeader, PAYLOAD_CAPACITY, SECTOR_SIZE, SLOT_COUNT,
};

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
    use super::*;

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
}
