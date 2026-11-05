//! Launch-config handling for the native supervisor.
//!
//! Owner: G-launch. Initial-stack command parsing plus domain-config and
//! input-file staging. Workload definitions stay with the entry points in
//! `main.rs`; later lanes (G01) replace the fixed demo launch values.

use ginux_domain::{ConfigError, DomainConfig};

use crate::supervisor::{
    exit_process, line, write_all, AT_FDCWD, O_RDONLY, SYS_CLOSE_HOST, SYS_OPENAT_HOST,
    SYS_READ_HOST,
};

pub const MAX_ELF_SIZE: usize = 64 * 1024;
pub const MAX_CONFIG_SIZE: usize = 16 * 1024;
pub const MAX_VFS_INDEX_SIZE: usize = 8 * 1024 * 1024;

pub static mut ELF_BYTES: [u8; MAX_ELF_SIZE] = [0; MAX_ELF_SIZE];
pub static mut CONFIG_BYTES: [u8; MAX_CONFIG_SIZE] = [0; MAX_CONFIG_SIZE];
pub static mut VFS_INDEX_BYTES: [u8; MAX_VFS_INDEX_SIZE] = [0; MAX_VFS_INDEX_SIZE];

/// Entry-point command decoded from the initial process stack.
pub enum Command {
    Runtime(*const u8),
    Config(*const u8),
    Lifecycle(*const u8),
    Vfs(*const u8, *const u8, *const u8),
    Native(*const u8, *const u8, usize),
    Domain(*const u8, *const u8),
    Process(*const u8),
    ProcessNative(*const u8, *const u8),
    Devices(*const u8, *const u8),
    DeviceBench(*const u8, *const u8, *const u8),
    KernelViews(*const u8),
}

/// Decode `argv` exactly as the previous inline chain did; `None` selects the
/// default interception demonstration.
pub fn parse_argv(argc: usize, argv: *const usize) -> Option<Command> {
    if argc < 3 {
        return None;
    }
    let command = unsafe { *argv.add(1) as *const u8 };
    if c_string_equals(command, b"runtime") {
        let path = unsafe { *argv.add(2) as *const u8 };
        Some(Command::Runtime(path))
    } else if c_string_equals(command, b"config") {
        let config_path = unsafe { *argv.add(2) as *const u8 };
        Some(Command::Config(config_path))
    } else if c_string_equals(command, b"lifecycle") {
        let config_path = unsafe { *argv.add(2) as *const u8 };
        Some(Command::Lifecycle(config_path))
    } else if argc >= 5 && c_string_equals(command, b"vfs") {
        let config_path = unsafe { *argv.add(2) as *const u8 };
        let rootfs_path = unsafe { *argv.add(3) as *const u8 };
        let index_path = unsafe { *argv.add(4) as *const u8 };
        Some(Command::Vfs(config_path, rootfs_path, index_path))
    } else if argc >= 4 && c_string_equals(command, b"native") {
        let config_path = unsafe { *argv.add(2) as *const u8 };
        let root_path = unsafe { *argv.add(3) as *const u8 };
        let mode = if argc >= 5 && c_string_equals(unsafe { *argv.add(4) as *const u8 }, b"hold") {
            1
        } else if argc >= 5 && c_string_equals(unsafe { *argv.add(4) as *const u8 }, b"probe") {
            2
        } else {
            0
        };
        Some(Command::Native(config_path, root_path, mode))
    } else if argc >= 4 && c_string_equals(command, b"domain") {
        let config_path = unsafe { *argv.add(2) as *const u8 };
        let fixture_path = unsafe { *argv.add(3) as *const u8 };
        Some(Command::Domain(config_path, fixture_path))
    } else if c_string_equals(command, b"process") {
        let config_path = unsafe { *argv.add(2) as *const u8 };
        Some(Command::Process(config_path))
    } else if argc >= 4 && c_string_equals(command, b"process-native") {
        let config_path = unsafe { *argv.add(2) as *const u8 };
        let fixture_path = unsafe { *argv.add(3) as *const u8 };
        Some(Command::ProcessNative(config_path, fixture_path))
    } else if argc >= 4 && c_string_equals(command, b"devices") {
        let config_path = unsafe { *argv.add(2) as *const u8 };
        let block_path = unsafe { *argv.add(3) as *const u8 };
        Some(Command::Devices(config_path, block_path))
    } else if argc >= 5 && c_string_equals(command, b"device-bench") {
        let config_path = unsafe { *argv.add(2) as *const u8 };
        let block_path = unsafe { *argv.add(3) as *const u8 };
        let kind = unsafe { *argv.add(4) as *const u8 };
        Some(Command::DeviceBench(config_path, block_path, kind))
    } else if c_string_equals(command, b"kernel-views") {
        let config_path = unsafe { *argv.add(2) as *const u8 };
        Some(Command::KernelViews(config_path))
    } else {
        None
    }
}

pub fn load_config(path: *const u8) -> DomainConfig {
    let length = read_config(path);
    let bytes = unsafe {
        core::slice::from_raw_parts(core::ptr::addr_of!(CONFIG_BYTES).cast::<u8>(), length)
    };
    match DomainConfig::parse(bytes) {
        Ok(config) => config,
        Err(error) => config_rejected(error),
    }
}

pub fn print_effective_config(config: &DomainConfig) {
    let normalized = match config.normalized() {
        Ok(normalized) => normalized,
        Err(error) => config_rejected(error),
    };
    line(b"GINUX_CONFIG: accepted format=toml schema=1");
    line(b"GINUX_CONFIG_EFFECTIVE_BEGIN");
    write_all(normalized.as_bytes());
    line(b"GINUX_CONFIG_EFFECTIVE_END");
}

pub fn config_rejected(error: ConfigError) -> ! {
    write_all(b"GINUX_CONFIG: rejected reason=");
    write_all(error.as_str().as_bytes());
    line(b" managed-started=false");
    exit_process(2)
}

pub fn read_file(path: *const u8) -> usize {
    use crate::supervisor::{syscall1, syscall3, syscall4};
    let descriptor = unsafe { syscall4(SYS_OPENAT_HOST, AT_FDCWD, path as usize, O_RDONLY, 0) };
    if descriptor < 0 {
        runtime_fail(b"fixture open failed");
    }
    let mut length = 0usize;
    while length < MAX_ELF_SIZE {
        let result = unsafe {
            syscall3(
                SYS_READ_HOST,
                descriptor as usize,
                core::ptr::addr_of_mut!(ELF_BYTES).cast::<u8>().add(length) as usize,
                MAX_ELF_SIZE - length,
            )
        };
        if result < 0 {
            let _ = unsafe { syscall1(SYS_CLOSE_HOST, descriptor as usize) };
            runtime_fail(b"fixture read failed");
        }
        if result == 0 {
            break;
        }
        length += result as usize;
    }
    let _ = unsafe { syscall1(SYS_CLOSE_HOST, descriptor as usize) };
    if length == 0 || length == MAX_ELF_SIZE {
        runtime_fail(b"fixture size invalid");
    }
    length
}

pub fn read_config(path: *const u8) -> usize {
    use crate::supervisor::{syscall1, syscall3, syscall4};
    let descriptor = unsafe { syscall4(SYS_OPENAT_HOST, AT_FDCWD, path as usize, O_RDONLY, 0) };
    if descriptor < 0 {
        config_rejected(ConfigError::InvalidValue);
    }
    let mut length = 0usize;
    while length < MAX_CONFIG_SIZE {
        let result = unsafe {
            syscall3(
                SYS_READ_HOST,
                descriptor as usize,
                core::ptr::addr_of_mut!(CONFIG_BYTES)
                    .cast::<u8>()
                    .add(length) as usize,
                MAX_CONFIG_SIZE - length,
            )
        };
        if result < 0 {
            let _ = unsafe { syscall1(SYS_CLOSE_HOST, descriptor as usize) };
            config_rejected(ConfigError::InvalidValue);
        }
        if result == 0 {
            break;
        }
        length += result as usize;
    }
    let _ = unsafe { syscall1(SYS_CLOSE_HOST, descriptor as usize) };
    if length == 0 || length == MAX_CONFIG_SIZE {
        config_rejected(ConfigError::ValueTooLong);
    }
    length
}

pub fn read_index(path: *const u8) -> usize {
    use crate::supervisor::{syscall1, syscall3, syscall4};
    let descriptor = unsafe { syscall4(SYS_OPENAT_HOST, AT_FDCWD, path as usize, O_RDONLY, 0) };
    if descriptor < 0 {
        runtime_fail(b"VFS index open failed");
    }
    let mut length = 0usize;
    while length < MAX_VFS_INDEX_SIZE {
        let result = unsafe {
            syscall3(
                SYS_READ_HOST,
                descriptor as usize,
                core::ptr::addr_of_mut!(VFS_INDEX_BYTES)
                    .cast::<u8>()
                    .add(length) as usize,
                MAX_VFS_INDEX_SIZE - length,
            )
        };
        if result < 0 {
            let _ = unsafe { syscall1(SYS_CLOSE_HOST, descriptor as usize) };
            runtime_fail(b"VFS index read failed");
        }
        if result == 0 {
            break;
        }
        length += result as usize;
    }
    let _ = unsafe { syscall1(SYS_CLOSE_HOST, descriptor as usize) };
    if length == 0 || length == MAX_VFS_INDEX_SIZE {
        runtime_fail(b"VFS index size invalid");
    }
    length
}

fn runtime_fail(reason: &[u8]) -> ! {
    write_all(b"GATE3: FAIL: ");
    line(reason);
    exit_process(1)
}

pub fn c_string_equals(value: *const u8, expected: &[u8]) -> bool {
    for (index, byte) in expected.iter().enumerate() {
        if unsafe { *value.add(index) } != *byte {
            return false;
        }
    }
    unsafe { *value.add(expected.len()) == 0 }
}
