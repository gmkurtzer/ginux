use std::env;
use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use ed25519_dalek::{Signer, SigningKey};
use sha2::{Digest, Sha256};

#[allow(dead_code)]
mod control_abi {
    include!("../../../specs/abi/control-v1.rs");
}

const MAGIC: &[u8; 8] = b"GINXMNF\0";
const VERSION: u16 = 2;
const HEADER_SIZE: u16 = 72;
const EXECUTABLE_MAGIC: &[u8; 8] = b"GINXBIN\0";
const EXECUTABLE_VERSION: u16 = 2;
const EXECUTABLE_HEADER_SIZE: u16 = 40;
const EXECUTABLE_MAX_CODE_SIZE: usize = 4096;
const DEVELOPMENT_SEED: [u8; 32] = [
    0x9d, 0x61, 0xb1, 0x9d, 0xef, 0xfd, 0x5a, 0x60, 0xba, 0x84, 0x4a, 0xf4, 0x92, 0xec, 0x2c, 0xc4,
    0x44, 0x49, 0xc5, 0x69, 0x7b, 0x32, 0x69, 0x19, 0x70, 0x3b, 0xac, 0x03, 0x1c, 0xae, 0x7f, 0x60,
];

fn main() {
    let arguments: Vec<String> = env::args().collect();
    if arguments.len() != 5 {
        fail("usage: ginux-manifest-tool MANIFEST_CONFIG BUNDLE_HEX ENTRY_CONFIG OUTPUT_RS");
    }
    let config = fs::read_to_string(&arguments[1]).unwrap_or_else(|error| {
        fail(&format!("cannot read {}: {error}", arguments[1]));
    });
    let bundle_source = fs::read_to_string(&arguments[2]).unwrap_or_else(|error| {
        fail(&format!("cannot read {}: {error}", arguments[2]));
    });
    let entry_source = fs::read_to_string(&arguments[3]).unwrap_or_else(|error| {
        fail(&format!("cannot read {}: {error}", arguments[3]));
    });
    let (sequence, grants) = parse_config(&config);
    let code = decode_hex(&bundle_source);
    if code.is_empty() || code.len() > EXECUTABLE_MAX_CODE_SIZE {
        fail("control-plane code must contain 1 through 4096 bytes");
    }
    let entries = parse_entries(&entry_source, code.len());
    let bundle = wrap_executable(&code, entries);

    let bundle_digest: [u8; 32] = Sha256::digest(&bundle).into();
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
    for (object, rights) in grants {
        manifest.extend_from_slice(&object.to_le_bytes());
        manifest.extend_from_slice(&rights.to_le_bytes());
    }

    let manifest_digest: [u8; 32] = Sha256::digest(&manifest).into();
    let signature = SigningKey::from_bytes(&DEVELOPMENT_SEED)
        .sign(&manifest)
        .to_bytes();
    let output = render(&manifest, &manifest_digest, &signature, &bundle);
    let output_path = Path::new(&arguments[4]);
    fs::write(output_path, output).unwrap_or_else(|error| {
        fail(&format!("cannot write {}: {error}", output_path.display()));
    });
    println!(
        "Generated {} from {} signed by the development-only RFC test key",
        output_path.display(),
        arguments[2]
    );
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

fn parse_entries(source: &str, code_size: usize) -> [u32; 3] {
    let names = ["control", "service", "restart"];
    let mut entries = [None; 3];
    for raw_line in source.lines() {
        let line = raw_line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let Some((name, value)) = line.split_once('=') else {
            fail("entry config lines must use NAME=OFFSET");
        };
        let Some(index) = names.iter().position(|candidate| *candidate == name.trim()) else {
            fail(&format!("unknown executable entry: {}", name.trim()));
        };
        if entries[index].is_some() {
            fail(&format!("duplicate executable entry: {}", names[index]));
        }
        let Some(value) = parse_integer(value) else {
            fail(&format!("invalid executable entry: {}", names[index]));
        };
        let Ok(value) = u32::try_from(value) else {
            fail(&format!("executable entry is too large: {}", names[index]));
        };
        entries[index] = Some(value);
    }
    let entries = entries.map(|entry| entry.unwrap_or_else(|| fail("entry config is incomplete")));
    if entries.iter().any(|entry| *entry as usize >= code_size)
        || entries[0] == entries[1]
        || entries[0] == entries[2]
        || entries[1] == entries[2]
    {
        fail("executable entries must be distinct offsets inside the code segment");
    }
    entries
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

fn decode_hex(source: &str) -> Vec<u8> {
    let discovery_word =
        ((u64::from(control_abi::VERSION)) << 16) | u64::from(control_abi::FEATURES);
    let descriptor_header = (control_abi::DESCRIPTOR_SIZE as u64)
        | ((u64::from(control_abi::VERSION)) << 32)
        | ((u64::from(control_abi::FEATURES)) << 48);
    let expanded = source
        .replace(
            "@DISCOVERY_WORD_LE32",
            &little_endian_hex(discovery_word, 4),
        )
        .replace(
            "@ABI_DESCRIPTOR_HEADER_LE64",
            &little_endian_hex(descriptor_header, 8),
        );
    let mut digits = String::new();
    for line in expanded.lines() {
        digits.extend(
            line.split('#')
                .next()
                .unwrap_or("")
                .chars()
                .filter(|character| !character.is_whitespace()),
        );
    }
    if !digits.len().is_multiple_of(2) || !digits.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        fail("bundle must contain an even number of hexadecimal digits");
    }
    (0..digits.len())
        .step_by(2)
        .map(|offset| u8::from_str_radix(&digits[offset..offset + 2], 16).unwrap())
        .collect()
}

fn little_endian_hex(value: u64, width: usize) -> String {
    let mut encoded = String::with_capacity(width * 2);
    for byte in &value.to_le_bytes()[..width] {
        write!(encoded, "{byte:02x}").unwrap();
    }
    encoded
}

fn render(manifest: &[u8], digest: &[u8; 32], signature: &[u8; 64], bundle: &[u8]) -> String {
    let mut output = String::from("// @generated by ginux-manifest-tool; do not edit.\n\n");
    render_array(&mut output, "BOOT_MANIFEST_SHA256", digest, false);
    render_array(&mut output, "BOOT_MANIFEST_SIGNATURE", signature, false);
    render_array(&mut output, "CONTROL_PLANE_BUNDLE", bundle, true);
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
