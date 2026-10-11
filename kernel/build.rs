use std::{env, ffi::OsString, fs, path::PathBuf, process::Command};

const PAGE_SIZE: usize = 4096;
const LINUX_PROBE_ENTRY_OFFSET: usize = 0x100;
const LINUX_PROBE_EXECUTABLE_PAGES: usize = 4;
const LINUX_PROBE_MAX_TEXT_SIZE: usize =
    LINUX_PROBE_EXECUTABLE_PAGES * PAGE_SIZE - LINUX_PROBE_ENTRY_OFFSET;
const LINUX_INTERPRETER_ENTRY_OFFSET: usize = 0x100;
const LINUX_INTERPRETER_DATA_OFFSET: usize = 0x400;
const LINUX_INTERPRETER_MAX_TEXT_SIZE: usize =
    LINUX_INTERPRETER_DATA_OFFSET - LINUX_INTERPRETER_ENTRY_OFFSET;
const LINUX_DEPENDENCY_MAX_TEXT_SIZE: usize = 64;

fn main() {
    println!("cargo:rerun-if-changed=linux-probe.S");
    println!("cargo:rerun-if-changed=linux-interpreter.S");
    println!("cargo:rerun-if-changed=linux-dependency.S");
    println!("cargo:rerun-if-changed=linux-auxiliary-dependency.S");
    println!("cargo:rerun-if-changed=linux-probe.ld");
    println!("cargo:rerun-if-changed=linux-linked-main.ld");
    println!("cargo:rerun-if-changed=linux-linked-dependency.ld");
    println!("cargo:rerun-if-changed=linux-linked-auxiliary.ld");
    println!("cargo:rerun-if-changed=linux-linked-interpreter.ld");
    println!("cargo:rerun-if-env-changed=CLANG");
    println!("cargo:rerun-if-env-changed=LLD");
    println!("cargo:rerun-if-env-changed=GINUX_LINUX_PROBE_EXPORT");
    println!("cargo:rerun-if-env-changed=GINUX_LINUX_DYNAMIC_EXPORT_DIR");

    let output = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR is set"));
    let object = output.join("linux-probe.o");
    let clang = env::var_os("CLANG").unwrap_or_else(|| "clang".into());
    let status = Command::new(&clang)
        .args(["-target", "x86_64-none-elf", "-c", "linux-probe.S", "-o"])
        .arg(&object)
        .status()
        .expect("failed to run clang for Linux probe");
    assert!(status.success(), "clang failed to assemble Linux probe");

    let elf = fs::read(&object).expect("failed to read Linux probe object");
    let text = elf_section(&elf, b".text").expect("Linux probe object has no .text section");
    assert!(
        !elf_section_has_relocations(&elf, b".text")
            .expect("failed to inspect Linux probe relocations"),
        "Linux probe .text contains unresolved relocations"
    );
    assert!(
        text.len() <= LINUX_PROBE_MAX_TEXT_SIZE,
        "Linux probe .text is {} bytes; the four-page executable profile permits at most {} bytes",
        text.len(),
        LINUX_PROBE_MAX_TEXT_SIZE
    );
    fs::write(output.join("linux-probe.bin"), text).expect("failed to write Linux probe bytes");

    let interpreter_object = output.join("linux-interpreter.o");
    let clang = env::var_os("CLANG").unwrap_or_else(|| "clang".into());
    let status = Command::new(clang)
        .args([
            "-target",
            "x86_64-none-elf",
            "-c",
            "linux-interpreter.S",
            "-o",
        ])
        .arg(&interpreter_object)
        .status()
        .expect("failed to run clang for Linux interpreter");
    assert!(
        status.success(),
        "clang failed to assemble Linux interpreter"
    );
    let interpreter_elf =
        fs::read(&interpreter_object).expect("failed to read Linux interpreter object");
    let interpreter_text = elf_section(&interpreter_elf, b".text")
        .expect("Linux interpreter object has no .text section");
    assert!(
        !elf_section_has_relocations(&interpreter_elf, b".text")
            .expect("failed to inspect Linux interpreter relocations"),
        "Linux interpreter .text contains unresolved relocations"
    );
    assert!(
        interpreter_text.len() <= LINUX_INTERPRETER_MAX_TEXT_SIZE,
        "Linux interpreter .text is {} bytes; the bounded profile permits at most {} bytes",
        interpreter_text.len(),
        LINUX_INTERPRETER_MAX_TEXT_SIZE
    );
    fs::write(output.join("linux-interpreter.bin"), interpreter_text)
        .expect("failed to write Linux interpreter bytes");

    let dependency_object = output.join("linux-dependency.o");
    let clang = env::var_os("CLANG").unwrap_or_else(|| "clang".into());
    let status = Command::new(&clang)
        .args([
            "-target",
            "x86_64-none-elf",
            "-c",
            "linux-dependency.S",
            "-o",
        ])
        .arg(&dependency_object)
        .status()
        .expect("failed to run clang for Linux dependency");
    assert!(
        status.success(),
        "clang failed to assemble Linux dependency"
    );
    let dependency_elf =
        fs::read(&dependency_object).expect("failed to read Linux dependency object");
    let dependency_text = elf_section(&dependency_elf, b".text")
        .expect("Linux dependency object has no .text section");
    assert!(
        !elf_section_has_relocations(&dependency_elf, b".text")
            .expect("failed to inspect Linux dependency relocations"),
        "Linux dependency .text contains unresolved relocations"
    );
    assert!(
        dependency_text.len() <= LINUX_DEPENDENCY_MAX_TEXT_SIZE,
        "Linux dependency .text is {} bytes; the bounded profile permits at most {} bytes",
        dependency_text.len(),
        LINUX_DEPENDENCY_MAX_TEXT_SIZE
    );
    fs::write(output.join("linux-dependency.bin"), dependency_text)
        .expect("failed to write Linux dependency bytes");

    let linked_main_object = output.join("linux-linked-main.o");
    let status = Command::new(&clang)
        .args([
            "-target",
            "x86_64-none-elf",
            "-DGINUX_LINKED",
            "-c",
            "linux-probe.S",
            "-o",
        ])
        .arg(&linked_main_object)
        .status()
        .expect("failed to assemble linked Linux main probe");
    assert!(
        status.success(),
        "clang failed to assemble linked Linux main probe"
    );
    let linked_dependency_object = output.join("linux-linked-dependency.o");
    let status = Command::new(&clang)
        .args([
            "-target",
            "x86_64-none-elf",
            "-DGINUX_LINKED",
            "-c",
            "linux-dependency.S",
            "-o",
        ])
        .arg(&linked_dependency_object)
        .status()
        .expect("failed to assemble linked Linux dependency");
    assert!(
        status.success(),
        "clang failed to assemble linked Linux dependency"
    );
    let linked_auxiliary_object = output.join("linux-linked-auxiliary.o");
    let status = Command::new(&clang)
        .args([
            "-target",
            "x86_64-none-elf",
            "-c",
            "linux-auxiliary-dependency.S",
            "-o",
        ])
        .arg(&linked_auxiliary_object)
        .status()
        .expect("failed to assemble linked Linux auxiliary dependency");
    assert!(
        status.success(),
        "clang failed to assemble linked Linux auxiliary dependency"
    );
    let linked_interpreter_object = output.join("linux-linked-interpreter.o");
    let status = Command::new(&clang)
        .args([
            "-target",
            "x86_64-none-elf",
            "-DGINUX_LINKED",
            "-c",
            "linux-interpreter.S",
            "-o",
        ])
        .arg(&linked_interpreter_object)
        .status()
        .expect("failed to assemble linked Linux interpreter");
    assert!(
        status.success(),
        "clang failed to assemble linked Linux interpreter"
    );

    let linked_dependency = output.join("libginux.so");
    let linked_dependency_script =
        PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("manifest directory"))
            .join("linux-linked-dependency.ld");
    let (lld, toolchain_library) = linker();
    let mut dependency_link = Command::new(&lld);
    dependency_link
        .args([
            "-flavor",
            "gnu",
            "-m",
            "elf_x86_64",
            "-shared",
            "-nostdlib",
            "--soname",
            "libginux.so",
            "--hash-style=gnu",
            "-T",
        ])
        .arg(&linked_dependency_script)
        .arg(&linked_dependency_object)
        .arg("-o")
        .arg(&linked_dependency);
    configure_linker_library_path(&mut dependency_link, toolchain_library.as_ref());
    let status = dependency_link
        .status()
        .expect("failed to link Linux dependency");
    assert!(status.success(), "LLD failed to link Linux dependency");

    let linked_auxiliary = output.join("libginux-aux.so");
    let linked_auxiliary_script =
        PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("manifest directory"))
            .join("linux-linked-auxiliary.ld");
    let mut auxiliary_link = Command::new(&lld);
    auxiliary_link
        .args([
            "-flavor",
            "gnu",
            "-m",
            "elf_x86_64",
            "-shared",
            "-nostdlib",
            "--soname",
            "libginux-aux.so",
            "--hash-style=gnu",
            "-T",
        ])
        .arg(&linked_auxiliary_script)
        .arg(&linked_auxiliary_object)
        .arg("-o")
        .arg(&linked_auxiliary);
    configure_linker_library_path(&mut auxiliary_link, toolchain_library.as_ref());
    let status = auxiliary_link
        .status()
        .expect("failed to link Linux auxiliary dependency");
    assert!(
        status.success(),
        "LLD failed to link Linux auxiliary dependency"
    );

    let linked_interpreter = output.join("ld-ginux.so");
    let linked_interpreter_script =
        PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("manifest directory"))
            .join("linux-linked-interpreter.ld");
    let mut interpreter_link = Command::new(&lld);
    interpreter_link
        .args([
            "-flavor",
            "gnu",
            "-m",
            "elf_x86_64",
            "-pie",
            "-nostdlib",
            "--hash-style=gnu",
            "-T",
        ])
        .arg(&linked_interpreter_script)
        .arg(&linked_interpreter_object)
        .arg("-o")
        .arg(&linked_interpreter);
    configure_linker_library_path(&mut interpreter_link, toolchain_library.as_ref());
    let status = interpreter_link
        .status()
        .expect("failed to link Linux interpreter");
    assert!(status.success(), "LLD failed to link Linux interpreter");

    let linked_main = output.join("linux-linked-main.elf");
    let linked_main_script =
        PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("manifest directory"))
            .join("linux-linked-main.ld");
    let mut main_link = Command::new(&lld);
    main_link
        .args([
            "-flavor",
            "gnu",
            "-m",
            "elf_x86_64",
            "-pie",
            "-nostdlib",
            "--dynamic-linker",
            "/lib64/ld-ginux.so",
            "--hash-style=sysv",
            "-T",
        ])
        .arg(&linked_main_script)
        .arg(&linked_main_object)
        .arg(&linked_dependency)
        .arg("-o")
        .arg(&linked_main);
    configure_linker_library_path(&mut main_link, toolchain_library.as_ref());
    let status = main_link.status().expect("failed to link Linux main probe");
    assert!(status.success(), "LLD failed to link Linux main probe");

    let linked_graph_main = output.join("linux-linked-graph-main.elf");
    let mut graph_main_link = Command::new(&lld);
    graph_main_link
        .args([
            "-flavor",
            "gnu",
            "-m",
            "elf_x86_64",
            "-pie",
            "-nostdlib",
            "--dynamic-linker",
            "/lib64/ld-ginux.so",
            "--hash-style=sysv",
            "-T",
        ])
        .arg(&linked_main_script)
        .arg(&linked_main_object)
        .arg("--no-as-needed")
        .arg(&linked_dependency)
        .arg(&linked_auxiliary)
        .arg("-o")
        .arg(&linked_graph_main);
    configure_linker_library_path(&mut graph_main_link, toolchain_library.as_ref());
    let status = graph_main_link
        .status()
        .expect("failed to link Linux dependency-graph main probe");
    assert!(
        status.success(),
        "LLD failed to link Linux dependency-graph main probe"
    );
    if let Some(directory) = env::var_os("GINUX_LINUX_DYNAMIC_EXPORT_DIR") {
        let directory = PathBuf::from(directory);
        fs::create_dir_all(&directory).expect("failed to create Linux dynamic export directory");
        fs::copy(&linked_main, directory.join("linux-linked-main.elf"))
            .expect("failed to export linked Linux main");
        fs::copy(
            &linked_graph_main,
            directory.join("linux-linked-graph-main.elf"),
        )
        .expect("failed to export linked Linux dependency-graph main");
        fs::copy(&linked_dependency, directory.join("libginux.so"))
            .expect("failed to export linked Linux dependency");
        fs::copy(&linked_auxiliary, directory.join("libginux-aux.so"))
            .expect("failed to export linked Linux auxiliary dependency");
        fs::copy(&linked_interpreter, directory.join("ld-ginux.so"))
            .expect("failed to export linked Linux interpreter");
    }

    let linked = output.join("linux-probe.elf");
    let link_script = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("manifest directory"))
        .join("linux-probe.ld");
    let (lld, toolchain_library) = linker();
    let mut link = Command::new(lld);
    link.args([
        "-flavor",
        "gnu",
        "-m",
        "elf_x86_64",
        "-static",
        "-nostdlib",
        "-e",
        "ginux_linux_probe",
        "-T",
    ])
    .arg(&link_script)
    .arg(&object)
    .arg("-o")
    .arg(&linked);
    if let Some(directory) = toolchain_library {
        #[cfg(target_os = "macos")]
        link.env("DYLD_LIBRARY_PATH", &directory);
        #[cfg(not(target_os = "macos"))]
        link.env("LD_LIBRARY_PATH", &directory);
    }
    let status = link.status().expect("failed to run LLD for Linux probe");
    assert!(status.success(), "LLD failed to link Linux probe");
    if let Some(destination) = env::var_os("GINUX_LINUX_PROBE_EXPORT") {
        fs::copy(&linked, destination).expect("failed to export linked Linux probe");
    }
}

fn configure_linker_library_path(command: &mut Command, directory: Option<&PathBuf>) {
    if let Some(directory) = directory {
        #[cfg(target_os = "macos")]
        command.env("DYLD_LIBRARY_PATH", directory);
        #[cfg(not(target_os = "macos"))]
        command.env("LD_LIBRARY_PATH", directory);
    }
}

fn linker() -> (OsString, Option<PathBuf>) {
    if let Some(linker) = env::var_os("LLD") {
        return (linker, None);
    }
    let rustc = env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    let output = Command::new(rustc)
        .args(["--print", "sysroot"])
        .output()
        .expect("failed to discover Rust sysroot");
    assert!(output.status.success(), "rustc did not report its sysroot");
    let sysroot = PathBuf::from(
        String::from_utf8(output.stdout)
            .expect("Rust sysroot was not UTF-8")
            .trim(),
    );
    let host = env::var("HOST").expect("Cargo HOST is set");
    let executable = if cfg!(windows) {
        "rust-lld.exe"
    } else {
        "rust-lld"
    };
    (
        sysroot
            .join("lib/rustlib")
            .join(host)
            .join("bin")
            .join(executable)
            .into_os_string(),
        Some(sysroot.join("lib")),
    )
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

fn elf_section_has_relocations(elf: &[u8], wanted: &[u8]) -> Option<bool> {
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
    let mut target = None;
    for index in 0..count {
        let header = section_offset.checked_add(index.checked_mul(entry_size)?)?;
        let name_offset = usize::try_from(read_u32(elf, header)?).ok()?;
        let name = names.get(name_offset..)?.split(|byte| *byte == 0).next()?;
        if name == wanted {
            target = Some(index);
            break;
        }
    }
    let target = u32::try_from(target?).ok()?;
    for index in 0..count {
        let header = section_offset.checked_add(index.checked_mul(entry_size)?)?;
        let section_type = read_u32(elf, header + 4)?;
        let size = read_u64(elf, header + 32)?;
        let target_index = read_u32(elf, header + 44)?;
        if matches!(section_type, 4 | 9) && size != 0 && target_index == target {
            return Some(true);
        }
    }
    Some(false)
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
