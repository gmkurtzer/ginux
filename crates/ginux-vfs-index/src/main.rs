use ginux_vfs::{
    encode_header, encode_record, Entry, Index, NodeKind, HEADER_SIZE, MAX_LINK, MAX_PATH,
    RECORD_SIZE,
};
use std::collections::BTreeMap;
use std::env;
use std::fs::File;
use std::io::{copy, Read, Seek, SeekFrom, Write};
use std::path::PathBuf;

type PaxRecords<'a> = Vec<(&'a [u8], &'a [u8])>;

#[derive(Debug)]
struct OwnedEntry {
    path: Vec<u8>,
    kind: NodeKind,
    mode: u32,
    uid: u32,
    gid: u32,
    size: u64,
    data_offset: u64,
    link: Vec<u8>,
}

#[derive(Debug)]
struct LayerRecord {
    layer: usize,
    path: Vec<u8>,
    kind: u8,
    record_offset: u64,
    record_len: u64,
    /// For a hard link: data offset and size of its target in the same layer.
    /// The link is written out as a regular file with that content, so it
    /// keeps the target's data even if a later layer replaces the target.
    hardlink: Option<(u64, u64)>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = env::args_os();
    let _program = arguments.next();
    let first = arguments.next().ok_or(
        "usage: ginux-vfs-index ROOTFS.tar OUTPUT.index | assemble OUTPUT.tar OUTPUT.index LAYER.tar...",
    )?;
    if first == "assemble" {
        let rootfs = arguments.next().ok_or("missing assembled rootfs output")?;
        let index = arguments.next().ok_or("missing assembled index output")?;
        let layers = arguments.map(PathBuf::from).collect::<Vec<_>>();
        if layers.is_empty() {
            return Err("at least one OCI layer is required".into());
        }
        let rootfs = PathBuf::from(rootfs);
        assemble_layers(&layers, rootfs.clone())?;
        return build_assembled_index(rootfs, PathBuf::from(index), layers.len());
    }
    let output = arguments
        .next()
        .ok_or("usage: ginux-vfs-index ROOTFS.tar OUTPUT.index")?;
    if arguments.next().is_some() {
        return Err("usage: ginux-vfs-index ROOTFS.tar OUTPUT.index".into());
    }
    build_index_from(first.into(), output.into())
}

fn build_index_from(input: PathBuf, output: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let mut source = File::open(input)?;
    let source_len = source.metadata()?.len();
    let mut entries = scan_tar(&mut source, source_len)?;
    entries.sort_by(|left, right| left.path.cmp(&right.path));
    entries.dedup_by(|left, right| left.path == right.path);
    if entries.len() > u32::MAX as usize {
        return Err("too many rootfs entries".into());
    }
    let mut destination = File::create(&output)?;
    let mut header = [0u8; HEADER_SIZE];
    encode_header(entries.len() as u32, &mut header);
    destination.write_all(&header)?;
    for owned in entries {
        let entry = Entry {
            path: &owned.path,
            kind: owned.kind,
            mode: owned.mode,
            uid: owned.uid,
            gid: owned.gid,
            size: owned.size,
            data_offset: owned.data_offset,
            link: &owned.link,
        };
        let mut record = [0u8; RECORD_SIZE];
        encode_record(entry, &mut record).map_err(|_| "invalid rootfs entry")?;
        destination.write_all(&record)?;
    }
    destination.flush()?;
    drop(destination);
    verify_index(&mut source, File::open(output)?)?;
    Ok(())
}

fn build_assembled_index(
    rootfs: PathBuf,
    output: PathBuf,
    layer_count: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    build_index_from(rootfs, output)?;
    println!("ASSEMBLE: layers={layer_count} whiteouts=applied");
    Ok(())
}

fn assemble_layers(
    layer_paths: &[PathBuf],
    output: PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut merged = BTreeMap::<Vec<u8>, LayerRecord>::new();
    let mut layers = Vec::with_capacity(layer_paths.len());
    for (layer_index, path) in layer_paths.iter().enumerate() {
        let mut source = File::open(path)?;
        let source_len = source.metadata()?.len();
        let records = scan_layer(&mut source, source_len, layer_index)?;

        for record in records.iter().filter(|record| whiteout(record).is_some()) {
            let (target, opaque) = whiteout(record).expect("filtered whiteout");
            if opaque {
                remove_descendants(&mut merged, &target);
            } else {
                merged.remove(&target);
                remove_descendants(&mut merged, &target);
            }
        }
        for record in records {
            if whiteout(&record).is_some() {
                continue;
            }
            if !matches!(record.kind, 0 | b'0' | b'1' | b'2' | b'5' | b'7') {
                continue;
            }
            if !matches!(record.kind, b'5') {
                remove_descendants(&mut merged, &record.path);
            }
            merged.insert(record.path.clone(), record);
        }
        layers.push(source);
    }

    let mut destination = File::create(output)?;
    for record in merged.values() {
        let source = layers
            .get_mut(record.layer)
            .ok_or("invalid layer record source")?;
        if let Some((data_offset, size)) = record.hardlink {
            write_hardlink_as_file(source, record, data_offset, size, &mut destination)?;
            continue;
        }
        source.seek(SeekFrom::Start(record.record_offset))?;
        let copied = copy(&mut source.take(record.record_len), &mut destination)?;
        if copied != record.record_len {
            return Err("truncated OCI layer record".into());
        }
    }
    destination.write_all(&[0u8; 1024])?;
    destination.flush()?;
    Ok(())
}

/// Write a hard-link record as a regular file: its PAX headers without
/// `linkpath`, its ustar header with type `0`, the target's size and no link
/// name, then the target's data from the same layer.
fn write_hardlink_as_file(
    source: &mut File,
    record: &LayerRecord,
    data_offset: u64,
    size: u64,
    destination: &mut File,
) -> Result<(), Box<dyn std::error::Error>> {
    if size > 0o77777777777 {
        return Err("hard-link target too large for a ustar size field".into());
    }
    let header_offset = record.record_offset + record.record_len - 512;
    let mut offset = record.record_offset;
    while offset < header_offset {
        source.seek(SeekFrom::Start(offset))?;
        let mut pax_header = [0u8; 512];
        source.read_exact(&mut pax_header)?;
        let pax_size = parse_octal(&pax_header[124..136])?;
        let mut data = vec![0u8; usize::try_from(pax_size).map_err(|_| "PAX record too large")?];
        source.read_exact(&mut data)?;
        // A regular file must not carry the link's target name.
        let mut kept = Vec::with_capacity(data.len());
        let mut start = 0usize;
        for (key, _) in parse_pax(&data)? {
            let space = data[start..]
                .iter()
                .position(|byte| *byte == b' ')
                .ok_or("invalid PAX length")?;
            let length = std::str::from_utf8(&data[start..start + space])?.parse::<usize>()?;
            if key != b"linkpath" {
                kept.extend_from_slice(&data[start..start + length]);
            }
            start += length;
        }
        if !kept.is_empty() {
            set_size_and_checksum(&mut pax_header, kept.len() as u64);
            destination.write_all(&pax_header)?;
            destination.write_all(&kept)?;
            destination.write_all(&vec![0u8; (512 - kept.len() % 512) % 512])?;
        }
        offset += 512 + pax_size.div_ceil(512) * 512;
    }
    source.seek(SeekFrom::Start(header_offset))?;
    let mut header = [0u8; 512];
    source.read_exact(&mut header)?;
    header[156] = b'0';
    header[157..257].fill(0);
    set_size_and_checksum(&mut header, size);
    destination.write_all(&header)?;
    source.seek(SeekFrom::Start(data_offset))?;
    if copy(&mut (&mut *source).take(size), destination)? != size {
        return Err("truncated hard-link target data".into());
    }
    let padding = (512 - size % 512) % 512;
    destination.write_all(&vec![0u8; padding as usize])?;
    Ok(())
}

fn set_size_and_checksum(header: &mut [u8; 512], size: u64) {
    header[124..136].copy_from_slice(format!("{size:011o}\0").as_bytes());
    header[148..156].fill(b' ');
    let checksum: u64 = header.iter().map(|byte| u64::from(*byte)).sum();
    header[148..156].copy_from_slice(format!("{checksum:06o}\0 ").as_bytes());
}

fn scan_layer(
    source: &mut File,
    source_len: u64,
    layer: usize,
) -> Result<Vec<LayerRecord>, Box<dyn std::error::Error>> {
    let mut records = Vec::new();
    // Regular files seen so far in this layer: path -> (data offset, size).
    let mut files = BTreeMap::<Vec<u8>, (u64, u64)>::new();
    let mut offset = 0u64;
    let mut pending_offset: Option<u64> = None;
    let mut pax_path: Option<Vec<u8>> = None;
    let mut pax_link: Option<Vec<u8>> = None;
    while offset + 512 <= source_len {
        source.seek(SeekFrom::Start(offset))?;
        let mut header = [0u8; 512];
        source.read_exact(&mut header)?;
        if header.iter().all(|byte| *byte == 0) {
            break;
        }
        verify_checksum(&header)?;
        let size = parse_octal(&header[124..136])?;
        let data_offset = offset + 512;
        let padded = size.checked_add(511).ok_or("tar size overflow")? / 512 * 512;
        let end = data_offset
            .checked_add(padded)
            .ok_or("tar offset overflow")?;
        if end > source_len {
            return Err("truncated OCI layer entry".into());
        }
        match header[156] {
            b'g' => return Err("global PAX headers are unsupported in OCI layers".into()),
            b'x' => {
                if pending_offset.is_none() {
                    pending_offset = Some(offset);
                }
                let size_usize = usize::try_from(size).map_err(|_| "PAX record too large")?;
                let mut data = vec![0u8; size_usize];
                source.read_exact(&mut data)?;
                for (key, value) in parse_pax(&data)? {
                    if key == b"path" {
                        pax_path = Some(value.to_vec());
                    } else if key == b"linkpath" {
                        pax_link = Some(value.to_vec());
                    }
                }
            }
            kind => {
                let path = pax_path.take().unwrap_or_else(|| header_path(&header));
                let path = normalize_archive_path(&path)?;
                let link = pax_link
                    .take()
                    .unwrap_or_else(|| field(&header[157..257]).to_vec());
                let hardlink = match kind {
                    0 | b'0' | b'7' => {
                        files.insert(path.clone(), (data_offset, size));
                        None
                    }
                    b'1' => {
                        let target = files.get(&normalize_archive_path(&link)?).copied();
                        if target.is_none() {
                            eprintln!(
                                "WARNING: dropping hard link {}: target is not an earlier regular file in its layer",
                                String::from_utf8_lossy(&path)
                            );
                            pending_offset = None;
                            offset = end;
                            continue;
                        }
                        target
                    }
                    _ => None,
                };
                if kind == b'1' {
                    if let Some(target) = hardlink {
                        files.insert(path.clone(), target);
                    }
                }
                let record_offset = pending_offset.take().unwrap_or(offset);
                records.push(LayerRecord {
                    layer,
                    path,
                    kind,
                    record_offset,
                    record_len: end - record_offset,
                    hardlink,
                });
            }
        }
        offset = end;
    }
    if pending_offset.is_some() {
        return Err("OCI layer ends with an unattached PAX header".into());
    }
    Ok(records)
}

fn whiteout(record: &LayerRecord) -> Option<(Vec<u8>, bool)> {
    let slash = record.path.iter().rposition(|byte| *byte == b'/')?;
    let parent = &record.path[..slash];
    let name = &record.path[slash + 1..];
    if name == b".wh..wh..opq" {
        return Some((if parent.is_empty() { b"/" } else { parent }.to_vec(), true));
    }
    let target_name = name.strip_prefix(b".wh.")?;
    if target_name.is_empty() {
        return None;
    }
    let mut target = if parent.is_empty() {
        b"/".to_vec()
    } else {
        parent.to_vec()
    };
    if target.len() > 1 {
        target.push(b'/');
    }
    target.extend_from_slice(target_name);
    Some((target, false))
}

fn remove_descendants(merged: &mut BTreeMap<Vec<u8>, LayerRecord>, path: &[u8]) {
    let mut prefix = path.to_vec();
    if prefix != b"/" {
        prefix.push(b'/');
    }
    merged.retain(|candidate, _| !candidate.starts_with(&prefix));
}

fn verify_index(source: &mut File, mut index_file: File) -> Result<(), Box<dyn std::error::Error>> {
    let mut bytes = Vec::new();
    index_file.read_to_end(&mut bytes)?;
    let index = Index::new(&bytes).map_err(|_| "generated index failed validation")?;
    for path in [
        b"/usr/bin/bash".as_slice(),
        b"/usr/lib64/ld-linux-x86-64.so.2".as_slice(),
        b"/usr/lib64/libc.so.6".as_slice(),
    ] {
        let entry = index.resolve(path).map_err(|_| {
            format!(
                "required Rocky runtime path missing: {}",
                String::from_utf8_lossy(path)
            )
        })?;
        if entry.kind != NodeKind::File || entry.size < 4 {
            return Err("required Rocky runtime path is not a regular ELF file".into());
        }
        source.seek(SeekFrom::Start(entry.data_offset))?;
        let mut magic = [0u8; 4];
        source.read_exact(&mut magic)?;
        if &magic != b"\x7fELF" {
            return Err("indexed Rocky runtime data offset is not an ELF file".into());
        }
    }
    println!(
        "INDEX: entries={} required-rocky-elfs=verified",
        index.len()
    );
    Ok(())
}

fn scan_tar(
    source: &mut File,
    source_len: u64,
) -> Result<Vec<OwnedEntry>, Box<dyn std::error::Error>> {
    let mut entries = Vec::new();
    let mut offset = 0u64;
    let mut pax_path: Option<Vec<u8>> = None;
    let mut pax_link: Option<Vec<u8>> = None;
    while offset + 512 <= source_len {
        source.seek(SeekFrom::Start(offset))?;
        let mut header = [0u8; 512];
        source.read_exact(&mut header)?;
        if header.iter().all(|byte| *byte == 0) {
            break;
        }
        verify_checksum(&header)?;
        let size = parse_octal(&header[124..136])?;
        let data_offset = offset + 512;
        let kind = header[156];
        if kind == b'x' || kind == b'g' {
            let size_usize = usize::try_from(size).map_err(|_| "PAX record too large")?;
            let mut data = vec![0u8; size_usize];
            source.read_exact(&mut data)?;
            for (key, value) in parse_pax(&data)? {
                if key == b"path" {
                    pax_path = Some(value.to_vec());
                } else if key == b"linkpath" {
                    pax_link = Some(value.to_vec());
                }
            }
        } else {
            let path = pax_path.take().unwrap_or_else(|| header_path(&header));
            let link = pax_link
                .take()
                .unwrap_or_else(|| field(&header[157..257]).to_vec());
            // Only symlinks and hard links carry a link name.
            let link = if matches!(kind, b'1' | b'2') {
                link
            } else {
                Vec::new()
            };
            let path = normalize_archive_path(&path)?;
            let node_kind = match kind {
                0 | b'0' | b'1' | b'7' => Some(NodeKind::File),
                b'5' => Some(NodeKind::Directory),
                b'2' => Some(NodeKind::Symlink),
                _ => None,
            };
            // A hard link shares the data of an earlier regular file.
            let (node_kind, size, data_offset, link) = if kind == b'1' {
                let target = normalize_archive_path(&link)?;
                match entries.iter().rev().find(|entry: &&OwnedEntry| {
                    entry.path == target && entry.kind == NodeKind::File
                }) {
                    Some(target) => (node_kind, target.size, target.data_offset, Vec::new()),
                    None => {
                        eprintln!(
                            "WARNING: dropping hard link {}: target is not an earlier regular file",
                            String::from_utf8_lossy(&path)
                        );
                        (None, 0, 0, Vec::new())
                    }
                }
            } else {
                (node_kind, size, data_offset, link)
            };
            if let Some(node_kind) = node_kind {
                if path.len() > MAX_PATH || link.len() > MAX_LINK {
                    return Err(format!(
                        "rootfs path or link exceeds index bounds: {}",
                        String::from_utf8_lossy(&path)
                    )
                    .into());
                }
                entries.push(OwnedEntry {
                    path,
                    kind: node_kind,
                    mode: parse_octal(&header[100..108])? as u32,
                    uid: parse_octal(&header[108..116])? as u32,
                    gid: parse_octal(&header[116..124])? as u32,
                    size: if node_kind == NodeKind::File { size } else { 0 },
                    data_offset,
                    link,
                });
            }
        }
        let padded = size.checked_add(511).ok_or("tar size overflow")? / 512 * 512;
        offset = data_offset
            .checked_add(padded)
            .ok_or("tar offset overflow")?;
    }
    if !entries.iter().any(|entry| entry.path == b"/") {
        entries.push(OwnedEntry {
            path: b"/".to_vec(),
            kind: NodeKind::Directory,
            mode: 0o755,
            uid: 0,
            gid: 0,
            size: 0,
            data_offset: 0,
            link: Vec::new(),
        });
    }
    Ok(entries)
}

fn field(bytes: &[u8]) -> &[u8] {
    let end = bytes
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(bytes.len());
    &bytes[..end]
}

fn header_path(header: &[u8; 512]) -> Vec<u8> {
    let name = field(&header[..100]);
    let prefix = field(&header[345..500]);
    let mut path = Vec::new();
    if !prefix.is_empty() {
        path.extend_from_slice(prefix);
        path.push(b'/');
    }
    path.extend_from_slice(name);
    path
}

fn normalize_archive_path(path: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut path = path;
    while path.starts_with(b"./") {
        path = &path[2..];
    }
    while path.ends_with(b"/") {
        path = &path[..path.len() - 1];
    }
    if path.is_empty() || path == b"." {
        return Ok(b"/".to_vec());
    }
    if path[0] == b'/' || path.split(|byte| *byte == b'/').any(|part| part == b"..") {
        return Err("unsafe rootfs path".into());
    }
    let mut output = Vec::with_capacity(path.len() + 1);
    output.push(b'/');
    output.extend_from_slice(path);
    Ok(output)
}

fn parse_octal(bytes: &[u8]) -> Result<u64, Box<dyn std::error::Error>> {
    let bytes = bytes
        .iter()
        .copied()
        .skip_while(|byte| *byte == b' ' || *byte == 0)
        .take_while(|byte| *byte != b' ' && *byte != 0)
        .collect::<Vec<_>>();
    if bytes.is_empty() {
        return Ok(0);
    }
    let text = std::str::from_utf8(&bytes)?;
    Ok(u64::from_str_radix(text, 8)?)
}

fn verify_checksum(header: &[u8; 512]) -> Result<(), Box<dyn std::error::Error>> {
    let expected = parse_octal(&header[148..156])?;
    let actual: u64 = header
        .iter()
        .enumerate()
        .map(|(index, byte)| {
            if (148..156).contains(&index) {
                b' ' as u64
            } else {
                *byte as u64
            }
        })
        .sum();
    if actual != expected {
        return Err("tar header checksum mismatch".into());
    }
    Ok(())
}

fn parse_pax(data: &[u8]) -> Result<PaxRecords<'_>, Box<dyn std::error::Error>> {
    let mut records = Vec::new();
    let mut offset = 0usize;
    while offset < data.len() {
        let space = data[offset..]
            .iter()
            .position(|byte| *byte == b' ')
            .ok_or("invalid PAX length")?
            + offset;
        let length = std::str::from_utf8(&data[offset..space])?.parse::<usize>()?;
        let end = offset.checked_add(length).ok_or("PAX length overflow")?;
        if end > data.len() || end <= space + 2 || data[end - 1] != b'\n' {
            return Err("invalid PAX record".into());
        }
        let record = &data[space + 1..end - 1];
        let equals = record
            .iter()
            .position(|byte| *byte == b'=')
            .ok_or("invalid PAX key")?;
        records.push((&record[..equals], &record[equals + 1..]));
        offset = end;
    }
    Ok(records)
}
