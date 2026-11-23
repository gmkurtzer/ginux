use flate2::bufread::MultiGzDecoder;
use reqwest::blocking::{Client, Response};
use reqwest::header::{ACCEPT, WWW_AUTHENTICATE};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::{Component, Path, PathBuf};
use tar::Archive;
use url::Url;

const ACCEPT_MANIFESTS: &str = "application/vnd.oci.image.index.v1+json, application/vnd.docker.distribution.manifest.list.v2+json, application/vnd.oci.image.manifest.v1+json, application/vnd.docker.distribution.manifest.v2+json";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageReference {
    pub registry: String,
    pub repository: String,
    pub digest: String,
}

#[derive(Debug, Clone)]
struct Descriptor {
    digest: String,
    media_type: String,
    size: Option<u64>,
}

pub fn parse_reference(value: &str) -> Result<ImageReference, String> {
    let (repository, digest) = value
        .rsplit_once('@')
        .ok_or_else(|| "OCI image reference must include @sha256:DIGEST".to_owned())?;
    validate_digest(digest)?;
    let (registry, repository) = repository
        .split_once('/')
        .ok_or_else(|| "OCI image reference must include a registry".to_owned())?;
    if registry.is_empty() || repository.is_empty() {
        return Err("OCI image reference has an empty registry or repository".into());
    }
    if !registry
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || b".-:".contains(&byte))
        || !repository
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._/-".contains(&byte))
    {
        return Err("OCI image reference contains unsupported characters".into());
    }
    let repository = if registry == "docker.io" && !repository.contains('/') {
        format!("library/{repository}")
    } else {
        repository.to_owned()
    };
    Ok(ImageReference {
        registry: registry.to_owned(),
        repository,
        digest: digest.to_owned(),
    })
}

pub fn materialize(
    image: &str,
    cache_root: &Path,
    destination: &Path,
    mut progress: impl Write,
) -> Result<(), String> {
    let reference = parse_reference(image)?;
    if destination.exists() && destination.read_dir().map_err(io_error)?.next().is_some() {
        return Err(format!(
            "instance root is not empty: {}",
            destination.display()
        ));
    }
    fs::create_dir_all(destination).map_err(io_error)?;
    let mut session = None;
    let mut fetch = |digest: &str, accept: &str, label: &str, size, progress: &mut _| {
        fetch_blob(cache_root, digest, label, size, progress, || {
            let registry = match &session {
                Some(registry) => registry,
                None => session.insert(RegistrySession::connect(&reference)?),
            };
            registry.download(
                &blob_url(&reference, digest, label.contains("manifest")),
                accept,
                label,
            )
        })
    };
    let root = fetch(
        &reference.digest,
        ACCEPT_MANIFESTS,
        "manifest",
        None,
        &mut progress,
    )?;
    let root_document = read_json(&root)?;
    let manifest_descriptor = select_manifest(&root_document)?;
    let manifest_path = if let Some(descriptor) = manifest_descriptor {
        fetch(
            &descriptor.digest,
            ACCEPT_MANIFESTS,
            "platform manifest",
            descriptor.size,
            &mut progress,
        )?
    } else {
        root
    };
    let manifest = read_json(&manifest_path)?;
    let config = descriptor(manifest.get("config"), "config")?;
    fetch(
        &config.digest,
        "application/vnd.oci.image.config.v1+json, application/vnd.docker.container.image.v1+json",
        "config",
        config.size,
        &mut progress,
    )?;
    let layers = manifest
        .get("layers")
        .and_then(Value::as_array)
        .ok_or_else(|| "OCI manifest has no layer array".to_owned())?;
    writeln!(
        progress,
        "Preparing {} from {} cached OCI layer(s)",
        image,
        layers.len()
    )
    .map_err(io_error)?;
    for (index, layer) in layers.iter().enumerate() {
        let layer = descriptor(Some(layer), "layer")?;
        let path = fetch(
            &layer.digest,
            &layer.media_type,
            &format!("layer {}/{}", index + 1, layers.len()),
            layer.size,
            &mut progress,
        )?;
        apply_layer(&path, &layer.media_type, destination)?;
    }
    Ok(())
}

struct RegistrySession {
    client: Client,
    token: Option<String>,
}

impl RegistrySession {
    fn connect(image: &ImageReference) -> Result<Self, String> {
        let client = Client::builder()
            .user_agent(concat!("ginux/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|error| format!("unable to initialize OCI client: {error}"))?;
        let token = registry_token(&client, image)?;
        Ok(Self { client, token })
    }

    fn download(
        &self,
        url: &str,
        accept: &str,
        label: &str,
    ) -> Result<(Response, Option<u64>), String> {
        let mut request = self.client.get(url).header(ACCEPT, accept);
        if let Some(token) = &self.token {
            request = request.bearer_auth(token);
        }
        let response = request
            .send()
            .and_then(Response::error_for_status)
            .map_err(|error| format!("unable to download OCI {label}: {error}"))?;
        let length = response.content_length();
        Ok((response, length))
    }
}

fn registry_token(client: &Client, image: &ImageReference) -> Result<Option<String>, String> {
    let url = manifest_url(image, &image.digest);
    let response = client
        .get(&url)
        .header(ACCEPT, ACCEPT_MANIFESTS)
        .send()
        .map_err(|error| format!("OCI registry request failed: {error}"))?;
    if response.status().is_success() {
        return Ok(None);
    }
    if response.status() != reqwest::StatusCode::UNAUTHORIZED {
        return Err(format!(
            "OCI registry returned HTTP {} for {url}",
            response.status()
        ));
    }
    let challenge = response
        .headers()
        .get(WWW_AUTHENTICATE)
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| "OCI registry omitted its authentication challenge".to_owned())?;
    let fields = bearer_fields(challenge)?;
    let realm = fields
        .iter()
        .find(|(key, _)| key == "realm")
        .map(|(_, value)| value)
        .ok_or_else(|| "OCI bearer challenge omitted realm".to_owned())?;
    let mut token_url =
        Url::parse(realm).map_err(|error| format!("invalid OCI authentication realm: {error}"))?;
    {
        let mut query = token_url.query_pairs_mut();
        if let Some((_, service)) = fields.iter().find(|(key, _)| key == "service") {
            query.append_pair("service", service);
        }
        let scope = fields
            .iter()
            .find(|(key, _)| key == "scope")
            .map(|(_, value)| value.clone())
            .unwrap_or_else(|| format!("repository:{}:pull", image.repository));
        query.append_pair("scope", &scope);
    }
    let document: Value = client
        .get(token_url)
        .send()
        .and_then(Response::error_for_status)
        .map_err(|error| format!("OCI token request failed: {error}"))?
        .json()
        .map_err(|error| format!("OCI token response is invalid: {error}"))?;
    document
        .get("token")
        .or_else(|| document.get("access_token"))
        .and_then(Value::as_str)
        .map(|value| Some(value.to_owned()))
        .ok_or_else(|| "OCI token response omitted a token".to_owned())
}

fn bearer_fields(challenge: &str) -> Result<Vec<(String, String)>, String> {
    let challenge = challenge
        .strip_prefix("Bearer ")
        .or_else(|| challenge.strip_prefix("bearer "))
        .ok_or_else(|| "unsupported OCI authentication challenge".to_owned())?;
    let mut fields = Vec::new();
    for field in challenge.split(',') {
        let (key, value) = field
            .trim()
            .split_once('=')
            .ok_or_else(|| "malformed OCI authentication challenge".to_owned())?;
        let value = value
            .strip_prefix('"')
            .and_then(|value| value.strip_suffix('"'))
            .ok_or_else(|| "malformed OCI authentication challenge value".to_owned())?;
        fields.push((key.to_owned(), value.to_owned()));
    }
    Ok(fields)
}

fn fetch_blob<R: Read>(
    cache_root: &Path,
    digest: &str,
    label: &str,
    expected_size: Option<u64>,
    progress: &mut impl Write,
    download: impl FnOnce() -> Result<(R, Option<u64>), String>,
) -> Result<PathBuf, String> {
    validate_digest(digest)?;
    let hex = digest.trim_start_matches("sha256:");
    let directory = cache_root.join("blobs/sha256");
    fs::create_dir_all(&directory).map_err(io_error)?;
    let target = directory.join(hex);
    if target.is_file() && digest_file(&target)? == digest {
        writeln!(progress, "  {label}: cached").map_err(io_error)?;
        return Ok(target);
    }
    let (mut response, content_length) = download()?;
    let total = expected_size.or(content_length);
    let temporary = directory.join(format!(".{hex}.{}.partial", std::process::id()));
    let result = (|| {
        let mut file = File::create(&temporary).map_err(io_error)?;
        let mut hasher = Sha256::new();
        let mut downloaded = 0u64;
        let mut next_report = 0u64;
        let mut buffer = [0u8; 64 * 1024];
        loop {
            let count = response.read(&mut buffer).map_err(io_error)?;
            if count == 0 {
                break;
            }
            file.write_all(&buffer[..count]).map_err(io_error)?;
            hasher.update(&buffer[..count]);
            downloaded += count as u64;
            if downloaded >= next_report {
                if let Some(total) = total.filter(|total| *total > 0) {
                    write!(
                        progress,
                        "\r  {label}: {:>3}% ({}/{})",
                        downloaded.saturating_mul(100) / total,
                        human_size(downloaded),
                        human_size(total)
                    )
                    .map_err(io_error)?;
                    progress.flush().map_err(io_error)?;
                    next_report = downloaded + (total / 100).max(256 * 1024);
                }
            }
        }
        file.sync_all().map_err(io_error)?;
        if total.is_some() {
            writeln!(progress).map_err(io_error)?;
        }
        let actual = format!("sha256:{:x}", hasher.finalize());
        if actual != digest {
            return Err(format!(
                "OCI {label} digest mismatch: expected {digest}, got {actual}"
            ));
        }
        fs::rename(&temporary, &target).map_err(io_error)?;
        Ok(target)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn select_manifest(document: &Value) -> Result<Option<Descriptor>, String> {
    let Some(manifests) = document.get("manifests") else {
        return Ok(None);
    };
    let manifests = manifests
        .as_array()
        .ok_or_else(|| "OCI index manifests is not an array".to_owned())?;
    for manifest in manifests {
        let platform = manifest.get("platform").and_then(Value::as_object);
        let os = platform
            .and_then(|value| value.get("os"))
            .and_then(Value::as_str);
        let architecture = platform
            .and_then(|value| value.get("architecture"))
            .and_then(Value::as_str);
        let variant = platform
            .and_then(|value| value.get("variant"))
            .and_then(Value::as_str)
            .unwrap_or("");
        if os == Some("linux") && architecture == Some("amd64") && variant.is_empty() {
            return descriptor(Some(manifest), "platform manifest").map(Some);
        }
    }
    Err("OCI image has no linux/amd64 manifest".into())
}

fn descriptor(value: Option<&Value>, label: &str) -> Result<Descriptor, String> {
    let value = value
        .and_then(Value::as_object)
        .ok_or_else(|| format!("OCI {label} descriptor is missing or invalid"))?;
    let digest = value
        .get("digest")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("OCI {label} descriptor has no digest"))?;
    validate_digest(digest)?;
    Ok(Descriptor {
        digest: digest.to_owned(),
        media_type: value
            .get("mediaType")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned(),
        size: value.get("size").and_then(Value::as_u64),
    })
}

fn apply_layer(blob: &Path, media_type: &str, destination: &Path) -> Result<(), String> {
    match media_type {
        "application/vnd.oci.image.layer.v1.tar+gzip"
        | "application/vnd.docker.image.rootfs.diff.tar.gzip"
        | "" => {
            apply_archive(
                Archive::new(MultiGzDecoder::new(BufReader::new(
                    File::open(blob).map_err(io_error)?,
                ))),
                blob,
                destination,
            )?;
        }
        "application/vnd.oci.image.layer.v1.tar"
        | "application/vnd.docker.image.rootfs.diff.tar" => {
            apply_archive(
                Archive::new(BufReader::new(File::open(blob).map_err(io_error)?)),
                blob,
                destination,
            )?;
        }
        other => return Err(format!("unsupported OCI layer media type: {other}")),
    }
    Ok(())
}

fn apply_archive<R: Read>(
    mut archive: Archive<R>,
    blob: &Path,
    destination: &Path,
) -> Result<(), String> {
    let whiteouts = collect_whiteouts(&mut archive)?;
    for whiteout in whiteouts {
        apply_whiteout(destination, &whiteout)?;
    }
    let file = File::open(blob).map_err(io_error)?;
    let is_gzip = {
        let mut reader = BufReader::new(file);
        let bytes = reader.fill_buf().map_err(io_error)?;
        bytes.starts_with(&[0x1f, 0x8b])
    };
    if is_gzip {
        unpack_entries(
            Archive::new(MultiGzDecoder::new(BufReader::new(
                File::open(blob).map_err(io_error)?,
            ))),
            destination,
        )
    } else {
        unpack_entries(
            Archive::new(BufReader::new(File::open(blob).map_err(io_error)?)),
            destination,
        )
    }
}

fn collect_whiteouts<R: Read>(archive: &mut Archive<R>) -> Result<Vec<PathBuf>, String> {
    let mut whiteouts = Vec::new();
    for entry in archive.entries().map_err(io_error)? {
        let entry = entry.map_err(io_error)?;
        let path = safe_tar_path(&entry.path().map_err(io_error)?)?;
        if is_whiteout(&path) {
            whiteouts.push(path);
        }
    }
    Ok(whiteouts)
}

fn unpack_entries<R: Read>(mut archive: Archive<R>, destination: &Path) -> Result<(), String> {
    archive.set_preserve_permissions(true);
    archive.set_preserve_mtime(true);
    archive.set_unpack_xattrs(true);
    for entry in archive.entries().map_err(io_error)? {
        let mut entry = entry.map_err(io_error)?;
        let path = safe_tar_path(&entry.path().map_err(io_error)?)?;
        if is_whiteout(&path) {
            continue;
        }
        entry
            .unpack_in(destination)
            .map_err(|error| format!("unable to unpack OCI entry {}: {error}", path.display()))?;
    }
    Ok(())
}

fn apply_whiteout(root: &Path, path: &Path) -> Result<(), String> {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "OCI whiteout name is not UTF-8".to_owned())?;
    let parent = path.parent().unwrap_or_else(|| Path::new(""));
    let target = if name == ".wh..wh..opq" {
        root.join(parent)
    } else {
        root.join(parent).join(name.trim_start_matches(".wh."))
    };
    if name == ".wh..wh..opq" {
        if target.is_dir() {
            for entry in fs::read_dir(&target).map_err(io_error)? {
                remove_path(&entry.map_err(io_error)?.path())?;
            }
        }
    } else {
        remove_path(&target)?;
    }
    Ok(())
}

fn remove_path(path: &Path) -> Result<(), String> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(io_error(error)),
    };
    if metadata.file_type().is_dir() {
        fs::remove_dir_all(path).map_err(io_error)
    } else {
        fs::remove_file(path).map_err(io_error)
    }
}

fn safe_tar_path(path: &Path) -> Result<PathBuf, String> {
    let mut safe = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(value) => safe.push(value),
            Component::CurDir => {}
            _ => return Err(format!("unsafe OCI layer path: {}", path.display())),
        }
    }
    Ok(safe)
}

fn is_whiteout(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with(".wh."))
}

fn manifest_url(image: &ImageReference, digest: &str) -> String {
    format!(
        "https://{}/v2/{}/manifests/{digest}",
        api_registry(&image.registry),
        image.repository
    )
}

fn blob_url(image: &ImageReference, digest: &str, manifest: bool) -> String {
    if manifest {
        manifest_url(image, digest)
    } else {
        format!(
            "https://{}/v2/{}/blobs/{digest}",
            api_registry(&image.registry),
            image.repository
        )
    }
}

fn api_registry(registry: &str) -> &str {
    if registry == "docker.io" {
        "registry-1.docker.io"
    } else {
        registry
    }
}

fn validate_digest(digest: &str) -> Result<(), String> {
    let Some(hex) = digest.strip_prefix("sha256:") else {
        return Err("OCI digest must use sha256".into());
    };
    if hex.len() != 64 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("OCI sha256 digest is malformed".into());
    }
    Ok(())
}

fn digest_file(path: &Path) -> Result<String, String> {
    let mut file = File::open(path).map_err(io_error)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let count = file.read(&mut buffer).map_err(io_error)?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    Ok(format!("sha256:{:x}", hasher.finalize()))
}

fn read_json(path: &Path) -> Result<Value, String> {
    serde_json::from_reader(BufReader::new(File::open(path).map_err(io_error)?))
        .map_err(|error| format!("invalid OCI JSON object {}: {error}", path.display()))
}

fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KiB", "MiB", "GiB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

fn io_error(error: io::Error) -> String {
    error.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("ginux-oci-{label}-{nonce}"));
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn reference_requires_registry_and_digest() {
        let parsed = parse_reference(
            "docker.io/alpine@sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        )
        .unwrap();
        assert_eq!(parsed.repository, "library/alpine");
        assert!(parse_reference("alpine:latest").is_err());
        assert!(parse_reference("docker.io/alpine@sha256:nope").is_err());
    }

    #[test]
    fn layer_whiteouts_replace_lower_entries() {
        let root = temp_dir("whiteout");
        fs::create_dir_all(root.join("etc/opaque")).unwrap();
        fs::write(root.join("etc/remove"), b"old").unwrap();
        fs::write(root.join("etc/opaque/old"), b"old").unwrap();
        let mut bytes = Vec::new();
        {
            let mut builder = tar::Builder::new(&mut bytes);
            append(&mut builder, "etc/.wh.remove", b"");
            append(&mut builder, "etc/opaque/.wh..wh..opq", b"");
            append(&mut builder, "etc/opaque/new", b"new");
            builder.finish().unwrap();
        }
        let mut archive = Archive::new(Cursor::new(bytes.clone()));
        let whiteouts = collect_whiteouts(&mut archive).unwrap();
        for path in whiteouts {
            apply_whiteout(&root, &path).unwrap();
        }
        unpack_entries(Archive::new(Cursor::new(bytes)), &root).unwrap();
        assert!(!root.join("etc/remove").exists());
        assert!(!root.join("etc/opaque/old").exists());
        assert_eq!(fs::read(root.join("etc/opaque/new")).unwrap(), b"new");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn unsafe_layer_paths_are_rejected() {
        assert!(safe_tar_path(Path::new("../escape")).is_err());
        assert!(safe_tar_path(Path::new("/absolute")).is_err());
    }

    fn cache_bytes(cache: &Path, bytes: &[u8]) -> String {
        let hex = format!("{:x}", Sha256::digest(bytes));
        let directory = cache.join("blobs/sha256");
        fs::create_dir_all(&directory).unwrap();
        fs::write(directory.join(&hex), bytes).unwrap();
        format!("sha256:{hex}")
    }

    #[test]
    fn cached_index_materializes_without_a_registry() {
        let cache = temp_dir("offline");
        let mut tar = tar::Builder::new(Vec::new());
        append(&mut tar, "message", b"from the layer");
        let layer = cache_bytes(&cache, &tar.into_inner().unwrap());
        let config = cache_bytes(&cache, b"{}");
        let manifest = serde_json::json!({
            "config": {"digest": config},
            "layers": [{"digest": layer, "mediaType": "application/vnd.oci.image.layer.v1.tar"}]
        });
        let manifest = cache_bytes(&cache, &serde_json::to_vec(&manifest).unwrap());
        let index = serde_json::json!({"manifests": [{
            "digest": manifest, "platform": {"os": "linux", "architecture": "amd64"}
        }]});
        let index = cache_bytes(&cache, &serde_json::to_vec(&index).unwrap());
        let destination = cache.join("root");
        let reference = format!("127.0.0.1:0/example@{index}");
        materialize(&reference, &cache, &destination, io::sink()).unwrap();
        assert_eq!(
            fs::read(destination.join("message")).unwrap(),
            b"from the layer"
        );
        fs::remove_dir_all(cache).unwrap();
    }

    #[test]
    fn cached_gzip_members_form_one_tar_stream() {
        use flate2::{write::GzEncoder, Compression};

        let cache = temp_dir("gzip-members");
        let config = cache_bytes(&cache, b"{}");
        let mut lower = tar::Builder::new(Vec::new());
        append(&mut lower, "obsolete", b"lower layer");
        let lower = cache_bytes(&cache, &lower.into_inner().unwrap());

        let mut upper = tar::Builder::new(Vec::new());
        append(&mut upper, "first", b"first file");
        let between_entries = upper.get_ref().len();
        append(&mut upper, ".wh.obsolete", b"");
        append(&mut upper, "second", b"second file");
        let upper = upper.into_inner().unwrap();

        for (case, member_size) in [
            ("single", upper.len()),
            ("between-entries", between_entries),
            ("inside-file", tar::Header::new_gnu().as_bytes().len() + 3),
        ] {
            let mut gzip = Vec::new();
            for member in upper.chunks(member_size) {
                let mut encoder = GzEncoder::new(&mut gzip, Compression::fast());
                encoder.write_all(member).unwrap();
                encoder.finish().unwrap();
            }
            let manifest = serde_json::json!({
                "config": {"digest": config},
                "layers": [
                    {"digest": lower, "mediaType": "application/vnd.oci.image.layer.v1.tar"},
                    {"digest": cache_bytes(&cache, &gzip), "mediaType": "application/vnd.oci.image.layer.v1.tar+gzip"}
                ]
            });
            let digest = cache_bytes(&cache, &serde_json::to_vec(&manifest).unwrap());
            let reference = format!("127.0.0.1:0/example@{digest}");
            let root = cache.join(case);
            materialize(&reference, &cache, &root, io::sink())
                .unwrap_or_else(|error| panic!("{case}: {error}"));
            assert!(!root.join("obsolete").exists(), "{case}: whiteout missed");
            assert_eq!(
                fs::read(root.join("first")).unwrap(),
                b"first file",
                "{case}"
            );
            assert_eq!(
                fs::read(root.join("second")).unwrap(),
                b"second file",
                "{case}"
            );
            assert_eq!(fs::read_dir(root).unwrap().count(), 2, "{case}");
        }
        fs::remove_dir_all(cache).unwrap();
    }

    #[test]
    fn blob_downloads_only_on_a_miss_and_repairs_verified_content() {
        let cache = temp_dir("repair");
        let bytes = b"image content".as_slice();
        let digest = format!("sha256:{:x}", Sha256::digest(bytes));
        let fetch = |body| {
            fetch_blob(&cache, &digest, "layer", None, &mut io::sink(), || {
                Ok((body, None))
            })
        };
        let path = fetch(bytes).unwrap();
        assert_eq!(fs::read(&path).unwrap(), bytes);
        fetch_blob::<&[u8]>(&cache, &digest, "layer", None, &mut io::sink(), || {
            panic!("valid cached content triggered a download")
        })
        .unwrap();
        fs::write(&path, b"damaged").unwrap();
        assert!(fetch(b"wrong").unwrap_err().contains("digest mismatch"));
        assert_eq!(fs::read(&path).unwrap(), b"damaged");
        assert_eq!(fs::read_dir(path.parent().unwrap()).unwrap().count(), 1);
        fetch(bytes).unwrap();
        assert_eq!(fs::read(path).unwrap(), bytes);
        fs::remove_dir_all(cache).unwrap();
    }

    fn append(builder: &mut tar::Builder<impl Write>, path: &str, body: &[u8]) {
        let mut header = tar::Header::new_gnu();
        header.set_size(body.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder.append_data(&mut header, path, body).unwrap();
    }
}
