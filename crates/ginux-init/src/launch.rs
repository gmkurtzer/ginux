//! Launch-config handling for the native supervisor.
//!
//! Owner: G-launch. Initial-stack command parsing plus instance-config and
//! input-file staging. Workload definitions stay with the entry points in
//! `main.rs`; `launch` runs the executable and arguments from instance config.

use ginux_core::{
    balanced_identity_result, balanced_mediated_result, balanced_surface, Action, SYS_EXIT,
    SYS_EXIT_GROUP,
};
use ginux_instance::{ConfigError, ExecutionBackend, ExecutionProfile, Instance, InstanceConfig};

use crate::dispatch::{
    balanced_surface_name, decide_native, native_arguments_allowed, NativeDecision,
};
use crate::supervisor::{
    exit_process, line, managed_host_identity, ptrace, standard_stream_refusal, stopped, syscall0,
    syscall1, syscall2, syscall3, syscall4, syscall5, virtual_sysinfo, wait, write_all,
    write_tracee_bytes, UserRegs, AT_FDCWD, O_RDONLY, PR_SET_NO_NEW_PRIVS, PTRACE_GETREGS,
    PTRACE_O_EXITKILL, PTRACE_O_TRACESYSGOOD, PTRACE_SETOPTIONS, PTRACE_SETREGS, PTRACE_SYSCALL,
    PTRACE_TRACEME, SYS_CHDIR, SYS_CHROOT, SYS_CLOSE_HOST, SYS_EXECVE, SYS_EXIT_HOST, SYS_FORK,
    SYS_FSYNC, SYS_MKDIRAT_HOST, SYS_OPENAT_HOST, SYS_PRCTL, SYS_PTRACE, SYS_READ_HOST, SYS_SETGID,
    SYS_SETGROUPS, SYS_SETUID, SYS_WRITE_HOST,
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
    ConfigMerge(*const u8, usize, *const usize),
    InstancePlan(*const u8),
    InstanceValidate(*const u8),
    InstanceExplain(*const u8, *const u8),
    InstanceDiff(*const u8, *const u8),
    InstanceInspect(*const u8),
    HookPlan(*const u8),
    Lifecycle(*const u8),
    Vfs(*const u8, *const u8, *const u8),
    Native(*const u8, *const u8, usize),
    NativeHook(*const u8, *const u8, usize),
    Instance(*const u8, *const u8),
    Process(*const u8),
    ProcessNative(*const u8, *const u8),
    Devices(*const u8, *const u8),
    DeviceBench(*const u8, *const u8, *const u8),
    KernelViews(*const u8),
    Launch(*const u8, *const u8, *const u8),
    LaunchUsage,
    Usage,
}

/// Decode `argv`. `None` (no arguments) selects the default interception
/// demonstration; an unknown or incomplete command is a usage error and never
/// falls through to the demonstration.
pub fn parse_argv(argc: usize, argv: *const usize) -> Option<Command> {
    if argc < 2 {
        return None;
    }
    let command = unsafe { *argv.add(1) as *const u8 };
    if c_string_equals(command, b"launch") && argc < 4 {
        return Some(Command::LaunchUsage);
    }
    if argc < 3 {
        return Some(Command::Usage);
    }
    if c_string_equals(command, b"runtime") {
        let path = unsafe { *argv.add(2) as *const u8 };
        Some(Command::Runtime(path))
    } else if c_string_equals(command, b"config") {
        let config_path = unsafe { *argv.add(2) as *const u8 };
        Some(Command::Config(config_path))
    } else if c_string_equals(command, b"config-merge") {
        let config_path = unsafe { *argv.add(2) as *const u8 };
        Some(Command::ConfigMerge(config_path, argc - 3, unsafe {
            argv.add(3)
        }))
    } else if c_string_equals(command, b"instance-plan") {
        let config_path = unsafe { *argv.add(2) as *const u8 };
        Some(Command::InstancePlan(config_path))
    } else if c_string_equals(command, b"instance-validate") {
        let config_path = unsafe { *argv.add(2) as *const u8 };
        Some(Command::InstanceValidate(config_path))
    } else if argc >= 4 && c_string_equals(command, b"instance-explain") {
        let config_path = unsafe { *argv.add(2) as *const u8 };
        let operation = unsafe { *argv.add(3) as *const u8 };
        Some(Command::InstanceExplain(config_path, operation))
    } else if argc >= 4 && c_string_equals(command, b"instance-diff") {
        let left = unsafe { *argv.add(2) as *const u8 };
        let right = unsafe { *argv.add(3) as *const u8 };
        Some(Command::InstanceDiff(left, right))
    } else if c_string_equals(command, b"instance-inspect") {
        let config_path = unsafe { *argv.add(2) as *const u8 };
        Some(Command::InstanceInspect(config_path))
    } else if c_string_equals(command, b"hook-plan") {
        let config_path = unsafe { *argv.add(2) as *const u8 };
        Some(Command::HookPlan(config_path))
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
        } else if argc >= 5 && c_string_equals(unsafe { *argv.add(4) as *const u8 }, b"configured")
        {
            3
        } else {
            0
        };
        Some(Command::Native(config_path, root_path, mode))
    } else if argc >= 5 && c_string_equals(command, b"native-hook") {
        let config_path = unsafe { *argv.add(2) as *const u8 };
        let root_path = unsafe { *argv.add(3) as *const u8 };
        let index = parse_decimal_c_string(unsafe { *argv.add(4) as *const u8 })?;
        Some(Command::NativeHook(config_path, root_path, index))
    } else if argc >= 4 && c_string_equals(command, b"instance") {
        let config_path = unsafe { *argv.add(2) as *const u8 };
        let fixture_path = unsafe { *argv.add(3) as *const u8 };
        Some(Command::Instance(config_path, fixture_path))
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
    } else if argc >= 4 && c_string_equals(command, b"launch") {
        let config_path = unsafe { *argv.add(2) as *const u8 };
        let root_path = unsafe { *argv.add(3) as *const u8 };
        if argc == 4 {
            Some(Command::Launch(config_path, root_path, core::ptr::null()))
        } else if argc == 6
            && c_string_equals(unsafe { *argv.add(4) as *const u8 }, b"--result-file")
            && unsafe { *(*argv.add(5) as *const u8) } != 0
        {
            let result_path = unsafe { *argv.add(5) as *const u8 };
            Some(Command::Launch(config_path, root_path, result_path))
        } else {
            Some(Command::LaunchUsage)
        }
    } else {
        Some(Command::Usage)
    }
}

fn parse_decimal_c_string(value: *const u8) -> Option<usize> {
    let bytes = c_string_bytes(value, 20).ok()?;
    if bytes.is_empty() || !bytes.iter().all(u8::is_ascii_digit) {
        return None;
    }
    let mut result = 0usize;
    for byte in bytes {
        result = result
            .checked_mul(10)?
            .checked_add(usize::from(*byte - b'0'))?;
    }
    Some(result)
}

pub fn load_config(path: *const u8) -> InstanceConfig {
    let length = read_config(path);
    let bytes = unsafe {
        core::slice::from_raw_parts(core::ptr::addr_of!(CONFIG_BYTES).cast::<u8>(), length)
    };
    match InstanceConfig::parse(bytes) {
        Ok(config) => config,
        Err(error) => config_rejected(error),
    }
}

pub fn print_effective_config(config: &InstanceConfig) {
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

pub fn read_file(path: *const u8) -> Result<usize, &'static [u8]> {
    use crate::supervisor::{syscall1, syscall3, syscall4};
    let descriptor = unsafe { syscall4(SYS_OPENAT_HOST, AT_FDCWD, path as usize, O_RDONLY, 0) };
    if descriptor < 0 {
        return Err(b"fixture open failed");
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
            return Err(b"fixture read failed");
        }
        if result == 0 {
            break;
        }
        length += result as usize;
    }
    let _ = unsafe { syscall1(SYS_CLOSE_HOST, descriptor as usize) };
    if length == 0 || length == MAX_ELF_SIZE {
        return Err(b"fixture size invalid");
    }
    Ok(length)
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

pub fn read_index(path: *const u8) -> Result<usize, &'static [u8]> {
    use crate::supervisor::{syscall1, syscall3, syscall4};
    let descriptor = unsafe { syscall4(SYS_OPENAT_HOST, AT_FDCWD, path as usize, O_RDONLY, 0) };
    if descriptor < 0 {
        return Err(b"VFS index open failed");
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
            return Err(b"VFS index read failed");
        }
        if result == 0 {
            break;
        }
        length += result as usize;
    }
    let _ = unsafe { syscall1(SYS_CLOSE_HOST, descriptor as usize) };
    if length == 0 || length == MAX_VFS_INDEX_SIZE {
        return Err(b"VFS index size invalid");
    }
    Ok(length)
}

pub fn c_string_equals(value: *const u8, expected: &[u8]) -> bool {
    for (index, byte) in expected.iter().enumerate() {
        if unsafe { *value.add(index) } != *byte {
            return false;
        }
    }
    unsafe { *value.add(expected.len()) == 0 }
}

pub fn c_string_bytes(value: *const u8, limit: usize) -> Result<&'static [u8], ConfigError> {
    let mut length = 0usize;
    while length < limit {
        if unsafe { *value.add(length) } == 0 {
            return Ok(unsafe { core::slice::from_raw_parts(value, length) });
        }
        length += 1;
    }
    Err(ConfigError::ValueTooLong)
}

// ---- G01: configured native launch ----
//
// General path that executes the configured instance init with its configured
// arguments, environment, and working directory under the existing balanced
// ptrace-native supervision. The fixed Bash-only `native` demonstration is
// untouched; this path is additive behind the `launch` command.
//
// Supported bootstrap: privileged chroot into the given host rootfs path,
// then privilege drop to the instance UID/GID. Recipe never requires host
// root: when the mechanism is unavailable the launcher reports an explicit
// startup failure instead of falling back to host execution.

pub const MAX_LAUNCH_ARG_LEN: usize = 128;
pub const MAX_LAUNCH_ENV_LEN: usize = 256;
pub const MAX_LAUNCH_PATH_LEN: usize = 128;

static mut LAUNCH_EXEC: [u8; 129] = [0; 129];
static mut LAUNCH_ARGS: [u8; 7 * 129] = [0; 7 * 129];
static mut LAUNCH_ENV: [u8; 8 * 257] = [0; 8 * 257];
static mut LAUNCH_WORKDIR: [u8; 129] = [0; 129];
static mut LAUNCH_ARGV: [*const u8; 9] = [core::ptr::null(); 9];
static mut LAUNCH_ENVP: [*const u8; 9] = [core::ptr::null(); 9];

pub fn launch_main(config_path: *const u8, root_path: *const u8, result_path: *const u8) -> ! {
    stage_result_path(result_path);
    let config = launch_load_config(config_path);
    launch_validate(&config);
    if let Some(reason) = standard_stream_refusal() {
        launch_startup_fail(reason);
    }
    print_launch_config(&config);
    let mut instance = Instance::create(config);
    if instance.start().is_err() {
        launch_instance_fail(&mut instance, b"invalid launch start transition");
    }
    stage_launch(&config);
    let child = unsafe { syscall0(SYS_FORK) };
    if child < 0 {
        launch_instance_fail(&mut instance, b"launch tracee fork failed");
    }
    if child == 0 {
        launch_tracee(root_path);
    }
    supervise_launch(child as usize, &mut instance)
}

fn launch_validate(config: &InstanceConfig) {
    if config.execution_profile != ExecutionProfile::Balanced
        || config.execution_backend != ExecutionBackend::PtraceNative
    {
        launch_startup_fail(b"launch requires profile=balanced backend=ptrace-native");
    }
    if !config.storage.as_slice().is_empty() || !config.devices.as_slice().is_empty() {
        launch_startup_fail(b"launch does not support storage or device assignments");
    }
    if config.id.as_bytes().is_empty() || config.id.as_bytes().len() > 64 {
        launch_startup_fail(b"instance id length invalid");
    }
    if config.root.as_bytes().is_empty() || config.root.as_bytes().len() > 128 {
        launch_startup_fail(b"instance root length invalid");
    }
    launch_validate_path(
        config.init.as_bytes(),
        b"instance init must be an absolute confined path",
    );
    launch_validate_path(
        config.workdir.as_bytes(),
        b"instance workdir must be an absolute confined path",
    );
    if config.args.as_slice().len() > 7 {
        launch_startup_fail(b"instance args exceed 7 entries");
    }
    for argument in config.args.as_slice() {
        if argument.as_bytes().len() > MAX_LAUNCH_ARG_LEN || argument.as_bytes().contains(&0) {
            launch_startup_fail(b"instance arg exceeds 128 bytes");
        }
    }
    if config.environment.as_slice().len() > 8 {
        launch_startup_fail(b"instance environment exceeds 8 entries");
    }
    for variable in config.environment.as_slice() {
        let bytes = variable.as_bytes();
        let equals = bytes.iter().position(|byte| *byte == b'=');
        if bytes.len() > MAX_LAUNCH_ENV_LEN
            || bytes.contains(&0)
            || equals == Some(0)
            || equals.is_none()
        {
            launch_startup_fail(b"instance environment entry must be NAME=value within 256 bytes");
        }
    }
}

fn launch_validate_path(path: &[u8], reason: &[u8]) {
    if path.is_empty()
        || path.len() > MAX_LAUNCH_PATH_LEN
        || path[0] != b'/'
        || path.contains(&0)
        || contains_dotdot(path)
    {
        launch_startup_fail(reason);
    }
}

fn contains_dotdot(path: &[u8]) -> bool {
    path.split(|byte| *byte == b'/').any(|part| part == b"..")
}

fn stage_launch(config: &InstanceConfig) {
    let exec = stage_nul(
        core::ptr::addr_of_mut!(LAUNCH_EXEC).cast::<u8>(),
        129,
        config.init.as_bytes(),
        b"instance init staging failed",
    );
    unsafe {
        core::ptr::addr_of_mut!(LAUNCH_ARGV[0]).write(exec);
    }
    let mut argc = 1usize;
    for argument in config.args.as_slice() {
        let slot = unsafe { core::ptr::addr_of_mut!(LAUNCH_ARGS[(argc - 1) * 129]).cast::<u8>() };
        let staged = stage_nul(
            slot,
            129,
            argument.as_bytes(),
            b"instance arg staging failed",
        );
        unsafe {
            core::ptr::addr_of_mut!(LAUNCH_ARGV[argc]).write(staged);
        }
        argc += 1;
    }
    unsafe {
        core::ptr::addr_of_mut!(LAUNCH_ARGV[argc]).write(core::ptr::null());
    }
    let mut envc = 0usize;
    for variable in config.environment.as_slice() {
        let slot = unsafe { core::ptr::addr_of_mut!(LAUNCH_ENV[envc * 257]).cast::<u8>() };
        let staged = stage_nul(
            slot,
            257,
            variable.as_bytes(),
            b"instance environment staging failed",
        );
        unsafe {
            core::ptr::addr_of_mut!(LAUNCH_ENVP[envc]).write(staged);
        }
        envc += 1;
    }
    unsafe {
        core::ptr::addr_of_mut!(LAUNCH_ENVP[envc]).write(core::ptr::null());
    }
    stage_nul(
        core::ptr::addr_of_mut!(LAUNCH_WORKDIR).cast::<u8>(),
        129,
        config.workdir.as_bytes(),
        b"instance workdir staging failed",
    );
}

fn stage_nul(buffer: *mut u8, capacity: usize, value: &[u8], reason: &[u8]) -> *const u8 {
    if value.len() + 1 > capacity || value.contains(&0) {
        launch_startup_fail(reason);
    }
    unsafe {
        core::ptr::copy_nonoverlapping(value.as_ptr(), buffer, value.len());
        *buffer.add(value.len()) = 0;
    }
    buffer as *const u8
}

// ---- G02: faithful workload completion and streams ----
//
// `--result-file <fresh host path>` (Ginux-owned): after teardown the
// supervisor writes exactly one JSON object to that host-side path:
// {"version":1,"outcome":"exited|signaled|supervisor-error",
//  "code":<i64|null>,"signal":<u64|null>,"generation":null}.
// `generation` is null until retained-storage generations exist. The launcher exit code
// mirrors the workload exit code when the supervisor is healthy (low 8 bits,
// as wait reports it); a terminating signal exits 128+signo (classic shell
// convention); supervisor failure exits 1; startup failure exits 2. The
// result file is authoritative; a missing or malformed result file is a
// protocol failure, never success. The file is created with O_CREAT|O_EXCL
// before launch, so a non-fresh path fails before executing a workload.
// Write, fsync, or close failure exits 1, even if the workload succeeded.
//
// Streams: workload stdout is the tracee's own fd 1, never written by the
// supervisor (the effective-config echo moved to stderr on this path, and
// every audit/diagnostic line below carries the `ginux:` prefix on fd 2).
// Audit volume is bounded (see AUDIT_BYTE_BUDGET).
//
// Startup-failure choice (stated): when --result-file was supplied, startup
// failures (bad config, invalid args, bootstrap unavailable) also write
// outcome=supervisor-error and exit 2. Without the flag no file is created.

pub const MAX_RESULT_PATH_LEN: usize = 512;
pub const AUDIT_BYTE_BUDGET: usize = 64 * 1024;

static mut RESULT_FD: isize = -1;

static mut AUDIT_BYTES: usize = 0;
static mut AUDIT_CAPPED: bool = false;
// One audit record is staged here and written whole once its newline
// arrives, so the budget never splits a record and the truncation notice
// always starts its own line.
const AUDIT_RECORD_CAPACITY: usize = 256;
static mut AUDIT_RECORD: [u8; AUDIT_RECORD_CAPACITY] = [0; AUDIT_RECORD_CAPACITY];
static mut AUDIT_RECORD_LEN: usize = 0;

const OUTCOME_EXITED: u8 = 0;
const OUTCOME_SIGNALED: u8 = 1;
const OUTCOME_SUPERVISOR_ERROR: u8 = 2;

fn c_string_len(value: *const u8) -> usize {
    // Volatile reads: keep LLVM from folding this NUL scan into a `strlen`
    // call, which the freestanding `-nostdlib -static` link does not provide
    // (see the volatile shims in `main.rs`).
    let mut length = 0usize;
    while unsafe { core::ptr::read_volatile(value.add(length)) } != 0 {
        length += 1;
    }
    length
}

fn stage_result_path(path: *const u8) {
    if path.is_null() {
        return;
    }
    let length = c_string_len(path);
    if length == 0 || length > MAX_RESULT_PATH_LEN {
        launch_startup_fail(b"result-file path length invalid");
    }
    // Reserve the result before starting any workload. O_EXCL prevents a
    // stale result being mistaken for this invocation's completion.
    const OPEN_FLAGS: usize = 1 | 64 | 128 | 0o2000000;
    let mut descriptor =
        unsafe { syscall4(SYS_OPENAT_HOST, AT_FDCWD, path as usize, OPEN_FLAGS, 0o600) };
    if descriptor < 0 {
        launch_startup_fail(b"result-file path is not fresh or not creatable");
    }
    if descriptor < 3 {
        // Keep the result separate from even closed stdin/stdout/stderr.
        const SYS_FCNTL: usize = 72;
        const F_DUPFD_CLOEXEC: usize = 1030;
        let duplicate = unsafe { syscall3(SYS_FCNTL, descriptor as usize, F_DUPFD_CLOEXEC, 3) };
        let _ = unsafe { syscall1(SYS_CLOSE_HOST, descriptor as usize) };
        if duplicate < 0 {
            launch_startup_fail(b"result-file descriptor reservation failed");
        }
        descriptor = duplicate;
    }
    unsafe {
        RESULT_FD = descriptor;
    }
}

fn launch_rejected(reason: &str) -> ! {
    write_stderr(b"ginux: launch config rejected reason=");
    write_stderr(reason.as_bytes());
    write_stderr(b"\n");
    write_completion_result(OUTCOME_SUPERVISOR_ERROR, false, 0, false, 0);
    exit_process(2)
}

fn launch_read_config(path: *const u8) -> usize {
    let descriptor = unsafe { syscall4(SYS_OPENAT_HOST, AT_FDCWD, path as usize, O_RDONLY, 0) };
    if descriptor < 0 {
        launch_rejected("config open failed");
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
            launch_rejected("config read failed");
        }
        if result == 0 {
            break;
        }
        length += result as usize;
    }
    let _ = unsafe { syscall1(SYS_CLOSE_HOST, descriptor as usize) };
    if length == 0 || length == MAX_CONFIG_SIZE {
        launch_rejected("config size invalid");
    }
    length
}

fn launch_load_config(path: *const u8) -> InstanceConfig {
    let length = launch_read_config(path);
    let bytes = unsafe {
        core::slice::from_raw_parts(core::ptr::addr_of!(CONFIG_BYTES).cast::<u8>(), length)
    };
    match InstanceConfig::parse(bytes) {
        Ok(config) => config,
        Err(error) => launch_rejected(error.as_str()),
    }
}

fn print_launch_config(config: &InstanceConfig) {
    let normalized = match config.normalized() {
        Ok(normalized) => normalized,
        Err(error) => launch_rejected(error.as_str()),
    };
    line_stderr(b"ginux: config accepted format=toml schema=1");
    line_stderr(b"ginux: effective config begin");
    for line in normalized.as_bytes().split(|byte| *byte == b'\n') {
        if !line.is_empty() {
            write_stderr(b"ginux: ");
            line_stderr(line);
        }
    }
    line_stderr(b"ginux: effective config end");
}

pub fn usage() -> ! {
    line_stderr(b"ginux: usage: ginux (no arguments: interception demonstration)");
    line_stderr(
        b"ginux: usage: ginux config|instance-plan|instance-validate|instance-inspect|lifecycle|process|kernel-views <config>",
    );
    line_stderr(b"ginux: usage: ginux instance-explain <config> <operation>");
    line_stderr(b"ginux: usage: ginux instance-diff <left-config> <right-config>");
    line_stderr(b"ginux: usage: ginux runtime <elf>");
    line_stderr(b"ginux: usage: ginux instance|process-native <config> <fixture>");
    line_stderr(b"ginux: usage: ginux native <config> <root> [configured|hold|probe]");
    line_stderr(b"ginux: usage: ginux devices <config> <block>");
    line_stderr(b"ginux: usage: ginux device-bench <config> <block> <kind>");
    line_stderr(b"ginux: usage: ginux vfs <config> <rootfs> <index>");
    line_stderr(b"ginux: usage: ginux launch <config> <root> [--result-file <fresh host path>]");
    exit_process(2)
}

pub fn launch_usage() -> ! {
    line_stderr(b"ginux: launch usage: launch <config> <root> [--result-file <fresh host path>]");
    exit_process(2)
}

fn audit_write(bytes: &[u8]) {
    let record = unsafe {
        if AUDIT_CAPPED {
            return;
        }
        let start = AUDIT_RECORD_LEN;
        if start + bytes.len() > AUDIT_RECORD_CAPACITY {
            // No record is this long; stop auditing rather than split one.
            audit_cap();
            return;
        }
        let record = core::slice::from_raw_parts_mut(
            core::ptr::addr_of_mut!(AUDIT_RECORD).cast::<u8>(),
            AUDIT_RECORD_CAPACITY,
        );
        record[start..start + bytes.len()].copy_from_slice(bytes);
        AUDIT_RECORD_LEN = start + bytes.len();
        if bytes.last() != Some(&b'\n') {
            return;
        }
        let length = AUDIT_RECORD_LEN;
        AUDIT_RECORD_LEN = 0;
        if AUDIT_BYTES + length > AUDIT_BYTE_BUDGET {
            audit_cap();
            return;
        }
        AUDIT_BYTES += length;
        &record[..length]
    };
    write_stderr(record);
}

fn audit_cap() {
    unsafe {
        AUDIT_CAPPED = true;
        AUDIT_RECORD_LEN = 0;
    }
    write_stderr(b"ginux: audit truncated budget=65536\n");
}

fn audit_line(message: &[u8]) {
    audit_write(message);
    audit_write(b"\n");
}

fn audit_u64(mut value: u64) {
    let mut buffer = [0u8; 20];
    let mut index = buffer.len();
    if value == 0 {
        audit_write(b"0");
        return;
    }
    while value != 0 {
        index -= 1;
        buffer[index] = b'0' + (value % 10) as u8;
        value /= 10;
    }
    audit_write(&buffer[index..]);
}

fn audit_i64(value: i64) {
    if value < 0 {
        audit_write(b"-");
        audit_u64(value.unsigned_abs());
    } else {
        audit_u64(value as u64);
    }
}

fn push_u64(buffer: &mut [u8], mut position: usize, mut value: u64) -> usize {
    let mut digits = [0u8; 20];
    let mut count = 0usize;
    if value == 0 {
        digits[0] = b'0';
        count = 1;
    } else {
        while value != 0 {
            digits[count] = b'0' + (value % 10) as u8;
            count += 1;
            value /= 10;
        }
    }
    let mut index = count;
    while index > 0 {
        index -= 1;
        if position < buffer.len() {
            buffer[position] = digits[index];
            position += 1;
        }
    }
    position
}

fn push_bytes(buffer: &mut [u8], mut position: usize, bytes: &[u8]) -> usize {
    for byte in bytes {
        if position < buffer.len() {
            buffer[position] = *byte;
            position += 1;
        }
    }
    position
}

fn write_completion_result(outcome: u8, has_code: bool, code: u64, has_signal: bool, signal: u64) {
    let descriptor = unsafe { RESULT_FD };
    if descriptor < 0 {
        return;
    }
    unsafe {
        RESULT_FD = -1;
    }
    let mut record = [0u8; 160];
    let mut position = 0usize;
    position = push_bytes(&mut record, position, b"{\"version\":1,\"outcome\":\"");
    position = push_bytes(
        &mut record,
        position,
        match outcome {
            OUTCOME_SIGNALED => b"signaled",
            OUTCOME_SUPERVISOR_ERROR => b"supervisor-error",
            _ => b"exited",
        },
    );
    position = push_bytes(&mut record, position, b"\",\"code\":");
    if has_code {
        position = push_u64(&mut record, position, code);
    } else {
        position = push_bytes(&mut record, position, b"null");
    }
    position = push_bytes(&mut record, position, b",\"signal\":");
    if has_signal {
        position = push_u64(&mut record, position, signal);
    } else {
        position = push_bytes(&mut record, position, b"null");
    }
    position = push_bytes(&mut record, position, b",\"generation\":null}\n");
    let mut offset = 0usize;
    while offset < position {
        let written = unsafe {
            syscall3(
                SYS_WRITE_HOST,
                descriptor as usize,
                record.as_ptr().add(offset) as usize,
                position - offset,
            )
        };
        if written == -4 {
            continue;
        }
        if written <= 0 {
            let _ = unsafe { syscall1(SYS_CLOSE_HOST, descriptor as usize) };
            line_stderr(b"ginux: result-file write failed");
            exit_process(1);
        }
        offset += written as usize;
    }
    let synced = loop {
        let result = unsafe { syscall1(SYS_FSYNC, descriptor as usize) };
        if result != -4 {
            break result;
        }
    };
    let closed = unsafe { syscall1(SYS_CLOSE_HOST, descriptor as usize) };
    if synced < 0 || closed < 0 {
        line_stderr(b"ginux: result-file sync or close failed");
        exit_process(1);
    }
}

fn finish_exited(code: u64) {
    line_stderr(b"ginux: launch supervisor=attached direct-unsupervised-syscalls=0");
    write_stderr(b"ginux: launch complete workload-exit-code=");
    write_stderr_u64(code);
    write_stderr(b"\n");
    write_completion_result(OUTCOME_EXITED, true, code, false, 0);
    exit_process(code as usize)
}

fn finish_signaled(signal: u64) {
    line_stderr(b"ginux: launch supervisor=attached direct-unsupervised-syscalls=0");
    write_stderr(b"ginux: launch complete workload-signal=");
    write_stderr_u64(signal);
    write_stderr(b"\n");
    write_completion_result(OUTCOME_SIGNALED, false, 0, true, signal);
    exit_process(128 + signal as usize)
}

fn launch_tracee(root_path: *const u8) -> ! {
    static ROOT: &[u8] = b"/\0";
    // Host descriptors (including the result file) must not grant the
    // workload authority outside the supplied root. Only stdio is shared.
    const SYS_CLOSE_RANGE: usize = 436;
    if unsafe { syscall3(SYS_CLOSE_RANGE, 3, u32::MAX as usize, 0) } < 0 {
        tracee_setup_fail(
            b"ginux: launch bootstrap failed: descriptor cleanup failed\n",
            129,
        );
    }
    if unsafe { syscall1(SYS_CHROOT, root_path as usize) } < 0 {
        tracee_setup_fail(
            b"ginux: launch bootstrap unavailable: chroot failed (host privilege required)\n",
            120,
        );
    }
    if unsafe { syscall1(SYS_CHDIR, ROOT.as_ptr() as usize) } < 0 {
        tracee_setup_fail(
            b"ginux: launch bootstrap failed: root directory unavailable\n",
            121,
        );
    }
    let workdir = core::ptr::addr_of!(LAUNCH_WORKDIR).cast::<u8>();
    let _ = unsafe { syscall3(SYS_MKDIRAT_HOST, AT_FDCWD, workdir as usize, 0o755) };
    if unsafe { syscall4(SYS_PTRACE, PTRACE_TRACEME, 0, 0, 0) } < 0 {
        tracee_setup_fail(
            b"ginux: launch bootstrap unavailable: ptrace traceme failed\n",
            122,
        );
    }
    let host_identity = managed_host_identity();
    if host_identity.drop_root {
        if unsafe { syscall2(SYS_SETGROUPS, 0, 0) } < 0 {
            tracee_setup_fail(
                b"ginux: launch bootstrap failed: supplementary groups drop failed\n",
                123,
            );
        }
        if unsafe { syscall1(SYS_SETGID, host_identity.gid) } < 0 {
            tracee_setup_fail(
                b"ginux: launch bootstrap failed: group privilege drop failed\n",
                124,
            );
        }
        if unsafe { syscall1(SYS_SETUID, host_identity.uid) } < 0 {
            tracee_setup_fail(
                b"ginux: launch bootstrap failed: user privilege drop failed\n",
                125,
            );
        }
    }
    if unsafe { syscall5(SYS_PRCTL, PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) } < 0 {
        tracee_setup_fail(
            b"ginux: launch bootstrap failed: no-new-privileges failed\n",
            126,
        );
    }
    if unsafe { syscall1(SYS_CHDIR, workdir as usize) } < 0 {
        tracee_setup_fail(
            b"ginux: launch startup failure: configured instance workdir unavailable\n",
            128,
        );
    }
    unsafe {
        syscall3(
            SYS_EXECVE,
            core::ptr::addr_of!(LAUNCH_EXEC) as usize,
            core::ptr::addr_of!(LAUNCH_ARGV) as usize,
            core::ptr::addr_of!(LAUNCH_ENVP) as usize,
        );
        write_stderr(b"ginux: launch startup failure: exec of configured instance init failed\n");
        syscall1(SYS_EXIT_HOST, 127);
    }
    loop {
        core::hint::spin_loop();
    }
}

fn tracee_setup_fail(message: &[u8], code: usize) -> ! {
    write_stderr(message);
    unsafe { syscall1(SYS_EXIT_HOST, code) };
    loop {
        core::hint::spin_loop();
    }
}

fn supervise_launch(pid: usize, instance: &mut Instance) -> ! {
    let mut status = 0i32;
    if wait(pid, &mut status) < 0 {
        launch_fail(instance, pid, b"launch tracee wait after exec failed");
    }
    if launch_exited(status) {
        launch_early_fail(instance, launch_exit_reason(launch_exit_code(status)));
    }
    if !stopped(status) || (status >> 8) & 0xff != 5 {
        launch_fail(instance, pid, b"launch tracee did not stop after exec");
    }
    let options = PTRACE_O_TRACESYSGOOD | PTRACE_O_EXITKILL;
    if ptrace(PTRACE_SETOPTIONS, pid, 0, options) < 0 || ptrace(PTRACE_SYSCALL, pid, 0, 0) < 0 {
        launch_fail(instance, pid, b"launch ptrace setup failed");
    }
    line_stderr(b"ginux: launch backend=ptrace-native profile=balanced elf-loader=host");
    line_stderr(b"ginux: launch tracee-uid=1000 tracee-gid=1000 root=host-path-read-only");

    let mut sequence = 0u64;
    let mut awaiting_exit = false;
    let mut override_exit = false;
    let mut override_result = 0i64;
    loop {
        status = 0;
        if wait(pid, &mut status) < 0 {
            launch_fail(instance, pid, b"launch tracee wait failed");
        }
        if stopped(status) {
            let signal = ((status >> 8) & 0xff) as u8;
            if signal != (5 | 0x80) {
                // Group stops cannot be resumed with signal reinjection.
                // Fail explicitly instead of spinning on SIGSTOP forever.
                const PTRACE_GETSIGINFO: usize = 0x4202;
                let mut info = [0u64; 16];
                if ptrace(PTRACE_GETSIGINFO, pid, 0, info.as_mut_ptr() as usize) < 0 {
                    launch_fail(instance, pid, b"launch group stops are unsupported");
                }
                // Signal-delivery stop, not a syscall trap: forward the
                // signal and keep supervising so a terminating signal is
                // reported as outcome=signaled below.
                audit_write(b"ginux: audit phase=signal-forwarded signal=");
                audit_u64(signal as u64);
                audit_write(b"\n");
                if ptrace(PTRACE_SYSCALL, pid, 0, signal as usize) < 0 {
                    launch_fail(instance, pid, b"launch signal forward failed");
                }
                continue;
            }
        } else if launch_exited(status) {
            let code = launch_exit_code(status) as u64;
            if instance.task_exited(true).is_err() {
                launch_fail(instance, pid, b"invalid launch exit transition");
            }
            finish_exited(code);
        } else if launch_signaled(status) {
            let signal = launch_term_signal(status) as u64;
            let _ = instance.task_exited(false);
            finish_signaled(signal);
        } else {
            launch_fail(instance, pid, b"launch tracee escaped supervision");
        }
        let mut registers = core::mem::MaybeUninit::<UserRegs>::uninit();
        if ptrace(PTRACE_GETREGS, pid, 0, registers.as_mut_ptr() as usize) < 0 {
            launch_fail(instance, pid, b"launch register read failed");
        }
        let mut registers = unsafe { registers.assume_init() };

        if awaiting_exit {
            if override_exit {
                registers.rax = override_result as u64;
                if ptrace(PTRACE_SETREGS, pid, 0, &registers as *const _ as usize) < 0 {
                    launch_fail(instance, pid, b"launch result override failed");
                }
            }
            audit_write(b"ginux: audit sequence=");
            audit_u64(sequence);
            audit_write(b" phase=exit result=");
            audit_i64(registers.rax as i64);
            audit_write(b"\n");
            awaiting_exit = false;
            override_exit = false;
            if ptrace(PTRACE_SYSCALL, pid, 0, 0) < 0 {
                launch_fail(instance, pid, b"launch resume failed");
            }
            continue;
        }

        // int 0x80 uses a different syscall table even from a 64-bit ELF.
        // Never classify that ABI using the x86-64 contract's numbers.
        const PTRACE_GET_SYSCALL_INFO: usize = 0x420e;
        let mut syscall_info = [0u64; 11];
        if ptrace(
            PTRACE_GET_SYSCALL_INFO,
            pid,
            core::mem::size_of_val(&syscall_info),
            syscall_info.as_mut_ptr() as usize,
        ) < 24
            || syscall_info[0] as u8 != 1
            || (syscall_info[0] >> 32) as u32 != 0xc000003e
        {
            launch_fail(instance, pid, b"launch requires the x86-64 syscall ABI");
        }
        sequence += 1;
        let syscall = registers.orig_rax;
        let transfer_limit = match syscall {
            0 | 17 => Some(1024 * 1024),
            1 if matches!(registers.rdi, 1 | 2) => Some(4096),
            _ => None,
        };
        if transfer_limit.is_some_and(|limit| registers.rdx > limit) {
            registers.rdx = transfer_limit.unwrap_or(registers.rdx);
            if ptrace(PTRACE_SETREGS, pid, 0, &registers as *const _ as usize) < 0 {
                launch_fail(instance, pid, b"launch transfer clamp failed");
            }
        }
        if syscall == 318 && registers.rsi > 256 {
            registers.rsi = 256;
            if ptrace(PTRACE_SETREGS, pid, 0, &registers as *const _ as usize) < 0 {
                launch_fail(instance, pid, b"launch getrandom clamp failed");
            }
        }
        let decision = match decide_native(syscall, registers.rdi, registers.rsi, registers.rdx) {
            NativeDecision::Contract(decision) => decision,
            NativeDecision::Hooked { lane, result } => {
                let lane_name: &[u8] = match lane {
                    crate::dispatch::Lane::Process => b"process",
                    crate::dispatch::Lane::Filesystem => b"filesystem",
                    crate::dispatch::Lane::Channel => b"channel",
                    crate::dispatch::Lane::Contract => b"contract",
                };
                audit_write(b"ginux: emulate surface=lane-");
                audit_write(lane_name);
                audit_write(b" result=");
                audit_i64(result);
                audit_write(b"\n");
                registers.orig_rax = u64::MAX;
                awaiting_exit = true;
                override_exit = true;
                override_result = result;
                if ptrace(PTRACE_SETREGS, pid, 0, &registers as *const _ as usize) < 0
                    || ptrace(PTRACE_SYSCALL, pid, 0, 0) < 0
                {
                    launch_fail(instance, pid, b"launch lane emulation failed");
                }
                continue;
            }
        };
        audit_write(b"ginux: audit sequence=");
        audit_u64(sequence);
        audit_write(b" phase=entry syscall=");
        audit_u64(syscall);
        audit_write(b" action=");
        match decision.action {
            Action::Emulate if balanced_identity_result(syscall).is_some() => {
                let result = balanced_identity_result(syscall).unwrap_or(0);
                audit_write(b"emulate surface=virtual-identity result=");
                audit_i64(result);
                audit_write(b"\n");
                registers.orig_rax = u64::MAX;
                awaiting_exit = true;
                override_exit = true;
                override_result = result;
                if ptrace(PTRACE_SETREGS, pid, 0, &registers as *const _ as usize) < 0
                    || ptrace(PTRACE_SYSCALL, pid, 0, 0) < 0
                {
                    launch_fail(instance, pid, b"launch getpid emulation failed");
                }
            }
            Action::Emulate if syscall == SYS_EXIT || syscall == SYS_EXIT_GROUP => {
                audit_line(b"emulate result=instance-stop");
                awaiting_exit = true;
                if ptrace(PTRACE_SYSCALL, pid, 0, 0) < 0 {
                    launch_fail(instance, pid, b"launch exit resume failed");
                }
            }
            Action::Mediate if syscall == 99 => {
                let result = write_tracee_bytes(pid, registers.rdi as usize, &virtual_sysinfo())
                    .map_or_else(|error| error, |_| 0);
                audit_write(b"emulate surface=virtual-system result=");
                audit_i64(result);
                audit_write(b"\n");
                registers.orig_rax = u64::MAX;
                awaiting_exit = true;
                override_exit = true;
                override_result = result;
                if ptrace(PTRACE_SETREGS, pid, 0, &registers as *const _ as usize) < 0
                    || ptrace(PTRACE_SYSCALL, pid, 0, 0) < 0
                {
                    launch_fail(instance, pid, b"launch sysinfo emulation failed");
                }
            }
            Action::Mediate if native_arguments_allowed(syscall, &registers) => {
                audit_write(b"mediate surface=");
                audit_line(balanced_surface_name(balanced_surface(syscall)));
                awaiting_exit = true;
                // A host result that is the caller's host tid is replaced
                // at the exit stop.
                if let Some(result) = balanced_mediated_result(syscall) {
                    override_exit = true;
                    override_result = result;
                }
                if ptrace(PTRACE_SYSCALL, pid, 0, 0) < 0 {
                    launch_fail(instance, pid, b"launch mediated syscall failed");
                }
            }
            Action::Mediate => {
                audit_line(b"deny result=-1 reason=invalid-arguments");
                registers.orig_rax = u64::MAX;
                awaiting_exit = true;
                override_exit = true;
                override_result = -1;
                if ptrace(PTRACE_SETREGS, pid, 0, &registers as *const _ as usize) < 0
                    || ptrace(PTRACE_SYSCALL, pid, 0, 0) < 0
                {
                    launch_fail(instance, pid, b"launch argument denial failed");
                }
            }
            Action::Deny(errno) => {
                audit_write(b"deny result=-");
                audit_u64(errno as u64);
                audit_write(b"\n");
                registers.orig_rax = u64::MAX;
                awaiting_exit = true;
                override_exit = true;
                override_result = -(errno as i64);
                if ptrace(PTRACE_SETREGS, pid, 0, &registers as *const _ as usize) < 0
                    || ptrace(PTRACE_SYSCALL, pid, 0, 0) < 0
                {
                    launch_fail(instance, pid, b"launch syscall denial failed");
                }
            }
            _ => launch_fail(instance, pid, b"invalid launch contract decision"),
        }
    }
}

fn launch_exited(status: i32) -> bool {
    status & 0x7f == 0
}

fn launch_exit_code(status: i32) -> i32 {
    (status >> 8) & 0xff
}

fn launch_signaled(status: i32) -> bool {
    let low = status & 0x7f;
    low != 0 && low != 0x7f
}

fn launch_term_signal(status: i32) -> i32 {
    status & 0x7f
}

fn launch_exit_reason(code: i32) -> &'static [u8] {
    match code {
        120 => b"bootstrap unavailable: chroot failed (host privilege required)",
        121 => b"bootstrap failed: root directory unavailable",
        122 => b"bootstrap unavailable: ptrace traceme failed",
        123 => b"bootstrap failed: supplementary groups drop failed",
        124 => b"bootstrap failed: group privilege drop failed",
        125 => b"bootstrap failed: user privilege drop failed",
        126 => b"bootstrap failed: no-new-privileges failed",
        127 => b"exec of configured instance init failed",
        128 => b"configured instance workdir unavailable",
        129 => b"descriptor cleanup failed",
        _ => b"tracee exited before supervision",
    }
}

fn launch_early_fail(instance: &mut Instance, reason: &[u8]) -> ! {
    let _ = instance.task_exited(false);
    launch_startup_fail(reason)
}

fn launch_instance_fail(instance: &mut Instance, reason: &[u8]) -> ! {
    let _ = instance.task_exited(false);
    launch_startup_fail(reason)
}

fn launch_fail(instance: &mut Instance, pid: usize, reason: &[u8]) -> ! {
    let _ = unsafe { syscall2(crate::supervisor::SYS_KILL, pid, crate::supervisor::SIGKILL) };
    let mut status = 0;
    let _ = wait(pid, &mut status);
    let _ = instance.task_exited(false);
    write_stderr(b"ginux: launch supervision failure: ");
    write_stderr(reason);
    write_stderr(b"\n");
    write_completion_result(OUTCOME_SUPERVISOR_ERROR, false, 0, false, 0);
    exit_process(1)
}

fn launch_startup_fail(reason: &[u8]) -> ! {
    write_stderr(b"ginux: launch startup failure: ");
    write_stderr(reason);
    write_stderr(b"\n");
    write_completion_result(OUTCOME_SUPERVISOR_ERROR, false, 0, false, 0);
    exit_process(2)
}

fn write_stderr(bytes: &[u8]) {
    let mut offset = 0;
    while offset < bytes.len() {
        let result = unsafe {
            syscall3(
                SYS_WRITE_HOST,
                2,
                bytes.as_ptr().add(offset) as usize,
                bytes.len() - offset,
            )
        };
        if result <= 0 {
            return;
        }
        offset += result as usize;
    }
}

fn line_stderr(message: &[u8]) {
    write_stderr(message);
    write_stderr(b"\n");
}

fn write_stderr_u64(mut value: u64) {
    let mut buffer = [0u8; 20];
    let mut index = buffer.len();
    if value == 0 {
        write_stderr(b"0");
        return;
    }
    while value != 0 {
        index -= 1;
        buffer[index] = b'0' + (value % 10) as u8;
        value /= 10;
    }
    write_stderr(&buffer[index..]);
}
