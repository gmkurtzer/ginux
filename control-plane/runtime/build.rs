use std::env;
use std::fmt::Write as _;
use std::fs;
use std::path::PathBuf;

fn main() {
    const MATRIX: &str = "../../docs/LINUX-COMPATIBILITY.md";
    println!("cargo:rerun-if-changed={MATRIX}");

    let matrix = fs::read_to_string(MATRIX).expect("read Linux compatibility matrix");
    let mut generated = String::new();
    let mut names = String::new();
    let mut count = 0_usize;
    let mut previous = None;

    for line in matrix.lines() {
        let columns: Vec<_> = line.split('|').map(str::trim).collect();
        if columns.len() < 5 {
            continue;
        }
        let Ok(number) = columns[1].parse::<u16>() else {
            continue;
        };
        let status = match columns[3] {
            "Complete" => "Pass",
            "Partial" | "Missing" => "Fail",
            "Policy-denied" => "PolicyExempt",
            _ => continue,
        };
        assert!(
            previous.is_none_or(|value| number > value),
            "syscall matrix is not ordered"
        );
        assert!(
            columns[2]
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_'),
            "invalid syscall name"
        );
        let name_offset = names.len();
        names.push_str(columns[2]);
        writeln!(
            generated,
            "    LinuxSyscallTest {{ number: {number}, name_offset: {name_offset}, name_length: {}, result: LinuxTestResult::{status} }},",
            columns[2].len()
        )
        .unwrap();
        previous = Some(number);
        count += 1;
    }
    assert_eq!(
        count, 375,
        "Linux syscall catalog size changed; review the runtime test contract"
    );
    generated.insert_str(
        0,
        &format!(
            "const LINUX_SYSCALL_NAMES: &[u8] = b\"{names}\";\nconst LINUX_SYSCALL_TESTS: &[LinuxSyscallTest] = &[\n"
        ),
    );
    generated.push_str("];\n");

    let output =
        PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR")).join("linux_syscall_catalog.rs");
    fs::write(output, generated).expect("write Linux syscall catalog");
}
