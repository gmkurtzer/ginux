use std::collections::BTreeSet;
use std::env;
use std::fmt::Write as _;
use std::fs;
use std::io::Write as _;
use std::path::Path;

use ed25519_dalek::{Signer, SigningKey};
use sha2::{Digest, Sha256};

#[allow(dead_code)]
mod control_abi {
    include!("../../../specs/abi/control-v4.rs");
}

const MAGIC: &[u8; 8] = b"GINXMNF\0";
const VERSION: u16 = 3;
const HEADER_SIZE: u16 = 112;
const EXECUTABLE_MAGIC: &[u8; 8] = b"GINXBIN\0";
const EXECUTABLE_VERSION: u16 = 2;
const EXECUTABLE_HEADER_SIZE: u16 = 40;
const EXECUTABLE_MAX_CODE_SIZE: usize = 4096;
const NETWORK_SERVICE_MAX_SIZE: usize = 64 * 1024;
const PROTOCOL_SERVICE_MAX_SIZE: usize = 64 * 1024;
const VOLUME_SERVICE_MAX_SIZE: usize = 64 * 1024;
const ROOT_IMAGE_MAX_SIZE: usize = 48 * 4096;
const DEVELOPMENT_SEED: [u8; 32] = [
    0x9d, 0x61, 0xb1, 0x9d, 0xef, 0xfd, 0x5a, 0x60, 0xba, 0x84, 0x4a, 0xf4, 0x92, 0xec, 0x2c, 0xc4,
    0x44, 0x49, 0xc5, 0x69, 0x7b, 0x32, 0x69, 0x19, 0x70, 0x3b, 0xac, 0x03, 0x1c, 0xae, 0x7f, 0x60,
];

fn main() {
    let arguments: Vec<String> = env::args().collect();
    if arguments.len() != 10 {
        fail(
            "usage: ginux-manifest-tool MANIFEST_CONFIG BUNDLE_HEX ROOT_IMAGE LINUX_ARTIFACTS NETWORK_SERVICE PROTOCOL_SERVICE VOLUME_SERVICE SYSTEM_PROFILE OUTPUT_RS",
        );
    }
    let config = fs::read_to_string(&arguments[1]).unwrap_or_else(|error| {
        fail(&format!("cannot read {}: {error}", arguments[1]));
    });
    let bundle_source = fs::read_to_string(&arguments[2]).unwrap_or_else(|error| {
        fail(&format!("cannot read {}: {error}", arguments[2]));
    });
    let (sequence, grants) = parse_config(&config);
    let (code, entries) = decode_hex(&bundle_source);
    if code.is_empty() || code.len() > EXECUTABLE_MAX_CODE_SIZE {
        fail("control-plane code must contain 1 through 4096 bytes");
    }
    let bundle = wrap_executable(&code, entries);
    let network_service = read_root_artifact(Path::new(&arguments[5]));
    if network_service.is_empty() || network_service.len() > NETWORK_SERVICE_MAX_SIZE {
        fail("network service must contain 1 through 65536 bytes");
    }
    let protocol_service = read_root_artifact(Path::new(&arguments[6]));
    if protocol_service.is_empty() || protocol_service.len() > PROTOCOL_SERVICE_MAX_SIZE {
        fail("protocol service must contain 1 through 65536 bytes");
    }
    let volume_service = read_root_artifact(Path::new(&arguments[7]));
    if volume_service.is_empty() || volume_service.len() > VOLUME_SERVICE_MAX_SIZE {
        fail("volume service must contain 1 through 65536 bytes");
    }
    let system_profile = fs::read(&arguments[8]).unwrap_or_else(|error| {
        fail(&format!("cannot read {}: {error}", arguments[8]));
    });
    validate_system_profile(&system_profile);
    let root_image = build_initramfs(
        &arguments[4],
        &network_service,
        &protocol_service,
        &volume_service,
        &system_profile,
    );
    if root_image.len() > ROOT_IMAGE_MAX_SIZE {
        fail("generated root image exceeds the 48-page publication capacity");
    }

    let bundle_digest: [u8; 32] = Sha256::digest(&bundle).into();
    let root_digest: [u8; 32] = Sha256::digest(&root_image).into();
    let total_size = usize::from(HEADER_SIZE) + grants.len() * 16;
    let mut manifest = Vec::with_capacity(total_size);
    manifest.extend_from_slice(MAGIC);
    manifest.extend_from_slice(&VERSION.to_le_bytes());
    manifest.extend_from_slice(&HEADER_SIZE.to_le_bytes());
    manifest.extend_from_slice(&(total_size as u32).to_le_bytes());
    manifest.extend_from_slice(&sequence.to_le_bytes());
    manifest.extend_from_slice(&(grants.len() as u16).to_le_bytes());
    manifest.extend_from_slice(&0_u16.to_le_bytes());
    manifest.extend_from_slice(&0_u32.to_le_bytes());
    manifest.extend_from_slice(&(bundle.len() as u64).to_le_bytes());
    manifest.extend_from_slice(&bundle_digest);
    manifest.extend_from_slice(&(root_image.len() as u64).to_le_bytes());
    manifest.extend_from_slice(&root_digest);
    for (object, rights) in grants {
        manifest.extend_from_slice(&object.to_le_bytes());
        manifest.extend_from_slice(&rights.to_le_bytes());
    }

    let manifest_digest: [u8; 32] = Sha256::digest(&manifest).into();
    let signature = SigningKey::from_bytes(&DEVELOPMENT_SEED)
        .sign(&manifest)
        .to_bytes();
    fs::write(&arguments[3], &root_image).unwrap_or_else(|error| {
        fail(&format!("cannot write {}: {error}", arguments[3]));
    });
    let output = render(
        &manifest,
        &manifest_digest,
        &signature,
        &bundle,
        &root_image,
    );
    let output_path = Path::new(&arguments[9]);
    fs::write(output_path, output).unwrap_or_else(|error| {
        fail(&format!("cannot write {}: {error}", output_path.display()));
    });
    println!(
        "Generated {} from {} signed by the development-only RFC test key",
        output_path.display(),
        arguments[2]
    );
}

fn build_initramfs(
    linux_artifacts: &str,
    network_service: &[u8],
    protocol_service: &[u8],
    volume_service: &[u8],
    system_profile: &[u8],
) -> Vec<u8> {
    let artifacts = Path::new(linux_artifacts);
    let linux_init = read_root_artifact(&artifacts.join("linux-linked-graph-main.elf"));
    let linux_interpreter = read_root_artifact(&artifacts.join("ld-ginux.so"));
    let linux_dependency = read_root_artifact(&artifacts.join("libginux.so"));
    let linux_auxiliary = read_root_artifact(&artifacts.join("libginux-aux.so"));
    let linux_exec_second = read_root_artifact(&artifacts.join("linux-exec-second.elf"));
    let linux_signal_probe = read_root_artifact(&artifacts.join("linux-signal-probe.elf"));
    let linux_system_init = read_root_artifact(&artifacts.join("linux-system-init.elf"));
    let mut archive = Vec::new();
    for (inode, mode, name, data) in [
        (1, 0o040555, "dev", &b""[..]),
        (2, 0o040555, "etc", &b""[..]),
        (3, 0o040555, "proc", &b""[..]),
        (4, 0o040555, "sys", &b""[..]),
        (5, 0o041777, "tmp", &b""[..]),
        (6, 0o040555, "var", &b""[..]),
        (7, 0o040555, "var/lib", &b""[..]),
        (8, 0o100555, "var/lib/state", &b"ready\n"[..]),
        (9, 0o100555, "init", &b"GINUX_INITRAMFS_INIT_V0\n"[..]),
        (10, 0o100555, "etc/hostname", &b"ginux\n"[..]),
        (
            11,
            0o100555,
            "etc/os-release",
            &b"NAME=Ginux\nID=ginux\n"[..],
        ),
        (12, 0o100555, "etc/motd", &b"Welcome to Ginux\n"[..]),
        (19, 0o040555, "bin", &b""[..]),
        (23, 0o040555, "sbin", &b""[..]),
        (22, 0o100444, "etc/ginux-system-profile", system_profile),
    ] {
        append_newc_entry(&mut archive, inode, mode, name, data);
    }
    for (inode, name, data) in [
        (13, "linux/init", linux_init.as_slice()),
        (14, "linux/ld-ginux.so", linux_interpreter.as_slice()),
        (15, "linux/libginux.so", linux_dependency.as_slice()),
        (16, "linux/libginux-aux.so", linux_auxiliary.as_slice()),
        (18, "bin/signal-probe", linux_signal_probe.as_slice()),
        (20, "bin/exec-second", linux_exec_second.as_slice()),
        (24, "sbin/init", linux_system_init.as_slice()),
        (25, "native/volume", volume_service),
        (26, "native/network", network_service),
        (27, "native/protocol", protocol_service),
    ] {
        append_newc_entry(&mut archive, inode, 0o100555, name, data);
    }
    append_newc_entry(&mut archive, 17, 0o100555, "var/lib/version", b"0\n");
    append_newc_entry(&mut archive, 28, 0, "TRAILER!!!", &[]);
    archive
}

fn validate_system_profile(bytes: &[u8]) {
    let text = std::str::from_utf8(bytes).unwrap_or_else(|_| fail("system profile is not UTF-8"));
    if !text.ends_with('\n') || text.contains('\r') || text.contains('\0') {
        fail("system profile must use newline-terminated LF text");
    }
    let mut lines = text.lines();
    if lines.next() != Some("GINUX_SYSTEM_PROFILE_V0") {
        fail("system profile has an unsupported version");
    }
    let mut names = BTreeSet::new();
    let mut ready = false;
    let mut metadata = [false; 4];
    for line in lines {
        let mut fields = line.split('\t');
        let Some(kind) = fields.next() else {
            fail("system profile row has no kind");
        };
        let Some(name) = fields.next() else {
            fail("system profile row has no name");
        };
        let Some(state) = fields.next() else {
            fail("system profile row has no state");
        };
        let Some(description) = fields.next() else {
            fail("system profile row has no description");
        };
        if fields.next().is_some()
            || !matches!(kind, "meta" | "runtime" | "filesystem" | "security")
            || name.is_empty()
            || description.is_empty()
            || !names.insert(name)
        {
            fail("system profile contains a malformed or duplicate row");
        }
        if kind == "meta" {
            let index = match (name, state, description) {
                ("architecture", "x86_64", "required") => 0,
                ("init", "/sbin/init", "required") => 1,
                ("console", "/dev/console", "required") => 2,
                ("root", "newc-ro", "required") => 3,
                _ => fail("system profile contains unsupported metadata"),
            };
            metadata[index] = true;
        } else {
            match state {
                "ready" => ready = true,
                "partial" | "missing" => {}
                _ => fail("system profile row has an unsupported state"),
            }
        }
    }
    if metadata != [true; 4] {
        fail("system profile is missing required metadata");
    }
    if !ready {
        fail("system profile must expose at least one ready requirement");
    }
}

fn read_root_artifact(path: &Path) -> Vec<u8> {
    fs::read(path).unwrap_or_else(|error| {
        fail(&format!(
            "cannot read Linux root artifact {}: {error}",
            path.display()
        ));
    })
}

fn append_newc_entry(archive: &mut Vec<u8>, inode: u32, mode: u32, name: &str, data: &[u8]) {
    archive.extend_from_slice(b"070701");
    for field in [
        inode,
        mode,
        0,
        0,
        1,
        0,
        u32::try_from(data.len()).unwrap_or_else(|_| fail("root file is too large")),
        0,
        0,
        0,
        0,
        u32::try_from(name.len() + 1).unwrap_or_else(|_| fail("root path is too large")),
        0,
    ] {
        write!(archive, "{field:08x}").unwrap();
    }
    archive.extend_from_slice(name.as_bytes());
    archive.push(0);
    pad_newc(archive);
    archive.extend_from_slice(data);
    pad_newc(archive);
}

fn pad_newc(archive: &mut Vec<u8>) {
    while !archive.len().is_multiple_of(4) {
        archive.push(0);
    }
}

fn wrap_executable(code: &[u8], entries: [u32; 3]) -> Vec<u8> {
    let total_size = usize::from(EXECUTABLE_HEADER_SIZE) + code.len();
    let mut executable = Vec::with_capacity(total_size);
    executable.extend_from_slice(EXECUTABLE_MAGIC);
    executable.extend_from_slice(&EXECUTABLE_VERSION.to_le_bytes());
    executable.extend_from_slice(&EXECUTABLE_HEADER_SIZE.to_le_bytes());
    executable.extend_from_slice(&(total_size as u32).to_le_bytes());
    executable.extend_from_slice(&(code.len() as u32).to_le_bytes());
    executable.extend_from_slice(&3_u16.to_le_bytes());
    executable.extend_from_slice(&0_u16.to_le_bytes());
    for entry in entries {
        executable.extend_from_slice(&entry.to_le_bytes());
    }
    executable.extend_from_slice(&0_u32.to_le_bytes());
    executable.extend_from_slice(code);
    executable
}

fn parse_config(source: &str) -> (u64, Vec<(u64, u64)>) {
    let mut sequence = None;
    let mut grants = Vec::new();
    for raw_line in source.lines() {
        let line = raw_line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        if let Some(value) = line.strip_prefix("sequence=") {
            if sequence.is_some() {
                fail("manifest contains multiple sequence values");
            }
            sequence = value.parse().ok();
            if sequence.is_none() {
                fail("manifest sequence is not an unsigned integer");
            }
        } else if let Some(value) = line.strip_prefix("grant=") {
            let Some((object, rights)) = value.split_once(',') else {
                fail("grant must use grant=OBJECT,RIGHTS");
            };
            let Some(object) = parse_integer(object) else {
                fail("grant object is invalid");
            };
            let Some(rights) = parse_integer(rights) else {
                fail("grant rights are invalid");
            };
            grants.push((object, rights));
        } else {
            fail(&format!("unknown manifest line: {line}"));
        }
    }
    let sequence = sequence.filter(|value| *value != 0).unwrap_or_else(|| {
        fail("manifest requires one nonzero sequence");
    });
    if grants.is_empty() || grants.len() > 8 {
        fail("manifest requires one through eight grants");
    }
    (sequence, grants)
}

fn parse_integer(value: &str) -> Option<u64> {
    let value = value.trim();
    value.strip_prefix("0x").map_or_else(
        || value.parse().ok(),
        |hex| u64::from_str_radix(hex, 16).ok(),
    )
}

fn decode_hex(source: &str) -> (Vec<u8>, [u32; 3]) {
    let discovery_word =
        ((u64::from(control_abi::VERSION)) << 16) | u64::from(control_abi::FEATURES);
    let descriptor_header = (control_abi::DESCRIPTOR_SIZE as u64)
        | ((u64::from(control_abi::VERSION)) << 32)
        | ((u64::from(control_abi::FEATURES)) << 48);
    let names = ["control", "service", "restart"];
    let mut entries = [None; 3];
    let mut code = Vec::new();
    for raw_line in source.lines() {
        let line = raw_line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        if let Some(name) = line.strip_prefix("@entry ") {
            let name = name.trim();
            let Some(index) = names.iter().position(|candidate| *candidate == name) else {
                fail(&format!("unknown executable entry: {name}"));
            };
            if entries[index].is_some() {
                fail(&format!("duplicate executable entry: {name}"));
            }
            entries[index] = Some(u32::try_from(code.len()).unwrap_or_else(|_| {
                fail(&format!("executable entry is too large: {name}"));
            }));
            continue;
        }
        let digits = line
            .replace(
                "@DISCOVERY_WORD_LE32",
                &little_endian_hex(discovery_word, 4),
            )
            .replace(
                "@ABI_DESCRIPTOR_HEADER_LE64",
                &little_endian_hex(descriptor_header, 8),
            )
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect::<String>();
        if !digits.len().is_multiple_of(2) || !digits.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            fail("bundle lines must contain entry labels or an even number of hexadecimal digits");
        }
        code.extend(
            (0..digits.len())
                .step_by(2)
                .map(|offset| u8::from_str_radix(&digits[offset..offset + 2], 16).unwrap()),
        );
    }
    let entries = entries.map(|entry| entry.unwrap_or_else(|| fail("entry labels are incomplete")));
    if entries.iter().any(|entry| *entry as usize >= code.len())
        || entries[0] == entries[1]
        || entries[0] == entries[2]
        || entries[1] == entries[2]
    {
        fail("executable entries must be distinct labels inside the code segment");
    }
    (code, entries)
}

fn little_endian_hex(value: u64, width: usize) -> String {
    let mut encoded = String::with_capacity(width * 2);
    for byte in &value.to_le_bytes()[..width] {
        write!(encoded, "{byte:02x}").unwrap();
    }
    encoded
}

fn render(
    manifest: &[u8],
    digest: &[u8; 32],
    signature: &[u8; 64],
    bundle: &[u8],
    root_image: &[u8],
) -> String {
    let mut output = String::from("// @generated by ginux-manifest-tool; do not edit.\n\n");
    render_array(&mut output, "BOOT_MANIFEST_SHA256", digest, false);
    render_array(&mut output, "BOOT_MANIFEST_SIGNATURE", signature, false);
    render_array(&mut output, "CONTROL_PLANE_BUNDLE", bundle, true);
    render_array(&mut output, "ROOT_IMAGE", root_image, true);
    render_array(&mut output, "BOOT_MANIFEST", manifest, true);
    output.pop();
    output
}

fn render_array(output: &mut String, name: &str, bytes: &[u8], is_static: bool) {
    let declaration = if is_static { "static" } else { "const" };
    writeln!(
        output,
        "pub(crate) {declaration} {name}: [u8; {}] = [",
        bytes.len()
    )
    .unwrap();
    for chunk in bytes.chunks(16) {
        output.push_str("    ");
        for (index, byte) in chunk.iter().enumerate() {
            if index != 0 {
                output.push(' ');
            }
            write!(output, "0x{byte:02x},").unwrap();
        }
        output.push('\n');
    }
    output.push_str("];\n\n");
}

fn fail(message: &str) -> ! {
    eprintln!("manifest generation failed: {message}");
    std::process::exit(1);
}
