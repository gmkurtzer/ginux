//! volume_surfaces (v2.7-3): third fuzz instance — `../crates/volume-format`,
//! rank-2 untrusted surface in the v2.6-1d inventory (volume sector content;
//! attacker = disk). Pure host-testable fns, kernel-faithful chain:
//! `Record::decode(&[u8; 512])` then `HostVolumeHeader::decode(payload)` —
//! the exact call pattern in kernel/src/lib.rs:1317/1332@c86945b1.
//!
//! Anything that panics (indexing, arithmetic, unwrap on attacker-controlled
//! sector bytes) is a finding; returning None is fine.

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    volume_surfaces(data)
});

fn volume_surfaces(data: &[u8]) {
    use ginux_volume_format as vf;

    // Harness modeling: libFuzzer inputs shorter than one sector are
    // zero-padded so short-input branches stay reachable; the kernel only
    // ever hands full sectors to decode().
    let mut sector = [0u8; vf::SECTOR_SIZE];
    let n = data.len().min(vf::SECTOR_SIZE);
    sector[..n].copy_from_slice(&data[..n]);

    // 1) commit-protocol record validator.
    if let Some(record) = vf::decode(&sector) {
        let _ = (record.generation, record.payload());
        // 2) kernel chain: host header decode over the record payload
        //    (kernel/src/lib.rs:1317/1332@c86945b1).
        let _ = vf::HostVolumeHeader::decode(record.payload());
    }

    // Direct header probe: in the kernel the payload length is attacker
    // chosen (Record length <= PAYLOAD_CAPACITY, arbitrary bytes), so a
    // 48-byte payload is reachable without a valid record wrapper.
    if data.len() >= vf::HOST_VOLUME_HEADER_SIZE {
        let _ = vf::HostVolumeHeader::decode(&data[..vf::HOST_VOLUME_HEADER_SIZE]);
    }

    // 3) builder surface (encode is the producer side of both slots).
    let mut gen = [0u8; 8];
    let glen = data.len().min(8);
    gen[..glen].copy_from_slice(&data[..glen]);
    let generation = u64::from_le_bytes(gen);
    let end = data.len().min(8 + vf::PAYLOAD_CAPACITY);
    let payload: &[u8] = if data.len() > 8 { &data[8..end] } else { &[] };
    if let Some((prepared, committed)) = vf::encode(generation, payload) {
        // Round-trip both commit-protocol slots back through the chain.
        let _ = vf::decode(&prepared);
        if let Some(record) = vf::decode(&committed) {
            let _ = vf::HostVolumeHeader::decode(record.payload());
        }
    }
}
