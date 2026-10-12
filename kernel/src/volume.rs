use core::{
    mem::MaybeUninit,
    sync::atomic::{AtomicBool, Ordering},
};

const SECTOR_SIZE: usize = 512;
const SLOT_COUNT: usize = 2;
const MAGIC: &[u8; 8] = b"GINUXPV0";
const VERSION: u32 = 1;
const HEADER_SIZE: usize = 72;
pub(crate) const PAYLOAD_CAPACITY: usize = SECTOR_SIZE - HEADER_SIZE;
const COMMIT_MARKER: u64 = 0x3144_4554_5449_4d43;

const VERSION_OFFSET: usize = 8;
const HEADER_SIZE_OFFSET: usize = 12;
const PAYLOAD_LENGTH_OFFSET: usize = 14;
const GENERATION_OFFSET: usize = 16;
const COMMIT_OFFSET: usize = 24;
const DIGEST_OFFSET: usize = 32;
const DIGEST_SIZE: usize = 32;
const RESERVED_OFFSET: usize = 64;

pub(crate) trait BlockDevice {
    fn read_sector(&mut self, sector: usize, output: &mut [u8; SECTOR_SIZE]) -> bool;
    fn write_sector(&mut self, sector: usize, input: &[u8; SECTOR_SIZE]) -> bool;
    fn flush(&mut self) -> bool;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Record {
    pub(crate) generation: u64,
    length: u16,
    payload: [u8; PAYLOAD_CAPACITY],
}

impl Record {
    pub(crate) fn payload(&self) -> &[u8] {
        &self.payload[..usize::from(self.length)]
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CommitError {
    Corrupt,
    Device,
    GenerationExhausted,
    PayloadTooLarge,
}

/// Commits a new record to the inactive slot.
///
/// The prepared sector is flushed before its commit marker is written. The
/// checksum describes the final committed sector, so neither a prepared nor a
/// torn commit-marker write can be mistaken for a committed record.
pub(crate) fn commit<D: BlockDevice>(
    device: &mut D,
    payload: &[u8],
) -> Result<Record, CommitError> {
    commit_through(device, payload, CommitStage::FinalFlush)?;
    recover_with_slot(device)
        .map_err(scan_error)?
        .map(|(_, record)| record)
        .ok_or(CommitError::Device)
}

pub(crate) fn recover<D: BlockDevice>(device: &mut D) -> Option<Record> {
    recover_with_slot(device)
        .ok()
        .flatten()
        .map(|(_, record)| record)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CommitStage {
    BeforeWrite,
    PreparedWrite,
    PreparedFlush,
    CommitWrite,
    FinalFlush,
}

fn commit_through<D: BlockDevice>(
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

    let (prepared, committed) = encode(generation, payload);
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

enum ScanError {
    Ambiguous,
    Corrupt,
    Device,
}

fn scan_error(error: ScanError) -> CommitError {
    match error {
        ScanError::Ambiguous | ScanError::Corrupt => CommitError::Corrupt,
        ScanError::Device => CommitError::Device,
    }
}

fn recover_with_slot<D: BlockDevice>(device: &mut D) -> Result<Option<(usize, Record)>, ScanError> {
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

fn encode(generation: u64, payload: &[u8]) -> ([u8; SECTOR_SIZE], [u8; SECTOR_SIZE]) {
    let mut committed = [0_u8; SECTOR_SIZE];
    committed[..MAGIC.len()].copy_from_slice(MAGIC);
    write_u32(&mut committed, VERSION_OFFSET, VERSION);
    write_u16(&mut committed, HEADER_SIZE_OFFSET, HEADER_SIZE as u16);
    write_u16(&mut committed, PAYLOAD_LENGTH_OFFSET, payload.len() as u16);
    write_u64(&mut committed, GENERATION_OFFSET, generation);
    write_u64(&mut committed, COMMIT_OFFSET, COMMIT_MARKER);
    committed[HEADER_SIZE..HEADER_SIZE + payload.len()].copy_from_slice(payload);
    let digest = digest_sector(&committed);
    committed[DIGEST_OFFSET..DIGEST_OFFSET + DIGEST_SIZE].copy_from_slice(&digest);

    let mut prepared = committed;
    write_u64(&mut prepared, COMMIT_OFFSET, 0);
    (prepared, committed)
}

fn decode(sector: &[u8; SECTOR_SIZE]) -> Option<Record> {
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
    crate::crypto::sha256(&canonical)
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

#[derive(Clone, Copy)]
struct MemoryBlockDevice {
    durable: [[u8; SECTOR_SIZE]; SLOT_COUNT],
    pending: [[u8; SECTOR_SIZE]; SLOT_COUNT],
}

static LIVE_INSTALLED: AtomicBool = AtomicBool::new(false);
static mut LIVE_DEVICE: MaybeUninit<MemoryBlockDevice> = MaybeUninit::uninit();

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum LiveError {
    Corrupt,
    Device,
    GenerationExhausted,
    PayloadTooLarge,
    Stale,
}

pub(crate) fn install_live() -> bool {
    if LIVE_INSTALLED.swap(true, Ordering::AcqRel) {
        return false;
    }
    unsafe {
        core::ptr::addr_of_mut!(LIVE_DEVICE).write(MaybeUninit::new(MemoryBlockDevice::new()));
    }
    live_device_mut().is_some_and(|device| recover(device).is_none())
}

pub(crate) fn live_generation() -> Result<u64, LiveError> {
    let device = live_device_mut().ok_or(LiveError::Device)?;
    recover_with_slot(device)
        .map_err(live_scan_error)
        .map(|record| record.map_or(0, |(_, record)| record.generation))
}

pub(crate) fn read_live(expected_generation: u64, output: &mut [u8]) -> Result<usize, LiveError> {
    let device = live_device_mut().ok_or(LiveError::Device)?;
    let record = recover_with_slot(device)
        .map_err(live_scan_error)?
        .map(|(_, record)| record)
        .ok_or(LiveError::Stale)?;
    if record.generation != expected_generation {
        return Err(LiveError::Stale);
    }
    let payload = record.payload();
    if output.len() < payload.len() {
        return Err(LiveError::PayloadTooLarge);
    }
    output[..payload.len()].copy_from_slice(payload);
    Ok(payload.len())
}

pub(crate) fn commit_live(expected_generation: u64, payload: &[u8]) -> Result<u64, LiveError> {
    let device = live_device_mut().ok_or(LiveError::Device)?;
    let current = recover_with_slot(device).map_err(live_scan_error)?;
    let current_generation = current.map_or(0, |(_, record)| record.generation);
    if current_generation != expected_generation {
        return Err(LiveError::Stale);
    }
    commit(device, payload)
        .map(|record| record.generation)
        .map_err(|error| match error {
            CommitError::Corrupt => LiveError::Corrupt,
            CommitError::Device => LiveError::Device,
            CommitError::GenerationExhausted => LiveError::GenerationExhausted,
            CommitError::PayloadTooLarge => LiveError::PayloadTooLarge,
        })
}

fn live_scan_error(error: ScanError) -> LiveError {
    match error {
        ScanError::Ambiguous | ScanError::Corrupt => LiveError::Corrupt,
        ScanError::Device => LiveError::Device,
    }
}

fn live_device_mut() -> Option<&'static mut MemoryBlockDevice> {
    if !LIVE_INSTALLED.load(Ordering::Acquire) {
        return None;
    }
    Some(unsafe { &mut *core::ptr::addr_of_mut!(LIVE_DEVICE).cast::<MemoryBlockDevice>() })
}

impl MemoryBlockDevice {
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

impl BlockDevice for MemoryBlockDevice {
    fn read_sector(&mut self, sector: usize, output: &mut [u8; SECTOR_SIZE]) -> bool {
        let Some(source) = self.pending.get(sector) else {
            return false;
        };
        *output = *source;
        true
    }

    fn write_sector(&mut self, sector: usize, input: &[u8; SECTOR_SIZE]) -> bool {
        let Some(target) = self.pending.get_mut(sector) else {
            return false;
        };
        *target = *input;
        true
    }

    fn flush(&mut self) -> bool {
        self.durable = self.pending;
        true
    }
}

pub(crate) fn verify_contract() -> bool {
    const OLD: &[u8] = b"volume-generation-one";
    const NEW: &[u8] = b"volume-generation-two";
    let mut baseline = MemoryBlockDevice::new();
    let Ok(first) = commit(&mut baseline, OLD) else {
        return false;
    };
    if first.generation != 1 || first.payload() != OLD {
        return false;
    }

    for stage in [
        CommitStage::BeforeWrite,
        CommitStage::PreparedWrite,
        CommitStage::PreparedFlush,
        CommitStage::CommitWrite,
        CommitStage::FinalFlush,
    ] {
        let mut device = baseline;
        if commit_through(&mut device, NEW, stage).is_err() {
            return false;
        }
        device.crash();
        let Some(recovered) = recover(&mut device) else {
            return false;
        };
        let expected = if stage == CommitStage::FinalFlush {
            (2, NEW)
        } else {
            (1, OLD)
        };
        if recovered.generation != expected.0 || recovered.payload() != expected.1 {
            return false;
        }
    }

    verify_rejections(baseline)
}

fn verify_rejections(baseline: MemoryBlockDevice) -> bool {
    if commit(&mut MemoryBlockDevice::new(), &[0_u8; PAYLOAD_CAPACITY + 1])
        != Err(CommitError::PayloadTooLarge)
    {
        return false;
    }

    let mut foreign = MemoryBlockDevice::new();
    foreign.pending[0][0] = 1;
    if !foreign.flush()
        || recover(&mut foreign).is_some()
        || commit(&mut foreign, b"must-not-initialize") != Err(CommitError::Corrupt)
    {
        return false;
    }

    let mut prepared_only = MemoryBlockDevice::new();
    let (prepared, _) = encode(1, b"interrupted-first-commit");
    prepared_only.pending[0] = prepared;
    if !prepared_only.flush()
        || recover(&mut prepared_only).is_some()
        || commit(&mut prepared_only, b"must-not-replace") != Err(CommitError::Corrupt)
    {
        return false;
    }

    let mut two_generations = baseline;
    if commit(&mut two_generations, b"new-generation").is_err() {
        return false;
    }
    for offset in [
        0,
        VERSION_OFFSET,
        HEADER_SIZE_OFFSET,
        PAYLOAD_LENGTH_OFFSET,
        GENERATION_OFFSET,
        COMMIT_OFFSET,
        DIGEST_OFFSET,
        RESERVED_OFFSET,
        HEADER_SIZE,
    ] {
        let mut device = two_generations;
        device.durable[1][offset] ^= 1;
        device.crash();
        if recover(&mut device).is_none_or(|record| {
            record.generation != 1 || record.payload() != b"volume-generation-one"
        }) {
            return false;
        }
    }

    let (prepared, committed) = encode(2, b"torn-marker");
    for committed_marker_bytes in 0..=8 {
        let mut device = baseline;
        device.durable[1] = prepared;
        device.durable[1][COMMIT_OFFSET..COMMIT_OFFSET + committed_marker_bytes]
            .copy_from_slice(&committed[COMMIT_OFFSET..COMMIT_OFFSET + committed_marker_bytes]);
        device.crash();
        let Some(record) = recover(&mut device) else {
            return false;
        };
        let expected_generation = if committed_marker_bytes == 8 { 2 } else { 1 };
        if record.generation != expected_generation {
            return false;
        }
    }

    let mut exhausted = MemoryBlockDevice::new();
    let (_, committed) = encode(u64::MAX, b"last-generation");
    exhausted.pending[0] = committed;
    if !exhausted.flush()
        || commit(&mut exhausted, b"overflow") != Err(CommitError::GenerationExhausted)
    {
        return false;
    }

    let mut ambiguous = MemoryBlockDevice::new();
    let (_, left) = encode(7, b"left");
    let (_, right) = encode(7, b"right");
    ambiguous.pending = [left, right];
    ambiguous.flush()
        && recover(&mut ambiguous).is_none()
        && commit(&mut ambiguous, b"must-not-overwrite") == Err(CommitError::Corrupt)
}
