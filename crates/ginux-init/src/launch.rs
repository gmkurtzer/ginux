//! Launch-config handling for the native supervisor.
//!
//! Owner: G-launch. Initial-stack command parsing plus domain-config and
//! input-file staging. Workload definitions stay with the entry points in
//! `main.rs`; later lanes (G01) replace the fixed demo launch values.

use ginux_core::{balanced_identity_result, balanced_surface, Action, SYS_EXIT, SYS_EXIT_GROUP};
use ginux_domain::{ConfigError, Domain, DomainConfig};

use crate::dispatch::{balanced_surface_name, decide_native, native_arguments_allowed, NativeDecision};
use crate::supervisor::{
    exit_process, line, ptrace, stopped, syscall0, syscall1, syscall2, syscall3, syscall4,
    syscall5, wait, write_all, AT_FDCWD, DOMAIN_GID, DOMAIN_UID, O_RDONLY, PR_SET_NO_NEW_PRIVS,
    PTRACE_GETREGS, PTRACE_KILL, PTRACE_O_EXITKILL, PTRACE_O_TRACESYSGOOD, PTRACE_SETOPTIONS,
    PTRACE_SETREGS, PTRACE_SYSCALL, PTRACE_TRACEME, SYS_CHDIR, SYS_CHROOT, SYS_CLOSE_HOST,
    SYS_EXECVE, SYS_EXIT_HOST, SYS_FORK, SYS_OPENAT_HOST, SYS_PRCTL, SYS_PTRACE, SYS_READ_HOST,
    SYS_SETGID, SYS_SETGROUPS, SYS_SETUID, SYS_WRITE_HOST, UserRegs,
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
    Launch(*const u8, *const u8),
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
    } else if argc >= 4 && c_string_equals(command, b"launch") {
        let config_path = unsafe { *argv.add(2) as *const u8 };
        let root_path = unsafe { *argv.add(3) as *const u8 };
        Some(Command::Launch(config_path, root_path))
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

// ---- G01: configured native launch ----
//
// General path that executes the configured domain init with its configured
// arguments, environment, and working directory under the existing balanced
// ptrace-native supervision. The fixed Bash-only `native` demonstration is
// untouched; this path is additive behind the `launch` command.
//
// Supported bootstrap: privileged chroot into the given host rootfs path,
// then privilege drop to the domain UID/GID. Recipe never requires host
// root: when the mechanism is unavailable the launcher reports an explicit
// startup failure instead of falling back to host execution.

pub const MAX_LAUNCH_ARG_LEN: usize = 128;
pub const MAX_LAUNCH_ENV_LEN: usize = 256;
pub const MAX_LAUNCH_PATH_LEN: usize = 128;

const SYS_MKDIRAT: usize = 259;

static mut LAUNCH_EXEC: [u8; 129] = [0; 129];
static mut LAUNCH_ARGS: [u8; 7 * 129] = [0; 7 * 129];
static mut LAUNCH_ENV: [u8; 8 * 257] = [0; 8 * 257];
static mut LAUNCH_WORKDIR: [u8; 129] = [0; 129];
static mut LAUNCH_ARGV: [*const u8; 9] = [core::ptr::null(); 9];
static mut LAUNCH_ENVP: [*const u8; 9] = [core::ptr::null(); 9];

pub fn launch_main(config_path: *const u8, root_path: *const u8) -> ! {
    let config = load_config(config_path);
    launch_validate(&config);
    print_effective_config(&config);
    let mut domain = Domain::create(config);
    if domain.start().is_err() {
        launch_domain_fail(&mut domain, b"invalid launch start transition");
    }
    stage_launch(&config);
    let child = unsafe { syscall0(SYS_FORK) };
    if child < 0 {
        launch_domain_fail(&mut domain, b"launch tracee fork failed");
    }
    if child == 0 {
        launch_tracee(root_path);
    }
    supervise_launch(child as usize, &mut domain)
}

fn launch_validate(config: &DomainConfig) {
    if config.id.as_bytes().is_empty() || config.id.as_bytes().len() > 64 {
        launch_startup_fail(b"domain id length invalid");
    }
    if config.root.as_bytes().is_empty() || config.root.as_bytes().len() > 128 {
        launch_startup_fail(b"domain root length invalid");
    }
    launch_validate_path(
        config.init.as_bytes(),
        b"domain init must be an absolute confined path",
    );
    launch_validate_path(
        config.workdir.as_bytes(),
        b"domain workdir must be an absolute confined path",
    );
    if config.args.as_slice().len() > 7 {
        launch_startup_fail(b"domain args exceed 7 entries");
    }
    for argument in config.args.as_slice() {
        if argument.as_bytes().len() > MAX_LAUNCH_ARG_LEN || argument.as_bytes().contains(&0) {
            launch_startup_fail(b"domain arg exceeds 128 bytes");
        }
    }
    if config.environment.as_slice().len() > 8 {
        launch_startup_fail(b"domain environment exceeds 8 entries");
    }
    for variable in config.environment.as_slice() {
        let bytes = variable.as_bytes();
        let equals = bytes.iter().position(|byte| *byte == b'=');
        if bytes.len() > MAX_LAUNCH_ENV_LEN
            || bytes.contains(&0)
            || equals == Some(0)
            || equals.is_none()
        {
            launch_startup_fail(b"domain environment entry must be NAME=value within 256 bytes");
        }
    }
}

fn launch_validate_path(path: &[u8], reason: &[u8]) {
    if path.len() < 2
        || path.len() > MAX_LAUNCH_PATH_LEN
        || path[0] != b'/'
        || path.contains(&0)
        || contains_dotdot(path)
    {
        launch_startup_fail(reason);
    }
}

fn contains_dotdot(path: &[u8]) -> bool {
    let mut index = 0;
    while index + 1 < path.len() {
        if path[index] == b'.' && path[index + 1] == b'.' {
            return true;
        }
        index += 1;
    }
    false
}

fn stage_launch(config: &DomainConfig) {
    let exec = stage_nul(
        core::ptr::addr_of_mut!(LAUNCH_EXEC).cast::<u8>(),
        129,
        config.init.as_bytes(),
        b"domain init staging failed",
    );
    unsafe {
        core::ptr::addr_of_mut!(LAUNCH_ARGV[0]).write(exec);
    }
    let mut argc = 1usize;
    for argument in config.args.as_slice() {
        let slot =
            unsafe { core::ptr::addr_of_mut!(LAUNCH_ARGS[(argc - 1) * 129]).cast::<u8>() };
        let staged = stage_nul(
            slot,
            129,
            argument.as_bytes(),
            b"domain arg staging failed",
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
            b"domain environment staging failed",
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
        b"domain workdir staging failed",
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

fn launch_tracee(root_path: *const u8) -> ! {
    static ROOT: &[u8] = b"/\0";
    if unsafe { syscall1(SYS_CHROOT, root_path as usize) } < 0 {
        tracee_setup_fail(
            b"ginux: launch bootstrap unavailable: chroot failed (host privilege required)\n",
            120,
        );
    }
    if unsafe { syscall1(SYS_CHDIR, ROOT.as_ptr() as usize) } < 0 {
        tracee_setup_fail(b"ginux: launch bootstrap failed: root directory unavailable\n", 121);
    }
    let workdir = core::ptr::addr_of!(LAUNCH_WORKDIR).cast::<u8>();
    let _ = unsafe { syscall3(SYS_MKDIRAT, AT_FDCWD, workdir as usize, 0o755) };
    if unsafe { syscall4(SYS_PTRACE, PTRACE_TRACEME, 0, 0, 0) } < 0 {
        tracee_setup_fail(
            b"ginux: launch bootstrap unavailable: ptrace traceme failed\n",
            122,
        );
    }
    if unsafe { syscall2(SYS_SETGROUPS, 0, 0) } < 0 {
        tracee_setup_fail(
            b"ginux: launch bootstrap failed: supplementary groups drop failed\n",
            123,
        );
    }
    if unsafe { syscall1(SYS_SETGID, DOMAIN_GID) } < 0 {
        tracee_setup_fail(b"ginux: launch bootstrap failed: group privilege drop failed\n", 124);
    }
    if unsafe { syscall1(SYS_SETUID, DOMAIN_UID) } < 0 {
        tracee_setup_fail(b"ginux: launch bootstrap failed: user privilege drop failed\n", 125);
    }
    if unsafe { syscall5(SYS_PRCTL, PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) } < 0 {
        tracee_setup_fail(b"ginux: launch bootstrap failed: no-new-privileges failed\n", 126);
    }
    if unsafe { syscall1(SYS_CHDIR, workdir as usize) } < 0 {
        tracee_setup_fail(
            b"ginux: launch startup failure: configured domain workdir unavailable\n",
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
        write_stderr(b"ginux: launch startup failure: exec of configured domain init failed\n");
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

fn supervise_launch(pid: usize, domain: &mut Domain) -> ! {
    let mut status = 0i32;
    if wait(pid, &mut status) < 0 {
        launch_fail(domain, pid, b"launch tracee wait after exec failed");
    }
    if launch_exited(status) {
        launch_early_fail(domain, pid, launch_exit_reason(launch_exit_code(status)));
    }
    if !stopped(status) {
        launch_fail(domain, pid, b"launch tracee did not stop after exec");
    }
    let options = PTRACE_O_TRACESYSGOOD | PTRACE_O_EXITKILL;
    if ptrace(PTRACE_SETOPTIONS, pid, 0, options) < 0 || ptrace(PTRACE_SYSCALL, pid, 0, 0) < 0 {
        launch_fail(domain, pid, b"launch ptrace setup failed");
    }
    line_stderr(b"ginux: launch backend=ptrace-native profile=balanced elf-loader=host");
    line_stderr(b"ginux: launch tracee-uid=1000 tracee-gid=1000 root=host-path-read-only");

    let mut sequence = 0u64;
    let mut awaiting_exit = false;
    let mut override_exit = false;
    let mut override_result = 0i64;
    loop {
        status = 0;
        if wait(pid, &mut status) < 0 || !stopped(status) {
            launch_fail(domain, pid, b"launch tracee escaped supervision");
        }
        let signal = ((status >> 8) & 0xff) as u8;
        if signal != (5 | 0x80) && signal != 5 {
            launch_fail(domain, pid, b"unexpected launch trace stop");
        }
        let mut registers = core::mem::MaybeUninit::<UserRegs>::uninit();
        if ptrace(PTRACE_GETREGS, pid, 0, registers.as_mut_ptr() as usize) < 0 {
            launch_fail(domain, pid, b"launch register read failed");
        }
        let mut registers = unsafe { registers.assume_init() };

        if awaiting_exit {
            if override_exit {
                registers.rax = override_result as u64;
                if ptrace(PTRACE_SETREGS, pid, 0, &registers as *const _ as usize) < 0 {
                    launch_fail(domain, pid, b"launch result override failed");
                }
            }
            write_stderr(b"ginux: audit sequence=");
            write_stderr_u64(sequence);
            write_stderr(b" phase=exit result=");
            write_stderr_i64(registers.rax as i64);
            write_stderr(b"\n");
            awaiting_exit = false;
            override_exit = false;
            if ptrace(PTRACE_SYSCALL, pid, 0, 0) < 0 {
                launch_fail(domain, pid, b"launch resume failed");
            }
            continue;
        }

        sequence += 1;
        let syscall = registers.orig_rax;
        let decision = match decide_native(syscall, registers.rdi, registers.rsi, registers.rdx) {
            NativeDecision::Contract(decision) => decision,
            NativeDecision::Hooked { lane, result } => {
                let lane_name: &[u8] = match lane {
                    crate::dispatch::Lane::Process => b"process",
                    crate::dispatch::Lane::Filesystem => b"filesystem",
                    crate::dispatch::Lane::Channel => b"channel",
                    crate::dispatch::Lane::Contract => b"contract",
                };
                write_stderr(b"ginux: emulate surface=lane-");
                write_stderr(lane_name);
                write_stderr(b" result=");
                write_stderr_i64(result);
                write_stderr(b"\n");
                registers.orig_rax = u64::MAX;
                awaiting_exit = true;
                override_exit = true;
                override_result = result;
                if ptrace(PTRACE_SETREGS, pid, 0, &registers as *const _ as usize) < 0
                    || ptrace(PTRACE_SYSCALL, pid, 0, 0) < 0
                {
                    launch_fail(domain, pid, b"launch lane emulation failed");
                }
                continue;
            }
        };
        write_stderr(b"ginux: audit sequence=");
        write_stderr_u64(sequence);
        write_stderr(b" phase=entry syscall=");
        write_stderr_u64(syscall);
        write_stderr(b" action=");
        match decision.action {
            Action::Emulate if balanced_identity_result(syscall).is_some() => {
                let result = balanced_identity_result(syscall).unwrap_or(0);
                write_stderr(b"emulate surface=virtual-identity result=");
                write_stderr_i64(result);
                write_stderr(b"\n");
                registers.orig_rax = u64::MAX;
                awaiting_exit = true;
                override_exit = true;
                override_result = result;
                if ptrace(PTRACE_SETREGS, pid, 0, &registers as *const _ as usize) < 0
                    || ptrace(PTRACE_SYSCALL, pid, 0, 0) < 0
                {
                    launch_fail(domain, pid, b"launch getpid emulation failed");
                }
            }
            Action::Emulate if syscall == SYS_EXIT || syscall == SYS_EXIT_GROUP => {
                line_stderr(b"emulate result=domain-stop");
                let code = registers.rdi;
                let _ = ptrace(PTRACE_KILL, pid, 0, 0);
                let _ = wait(pid, &mut status);
                if domain.task_exited(true).is_err() {
                    launch_fail(domain, pid, b"invalid launch exit transition");
                }
                line_stderr(b"ginux: launch supervisor=attached direct-unsupervised-syscalls=0");
                write_stderr(b"ginux: launch complete workload-exit-code=");
                write_stderr_u64(code);
                write_stderr(b"\n");
                exit_process(0)
            }
            Action::Mediate if native_arguments_allowed(syscall, &registers) => {
                write_stderr(b"mediate surface=");
                line_stderr(balanced_surface_name(balanced_surface(syscall)));
                awaiting_exit = true;
                if ptrace(PTRACE_SYSCALL, pid, 0, 0) < 0 {
                    launch_fail(domain, pid, b"launch mediated syscall failed");
                }
            }
            Action::Mediate => {
                line_stderr(b"deny result=-1 reason=invalid-arguments");
                registers.orig_rax = u64::MAX;
                awaiting_exit = true;
                override_exit = true;
                override_result = -1;
                if ptrace(PTRACE_SETREGS, pid, 0, &registers as *const _ as usize) < 0
                    || ptrace(PTRACE_SYSCALL, pid, 0, 0) < 0
                {
                    launch_fail(domain, pid, b"launch argument denial failed");
                }
            }
            Action::Deny(errno) => {
                write_stderr(b"deny result=-");
                write_stderr_u64(errno as u64);
                write_stderr(b"\n");
                registers.orig_rax = u64::MAX;
                awaiting_exit = true;
                override_exit = true;
                override_result = -(errno as i64);
                if ptrace(PTRACE_SETREGS, pid, 0, &registers as *const _ as usize) < 0
                    || ptrace(PTRACE_SYSCALL, pid, 0, 0) < 0
                {
                    launch_fail(domain, pid, b"launch syscall denial failed");
                }
            }
            _ => launch_fail(domain, pid, b"invalid launch contract decision"),
        }
    }
}

fn launch_exited(status: i32) -> bool {
    status & 0x7f == 0
}

fn launch_exit_code(status: i32) -> i32 {
    (status >> 8) & 0xff
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
        127 => b"exec of configured domain init failed",
        128 => b"configured domain workdir unavailable",
        _ => b"tracee exited before supervision",
    }
}

fn launch_early_fail(domain: &mut Domain, pid: usize, reason: &[u8]) -> ! {
    let _ = ptrace(PTRACE_KILL, pid, 0, 0);
    let mut status = 0;
    let _ = wait(pid, &mut status);
    let _ = domain.task_exited(false);
    launch_startup_fail(reason)
}

fn launch_domain_fail(domain: &mut Domain, reason: &[u8]) -> ! {
    let _ = domain.task_exited(false);
    launch_startup_fail(reason)
}

fn launch_fail(domain: &mut Domain, pid: usize, reason: &[u8]) -> ! {
    let _ = ptrace(PTRACE_KILL, pid, 0, 0);
    let mut status = 0;
    let _ = wait(pid, &mut status);
    let _ = domain.task_exited(false);
    write_stderr(b"ginux: launch supervision failure: ");
    write_stderr(reason);
    write_stderr(b"\n");
    exit_process(1)
}

fn launch_startup_fail(reason: &[u8]) -> ! {
    write_stderr(b"ginux: launch startup failure: ");
    write_stderr(reason);
    write_stderr(b"\n");
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

fn write_stderr_i64(value: i64) {
    if value < 0 {
        write_stderr(b"-");
        write_stderr_u64(value.unsigned_abs());
    } else {
        write_stderr_u64(value as u64);
    }
}
