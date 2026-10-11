use std::{env, fs, path::PathBuf, process::Command};

fn main() {
    println!("cargo:rerun-if-changed=linux-probe.S");
    println!("cargo:rerun-if-env-changed=CLANG");

    let output = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR is set"));
    let object = output.join("linux-probe.o");
    let clang = env::var_os("CLANG").unwrap_or_else(|| "clang".into());
    let status = Command::new(clang)
        .args(["-target", "x86_64-none-elf", "-c", "linux-probe.S", "-o"])
        .arg(&object)
        .status()
        .expect("failed to run clang for Linux probe");
    assert!(status.success(), "clang failed to assemble Linux probe");

    let elf = fs::read(&object).expect("failed to read Linux probe object");
    let text = elf_section(&elf, b".text").expect("Linux probe object has no .text section");
    fs::write(output.join("linux-probe.bin"), text).expect("failed to write Linux probe bytes");
}

fn elf_section<'a>(elf: &'a [u8], wanted: &[u8]) -> Option<&'a [u8]> {
    (elf.get(..6)? == b"\x7fELF\x02\x01").then_some(())?;
    let section_offset = usize::try_from(read_u64(elf, 40)?).ok()?;
    let entry_size = usize::from(read_u16(elf, 58)?);
    let count = usize::from(read_u16(elf, 60)?);
    let names_index = usize::from(read_u16(elf, 62)?);
    if entry_size < 64 || names_index >= count {
        return None;
    }
    let names_header = section_offset.checked_add(names_index.checked_mul(entry_size)?)?;
    let names_offset = usize::try_from(read_u64(elf, names_header + 24)?).ok()?;
    let names_size = usize::try_from(read_u64(elf, names_header + 32)?).ok()?;
    let names = elf.get(names_offset..names_offset.checked_add(names_size)?)?;
    for index in 0..count {
        let header = section_offset.checked_add(index.checked_mul(entry_size)?)?;
        let name_offset = usize::try_from(read_u32(elf, header)?).ok()?;
        let name = names.get(name_offset..)?.split(|byte| *byte == 0).next()?;
        if name == wanted {
            let offset = usize::try_from(read_u64(elf, header + 24)?).ok()?;
            let size = usize::try_from(read_u64(elf, header + 32)?).ok()?;
            return elf.get(offset..offset.checked_add(size)?);
        }
    }
    None
}

fn read_u16(bytes: &[u8], offset: usize) -> Option<u16> {
    Some(u16::from_le_bytes(
        bytes.get(offset..offset + 2)?.try_into().ok()?,
    ))
}

fn read_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        bytes.get(offset..offset + 4)?.try_into().ok()?,
    ))
}

fn read_u64(bytes: &[u8], offset: usize) -> Option<u64> {
    Some(u64::from_le_bytes(
        bytes.get(offset..offset + 8)?.try_into().ok()?,
    ))
}
