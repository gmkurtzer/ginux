use std::env;
use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use ed25519_dalek::{Signer, SigningKey};
use sha2::{Digest, Sha256};

const MAGIC: &[u8; 8] = b"GINXMNF\0";
const VERSION: u16 = 2;
const HEADER_SIZE: u16 = 72;
const EXECUTABLE_MAGIC: &[u8; 8] = b"GINXBIN\0";
const EXECUTABLE_VERSION: u16 = 1;
const EXECUTABLE_HEADER_SIZE: u16 = 32;
const EXECUTABLE_MAX_CODE_SIZE: usize = 4096;
const DEVELOPMENT_SEED: [u8; 32] = [
    0x9d, 0x61, 0xb1, 0x9d, 0xef, 0xfd, 0x5a, 0x60, 0xba, 0x84, 0x4a, 0xf4, 0x92, 0xec, 0x2c, 0xc4,
    0x44, 0x49, 0xc5, 0x69, 0x7b, 0x32, 0x69, 0x19, 0x70, 0x3b, 0xac, 0x03, 0x1c, 0xae, 0x7f, 0x60,
];

fn main() {
    let arguments: Vec<String> = env::args().collect();
    if arguments.len() != 4 {
        fail("usage: ginux-manifest-tool MANIFEST_CONFIG BUNDLE_HEX OUTPUT_RS");
    }
    let config = fs::read_to_string(&arguments[1]).unwrap_or_else(|error| {
        fail(&format!("cannot read {}: {error}", arguments[1]));
    });
    let bundle_source = fs::read_to_string(&arguments[2]).unwrap_or_else(|error| {
        fail(&format!("cannot read {}: {error}", arguments[2]));
    });
    let (sequence, grants) = parse_config(&config);
    let code = decode_hex(&bundle_source);
    if code.is_empty() || code.len() > EXECUTABLE_MAX_CODE_SIZE {
        fail("control-plane code must contain 1 through 4096 bytes");
    }
    let bundle = wrap_executable(&code);

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
    let output_path = Path::new(&arguments[3]);
    fs::write(output_path, output).unwrap_or_else(|error| {
        fail(&format!("cannot write {}: {error}", output_path.display()));
    });
    println!(
        "Generated {} from {} signed by the development-only RFC test key",
        output_path.display(),
        arguments[2]
    );
}

fn wrap_executable(code: &[u8]) -> Vec<u8> {
    let total_size = usize::from(EXECUTABLE_HEADER_SIZE) + code.len();
    let mut executable = Vec::with_capacity(total_size);
    executable.extend_from_slice(EXECUTABLE_MAGIC);
    executable.extend_from_slice(&EXECUTABLE_VERSION.to_le_bytes());
    executable.extend_from_slice(&EXECUTABLE_HEADER_SIZE.to_le_bytes());
    executable.extend_from_slice(&(total_size as u32).to_le_bytes());
    executable.extend_from_slice(&0_u32.to_le_bytes());
    executable.extend_from_slice(&(code.len() as u32).to_le_bytes());
    executable.extend_from_slice(&0_u32.to_le_bytes());
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

fn decode_hex(source: &str) -> Vec<u8> {
    let mut digits = String::new();
    for line in source.lines() {
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
