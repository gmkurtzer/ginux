use std::{env, ffi::OsString, fs, path::PathBuf, process::Command};

const PAGE_SIZE: usize = 4096;
const LINUX_PROBE_ENTRY_OFFSET: usize = 0x100;
const LINUX_PROBE_EXECUTABLE_PAGES: usize = 5;
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
    println!("cargo:rerun-if-changed=linux-exec-target.S");
    println!("cargo:rerun-if-changed=linux-system-init.S");
    println!("cargo:rerun-if-changed=linux-probe.ld");
    println!("cargo:rerun-if-changed=linux-linked-main.ld");
    println!("cargo:rerun-if-changed=linux-linked-dependency.ld");
    println!("cargo:rerun-if-changed=linux-linked-auxiliary.ld");
    println!("cargo:rerun-if-changed=linux-linked-interpreter.ld");
    println!("cargo:rerun-if-changed=linux-signal-probe.ld");
    println!("cargo:rerun-if-changed=linux-exec-second.ld");
    println!("cargo:rerun-if-changed=linux-system-init.ld");
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
        "Linux probe .text is {} bytes; the five-page executable profile permits at most {} bytes",
        text.len(),
        LINUX_PROBE_MAX_TEXT_SIZE
    );
    fs::write(output.join("linux-probe.bin"), text).expect("failed to write Linux probe bytes");

    let second_probe_object = output.join("linux-probe-second.o");
    let status = Command::new(&clang)
        .args([
            "-target",
            "x86_64-none-elf",
            "-DGINUX_EXEC_SECOND",
            "-c",
            "linux-probe.S",
            "-o",
        ])
        .arg(&second_probe_object)
        .status()
        .expect("failed to run clang for second Linux probe");
    assert!(
        status.success(),
        "clang failed to assemble second Linux probe"
    );
    let second_probe_elf =
        fs::read(&second_probe_object).expect("failed to read second Linux probe object");
    let second_probe_text = elf_section(&second_probe_elf, b".text")
        .expect("second Linux probe object has no .text section");
    assert_eq!(
        second_probe_text.len(),
        text.len(),
        "second Linux probe must preserve the bounded executable layout"
    );
    fs::write(output.join("linux-probe-second.bin"), second_probe_text)
        .expect("failed to write second Linux probe bytes");

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
    let linked_graph_main_object = output.join("linux-linked-graph-main.o");
    let status = Command::new(&clang)
        .args([
            "-target",
            "x86_64-none-elf",
            "-DGINUX_LINKED",
            "-DGINUX_GRAPH",
            "-c",
            "linux-probe.S",
            "-o",
        ])
        .arg(&linked_graph_main_object)
        .status()
        .expect("failed to assemble linked Linux dependency-graph main probe");
    assert!(
        status.success(),
        "clang failed to assemble linked Linux dependency-graph main probe"
    );
    let linked_graph_main_elf = fs::read(&linked_graph_main_object)
        .expect("failed to read linked Linux dependency-graph main object");
    assert_eq!(
        elf_section_relocation_type_count(&linked_graph_main_elf, b".text", 23)
            .expect("failed to inspect linked Linux main TLS relocations"),
        2,
        "linked Linux main must contain exactly two R_X86_64_TPOFF32 references"
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
    let linked_dependency_elf =
        fs::read(&linked_dependency_object).expect("failed to read linked Linux dependency object");
    assert_eq!(
        elf_section_relocation_type_count(&linked_dependency_elf, b".text", 22)
            .expect("failed to inspect linked Linux dependency TLS relocations"),
        1,
        "linked Linux dependency must contain exactly one R_X86_64_GOTTPOFF reference"
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
    let linked_auxiliary_elf = fs::read(&linked_auxiliary_object)
        .expect("failed to read linked Linux auxiliary dependency object");
    assert_eq!(
        elf_section_relocation_type_count(&linked_auxiliary_elf, b".text", 22)
            .expect("failed to inspect linked Linux auxiliary dependency TLS relocations"),
        1,
        "linked Linux auxiliary dependency must contain exactly one R_X86_64_GOTTPOFF reference"
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

    let exec_second_object = output.join("linux-exec-second.o");
    let status = Command::new(&clang)
        .args([
            "-target",
            "x86_64-none-elf",
            "-DGINUX_EXEC_SECOND",
            "-c",
            "linux-exec-target.S",
            "-o",
        ])
        .arg(&exec_second_object)
        .status()
        .expect("failed to assemble second Linux exec target");
    assert!(
        status.success(),
        "clang failed to assemble second Linux exec target"
    );
    let signal_probe_object = output.join("linux-signal-probe.o");
    let status = Command::new(&clang)
        .args([
            "-target",
            "x86_64-none-elf",
            "-DGINUX_SIGNAL_PROBE",
            "-c",
            "linux-exec-target.S",
            "-o",
        ])
        .arg(&signal_probe_object)
        .status()
        .expect("failed to assemble Linux signal probe");
    assert!(
        status.success(),
        "clang failed to assemble Linux signal probe"
    );
    let system_init_object = output.join("linux-system-init.o");
    let status = Command::new(&clang)
        .args([
            "-target",
            "x86_64-none-elf",
            "-c",
            "linux-system-init.S",
            "-o",
        ])
        .arg(&system_init_object)
        .status()
        .expect("failed to assemble minimal Linux system init");
    assert!(
        status.success(),
        "clang failed to assemble minimal Linux system init"
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
            "--init=ginux_dependency_init",
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
            "--init=ginux_auxiliary_init",
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

    let exec_second = output.join("linux-exec-second.elf");
    let exec_second_script =
        PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("manifest directory"))
            .join("linux-exec-second.ld");
    let mut exec_second_link = Command::new(&lld);
    exec_second_link
        .args([
            "-flavor",
            "gnu",
            "-m",
            "elf_x86_64",
            "-static",
            "-nostdlib",
            "-e",
            "ginux_linux_exec_target",
            "-T",
        ])
        .arg(&exec_second_script)
        .arg(&exec_second_object)
        .arg("-o")
        .arg(&exec_second);
    configure_linker_library_path(&mut exec_second_link, toolchain_library.as_ref());
    let status = exec_second_link
        .status()
        .expect("failed to link second Linux exec target");
    assert!(
        status.success(),
        "LLD failed to link second Linux exec target"
    );

    let signal_probe = output.join("linux-signal-probe.elf");
    let signal_probe_script =
        PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("manifest directory"))
            .join("linux-signal-probe.ld");
    let mut signal_probe_link = Command::new(&lld);
    signal_probe_link
        .args([
            "-flavor",
            "gnu",
            "-m",
            "elf_x86_64",
            "-static",
            "-nostdlib",
            "-e",
            "ginux_linux_exec_target",
            "-T",
        ])
        .arg(&signal_probe_script)
        .arg(&signal_probe_object)
        .arg("-o")
        .arg(&signal_probe);
    configure_linker_library_path(&mut signal_probe_link, toolchain_library.as_ref());
    let status = signal_probe_link
        .status()
        .expect("failed to link Linux signal probe");
    assert!(status.success(), "LLD failed to link Linux signal probe");

    let system_init = output.join("linux-system-init.elf");
    let system_init_script =
        PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("manifest directory"))
            .join("linux-system-init.ld");
    let mut system_init_link = Command::new(&lld);
    system_init_link
        .args([
            "-flavor",
            "gnu",
            "-m",
            "elf_x86_64",
            "-static",
            "-nostdlib",
            "-e",
            "ginux_linux_system_init",
            "-T",
        ])
        .arg(&system_init_script)
        .arg(&system_init_object)
        .arg("-o")
        .arg(&system_init);
    configure_linker_library_path(&mut system_init_link, toolchain_library.as_ref());
    let status = system_init_link
        .status()
        .expect("failed to link minimal Linux system init");
    assert!(
        status.success(),
        "LLD failed to link minimal Linux system init"
    );

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
            "--init=ginux_linux_init",
            "-T",
        ])
        .arg(&linked_main_script)
        .arg(&linked_graph_main_object)
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
        fs::copy(&exec_second, directory.join("linux-exec-second.elf"))
            .expect("failed to export second Linux exec target");
        fs::copy(&signal_probe, directory.join("linux-signal-probe.elf"))
            .expect("failed to export Linux signal probe");
        fs::copy(&system_init, directory.join("linux-system-init.elf"))
            .expect("failed to export minimal Linux system init");
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

fn elf_section_relocation_type_count(
    elf: &[u8],
    wanted: &[u8],
    relocation_type: u32,
) -> Option<usize> {
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
    let mut matching = 0;
    for index in 0..count {
        let header = section_offset.checked_add(index.checked_mul(entry_size)?)?;
        let section_type = read_u32(elf, header + 4)?;
        let size = usize::try_from(read_u64(elf, header + 32)?).ok()?;
        let relocation_entry_size = usize::try_from(read_u64(elf, header + 56)?).ok()?;
        let target_index = read_u32(elf, header + 44)?;
        if matches!(section_type, 4 | 9) && size != 0 && target_index == target {
            let minimum_entry_size = if section_type == 4 { 24 } else { 16 };
            if relocation_entry_size < minimum_entry_size || size % relocation_entry_size != 0 {
                return None;
            }
            let offset = usize::try_from(read_u64(elf, header + 24)?).ok()?;
            let relocations = elf.get(offset..offset.checked_add(size)?)?;
            for relocation in relocations.chunks_exact(relocation_entry_size) {
                if read_u64(relocation, 8)? as u32 == relocation_type {
                    matching += 1;
                }
            }
        }
    }
    Some(matching)
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
