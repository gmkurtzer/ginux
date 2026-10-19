use core::{
    mem::MaybeUninit,
    sync::atomic::{AtomicBool, Ordering},
};

use ginux_volume_format::{encode, PAYLOAD_CAPACITY, SECTOR_SIZE, SLOT_COUNT};
use ginux_volume_service::{commit_through, recover_with_slot, CommitStage, ScanError};

pub(crate) use ginux_volume_service::{
    commit, open_host_volume, recover, BlockDevice, CommitError, HostVolumeDisposition,
};

const VERSION_OFFSET: usize = 8;
const HEADER_SIZE_OFFSET: usize = 12;
const PAYLOAD_LENGTH_OFFSET: usize = 14;
const GENERATION_OFFSET: usize = 16;
const COMMIT_OFFSET: usize = 24;
const DIGEST_OFFSET: usize = 32;
const RESERVED_OFFSET: usize = 64;
const HEADER_SIZE: usize = 72;

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
    let Some((prepared, _)) = encode(1, b"interrupted-first-commit") else {
        return false;
    };
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

    let Some((prepared, committed)) = encode(2, b"torn-marker") else {
        return false;
    };
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
    let Some((_, committed)) = encode(u64::MAX, b"last-generation") else {
        return false;
    };
    exhausted.pending[0] = committed;
    if !exhausted.flush()
        || commit(&mut exhausted, b"overflow") != Err(CommitError::GenerationExhausted)
    {
        return false;
    }

    let mut ambiguous = MemoryBlockDevice::new();
    let (Some((_, left)), Some((_, right))) = (encode(7, b"left"), encode(7, b"right")) else {
        return false;
    };
    ambiguous.pending = [left, right];
    ambiguous.flush()
        && recover(&mut ambiguous).is_none()
        && commit(&mut ambiguous, b"must-not-overwrite") == Err(CommitError::Corrupt)
}
