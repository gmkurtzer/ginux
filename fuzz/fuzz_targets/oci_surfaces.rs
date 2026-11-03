#![no_main]

// Fuzz harness for the OCI surfaces of gmkurtzer/ginux `crates/oci`.
//
// Upstream code is consumed read-only through a path dependency
// (`../crates/oci`); nothing upstream is modified or rewritten.
// Every parser that accepts bytes or strings derived from untrusted registry
// or layer input is exercised with the same raw fuzzer input:
//
//   1. tar/PAX rootfs parsing        Rootfs::parse (+ entry_count)
//      v2.6-2: FULL traversal/query layer mirroring the kernel call pattern
//      in kernel/src/published_rootfs.rs@c86945b1 (metadata/from_metadata,
//      entry_and_next_at_offset linear pass, entry_at_offset, find_index,
//      next_child, copy_path) with attacker-shaped paths/indices — this layer
//      measured 0% coverage pre-fix (reports/fuzz-quality.md §a).
//   2. registry manifest             parse_registry_manifest (+ layer walk)
//   3. manifest index selection      select_manifest / platform match
//   4. image configuration           image_configuration_matches_platform
//   5. image reference               ImageReference::parse (+ writers)
//   6. digest validation             valid_registry_digest
//   7. bearer auth surfaces          BearerChallenge::parse / parse_bearer_token
//   8. digest verification           verify_sha256
//
// Anything that panics (indexing, arithmetic, unwrap on attacker-controlled
// data) is a finding; returning None/Err is fine.

use libfuzzer_sys::fuzz_target;

/// Same buffer size the kernel uses for entry paths
/// (TAR_PREFIX_SIZE + 1 + TAR_NAME_SIZE = 256).
const PATH_BUF: usize = 256;

fuzz_target!(|data: &[u8]| {
    // 1) tar/PAX rootfs (layer bytes: ustar headers, PAX extended headers,
    //    truncated/trailing garbage).
    if let Some(rootfs) = ginux_oci::Rootfs::parse(data) {
        let _ = rootfs.entry_count();

        // Reopen path used by the kernel: metadata -> RootfsMetadata::new ->
        // from_metadata over the same immutable bytes.
        let metadata = rootfs.metadata();
        let _ = ginux_oci::RootfsMetadata::new(
            metadata.entry_count(),
            metadata.content_length(),
        );
        let _ = ginux_oci::Rootfs::from_metadata(data, metadata);
        let _ = rootfs.content_length();

        // Linear indexing pass: offsets come from the archive itself, exactly
        // like published_rootfs.rs (entry_and_next_at_offset, line 207).
        let mut offsets = [0_usize; 64];
        let mut count = 0_usize;
        let mut offset = 0_usize;
        while count < offsets.len() {
            match rootfs.entry_and_next_at_offset(offset) {
                Some((_, next)) => {
                    offsets[count] = offset;
                    count += 1;
                    if next <= offset {
                        break;
                    }
                    offset = next;
                }
                None => break,
            }
        }
        for &stored in &offsets[..count] {
            let _ = rootfs.entry_at_offset(stored);
        }
        for index in 0..count {
            let _ = rootfs.entry_indexed(index);
            let _ = rootfs.entry(index);
            let mut out = [0_u8; PATH_BUF];
            let _ = rootfs.copy_path(index, &mut out);
        }
        // Path queries with paths taken from the archive's own entries — those
        // bytes are attacker-controlled layer content by definition, and they
        // are what reaches whiteout/overlay matching in the kernel.
        for index in 0..count {
            let mut pb = [0_u8; PATH_BUF];
            if let Some(len) = rootfs.copy_path(index, &mut pb) {
                let entry_path = &pb[..len];
                let _ = rootfs.find_index(entry_path);
                let _ = rootfs.find(entry_path);
                if let Some((_, stored, _)) = rootfs.find_indexed(entry_path) {
                    let _ = rootfs.entry_at_offset(stored);
                }
                let mut cursor = 0_usize;
                for _ in 0..2 {
                    match rootfs.next_child(entry_path, cursor) {
                        Some(Some(child)) => cursor = child.next_cursor,
                        _ => break,
                    }
                }
            }
        }
        // Out-of-range probes (None branches).
        let _ = rootfs.entry_indexed(count + 1);
        let _ = rootfs.entry(count + 1);
        let mut out = [0_u8; PATH_BUF];
        let _ = rootfs.copy_path(count + 1, &mut out);

        // Attacker-shaped path queries (kernel: find_index with a guest path).
        if data.len() >= 2 {
            let mid = data.len() / 2;
            let path_a = &data[..mid];
            let path_b = &data[mid..];
            let _ = rootfs.find_index(path_a);
            let _ = rootfs.find(path_a);
            let _ = rootfs.find_indexed(path_b);
            if let Some((_, stored, _)) = rootfs.find_indexed(path_a) {
                let _ = rootfs.entry_at_offset(stored);
            }
            // Child iteration with attacker parent + cursor.
            let mut cursor = mid % (count + 1);
            for _ in 0..4 {
                match rootfs.next_child(path_a, cursor) {
                    Some(Some(child)) => cursor = child.next_cursor,
                    _ => break,
                }
            }
            let _ = rootfs.copy_path(0, &mut out);
        }
    }

    // 2) registry manifest (manifest JSON with config + layers).
    if let Some(manifest) = ginux_oci::parse_registry_manifest(data) {
        for layer in manifest.layers.iter() {
            let _ = (layer.media_type, layer.digest, layer.size);
        }
        let _ = (manifest.config.media_type, manifest.config.digest);
    }

    // 3) manifest index selection against the native platform.
    let _ = ginux_oci::select_manifest(data, ginux_oci::Platform::native_linux());

    // 4) image configuration bytes vs native platform.
    let _ = ginux_oci::image_configuration_matches_platform(
        data,
        ginux_oci::Platform::native_linux(),
    );

    // 5) image reference (registry/repository/tag-or-digest bytes).
    if let Ok(reference) = ginux_oci::ImageReference::parse(data) {
        let mut out = [0_u8; ginux_oci::REFERENCE_CAPACITY];
        let _ = reference.write_canonical(&mut out);
        let _ = reference.write_manifest_path(&mut out);
        let _ = reference.manifest_digest_matches(data);
    }

    // 6)/7) UTF-8 string surfaces: digest validation and bearer challenge.
    if let Ok(text) = core::str::from_utf8(data) {
        let _ = ginux_oci::valid_registry_digest(text);
        let _ = ginux_oci::BearerChallenge::parse(text);
    }

    // 7) bearer token body.
    let _ = ginux_oci::parse_bearer_token(data);

    // 8) sha256 verification (expected digest of all-zero bytes; the point is
    //    reaching the comparison logic with attacker-shaped input).
    let expected = [0_u8; 32];
    let _ = ginux_oci::verify_sha256(data, &expected);
});
