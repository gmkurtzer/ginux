//! Power-loss tests for every transactional commit in this crate.
//!
//! The device keeps the writes since the last flush in a volatile cache. A
//! crash after any number of device operations persists an arbitrary subset
//! of those cached writes, as a write-back cache may, and optionally tears one
//! of them in half. Recovery must then return exactly the previous or the new
//! record, and the next commit must succeed.

extern crate std;

use super::*;
use std::vec;
use std::vec::Vec;

const DEVICE_SECTORS: usize = 256;

#[derive(Clone)]
struct CrashDevice {
    durable: Vec<[u8; SECTOR_SIZE]>,
    cached: Vec<(usize, [u8; SECTOR_SIZE])>,
    operations: usize,
    power_fails_after: Option<usize>,
}

impl CrashDevice {
    fn new() -> Self {
        Self {
            durable: vec![[0; SECTOR_SIZE]; DEVICE_SECTORS],
            cached: Vec::new(),
            operations: 0,
            power_fails_after: None,
        }
    }

    fn powered(&mut self) -> bool {
        self.operations += 1;
        self.power_fails_after
            .is_none_or(|limit| self.operations <= limit)
    }

    /// The disk after power returns: the durable sectors plus the selected
    /// cached writes in issue order. `torn` replaces the second half of one
    /// selected write with the sector's previous contents.
    fn after_crash(&self, persisted: &[bool], torn: Option<usize>) -> Self {
        let mut durable = self.durable.clone();
        for (index, (sector, bytes)) in self.cached.iter().enumerate() {
            if !persisted[index] {
                continue;
            }
            let mut written = *bytes;
            if torn == Some(index) {
                written[SECTOR_SIZE / 2..].copy_from_slice(&durable[*sector][SECTOR_SIZE / 2..]);
            }
            durable[*sector] = written;
        }
        Self {
            durable,
            cached: Vec::new(),
            operations: 0,
            power_fails_after: None,
        }
    }
}

impl BlockDevice for CrashDevice {
    fn read_sector(&mut self, sector: usize, output: &mut [u8; SECTOR_SIZE]) -> bool {
        let Some(durable) = self.durable.get(sector) else {
            return false;
        };
        *output = self
            .cached
            .iter()
            .rev()
            .find(|(cached, _)| *cached == sector)
            .map_or(*durable, |(_, bytes)| *bytes);
        true
    }

    fn write_sector(&mut self, sector: usize, input: &[u8; SECTOR_SIZE]) -> bool {
        if sector >= self.durable.len() || !self.powered() {
            return false;
        }
        self.cached.push((sector, *input));
        true
    }

    fn flush(&mut self) -> bool {
        if !self.powered() {
            return false;
        }
        for (sector, bytes) in self.cached.drain(..) {
            self.durable[sector] = bytes;
        }
        true
    }
}

/// Subsets of `count` cached writes. Small caches get every subset. Larger
/// ones get none, all, each single write and each all-but-one in the full
/// run, and in the quick run none, all, and the first, last and middle write
/// alone and left out.
fn persisted_subsets(count: usize, full: bool) -> Vec<Vec<bool>> {
    if count <= 10 {
        return (0..1_u32 << count)
            .map(|mask| (0..count).map(|bit| mask & (1 << bit) != 0).collect())
            .collect();
    }
    let chosen: Vec<usize> = if full {
        (0..count).collect()
    } else {
        vec![0, count / 2, count - 1]
    };
    let mut subsets = vec![vec![false; count], vec![true; count]];
    for chosen in chosen {
        subsets.push((0..count).map(|index| index == chosen).collect());
        subsets.push((0..count).map(|index| index != chosen).collect());
    }
    subsets
}

/// One transactional store under test.
trait Store {
    /// The host record deliberately fails closed after an interrupted first
    /// commit (kernel/src/volume.rs `prepared_only` boot contract).
    const FIRST_COMMIT_FAILS_CLOSED: bool = false;
    fn commit(device: &mut CrashDevice, payload: &[u8]) -> bool;
    /// Ok(None) for an empty store, Ok(Some(payload)) for a committed one.
    fn recover(device: &mut CrashDevice) -> Result<Option<Vec<u8>>, ()>;
}

struct HostRecord;
impl Store for HostRecord {
    const FIRST_COMMIT_FAILS_CLOSED: bool = true;
    fn commit(device: &mut CrashDevice, payload: &[u8]) -> bool {
        commit(device, payload).is_ok()
    }
    fn recover(device: &mut CrashDevice) -> Result<Option<Vec<u8>>, ()> {
        recover_with_slot(device)
            .map(|selected| selected.map(|(_, record)| record.payload().to_vec()))
            .map_err(|_| ())
    }
}

struct LocalConfig;
impl Store for LocalConfig {
    fn commit(device: &mut CrashDevice, payload: &[u8]) -> bool {
        commit_local_config(device, payload).is_ok()
    }
    fn recover(device: &mut CrashDevice) -> Result<Option<Vec<u8>>, ()> {
        let mut output = vec![0; LOCAL_CONFIG_PAYLOAD_CAPACITY];
        recover_local_config(device, &mut output)
            .map(|record| record.map(|record| output[..record.length].to_vec()))
            .map_err(|_| ())
    }
}

struct OciCatalog;
impl Store for OciCatalog {
    fn commit(device: &mut CrashDevice, payload: &[u8]) -> bool {
        commit_oci_catalog(device, payload).is_ok()
    }
    fn recover(device: &mut CrashDevice) -> Result<Option<Vec<u8>>, ()> {
        let mut output = vec![0; OCI_CATALOG_PAYLOAD_CAPACITY];
        recover_oci_catalog(device, &mut output)
            .map(|record| record.map(|record| output[..record.length].to_vec()))
            .map_err(|_| ())
    }
}

fn payload(seed: u8, length: usize) -> Vec<u8> {
    (0..length)
        .map(|index| seed.wrapping_mul(31).wrapping_add(index as u8) | 1)
        .collect()
}

/// Commits `history` durably, then crashes the next commit after every number
/// of device operations with every tested subset of cached writes (and every
/// torn variant), and checks recovery and the following commit.
fn check_power_loss<S: Store>(
    history: &[Vec<u8>],
    next: &[u8],
    torn_writes: bool,
    full: bool,
) -> usize {
    let mut base = CrashDevice::new();
    for previous in history {
        assert!(S::commit(&mut base, previous), "history commit");
    }
    let old = history.last().cloned();
    let mut probe = base.clone();
    assert!(S::commit(&mut probe, next), "uncrashed commit");
    let total_operations = probe.operations;
    let mut checked = 0;
    for limit in 0..=total_operations {
        let mut device = base.clone();
        device.power_fails_after = Some(limit);
        let committed = S::commit(&mut device, next);
        for persisted in persisted_subsets(device.cached.len(), full) {
            let torn_choices: Vec<Option<usize>> = if torn_writes {
                let chosen: Vec<usize> = (0..persisted.len())
                    .filter(|index| persisted[*index])
                    .collect();
                let tear: Vec<usize> = if chosen.len() <= 4 {
                    chosen
                } else {
                    vec![
                        chosen[0],
                        chosen[chosen.len() / 2],
                        chosen[chosen.len() - 1],
                    ]
                };
                core::iter::once(None)
                    .chain(tear.into_iter().map(Some))
                    .collect()
            } else {
                vec![None]
            };
            for torn in torn_choices {
                let mut crashed = device.after_crash(&persisted, torn);
                let recovered = S::recover(&mut crashed);
                let context = || {
                    std::format!(
                        "history {} limit {limit}/{total_operations} committed {committed} \
                         persisted {persisted:?} torn {torn:?}: {recovered:?}",
                        history.len()
                    )
                };
                match &recovered {
                    Ok(Some(found)) if found.as_slice() == next => {}
                    Ok(Some(found)) if !committed && Some(found) == old.as_ref() => {}
                    Ok(None) if !committed && old.is_none() => {}
                    Err(()) if S::FIRST_COMMIT_FAILS_CLOSED && !committed && old.is_none() => {
                        checked += 1;
                        continue;
                    }
                    _ => panic!(
                        "recovered neither the old nor the new record: {}",
                        context()
                    ),
                }
                let third = payload(0xa5, next.len().max(1));
                assert!(
                    S::commit(&mut crashed, &third),
                    "next commit failed: {}",
                    context()
                );
                assert_eq!(
                    S::recover(&mut crashed),
                    Ok(Some(third)),
                    "next commit not recovered: {}",
                    context()
                );
                checked += 1;
            }
        }
    }
    checked
}

fn check_store<S: Store>(lengths: &[usize], full: bool) {
    let mut checked = 0;
    for &length in lengths {
        for history_length in 0..if full { 4 } else { 3 } {
            let history: Vec<Vec<u8>> = (0..history_length)
                .map(|index| payload(index as u8 + 1, length))
                .collect();
            checked += check_power_loss::<S>(&history, &payload(0x40, length), false, full);
            checked += check_power_loss::<S>(&history, &payload(0x41, length), true, full);
        }
    }
    assert!(checked > 0);
}

#[test]
fn host_record_survives_power_loss_at_every_operation() {
    check_store::<HostRecord>(&[1, 24, ginux_volume_format::PAYLOAD_CAPACITY], true);
}

#[test]
fn local_config_survives_power_loss_at_every_operation() {
    check_store::<LocalConfig>(
        &[
            1,
            SECTOR_SIZE - LOCAL_CONFIG_HEADER_SIZE,
            SECTOR_SIZE - LOCAL_CONFIG_HEADER_SIZE + 1,
            3000,
        ],
        false,
    );
}

#[test]
fn oci_catalog_survives_power_loss_at_every_operation() {
    check_store::<OciCatalog>(&[1, 5000], false);
}

/// Every single and all-but-one write, full-size payloads and one more prior
/// commit; minutes in debug builds, so run it explicitly:
/// `cargo test -p ginux-volume-service --release -- --ignored power_loss`.
#[test]
#[ignore]
fn full_capacity_stores_survive_power_loss_exhaustively() {
    check_store::<LocalConfig>(&[1, 3000, LOCAL_CONFIG_PAYLOAD_CAPACITY], true);
    check_store::<OciCatalog>(&[1, 5000, OCI_CATALOG_PAYLOAD_CAPACITY], true);
}
