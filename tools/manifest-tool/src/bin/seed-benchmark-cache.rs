use std::{
    env,
    fs::{File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    process,
};

use ginux_oci_cache::{Catalog, Layer, Platform, ENCODED_SIZE, OBJECT_START_SECTOR};
use ginux_volume_format::SECTOR_SIZE;
use ginux_volume_service::{
    commit_oci_catalog, open_host_volume, BlockDevice, OCI_OBJECT_START_SECTOR,
};
use sha2::{Digest as _, Sha256};

const VOLUME_BYTES: u64 = 64 * 1024 * 1024 * 1024;

struct FileDevice(File);

impl BlockDevice for FileDevice {
    fn read_sector(&mut self, sector: usize, output: &mut [u8; SECTOR_SIZE]) -> bool {
        self.0
            .seek(SeekFrom::Start((sector * SECTOR_SIZE) as u64))
            .and_then(|_| self.0.read_exact(output))
            .is_ok()
    }

    fn write_sector(&mut self, sector: usize, input: &[u8; SECTOR_SIZE]) -> bool {
        self.0
            .seek(SeekFrom::Start((sector * SECTOR_SIZE) as u64))
            .and_then(|_| self.0.write_all(input))
            .is_ok()
    }

    fn flush(&mut self) -> bool {
        self.0.sync_data().is_ok()
    }
}

fn fail(message: &str) -> ! {
    eprintln!("seed-benchmark-cache: {message}");
    process::exit(1);
}

fn tar_octal(field: &[u8]) -> Option<usize> {
    let text = field.split(|byte| *byte == 0).next()?;
    let text = std::str::from_utf8(text).ok()?.trim();
    usize::from_str_radix(text, 8).ok()
}

fn tar_field(field: &[u8]) -> Option<&[u8]> {
    let end = field
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(field.len());
    field[end..]
        .iter()
        .all(|byte| *byte == 0)
        .then_some(&field[..end])
}

fn valid_tar_path(path: &[u8]) -> bool {
    !path.is_empty()
        && path[0] != b'/'
        && path.iter().all(|byte| byte.is_ascii_graphic())
        && path
            .split(|byte| *byte == b'/')
            .all(|component| !component.is_empty() && component != b"." && component != b"..")
}

fn diagnose_rootfs(bytes: &[u8]) -> String {
    let mut offset = 0_usize;
    let mut entries = 0_usize;
    while offset + 512 <= bytes.len() {
        let header = &bytes[offset..offset + 512];
        if header.iter().all(|byte| *byte == 0) {
            let mut boundaries = Vec::new();
            let mut scan = 0_usize;
            while scan + 512 <= bytes.len() {
                let candidate = &bytes[scan..scan + 512];
                if candidate.iter().all(|byte| *byte == 0) {
                    break;
                }
                let size = tar_octal(&candidate[124..136]).unwrap_or(0);
                scan += 512 + size.div_ceil(512) * 512;
                if candidate[156] != b'x' {
                    boundaries.push(scan);
                }
            }
            let mut low = 0_usize;
            let mut high = boundaries.len();
            while low < high {
                let middle = low + (high - low) / 2;
                let mut prefix = Vec::with_capacity(boundaries[middle] + 1024);
                prefix.extend_from_slice(&bytes[..boundaries[middle]]);
                prefix.resize(prefix.len() + 1024, 0);
                if ginux_oci::Rootfs::parse(&prefix).is_some() {
                    low = middle + 1;
                } else {
                    high = middle;
                }
            }
            let failing_end = boundaries.get(low).copied().unwrap_or(offset);
            let failing_start = boundaries.get(low.wrapping_sub(1)).copied().unwrap_or(0);
            let failing = &bytes[failing_start..failing_start + 512];
            let failing_name = tar_field(&failing[..100]).unwrap_or(b"<unterminated>");
            return format!(
                "parser rejected entry near logical index {low}, offsets {failing_start}..{failing_end}, path {}",
                String::from_utf8_lossy(failing_name)
            );
        }
        let name = tar_field(&header[..100]).unwrap_or(b"<unterminated>");
        let reason = (|| {
            if &header[257..263] != b"ustar\0" {
                return Some("non-ustar header");
            }
            let stored = tar_octal(&header[148..156])?;
            let actual: usize = header
                .iter()
                .enumerate()
                .map(|(index, byte)| {
                    if (148..156).contains(&index) {
                        b' ' as usize
                    } else {
                        *byte as usize
                    }
                })
                .sum();
            if stored != actual {
                return Some("checksum mismatch");
            }
            let size = tar_octal(&header[124..136])?;
            tar_octal(&header[100..108])?;
            if !matches!(header[156], 0 | b'0' | b'1' | b'2' | b'5' | b'x') {
                return Some("unsupported entry type");
            }
            if matches!(header[156], b'1' | b'2' | b'5') && size != 0 {
                return Some("non-file entry has data");
            }
            let prefix = tar_field(&header[345..500])?;
            if !valid_tar_path(name) || !prefix.is_empty() && !valid_tar_path(prefix) {
                return Some("invalid entry path");
            }
            let link = tar_field(&header[157..257])?;
            if matches!(header[156], b'1' | b'2') && link.is_empty() {
                return Some("empty link target");
            }
            if header[156] == b'1' && !valid_tar_path(link.strip_prefix(b"/").unwrap_or(link)) {
                return Some("invalid hardlink target");
            }
            None
        })();
        if let Some(reason) = reason {
            return format!(
                "{reason} at entry {entries}, offset {offset}, path {}",
                String::from_utf8_lossy(name)
            );
        }
        let size = tar_octal(&header[124..136]).unwrap_or(0);
        offset += 512 + size.div_ceil(512) * 512;
        entries += 1;
    }
    format!("archive ended without terminator after {entries} entries")
}

fn normalize_exported_tar(bytes: &mut [u8]) {
    let mut offset = 0_usize;
    while offset + 512 <= bytes.len() {
        let header = &mut bytes[offset..offset + 512];
        if header.iter().all(|byte| *byte == 0) {
            break;
        }
        let size = tar_octal(&header[124..136]).unwrap_or_else(|| fail("invalid tar size"));
        if header[156] == b'5' {
            let end = header[..100]
                .iter()
                .position(|byte| *byte == 0)
                .unwrap_or(100);
            if end != 0 && header[end - 1] == b'/' {
                header[end - 1] = 0;
                header[148..156].fill(b' ');
                let checksum: u64 = header.iter().map(|byte| u64::from(*byte)).sum();
                header[148..156].copy_from_slice(format!("{checksum:06o}\0 ").as_bytes());
            }
        }
        offset = offset
            .checked_add(512 + size.div_ceil(512) * 512)
            .unwrap_or_else(|| fail("tar offset overflow"));
    }
}

fn main() {
    let arguments: Vec<String> = env::args().collect();
    if arguments.len() != 4 {
        fail("usage: seed-benchmark-cache VOLUME ROOTFS-TAR IMAGE-REFERENCE");
    }
    if OBJECT_START_SECTOR as usize != OCI_OBJECT_START_SECTOR {
        fail("OCI cache layout constants disagree");
    }
    let mut rootfs = std::fs::read(&arguments[2])
        .unwrap_or_else(|error| fail(&format!("cannot read rootfs: {error}")));
    if rootfs.is_empty() {
        fail("rootfs is empty");
    }
    normalize_exported_tar(&mut rootfs);
    ginux_oci::Rootfs::parse(&rootfs).unwrap_or_else(|| fail(&diagnose_rootfs(&rootfs)));
    let digest: [u8; 32] = Sha256::digest(&rootfs).into();
    let file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .read(true)
        .write(true)
        .open(&arguments[1])
        .unwrap_or_else(|error| fail(&format!("cannot create volume: {error}")));
    file.set_len(VOLUME_BYTES)
        .unwrap_or_else(|error| fail(&format!("cannot size volume: {error}")));
    let mut device = FileDevice(file);
    open_host_volume(&mut device, VOLUME_BYTES / SECTOR_SIZE as u64)
        .unwrap_or_else(|error| fail(&format!("cannot initialize volume: {error:?}")));

    let mut sector = [0_u8; SECTOR_SIZE];
    for (index, chunk) in rootfs.chunks(SECTOR_SIZE).enumerate() {
        sector.fill(0);
        sector[..chunk.len()].copy_from_slice(chunk);
        if !device.write_sector(OBJECT_START_SECTOR as usize + index, &sector) {
            fail("cannot write rootfs extent");
        }
    }
    if !device.flush() {
        fail("cannot flush rootfs extent");
    }

    let object = Layer {
        digest,
        start_sector: OBJECT_START_SECTOR,
        length: rootfs.len() as u64,
    };
    let mut catalog = Catalog::new();
    if !catalog.upsert(arguments[3].as_bytes(), Platform::LinuxAmd64, &[object])
        || !catalog.set_rootfs(arguments[3].as_bytes(), Platform::LinuxAmd64, object)
    {
        fail("cannot create OCI catalog entry");
    }
    let mut encoded = [0_u8; ENCODED_SIZE];
    if !catalog.encode(&mut encoded) {
        fail("cannot encode OCI catalog");
    }
    commit_oci_catalog(&mut device, &encoded)
        .unwrap_or_else(|error| fail(&format!("cannot commit OCI catalog: {error:?}")));
    println!(
        "Seeded {} ({} bytes, sha256:{})",
        arguments[3],
        rootfs.len(),
        digest
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    );
}
