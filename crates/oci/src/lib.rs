#![no_std]

#[cfg(test)]
extern crate std;

use heapless::Vec;
use serde::Deserialize;
#[cfg(feature = "sha256")]
use sha2::{Digest, Sha256};

const TAR_BLOCK_SIZE: usize = 512;
const TAR_HEADER_SIZE: usize = 512;
const TAR_MAX_ENTRIES: usize = 32_768;
const TAR_NAME_SIZE: usize = 100;
const TAR_PREFIX_SIZE: usize = 155;
pub const REFERENCE_CAPACITY: usize = 128;
pub const OCI_LAYER_LIMIT: usize = 16;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BearerChallenge<'a> {
    pub realm: &'a str,
    pub service: &'a str,
    pub scope: &'a str,
}

impl<'a> BearerChallenge<'a> {
    pub fn parse(header: &'a str) -> Option<Self> {
        let fields = header.strip_prefix("Bearer ")?;
        Some(Self {
            realm: challenge_field(fields, "realm")?,
            service: challenge_field(fields, "service")?,
            scope: challenge_field(fields, "scope")?,
        })
    }
}

fn challenge_field<'a>(fields: &'a str, name: &str) -> Option<&'a str> {
    fields.split(',').find_map(|field| {
        let field = field.trim();
        let (key, value) = field.split_once('=')?;
        (key == name)
            .then(|| value.strip_prefix('"')?.strip_suffix('"'))
            .flatten()
    })
}

#[derive(Deserialize)]
struct TokenResponse<'a> {
    #[serde(borrow)]
    token: Option<&'a str>,
    #[serde(borrow)]
    access_token: Option<&'a str>,
}

pub fn parse_bearer_token(body: &[u8]) -> Option<&str> {
    let (response, used): (TokenResponse<'_>, usize) = serde_json_core::from_slice(body).ok()?;
    (body[used..].iter().all(u8::is_ascii_whitespace))
        .then(|| response.token.or(response.access_token))
        .flatten()
        .filter(|token| !token.is_empty())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RegistryDescriptor<'a> {
    pub media_type: &'a str,
    pub digest: &'a str,
    pub size: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Platform<'a> {
    pub os: &'a str,
    pub architecture: &'a str,
    pub variant: &'a str,
}

impl Platform<'static> {
    pub const fn native_linux() -> Self {
        #[cfg(target_arch = "x86_64")]
        return Self {
            os: "linux",
            architecture: "amd64",
            variant: "",
        };
        #[cfg(target_arch = "aarch64")]
        return Self {
            os: "linux",
            architecture: "arm64",
            variant: "v8",
        };
        #[cfg(target_arch = "arm")]
        return Self {
            os: "linux",
            architecture: "arm",
            variant: "v7",
        };
        #[cfg(target_arch = "riscv64")]
        return Self {
            os: "linux",
            architecture: "riscv64",
            variant: "",
        };
        #[cfg(not(any(
            target_arch = "x86_64",
            target_arch = "aarch64",
            target_arch = "arm",
            target_arch = "riscv64"
        )))]
        compile_error!("define the OCI architecture mapping for this native target");
    }
}

#[derive(Deserialize)]
struct JsonPlatform<'a> {
    #[serde(borrow)]
    architecture: &'a str,
    #[serde(borrow)]
    os: &'a str,
    #[serde(default, borrow)]
    variant: &'a str,
}

#[derive(Deserialize)]
struct JsonDescriptor<'a> {
    #[serde(rename = "mediaType", borrow)]
    media_type: &'a str,
    #[serde(borrow)]
    digest: &'a str,
    size: usize,
    #[serde(default, borrow)]
    platform: Option<JsonPlatform<'a>>,
}

#[derive(Deserialize)]
struct JsonIndex<'a> {
    #[serde(rename = "schemaVersion")]
    schema_version: u8,
    #[serde(borrow)]
    manifests: Vec<JsonDescriptor<'a>, OCI_LAYER_LIMIT>,
}

#[derive(Deserialize)]
struct JsonManifest<'a> {
    #[serde(rename = "schemaVersion")]
    schema_version: u8,
    #[serde(borrow)]
    config: JsonDescriptor<'a>,
    #[serde(borrow)]
    layers: Vec<JsonDescriptor<'a>, OCI_LAYER_LIMIT>,
}

#[derive(Deserialize)]
struct JsonImageConfiguration<'a> {
    #[serde(borrow)]
    architecture: &'a str,
    #[serde(borrow)]
    os: &'a str,
    #[serde(default, borrow)]
    variant: &'a str,
}

pub fn select_manifest<'a>(
    body: &'a [u8],
    required: Platform<'_>,
) -> Option<RegistryDescriptor<'a>> {
    let (index, used): (JsonIndex<'_>, usize) = serde_json_core::from_slice(body).ok()?;
    if index.schema_version != 2 || body[used..].iter().any(|byte| !byte.is_ascii_whitespace()) {
        return None;
    }
    index.manifests.iter().find_map(|descriptor| {
        descriptor.platform.as_ref().and_then(|platform| {
            (platform_matches(
                required,
                platform.os,
                platform.architecture,
                platform.variant,
            ) && valid_registry_digest(descriptor.digest)
                && descriptor.size != 0)
                .then_some(RegistryDescriptor {
                    media_type: descriptor.media_type,
                    digest: descriptor.digest,
                    size: descriptor.size,
                })
        })
    })
}

pub fn image_configuration_matches_platform(body: &[u8], required: Platform<'_>) -> bool {
    let Ok((configuration, used)) = serde_json_core::from_slice::<JsonImageConfiguration<'_>>(body)
    else {
        return false;
    };
    body[used..].iter().all(u8::is_ascii_whitespace)
        && platform_matches(
            required,
            configuration.os,
            configuration.architecture,
            configuration.variant,
        )
}

fn platform_matches(required: Platform<'_>, os: &str, architecture: &str, variant: &str) -> bool {
    os == required.os
        && architecture == required.architecture
        && (variant == required.variant || variant.is_empty() && !required.variant.is_empty())
}

#[derive(Debug, Eq, PartialEq)]
pub struct RegistryManifest<'a> {
    pub config: RegistryDescriptor<'a>,
    pub layers: Vec<RegistryDescriptor<'a>, OCI_LAYER_LIMIT>,
}

pub fn parse_registry_manifest(body: &[u8]) -> Option<RegistryManifest<'_>> {
    let (manifest, used): (JsonManifest<'_>, usize) = serde_json_core::from_slice(body).ok()?;
    if manifest.schema_version != 2
        || body[used..].iter().any(|byte| !byte.is_ascii_whitespace())
        || !valid_json_descriptor(&manifest.config)
        || manifest.layers.is_empty()
        || manifest
            .layers
            .iter()
            .any(|layer| !valid_json_descriptor(layer))
    {
        return None;
    }
    let mut layers = Vec::new();
    for layer in &manifest.layers {
        layers
            .push(RegistryDescriptor {
                media_type: layer.media_type,
                digest: layer.digest,
                size: layer.size,
            })
            .ok()?;
    }
    Some(RegistryManifest {
        config: RegistryDescriptor {
            media_type: manifest.config.media_type,
            digest: manifest.config.digest,
            size: manifest.config.size,
        },
        layers,
    })
}

fn valid_json_descriptor(descriptor: &JsonDescriptor<'_>) -> bool {
    !descriptor.media_type.is_empty()
        && valid_registry_digest(descriptor.digest)
        && descriptor.size != 0
}

pub fn valid_registry_digest(digest: &str) -> bool {
    digest.len() == 71
        && digest.starts_with("sha256:")
        && digest.as_bytes()[7..].iter().all(u8::is_ascii_hexdigit)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReferenceError {
    Empty,
    TooLong,
    InvalidRegistry,
    InvalidRepository,
    InvalidTag,
    InvalidDigest,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ImageReference<'a> {
    pub registry: &'a [u8],
    pub repository: &'a [u8],
    pub selector: &'a [u8],
    pub digest: bool,
}

impl<'a> ImageReference<'a> {
    pub fn parse(reference: &'a [u8]) -> Result<Self, ReferenceError> {
        if reference.is_empty() {
            return Err(ReferenceError::Empty);
        }
        if reference.len() > REFERENCE_CAPACITY || !reference.iter().all(u8::is_ascii_graphic) {
            return Err(ReferenceError::TooLong);
        }
        let (name, selector, digest) = if let Some(index) =
            reference.iter().position(|byte| *byte == b'@')
        {
            if reference[index + 1..].contains(&b'@') {
                return Err(ReferenceError::InvalidDigest);
            }
            (&reference[..index], &reference[index + 1..], true)
        } else {
            let slash = reference.iter().rposition(|byte| *byte == b'/');
            let colon = reference.iter().rposition(|byte| *byte == b':');
            if let Some(index) = colon.filter(|index| slash.is_none_or(|slash| *index > slash)) {
                (&reference[..index], &reference[index + 1..], false)
            } else {
                (reference, b"latest" as &[u8], false)
            }
        };
        if name.is_empty() {
            return Err(ReferenceError::InvalidRepository);
        }
        if digest {
            if selector.len() != 71
                || !selector.starts_with(b"sha256:")
                || !selector[7..].iter().all(u8::is_ascii_hexdigit)
            {
                return Err(ReferenceError::InvalidDigest);
            }
        } else if selector.is_empty()
            || selector.len() > REFERENCE_CAPACITY
            || !selector
                .iter()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'-'))
        {
            return Err(ReferenceError::InvalidTag);
        }

        let first_end = name.iter().position(|byte| *byte == b'/');
        let explicit_registry = first_end.is_some_and(|end| {
            let first = &name[..end];
            first == b"localhost" || first.contains(&b'.') || first.contains(&b':')
        });
        let (registry, repository) = if explicit_registry {
            let end = first_end.unwrap_or(0);
            (&name[..end], &name[end + 1..])
        } else {
            (b"docker.io" as &[u8], name)
        };
        if !valid_registry(registry) {
            return Err(ReferenceError::InvalidRegistry);
        }
        if !valid_repository(repository) {
            return Err(ReferenceError::InvalidRepository);
        }
        Ok(Self {
            registry,
            repository,
            selector,
            digest,
        })
    }

    pub fn write_canonical(self, output: &mut [u8]) -> Result<usize, ReferenceError> {
        let official = self.registry == b"docker.io" && !self.repository.contains(&b'/');
        let length = self.registry.len()
            + 1
            + usize::from(official) * b"library/".len()
            + self.repository.len()
            + 1
            + self.selector.len();
        let output = output.get_mut(..length).ok_or(ReferenceError::TooLong)?;
        let mut used = copy_part(output, 0, self.registry);
        output[used] = b'/';
        used += 1;
        if official {
            used = copy_part(output, used, b"library/");
        }
        used = copy_part(output, used, self.repository);
        output[used] = if self.digest { b'@' } else { b':' };
        used += 1;
        used = copy_part(output, used, self.selector);
        Ok(used)
    }

    pub fn write_manifest_path(self, output: &mut [u8]) -> Result<usize, ReferenceError> {
        let official = self.registry == b"docker.io" && !self.repository.contains(&b'/');
        let prefix = b"/v2/";
        let suffix = b"/manifests/";
        let length = prefix.len()
            + usize::from(official) * b"library/".len()
            + self.repository.len()
            + suffix.len()
            + self.selector.len();
        let output = output.get_mut(..length).ok_or(ReferenceError::TooLong)?;
        let mut used = copy_part(output, 0, prefix);
        if official {
            used = copy_part(output, used, b"library/");
        }
        used = copy_part(output, used, self.repository);
        used = copy_part(output, used, suffix);
        used = copy_part(output, used, self.selector);
        Ok(used)
    }
}

fn copy_part(output: &mut [u8], offset: usize, input: &[u8]) -> usize {
    output[offset..offset + input.len()].copy_from_slice(input);
    offset + input.len()
}

fn valid_registry(registry: &[u8]) -> bool {
    !registry.is_empty()
        && registry.iter().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'-' | b':')
        })
        && !registry.starts_with(b".")
        && !registry.ends_with(b".")
}

fn valid_repository(repository: &[u8]) -> bool {
    !repository.is_empty()
        && repository.len() <= REFERENCE_CAPACITY
        && repository.split(|byte| *byte == b'/').all(|component| {
            !component.is_empty()
                && component.iter().all(|byte| {
                    byte.is_ascii_lowercase()
                        || byte.is_ascii_digit()
                        || matches!(byte, b'.' | b'_' | b'-')
                })
                && !component.starts_with(b".")
                && !component.ends_with(b".")
        })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EntryKind {
    File,
    Directory,
    Symlink,
    Hardlink,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Entry<'a> {
    pub path: &'a [u8],
    pub prefix: &'a [u8],
    pub data: &'a [u8],
    pub link: &'a [u8],
    pub kind: EntryKind,
    pub mode: u32,
}

pub struct Rootfs<'a> {
    bytes: &'a [u8],
    entry_count: usize,
    content_length: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Child<'a> {
    pub index: usize,
    pub next_cursor: usize,
    pub entry: Entry<'a>,
    pub name: &'a [u8],
}

impl<'a> Rootfs<'a> {
    pub fn parse(bytes: &'a [u8]) -> Option<Self> {
        let mut offset = 0_usize;
        let mut entry_count = 0;
        loop {
            let header = bytes.get(offset..offset.checked_add(TAR_HEADER_SIZE)?)?;
            if header.iter().all(|byte| *byte == 0) {
                let second =
                    bytes.get(offset + TAR_BLOCK_SIZE..offset.checked_add(TAR_BLOCK_SIZE * 2)?)?;
                if !second.iter().all(|byte| *byte == 0)
                    || bytes[offset + TAR_BLOCK_SIZE * 2..]
                        .iter()
                        .any(|byte| *byte != 0)
                {
                    return None;
                }
                return (entry_count != 0).then_some(Self {
                    bytes,
                    entry_count,
                    content_length: offset,
                });
            }
            let (_, next) = parse_entry(bytes, offset)?;
            entry_count = entry_count.checked_add(1)?;
            if entry_count > TAR_MAX_ENTRIES {
                return None;
            }
            offset = next;
        }
    }

    pub const fn entry_count(&self) -> usize {
        self.entry_count
    }

    /// Length through the final archive entry, excluding the two terminating
    /// zero blocks. OCI layer installers use this to join verified tar layers.
    pub const fn content_length(&self) -> usize {
        self.content_length
    }

    pub fn find(&self, path: &[u8]) -> Option<Entry<'a>> {
        self.find_index(path).map(|(_, entry)| entry)
    }

    pub fn entry(&self, wanted: usize) -> Option<Entry<'a>> {
        if wanted >= self.entry_count {
            return None;
        }
        let mut offset = 0;
        for index in 0..self.entry_count {
            let (entry, next) = parse_entry(self.bytes, offset)?;
            if index == wanted {
                return Some(entry);
            }
            offset = next;
        }
        None
    }

    pub fn find_index(&self, path: &[u8]) -> Option<(usize, Entry<'a>)> {
        if !valid_path(path)
            || path
                .rsplit(|byte| *byte == b'/')
                .next()
                .is_some_and(|name| name.starts_with(b".wh."))
        {
            return None;
        }
        let mut offset = 0;
        let mut found = None;
        for index in 0..self.entry_count {
            let (entry, next) = parse_entry(self.bytes, offset)?;
            if whiteout_hides(entry, path) {
                found = None;
            } else if entry_path_matches(entry, path) {
                // OCI layers are applied in manifest order. The last entry for
                // a path is therefore the visible overlay entry.
                found = Some((index, entry));
            }
            offset = next;
        }
        found
    }

    pub fn next_child(&self, parent: &[u8], cursor: usize) -> Option<Option<Child<'a>>> {
        if !parent.is_empty() && !valid_path(parent) || cursor > self.entry_count {
            return None;
        }
        let mut offset = 0;
        for index in 0..self.entry_count {
            let (entry, next) = parse_entry(self.bytes, offset)?;
            offset = next;
            if index < cursor {
                continue;
            }
            let mut full_path = [0_u8; TAR_PREFIX_SIZE + 1 + TAR_NAME_SIZE];
            let length = copy_entry_path(entry, &mut full_path)?;
            let path = &full_path[..length];
            if self
                .find_index(path)
                .is_none_or(|(visible, _)| visible != index)
            {
                continue;
            }
            let remainder = if parent.is_empty() {
                path
            } else {
                let Some(remainder) = path.strip_prefix(parent) else {
                    continue;
                };
                let Some(remainder) = remainder.strip_prefix(b"/") else {
                    continue;
                };
                remainder
            };
            if remainder.is_empty() || remainder.contains(&b'/') {
                continue;
            }
            let name = entry.path.rsplit(|byte| *byte == b'/').next()?;
            return Some(Some(Child {
                index,
                next_cursor: index + 1,
                entry,
                name,
            }));
        }
        Some(None)
    }

    pub fn copy_path(&self, index: usize, output: &mut [u8]) -> Option<usize> {
        copy_entry_path(self.entry(index)?, output)
    }
}

fn whiteout_hides(entry: Entry<'_>, path: &[u8]) -> bool {
    if entry.kind != EntryKind::File || !entry.data.is_empty() {
        return false;
    }
    let mut entry_path = [0_u8; TAR_PREFIX_SIZE + 1 + TAR_NAME_SIZE];
    let Some(length) = copy_entry_path(entry, &mut entry_path) else {
        return false;
    };
    let entry_path = &entry_path[..length];
    let split = entry_path
        .iter()
        .rposition(|byte| *byte == b'/')
        .map_or((b"" as &[u8], entry_path), |index| {
            (&entry_path[..index], &entry_path[index + 1..])
        });
    let (directory, name) = split;
    if name == b".wh..wh..opq" {
        return !directory.is_empty()
            && path
                .strip_prefix(directory)
                .is_some_and(|rest| rest.starts_with(b"/"));
    }
    let Some(target) = name.strip_prefix(b".wh.") else {
        return false;
    };
    let path_split = path
        .iter()
        .rposition(|byte| *byte == b'/')
        .map_or((b"" as &[u8], path), |index| {
            (&path[..index], &path[index + 1..])
        });
    path_split.0 == directory && path_split.1 == target
}

#[cfg(feature = "sha256")]
pub fn verify_sha256(bytes: &[u8], expected: &[u8; 32]) -> bool {
    let actual: [u8; 32] = Sha256::digest(bytes).into();
    actual == *expected
}

fn parse_entry(bytes: &[u8], offset: usize) -> Option<(Entry<'_>, usize)> {
    let header = bytes.get(offset..offset.checked_add(TAR_HEADER_SIZE)?)?;
    if &header[257..263] != b"ustar\0" || checksum(header)? != stored_checksum(header)? {
        return None;
    }
    if header[156] == b'x' {
        let size = usize::try_from(octal(&header[124..136])?).ok()?;
        let data_start = offset.checked_add(TAR_HEADER_SIZE)?;
        let data_end = data_start.checked_add(size)?;
        let overrides = parse_pax_overrides(bytes.get(data_start..data_end)?)?;
        let padded = size.checked_add(TAR_BLOCK_SIZE - 1)? / TAR_BLOCK_SIZE * TAR_BLOCK_SIZE;
        let next = data_start.checked_add(padded)?;
        let (mut entry, next) = parse_entry(bytes, next)?;
        if let Some(path) = overrides.path {
            let path = if entry.kind == EntryKind::Directory {
                path.strip_suffix(b"/").unwrap_or(path)
            } else {
                path
            };
            if path.is_empty() || !valid_entry_path(b"", path) {
                return None;
            }
            entry.path = path;
            entry.prefix = b"";
        }
        if let Some(link) = overrides.link {
            if link.is_empty()
                || entry.kind == EntryKind::Hardlink
                    && !valid_path(link.strip_prefix(b"/").unwrap_or(link))
            {
                return None;
            }
            entry.link = link;
        }
        return Some((entry, next));
    }
    let stored_path = terminated(&header[..TAR_NAME_SIZE])?;
    let prefix = terminated(&header[345..345 + TAR_PREFIX_SIZE])?;
    let size = usize::try_from(octal(&header[124..136])?).ok()?;
    let mode = u32::try_from(octal(&header[100..108])?).ok()?;
    let kind = match header[156] {
        0 | b'0' => EntryKind::File,
        b'5' if size == 0 => EntryKind::Directory,
        b'2' if size == 0 => EntryKind::Symlink,
        b'1' if size == 0 => EntryKind::Hardlink,
        _ => return None,
    };
    let path = if kind == EntryKind::Directory {
        stored_path.strip_suffix(b"/").unwrap_or(stored_path)
    } else {
        stored_path
    };
    if path.is_empty() || !valid_entry_path(prefix, path) {
        return None;
    }
    let link = terminated(&header[157..257])?;
    if matches!(kind, EntryKind::Symlink | EntryKind::Hardlink) && link.is_empty() {
        return None;
    }
    if kind == EntryKind::Hardlink && !valid_path(link.strip_prefix(b"/").unwrap_or(link)) {
        return None;
    }
    let data_start = offset.checked_add(TAR_HEADER_SIZE)?;
    let data_end = data_start.checked_add(size)?;
    let data = bytes.get(data_start..data_end)?;
    let padded = size.checked_add(TAR_BLOCK_SIZE - 1)? / TAR_BLOCK_SIZE * TAR_BLOCK_SIZE;
    let next = data_start.checked_add(padded)?;
    bytes.get(offset..next)?;
    Some((
        Entry {
            path,
            prefix,
            data,
            link,
            kind,
            mode,
        },
        next,
    ))
}

#[derive(Clone, Copy)]
struct PaxOverrides<'a> {
    path: Option<&'a [u8]>,
    link: Option<&'a [u8]>,
}

fn parse_pax_overrides(mut bytes: &[u8]) -> Option<PaxOverrides<'_>> {
    let mut overrides = PaxOverrides {
        path: None,
        link: None,
    };
    while !bytes.is_empty() {
        let separator = bytes.iter().position(|byte| *byte == b' ')?;
        let length = bytes[..separator].iter().try_fold(0_usize, |value, byte| {
            byte.is_ascii_digit()
                .then(|| {
                    value
                        .checked_mul(10)?
                        .checked_add(usize::from(*byte - b'0'))
                })
                .flatten()
        })?;
        let record = bytes.get(..length)?;
        if length <= separator + 2 || record[length - 1] != b'\n' {
            return None;
        }
        let field = &record[separator + 1..length - 1];
        let equals = field.iter().position(|byte| *byte == b'=')?;
        let (key, value) = (&field[..equals], &field[equals + 1..]);
        match key {
            b"path" if overrides.path.is_none() => overrides.path = Some(value),
            b"linkpath" if overrides.link.is_none() => overrides.link = Some(value),
            b"path" | b"linkpath" => return None,
            _ => {}
        }
        bytes = &bytes[length..];
    }
    Some(overrides)
}

fn entry_path_matches(entry: Entry<'_>, wanted: &[u8]) -> bool {
    if entry.prefix.is_empty() {
        entry.path == wanted
    } else {
        wanted.len() == entry.prefix.len() + 1 + entry.path.len()
            && wanted.starts_with(entry.prefix)
            && wanted[entry.prefix.len()] == b'/'
            && &wanted[entry.prefix.len() + 1..] == entry.path
    }
}

fn copy_entry_path(entry: Entry<'_>, output: &mut [u8]) -> Option<usize> {
    let length = entry.prefix.len() + usize::from(!entry.prefix.is_empty()) + entry.path.len();
    let output = output.get_mut(..length)?;
    let mut used = 0;
    if !entry.prefix.is_empty() {
        output[..entry.prefix.len()].copy_from_slice(entry.prefix);
        used = entry.prefix.len();
        output[used] = b'/';
        used += 1;
    }
    output[used..].copy_from_slice(entry.path);
    Some(length)
}

fn terminated(field: &[u8]) -> Option<&[u8]> {
    let end = field
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(field.len());
    field[end..]
        .iter()
        .all(|byte| *byte == 0)
        .then_some(&field[..end])
}

fn octal(field: &[u8]) -> Option<u64> {
    let mut value = 0_u64;
    let mut digit = false;
    let mut ended = false;
    for byte in field {
        match *byte {
            b'0'..=b'7' if !ended => {
                value = value.checked_mul(8)?.checked_add(u64::from(*byte - b'0'))?;
                digit = true;
            }
            0 | b' ' if digit => ended = true,
            b' ' if !digit => {}
            _ => return None,
        }
    }
    digit.then_some(value)
}

fn stored_checksum(header: &[u8]) -> Option<u64> {
    octal(header.get(148..156)?)
}

fn checksum(header: &[u8]) -> Option<u64> {
    (header.len() == TAR_HEADER_SIZE).then(|| {
        header
            .iter()
            .enumerate()
            .map(|(index, byte)| {
                if (148..156).contains(&index) {
                    u64::from(b' ')
                } else {
                    u64::from(*byte)
                }
            })
            .sum()
    })
}

fn valid_entry_path(prefix: &[u8], path: &[u8]) -> bool {
    valid_path(path) && (prefix.is_empty() || valid_path(prefix)) && !prefix.ends_with(b"/")
}

fn valid_path(path: &[u8]) -> bool {
    !path.is_empty()
        && path[0] != b'/'
        && path.iter().all(|byte| byte.is_ascii_graphic())
        && path
            .split(|byte| *byte == b'/')
            .all(|component| !component.is_empty() && component != b"." && component != b"..")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::format;
    use std::vec;
    use std::vec::Vec;

    #[test]
    fn canonicalizes_docker_hub_and_explicit_registry_references() {
        let mut output = [0_u8; REFERENCE_CAPACITY];
        let alpine = ImageReference::parse(b"alpine").unwrap();
        let length = alpine.write_canonical(&mut output).unwrap();
        assert_eq!(&output[..length], b"docker.io/library/alpine:latest");
        let length = alpine.write_manifest_path(&mut output).unwrap();
        assert_eq!(&output[..length], b"/v2/library/alpine/manifests/latest");

        let rocky = ImageReference::parse(b"rockylinux/rockylinux:10").unwrap();
        let length = rocky.write_canonical(&mut output).unwrap();
        assert_eq!(&output[..length], b"docker.io/rockylinux/rockylinux:10");
        let length = rocky.write_manifest_path(&mut output).unwrap();
        assert_eq!(&output[..length], b"/v2/rockylinux/rockylinux/manifests/10");

        let private = ImageReference::parse(b"registry.example:5000/team/image:v1").unwrap();
        let length = private.write_canonical(&mut output).unwrap();
        assert_eq!(&output[..length], b"registry.example:5000/team/image:v1");
    }

    #[test]
    fn accepts_sha256_selectors_and_rejects_ambiguous_references() {
        let digest = b"sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let mut reference = Vec::from(&b"alpine@"[..]);
        reference.extend_from_slice(digest);
        let parsed = ImageReference::parse(&reference).unwrap();
        assert!(parsed.digest);
        assert_eq!(parsed.selector, digest);

        for invalid in [
            b"" as &[u8],
            b"UPPER/image:tag",
            b"docker.io//image:tag",
            b"image:",
            b"image@sha256:abcd",
            b"image@sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdeg",
        ] {
            assert!(ImageReference::parse(invalid).is_err(), "{invalid:?}");
        }
    }

    #[test]
    fn parses_registry_authentication_contract() {
        let challenge = BearerChallenge::parse(
            "Bearer realm=\"https://auth.docker.io/token\",service=\"registry.docker.io\",scope=\"repository:library/alpine:pull\"",
        )
        .unwrap();
        assert_eq!(challenge.realm, "https://auth.docker.io/token");
        assert_eq!(challenge.service, "registry.docker.io");
        assert_eq!(challenge.scope, "repository:library/alpine:pull");
        assert_eq!(
            parse_bearer_token(br#"{"token":"signed-token"}"#),
            Some("signed-token")
        );
        assert_eq!(
            parse_bearer_token(b"{\"access_token\":\"alternate-token\"}\n"),
            Some("alternate-token")
        );
        assert!(BearerChallenge::parse("Basic realm=\"registry\"").is_none());
        assert!(parse_bearer_token(br#"{"token":""}"#).is_none());
    }

    #[test]
    fn selects_required_platform_and_parses_manifest() {
        let digest = "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let index = format!(
            r#"{{"schemaVersion":2,"mediaType":"application/vnd.oci.image.index.v1+json","manifests":[{{"mediaType":"application/vnd.oci.image.manifest.v1+json","digest":"{digest}","size":611,"platform":{{"architecture":"arm64","os":"linux"}}}},{{"mediaType":"application/vnd.oci.image.manifest.v1+json","digest":"{digest}","size":612,"platform":{{"architecture":"amd64","os":"linux"}}}}]}}"#
        );
        let selected = select_manifest(
            index.as_bytes(),
            Platform {
                os: "linux",
                architecture: "amd64",
                variant: "",
            },
        )
        .unwrap();
        assert_eq!(selected.digest, digest);
        assert_eq!(selected.size, 612);

        let manifest = format!(
            r#"{{"schemaVersion":2,"config":{{"mediaType":"application/vnd.oci.image.config.v1+json","digest":"{digest}","size":581}},"layers":[{{"mediaType":"application/vnd.oci.image.layer.v1.tar+gzip","digest":"{digest}","size":3651200}}]}}"#
        );
        let parsed = parse_registry_manifest(manifest.as_bytes()).unwrap();
        assert_eq!(parsed.config.size, 581);
        assert_eq!(parsed.layers.len(), 1);
        assert_eq!(parsed.layers[0].size, 3_651_200);

        assert!(image_configuration_matches_platform(
            br#"{"architecture":"amd64","os":"linux","rootfs":{"type":"layers"}}"#,
            Platform {
                os: "linux",
                architecture: "amd64",
                variant: "",
            },
        ));
    }

    #[test]
    fn registry_parsers_reject_wrong_platform_and_bad_digests() {
        let digest = "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let index = format!(
            r#"{{"schemaVersion":2,"manifests":[{{"mediaType":"manifest","digest":"{digest}","size":1,"platform":{{"architecture":"amd64","os":"windows"}}}}]}}"#
        );
        let amd64 = Platform {
            os: "linux",
            architecture: "amd64",
            variant: "",
        };
        assert!(select_manifest(index.as_bytes(), amd64).is_none());
        assert!(!image_configuration_matches_platform(
            br#"{"architecture":"arm64","os":"linux","variant":"v8"}"#,
            amd64,
        ));
        assert!(image_configuration_matches_platform(
            br#"{"architecture":"arm64","os":"linux"}"#,
            Platform {
                os: "linux",
                architecture: "arm64",
                variant: "v8",
            },
        ));
        assert!(!image_configuration_matches_platform(
            br#"{"architecture":"arm64","os":"linux","variant":"v7"}"#,
            Platform {
                os: "linux",
                architecture: "arm64",
                variant: "v8",
            },
        ));
        assert!(!valid_registry_digest("sha256:abcd"));
    }

    fn archive(path: &[u8], data: &[u8], kind: u8, link: &[u8]) -> Vec<u8> {
        let mut output = vec![0_u8; TAR_BLOCK_SIZE];
        output[..path.len()].copy_from_slice(path);
        output[100..108].copy_from_slice(b"0000755\0");
        output[108..116].copy_from_slice(b"0000000\0");
        output[116..124].copy_from_slice(b"0000000\0");
        let size = format!("{:011o}\0", data.len());
        output[124..136].copy_from_slice(size.as_bytes());
        output[136..148].copy_from_slice(b"00000000000\0");
        output[148..156].fill(b' ');
        output[156] = kind;
        output[157..157 + link.len()].copy_from_slice(link);
        output[257..263].copy_from_slice(b"ustar\0");
        output[263..265].copy_from_slice(b"00");
        let sum: u64 = output.iter().map(|byte| u64::from(*byte)).sum();
        output[148..156].copy_from_slice(format!("{:06o}\0 ", sum).as_bytes());
        output.extend_from_slice(data);
        output.resize(output.len().div_ceil(TAR_BLOCK_SIZE) * TAR_BLOCK_SIZE, 0);
        output.resize(output.len() + TAR_BLOCK_SIZE * 2, 0);
        output
    }

    #[test]
    fn indexes_regular_file_symlink_and_hardlink() {
        let bytes = archive(b"bin/busybox", b"ELF", b'0', b"");
        let root = Rootfs::parse(&bytes).unwrap();
        assert_eq!(root.entry_count(), 1);
        assert_eq!(root.find(b"bin/busybox").unwrap().data, b"ELF");

        let bytes = archive(b"bin/sh", b"", b'2', b"/bin/busybox");
        let root = Rootfs::parse(&bytes).unwrap();
        assert_eq!(root.find(b"bin/sh").unwrap().link, b"/bin/busybox");

        let bytes = archive(b"bin/tool", b"", b'1', b"bin/busybox");
        let root = Rootfs::parse(&bytes).unwrap();
        assert_eq!(root.find(b"bin/tool").unwrap().kind, EntryKind::Hardlink);

        let mut bytes = archive(b"PaxHeaders/tool", b"21 linkpath=bin/real\n", b'x', b"");
        bytes.truncate(bytes.len() - TAR_BLOCK_SIZE * 2);
        bytes.extend_from_slice(&archive(b"bin/tool", b"", b'1', b"truncated"));
        let root = Rootfs::parse(&bytes).unwrap();
        assert_eq!(root.entry_count(), 1);
        assert_eq!(root.find(b"bin/tool").unwrap().link, b"bin/real");
    }

    #[test]
    fn applies_later_layer_replacements_and_whiteouts() {
        let mut bytes = archive(b"etc/value", b"old", b'0', b"");
        bytes.truncate(bytes.len() - TAR_BLOCK_SIZE * 2);
        bytes.extend_from_slice(&archive(b"etc/value", b"new", b'0', b""));
        let root = Rootfs::parse(&bytes).unwrap();
        assert_eq!(root.find(b"etc/value").unwrap().data, b"new");

        let mut bytes = archive(b"etc/value", b"old", b'0', b"");
        bytes.truncate(bytes.len() - TAR_BLOCK_SIZE * 2);
        bytes.extend_from_slice(&archive(b"etc/.wh.value", b"", b'0', b""));
        let root = Rootfs::parse(&bytes).unwrap();
        assert!(root.find(b"etc/value").is_none());
        assert!(root.find(b"etc/.wh.value").is_none());
    }

    #[test]
    fn rejects_traversal_corruption_and_trailing_data() {
        assert!(Rootfs::parse(&archive(b"../escape", b"x", b'0', b"")).is_none());
        let mut corrupt = archive(b"bin/sh", b"x", b'0', b"");
        corrupt[0] ^= 1;
        assert!(Rootfs::parse(&corrupt).is_none());
        let mut trailing = archive(b"bin/sh", b"x", b'0', b"");
        trailing.push(1);
        assert!(Rootfs::parse(&trailing).is_none());
    }
}
