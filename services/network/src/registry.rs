use core::{
    cell::UnsafeCell,
    fmt::Write as _,
    ptr::{read_volatile, write_volatile},
    sync::atomic::{compiler_fence, AtomicU32, AtomicU64, Ordering},
};

use base64ct::{Base64, Encoding};
use embedded_io::{ErrorKind, ErrorType, Read, Write};
use embedded_tls::{
    blocking::TlsConnection, pki::CertVerifier as ChainVerifier, Aes128GcmSha256, Certificate,
    CertificateEntryRef, CertificateRef, CertificateVerifyRef, CryptoProvider, TlsCipherSuite,
    TlsClock, TlsConfig, TlsContext, TlsError, TlsVerifier,
};
use ginux_native_abi::NativeNetworkBootV8;
use ginux_native_abi::{
    NativeImageStoreRequestV0, NATIVE_IMAGE_STORE_ABORT, NATIVE_IMAGE_STORE_OPERATION,
    NATIVE_IMAGE_STORE_PUBLISH, NATIVE_IMAGE_STORE_RESULT_FAILED, NATIVE_PROGRESS_OPERATION,
};
use heapless::{String, Vec};
use p256::ecdsa::DerSignature;
use rand_chacha::ChaCha8Rng;
use rand_core::SeedableRng;
use smoltcp::{
    iface::{Config, Interface, SocketHandle, SocketSet, SocketStorage},
    phy::{Device, DeviceCapabilities, Medium, PacketMeta, RxToken, TxToken},
    socket::{dns, tcp},
    time::Instant,
    wire::{DnsQueryType, EthernetAddress, IpAddress, IpCidr, Ipv4Address},
};

use crate::{
    reset_registry_allocator, RX_AVAIL_OFFSET, RX_BUFFER_OFFSET, RX_DESCRIPTOR_LENGTH,
    RX_DESC_OFFSET, RX_USED_OFFSET, TX_AVAIL_OFFSET, TX_BUFFER_OFFSET, TX_DESC_OFFSET,
    TX_HEADER_LENGTH, TX_USED_OFFSET,
};

const QUEUE_SIZE: u16 = 8;
const TCP_BUFFER_SIZE: usize = 16 * 1024;
const NETWORK_POLL_LIMIT: usize = 25_000_000;
const POLLS_PER_MILLISECOND: usize = 1024;
const TLS_RECORD_SIZE: usize = 16_640;
const CERTIFICATE_SIZE: usize = 4096;
const HTTP_HEADER_SIZE: usize = 8192;
const METADATA_BODY_SIZE: usize = 64 * 1024;

struct RegistryServiceOwner {
    clock: AtomicU64,
    progress_capability: AtomicU64,
    dns_fallback_a: AtomicU32,
    dns_fallback_state: AtomicU32,
    dns_query_sent: AtomicU32,
    tcp_rx: UnsafeCell<[u8; TCP_BUFFER_SIZE]>,
    tcp_tx: UnsafeCell<[u8; TCP_BUFFER_SIZE]>,
    dns_queries: UnsafeCell<[Option<dns::DnsQuery>; 1]>,
    socket_storage: UnsafeCell<[SocketStorage<'static>; 2]>,
    tls_read: UnsafeCell<[u8; TLS_RECORD_SIZE]>,
    tls_write: UnsafeCell<[u8; TLS_RECORD_SIZE]>,
    root_der: UnsafeCell<[u8; 2048]>,
    http_header: UnsafeCell<[u8; HTTP_HEADER_SIZE]>,
    metadata_body: UnsafeCell<[u8; METADATA_BODY_SIZE]>,
}

unsafe impl Sync for RegistryServiceOwner {}

static REGISTRY_SERVICE: RegistryServiceOwner = RegistryServiceOwner {
    clock: AtomicU64::new(0),
    progress_capability: AtomicU64::new(0),
    dns_fallback_a: AtomicU32::new(0),
    dns_fallback_state: AtomicU32::new(0),
    dns_query_sent: AtomicU32::new(0),
    tcp_rx: UnsafeCell::new([0; TCP_BUFFER_SIZE]),
    tcp_tx: UnsafeCell::new([0; TCP_BUFFER_SIZE]),
    dns_queries: UnsafeCell::new([None]),
    socket_storage: UnsafeCell::new([SocketStorage::EMPTY; 2]),
    tls_read: UnsafeCell::new([0; TLS_RECORD_SIZE]),
    tls_write: UnsafeCell::new([0; TLS_RECORD_SIZE]),
    root_der: UnsafeCell::new([0; 2048]),
    http_header: UnsafeCell::new([0; HTTP_HEADER_SIZE]),
    metadata_body: UnsafeCell::new([0; METADATA_BODY_SIZE]),
};

#[inline(never)]
pub(crate) unsafe fn pull(info: &NativeNetworkBootV8) -> bool {
    REGISTRY_SERVICE
        .clock
        .store(info.unix_time, Ordering::Release);
    progress(b"initializing registry network stack");
    let Some(mut device) = VirtioNet::new(info) else {
        progress(b"registry network device initialization failed");
        return false;
    };
    let mac = info.mac_address.to_be_bytes();
    let mut config = Config::new(EthernetAddress(mac[2..].try_into().unwrap()).into());
    config.random_seed = info.unix_time ^ u64::from(info.ipv4);
    let mut interface = Interface::new(config, &mut device, Instant::from_millis(0));
    let ip = info.ipv4.to_be_bytes();
    interface.update_ip_addrs(|addresses| {
        let _ = addresses.push(IpCidr::new(
            IpAddress::Ipv4(Ipv4Address::from_octets(ip)),
            info.prefix,
        ));
    });
    let gateway = info.gateway.to_be_bytes();
    if interface
        .routes_mut()
        .add_default_ipv4_route(Ipv4Address::from_octets(gateway))
        .is_err()
    {
        progress(b"registry default route installation failed");
        return false;
    }
    progress(b"registry network stack ready");
    let dns_address = info.dns.to_be_bytes();
    let dns_servers = [IpAddress::Ipv4(Ipv4Address::from_octets(dns_address))];
    let dns_socket = dns::Socket::new(
        &dns_servers,
        core::slice::from_raw_parts_mut(REGISTRY_SERVICE.dns_queries.get().cast(), 1),
    );
    let tcp_rx = tcp::SocketBuffer::new(core::slice::from_raw_parts_mut(
        REGISTRY_SERVICE.tcp_rx.get().cast(),
        TCP_BUFFER_SIZE,
    ));
    let tcp_tx = tcp::SocketBuffer::new(core::slice::from_raw_parts_mut(
        REGISTRY_SERVICE.tcp_tx.get().cast(),
        TCP_BUFFER_SIZE,
    ));
    let tcp_socket = tcp::Socket::new(tcp_rx, tcp_tx);
    let mut sockets = SocketSet::new(core::slice::from_raw_parts_mut(
        REGISTRY_SERVICE.socket_storage.get().cast(),
        2,
    ));
    let dns_handle = sockets.add(dns_socket);
    let tcp_handle = sockets.add(tcp_socket);

    let reference = &info.reference[..usize::from(info.reference_size)];
    let Ok(reference) = ginux_oci::ImageReference::parse(reference) else {
        return false;
    };
    let host = if reference.registry == b"docker.io" {
        "registry-1.docker.io"
    } else {
        let Ok(host) = core::str::from_utf8(reference.registry) else {
            return false;
        };
        host
    };
    let path = &mut *REGISTRY_PULL.path.get();
    path.clear();
    let mut path_bytes = [0; 384];
    let Ok(path_length) = reference.write_manifest_path(&mut path_bytes) else {
        return false;
    };
    let Ok(path_text) = core::str::from_utf8(&path_bytes[..path_length]) else {
        return false;
    };
    if path.push_str(path_text).is_err() {
        return false;
    }
    let response = &mut *REGISTRY_PULL.response.get();
    *response = HttpMetadata::empty();
    progress(b"connecting to registry");
    if !https_get(
        host,
        path.as_str(),
        None,
        &mut interface,
        &mut device,
        &mut sockets,
        dns_handle,
        tcp_handle,
        response,
        &mut DiscardSink,
    ) {
        return false;
    }
    sockets.get_mut::<tcp::Socket>(tcp_handle).abort();
    progress(b"registry endpoint reached");
    let token = &mut *REGISTRY_PULL.token.get();
    token.clear();
    if response.status == 401 {
        progress(b"authenticating with registry");
        let Some(challenge) = ginux_oci::BearerChallenge::parse(response.www_authenticate.as_str())
        else {
            return false;
        };
        let Ok(realm) = String::<512>::try_from(challenge.realm) else {
            return false;
        };
        let Ok(service) = String::<256>::try_from(challenge.service) else {
            return false;
        };
        let Ok(scope) = String::<512>::try_from(challenge.scope) else {
            return false;
        };
        let Some((auth_host, auth_path)) = split_https_url(realm.as_str()) else {
            return false;
        };
        let token_path = &mut *REGISTRY_PULL.token_path.get();
        token_path.clear();
        if write!(
            token_path,
            "{auth_path}?service={}&scope={}",
            service, scope
        )
        .is_err()
        {
            return false;
        }
        {
            let mut token_body = unsafe { BufferSink::metadata() };
            if !unsafe {
                https_get(
                    auth_host,
                    token_path.as_str(),
                    None,
                    &mut interface,
                    &mut device,
                    &mut sockets,
                    dns_handle,
                    tcp_handle,
                    response,
                    &mut token_body,
                )
            } || response.status != 200
            {
                progress_status(b"registry token request returned HTTP ", response.status);
                return false;
            }
            sockets.get_mut::<tcp::Socket>(tcp_handle).abort();
            let Some(value) = ginux_oci::parse_bearer_token(token_body.bytes()) else {
                return false;
            };
            if token.push_str(value).is_err() {
                return false;
            }
        }
        progress(b"registry token accepted");
    } else if response.status != 200 {
        return false;
    }

    let mut manifest_body = unsafe { BufferSink::metadata() };
    progress(b"pulling image manifest");
    if !unsafe {
        https_get(
            host,
            path.as_str(),
            (!token.is_empty()).then_some(token.as_str()),
            &mut interface,
            &mut device,
            &mut sockets,
            dns_handle,
            tcp_handle,
            response,
            &mut manifest_body,
        )
    } || response.status != 200
    {
        progress_status(b"registry manifest request returned HTTP ", response.status);
        return false;
    }
    sockets.get_mut::<tcp::Socket>(tcp_handle).abort();
    let platform = ginux_oci::Platform::native_linux();
    if let Some(selected) = ginux_oci::select_manifest(manifest_body.bytes(), platform) {
        let Some(selected_digest) = parse_digest(selected.digest) else {
            return false;
        };
        let selected_size = selected.size;
        path.clear();
        if write_manifest_path(reference, selected.digest, path).is_none() {
            return false;
        }
        manifest_body.clear();
        if !unsafe {
            https_get(
                host,
                path.as_str(),
                (!token.is_empty()).then_some(token.as_str()),
                &mut interface,
                &mut device,
                &mut sockets,
                dns_handle,
                tcp_handle,
                response,
                &mut manifest_body,
            )
        } || response.status != 200
        {
            progress_status(
                b"registry selected manifest returned HTTP ",
                response.status,
            );
            return false;
        }
        sockets.get_mut::<tcp::Socket>(tcp_handle).abort();
        if manifest_body.bytes().len() != selected_size
            || !ginux_oci::verify_sha256(manifest_body.bytes(), &selected_digest)
        {
            progress(b"selected manifest digest or size rejected");
            return false;
        }
    }
    let (config_digest_text, config_size, layers) = {
        let Some(manifest) = ginux_oci::parse_registry_manifest(manifest_body.bytes()) else {
            progress(b"registry manifest format rejected");
            return false;
        };
        let Ok(config_digest) = String::<72>::try_from(manifest.config.digest) else {
            return false;
        };
        let mut layers = Vec::<(String<72>, usize), { ginux_oci::OCI_LAYER_LIMIT }>::new();
        for layer in &manifest.layers {
            let Ok(digest) = String::<72>::try_from(layer.digest) else {
                return false;
            };
            if layers.push((digest, layer.size)).is_err() {
                return false;
            }
        }
        (config_digest, manifest.config.size, layers)
    };
    let Some(config_digest) = parse_digest(config_digest_text.as_str()) else {
        return false;
    };
    let mut config_path = String::<384>::new();
    if write_blob_path(reference, config_digest_text.as_str(), &mut config_path).is_none() {
        return false;
    }
    manifest_body.clear();
    progress(b"pulling image configuration");
    if !unsafe {
        https_get(
            host,
            config_path.as_str(),
            (!token.is_empty()).then_some(token.as_str()),
            &mut interface,
            &mut device,
            &mut sockets,
            dns_handle,
            tcp_handle,
            response,
            &mut manifest_body,
        )
    } {
        progress(b"registry image configuration transfer failed");
        return false;
    }
    sockets.get_mut::<tcp::Socket>(tcp_handle).abort();
    if matches!(response.status, 302 | 307) {
        let Ok(location) = String::<1536>::try_from(response.location.as_str()) else {
            return false;
        };
        let Some((redirect_host, redirect_path)) = split_https_url(location.as_str()) else {
            return false;
        };
        manifest_body.clear();
        if !unsafe {
            https_get(
                redirect_host,
                redirect_path,
                None,
                &mut interface,
                &mut device,
                &mut sockets,
                dns_handle,
                tcp_handle,
                response,
                &mut manifest_body,
            )
        } {
            return false;
        }
        sockets.get_mut::<tcp::Socket>(tcp_handle).abort();
    }
    if response.status != 200
        || manifest_body.bytes().len() != config_size
        || !ginux_oci::verify_sha256(manifest_body.bytes(), &config_digest)
        || !ginux_oci::image_configuration_matches_platform(manifest_body.bytes(), platform)
    {
        progress(b"image configuration platform, digest, or size rejected");
        return false;
    }
    progress_platform(platform);
    progress(b"image configuration verified");
    let layer_count = layers.len();
    for (layer_index, (layer_digest, layer_size)) in layers.iter().enumerate() {
        let Some(digest) = parse_digest(layer_digest.as_str()) else {
            return false;
        };
        let mut blob_path = String::<384>::new();
        if write_blob_path(reference, layer_digest.as_str(), &mut blob_path).is_none() {
            return false;
        }
        let Some(mut layer_sink) = StoreSink::new(info.image_store_capability, digest, *layer_size)
        else {
            return false;
        };
        progress_layer(
            "pulling",
            layer_index + 1,
            layer_count,
            layer_digest.as_str(),
            *layer_size,
        );
        let fetched = unsafe {
            https_get(
                host,
                blob_path.as_str(),
                (!token.is_empty()).then_some(token.as_str()),
                &mut interface,
                &mut device,
                &mut sockets,
                dns_handle,
                tcp_handle,
                response,
                &mut layer_sink,
            )
        };
        if !fetched {
            progress(b"registry layer transfer failed");
            return false;
        }
        sockets.get_mut::<tcp::Socket>(tcp_handle).abort();
        if matches!(response.status, 302 | 307) {
            let Ok(location) = String::<1536>::try_from(response.location.as_str()) else {
                progress(b"registry layer redirect exceeded limit");
                return false;
            };
            let Some((redirect_host, redirect_path)) = split_https_url(location.as_str()) else {
                progress(b"registry layer redirect was not HTTPS");
                return false;
            };
            progress(b"following verified image-layer redirect");
            if !unsafe {
                https_get(
                    redirect_host,
                    redirect_path,
                    None,
                    &mut interface,
                    &mut device,
                    &mut sockets,
                    dns_handle,
                    tcp_handle,
                    response,
                    &mut layer_sink,
                )
            } {
                progress(b"registry redirected layer transfer failed");
                return false;
            }
            sockets.get_mut::<tcp::Socket>(tcp_handle).abort();
        }
        if response.status != 200 {
            progress_status(b"registry layer request returned HTTP ", response.status);
            return false;
        }
        if !layer_sink.publish() {
            progress(b"registry layer digest or publication rejected");
            return false;
        }
        progress_layer(
            "verified",
            layer_index + 1,
            layer_count,
            layer_digest.as_str(),
            *layer_size,
        );
    }
    true
}

pub(crate) fn progress(message: &[u8]) {
    let mut result = NATIVE_PROGRESS_OPERATION;
    unsafe {
        core::arch::asm!(
            "int 0x80",
            inlateout("rax") result,
            in("rdi") REGISTRY_SERVICE.progress_capability.load(Ordering::Acquire),
            in("rsi") message.as_ptr() as u64,
            in("rdx") message.len() as u64,
            options(nostack),
        );
    }
    let _ = result;
}

pub(crate) fn set_progress_capability(capability: u64) {
    REGISTRY_SERVICE
        .progress_capability
        .store(capability, Ordering::Release);
}

fn progress_status(prefix: &[u8], status: u16) {
    let mut message = String::<96>::new();
    if let Ok(prefix) = core::str::from_utf8(prefix) {
        let _ = message.push_str(prefix);
        let _ = write!(message, "{status}");
        progress(message.as_bytes());
    }
}

fn progress_platform(platform: ginux_oci::Platform<'_>) {
    let mut message = String::<96>::new();
    let _ = write!(
        message,
        "manifest selected for {}/{}",
        platform.os, platform.architecture
    );
    if !platform.variant.is_empty() {
        let _ = write!(message, "/{}", platform.variant);
    }
    progress(message.as_bytes());
}

fn progress_layer(stage: &str, index: usize, count: usize, digest: &str, size: usize) {
    let mut message = String::<128>::new();
    let _ = write!(
        message,
        "layer {index}/{count}: {stage} {digest} ({size} bytes)"
    );
    progress(message.as_bytes());
}

fn split_https_url(url: &str) -> Option<(&str, &str)> {
    let remainder = url.strip_prefix("https://")?;
    let slash = remainder.find('/').unwrap_or(remainder.len());
    let host = &remainder[..slash];
    let path = remainder
        .get(slash..)
        .filter(|path| !path.is_empty())
        .unwrap_or("/");
    (!host.is_empty() && !host.contains(':')).then_some((host, path))
}

fn repository<'a>(
    reference: ginux_oci::ImageReference<'_>,
    output: &'a mut String<256>,
) -> Option<&'a str> {
    if reference.registry == b"docker.io" && !reference.repository.contains(&b'/') {
        output.push_str("library/").ok()?;
    }
    output
        .push_str(core::str::from_utf8(reference.repository).ok()?)
        .ok()?;
    Some(output.as_str())
}

fn write_manifest_path(
    reference: ginux_oci::ImageReference<'_>,
    selector: &str,
    output: &mut String<384>,
) -> Option<()> {
    let mut repository_name = String::<256>::new();
    let repository = repository(reference, &mut repository_name)?;
    write!(output, "/v2/{repository}/manifests/{selector}").ok()
}

fn write_blob_path(
    reference: ginux_oci::ImageReference<'_>,
    digest: &str,
    output: &mut String<384>,
) -> Option<()> {
    let mut repository_name = String::<256>::new();
    let repository = repository(reference, &mut repository_name)?;
    write!(output, "/v2/{repository}/blobs/{digest}").ok()
}

fn parse_digest(value: &str) -> Option<[u8; 32]> {
    let hex = value.strip_prefix("sha256:")?.as_bytes();
    if hex.len() != 64 {
        return None;
    }
    let mut digest = [0; 32];
    for (index, output) in digest.iter_mut().enumerate() {
        let pair = &hex[index * 2..index * 2 + 2];
        *output = hex_nibble(pair[0])?
            .checked_mul(16)?
            .checked_add(hex_nibble(pair[1])?)?;
    }
    Some(digest)
}

fn hex_nibble(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

struct RegistryClock;

impl TlsClock for RegistryClock {
    fn now() -> Option<u64> {
        Some(REGISTRY_SERVICE.clock.load(Ordering::Acquire))
    }
}

struct StrictVerifier<'a> {
    inner: ChainVerifier<'a, Aes128GcmSha256, RegistryClock, CERTIFICATE_SIZE>,
    hostname: String<64>,
}

impl<'a> StrictVerifier<'a> {
    fn new(ca: Certificate<&'a [u8]>) -> Self {
        Self {
            inner: ChainVerifier::new(ca),
            hostname: String::new(),
        }
    }
}

impl TlsVerifier<Aes128GcmSha256> for StrictVerifier<'_> {
    fn set_hostname_verification(&mut self, hostname: &str) -> Result<(), TlsError> {
        self.hostname.clear();
        self.hostname
            .push_str(hostname)
            .map_err(|_| TlsError::InsufficientSpace)?;

        // embedded-tls 0.19 retains only the first three SANs. Docker's current
        // certificate places *.docker.io later in the same signed SAN extension,
        // so use an early SAN for the library's redundant hostname check. The
        // complete SAN extension is checked independently below before the chain,
        // time, and signature checks are delegated to embedded-tls.
        let delegated = if hostname == "registry-1.docker.io" {
            "registry.hub.docker.com"
        } else {
            hostname
        };
        self.inner.set_hostname_verification(delegated)
    }

    fn verify_certificate(
        &mut self,
        transcript: &<Aes128GcmSha256 as TlsCipherSuite>::Hash,
        certificate: CertificateRef,
    ) -> Result<(), TlsError> {
        let Some(CertificateEntryRef::X509(leaf)) = certificate.entries.first() else {
            return Err(TlsError::InvalidCertificate);
        };
        if !certificate_has_matching_san(leaf, self.hostname.as_str()) {
            return Err(TlsError::InvalidCertificate);
        }
        self.inner.verify_certificate(transcript, certificate)
    }

    fn verify_signature(&mut self, verify: CertificateVerifyRef) -> Result<(), TlsError> {
        self.inner.verify_signature(verify)
    }
}

struct DerReader<'a> {
    bytes: &'a [u8],
    cursor: usize,
}

impl<'a> DerReader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, cursor: 0 }
    }

    fn next(&mut self) -> Option<(u8, &'a [u8])> {
        let tag = *self.bytes.get(self.cursor)?;
        self.cursor += 1;
        let first = *self.bytes.get(self.cursor)?;
        self.cursor += 1;
        let length = if first & 0x80 == 0 {
            usize::from(first)
        } else {
            let count = usize::from(first & 0x7f);
            if count == 0 || count > core::mem::size_of::<usize>() {
                return None;
            }
            let mut length = 0usize;
            for _ in 0..count {
                length = length
                    .checked_mul(256)?
                    .checked_add(usize::from(*self.bytes.get(self.cursor)?))?;
                self.cursor += 1;
            }
            length
        };
        let end = self.cursor.checked_add(length)?;
        let value = self.bytes.get(self.cursor..end)?;
        self.cursor = end;
        Some((tag, value))
    }

    fn required(&mut self, expected: u8) -> Option<&'a [u8]> {
        let (tag, value) = self.next()?;
        (tag == expected).then_some(value)
    }
}

fn certificate_has_matching_san(certificate: &[u8], hostname: &str) -> bool {
    let mut certificate_reader = DerReader::new(certificate);
    let Some(certificate_sequence) = certificate_reader.required(0x30) else {
        return false;
    };
    let mut certificate_sequence = DerReader::new(certificate_sequence);
    let Some(tbs) = certificate_sequence.required(0x30) else {
        return false;
    };
    let mut tbs = DerReader::new(tbs);

    let Some((first_tag, _)) = tbs.next() else {
        return false;
    };
    if first_tag != 0xa0 && first_tag != 0x02 {
        return false;
    }
    if first_tag == 0xa0 && tbs.required(0x02).is_none() {
        return false;
    }
    for expected in [0x30, 0x30, 0x30, 0x30, 0x30] {
        if tbs.required(expected).is_none() {
            return false;
        }
    }

    while let Some((tag, value)) = tbs.next() {
        if tag != 0xa3 {
            continue;
        }
        let mut explicit = DerReader::new(value);
        let Some(extensions) = explicit.required(0x30) else {
            return false;
        };
        let mut extensions = DerReader::new(extensions);
        while let Some((tag, extension)) = extensions.next() {
            if tag != 0x30 {
                return false;
            }
            let mut extension = DerReader::new(extension);
            let Some(oid) = extension.required(0x06) else {
                return false;
            };
            let Some((tag, value)) = extension.next() else {
                return false;
            };
            let value = if tag == 0x01 {
                let Some((tag, value)) = extension.next() else {
                    return false;
                };
                if tag != 0x04 {
                    return false;
                }
                value
            } else if tag == 0x04 {
                value
            } else {
                return false;
            };
            if oid != [0x55, 0x1d, 0x11] {
                continue;
            }
            let mut general_names = DerReader::new(value);
            let Some(general_names) = general_names.required(0x30) else {
                return false;
            };
            let mut general_names = DerReader::new(general_names);
            while let Some((tag, name)) = general_names.next() {
                if tag == 0x82 && dns_name_matches(name, hostname.as_bytes()) {
                    return true;
                }
            }
            return false;
        }
        return false;
    }
    false
}

fn dns_name_matches(pattern: &[u8], hostname: &[u8]) -> bool {
    if pattern.len() == hostname.len()
        && pattern
            .iter()
            .zip(hostname)
            .all(|(left, right)| left.eq_ignore_ascii_case(right))
    {
        return true;
    }
    let Some(suffix) = pattern.strip_prefix(b"*.") else {
        return false;
    };
    if hostname.len() <= suffix.len() || hostname[hostname.len() - suffix.len()..] != *suffix {
        return false;
    }
    let prefix = &hostname[..hostname.len() - suffix.len()];
    prefix.ends_with(b".") && prefix.len() > 1 && !prefix[..prefix.len() - 1].contains(&b'.')
}

struct Provider<'a> {
    rng: ChaCha8Rng,
    verifier: StrictVerifier<'a>,
}

impl CryptoProvider for Provider<'_> {
    type CipherSuite = Aes128GcmSha256;
    type Signature = DerSignature;

    fn rng(&mut self) -> impl embedded_tls::CryptoRngCore {
        &mut self.rng
    }

    fn verifier(&mut self) -> Result<&mut impl TlsVerifier<Self::CipherSuite>, TlsError> {
        Ok(&mut self.verifier)
    }
}

trait BodySink {
    fn write(&mut self, bytes: &[u8]) -> bool;
}

struct DiscardSink;

impl BodySink for DiscardSink {
    fn write(&mut self, _bytes: &[u8]) -> bool {
        true
    }
}

struct BufferSink {
    bytes: &'static mut [u8],
    used: usize,
}

impl BufferSink {
    unsafe fn metadata() -> Self {
        Self {
            bytes: core::slice::from_raw_parts_mut(
                REGISTRY_SERVICE.metadata_body.get().cast(),
                METADATA_BODY_SIZE,
            ),
            used: 0,
        }
    }

    fn bytes(&self) -> &[u8] {
        &self.bytes[..self.used]
    }

    fn clear(&mut self) {
        self.used = 0;
    }
}

impl BodySink for BufferSink {
    fn write(&mut self, bytes: &[u8]) -> bool {
        let Some(end) = self.used.checked_add(bytes.len()) else {
            return false;
        };
        let Some(output) = self.bytes.get_mut(self.used..end) else {
            return false;
        };
        output.copy_from_slice(bytes);
        self.used = end;
        true
    }
}

struct StoreSink {
    capability: u64,
    handle: u64,
    failed: bool,
}

impl StoreSink {
    fn new(capability: u64, digest: [u8; 32], length: usize) -> Option<Self> {
        let request = NativeImageStoreRequestV0::stage(length as u64, digest);
        let handle = image_store_call(capability, &request);
        (handle != NATIVE_IMAGE_STORE_RESULT_FAILED).then_some(Self {
            capability,
            handle,
            failed: false,
        })
    }

    fn publish(&mut self) -> bool {
        if self.failed {
            return false;
        }
        let request = NativeImageStoreRequestV0::finish(NATIVE_IMAGE_STORE_PUBLISH, self.handle);
        let published = image_store_call(self.capability, &request) == 0;
        self.handle = 0;
        published
    }
}

impl BodySink for StoreSink {
    fn write(&mut self, bytes: &[u8]) -> bool {
        for chunk in bytes.chunks(ginux_native_abi::NATIVE_IMAGE_STORE_CHUNK_LIMIT as usize) {
            let request = NativeImageStoreRequestV0::append(
                self.handle,
                chunk.as_ptr() as u64,
                chunk.len() as u32,
            );
            if image_store_call(self.capability, &request) == NATIVE_IMAGE_STORE_RESULT_FAILED {
                self.failed = true;
                return false;
            }
        }
        true
    }
}

impl Drop for StoreSink {
    fn drop(&mut self) {
        if self.handle != 0 {
            let request = NativeImageStoreRequestV0::finish(NATIVE_IMAGE_STORE_ABORT, self.handle);
            let _ = image_store_call(self.capability, &request);
        }
    }
}

fn image_store_call(capability: u64, request: &NativeImageStoreRequestV0) -> u64 {
    let mut result = NATIVE_IMAGE_STORE_OPERATION;
    unsafe {
        core::arch::asm!(
            "int 0x80",
            inlateout("rax") result,
            in("rdi") capability,
            in("rsi") request as *const NativeImageStoreRequestV0 as u64,
            options(nostack),
        );
    }
    result
}

struct HttpMetadata {
    status: u16,
    content_length: Option<usize>,
    chunked: bool,
    www_authenticate: String<1024>,
    location: String<1536>,
}

impl HttpMetadata {
    const fn empty() -> Self {
        Self {
            status: 0,
            content_length: None,
            chunked: false,
            www_authenticate: String::new(),
            location: String::new(),
        }
    }
}

struct RegistryPullOwner {
    path: UnsafeCell<String<384>>,
    response: UnsafeCell<HttpMetadata>,
    token: UnsafeCell<String<4096>>,
    token_path: UnsafeCell<String<1536>>,
}

unsafe impl Sync for RegistryPullOwner {}

static REGISTRY_PULL: RegistryPullOwner = RegistryPullOwner {
    path: UnsafeCell::new(String::new()),
    response: UnsafeCell::new(HttpMetadata::empty()),
    token: UnsafeCell::new(String::new()),
    token_path: UnsafeCell::new(String::new()),
};

#[allow(clippy::too_many_arguments)]
#[inline(never)]
unsafe fn https_get(
    host: &str,
    path: &str,
    authorization: Option<&str>,
    interface: &mut Interface,
    device: &mut VirtioNet,
    sockets: &mut SocketSet<'static>,
    dns_handle: SocketHandle,
    tcp_handle: SocketHandle,
    metadata: &mut HttpMetadata,
    sink: &mut impl BodySink,
) -> bool {
    reset_registry_allocator();
    let Some(address) = resolve(host, interface, device, sockets, dns_handle) else {
        progress(b"registry DNS resolution failed");
        return false;
    };
    progress(b"registry DNS resolved");
    let Some(stream) = TcpStream::connect(address, 443, interface, device, sockets, tcp_handle)
    else {
        progress(b"registry TCP connection failed");
        return false;
    };
    progress(b"registry TCP connected");
    let root = if host == "auth.docker.io" {
        include_str!("../certs/isrg-root-x1.pem")
    } else {
        include_str!("../certs/starfield-services-root-g2.pem")
    };
    let Some(root) = decode_pem(root) else {
        progress(b"registry trust anchor rejected");
        return false;
    };
    let tls_read =
        core::slice::from_raw_parts_mut(REGISTRY_SERVICE.tls_read.get().cast(), TLS_RECORD_SIZE);
    let tls_write =
        core::slice::from_raw_parts_mut(REGISTRY_SERVICE.tls_write.get().cast(), TLS_RECORD_SIZE);
    let mut tls = TlsConnection::new(stream, tls_read, tls_write);
    let config = TlsConfig::new()
        .with_server_name(host)
        .enable_rsa_signatures();
    let provider = Provider {
        rng: ChaCha8Rng::from_seed(seed()),
        verifier: StrictVerifier::new(Certificate::X509(root)),
    };
    if let Err(error) = tls.open(TlsContext::new(&config, provider)) {
        progress(match error {
            TlsError::InvalidCertificate => b"registry TLS certificate chain or name rejected",
            TlsError::InvalidCertificateEntry | TlsError::ParseError(_) => {
                b"registry TLS certificate could not be decoded"
            }
            TlsError::InvalidSignature => b"registry TLS handshake signature rejected",
            TlsError::InvalidSignatureScheme => b"registry TLS signature scheme rejected",
            TlsError::InvalidCipherSuite => b"registry TLS cipher suite rejected",
            TlsError::InvalidKeyShare => b"registry TLS key exchange rejected",
            TlsError::InvalidHandshake | TlsError::MissingHandshake => {
                b"registry TLS handshake message rejected"
            }
            TlsError::InvalidRecord | TlsError::UnknownContentType => {
                b"registry TLS record rejected"
            }
            TlsError::DecodeError => b"registry TLS message decode failed",
            TlsError::EncodeError => b"registry TLS message encode failed",
            TlsError::InternalError | TlsError::UnableToInitializeCryptoEngine => {
                b"registry TLS internal crypto initialization failed"
            }
            TlsError::Unimplemented => b"registry TLS peer requires unsupported behavior",
            TlsError::CryptoError => b"registry TLS cryptographic operation failed",
            TlsError::InsufficientSpace | TlsError::OutOfMemory => {
                b"registry TLS workspace exhausted"
            }
            TlsError::Io(_) | TlsError::IoError | TlsError::ConnectionClosed => {
                b"registry TLS transport failed"
            }
            TlsError::HandshakeAborted(_, _) | TlsError::AbortHandshake(_, _) => {
                b"registry TLS peer aborted handshake"
            }
            _ => b"registry TLS protocol negotiation failed",
        });
        return false;
    }
    progress(b"registry TLS established");
    let mut request = String::<6144>::new();
    if write!(
        request,
        "GET {path} HTTP/1.1\r\nHost: {host}\r\nUser-Agent: ginux/0.1\r\nAccept: application/vnd.oci.image.index.v1+json, application/vnd.docker.distribution.manifest.list.v2+json, application/vnd.oci.image.manifest.v1+json, application/vnd.docker.distribution.manifest.v2+json\r\nConnection: close\r\n"
    )
    .is_err()
    {
        return false;
    }
    if let Some(authorization) = authorization {
        if write!(request, "Authorization: Bearer {authorization}\r\n").is_err() {
            progress(b"registry authorization header exceeded request limit");
            return false;
        }
    }
    if request.push_str("\r\n").is_err() || tls.write_all(request.as_bytes()).is_err() {
        progress(b"registry HTTP request failed");
        return false;
    }
    if tls.flush().is_err() || !read_response(&mut tls, metadata, sink) {
        progress(b"registry HTTP response failed");
        return false;
    }
    let mut stream = match tls.close() {
        Ok(stream) | Err((stream, _)) => stream,
    };
    stream.abort();
    true
}

unsafe fn decode_pem(pem: &str) -> Option<&'static [u8]> {
    let mut encoded = String::<2048>::new();
    for line in pem.lines() {
        if !line.starts_with("-----") {
            encoded.push_str(line).ok()?;
        }
    }
    let output = core::slice::from_raw_parts_mut(REGISTRY_SERVICE.root_der.get().cast(), 2048);
    let decoded = Base64::decode(encoded.as_bytes(), output).ok()?;
    Some(core::slice::from_raw_parts(decoded.as_ptr(), decoded.len()))
}

fn seed() -> [u8; 32] {
    let mut output = [0; 32];
    let time = REGISTRY_SERVICE.clock.load(Ordering::Acquire);
    for index in 0..4 {
        output[index * 8..index * 8 + 8]
            .copy_from_slice(&time.wrapping_mul((index + 1) as u64).to_le_bytes());
    }
    output
}

fn read_response(
    stream: &mut impl Read,
    metadata: &mut HttpMetadata,
    sink: &mut impl BodySink,
) -> bool {
    let header = unsafe {
        core::slice::from_raw_parts_mut(REGISTRY_SERVICE.http_header.get().cast(), HTTP_HEADER_SIZE)
    };
    let mut used = 0;
    let header_end = loop {
        if used == header.len() {
            return false;
        }
        let Ok(count) = stream.read(&mut header[used..]) else {
            return false;
        };
        if count == 0 {
            return false;
        }
        used += count;
        if let Some(end) = header[..used]
            .windows(4)
            .position(|bytes| bytes == b"\r\n\r\n")
        {
            break end + 4;
        }
    };
    let Ok(text) = core::str::from_utf8(&header[..header_end]) else {
        return false;
    };
    let mut lines = text.split("\r\n");
    let Some(status) = lines.next().and_then(|line| line.split_whitespace().nth(1)) else {
        return false;
    };
    let Ok(status) = status.parse::<u16>() else {
        return false;
    };
    *metadata = HttpMetadata::empty();
    metadata.status = status;
    for line in lines {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        let value = value.trim();
        if name.eq_ignore_ascii_case("content-length") {
            metadata.content_length = value.parse().ok();
        } else if name.eq_ignore_ascii_case("transfer-encoding") {
            metadata.chunked = value.eq_ignore_ascii_case("chunked");
        } else if name.eq_ignore_ascii_case("www-authenticate") {
            if metadata.www_authenticate.push_str(value).is_err() {
                return false;
            }
        } else if name.eq_ignore_ascii_case("location")
            && metadata.location.push_str(value).is_err()
        {
            return false;
        }
    }
    let initial = &header[header_end..used];
    if metadata.chunked {
        progress(b"registry HTTP response uses chunked body");
        let accepted = read_chunked(stream, initial, sink);
        if !accepted {
            progress(b"registry chunked body rejected");
        }
        accepted
    } else if let Some(length) = metadata.content_length {
        progress(b"registry HTTP response has fixed-length body");
        let accepted = read_length(stream, initial, length, sink);
        if !accepted {
            progress(b"registry fixed-length body rejected");
        }
        accepted
    } else {
        progress(b"registry HTTP response is close-delimited");
        let accepted = read_until_close(stream, initial, sink);
        if !accepted {
            progress(b"registry close-delimited body rejected");
        }
        accepted
    }
}

fn read_length(
    stream: &mut impl Read,
    initial: &[u8],
    length: usize,
    sink: &mut impl BodySink,
) -> bool {
    if initial.len() > length || !sink.write(initial) {
        return false;
    }
    let mut remaining = length - initial.len();
    let mut buffer = [0; 4096];
    while remaining != 0 {
        let requested = remaining.min(buffer.len());
        let Ok(count) = stream.read(&mut buffer[..requested]) else {
            return false;
        };
        if count == 0 || !sink.write(&buffer[..count]) {
            return false;
        }
        remaining -= count;
    }
    true
}

fn read_until_close(stream: &mut impl Read, initial: &[u8], sink: &mut impl BodySink) -> bool {
    if !sink.write(initial) {
        return false;
    }
    let mut buffer = [0; 4096];
    loop {
        match stream.read(&mut buffer) {
            Ok(0) => return true,
            Ok(count) if sink.write(&buffer[..count]) => {}
            _ => return false,
        }
    }
}

// Chunk boundaries may straddle TLS records, so retain a compact carry buffer.
fn read_chunked(stream: &mut impl Read, initial: &[u8], sink: &mut impl BodySink) -> bool {
    let mut pending = [0; 8192];
    if initial.len() > pending.len() {
        return false;
    }
    pending[..initial.len()].copy_from_slice(initial);
    let mut used = initial.len();
    loop {
        let line_end = loop {
            if let Some(end) = pending[..used]
                .windows(2)
                .position(|bytes| bytes == b"\r\n")
            {
                break end;
            }
            if used == pending.len() {
                return false;
            }
            let Ok(count) = stream.read(&mut pending[used..]) else {
                return false;
            };
            if count == 0 {
                return false;
            }
            used += count;
        };
        let Ok(size_text) = core::str::from_utf8(&pending[..line_end]) else {
            return false;
        };
        let Some(size_text) = size_text.split(';').next() else {
            return false;
        };
        let Ok(size) = usize::from_str_radix(size_text, 16) else {
            return false;
        };
        let consumed = line_end + 2;
        pending.copy_within(consumed..used, 0);
        used -= consumed;
        if size == 0 {
            return true;
        }
        let mut remaining = size;
        while remaining != 0 {
            if used == 0 {
                let Ok(count) = stream.read(&mut pending) else {
                    return false;
                };
                if count == 0 {
                    return false;
                }
                used = count;
            }
            let count = remaining.min(used);
            if !sink.write(&pending[..count]) {
                return false;
            }
            pending.copy_within(count..used, 0);
            used -= count;
            remaining -= count;
        }
        while used < 2 {
            let Ok(count) = stream.read(&mut pending[used..]) else {
                return false;
            };
            if count == 0 {
                return false;
            }
            used += count;
        }
        if &pending[..2] != b"\r\n" {
            return false;
        }
        pending.copy_within(2..used, 0);
        used -= 2;
    }
}

fn resolve(
    host: &str,
    interface: &mut Interface,
    device: &mut VirtioNet,
    sockets: &mut SocketSet<'static>,
    handle: SocketHandle,
) -> Option<IpAddress> {
    let mut elapsed_polls = 0_usize;
    for attempt in 0..3 {
        REGISTRY_SERVICE.dns_fallback_a.store(0, Ordering::Release);
        REGISTRY_SERVICE
            .dns_fallback_state
            .store(0, Ordering::Release);
        REGISTRY_SERVICE.dns_query_sent.store(0, Ordering::Release);
        let query = match sockets.get_mut::<dns::Socket>(handle).start_query(
            interface.context(),
            host,
            DnsQueryType::A,
        ) {
            Ok(query) => query,
            Err(_) => {
                progress(b"registry DNS query creation failed");
                return None;
            }
        };
        let mut pending = true;
        for _ in 0..NETWORK_POLL_LIMIT {
            elapsed_polls = elapsed_polls.saturating_add(1);
            let _ = interface.poll(
                Instant::from_millis((elapsed_polls / POLLS_PER_MILLISECOND) as i64),
                device,
                sockets,
            );
            let fallback = REGISTRY_SERVICE.dns_fallback_a.swap(0, Ordering::AcqRel);
            if fallback != 0 {
                sockets.get_mut::<dns::Socket>(handle).cancel_query(query);
                return Some(IpAddress::Ipv4(Ipv4Address::from_octets(
                    fallback.to_be_bytes(),
                )));
            }
            match sockets
                .get_mut::<dns::Socket>(handle)
                .get_query_result(query)
            {
                Ok(addresses) => {
                    pending = false;
                    if let Some(address) = addresses.first().copied() {
                        return Some(address);
                    }
                    break;
                }
                Err(dns::GetQueryResultError::Pending) => core::hint::spin_loop(),
                Err(dns::GetQueryResultError::Failed) => {
                    pending = false;
                    break;
                }
            }
        }
        if pending {
            sockets.get_mut::<dns::Socket>(handle).cancel_query(query);
        }
        if attempt == 2 {
            let mut message = String::<96>::new();
            let _ = write!(
                message,
                "registry DNS response rejected (wire state {}, query sent {})",
                REGISTRY_SERVICE.dns_fallback_state.load(Ordering::Acquire),
                REGISTRY_SERVICE.dns_query_sent.load(Ordering::Acquire),
            );
            progress(message.as_bytes());
        }
    }
    None
}

struct TcpStream<'a> {
    interface: &'a mut Interface,
    device: &'a mut VirtioNet,
    sockets: &'a mut SocketSet<'static>,
    handle: SocketHandle,
    tick: i64,
    polls: usize,
}

impl<'a> TcpStream<'a> {
    fn connect(
        address: IpAddress,
        port: u16,
        interface: &'a mut Interface,
        device: &'a mut VirtioNet,
        sockets: &'a mut SocketSet<'static>,
        handle: SocketHandle,
    ) -> Option<Self> {
        let local_port = 53_721;
        sockets
            .get_mut::<tcp::Socket>(handle)
            .connect(interface.context(), (address, port), local_port)
            .ok()?;
        let mut stream = Self {
            interface,
            device,
            sockets,
            handle,
            tick: 0,
            polls: 0,
        };
        for _ in 0..NETWORK_POLL_LIMIT {
            stream.poll();
            let socket = stream.sockets.get::<tcp::Socket>(stream.handle);
            if socket.may_send() && socket.may_recv() {
                return Some(stream);
            }
            if !socket.is_open() {
                return None;
            }
        }
        None
    }

    fn poll(&mut self) {
        self.polls = self.polls.wrapping_add(1);
        if self.polls.is_multiple_of(POLLS_PER_MILLISECOND) {
            self.tick += 1;
        }
        let _ = self
            .interface
            .poll(Instant::from_millis(self.tick), self.device, self.sockets);
    }

    fn abort(&mut self) {
        self.sockets.get_mut::<tcp::Socket>(self.handle).abort();
        self.poll();
    }
}

impl ErrorType for TcpStream<'_> {
    type Error = ErrorKind;
}

impl Read for TcpStream<'_> {
    fn read(&mut self, output: &mut [u8]) -> Result<usize, Self::Error> {
        if output.is_empty() {
            return Ok(0);
        }
        for _ in 0..NETWORK_POLL_LIMIT {
            self.poll();
            let socket = self.sockets.get_mut::<tcp::Socket>(self.handle);
            if socket.can_recv() {
                return socket.recv_slice(output).map_err(|_| ErrorKind::Other);
            }
            if !socket.may_recv() {
                return Ok(0);
            }
        }
        Err(ErrorKind::TimedOut)
    }
}

impl Write for TcpStream<'_> {
    fn write(&mut self, input: &[u8]) -> Result<usize, Self::Error> {
        for _ in 0..NETWORK_POLL_LIMIT {
            self.poll();
            let socket = self.sockets.get_mut::<tcp::Socket>(self.handle);
            if socket.can_send() {
                return socket.send_slice(input).map_err(|_| ErrorKind::Other);
            }
            if !socket.may_send() {
                return Err(ErrorKind::BrokenPipe);
            }
        }
        Err(ErrorKind::TimedOut)
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        for _ in 0..NETWORK_POLL_LIMIT {
            self.poll();
            if self.sockets.get::<tcp::Socket>(self.handle).send_queue() == 0 {
                return Ok(());
            }
        }
        Err(ErrorKind::TimedOut)
    }
}

struct VirtioNet {
    dma: usize,
    dma_device: u64,
    rx_notify: usize,
    tx_notify: usize,
    rx_avail: u16,
    rx_used: u16,
    tx_avail: u16,
    tx_used: u16,
}

impl VirtioNet {
    unsafe fn new(info: &NativeNetworkBootV8) -> Option<Self> {
        let mut device = Self {
            dma: usize::try_from(info.dma_virtual_address).ok()?,
            dma_device: info.dma_device_address,
            rx_notify: usize::try_from(info.receive_notification_address).ok()?,
            tx_notify: usize::try_from(info.transmit_notification_address).ok()?,
            rx_avail: 0,
            rx_used: 0,
            tx_avail: 0,
            tx_used: 0,
        };
        device.post_receive();
        Some(device)
    }

    unsafe fn post_receive(&mut self) {
        write_u64(
            self.dma + RX_DESC_OFFSET as usize,
            self.dma_device + RX_BUFFER_OFFSET,
        );
        write_u32(
            self.dma + RX_DESC_OFFSET as usize + 8,
            RX_DESCRIPTOR_LENGTH as u32,
        );
        write_u16(self.dma + RX_DESC_OFFSET as usize + 12, 2);
        write_u16(self.dma + RX_DESC_OFFSET as usize + 14, 0);
        write_u16(
            self.dma + RX_AVAIL_OFFSET as usize + 4 + usize::from(self.rx_avail % QUEUE_SIZE) * 2,
            0,
        );
        self.rx_avail = self.rx_avail.wrapping_add(1);
        compiler_fence(Ordering::SeqCst);
        write_u16(self.dma + RX_AVAIL_OFFSET as usize + 2, self.rx_avail);
        write_u16(self.rx_notify, 0);
    }

    unsafe fn receive_length(&self) -> Option<usize> {
        let used = read_u16(self.dma + RX_USED_OFFSET as usize + 2);
        if used == self.rx_used {
            return None;
        }
        let element =
            self.dma + RX_USED_OFFSET as usize + 4 + usize::from(self.rx_used % QUEUE_SIZE) * 8;
        let length = read_u32(element + 4) as usize;
        length.checked_sub(TX_HEADER_LENGTH)
    }

    unsafe fn tx_ready(&mut self) -> bool {
        let used = read_u16(self.dma + TX_USED_OFFSET as usize + 2);
        if used != self.tx_used {
            self.tx_used = used;
        }
        self.tx_used == self.tx_avail
    }
}

struct VirtioRxToken(*mut VirtioNet, usize);
struct VirtioTxToken(*mut VirtioNet);

impl RxToken for VirtioRxToken {
    fn consume<R, F>(self, callback: F) -> R
    where
        F: FnOnce(&[u8]) -> R,
    {
        unsafe {
            let device = &mut *self.0;
            let bytes = core::slice::from_raw_parts(
                (device.dma + RX_BUFFER_OFFSET as usize + TX_HEADER_LENGTH) as *const u8,
                self.1,
            );
            if let Some(address) = first_dns_a_record(bytes) {
                REGISTRY_SERVICE
                    .dns_fallback_a
                    .store(address, Ordering::Release);
            }
            let result = callback(bytes);
            device.rx_used = device.rx_used.wrapping_add(1);
            device.post_receive();
            result
        }
    }

    fn meta(&self) -> PacketMeta {
        PacketMeta::default()
    }
}

fn is_dns_query(frame: &[u8]) -> bool {
    let Some(protocol) = frame.get(23) else {
        return false;
    };
    if frame.get(12..14) != Some(&[0x08, 0x00]) || *protocol != 17 {
        return false;
    }
    let header_length = usize::from(frame[14] & 0x0f) * 4;
    let udp = 14 + header_length;
    frame
        .get(udp + 2..udp + 4)
        .and_then(|bytes| bytes.try_into().ok())
        .map(u16::from_be_bytes)
        == Some(53)
}

fn first_dns_a_record(frame: &[u8]) -> Option<u32> {
    if frame.len() < 14 + 20 + 8 + 12
        || u16::from_be_bytes(frame[12..14].try_into().ok()?) != 0x0800
    {
        return None;
    }
    let ip = 14;
    let header_length = usize::from(frame[ip] & 0x0f).checked_mul(4)?;
    if header_length < 20 || frame.get(ip + 9).copied()? != 17 {
        return None;
    }
    let udp = ip.checked_add(header_length)?;
    if u16::from_be_bytes(frame.get(udp..udp + 2)?.try_into().ok()?) != 53 {
        return None;
    }
    REGISTRY_SERVICE
        .dns_fallback_state
        .store(1, Ordering::Release);
    let dns = udp.checked_add(8)?;
    let message = frame.get(dns..)?;
    let flags = u16::from_be_bytes(message.get(2..4)?.try_into().ok()?);
    if flags & 0x8000 == 0 {
        return None;
    }
    REGISTRY_SERVICE
        .dns_fallback_state
        .store(2, Ordering::Release);
    if flags & 0x000f != 0 {
        REGISTRY_SERVICE
            .dns_fallback_state
            .store(10 + u32::from(flags & 0x000f), Ordering::Release);
        return None;
    }
    let questions = usize::from(u16::from_be_bytes(message.get(4..6)?.try_into().ok()?));
    let answers = usize::from(u16::from_be_bytes(message.get(6..8)?.try_into().ok()?));
    let authority = usize::from(u16::from_be_bytes(message.get(8..10)?.try_into().ok()?));
    let additional = usize::from(u16::from_be_bytes(message.get(10..12)?.try_into().ok()?));
    let mut cursor = 12;
    for _ in 0..questions {
        cursor = skip_dns_name(message, cursor)?.checked_add(4)?;
        message.get(..cursor)?;
    }
    for _ in 0..answers.checked_add(authority)?.checked_add(additional)? {
        cursor = skip_dns_name(message, cursor)?;
        let record_type = u16::from_be_bytes(message.get(cursor..cursor + 2)?.try_into().ok()?);
        let class = u16::from_be_bytes(message.get(cursor + 2..cursor + 4)?.try_into().ok()?);
        let length = usize::from(u16::from_be_bytes(
            message.get(cursor + 8..cursor + 10)?.try_into().ok()?,
        ));
        let data = cursor.checked_add(10)?;
        let end = data.checked_add(length)?;
        let bytes = message.get(data..end)?;
        if record_type == 1 && class == 1 && length == 4 {
            return Some(u32::from_be_bytes(bytes.try_into().ok()?));
        }
        cursor = end;
    }
    REGISTRY_SERVICE
        .dns_fallback_state
        .store(4, Ordering::Release);
    None
}

fn skip_dns_name(message: &[u8], mut cursor: usize) -> Option<usize> {
    loop {
        let length = *message.get(cursor)?;
        if length == 0 {
            return cursor.checked_add(1);
        }
        if length & 0xc0 == 0xc0 {
            message.get(cursor + 1)?;
            return cursor.checked_add(2);
        }
        if length & 0xc0 != 0 || length > 63 {
            return None;
        }
        cursor = cursor.checked_add(1 + usize::from(length))?;
        message.get(..cursor)?;
    }
}

impl TxToken for VirtioTxToken {
    fn consume<R, F>(self, length: usize, callback: F) -> R
    where
        F: FnOnce(&mut [u8]) -> R,
    {
        unsafe {
            let device = &mut *self.0;
            let header = core::slice::from_raw_parts_mut(
                (device.dma + TX_BUFFER_OFFSET as usize) as *mut u8,
                TX_HEADER_LENGTH,
            );
            header.fill(0);
            let bytes = core::slice::from_raw_parts_mut(
                (device.dma + TX_BUFFER_OFFSET as usize + TX_HEADER_LENGTH) as *mut u8,
                length,
            );
            let result = callback(bytes);
            if is_dns_query(bytes) {
                REGISTRY_SERVICE.dns_query_sent.store(1, Ordering::Release);
            }
            write_u64(
                device.dma + TX_DESC_OFFSET as usize,
                device.dma_device + TX_BUFFER_OFFSET,
            );
            write_u32(
                device.dma + TX_DESC_OFFSET as usize + 8,
                (length + TX_HEADER_LENGTH) as u32,
            );
            write_u16(device.dma + TX_DESC_OFFSET as usize + 12, 0);
            write_u16(device.dma + TX_DESC_OFFSET as usize + 14, 0);
            write_u16(
                device.dma
                    + TX_AVAIL_OFFSET as usize
                    + 4
                    + usize::from(device.tx_avail % QUEUE_SIZE) * 2,
                0,
            );
            device.tx_avail = device.tx_avail.wrapping_add(1);
            compiler_fence(Ordering::SeqCst);
            write_u16(device.dma + TX_AVAIL_OFFSET as usize + 2, device.tx_avail);
            write_u16(device.tx_notify, 1);
            result
        }
    }
}

impl Device for VirtioNet {
    type RxToken<'a>
        = VirtioRxToken
    where
        Self: 'a;
    type TxToken<'a>
        = VirtioTxToken
    where
        Self: 'a;

    fn receive(&mut self, _timestamp: Instant) -> Option<(Self::RxToken<'_>, Self::TxToken<'_>)> {
        let length = unsafe { self.receive_length()? };
        let pointer = self as *mut Self;
        Some((VirtioRxToken(pointer, length), VirtioTxToken(pointer)))
    }

    fn transmit(&mut self, _timestamp: Instant) -> Option<Self::TxToken<'_>> {
        unsafe { self.tx_ready().then_some(VirtioTxToken(self as *mut Self)) }
    }

    fn capabilities(&self) -> DeviceCapabilities {
        let mut capabilities = DeviceCapabilities::default();
        capabilities.medium = Medium::Ethernet;
        capabilities.max_transmission_unit = 1514;
        capabilities
    }
}

unsafe fn write_u16(address: usize, value: u16) {
    write_volatile(address as *mut u16, value);
}

unsafe fn write_u32(address: usize, value: u32) {
    write_volatile(address as *mut u32, value);
}

unsafe fn write_u64(address: usize, value: u64) {
    write_volatile(address as *mut u64, value);
}

unsafe fn read_u16(address: usize) -> u16 {
    read_volatile(address as *const u16)
}

unsafe fn read_u32(address: usize) -> u32 {
    read_volatile(address as *const u32)
}
