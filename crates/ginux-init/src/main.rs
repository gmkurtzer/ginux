#![no_std]
#![no_main]

use core::arch::{asm, global_asm};
use core::panic::PanicInfo;
use ginux_core::{
    balanced_arguments_allowed, balanced_identity_result, balanced_surface, decide,
    decide_balanced, decide_systemd, systemd_surface, Action, BalancedSurface, SYS_EXIT,
    SYS_EXIT_GROUP, SYS_GETPID, SYS_UNAME, SYS_WRITE,
};
use ginux_devices::{
    BlockBackend, DeviceError, DeviceKind, DeviceName, DeviceRegistry, DeviceView, IsolatedNetwork,
    Partition, IOCTL_CONSOLE_GET_SIZE,
};
use ginux_domain::{ConfigError, Domain, DomainConfig, NetworkBackend};
use ginux_kernel_views::{
    CgroupFs, IdentityFs, IdentityView, MemoryView, MountView, NamespaceView, ProcFs, SysFs,
    ViewError,
};
use ginux_process::{
    Credentials, EventFd, EventPoll, FutexTable, ProcessError, ProcessTable, SignalAction,
    SignalDelivery, TaskId, TaskView, TimerTable, VirtualClock, EVENT_READ, EVENT_WRITE,
};
use ginux_runtime::{execute, AddressSpace, AuditAction, LoadError};
use ginux_vfs::{BaseReader, Index, VfsError, VirtualFs, O_CREATE, O_READ, O_WRITE};

global_asm!(
    ".global _start",
    ".type _start,@function",
    "_start:",
    "xor %rbp, %rbp",
    "mov %rsp, %rdi",
    "andq $-16, %rsp",
    "call rust_main",
    "ud2",
    options(att_syntax)
);

const SYS_WRITE_HOST: usize = 1;
const SYS_READ_HOST: usize = 0;
const SYS_CLOSE_HOST: usize = 3;
const SYS_DUP_HOST: usize = 32;
const SYS_FCNTL_HOST: usize = 72;
const SYS_RT_SIGPROCMASK: usize = 14;
const SYS_PREAD64: usize = 17;
const SYS_PWRITE64: usize = 18;
const SYS_FSYNC: usize = 74;
const SYS_CLOCK_GETTIME: usize = 228;
const SYS_CHDIR: usize = 80;
const SYS_SETGID: usize = 106;
const SYS_SETUID: usize = 105;
const SYS_SETGROUPS: usize = 116;
const SYS_PRCTL: usize = 157;
const SYS_CHROOT: usize = 161;
const SYS_PAUSE: usize = 34;
const SYS_FORK: usize = 57;
const SYS_EXECVE: usize = 59;
const SYS_EXIT_HOST: usize = 60;
const SYS_WAIT4: usize = 61;
const SYS_KILL: usize = 62;
const SYS_PTRACE: usize = 101;
const SYS_OPENAT_HOST: usize = 257;
const SYS_MKDIRAT_HOST: usize = 258;
const SYS_UNLINKAT_HOST: usize = 263;
const SYS_SYMLINKAT_HOST: usize = 266;
const SYS_GETDENTS64_HOST: usize = 217;
const SYS_SIGNALFD4: usize = 289;
const AT_FDCWD: usize = (-100isize) as usize;
const AT_REMOVEDIR: usize = 0x200;
const O_RDONLY: usize = 0;
const O_WRONLY: usize = 1;
const O_RDWR: usize = 2;
const O_CREATE_HOST: usize = 0x40;
const O_TRUNC_HOST: usize = 0x200;
const O_DIRECTORY_HOST: usize = 0x1_0000;
const MS_RDONLY: u64 = 1;
const MS_NOSUID: u64 = 2;
const MS_NODEV: u64 = 4;
const MS_NOEXEC: u64 = 8;
const MS_REMOUNT: u64 = 32;
const MS_BIND: u64 = 4096;
const MS_MOVE: u64 = 8192;
const MS_REC: u64 = 16384;
const CLOCK_MONOTONIC: usize = 1;
const CLOCK_PROCESS_CPUTIME_ID: usize = 2;

const PTRACE_TRACEME: usize = 0;
const PTRACE_PEEKTEXT: usize = 1;
const PTRACE_POKETEXT: usize = 4;
const PTRACE_POKEDATA: usize = 5;
const PTRACE_CONT: usize = 7;
const PTRACE_PEEKDATA: usize = 2;
const PTRACE_KILL: usize = 8;
const PTRACE_GETREGS: usize = 12;
const PTRACE_SETREGS: usize = 13;
const PTRACE_SYSCALL: usize = 24;
const PTRACE_SYSEMU: usize = 31;
const PTRACE_SETOPTIONS: usize = 0x4200;
const PTRACE_GETEVENTMSG: usize = 0x4201;
const PTRACE_O_TRACESYSGOOD: usize = 1;
const PTRACE_O_TRACEFORK: usize = 1 << 1;
const PTRACE_O_TRACEVFORK: usize = 1 << 2;
const PTRACE_O_TRACECLONE: usize = 1 << 3;
const PTRACE_O_TRACEEXEC: usize = 1 << 4;
const PTRACE_O_EXITKILL: usize = 1 << 20;
const PTRACE_EVENT_FORK: usize = 1;
const PTRACE_EVENT_VFORK: usize = 2;
const PTRACE_EVENT_CLONE: usize = 3;
const PTRACE_EVENT_EXEC: usize = 4;

const EPERM: i64 = 1;
const ECHILD: i64 = 10;
const ENOSYS: i64 = 38;
const VIRTUAL_PID: u64 = 4242;
const MAX_ELF_SIZE: usize = 64 * 1024;
const MAX_CONFIG_SIZE: usize = 16 * 1024;
const MAX_VFS_INDEX_SIZE: usize = 8 * 1024 * 1024;
const SIG_BLOCK: usize = 0;
const SIGTERM: usize = 15;
const SIGKILL: usize = 9;
const PR_SET_NO_NEW_PRIVS: usize = 38;
const DOMAIN_UID: usize = 1000;
const DOMAIN_GID: usize = 1000;
const DOMAIN_NULL_FD: usize = 240;
const DOMAIN_CONSOLE_FD: usize = 241;
const NATIVE_PROC_FD_LIMIT: usize = 512;

static mut ELF_BYTES: [u8; MAX_ELF_SIZE] = [0; MAX_ELF_SIZE];
static mut CONFIG_BYTES: [u8; MAX_CONFIG_SIZE] = [0; MAX_CONFIG_SIZE];
static mut VFS_INDEX_BYTES: [u8; MAX_VFS_INDEX_SIZE] = [0; MAX_VFS_INDEX_SIZE];

#[repr(C)]
struct UserRegs {
    r15: u64,
    r14: u64,
    r13: u64,
    r12: u64,
    rbp: u64,
    rbx: u64,
    r11: u64,
    r10: u64,
    r9: u64,
    r8: u64,
    rax: u64,
    rcx: u64,
    rdx: u64,
    rsi: u64,
    rdi: u64,
    orig_rax: u64,
    rip: u64,
    cs: u64,
    eflags: u64,
    rsp: u64,
    ss: u64,
    fs_base: u64,
    gs_base: u64,
    ds: u64,
    es: u64,
    fs: u64,
    gs: u64,
}

#[panic_handler]
fn panic(_info: &PanicInfo<'_>) -> ! {
    fail(b"panic")
}

#[no_mangle]
unsafe extern "C" fn memcpy(
    destination: *mut core::ffi::c_void,
    source: *const core::ffi::c_void,
    length: usize,
) -> *mut core::ffi::c_void {
    let destination_bytes = destination.cast::<u8>();
    let source_bytes = source.cast::<u8>();
    let mut index = 0;
    while index < length {
        core::ptr::write_volatile(
            destination_bytes.add(index),
            core::ptr::read_volatile(source_bytes.add(index)),
        );
        index += 1;
    }
    destination
}

#[no_mangle]
unsafe extern "C" fn memset(
    destination: *mut core::ffi::c_void,
    value: i32,
    length: usize,
) -> *mut core::ffi::c_void {
    let destination_bytes = destination.cast::<u8>();
    let mut index = 0;
    while index < length {
        core::ptr::write_volatile(destination_bytes.add(index), value as u8);
        index += 1;
    }
    destination
}

#[no_mangle]
unsafe extern "C" fn bcmp(
    left: *const core::ffi::c_void,
    right: *const core::ffi::c_void,
    length: usize,
) -> i32 {
    let left = left.cast::<u8>();
    let right = right.cast::<u8>();
    let mut index = 0;
    while index < length {
        if core::ptr::read_volatile(left.add(index)) != core::ptr::read_volatile(right.add(index)) {
            return 1;
        }
        index += 1;
    }
    0
}

#[no_mangle]
unsafe extern "C" fn memcmp(
    left: *const core::ffi::c_void,
    right: *const core::ffi::c_void,
    length: usize,
) -> i32 {
    let left = left.cast::<u8>();
    let right = right.cast::<u8>();
    let mut index = 0;
    while index < length {
        let left_byte = core::ptr::read_volatile(left.add(index));
        let right_byte = core::ptr::read_volatile(right.add(index));
        if left_byte != right_byte {
            return left_byte as i32 - right_byte as i32;
        }
        index += 1;
    }
    0
}

#[no_mangle]
extern "C" fn rust_main(initial_stack: *const usize) -> ! {
    let argc = unsafe { *initial_stack };
    let argv = unsafe { initial_stack.add(1) };
    if argc >= 3 {
        let command = unsafe { *argv.add(1) as *const u8 };
        if c_string_equals(command, b"runtime") {
            let path = unsafe { *argv.add(2) as *const u8 };
            runtime_main(path);
        } else if c_string_equals(command, b"config") {
            let config_path = unsafe { *argv.add(2) as *const u8 };
            config_main(config_path);
        } else if c_string_equals(command, b"lifecycle") {
            let config_path = unsafe { *argv.add(2) as *const u8 };
            lifecycle_main(config_path);
        } else if argc >= 5 && c_string_equals(command, b"vfs") {
            let config_path = unsafe { *argv.add(2) as *const u8 };
            let rootfs_path = unsafe { *argv.add(3) as *const u8 };
            let index_path = unsafe { *argv.add(4) as *const u8 };
            vfs_main(config_path, rootfs_path, index_path);
        } else if argc >= 4 && c_string_equals(command, b"native") {
            let config_path = unsafe { *argv.add(2) as *const u8 };
            let root_path = unsafe { *argv.add(3) as *const u8 };
            let mode = if argc >= 5
                && c_string_equals(unsafe { *argv.add(4) as *const u8 }, b"hold")
            {
                1
            } else if argc >= 5 && c_string_equals(unsafe { *argv.add(4) as *const u8 }, b"probe") {
                2
            } else {
                0
            };
            native_main(config_path, root_path, mode);
        } else if argc >= 4 && c_string_equals(command, b"domain") {
            let config_path = unsafe { *argv.add(2) as *const u8 };
            let fixture_path = unsafe { *argv.add(3) as *const u8 };
            domain_main(config_path, fixture_path);
        } else if c_string_equals(command, b"process") {
            let config_path = unsafe { *argv.add(2) as *const u8 };
            process_main(config_path);
        } else if argc >= 4 && c_string_equals(command, b"process-native") {
            let config_path = unsafe { *argv.add(2) as *const u8 };
            let fixture_path = unsafe { *argv.add(3) as *const u8 };
            process_native_main(config_path, fixture_path);
        } else if argc >= 4 && c_string_equals(command, b"devices") {
            let config_path = unsafe { *argv.add(2) as *const u8 };
            let block_path = unsafe { *argv.add(3) as *const u8 };
            devices_main(config_path, block_path);
        } else if argc >= 5 && c_string_equals(command, b"device-bench") {
            let config_path = unsafe { *argv.add(2) as *const u8 };
            let block_path = unsafe { *argv.add(3) as *const u8 };
            let kind = unsafe { *argv.add(4) as *const u8 };
            device_bench_main(config_path, block_path, kind);
        } else if c_string_equals(command, b"kernel-views") {
            let config_path = unsafe { *argv.add(2) as *const u8 };
            kernel_views_main(config_path);
        }
    }
    gate1_main()
}

fn kernel_views_main(config_path: *const u8) -> ! {
    let config = load_config(config_path);
    let mut domain = Domain::create(config);
    if domain.start().is_err() {
        kernel_views_fail(b"domain start failed");
    }
    let mut processes: ProcessTable<4> = ProcessTable::new();
    let init = processes
        .create_init(Credentials::root())
        .unwrap_or_else(|_| kernel_views_fail(b"virtual init failed"));
    let child = processes
        .fork(init)
        .unwrap_or_else(|_| kernel_views_fail(b"virtual child failed"));
    processes
        .credentials_mut(child)
        .and_then(|credentials| credentials.set_uid(1000))
        .unwrap_or_else(|_| kernel_views_fail(b"virtual credentials failed"));
    let mut tasks = [processes.view(init).unwrap(); 4];
    let task_count = processes.views(&mut tasks);
    let identity = IdentityView {
        hostname: b"gate8-domain",
        machine_id: b"0123456789abcdef0123456789abcdef",
        boot_id: b"11111111-2222-3333-4444-555555555555",
    };
    let mounts = [
        MountView {
            source: b"ginux-root",
            target: b"/",
            filesystem: b"ginuxfs",
            options: b"ro",
        },
        MountView {
            source: b"proc",
            target: b"/proc",
            filesystem: b"proc",
            options: b"rw,nosuid,nodev,noexec",
        },
    ];
    let procfs = ProcFs::new(
        &tasks[..task_count],
        child.pid,
        12_340_000_000,
        MemoryView {
            total_bytes: 512 * 1024 * 1024,
            available_bytes: 384 * 1024 * 1024,
        },
        identity,
        &mounts,
    );
    let mut output = [0u8; 512];
    let status_len = procfs
        .read(b"/proc/2/status", &mut output)
        .unwrap_or_else(|_| kernel_views_fail(b"proc status failed"));
    if !contains(&output[..status_len], b"Pid:\t2\n")
        || !contains(&output[..status_len], b"PPid:\t1\n")
        || procfs.read(b"/proc/424242/status", &mut output) != Err(ViewError::NotFound)
        || procfs.write(b"/proc/sys/kernel/hostname", b"host") != Err(ViewError::ReadOnly)
    {
        kernel_views_fail(b"proc isolation failed");
    }
    line(b"GINUX_VIEWS: proc pids=1,2 host-pid=absent mutation=denied");

    let mut registry: DeviceRegistry<6, 2> = DeviceRegistry::new();
    if registry
        .assign(7, 100, DeviceName::new(b"vda").unwrap(), DeviceKind::Block)
        .is_err()
        || registry
            .assign(
                7,
                101,
                DeviceName::new(b"net0").unwrap(),
                DeviceKind::Network,
            )
            .is_err()
        || registry
            .assign(
                8,
                102,
                DeviceName::new(b"host-only").unwrap(),
                DeviceKind::Block,
            )
            .is_err()
    {
        kernel_views_fail(b"device assignment failed");
    }
    let placeholder = DeviceView {
        name: DeviceName::new(b"none").unwrap(),
        kind: DeviceKind::Null,
        major: 0,
        minor: 0,
    };
    let mut devices = [placeholder; 6];
    let device_count = registry.views(7, &mut devices);
    let sysfs = SysFs::new(&devices[..device_count], 2);
    if sysfs
        .read(b"/sys/class/block/vda/dev", &mut output)
        .is_err()
        || sysfs
            .read(b"/sys/class/net/net0/type", &mut output)
            .is_err()
        || sysfs.read(b"/sys/class/block/host-only/dev", &mut output) != Err(ViewError::NotFound)
        || sysfs.write(b"/sys/class/block/vda/dev", b"8:0") != Err(ViewError::ReadOnly)
    {
        kernel_views_fail(b"sysfs isolation failed");
    }
    line(b"GINUX_VIEWS: sys devices=vda,net0 host-device=absent mutation=denied");

    let mut cgroups: CgroupFs<4> = CgroupFs::new();
    if cgroups.create(b"/system.slice", 0).is_err()
        || cgroups
            .write(b"/cgroup.subtree_control", b"+cpu +memory +pids\n", 0)
            .is_err()
        || cgroups.write(b"/system.slice/pids.max", b"4\n", 0).is_err()
        || cgroups.attach(b"/system.slice", child.pid, 0).is_err()
        || cgroups.create(b"/host.slice", 1000) != Err(ViewError::PermissionDenied)
    {
        kernel_views_fail(b"cgroup isolation failed");
    }
    line(b"GINUX_VIEWS: cgroup path=/system.slice pid=2 host-mutation=denied");

    let identities = IdentityFs::new(
        &tasks[..task_count],
        identity,
        NamespaceView {
            mount: 101,
            pid: 102,
            uts: 103,
            cgroup: 104,
        },
    );
    if identities.read(b"/etc/machine-id", &mut output).is_err()
        || identities.readlink(b"/proc/1/ns/mnt", &mut output).is_err()
        || identities.readlink(b"/proc/424242/ns/pid", &mut output) != Err(ViewError::NotFound)
    {
        kernel_views_fail(b"identity isolation failed");
    }
    line(b"GINUX_VIEWS: identity hostname=gate8-domain namespaces=virtual host-id=absent");

    if processes.shutdown(0) != 2 || processes.reap_all() != 2 || domain.task_exited(true).is_err()
    {
        kernel_views_fail(b"cleanup failed");
    }
    line(b"GATE8_VIEWS: PASS");
    exit_process(0)
}

fn kernel_views_fail(reason: &[u8]) -> ! {
    write_all(b"GATE8_VIEWS: FAIL: ");
    line(reason);
    exit_process(1)
}

fn process_main(config_path: *const u8) -> ! {
    let config = load_config(config_path);
    let mut domain = Domain::create(config);
    if domain.start().is_err() {
        process_fail(b"domain start failed");
    }
    let mut processes: ProcessTable<8> = ProcessTable::new();
    let init = processes
        .create_init(Credentials::root())
        .unwrap_or_else(|_| process_fail(b"virtual init failed"));
    if init.pid != 1 || processes.view(init).map(|view| view.parent) != Ok(0) {
        process_fail(b"virtual init identity failed");
    }
    line(b"GINUX_PROCESS: init=1 parent=0 host-pid=unexposed");

    let mut final_child = init;
    for iteration in 0..256u32 {
        let child = processes
            .fork(init)
            .unwrap_or_else(|_| process_fail(b"fork failed"));
        let thread = processes
            .clone_thread(child)
            .unwrap_or_else(|_| process_fail(b"clone thread failed"));
        if processes.view(thread).map(|view| view.tgid) != Ok(child.pid)
            || processes.exec(child) != Ok(1)
        {
            process_fail(b"thread or exec identity failed");
        }
        processes
            .exit(thread, 0)
            .unwrap_or_else(|_| process_fail(b"thread exit failed"));
        processes
            .exit(child, iteration as i32)
            .unwrap_or_else(|_| process_fail(b"child exit failed"));
        if processes.wait(init, Some(thread.pid)).is_err()
            || processes
                .wait(init, Some(child.pid))
                .map(|result| result.status)
                != Ok(iteration as i32)
        {
            process_fail(b"wait or reap failed");
        }
        final_child = child;
    }
    if processes.view(final_child) != Err(ProcessError::InvalidTask) || processes.active() != 1 {
        process_fail(b"stale identity or lifecycle leak");
    }
    line(b"GINUX_PROCESS: fork-clone-exec-wait iterations=256 stale-id=denied active=1");

    let signal_child = processes
        .fork(init)
        .unwrap_or_else(|_| process_fail(b"signal child failed"));
    processes
        .set_signal_action(signal_child, 10, SignalAction::Handler(0x4000))
        .unwrap_or_else(|_| process_fail(b"signal action failed"));
    processes
        .signal(signal_child, 10)
        .unwrap_or_else(|_| process_fail(b"signal queue failed"));
    if processes.blocking_interrupted(signal_child) != Ok(true)
        || processes.begin_signal(signal_child)
            != Ok(Some(SignalDelivery::Handler {
                signal: 10,
                address: 0x4000,
            }))
        || processes.signal_return(signal_child).is_err()
    {
        process_fail(b"signal delivery or return failed");
    }
    line(b"GINUX_PROCESS: signal=10 blocked-wait=interrupted handler=0x4000 return=PASS");

    let credentials = processes
        .credentials_mut(signal_child)
        .unwrap_or_else(|_| process_fail(b"credentials lookup failed"));
    if credentials.set_groups(&[10, 20]).is_err()
        || credentials.set_gid(1000).is_err()
        || credentials.set_uid(1000).is_err()
        || credentials.capabilities != 0
        || credentials.set_uid(0) != Err(ProcessError::PermissionDenied)
    {
        process_fail(b"virtual credential transition failed");
    }
    line(b"GINUX_PROCESS: virtual-uid=1000 virtual-gid=1000 host-credentials=unchanged regain-root=denied");

    let mut futex: FutexTable<2> = FutexTable::new();
    if futex.wait(signal_child, 0x1000, 7, 7).is_err() {
        process_fail(b"futex wait failed");
    }
    let mut woken = [init; 1];
    if futex.wake(0x1000, 1, &mut woken) != Ok(1) || woken[0] != signal_child {
        process_fail(b"futex wake failed");
    }
    let mut event = EventFd::new(0, false);
    let mut poll: EventPoll<2> = EventPoll::new();
    if poll.add(7, EVENT_READ | EVENT_WRITE).is_err()
        || event.write(9).is_err()
        || poll.ready(7, EVENT_READ) != Ok(EVENT_READ)
        || event.read() != Ok(9)
    {
        process_fail(b"eventfd or epoll failed");
    }
    let mut clock = VirtualClock::new();
    let mut timers: TimerTable<2> = TimerTable::new();
    let timer = timers
        .create()
        .unwrap_or_else(|_| process_fail(b"timer create failed"));
    if timers.arm(timer, 0, 50, 25).is_err() || clock.advance(125).is_err() {
        process_fail(b"timer arm failed");
    }
    timers.update(clock.monotonic_ns());
    if timers.read(timer) != Ok(4) {
        process_fail(b"timer expiration failed");
    }
    line(b"GINUX_PROCESS: futex-wake=1 eventfd=9 epoll=readable timer-expirations=4");

    if processes.shutdown(137) != 2 || processes.reap_all() != 2 || processes.active() != 0 {
        process_fail(b"shutdown cleanup failed");
    }
    if domain.task_exited(true).is_err() {
        process_fail(b"domain cleanup failed");
    }
    line(b"GINUX_PROCESS: shutdown tasks=0 waiters=0 events=0 timers=0 resources=released");
    line(b"GATE7_PROCESS: PASS");
    exit_process(0)
}

fn process_fail(reason: &[u8]) -> ! {
    write_all(b"GATE7_PROCESS: FAIL: ");
    line(reason);
    exit_process(1)
}

#[derive(Clone, Copy)]
struct NativeProcessTask {
    host_pid: usize,
    virtual_id: ginux_process::TaskId,
    awaiting_exit: bool,
    override_result: bool,
    result: i64,
    syscall: u64,
    translated_rdi: u64,
    fork_result_pending: u32,
    fork_breakpoint_rip: usize,
    fork_breakpoint_word: usize,
    tracing: bool,
}

fn process_native_main(config_path: *const u8, fixture_path: *const u8) -> ! {
    let config = load_config(config_path);
    let mut domain = Domain::create(config);
    if domain.start().is_err() {
        process_fail(b"native process domain start failed");
    }
    let child = unsafe { syscall0(SYS_FORK) };
    if child < 0 {
        process_fail(b"native process tracee fork failed");
    }
    if child == 0 {
        process_native_tracee(fixture_path);
    }
    supervise_process_native(child as usize, &mut domain)
}

fn process_native_tracee(fixture_path: *const u8) -> ! {
    if unsafe { syscall4(SYS_PTRACE, PTRACE_TRACEME, 0, 0, 0) } < 0
        || unsafe { syscall5(SYS_PRCTL, PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) } < 0
    {
        native_tracee_setup_fail(130);
    }
    let argv = [fixture_path, core::ptr::null()];
    let environment: [*const u8; 1] = [core::ptr::null()];
    unsafe {
        syscall3(
            SYS_EXECVE,
            fixture_path as usize,
            argv.as_ptr() as usize,
            environment.as_ptr() as usize,
        );
        syscall1(SYS_EXIT_HOST, 131);
    }
    loop {
        core::hint::spin_loop();
    }
}

fn supervise_process_native(root_host_pid: usize, domain: &mut Domain) -> ! {
    let mut status = 0i32;
    if wait(root_host_pid, &mut status) < 0 || !stopped(status) {
        write_all(b"GINUX_NATIVE_PROCESS: initial-status=");
        write_i64(status as i64);
        write_all(b"\n");
        native_fail(
            domain,
            root_host_pid,
            b"process fixture did not stop after exec",
        );
    }
    let options = PTRACE_O_TRACESYSGOOD | PTRACE_O_TRACEFORK | PTRACE_O_EXITKILL;
    if ptrace(PTRACE_SETOPTIONS, root_host_pid, 0, options) < 0 {
        native_fail(domain, root_host_pid, b"process ptrace options failed");
    }
    let mut processes: ProcessTable<8> = ProcessTable::new();
    let root_virtual = processes
        .create_init(Credentials::unprivileged(1000, 1000))
        .unwrap_or_else(|_| process_native_fail(domain, root_host_pid, b"virtual init failed"));
    let mut tasks: [Option<NativeProcessTask>; 8] = [None; 8];
    tasks[0] = Some(NativeProcessTask {
        host_pid: root_host_pid,
        virtual_id: root_virtual,
        awaiting_exit: false,
        override_result: false,
        result: 0,
        syscall: 0,
        translated_rdi: 0,
        fork_result_pending: 0,
        fork_breakpoint_rip: 0,
        fork_breakpoint_word: 0,
        tracing: true,
    });
    if ptrace(PTRACE_SYSCALL, root_host_pid, 0, 0) < 0 {
        process_native_fail(domain, root_host_pid, b"process initial resume failed");
    }
    line(b"GINUX_NATIVE_PROCESS: virtual-init=1 host-pid=unexposed ptrace=attached");
    let mut native_child_exits = 0usize;

    loop {
        status = 0;
        let waited = unsafe {
            syscall4(
                SYS_WAIT4,
                (-1isize) as usize,
                &mut status as *mut _ as usize,
                0,
                0,
            )
        };
        if waited <= 0 {
            process_native_fail(domain, root_host_pid, b"process wait-any failed");
        }
        let host_pid = waited as usize;
        let Some(task_index) = tasks
            .iter()
            .position(|task| task.is_some_and(|task| task.host_pid == host_pid))
        else {
            process_native_fail(domain, root_host_pid, b"unknown host tracee");
        };
        let task = tasks[task_index].unwrap();

        if !stopped(status) {
            let exit_status = if status & 0x7f == 0 {
                status >> 8
            } else {
                128 + (status & 0x7f)
            };
            write_all(b"GINUX_NATIVE_TASK: virtual-pid=");
            write_u64(task.virtual_id.pid as u64);
            write_all(b" exit-status=");
            write_i64(exit_status as i64);
            write_all(b"\n");
            if processes.exit(task.virtual_id, exit_status).is_err() {
                process_native_fail(domain, root_host_pid, b"virtual exit failed");
            }
            if let Some(entry) = tasks[task_index].as_mut() {
                entry.tracing = false;
            }
            if task.virtual_id != root_virtual && exit_status == 23 {
                native_child_exits += 1;
            }
            if task.virtual_id == root_virtual {
                if exit_status != 0
                    || native_child_exits != 32
                    || tasks.iter().flatten().any(|entry| entry.tracing)
                {
                    write_all(b"GINUX_NATIVE_PROCESS: root-exit=");
                    write_i64(exit_status as i64);
                    write_all(b" tracing-remaining=");
                    write_u64(tasks.iter().flatten().filter(|entry| entry.tracing).count() as u64);
                    write_all(b"\n");
                    process_native_fail(domain, root_host_pid, b"native process fixture failed");
                }
                if processes.reap_all() == 0 || processes.active() != 0 {
                    process_native_fail(domain, root_host_pid, b"native process cleanup failed");
                }
                if domain.task_exited(true).is_err() {
                    process_native_fail(domain, root_host_pid, b"native domain exit failed");
                }
                line(b"GINUX_NATIVE_PROCESS: fork=mediated iterations=32 concurrent-children=2 child=ptrace-attached pid-translation=PASS");
                line(b"GINUX_NATIVE_PROCESS: wait=translated child-exits=32 exit-status=23 tasks=0 cleanup=PASS");
                line(b"GATE7_NATIVE_PROCESS: PASS");
                exit_process(0);
            }
            continue;
        }

        let event = (status as usize) >> 16;
        if event == PTRACE_EVENT_FORK {
            let mut child_host_pid = 0usize;
            if ptrace(
                PTRACE_GETEVENTMSG,
                host_pid,
                0,
                &mut child_host_pid as *mut _ as usize,
            ) < 0
            {
                process_native_fail(domain, root_host_pid, b"fork event read failed");
            }
            let child_virtual = processes.fork(task.virtual_id).unwrap_or_else(|_| {
                process_native_fail(domain, root_host_pid, b"virtual fork failed")
            });
            let Some(child_slot) = tasks.iter().position(Option::is_none) else {
                process_native_fail(domain, root_host_pid, b"host task table full");
            };
            tasks[child_slot] = Some(NativeProcessTask {
                host_pid: child_host_pid,
                virtual_id: child_virtual,
                awaiting_exit: false,
                override_result: false,
                result: 0,
                syscall: 0,
                translated_rdi: 0,
                fork_result_pending: 0,
                fork_breakpoint_rip: 0,
                fork_breakpoint_word: 0,
                tracing: true,
            });
            let mut parent_registers = core::mem::MaybeUninit::<UserRegs>::uninit();
            if ptrace(
                PTRACE_GETREGS,
                host_pid,
                0,
                parent_registers.as_mut_ptr() as usize,
            ) < 0
            {
                process_native_fail(domain, root_host_pid, b"fork breakpoint registers failed");
            }
            let parent_registers = unsafe { parent_registers.assume_init() };
            let breakpoint_rip = parent_registers.rip as usize;
            let mut breakpoint_word = 0usize;
            if ptrace(
                PTRACE_PEEKTEXT,
                host_pid,
                breakpoint_rip,
                &mut breakpoint_word as *mut _ as usize,
            ) < 0
            {
                process_native_fail(domain, root_host_pid, b"fork breakpoint read failed");
            }
            if ptrace(
                PTRACE_POKETEXT,
                host_pid,
                breakpoint_rip,
                (breakpoint_word & !0xff) | 0xcc,
            ) < 0
            {
                process_native_fail(domain, root_host_pid, b"fork breakpoint install failed");
            }
            let parent = tasks[task_index].as_mut().unwrap();
            parent.awaiting_exit = false;
            parent.override_result = false;
            parent.fork_result_pending = child_virtual.pid;
            parent.fork_breakpoint_rip = breakpoint_rip;
            parent.fork_breakpoint_word = breakpoint_word;
            if ptrace(PTRACE_CONT, host_pid, 0, 0) < 0 {
                process_native_fail(
                    domain,
                    root_host_pid,
                    b"fork parent breakpoint resume failed",
                );
            }
            continue;
        }

        let signal = ((status >> 8) & 0xff) as u8;
        if signal == 5 && tasks[task_index].unwrap().fork_result_pending != 0 {
            let mut parent_registers = core::mem::MaybeUninit::<UserRegs>::uninit();
            if ptrace(
                PTRACE_GETREGS,
                host_pid,
                0,
                parent_registers.as_mut_ptr() as usize,
            ) < 0
            {
                process_native_fail(domain, root_host_pid, b"fork result read failed");
            }
            let mut parent_registers = unsafe { parent_registers.assume_init() };
            let pending = tasks[task_index].unwrap();
            let virtual_pid = pending.fork_result_pending;
            if ptrace(
                PTRACE_POKETEXT,
                host_pid,
                pending.fork_breakpoint_rip,
                pending.fork_breakpoint_word,
            ) < 0
            {
                process_native_fail(domain, root_host_pid, b"fork breakpoint restore failed");
            }
            write_all(b"GINUX_NATIVE_PROCESS_AUDIT: fork host-result=");
            write_i64(parent_registers.rax as i64);
            write_all(b" virtual-result=");
            write_u64(virtual_pid as u64);
            write_all(b"\n");
            parent_registers.rip = pending.fork_breakpoint_rip as u64;
            parent_registers.rax = virtual_pid as u64;
            if ptrace(
                PTRACE_SETREGS,
                host_pid,
                0,
                &parent_registers as *const _ as usize,
            ) < 0
            {
                process_native_fail(domain, root_host_pid, b"fork result translation failed");
            }
            let parent = tasks[task_index].as_mut().unwrap();
            parent.fork_result_pending = 0;
            parent.fork_breakpoint_rip = 0;
            parent.fork_breakpoint_word = 0;
            if ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0 {
                process_native_fail(domain, root_host_pid, b"fork parent resume failed");
            }
            continue;
        }
        if signal == 19 {
            let _ = ptrace(PTRACE_SETOPTIONS, host_pid, 0, options);
            if ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0 {
                process_native_fail(domain, root_host_pid, b"fork child resume failed");
            }
            continue;
        }
        if signal == 17 {
            if ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0 {
                process_native_fail(domain, root_host_pid, b"SIGCHLD mediation failed");
            }
            continue;
        }
        if signal != (5 | 0x80) {
            write_all(b"GINUX_NATIVE_PROCESS: unexpected-signal=");
            write_u64(signal as u64);
            write_all(b" event=");
            write_u64(event as u64);
            write_all(b" host-pid=");
            write_u64(host_pid as u64);
            write_all(b" virtual-pid=");
            write_u64(task.virtual_id.pid as u64);
            let mut diagnostic_registers = core::mem::MaybeUninit::<UserRegs>::uninit();
            if ptrace(
                PTRACE_GETREGS,
                host_pid,
                0,
                diagnostic_registers.as_mut_ptr() as usize,
            ) >= 0
            {
                let diagnostic_registers = unsafe { diagnostic_registers.assume_init() };
                write_all(b" rip=");
                write_u64(diagnostic_registers.rip);
                write_all(b" orig-syscall=");
                write_u64(diagnostic_registers.orig_rax);
            }
            write_all(b"\n");
            process_native_fail(domain, root_host_pid, b"unexpected process trace stop");
        }
        let mut registers = core::mem::MaybeUninit::<UserRegs>::uninit();
        if ptrace(PTRACE_GETREGS, host_pid, 0, registers.as_mut_ptr() as usize) < 0 {
            process_native_fail(domain, root_host_pid, b"process registers read failed");
        }
        let mut registers = unsafe { registers.assume_init() };
        let mut current = tasks[task_index].unwrap();
        if current.awaiting_exit {
            let restore_translated_argument = current.syscall == 61 && current.translated_rdi != 0;
            if current.syscall == 61 && registers.rax as i64 > 0 {
                let waited_host = registers.rax as usize;
                let Some(waited_index) = tasks
                    .iter()
                    .position(|entry| entry.is_some_and(|entry| entry.host_pid == waited_host))
                else {
                    process_native_fail(domain, root_host_pid, b"wait result host pid unknown");
                };
                let waited_virtual = tasks[waited_index].unwrap().virtual_id;
                write_all(b"GINUX_NATIVE_PROCESS_AUDIT: wait host-result=");
                write_i64(registers.rax as i64);
                write_all(b" virtual-result=");
                write_u64(waited_virtual.pid as u64);
                write_all(b"\n");
                if processes
                    .wait(current.virtual_id, Some(waited_virtual.pid))
                    .is_err()
                {
                    process_native_fail(domain, root_host_pid, b"virtual wait failed");
                }
                registers.rax = waited_virtual.pid as u64;
                registers.rdi = current.translated_rdi;
                tasks[waited_index] = None;
                current.override_result = true;
            } else if current.override_result {
                registers.rax = current.result as u64;
            }
            if restore_translated_argument {
                registers.rdi = current.translated_rdi;
            }
            if (current.override_result || restore_translated_argument)
                && ptrace(PTRACE_SETREGS, host_pid, 0, &registers as *const _ as usize) < 0
            {
                process_native_fail(domain, root_host_pid, b"process result translation failed");
            }
            current.awaiting_exit = false;
            current.override_result = false;
            current.translated_rdi = 0;
            tasks[task_index] = Some(current);
            if ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0 {
                process_native_fail(domain, root_host_pid, b"process exit resume failed");
            }
            continue;
        }

        current.syscall = registers.orig_rax;
        current.awaiting_exit = true;
        match registers.orig_rax {
            39 | 110 => {
                let view = processes.view(current.virtual_id).unwrap_or_else(|_| {
                    process_native_fail(domain, root_host_pid, b"identity lookup failed")
                });
                current.result = if registers.orig_rax == 39 {
                    view.id.pid as i64
                } else {
                    view.parent as i64
                };
                current.override_result = true;
                registers.orig_rax = u64::MAX;
            }
            57 => {}
            61 => {
                if registers.rdi != u64::MAX {
                    let virtual_pid = registers.rdi as u32;
                    let Some(target) = tasks
                        .iter()
                        .flatten()
                        .find(|entry| entry.virtual_id.pid == virtual_pid)
                    else {
                        write_all(b"GINUX_NATIVE_PROCESS_AUDIT: invalid-wait-selector=");
                        write_u64(virtual_pid as u64);
                        write_all(b" caller=");
                        write_u64(current.virtual_id.pid as u64);
                        write_all(b"\n");
                        process_native_fail(domain, root_host_pid, b"wait selector invalid");
                    };
                    current.translated_rdi = registers.rdi;
                    registers.rdi = target.host_pid as u64;
                }
            }
            1 if matches!(registers.rdi, 1 | 2) && registers.rdx <= 4096 => {}
            60 | 231 => {}
            _ => {
                current.result = -ENOSYS;
                current.override_result = true;
                registers.orig_rax = u64::MAX;
            }
        }
        tasks[task_index] = Some(current);
        if ptrace(PTRACE_SETREGS, host_pid, 0, &registers as *const _ as usize) < 0
            || ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0
        {
            process_native_fail(domain, root_host_pid, b"process syscall dispatch failed");
        }
    }
}

fn process_native_fail(domain: &mut Domain, root_pid: usize, reason: &[u8]) -> ! {
    let _ = ptrace(PTRACE_KILL, root_pid, 0, 0);
    domain_fail(domain, reason)
}

struct HostBlock {
    descriptor: usize,
    len: u64,
}

impl BlockBackend for HostBlock {
    fn len(&self) -> u64 {
        self.len
    }

    fn read_at(&mut self, offset: u64, output: &mut [u8]) -> Result<(), DeviceError> {
        let result = unsafe {
            syscall4(
                SYS_PREAD64,
                self.descriptor,
                output.as_mut_ptr() as usize,
                output.len(),
                offset as usize,
            )
        };
        if result == output.len() as isize {
            Ok(())
        } else {
            Err(DeviceError::Backend)
        }
    }

    fn write_at(&mut self, offset: u64, input: &[u8]) -> Result<(), DeviceError> {
        let result = unsafe {
            syscall4(
                SYS_PWRITE64,
                self.descriptor,
                input.as_ptr() as usize,
                input.len(),
                offset as usize,
            )
        };
        if result == input.len() as isize {
            Ok(())
        } else {
            Err(DeviceError::Backend)
        }
    }

    fn flush(&mut self) -> Result<(), DeviceError> {
        if unsafe { syscall1(SYS_FSYNC, self.descriptor) } == 0 {
            Ok(())
        } else {
            Err(DeviceError::Backend)
        }
    }

    fn close(&mut self) -> Result<(), DeviceError> {
        if unsafe { syscall1(SYS_CLOSE_HOST, self.descriptor) } == 0 {
            Ok(())
        } else {
            Err(DeviceError::Backend)
        }
    }
}

fn devices_main(config_path: *const u8, block_path: *const u8) -> ! {
    let config = load_config(config_path);
    let domain_id = 6u64;
    let mut registry: DeviceRegistry<8, 8> = DeviceRegistry::new();
    let specifications = [
        (b"console".as_slice(), DeviceKind::Console, 1u64),
        (b"null".as_slice(), DeviceKind::Null, 2),
        (b"zero".as_slice(), DeviceKind::Zero, 3),
        (b"random".as_slice(), DeviceKind::Random, 4),
        (b"block0".as_slice(), DeviceKind::Block, 100),
        (b"net0".as_slice(), DeviceKind::Network, 200),
    ];
    for (name, kind, backend) in specifications {
        if text_list_contains(config.devices.as_slice(), name)
            && registry
                .assign(
                    domain_id,
                    backend,
                    DeviceName::new(name).unwrap_or_else(|_| device_fail(b"invalid device name")),
                    kind,
                )
                .is_err()
        {
            device_fail(b"device assignment failed");
        }
    }
    if !text_list_contains(config.storage.as_slice(), b"block0")
        || config.network_backend != ginux_domain::NetworkBackend::Mediated
    {
        device_fail(b"required normalized assignment missing");
    }
    let mut visible = [DeviceName::new(b"unused").unwrap(); 8];
    let visible_count = registry.visible(domain_id, &mut visible);
    if visible_count != 6 {
        device_fail(b"unexpected device projection");
    }
    line(b"GINUX_DEVICES: visible=console,null,zero,random,block0,net0 unassigned=hidden");

    let zero = registry
        .open(domain_id, b"zero")
        .unwrap_or_else(|_| device_fail(b"zero open failed"));
    let mut zeros = [1u8; 16];
    if registry.read(domain_id, zero, &mut zeros) != Ok(16) || zeros != [0; 16] {
        device_fail(b"zero read failed");
    }
    let console = registry
        .open(domain_id, b"console")
        .unwrap_or_else(|_| device_fail(b"console open failed"));
    let mut window = [0u8; 8];
    if registry
        .ioctl(domain_id, console, IOCTL_CONSOLE_GET_SIZE, &mut window)
        .is_err()
        || registry.ioctl(domain_id, console, 0xdead, &mut []).is_ok()
        || registry.map(domain_id, console).is_ok()
    {
        device_fail(b"ioctl or mapping policy failed");
    }
    line(b"GINUX_DEVICES: handles=virtual pseudo=PASS ioctl-unknown=denied mmap=denied");

    let host_descriptor =
        unsafe { syscall4(SYS_OPENAT_HOST, AT_FDCWD, block_path as usize, O_RDWR, 0) };
    if host_descriptor < 0 {
        device_fail(b"block backend open failed");
    }
    let backend = HostBlock {
        descriptor: host_descriptor as usize,
        len: 4096,
    };
    let mut partition = Partition::new(backend, 1024, 2048, false)
        .unwrap_or_else(|_| device_fail(b"partition creation failed"));
    if partition.write_at(0, b"GINUX-BLOCK").is_err() || partition.flush().is_err() {
        device_fail(b"partition write failed");
    }
    let mut block_value = [0u8; 11];
    if partition.read_at(0, &mut block_value).is_err()
        || &block_value != b"GINUX-BLOCK"
        || partition.write_at(2047, b"xx").is_ok()
    {
        device_fail(b"partition confinement failed");
    }
    if partition.close().is_err() {
        device_fail(b"partition close failed");
    }
    line(b"GINUX_BLOCK: backend=file partition=1024:2048 rw=PASS flush=PASS bounds=denied");

    let mut network: IsolatedNetwork<4> = IsolatedNetwork::new();
    if network.send(b"domain-packet").is_err() {
        device_fail(b"network send failed");
    }
    let mut packet = [0u8; 64];
    if network.receive(&mut packet) != Ok(13)
        || &packet[..13] != b"domain-packet"
        || network.control(1).is_ok()
    {
        device_fail(b"network mediation failed");
    }
    network.close();
    line(
        b"GINUX_NETWORK: backend=isolated-queue send=PASS receive=PASS control=denied host-fd=none",
    );

    if registry.assign(
        7,
        100,
        DeviceName::new(b"otherdisk").unwrap(),
        DeviceKind::Block,
    ) != Err(DeviceError::AssignmentConflict)
    {
        device_fail(b"exclusive conflict accepted");
    }
    registry.revoke(domain_id);
    if registry
        .assign(
            7,
            100,
            DeviceName::new(b"otherdisk").unwrap(),
            DeviceKind::Block,
        )
        .is_err()
        || registry.write(domain_id, console, b"revoked").is_ok()
    {
        device_fail(b"revocation failed");
    }
    line(b"GINUX_DEVICES: conflict=denied teardown=flushed,released,reusable");
    line(b"GATE6_DEVICES: PASS");
    exit_process(0)
}

fn text_list_contains<const N: usize>(values: &[ginux_domain::Text<N>], expected: &[u8]) -> bool {
    values.iter().any(|value| value.as_bytes() == expected)
}

fn device_fail(reason: &[u8]) -> ! {
    write_all(b"GATE6_DEVICES: FAIL: ");
    line(reason);
    exit_process(1)
}

#[repr(C)]
struct Timespec {
    seconds: i64,
    nanoseconds: i64,
}

fn clock_nanoseconds(clock: usize) -> u64 {
    let mut value = Timespec {
        seconds: 0,
        nanoseconds: 0,
    };
    if unsafe { syscall2(SYS_CLOCK_GETTIME, clock, &mut value as *mut _ as usize) } != 0
        || value.seconds < 0
        || value.nanoseconds < 0
    {
        device_fail(b"benchmark clock failed");
    }
    value.seconds as u64 * 1_000_000_000 + value.nanoseconds as u64
}

fn device_bench_main(config_path: *const u8, block_path: *const u8, kind: *const u8) -> ! {
    let config = load_config(config_path);
    let repetitions = 1024u64;
    let operations = repetitions * 2;
    let (bytes, wall_ns, cpu_ns) = if c_string_equals(kind, b"block") {
        if !text_list_contains(config.storage.as_slice(), b"block0") {
            device_fail(b"block benchmark assignment missing");
        }
        let descriptor =
            unsafe { syscall4(SYS_OPENAT_HOST, AT_FDCWD, block_path as usize, O_RDWR, 0) };
        if descriptor < 0 {
            device_fail(b"benchmark block open failed");
        }
        let backend = HostBlock {
            descriptor: descriptor as usize,
            len: 4096,
        };
        let mut partition = Partition::new(backend, 1024, 2048, false)
            .unwrap_or_else(|_| device_fail(b"benchmark partition failed"));
        let data = [0x5au8; 256];
        let mut output = [0u8; 256];
        let wall_start = clock_nanoseconds(CLOCK_MONOTONIC);
        let cpu_start = clock_nanoseconds(CLOCK_PROCESS_CPUTIME_ID);
        for iteration in 0..repetitions {
            let offset = (iteration % 8) * 256;
            if partition.write_at(offset, &data).is_err()
                || partition.read_at(offset, &mut output).is_err()
            {
                device_fail(b"benchmark block I/O failed");
            }
        }
        if partition.close().is_err() {
            device_fail(b"benchmark block close failed");
        }
        (
            repetitions * 512,
            clock_nanoseconds(CLOCK_MONOTONIC) - wall_start,
            clock_nanoseconds(CLOCK_PROCESS_CPUTIME_ID) - cpu_start,
        )
    } else if c_string_equals(kind, b"network") {
        if config.network_backend != ginux_domain::NetworkBackend::Mediated {
            device_fail(b"network benchmark assignment missing");
        }
        let mut network: IsolatedNetwork<2> = IsolatedNetwork::new();
        let frame = [0xabu8; 64];
        let mut output = [0u8; 64];
        let wall_start = clock_nanoseconds(CLOCK_MONOTONIC);
        let cpu_start = clock_nanoseconds(CLOCK_PROCESS_CPUTIME_ID);
        for _ in 0..repetitions {
            if network.send(&frame).is_err() || network.receive(&mut output) != Ok(64) {
                device_fail(b"benchmark network I/O failed");
            }
        }
        network.close();
        (
            repetitions * 128,
            clock_nanoseconds(CLOCK_MONOTONIC) - wall_start,
            clock_nanoseconds(CLOCK_PROCESS_CPUTIME_ID) - cpu_start,
        )
    } else {
        device_fail(b"unknown benchmark kind");
    };
    let throughput = bytes.saturating_mul(1_000_000_000) / wall_ns.max(1);
    let latency = wall_ns / operations;
    write_all(b"GINUX_BENCH: kind=");
    if c_string_equals(kind, b"block") {
        write_all(b"block");
    } else {
        write_all(b"network");
    }
    write_all(b" workload=rw-pairs repetitions=");
    write_u64(repetitions);
    write_all(b" operations=");
    write_u64(operations);
    write_all(b" bytes=");
    write_u64(bytes);
    write_all(b" wall_ns=");
    write_u64(wall_ns);
    write_all(b" cpu_ns=");
    write_u64(cpu_ns);
    write_all(b" throughput_Bps=");
    write_u64(throughput);
    write_all(b" latency_ns_per_op=");
    write_u64(latency);
    write_all(b" environment=rocky10-x86_64-qemu-tcg\n");
    line(b"GINUX_BENCH: PASS");
    exit_process(0)
}

struct HostRootfs {
    descriptor: usize,
}

impl BaseReader for HostRootfs {
    fn read_at(&mut self, offset: u64, output: &mut [u8]) -> Result<(), VfsError> {
        let result = unsafe {
            syscall4(
                SYS_PREAD64,
                self.descriptor,
                output.as_mut_ptr() as usize,
                output.len(),
                offset as usize,
            )
        };
        if result == output.len() as isize {
            Ok(())
        } else {
            Err(VfsError::Io)
        }
    }
}

fn vfs_main(config_path: *const u8, rootfs_path: *const u8, index_path: *const u8) -> ! {
    let config = load_config(config_path);
    let mut domain = Domain::create(config);
    if domain.start().is_err() {
        domain_fail(&mut domain, b"invalid start transition");
    }
    let index_length = read_index(index_path);
    let index_bytes = unsafe {
        core::slice::from_raw_parts(
            core::ptr::addr_of!(VFS_INDEX_BYTES).cast::<u8>(),
            index_length,
        )
    };
    let index = match Index::new(index_bytes) {
        Ok(index) => index,
        Err(_) => domain_fail(&mut domain, b"invalid VFS index"),
    };
    let descriptor =
        unsafe { syscall4(SYS_OPENAT_HOST, AT_FDCWD, rootfs_path as usize, O_RDONLY, 0) };
    if descriptor < 0 {
        domain_fail(&mut domain, b"rootfs open failed");
    }
    let reader = HostRootfs {
        descriptor: descriptor as usize,
    };
    let mut vfs: VirtualFs<'_, HostRootfs, 8, 4096, 8> = VirtualFs::new(index, reader);

    let os_fd = match vfs.open(b"/etc/os-release", O_READ) {
        Ok(value) => value,
        Err(_) => vfs_fail(
            &mut domain,
            &mut vfs,
            descriptor as usize,
            b"os-release lookup failed",
        ),
    };
    let mut os_release = [0u8; 512];
    let os_length = match vfs.read(os_fd, &mut os_release) {
        Ok(value) => value,
        Err(_) => vfs_fail(
            &mut domain,
            &mut vfs,
            descriptor as usize,
            b"os-release read failed",
        ),
    };
    let _ = vfs.close(os_fd);
    if !contains(&os_release[..os_length], b"Rocky Linux") {
        vfs_fail(
            &mut domain,
            &mut vfs,
            descriptor as usize,
            b"Rocky identity missing",
        );
    }

    let bash_fd = match vfs.open(b"/usr/bin/bash", O_READ) {
        Ok(value) => value,
        Err(_) => vfs_fail(
            &mut domain,
            &mut vfs,
            descriptor as usize,
            b"bash lookup failed",
        ),
    };
    let mut magic = [0u8; 4];
    if vfs.read(bash_fd, &mut magic) != Ok(4) || &magic != b"\x7fELF" {
        vfs_fail(
            &mut domain,
            &mut vfs,
            descriptor as usize,
            b"bash ELF read failed",
        );
    }
    let _ = vfs.close(bash_fd);

    let overlay_fd = match vfs.open(b"/ginux-overlay-proof", O_READ | O_WRITE | O_CREATE) {
        Ok(value) => value,
        Err(_) => vfs_fail(
            &mut domain,
            &mut vfs,
            descriptor as usize,
            b"overlay create failed",
        ),
    };
    if vfs.write(overlay_fd, b"domain-local") != Ok(12) {
        vfs_fail(
            &mut domain,
            &mut vfs,
            descriptor as usize,
            b"overlay write failed",
        );
    }
    let _ = vfs.close(overlay_fd);

    write_all(b"GINUX_VFS: root=");
    line(config.root.as_bytes());
    line(b"GINUX_VFS: base=/etc/os-release identity=Rocky-Linux");
    line(b"GINUX_VFS: base=/usr/bin/bash format=ELF64-x86_64");
    line(b"GINUX_VFS: overlay=/ginux-overlay-proof value=domain-local");
    line(b"GINUX_VFS: managed-host-paths=unexposed managed-host-fds=unexposed");
    vfs.teardown();
    let _ = unsafe { syscall1(SYS_CLOSE_HOST, descriptor as usize) };
    if domain.task_exited(true).is_err() {
        domain_fail(&mut domain, b"invalid task-exit transition");
    }
    line(b"GINUX_VFS: teardown overlay-files=0 descriptors=0");
    line(b"GINUX_VFS_CHECK: PASS");
    exit_process(0)
}

fn vfs_fail(
    domain: &mut Domain,
    vfs: &mut VirtualFs<'_, HostRootfs, 8, 4096, 8>,
    descriptor: usize,
    reason: &[u8],
) -> ! {
    vfs.teardown();
    let _ = unsafe { syscall1(SYS_CLOSE_HOST, descriptor) };
    domain_fail(domain, reason)
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    needle.is_empty()
        || haystack
            .windows(needle.len())
            .any(|window| window == needle)
}

fn native_main(config_path: *const u8, root_path: *const u8, mode: usize) -> ! {
    let config = load_config(config_path);
    print_effective_config(&config);
    let mode = if mode == 0 && config.contract_profile.as_bytes() == b"gate9-systemd" {
        3
    } else {
        mode
    };
    let mut domain = Domain::create(config);
    if domain.start().is_err() {
        domain_fail(&mut domain, b"invalid native start transition");
    }
    let root_descriptor = unsafe {
        syscall4(
            SYS_OPENAT_HOST,
            AT_FDCWD,
            root_path as usize,
            O_RDONLY | O_DIRECTORY_HOST,
            0,
        )
    };
    if root_descriptor < 0 {
        domain_fail(&mut domain, b"native root open failed");
    }
    let child = unsafe { syscall0(SYS_FORK) };
    if child < 0 {
        domain_fail(&mut domain, b"native tracee fork failed");
    }
    if child == 0 {
        native_tracee(root_path, mode, &config);
    }
    supervise_native(child as usize, &mut domain, mode, root_descriptor as usize)
}

fn native_tracee(root_path: *const u8, mode: usize, config: &DomainConfig) -> ! {
    static ROOT: &[u8] = b"/\0";
    static BASH: &[u8] = b"/usr/bin/bash\0";
    static ARG1: &[u8] = b"--noprofile\0";
    static ARG2: &[u8] = b"--norc\0";
    static ARG3: &[u8] = b"-c\0";
    static COMMAND: &[u8] = b"printf 'GINUX_NATIVE_SHELL: PASS\\n'\0";
    static HOLD_COMMAND: &[u8] = b"printf 'GINUX_NATIVE_HOLD: READY\\n'; while :; do :; done\0";
    static PROBE_COMMAND: &[u8] = b"if : > /tmp/ginux-mutation; then exit 90; else printf 'GINUX_BALANCED_MUTATION: BLOCKED\\n'; fi; if exec 9<>/dev/tcp/127.0.0.1/9; then exit 91; else printf 'GINUX_BALANCED_NETWORK: BLOCKED\\n'; fi\0";
    static ENV_PATH: &[u8] = b"PATH=/usr/bin:/bin\0";
    static ENV_HOME: &[u8] = b"HOME=/\0";
    static ENV_LOCALE: &[u8] = b"LC_ALL=C\0";
    static HOST_DEV_NULL: &[u8] = b"/dev/null\0";

    let null_fd = unsafe {
        syscall4(
            SYS_OPENAT_HOST,
            AT_FDCWD,
            HOST_DEV_NULL.as_ptr() as usize,
            O_RDWR,
            0,
        )
    };
    if null_fd < 0 {
        native_tracee_setup_fail(119);
    }
    let protected_null = unsafe { syscall3(SYS_FCNTL_HOST, null_fd as usize, 0, DOMAIN_NULL_FD) };
    let _ = unsafe { syscall1(SYS_CLOSE_HOST, null_fd as usize) };
    if protected_null != DOMAIN_NULL_FD as isize {
        native_tracee_setup_fail(119);
    }
    let protected_console = unsafe { syscall3(SYS_FCNTL_HOST, 0, 0, DOMAIN_CONSOLE_FD) };
    if protected_console != DOMAIN_CONSOLE_FD as isize {
        native_tracee_setup_fail(119);
    }

    if unsafe { syscall1(SYS_CHROOT, root_path as usize) } < 0 {
        native_tracee_setup_fail(120);
    }
    if unsafe { syscall1(SYS_CHDIR, ROOT.as_ptr() as usize) } < 0 {
        native_tracee_setup_fail(121);
    }
    let mut inherited = 3usize;
    while inherited < 256 {
        if inherited != DOMAIN_NULL_FD && inherited != DOMAIN_CONSOLE_FD {
            let _ = unsafe { syscall1(SYS_CLOSE_HOST, inherited) };
        }
        inherited += 1;
    }
    if unsafe { syscall4(SYS_PTRACE, PTRACE_TRACEME, 0, 0, 0) } < 0 {
        native_tracee_setup_fail(122);
    }
    if unsafe { syscall2(SYS_SETGROUPS, 0, 0) } < 0 {
        native_tracee_setup_fail(123);
    }
    if unsafe { syscall1(SYS_SETGID, DOMAIN_GID) } < 0 {
        native_tracee_setup_fail(124);
    }
    if unsafe { syscall1(SYS_SETUID, DOMAIN_UID) } < 0 {
        native_tracee_setup_fail(125);
    }
    if unsafe { syscall5(SYS_PRCTL, PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) } < 0 {
        native_tracee_setup_fail(126);
    }
    let bash_argv = [
        BASH.as_ptr(),
        ARG1.as_ptr(),
        ARG2.as_ptr(),
        ARG3.as_ptr(),
        match mode {
            1 => HOLD_COMMAND.as_ptr(),
            2 => PROBE_COMMAND.as_ptr(),
            _ => COMMAND.as_ptr(),
        },
        core::ptr::null(),
    ];
    let mut configured_init = [0u8; 129];
    copy_c_string(config.init.as_bytes(), &mut configured_init);
    let mut configured_args = [[0u8; 129]; 7];
    let mut configured_argv = [core::ptr::null::<u8>(); 9];
    configured_argv[0] = configured_init.as_ptr();
    for (index, argument) in config.args.as_slice().iter().enumerate() {
        copy_c_string(argument.as_bytes(), &mut configured_args[index]);
        configured_argv[index + 1] = configured_args[index].as_ptr();
    }
    let mut configured_environment = [[0u8; 257]; 8];
    let mut environment = [core::ptr::null::<u8>(); 12];
    environment[0] = ENV_PATH.as_ptr();
    environment[1] = ENV_HOME.as_ptr();
    environment[2] = ENV_LOCALE.as_ptr();
    for (index, variable) in config.environment.as_slice().iter().enumerate() {
        copy_c_string(variable.as_bytes(), &mut configured_environment[index]);
        environment[index + 3] = configured_environment[index].as_ptr();
    }
    unsafe {
        syscall3(
            SYS_EXECVE,
            if mode == 3 {
                configured_init.as_ptr() as usize
            } else {
                BASH.as_ptr() as usize
            },
            if mode == 3 {
                configured_argv.as_ptr() as usize
            } else {
                bash_argv.as_ptr() as usize
            },
            environment.as_ptr() as usize,
        );
        syscall1(SYS_EXIT_HOST, 127);
    }
    loop {
        core::hint::spin_loop();
    }
}

fn copy_c_string(source: &[u8], destination: &mut [u8]) {
    if source.len() >= destination.len() {
        native_tracee_setup_fail(118);
    }
    destination[..source.len()].copy_from_slice(source);
    destination[source.len()] = 0;
}

fn native_tracee_setup_fail(code: usize) -> ! {
    unsafe { syscall1(SYS_EXIT_HOST, code) };
    loop {
        core::hint::spin_loop();
    }
}

#[derive(Clone, Copy)]
struct NativeSupervisorTask {
    host_pid: usize,
    virtual_id: TaskId,
    awaiting_exit: bool,
    override_exit: bool,
    override_result: i64,
    pending_console_open: bool,
    console_fd: i64,
    pipe2_address: usize,
    pipe_fds: [i64; 8],
    proc_fds: [u64; NATIVE_PROC_FD_LIMIT / 64],
    cgroup_procs_fds: [u64; NATIVE_PROC_FD_LIMIT / 64],
    pending_cgroup_procs_open: bool,
    capability_bounding: u64,
    personality: u64,
    seccomp_filtered: bool,
    pending_mountinfo_open: bool,
    mountinfo_fd: i64,
    pending_recvmsg: usize,
    pending_peercred: usize,
    pending_stat: usize,
    close_range_first: i64,
    close_range_last: i64,
    syscall: u64,
    translated_rdi: u64,
    waitid_info: u64,
    fork_result_pending: u32,
    fork_breakpoint_rip: usize,
    fork_breakpoint_word: usize,
    initial_stop_pending: bool,
    tracing: bool,
    limits: [[u64; 2]; 16],
}

fn native_default_limits() -> [[u64; 2]; 16] {
    let mut limits = [[u64::MAX; 2]; 16];
    limits[3] = [8 * 1024 * 1024, u64::MAX];
    limits[6] = [4096, 4096];
    limits[7] = [4096, 4096];
    limits[8] = [64 * 1024, 64 * 1024];
    limits
}

fn native_proc_materialize(root_descriptor: usize, task: TaskView, name: &[u8]) -> bool {
    let mut path = [0u8; 64];
    let Some(_) = native_proc_path(task.id.pid, b"", &mut path) else {
        return false;
    };
    let mkdir_result = unsafe {
        syscall3(
            SYS_MKDIRAT_HOST,
            root_descriptor,
            path.as_ptr() as usize,
            0o555,
        )
    };
    if mkdir_result < 0 && mkdir_result != -17 {
        return false;
    }
    let Some(_) = native_proc_path(task.id.pid, b"/fdinfo", &mut path) else {
        return false;
    };
    let mkdir_result = unsafe {
        syscall3(
            SYS_MKDIRAT_HOST,
            root_descriptor,
            path.as_ptr() as usize,
            0o555,
        )
    };
    if mkdir_result < 0 && mkdir_result != -17 {
        return false;
    }
    let Some(_) = native_proc_path(task.id.pid, b"/fd", &mut path) else {
        return false;
    };
    let mkdir_result = unsafe {
        syscall3(
            SYS_MKDIRAT_HOST,
            root_descriptor,
            path.as_ptr() as usize,
            0o555,
        )
    };
    if mkdir_result < 0 && mkdir_result != -17 {
        return false;
    }

    let mut stat = [0u8; 384];
    let Some(stat_len) = native_proc_stat(task, name, &mut stat) else {
        return false;
    };
    let Some(_) = native_proc_path(task.id.pid, b"/stat", &mut path) else {
        return false;
    };
    if !native_proc_write(root_descriptor, &path, &stat[..stat_len]) {
        return false;
    }

    let Some(_) = native_proc_path(task.id.pid, b"/comm", &mut path) else {
        return false;
    };
    let mut comm = [0u8; 64];
    let mut comm_len = 0usize;
    if native_buffer_append(&mut comm, &mut comm_len, name).is_none()
        || native_buffer_append(&mut comm, &mut comm_len, b"\n").is_none()
        || !native_proc_write(root_descriptor, &path, &comm[..comm_len])
    {
        return false;
    }

    let Some(_) = native_proc_path(task.id.pid, b"/cgroup", &mut path) else {
        return false;
    };
    if !native_proc_write(root_descriptor, &path, b"0::/\n") {
        return false;
    }

    let mut status = [0u8; 512];
    let Some(status_len) = native_proc_status(task, name, &mut status) else {
        return false;
    };
    let Some(_) = native_proc_path(task.id.pid, b"/status", &mut path) else {
        return false;
    };
    if !native_proc_write(root_descriptor, &path, &status[..status_len]) {
        return false;
    }

    let Some(_) = native_proc_path(task.id.pid, b"/cmdline", &mut path) else {
        return false;
    };
    let mut cmdline = [0u8; 128];
    let mut cmdline_len = 0usize;
    if !(native_buffer_append(&mut cmdline, &mut cmdline_len, name).is_some()
        && native_buffer_append(&mut cmdline, &mut cmdline_len, b"\0").is_some()
        && native_proc_write(root_descriptor, &path, &cmdline[..cmdline_len]))
    {
        return false;
    }

    let Some(_) = native_proc_path(task.id.pid, b"/oom_score_adj", &mut path) else {
        return false;
    };
    if !native_proc_write_mode(root_descriptor, &path, b"0\n", 0o644) {
        return false;
    }

    let Some(_) = native_proc_path(task.id.pid, b"/mountinfo", &mut path) else {
        return false;
    };
    native_proc_write(
        root_descriptor,
        &path,
        b"1 0 0:1 / / rw,nosuid,nodev - proc proc rw\n",
    )
}

fn native_proc_remove(root_descriptor: usize, pid: u32) {
    let mut path = [0u8; 64];
    for suffix in [
        b"/stat".as_slice(),
        b"/comm",
        b"/cgroup",
        b"/status",
        b"/cmdline",
        b"/oom_score_adj",
        b"/mountinfo",
    ] {
        if native_proc_path(pid, suffix, &mut path).is_some() {
            let _ = unsafe {
                syscall3(
                    SYS_UNLINKAT_HOST,
                    root_descriptor,
                    path.as_ptr() as usize,
                    0,
                )
            };
        }
    }
    if native_proc_path(pid, b"/fd", &mut path).is_some() {
        let _ = unsafe {
            syscall3(
                SYS_UNLINKAT_HOST,
                root_descriptor,
                path.as_ptr() as usize,
                AT_REMOVEDIR,
            )
        };
    }
    if native_proc_path(pid, b"/fdinfo", &mut path).is_some() {
        let _ = unsafe {
            syscall3(
                SYS_UNLINKAT_HOST,
                root_descriptor,
                path.as_ptr() as usize,
                AT_REMOVEDIR,
            )
        };
    }
    if native_proc_path(pid, b"", &mut path).is_some() {
        let _ = unsafe {
            syscall3(
                SYS_UNLINKAT_HOST,
                root_descriptor,
                path.as_ptr() as usize,
                AT_REMOVEDIR,
            )
        };
    }
}

fn native_proc_fd_path(pid: u32, fd: usize, output: &mut [u8]) -> Option<usize> {
    let mut length = 0usize;
    native_buffer_append(output, &mut length, b"proc/")?;
    native_buffer_append_u32(output, &mut length, pid)?;
    native_buffer_append(output, &mut length, b"/fd/")?;
    native_buffer_append_u32(output, &mut length, u32::try_from(fd).ok()?)?;
    if length >= output.len() {
        return None;
    }
    output[length] = 0;
    Some(length)
}

fn native_proc_fdinfo_path(pid: u32, fd: usize, output: &mut [u8]) -> Option<usize> {
    let mut length = 0usize;
    native_buffer_append(output, &mut length, b"proc/")?;
    native_buffer_append_u32(output, &mut length, pid)?;
    native_buffer_append(output, &mut length, b"/fdinfo/")?;
    native_buffer_append_u32(output, &mut length, u32::try_from(fd).ok()?)?;
    if length >= output.len() {
        return None;
    }
    output[length] = 0;
    Some(length)
}

fn native_proc_materialize_pidfd_info(
    root_descriptor: usize,
    caller_host_pid: usize,
    caller_virtual_pid: u32,
    fd: usize,
    tasks: &[Option<NativeSupervisorTask>; 128],
) -> bool {
    let Some(target_host_pid) = native_pidfd_host_target(caller_host_pid, fd) else {
        return false;
    };
    let target_virtual_pid = if target_host_pid == caller_host_pid {
        caller_virtual_pid
    } else {
        let Some(target_virtual_pid) = tasks
            .iter()
            .flatten()
            .find(|task| task.host_pid == target_host_pid)
            .map(|task| task.virtual_id.pid)
        else {
            return false;
        };
        target_virtual_pid
    };

    let mut virtual_info = [0u8; 192];
    let mut length = 0usize;
    if native_buffer_append(
        &mut virtual_info,
        &mut length,
        b"pos:\t0\nflags:\t02000002\nmnt_id:\t5\nino:\t1\nPid:\t",
    )
    .is_none()
        || native_buffer_append_u32(&mut virtual_info, &mut length, target_virtual_pid).is_none()
        || native_buffer_append(&mut virtual_info, &mut length, b"\nNSpid:\t").is_none()
        || native_buffer_append_u32(&mut virtual_info, &mut length, target_virtual_pid).is_none()
        || native_buffer_append(&mut virtual_info, &mut length, b"\n").is_none()
    {
        return false;
    }
    let mut path = [0u8; 96];
    let Some(_) = native_proc_fdinfo_path(caller_virtual_pid, fd, &mut path) else {
        return false;
    };
    native_proc_write(root_descriptor, &path, &virtual_info[..length])
}

fn native_pidfd_host_target(caller_host_pid: usize, fd: usize) -> Option<usize> {
    let mut host_path = [0u8; 96];
    let mut host_path_len = 0usize;
    if native_buffer_append(&mut host_path, &mut host_path_len, b"/proc/").is_none()
        || native_buffer_append_u32(
            &mut host_path,
            &mut host_path_len,
            u32::try_from(caller_host_pid).unwrap_or(u32::MAX),
        )
        .is_none()
        || native_buffer_append(&mut host_path, &mut host_path_len, b"/fdinfo/").is_none()
        || native_buffer_append_u32(
            &mut host_path,
            &mut host_path_len,
            u32::try_from(fd).unwrap_or(u32::MAX),
        )
        .is_none()
        || host_path_len >= host_path.len()
    {
        return None;
    }
    host_path[host_path_len] = 0;

    let descriptor = unsafe {
        syscall4(
            SYS_OPENAT_HOST,
            AT_FDCWD,
            host_path.as_ptr() as usize,
            O_RDONLY,
            0,
        )
    };
    if descriptor < 0 {
        return None;
    }
    let mut host_info = [0u8; 512];
    let count = unsafe {
        syscall3(
            SYS_READ_HOST,
            descriptor as usize,
            host_info.as_mut_ptr() as usize,
            host_info.len(),
        )
    };
    let _ = unsafe { syscall1(SYS_CLOSE_HOST, descriptor as usize) };
    if count <= 0 {
        return None;
    }

    let host_info = &host_info[..count as usize];
    let pid_field = host_info.windows(5).position(|field| field == b"Pid:\t")?;
    let mut target_host_pid = 0usize;
    let mut digits = 0usize;
    for byte in &host_info[pid_field + 5..] {
        if !byte.is_ascii_digit() {
            break;
        }
        target_host_pid = target_host_pid
            .saturating_mul(10)
            .saturating_add(usize::from(*byte - b'0'));
        digits += 1;
    }
    if digits == 0 {
        return None;
    }
    Some(target_host_pid)
}

fn native_proc_clear_fds(
    root_descriptor: usize,
    virtual_pid: u32,
    entries: &mut [u64; NATIVE_PROC_FD_LIMIT / 64],
) -> bool {
    let mut entry_path = [0u8; 80];
    for fd in 0..NATIVE_PROC_FD_LIMIT {
        if entries[fd / 64] & (1u64 << (fd % 64)) == 0 {
            continue;
        }
        let Some(_) = native_proc_fd_path(virtual_pid, fd, &mut entry_path) else {
            return false;
        };
        let result = unsafe {
            syscall3(
                SYS_UNLINKAT_HOST,
                root_descriptor,
                entry_path.as_ptr() as usize,
                0,
            )
        };
        if result < 0 && result != -2 {
            return false;
        }
    }
    *entries = [0; NATIVE_PROC_FD_LIMIT / 64];
    true
}

fn native_fd_set(entries: &mut [u64; NATIVE_PROC_FD_LIMIT / 64], fd: usize) -> bool {
    if fd >= NATIVE_PROC_FD_LIMIT {
        return false;
    }
    entries[fd / 64] |= 1u64 << (fd % 64);
    true
}

fn native_fd_contains(entries: &[u64; NATIVE_PROC_FD_LIMIT / 64], fd: usize) -> bool {
    fd < NATIVE_PROC_FD_LIMIT && entries[fd / 64] & (1u64 << (fd % 64)) != 0
}

fn native_fd_clear(entries: &mut [u64; NATIVE_PROC_FD_LIMIT / 64], fd: usize) {
    if fd < NATIVE_PROC_FD_LIMIT {
        entries[fd / 64] &= !(1u64 << (fd % 64));
    }
}

fn native_fd_clear_range(
    entries: &mut [u64; NATIVE_PROC_FD_LIMIT / 64],
    first: usize,
    last: usize,
) {
    if first >= NATIVE_PROC_FD_LIMIT || first > last {
        return;
    }
    let last = core::cmp::min(last, NATIVE_PROC_FD_LIMIT.saturating_sub(1));
    for fd in first..=last {
        native_fd_clear(entries, fd);
    }
}

fn native_proc_refresh_fds(
    root_descriptor: usize,
    host_pid: usize,
    virtual_pid: u32,
    previous: &mut [u64; NATIVE_PROC_FD_LIMIT / 64],
) -> bool {
    if !native_proc_clear_fds(root_descriptor, virtual_pid, previous) {
        return false;
    }
    let mut entry_path = [0u8; 80];

    let mut host_path = [0u8; 64];
    let mut host_path_len = 0usize;
    if native_buffer_append(&mut host_path, &mut host_path_len, b"/proc/").is_none()
        || native_buffer_append_u32(
            &mut host_path,
            &mut host_path_len,
            u32::try_from(host_pid).unwrap_or(u32::MAX),
        )
        .is_none()
        || native_buffer_append(&mut host_path, &mut host_path_len, b"/fd").is_none()
        || host_path_len >= host_path.len()
    {
        return false;
    }
    host_path[host_path_len] = 0;
    let directory = unsafe {
        syscall4(
            SYS_OPENAT_HOST,
            AT_FDCWD,
            host_path.as_ptr() as usize,
            O_RDONLY | O_DIRECTORY_HOST,
            0,
        )
    };
    if directory < 0 {
        return false;
    }

    let mut entries = [0u8; 4096];
    let mut success = true;
    loop {
        let count = unsafe {
            syscall3(
                SYS_GETDENTS64_HOST,
                directory as usize,
                entries.as_mut_ptr() as usize,
                entries.len(),
            )
        };
        if count < 0 {
            success = false;
            break;
        }
        if count == 0 {
            break;
        }
        let mut offset = 0usize;
        while offset < count as usize {
            if offset.saturating_add(19) > count as usize {
                success = false;
                break;
            }
            let record_length =
                u16::from_ne_bytes([entries[offset + 16], entries[offset + 17]]) as usize;
            if record_length < 20 || offset.saturating_add(record_length) > count as usize {
                success = false;
                break;
            }
            let mut fd = 0usize;
            let mut digits = 0usize;
            for byte in &entries[offset + 19..offset + record_length] {
                if *byte == 0 {
                    break;
                }
                if !byte.is_ascii_digit() {
                    digits = 0;
                    break;
                }
                fd = fd
                    .saturating_mul(10)
                    .saturating_add(usize::from(*byte - b'0'));
                digits += 1;
            }
            if digits > 0
                && fd < NATIVE_PROC_FD_LIMIT
                && !matches!(fd, DOMAIN_NULL_FD | DOMAIN_CONSOLE_FD)
            {
                let Some(_) = native_proc_fd_path(virtual_pid, fd, &mut entry_path) else {
                    success = false;
                    break;
                };
                let target = b"ginux-fd\0";
                let result = unsafe {
                    syscall3(
                        SYS_SYMLINKAT_HOST,
                        target.as_ptr() as usize,
                        root_descriptor,
                        entry_path.as_ptr() as usize,
                    )
                };
                if result < 0 && result != -17 {
                    success = false;
                    break;
                }
                previous[fd / 64] |= 1u64 << (fd % 64);
            }
            offset += record_length;
        }
        if !success {
            break;
        }
    }
    let _ = unsafe { syscall1(SYS_CLOSE_HOST, directory as usize) };
    success
}

fn native_proc_path(pid: u32, suffix: &[u8], output: &mut [u8]) -> Option<usize> {
    let mut length = 0usize;
    native_buffer_append(output, &mut length, b"proc/")?;
    native_buffer_append_u32(output, &mut length, pid)?;
    native_buffer_append(output, &mut length, suffix)?;
    if length >= output.len() {
        return None;
    }
    output[length] = 0;
    Some(length)
}

fn native_proc_stat(task: TaskView, name: &[u8], output: &mut [u8]) -> Option<usize> {
    let mut length = 0usize;
    native_buffer_append_u32(output, &mut length, task.id.pid)?;
    native_buffer_append(output, &mut length, b" (")?;
    native_buffer_append(output, &mut length, name)?;
    native_buffer_append(output, &mut length, b") S ")?;
    native_buffer_append_u32(output, &mut length, task.parent)?;
    native_buffer_append(output, &mut length, b" ")?;
    native_buffer_append_u32(output, &mut length, task.process_group)?;
    native_buffer_append(output, &mut length, b" ")?;
    native_buffer_append_u32(output, &mut length, task.session)?;
    native_buffer_append(
        output,
        &mut length,
        b" 0 -1 0 0 0 0 0 0 0 0 0 20 0 1 0 1 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 17 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0\n",
    )?;
    Some(length)
}

fn native_proc_status(task: TaskView, name: &[u8], output: &mut [u8]) -> Option<usize> {
    let mut length = 0usize;
    native_buffer_append(output, &mut length, b"Name:\t")?;
    native_buffer_append(output, &mut length, name)?;
    native_buffer_append(output, &mut length, b"\nState:\tS (sleeping)\nTgid:\t")?;
    native_buffer_append_u32(output, &mut length, task.tgid)?;
    native_buffer_append(output, &mut length, b"\nPid:\t")?;
    native_buffer_append_u32(output, &mut length, task.id.pid)?;
    native_buffer_append(output, &mut length, b"\nPPid:\t")?;
    native_buffer_append_u32(output, &mut length, task.parent)?;
    native_buffer_append(output, &mut length, b"\nTracerPid:\t0\nUid:\t")?;
    for _ in 0..4 {
        native_buffer_append_u32(output, &mut length, task.credentials.uid)?;
        native_buffer_append(output, &mut length, b"\t")?;
    }
    native_buffer_append(output, &mut length, b"\nGid:\t")?;
    for _ in 0..4 {
        native_buffer_append_u32(output, &mut length, task.credentials.gid)?;
        native_buffer_append(output, &mut length, b"\t")?;
    }
    native_buffer_append(output, &mut length, b"\nGroups:\t\nNStgid:\t")?;
    native_buffer_append_u32(output, &mut length, task.tgid)?;
    native_buffer_append(output, &mut length, b"\nNSpid:\t")?;
    native_buffer_append_u32(output, &mut length, task.id.pid)?;
    native_buffer_append(output, &mut length, b"\nNSpgid:\t")?;
    native_buffer_append_u32(output, &mut length, task.process_group)?;
    native_buffer_append(output, &mut length, b"\nNSsid:\t")?;
    native_buffer_append_u32(output, &mut length, task.session)?;
    native_buffer_append(output, &mut length, b"\nThreads:\t1\n\n")?;
    Some(length)
}

fn native_buffer_append(output: &mut [u8], length: &mut usize, bytes: &[u8]) -> Option<()> {
    let end = length.checked_add(bytes.len())?;
    if end > output.len() {
        return None;
    }
    output[*length..end].copy_from_slice(bytes);
    *length = end;
    Some(())
}

fn native_buffer_append_u32(output: &mut [u8], length: &mut usize, mut value: u32) -> Option<()> {
    let mut digits = [0u8; 10];
    let mut start = digits.len();
    if value == 0 {
        return native_buffer_append(output, length, b"0");
    }
    while value != 0 {
        start -= 1;
        digits[start] = b'0' + (value % 10) as u8;
        value /= 10;
    }
    native_buffer_append(output, length, &digits[start..])
}

fn native_proc_write(root_descriptor: usize, path: &[u8], bytes: &[u8]) -> bool {
    native_proc_write_mode(root_descriptor, path, bytes, 0o444)
}

fn native_proc_write_mode(root_descriptor: usize, path: &[u8], bytes: &[u8], mode: usize) -> bool {
    let descriptor = unsafe {
        syscall4(
            SYS_OPENAT_HOST,
            root_descriptor,
            path.as_ptr() as usize,
            O_WRONLY | O_CREATE_HOST | O_TRUNC_HOST,
            mode,
        )
    };
    if descriptor < 0 {
        return false;
    }
    let mut offset = 0usize;
    while offset < bytes.len() {
        let result = unsafe {
            syscall3(
                SYS_WRITE_HOST,
                descriptor as usize,
                bytes.as_ptr().add(offset) as usize,
                bytes.len() - offset,
            )
        };
        if result <= 0 {
            let _ = unsafe { syscall1(SYS_CLOSE_HOST, descriptor as usize) };
            return false;
        }
        offset += result as usize;
    }
    unsafe { syscall1(SYS_CLOSE_HOST, descriptor as usize) == 0 }
}

fn supervise_native(pid: usize, domain: &mut Domain, mode: usize, root_descriptor: usize) -> ! {
    let write_all = |bytes: &[u8]| write_all_to(2, bytes);
    let write_u64 = |value: u64| write_u64_to(2, value);
    let write_i64 = |value: i64| write_i64_to(2, value);
    let write_tracee_string = |pid: usize, address: usize| {
        write_tracee_string_to(2, pid, address);
    };
    let line = |message: &[u8]| line_to(2, message);

    let mut status = 0i32;
    if wait(pid, &mut status) < 0 {
        native_fail(domain, pid, b"native tracee wait after exec failed");
    }
    if !stopped(status) {
        write_all(b"GINUX_NATIVE: pre-exec-status=");
        write_i64(status as i64);
        write_all(b"\n");
        native_fail(domain, pid, b"native tracee did not stop after exec");
    }
    let options = PTRACE_O_TRACESYSGOOD
        | PTRACE_O_TRACEFORK
        | PTRACE_O_TRACEVFORK
        | PTRACE_O_TRACECLONE
        | PTRACE_O_TRACEEXEC
        | PTRACE_O_EXITKILL;
    if ptrace(PTRACE_SETOPTIONS, pid, 0, options) < 0 || ptrace(PTRACE_SYSCALL, pid, 0, 0) < 0 {
        native_fail(domain, pid, b"native ptrace setup failed");
    }
    if mode == 3 {
        line(b"GINUX_NATIVE: backend=ptrace-native profile=balanced elf-loader=host contract=gate9-systemd");
        line(b"GINUX_NATIVE: virtual-pid=1 virtual-ppid=0 root=oci-overlay");
    } else {
        line(b"GINUX_NATIVE: backend=ptrace-native profile=balanced elf-loader=host contract=gate5-balanced");
        line(b"GINUX_NATIVE: tracee-uid=1000 tracee-gid=1000 root=oci-read-only");
    }

    let mut sequence = 0u64;
    let mut processes: ProcessTable<128> = ProcessTable::new();
    let virtual_credentials = if mode == 3 {
        Credentials::root()
    } else {
        Credentials::unprivileged(DOMAIN_UID as u32, DOMAIN_GID as u32)
    };
    let root_virtual = processes
        .create_init(virtual_credentials)
        .unwrap_or_else(|_| native_fail(domain, pid, b"native virtual init failed"));
    let mut tasks: [Option<NativeSupervisorTask>; 128] = [None; 128];
    let mut pending_child_stops = [0usize; 16];
    let mut peak_managed_tasks = 1usize;
    tasks[0] = Some(NativeSupervisorTask {
        host_pid: pid,
        virtual_id: root_virtual,
        awaiting_exit: false,
        override_exit: false,
        override_result: 0,
        pending_console_open: false,
        console_fd: DOMAIN_CONSOLE_FD as i64,
        pipe2_address: 0,
        pipe_fds: [-1; 8],
        proc_fds: [0; NATIVE_PROC_FD_LIMIT / 64],
        cgroup_procs_fds: [0; NATIVE_PROC_FD_LIMIT / 64],
        pending_cgroup_procs_open: false,
        capability_bounding: (1u64 << 41) - 1,
        personality: 0,
        seccomp_filtered: false,
        pending_mountinfo_open: false,
        mountinfo_fd: -1,
        pending_recvmsg: 0,
        pending_peercred: 0,
        pending_stat: 0,
        close_range_first: -1,
        close_range_last: -1,
        syscall: 0,
        translated_rdi: 0,
        waitid_info: 0,
        fork_result_pending: 0,
        fork_breakpoint_rip: 0,
        fork_breakpoint_word: 0,
        initial_stop_pending: false,
        tracing: true,
        limits: native_default_limits(),
    });
    if mode == 3 {
        let root_view = processes
            .view(root_virtual)
            .unwrap_or_else(|_| native_fail(domain, pid, b"native root proc view failed"));
        if !native_proc_materialize(root_descriptor, root_view, b"systemd") {
            native_fail(domain, pid, b"native root proc materialization failed");
        }
    }
    loop {
        status = 0;
        let waited = unsafe {
            syscall4(
                SYS_WAIT4,
                (-1isize) as usize,
                &mut status as *mut _ as usize,
                0,
                0,
            )
        };
        if waited <= 0 {
            native_fail(domain, pid, b"native wait-any failed");
        }
        let host_pid = waited as usize;
        let task_index = tasks
            .iter()
            .position(|task| task.is_some_and(|task| task.host_pid == host_pid));
        let Some(task_index) = task_index else {
            let initial_child_stop = stopped(status) && ((status >> 8) & 0xff) as u8 == 19;
            let Some(pending_slot) = pending_child_stops.iter_mut().find(|child| **child == 0)
            else {
                native_fail(domain, pid, b"pending native child table full");
            };
            if initial_child_stop {
                *pending_slot = host_pid;
                continue;
            }
            native_fail(domain, pid, b"unknown native tracee");
        };
        let mut task = tasks[task_index].unwrap();
        if !stopped(status) {
            let exit_status = if status & 0x7f == 0 {
                status >> 8
            } else {
                128 + (status & 0x7f)
            };
            if processes.exit(task.virtual_id, exit_status).is_err() {
                native_fail(domain, pid, b"native virtual exit failed");
            }
            if mode == 3 && task.virtual_id != root_virtual {
                let _ =
                    native_proc_clear_fds(root_descriptor, task.virtual_id.pid, &mut task.proc_fds);
                native_proc_remove(root_descriptor, task.virtual_id.pid);
            }
            task.tracing = false;
            tasks[task_index] = Some(task);
            if task.virtual_id == root_virtual {
                for descendant in tasks.iter().flatten() {
                    if descendant.tracing {
                        let _ = ptrace(PTRACE_KILL, descendant.host_pid, 0, 0);
                    }
                }
                if domain.task_exited(exit_status == 0).is_err() {
                    domain_fail(domain, b"native exit transition failed");
                }
                line(b"GINUX_NATIVE: supervisor=attached direct-unsupervised-syscalls=0");
                write_all(b"GINUX_NATIVE_MEASURE: peak-managed-tasks=");
                write_u64(peak_managed_tasks as u64);
                write_all(b"\n");
                if exit_status == 0 {
                    line(b"GINUX_NATIVE_CHECK: PASS");
                }
                exit_process(exit_status as usize)
            }
            continue;
        }
        let signal = ((status >> 8) & 0xff) as u8;
        let event = ((status as usize) >> 16) & 0xffff;
        if matches!(
            event,
            PTRACE_EVENT_FORK | PTRACE_EVENT_VFORK | PTRACE_EVENT_CLONE
        ) {
            let mut child_host_pid = 0usize;
            if ptrace(
                PTRACE_GETEVENTMSG,
                host_pid,
                0,
                &mut child_host_pid as *mut _ as usize,
            ) < 0
                || child_host_pid == 0
            {
                native_fail(domain, pid, b"native child event read failed");
            }
            let child_virtual = if event == PTRACE_EVENT_CLONE {
                processes.clone_thread(task.virtual_id)
            } else {
                processes.fork(task.virtual_id)
            }
            .unwrap_or_else(|_| native_fail(domain, pid, b"native virtual child failed"));
            if mode == 3 {
                let child_view = processes.view(child_virtual).unwrap_or_else(|_| {
                    native_fail(domain, pid, b"native virtual child view failed")
                });
                if !native_proc_materialize(root_descriptor, child_view, b"ginux-task") {
                    native_fail(domain, pid, b"native virtual proc materialization failed");
                }
            }
            let Some(child_slot) = tasks.iter().position(Option::is_none) else {
                native_fail(domain, pid, b"native task table full");
            };
            let pending_child_slot = pending_child_stops
                .iter()
                .position(|child| *child == child_host_pid);
            tasks[child_slot] = Some(NativeSupervisorTask {
                host_pid: child_host_pid,
                virtual_id: child_virtual,
                awaiting_exit: false,
                override_exit: false,
                override_result: 0,
                pending_console_open: false,
                console_fd: task.console_fd,
                pipe2_address: 0,
                pipe_fds: task.pipe_fds,
                proc_fds: [0; NATIVE_PROC_FD_LIMIT / 64],
                cgroup_procs_fds: task.cgroup_procs_fds,
                pending_cgroup_procs_open: false,
                capability_bounding: task.capability_bounding,
                personality: task.personality,
                seccomp_filtered: task.seccomp_filtered,
                pending_mountinfo_open: false,
                mountinfo_fd: task.mountinfo_fd,
                pending_recvmsg: 0,
                pending_peercred: 0,
                pending_stat: 0,
                close_range_first: -1,
                close_range_last: -1,
                syscall: 0,
                translated_rdi: 0,
                waitid_info: 0,
                fork_result_pending: 0,
                fork_breakpoint_rip: 0,
                fork_breakpoint_word: 0,
                initial_stop_pending: pending_child_slot.is_none(),
                tracing: true,
                limits: task.limits,
            });
            peak_managed_tasks = core::cmp::max(peak_managed_tasks, processes.active());
            if let Some(pending_slot) = pending_child_slot {
                pending_child_stops[pending_slot] = 0;
                let _ = ptrace(PTRACE_SETOPTIONS, child_host_pid, 0, options);
                if ptrace(PTRACE_SYSCALL, child_host_pid, 0, 0) < 0 {
                    native_fail(domain, pid, b"pending native child resume failed");
                }
            }
            if event == PTRACE_EVENT_VFORK {
                task.awaiting_exit = true;
                task.override_exit = true;
                task.override_result = child_virtual.pid as i64;
                task.fork_result_pending = 0;
                task.fork_breakpoint_rip = 0;
                task.fork_breakpoint_word = 0;
                tasks[task_index] = Some(task);
                if ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0 {
                    native_fail(domain, pid, b"native vfork parent resume failed");
                }
                continue;
            }
            let mut parent_registers = core::mem::MaybeUninit::<UserRegs>::uninit();
            if ptrace(
                PTRACE_GETREGS,
                host_pid,
                0,
                parent_registers.as_mut_ptr() as usize,
            ) < 0
            {
                native_fail(domain, pid, b"native fork registers failed");
            }
            let parent_registers = unsafe { parent_registers.assume_init() };
            let breakpoint_rip = parent_registers.rip as usize;
            let mut breakpoint_word = 0usize;
            if ptrace(
                PTRACE_PEEKTEXT,
                host_pid,
                breakpoint_rip,
                &mut breakpoint_word as *mut _ as usize,
            ) < 0
                || ptrace(
                    PTRACE_POKETEXT,
                    host_pid,
                    breakpoint_rip,
                    (breakpoint_word & !0xff) | 0xcc,
                ) < 0
            {
                native_fail(domain, pid, b"native fork breakpoint failed");
            }
            task.awaiting_exit = false;
            task.override_exit = false;
            task.fork_result_pending = child_virtual.pid;
            task.fork_breakpoint_rip = breakpoint_rip;
            task.fork_breakpoint_word = breakpoint_word;
            tasks[task_index] = Some(task);
            if ptrace(PTRACE_CONT, host_pid, 0, 0) < 0 {
                native_fail(domain, pid, b"native parent resume after child failed");
            }
            continue;
        }
        if event == PTRACE_EVENT_EXEC {
            let _ = processes.exec(task.virtual_id);
            if mode == 3 && task.virtual_id != root_virtual {
                let task_view = processes
                    .view(task.virtual_id)
                    .unwrap_or_else(|_| native_fail(domain, pid, b"native exec view failed"));
                if !native_proc_materialize(root_descriptor, task_view, b"systemd-executor") {
                    native_fail(domain, pid, b"native exec proc update failed");
                }
            }
            tasks[task_index] = Some(task);
            if ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0 {
                native_fail(domain, pid, b"native resume after exec failed");
            }
            continue;
        }
        if event != 0 {
            native_fail(domain, pid, b"unexpected native ptrace event");
        }
        if signal == 19 {
            if !task.initial_stop_pending {
                native_fail(domain, pid, b"unexpected native trace stop");
            }
            task.initial_stop_pending = false;
            tasks[task_index] = Some(task);
            let _ = ptrace(PTRACE_SETOPTIONS, host_pid, 0, options);
            if ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0 {
                native_fail(domain, pid, b"native child initial resume failed");
            }
            continue;
        }
        if signal == 17 {
            if ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0 {
                native_fail(domain, pid, b"native SIGCHLD resume failed");
            }
            continue;
        }
        if signal == 5 && task.fork_result_pending != 0 {
            let mut parent_registers = core::mem::MaybeUninit::<UserRegs>::uninit();
            if ptrace(
                PTRACE_GETREGS,
                host_pid,
                0,
                parent_registers.as_mut_ptr() as usize,
            ) < 0
            {
                native_fail(domain, pid, b"native fork result read failed");
            }
            let mut parent_registers = unsafe { parent_registers.assume_init() };
            if ptrace(
                PTRACE_POKETEXT,
                host_pid,
                task.fork_breakpoint_rip,
                task.fork_breakpoint_word,
            ) < 0
            {
                native_fail(domain, pid, b"native fork breakpoint restore failed");
            }
            parent_registers.rip = task.fork_breakpoint_rip as u64;
            parent_registers.rax = task.fork_result_pending as u64;
            if ptrace(
                PTRACE_SETREGS,
                host_pid,
                0,
                &parent_registers as *const _ as usize,
            ) < 0
            {
                native_fail(domain, pid, b"native fork result translation failed");
            }
            task.fork_result_pending = 0;
            task.fork_breakpoint_rip = 0;
            task.fork_breakpoint_word = 0;
            tasks[task_index] = Some(task);
            if ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0 {
                native_fail(
                    domain,
                    pid,
                    b"native parent resume after translation failed",
                );
            }
            continue;
        }
        if signal != (5 | 0x80) && signal != 5 {
            write_all(b"GINUX_NATIVE: unexpected-signal=");
            write_u64(signal as u64);
            write_all(b" event=");
            write_u64(event as u64);
            write_all(b" host-pid=");
            write_u64(host_pid as u64);
            write_all(b" virtual-pid=");
            write_u64(task.virtual_id.pid as u64);
            write_all(b"\n");
            tasks[task_index] = Some(task);
            if ptrace(PTRACE_SYSCALL, host_pid, 0, signal as usize) < 0 {
                native_fail(domain, pid, b"native signal delivery failed");
            }
            continue;
        }
        let mut registers = core::mem::MaybeUninit::<UserRegs>::uninit();
        if ptrace(PTRACE_GETREGS, host_pid, 0, registers.as_mut_ptr() as usize) < 0 {
            native_fail(domain, pid, b"native register read failed");
        }
        let mut registers = unsafe { registers.assume_init() };

        if !task.awaiting_exit && registers.orig_rax == u64::MAX {
            tasks[task_index] = Some(task);
            if ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0 {
                native_fail(domain, pid, b"native child phase synchronization failed");
            }
            continue;
        }

        if task.awaiting_exit {
            if task.syscall == 247 && registers.rax as i64 == 0 && task.waitid_info != 0 {
                let mut identity = 0u64;
                if ptrace(
                    PTRACE_PEEKDATA,
                    host_pid,
                    task.waitid_info as usize + 16,
                    &mut identity as *mut _ as usize,
                ) < 0
                {
                    native_fail(domain, pid, b"native waitid result read failed");
                }
                let waited_host = identity as u32 as usize;
                if waited_host != 0 {
                    let Some(waited_task) = tasks
                        .iter()
                        .flatten()
                        .find(|candidate| candidate.host_pid == waited_host)
                        .copied()
                    else {
                        native_fail(domain, pid, b"native waitid result unknown");
                    };
                    let translated =
                        (identity & !u32::MAX as u64) | u64::from(waited_task.virtual_id.pid);
                    if ptrace(
                        PTRACE_POKEDATA,
                        host_pid,
                        task.waitid_info as usize + 16,
                        translated as usize,
                    ) < 0
                    {
                        native_fail(domain, pid, b"native waitid result translation failed");
                    }
                }
            }
            if task.syscall == 61 && registers.rax as i64 > 0 {
                let waited_host = registers.rax as usize;
                let Some(waited_task) = tasks
                    .iter()
                    .flatten()
                    .find(|candidate| candidate.host_pid == waited_host)
                    .copied()
                else {
                    native_fail(domain, pid, b"native wait result unknown");
                };
                if processes
                    .wait(task.virtual_id, Some(waited_task.virtual_id.pid))
                    .is_err()
                {
                    native_fail(domain, pid, b"native virtual wait result invalid");
                }
                registers.rax = waited_task.virtual_id.pid as u64;
                task.override_exit = true;
                if let Some(waited_slot) = tasks.iter_mut().find(|candidate| {
                    candidate.is_some_and(|candidate| candidate.host_pid == waited_host)
                }) {
                    *waited_slot = None;
                }
            }
            if task.translated_rdi != 0 {
                registers.rdi = task.translated_rdi;
            }
            if task.override_exit {
                if task.syscall != 61 || registers.rax as i64 <= 0 {
                    registers.rax = task.override_result as u64;
                }
                if ptrace(PTRACE_SETREGS, host_pid, 0, &registers as *const _ as usize) < 0 {
                    native_fail(domain, pid, b"native result override failed");
                }
            }
            write_all(b"GINUX_NATIVE_AUDIT: sequence=");
            write_u64(sequence);
            write_all(b" virtual-pid=");
            write_u64(task.virtual_id.pid as u64);
            write_all(b" phase=exit result=");
            write_i64(registers.rax as i64);
            write_all(b"\n");
            if task.pending_console_open {
                task.console_fd = registers.rax as i64;
                task.pending_console_open = false;
            }
            if task.pending_mountinfo_open {
                if registers.rax as i64 >= 0 {
                    task.mountinfo_fd = registers.rax as i64;
                }
                task.pending_mountinfo_open = false;
            }
            if task.pending_cgroup_procs_open {
                if registers.rax as i64 >= 0
                    && !native_fd_set(&mut task.cgroup_procs_fds, registers.rax as usize)
                {
                    native_fail(domain, pid, b"native cgroup descriptor table full");
                }
                task.pending_cgroup_procs_open = false;
            }
            if task.pending_recvmsg != 0 {
                if registers.rax as i64 >= 0
                    && !native_rewrite_recvmsg_credentials(
                        host_pid,
                        task.pending_recvmsg,
                        &tasks,
                        task,
                        &processes,
                    )
                {
                    native_fail(domain, pid, b"native recvmsg credential translation failed");
                }
                task.pending_recvmsg = 0;
            }
            if task.pending_peercred != 0 {
                if registers.rax as i64 == 0
                    && !native_rewrite_ucred(
                        host_pid,
                        task.pending_peercred,
                        &tasks,
                        task,
                        &processes,
                    )
                {
                    native_fail(domain, pid, b"native peer credential translation failed");
                }
                task.pending_peercred = 0;
            }
            if task.pending_stat != 0 {
                if registers.rax as i64 == 0 {
                    let view = processes.view(task.virtual_id).unwrap_or_else(|_| {
                        native_fail(domain, pid, b"native stat identity failed")
                    });
                    if !native_rewrite_stat_identity(
                        host_pid,
                        task.pending_stat,
                        view.credentials.uid,
                        view.credentials.gid,
                    ) {
                        native_fail(domain, pid, b"native stat identity translation failed");
                    }
                }
                task.pending_stat = 0;
            }
            if task.syscall == 293 && registers.rax as i64 == 0 && task.pipe2_address != 0 {
                let read_fd = read_tracee_u32(host_pid, task.pipe2_address);
                let write_fd = read_tracee_u32(host_pid, task.pipe2_address + 4);
                let Some((read_fd, write_fd)) = read_fd.zip(write_fd) else {
                    native_fail(domain, pid, b"native pipe2 descriptor read failed");
                };
                for pipe_fd in [i64::from(read_fd), i64::from(write_fd)] {
                    let Some(slot) = task.pipe_fds.iter_mut().find(|fd| **fd < 0) else {
                        native_fail(domain, pid, b"native pipe descriptor table full");
                    };
                    *slot = pipe_fd;
                }
            }
            if task.syscall == ginux_core::SYS_CLOSE_RANGE && registers.rax as i64 == 0 {
                for pipe_fd in &mut task.pipe_fds {
                    if *pipe_fd >= task.close_range_first && *pipe_fd <= task.close_range_last {
                        *pipe_fd = -1;
                    }
                }
                if task.close_range_first >= 0 && task.close_range_last >= 0 {
                    native_fd_clear_range(
                        &mut task.cgroup_procs_fds,
                        task.close_range_first as usize,
                        task.close_range_last as usize,
                    );
                }
            }
            task.awaiting_exit = false;
            task.override_exit = false;
            task.translated_rdi = 0;
            task.waitid_info = 0;
            task.pipe2_address = 0;
            task.close_range_first = -1;
            task.close_range_last = -1;
            tasks[task_index] = Some(task);
            if ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0 {
                native_fail(domain, pid, b"native resume failed");
            }
            continue;
        }

        sequence += 1;
        let syscall = registers.orig_rax;
        task.syscall = syscall;
        let shared_network = mode == 3 && domain.config().network_backend == NetworkBackend::Shared;
        let decision = if mode == 3 {
            decide_systemd(syscall, shared_network)
        } else {
            decide_balanced(syscall)
        };
        write_all(b"GINUX_NATIVE_AUDIT: sequence=");
        write_u64(sequence);
        write_all(b" virtual-pid=");
        write_u64(task.virtual_id.pid as u64);
        write_all(b" phase=entry syscall=");
        write_u64(syscall);
        if mode == 3 && syscall == 59 {
            write_all(b" path=");
            write_tracee_string(host_pid, registers.rdi as usize);
        } else if mode == 3 && matches!(syscall, 257 | ginux_core::SYS_READLINKAT) {
            write_all(b" path=");
            write_tracee_string(host_pid, registers.rsi as usize);
        } else if mode == 3 && syscall == 16 {
            write_all(b" fd=");
            write_u64(registers.rdi);
            write_all(b" request=");
            write_u64(registers.rsi);
        } else if mode == 3 && syscall == 165 {
            write_all(b" source=");
            write_tracee_string(host_pid, registers.rdi as usize);
            write_all(b" target=");
            write_tracee_string(host_pid, registers.rsi as usize);
            write_all(b" type=");
            write_tracee_string(host_pid, registers.rdx as usize);
            write_all(b" flags=");
            write_u64(registers.r10);
        } else if mode == 3 && matches!(syscall, 33 | 292) {
            write_all(b" source-fd=");
            write_u64(registers.rdi);
            write_all(b" target-fd=");
            write_u64(registers.rsi);
            if syscall == 292 {
                write_all(b" flags=");
                write_u64(registers.rdx);
            }
        } else if mode == 3 && syscall == 247 {
            write_all(b" idtype=");
            write_u64(registers.rdi);
            write_all(b" id=");
            write_u64(registers.rsi);
            write_all(b" options=");
            write_u64(registers.r10);
        } else if mode == 3 && syscall == 72 {
            write_all(b" fd=");
            write_u64(registers.rdi);
            write_all(b" command=");
            write_u64(registers.rsi);
            write_all(b" argument=");
            write_u64(registers.rdx);
        } else if mode == 3 && syscall == ginux_core::SYS_CLOSE_RANGE {
            write_all(b" first=");
            write_u64(registers.rdi);
            write_all(b" last=");
            write_u64(registers.rsi);
            write_all(b" flags=");
            write_u64(registers.rdx);
        } else if mode == 3 && syscall == ginux_core::SYS_PIDFD_SEND_SIGNAL {
            write_all(b" fd=");
            write_u64(registers.rdi);
            write_all(b" signal=");
            write_u64(registers.rsi);
            write_all(b" info=");
            write_u64(registers.rdx);
            write_all(b" flags=");
            write_u64(registers.r10);
        } else if mode == 3 && syscall == ginux_core::SYS_LINKAT {
            write_all(b" old-dirfd=");
            write_i64(registers.rdi as i32 as i64);
            write_all(b" old-path=");
            write_tracee_string(host_pid, registers.rsi as usize);
            write_all(b" new-dirfd=");
            write_i64(registers.rdx as i32 as i64);
            write_all(b" new-path=");
            write_tracee_string(host_pid, registers.r10 as usize);
            write_all(b" flags=");
            write_u64(registers.r8);
        } else if mode == 3 && syscall == 61 {
            write_all(b" selector=");
            write_i64(registers.rdi as i64);
            write_all(b" options=");
            write_u64(registers.r10);
        } else if mode == 3 && syscall == 62 {
            write_all(b" target=");
            write_i64(registers.rdi as i64);
            write_all(b" signal=");
            write_u64(registers.rsi);
        } else if mode == 3 && syscall == 157 {
            write_all(b" option=");
            write_u64(registers.rdi);
            write_all(b" argument2=");
            write_u64(registers.rsi);
            write_all(b" argument3=");
            write_u64(registers.rdx);
        } else if mode == 3 && syscall == 126 {
            write_all(b" header=");
            write_u64(registers.rdi);
            write_all(b" data=");
            write_u64(registers.rsi);
        } else if mode == 3 && matches!(syscall, 128 | 133 | 135 | 259 | 317 | 442) {
            write_all(b" argument1=");
            write_u64(registers.rdi);
            write_all(b" argument2=");
            write_u64(registers.rsi);
            write_all(b" argument3=");
            write_u64(registers.rdx);
            write_all(b" argument4=");
            write_u64(registers.r10);
        } else if mode == 3 && matches!(syscall, 253..=255) {
            write_all(b" fd=");
            write_u64(registers.rdi);
            write_all(b" path-or-flags=");
            write_u64(registers.rsi);
            write_all(b" mask=");
            write_u64(registers.rdx);
        }
        let proc_fdinfo = if mode == 3 && syscall == 257 {
            tracee_decimal_fdinfo_path(host_pid, registers.rsi as usize)
        } else {
            None
        };
        let cgroup_procs_path = mode == 3
            && syscall == 257
            && tracee_cgroup_procs_path(host_pid, registers.rsi as usize);
        let proc_link_source_fd = if mode == 3 && syscall == ginux_core::SYS_LINKAT {
            tracee_proc_self_fd_number(host_pid, registers.rsi as usize)
        } else {
            None
        };
        if let Some(fd) = proc_fdinfo {
            if native_proc_materialize_pidfd_info(
                root_descriptor,
                host_pid,
                task.virtual_id.pid,
                fd,
                &tasks,
            ) {
                write_all(b" fdinfo=virtual-pidfd");
            } else {
                write_all(b" fdinfo=unavailable");
            }
        }
        let mut proc_self_path = [0u8; 128];
        let proc_self_path_len = if mode == 3 && syscall == 257 {
            translate_proc_self_path(
                host_pid,
                registers.rsi as usize,
                task.virtual_id.pid,
                &mut proc_self_path,
            )
        } else {
            None
        };
        let mut proc_fd_target = [0u8; 512];
        let proc_fd_target_len = if mode == 3
            && syscall == 257
            && tracee_decimal_fd_path(host_pid, registers.rsi as usize)
        {
            translate_proc_self_fd_target(host_pid, registers.rsi as usize, &mut proc_fd_target)
        } else {
            None
        };
        let mut inotify_host_path = [0u8; 128];
        let inotify_host_path_len = if mode == 3
            && syscall == 254
            && tracee_decimal_fd_path(host_pid, registers.rsi as usize)
        {
            translate_proc_self_fd_target(host_pid, registers.rsi as usize, &mut inotify_host_path)
        } else {
            None
        };
        write_all(b" action=");
        match decision.action {
            Action::Emulate if native_identity_result(syscall, mode).is_some() => {
                let result = if mode == 3 {
                    let view = processes
                        .view(task.virtual_id)
                        .unwrap_or_else(|_| native_fail(domain, pid, b"native identity failed"));
                    match syscall {
                        SYS_GETPID | ginux_core::SYS_GETTID => view.id.pid as i64,
                        ginux_core::SYS_GETPPID => view.parent as i64,
                        ginux_core::SYS_GETPGRP => view.process_group as i64,
                        ginux_core::SYS_GETUID | ginux_core::SYS_GETEUID => {
                            i64::from(view.credentials.uid)
                        }
                        ginux_core::SYS_GETGID | ginux_core::SYS_GETEGID => {
                            i64::from(view.credentials.gid)
                        }
                        _ => native_identity_result(syscall, mode).unwrap_or(0),
                    }
                } else {
                    native_identity_result(syscall, mode).unwrap_or(0)
                };
                write_all(b"emulate surface=virtual-identity result=");
                write_i64(result);
                write_all(b"\n");
                registers.orig_rax = u64::MAX;
                task.awaiting_exit = true;
                task.override_exit = true;
                task.override_result = result;
                if ptrace(PTRACE_SETREGS, host_pid, 0, &registers as *const _ as usize) < 0
                    || ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0
                {
                    native_fail(domain, pid, b"native getpid emulation failed");
                }
            }
            Action::Emulate if mode == 3 && syscall == ginux_core::SYS_GETPRIORITY => {
                let self_process = registers.rdi == 0
                    && (registers.rsi == 0 || registers.rsi == u64::from(task.virtual_id.pid));
                let result = if self_process { 20 } else { -EPERM };
                write_all(b"emulate surface=virtual-priority result=");
                write_i64(result);
                write_all(b"\n");
                registers.orig_rax = u64::MAX;
                task.awaiting_exit = true;
                task.override_exit = true;
                task.override_result = result;
                if ptrace(PTRACE_SETREGS, host_pid, 0, &registers as *const _ as usize) < 0
                    || ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0
                {
                    native_fail(domain, pid, b"native priority emulation failed");
                }
            }
            Action::Emulate if mode == 3 && syscall == ginux_core::SYS_SETPRIORITY => {
                let no_op_self = registers.rdi == 0
                    && (registers.rsi == 0 || registers.rsi == u64::from(task.virtual_id.pid))
                    && registers.rdx == 0;
                let result = if no_op_self { 0 } else { -EPERM };
                write_all(b"emulate surface=virtual-priority result=");
                write_i64(result);
                write_all(b"\n");
                registers.orig_rax = u64::MAX;
                task.awaiting_exit = true;
                task.override_exit = true;
                task.override_result = result;
                if ptrace(PTRACE_SETREGS, host_pid, 0, &registers as *const _ as usize) < 0
                    || ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0
                {
                    native_fail(domain, pid, b"native priority update emulation failed");
                }
            }
            Action::Mediate if mode == 3 && syscall == 302 => {
                let self_process =
                    registers.rdi == 0 || registers.rdi == u64::from(task.virtual_id.pid);
                let resource = usize::try_from(registers.rsi).unwrap_or(usize::MAX);
                let old_limit = if resource < task.limits.len() {
                    task.limits[resource]
                } else {
                    [0; 2]
                };
                let result = if !self_process {
                    -EPERM
                } else if resource >= task.limits.len() {
                    -22
                } else if registers.rdx != 0 {
                    if let Some(new_limit) = read_tracee_u64_pair(host_pid, registers.rdx as usize)
                    {
                        if new_limit[0] > new_limit[1] {
                            -22
                        } else {
                            task.limits[resource] = new_limit;
                            0
                        }
                    } else {
                        -14
                    }
                } else {
                    0
                };
                let result = if result == 0 && registers.r10 != 0 {
                    let mut encoded = [0u8; 16];
                    encoded[..8].copy_from_slice(&old_limit[0].to_ne_bytes());
                    encoded[8..].copy_from_slice(&old_limit[1].to_ne_bytes());
                    if write_tracee_bytes(host_pid, registers.r10 as usize, &encoded) {
                        0
                    } else {
                        -14
                    }
                } else {
                    result
                };
                write_all(b"emulate surface=virtual-rlimit result=");
                write_i64(result);
                write_all(b"\n");
                registers.orig_rax = u64::MAX;
                task.awaiting_exit = true;
                task.override_exit = true;
                task.override_result = result;
                if ptrace(PTRACE_SETREGS, host_pid, 0, &registers as *const _ as usize) < 0
                    || ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0
                {
                    native_fail(domain, pid, b"native rlimit emulation failed");
                }
            }
            Action::Mediate if mode == 3 && syscall == 126 => {
                let mut header = 0u64;
                let readable = registers.rdi != 0
                    && registers.rsi != 0
                    && ptrace(
                        PTRACE_PEEKDATA,
                        host_pid,
                        registers.rdi as usize,
                        &mut header as *mut _ as usize,
                    ) >= 0;
                let version = header as u32;
                let requested_pid = (header >> 32) as u32 as i32;
                let valid_version = matches!(version, 0x1998_0330 | 0x2007_1026 | 0x2008_0522);
                let self_process = requested_pid == 0
                    || u32::try_from(requested_pid).ok() == Some(task.virtual_id.pid);
                let result = if readable && valid_version && self_process {
                    0
                } else {
                    -EPERM
                };
                write_all(b"emulate surface=virtual-capabilities result=");
                write_i64(result);
                write_all(b"\n");
                registers.orig_rax = u64::MAX;
                task.awaiting_exit = true;
                task.override_exit = true;
                task.override_result = result;
                if ptrace(PTRACE_SETREGS, host_pid, 0, &registers as *const _ as usize) < 0
                    || ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0
                {
                    native_fail(domain, pid, b"native virtual capset failed");
                }
            }
            Action::Mediate if mode == 3 && syscall == 157 && matches!(registers.rdi, 23 | 24) => {
                let capability = usize::try_from(registers.rsi).unwrap_or(usize::MAX);
                let valid = capability < 41;
                let result = if !valid {
                    -22
                } else if registers.rdi == 23 {
                    if task.capability_bounding & (1u64 << capability) != 0 {
                        1
                    } else {
                        0
                    }
                } else {
                    task.capability_bounding &= !(1u64 << capability);
                    0
                };
                write_all(b"emulate surface=virtual-capability-bounding-set result=");
                write_i64(result);
                write_all(b"\n");
                registers.orig_rax = u64::MAX;
                task.awaiting_exit = true;
                task.override_exit = true;
                task.override_result = result;
                if ptrace(PTRACE_SETREGS, host_pid, 0, &registers as *const _ as usize) < 0
                    || ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0
                {
                    native_fail(domain, pid, b"native virtual capability prctl failed");
                }
            }
            Action::Mediate if mode == 3 && syscall == 135 => {
                const PERSONALITY_QUERY: u64 = u32::MAX as u64;
                let result = if registers.rdi == PERSONALITY_QUERY {
                    task.personality as i64
                } else if registers.rdi <= u32::MAX as u64 {
                    let previous = task.personality;
                    task.personality = registers.rdi;
                    previous as i64
                } else {
                    -22
                };
                write_all(b"emulate surface=virtual-personality result=");
                write_i64(result);
                write_all(b"\n");
                registers.orig_rax = u64::MAX;
                task.awaiting_exit = true;
                task.override_exit = true;
                task.override_result = result;
                if ptrace(PTRACE_SETREGS, host_pid, 0, &registers as *const _ as usize) < 0
                    || ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0
                {
                    native_fail(domain, pid, b"native virtual personality failed");
                }
            }
            Action::Mediate if mode == 3 && syscall == 317 => {
                const SECCOMP_SET_MODE_FILTER: u64 = 1;
                const SECCOMP_FILTER_FLAG_ALLOWED: u64 = 1 | 2 | 4 | 16;
                let valid = registers.rdi == SECCOMP_SET_MODE_FILTER
                    && registers.rsi & !SECCOMP_FILTER_FLAG_ALLOWED == 0
                    && registers.rdx != 0;
                let result = if valid {
                    task.seccomp_filtered = true;
                    0
                } else {
                    -22
                };
                write_all(b"emulate surface=virtual-seccomp result=");
                write_i64(result);
                write_all(b"\n");
                registers.orig_rax = u64::MAX;
                task.awaiting_exit = true;
                task.override_exit = true;
                task.override_result = result;
                if ptrace(PTRACE_SETREGS, host_pid, 0, &registers as *const _ as usize) < 0
                    || ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0
                {
                    native_fail(domain, pid, b"native virtual seccomp failed");
                }
            }
            Action::Emulate if mode == 3 && syscall == ginux_core::SYS_REBOOT => {
                const LINUX_REBOOT_MAGIC1: u64 = 0xfee1_dead;
                const LINUX_REBOOT_MAGIC2: u64 = 0x2812_1969;
                const LINUX_REBOOT_CMD_POWER_OFF: u64 = 0x4321_fedc;
                let root_init = task.virtual_id == root_virtual
                    && processes
                        .view(task.virtual_id)
                        .is_ok_and(|view| view.credentials.uid == 0);
                let allowed = root_init
                    && registers.rdi == LINUX_REBOOT_MAGIC1
                    && registers.rsi == LINUX_REBOOT_MAGIC2
                    && registers.rdx == LINUX_REBOOT_CMD_POWER_OFF;
                if allowed {
                    line(b"translate surface=virtual-poweroff result=domain-shutdown");
                    registers.orig_rax = ginux_core::SYS_EXIT_GROUP;
                    registers.rdi = 0;
                    task.awaiting_exit = false;
                    task.override_exit = false;
                } else {
                    line(b"deny result=-1 reason=invalid-virtual-poweroff");
                    registers.orig_rax = u64::MAX;
                    task.awaiting_exit = true;
                    task.override_exit = true;
                    task.override_result = -EPERM;
                }
                if ptrace(PTRACE_SETREGS, host_pid, 0, &registers as *const _ as usize) < 0
                    || ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0
                {
                    native_fail(domain, pid, b"native virtual poweroff failed");
                }
            }
            Action::Emulate if mode == 3 && syscall == 63 => {
                let mut uts = [0u8; 65 * 6];
                write_uts_field(&mut uts, 0, b"Linux");
                write_uts_field(&mut uts, 1, b"ginux");
                write_uts_field(&mut uts, 2, b"6.12.0-ginux");
                write_uts_field(&mut uts, 3, b"#1 Ginux");
                write_uts_field(&mut uts, 4, b"x86_64");
                write_uts_field(&mut uts, 5, b"localdomain");
                let allowed = write_tracee_bytes(host_pid, registers.rdi as usize, &uts);
                write_all(b"emulate surface=virtual-identity result=");
                write_i64(if allowed { 0 } else { -EPERM });
                write_all(b"\n");
                registers.orig_rax = u64::MAX;
                task.awaiting_exit = true;
                task.override_exit = true;
                task.override_result = if allowed { 0 } else { -EPERM };
                if ptrace(PTRACE_SETREGS, host_pid, 0, &registers as *const _ as usize) < 0
                    || ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0
                {
                    native_fail(domain, pid, b"native uname emulation failed");
                }
            }
            Action::Emulate if mode == 3 && matches!(syscall, 165 | 166) => {
                let proc_mount = tracee_string_equals(host_pid, registers.rdi as usize, b"proc")
                    && tracee_string_equals(host_pid, registers.rdx as usize, b"proc");
                let sysfs_mount = tracee_string_equals(host_pid, registers.rdi as usize, b"sysfs")
                    && tracee_string_equals(host_pid, registers.rdx as usize, b"sysfs");
                let dev_mount = tracee_string_equals(host_pid, registers.rdi as usize, b"devtmpfs")
                    && tracee_string_equals(host_pid, registers.rdx as usize, b"devtmpfs");
                let tmpfs_mount = tracee_string_equals(host_pid, registers.rdi as usize, b"tmpfs")
                    && tracee_string_equals(host_pid, registers.rdx as usize, b"tmpfs");
                let cgroup_mount =
                    tracee_string_equals(host_pid, registers.rdi as usize, b"cgroup2")
                        && tracee_string_equals(host_pid, registers.rdx as usize, b"cgroup2");
                let legacy_cgroup_mount =
                    tracee_string_equals(host_pid, registers.rdi as usize, b"cgroup")
                        && tracee_string_equals(host_pid, registers.rdx as usize, b"cgroup");
                let type_ok = proc_mount
                    || sysfs_mount
                    || dev_mount
                    || tmpfs_mount
                    || cgroup_mount
                    || legacy_cgroup_mount;
                let target_ok = tracee_decimal_fd_path(host_pid, registers.rsi as usize)
                    || (proc_mount
                        && tracee_string_equals(host_pid, registers.rsi as usize, b"/proc"));
                let credentials_read_only_remount = registers.rdi == 0
                    && registers.rdx == 0
                    && registers.r10
                        == MS_RDONLY | MS_NOSUID | MS_NODEV | MS_NOEXEC | MS_REMOUNT | MS_BIND
                    && tracee_decimal_fd_path(host_pid, registers.rsi as usize);
                let credentials_move = registers.rdx == 0
                    && registers.r10 == MS_MOVE
                    && tracee_single_component_below(
                        host_pid,
                        registers.rdi as usize,
                        b"/run/systemd/temporary-credentials/",
                    )
                    && tracee_decimal_fd_path(host_pid, registers.rsi as usize);
                let private_root = registers.rdi == 0
                    && tracee_string_equals(host_pid, registers.rsi as usize, b"/")
                    && registers.rdx == 0
                    && registers.r10 == (MS_REC | 0x4_0000);
                let allowed = if syscall == 165 {
                    (target_ok && type_ok)
                        || credentials_read_only_remount
                        || credentials_move
                        || private_root
                } else {
                    tracee_decimal_fd_path(host_pid, registers.rdi as usize)
                };
                write_all(b"emulate surface=virtual-mount result=");
                write_i64(if allowed { 0 } else { -EPERM });
                write_all(b"\n");
                registers.orig_rax = u64::MAX;
                task.awaiting_exit = true;
                task.override_exit = true;
                task.override_result = if allowed { 0 } else { -EPERM };
                if ptrace(PTRACE_SETREGS, host_pid, 0, &registers as *const _ as usize) < 0
                    || ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0
                {
                    native_fail(domain, pid, b"native virtual mount emulation failed");
                }
            }
            Action::Emulate if mode == 3 && syscall == 112 => {
                let result = processes
                    .create_session(task.virtual_id)
                    .map(i64::from)
                    .unwrap_or(-EPERM);
                if result >= 0 {
                    let task_view = processes.view(task.virtual_id).unwrap_or_else(|_| {
                        native_fail(domain, pid, b"native session view failed")
                    });
                    if !native_proc_materialize(root_descriptor, task_view, b"systemd-executor") {
                        native_fail(domain, pid, b"native session proc update failed");
                    }
                }
                write_all(b"emulate surface=virtual-session result=");
                write_i64(result);
                write_all(b"\n");
                registers.orig_rax = u64::MAX;
                task.awaiting_exit = true;
                task.override_exit = true;
                task.override_result = result;
                if ptrace(PTRACE_SETREGS, host_pid, 0, &registers as *const _ as usize) < 0
                    || ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0
                {
                    native_fail(domain, pid, b"native virtual setsid failed");
                }
            }
            Action::Emulate if mode == 3 && syscall == 109 => {
                let changed_task = u32::try_from(registers.rdi)
                    .ok()
                    .zip(u32::try_from(registers.rsi).ok())
                    .and_then(|(target, group)| {
                        processes
                            .set_process_group_for(task.virtual_id, target, group)
                            .ok()
                    });
                let result = if let Some(changed_task) = changed_task {
                    let task_view = processes.view(changed_task).unwrap_or_else(|_| {
                        native_fail(domain, pid, b"native process group view failed")
                    });
                    if !native_proc_materialize(root_descriptor, task_view, b"login") {
                        native_fail(domain, pid, b"native process group proc update failed");
                    }
                    0
                } else {
                    -EPERM
                };
                write_all(b"emulate surface=virtual-process-group result=");
                write_i64(result);
                write_all(b"\n");
                registers.orig_rax = u64::MAX;
                task.awaiting_exit = true;
                task.override_exit = true;
                task.override_result = result;
                if ptrace(PTRACE_SETREGS, host_pid, 0, &registers as *const _ as usize) < 0
                    || ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0
                {
                    native_fail(domain, pid, b"native virtual setpgid failed");
                }
            }
            Action::Emulate
                if mode == 3
                    && matches!(
                        syscall,
                        ginux_core::SYS_SETUID
                            | ginux_core::SYS_SETGID
                            | ginux_core::SYS_SETREUID
                            | ginux_core::SYS_SETREGID
                            | ginux_core::SYS_SETGROUPS
                            | ginux_core::SYS_SETRESUID
                            | ginux_core::SYS_SETRESGID
                    ) =>
            {
                let current = processes
                    .view(task.virtual_id)
                    .unwrap_or_else(|_| native_fail(domain, pid, b"native credential view failed"));
                let result = match syscall {
                    ginux_core::SYS_SETUID => u32::try_from(registers.rdi)
                        .map_err(|_| ProcessError::InvalidCredential)
                        .and_then(|uid| {
                            processes
                                .credentials_mut(task.virtual_id)
                                .and_then(|credentials| credentials.set_uid(uid))
                        }),
                    ginux_core::SYS_SETGID => u32::try_from(registers.rdi)
                        .map_err(|_| ProcessError::InvalidCredential)
                        .and_then(|gid| {
                            processes
                                .credentials_mut(task.virtual_id)
                                .and_then(|credentials| credentials.set_gid(gid))
                        }),
                    ginux_core::SYS_SETREUID | ginux_core::SYS_SETRESUID => {
                        let values = if syscall == ginux_core::SYS_SETREUID {
                            [registers.rdi, registers.rsi, u64::MAX]
                        } else {
                            [registers.rdi, registers.rsi, registers.rdx]
                        };
                        virtual_credential_target(current.credentials.uid, &values)
                            .ok_or(ProcessError::InvalidCredential)
                            .and_then(|uid| {
                                processes
                                    .credentials_mut(task.virtual_id)
                                    .and_then(|credentials| credentials.set_uid(uid))
                            })
                    }
                    ginux_core::SYS_SETREGID | ginux_core::SYS_SETRESGID => {
                        let values = if syscall == ginux_core::SYS_SETREGID {
                            [registers.rdi, registers.rsi, u64::MAX]
                        } else {
                            [registers.rdi, registers.rsi, registers.rdx]
                        };
                        virtual_credential_target(current.credentials.gid, &values)
                            .ok_or(ProcessError::InvalidCredential)
                            .and_then(|gid| {
                                processes
                                    .credentials_mut(task.virtual_id)
                                    .and_then(|credentials| credentials.set_gid(gid))
                            })
                    }
                    ginux_core::SYS_SETGROUPS => {
                        let count = registers.rdi as usize;
                        if count > ginux_process::MAX_GROUPS {
                            Err(ProcessError::GroupLimit)
                        } else {
                            let mut groups = [0u32; ginux_process::MAX_GROUPS];
                            let mut valid = true;
                            for (index, group) in groups[..count].iter_mut().enumerate() {
                                let Some(value) = read_tracee_u32(
                                    host_pid,
                                    registers.rsi as usize + index * core::mem::size_of::<u32>(),
                                ) else {
                                    valid = false;
                                    break;
                                };
                                *group = value;
                            }
                            if valid {
                                processes
                                    .credentials_mut(task.virtual_id)
                                    .and_then(|credentials| {
                                        credentials.set_groups(&groups[..count])
                                    })
                            } else {
                                Err(ProcessError::InvalidCredential)
                            }
                        }
                    }
                    _ => Err(ProcessError::InvalidCredential),
                };
                let result = if result.is_ok() { 0 } else { -EPERM };
                write_all(b"emulate surface=virtual-credentials result=");
                write_i64(result);
                write_all(b"\n");
                registers.orig_rax = u64::MAX;
                task.awaiting_exit = true;
                task.override_exit = true;
                task.override_result = result;
                if ptrace(PTRACE_SETREGS, host_pid, 0, &registers as *const _ as usize) < 0
                    || ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0
                {
                    native_fail(domain, pid, b"native virtual credential update failed");
                }
            }
            Action::Emulate if mode == 3 && matches!(syscall, 92 | 93 | 94 | 260) => {
                let root = processes
                    .view(task.virtual_id)
                    .is_ok_and(|view| view.credentials.uid == 0);
                let allowed = match syscall {
                    92 | 94 => tracee_string_bounded(host_pid, registers.rdi as usize, false),
                    93 => registers.rdi < DOMAIN_NULL_FD as u64,
                    260 => {
                        let directory = registers.rdi as u32 == (-100i32) as u32
                            || registers.rdi < DOMAIN_NULL_FD as u64;
                        let flags = registers.r8;
                        directory
                            && flags & !(0x100 | 0x1000) == 0
                            && tracee_string_bounded(
                                host_pid,
                                registers.rsi as usize,
                                flags & 0x1000 != 0,
                            )
                    }
                    _ => false,
                };
                write_all(b"emulate surface=virtual-file-owner result=");
                write_i64(if root && allowed { 0 } else { -EPERM });
                write_all(b"\n");
                registers.orig_rax = u64::MAX;
                task.awaiting_exit = true;
                task.override_exit = true;
                task.override_result = if root && allowed { 0 } else { -EPERM };
                if ptrace(PTRACE_SETREGS, host_pid, 0, &registers as *const _ as usize) < 0
                    || ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0
                {
                    native_fail(domain, pid, b"native virtual file ownership failed");
                }
            }
            Action::Mediate
                if mode == 3
                    && syscall == 84
                    && tracee_string_starts_with(
                        host_pid,
                        registers.rdi as usize,
                        b"/sys/fs/cgroup/",
                    ) =>
            {
                write_all(b"emulate surface=virtual-cgroup-remove result=0\n");
                registers.orig_rax = u64::MAX;
                task.awaiting_exit = true;
                task.override_exit = true;
                task.override_result = 0;
                if ptrace(PTRACE_SETREGS, host_pid, 0, &registers as *const _ as usize) < 0
                    || ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0
                {
                    native_fail(domain, pid, b"native virtual cgroup removal failed");
                }
            }
            Action::Mediate
                if mode == 3
                    && syscall == 452
                    && registers.rdx & !0o7777 == 0
                    && registers.r10 == 0x1000
                    && tracee_string_equals(host_pid, registers.rsi as usize, b"")
                    && {
                        let mut target = [0u8; 64];
                        native_fd_domain_target(host_pid, registers.rdi as usize, &mut target)
                            .is_some_and(|length| &target[..length] == b"/dev/console")
                    } =>
            {
                write_all(b"emulate surface=virtual-console-mode result=0\n");
                registers.orig_rax = u64::MAX;
                task.awaiting_exit = true;
                task.override_exit = true;
                task.override_result = 0;
                if ptrace(PTRACE_SETREGS, host_pid, 0, &registers as *const _ as usize) < 0
                    || ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0
                {
                    native_fail(domain, pid, b"native virtual console fchmodat2 failed");
                }
            }
            Action::Mediate if mode == 3 && syscall == 91 && registers.rdi <= 2 => {
                let allowed = registers.rsi & !0o7777 == 0;
                write_all(b"emulate surface=virtual-console-mode result=");
                write_i64(if allowed { 0 } else { -EPERM });
                write_all(b"\n");
                registers.orig_rax = u64::MAX;
                task.awaiting_exit = true;
                task.override_exit = true;
                task.override_result = if allowed { 0 } else { -EPERM };
                if ptrace(PTRACE_SETREGS, host_pid, 0, &registers as *const _ as usize) < 0
                    || ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0
                {
                    native_fail(domain, pid, b"native virtual console fchmod failed");
                }
            }
            Action::Emulate if mode == 3 && syscall == 153 => {
                write_all(b"emulate surface=virtual-console-hangup result=0\n");
                registers.orig_rax = u64::MAX;
                task.awaiting_exit = true;
                task.override_exit = true;
                task.override_result = 0;
                if ptrace(PTRACE_SETREGS, host_pid, 0, &registers as *const _ as usize) < 0
                    || ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0
                {
                    native_fail(domain, pid, b"native virtual console vhangup failed");
                }
            }
            Action::Emulate if syscall == SYS_EXIT || syscall == SYS_EXIT_GROUP => {
                line(b"mediate surface=lifecycle result=pending");
                task.awaiting_exit = false;
                task.override_exit = false;
                if ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0 {
                    native_fail(domain, pid, b"native lifecycle resume failed");
                }
            }
            Action::Mediate if mode == 3 && syscall == 62 => {
                const SYSTEMD_SIGRTMIN_POWER_OFF: u64 = 38;
                let privileged = processes
                    .view(task.virtual_id)
                    .is_ok_and(|view| view.credentials.uid == 0);
                let poweroff = privileged
                    && registers.rdi == u64::from(root_virtual.pid)
                    && registers.rsi == SYSTEMD_SIGRTMIN_POWER_OFF;
                if poweroff {
                    line(b"mediate surface=virtual-systemd-poweroff result=pending");
                    task.translated_rdi = registers.rdi;
                    registers.rdi = pid as u64;
                    task.awaiting_exit = true;
                    if ptrace(PTRACE_SETREGS, host_pid, 0, &registers as *const _ as usize) < 0
                        || ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0
                    {
                        native_fail(domain, pid, b"native systemd poweroff signal failed");
                    }
                } else if privileged
                    && (registers.rdi as u32 as i32) < 0
                    && matches!(registers.rsi, 1 | 9 | 15 | 18 | 19)
                {
                    let selector = registers.rdi as u32 as i32;
                    let mut selected = 0usize;
                    if matches!(registers.rsi, 18 | 19) {
                        selected = 1;
                    } else {
                        for candidate in tasks.iter().flatten() {
                            if !candidate.tracing || candidate.host_pid == host_pid {
                                continue;
                            }
                            let matches_selector = selector == -1
                                || processes.view(candidate.virtual_id).is_ok_and(|view| {
                                    view.process_group == selector.unsigned_abs()
                                });
                            if matches_selector {
                                let _ = unsafe {
                                    syscall2(SYS_KILL, candidate.host_pid, registers.rsi as usize)
                                };
                                selected += 1;
                            }
                        }
                    }
                    write_all(b"emulate surface=virtual-group-signal targets=");
                    write_u64(selected as u64);
                    write_all(b" result=");
                    write_i64(if selected == 0 { -3 } else { 0 });
                    write_all(b"\n");
                    registers.orig_rax = u64::MAX;
                    task.awaiting_exit = true;
                    task.override_exit = true;
                    task.override_result = if selected == 0 { -3 } else { 0 };
                    if ptrace(PTRACE_SETREGS, host_pid, 0, &registers as *const _ as usize) < 0
                        || ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0
                    {
                        native_fail(domain, pid, b"native group signal emulation failed");
                    }
                } else if privileged && registers.rdi > 0 && registers.rsi <= 64 {
                    let selected = registers.rdi as u32;
                    let target = tasks.iter().flatten().find(|candidate| {
                        candidate.tracing && candidate.virtual_id.pid == selected
                    });
                    if let Some(target) = target {
                        line(b"translate surface=virtual-task-signal result=pending");
                        task.translated_rdi = registers.rdi;
                        registers.rdi = target.host_pid as u64;
                        task.awaiting_exit = true;
                        if ptrace(PTRACE_SETREGS, host_pid, 0, &registers as *const _ as usize) < 0
                            || ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0
                        {
                            native_fail(domain, pid, b"native task signal translation failed");
                        }
                    } else {
                        line(b"deny result=-3 reason=unknown-virtual-signal-target");
                        registers.orig_rax = u64::MAX;
                        task.awaiting_exit = true;
                        task.override_exit = true;
                        task.override_result = -3;
                        if ptrace(PTRACE_SETREGS, host_pid, 0, &registers as *const _ as usize) < 0
                            || ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0
                        {
                            native_fail(domain, pid, b"native unknown signal target denial failed");
                        }
                    }
                } else {
                    line(b"deny result=-1 reason=invalid-virtual-signal");
                    registers.orig_rax = u64::MAX;
                    task.awaiting_exit = true;
                    task.override_exit = true;
                    task.override_result = -EPERM;
                    if ptrace(PTRACE_SETREGS, host_pid, 0, &registers as *const _ as usize) < 0
                        || ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0
                    {
                        native_fail(domain, pid, b"native virtual signal denial failed");
                    }
                }
            }
            Action::Mediate if mode == 3 && syscall == ginux_core::SYS_PIDFD_SEND_SIGNAL => {
                let target_host_pid = native_pidfd_host_target(host_pid, registers.rdi as usize);
                let managed_target = target_host_pid.is_some_and(|selected| {
                    tasks
                        .iter()
                        .flatten()
                        .any(|candidate| candidate.host_pid == selected)
                });
                let allowed = managed_target
                    && registers.rsi <= 64
                    && registers.rdx == 0
                    && registers.r10 == 0;
                if allowed && registers.rsi == 18 {
                    line(b"emulate surface=virtual-pidfd-signal result=0");
                    registers.orig_rax = u64::MAX;
                    task.awaiting_exit = true;
                    task.override_exit = true;
                    task.override_result = 0;
                    if ptrace(PTRACE_SETREGS, host_pid, 0, &registers as *const _ as usize) < 0
                        || ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0
                    {
                        native_fail(domain, pid, b"native pidfd continue emulation failed");
                    }
                } else if allowed {
                    line(b"mediate surface=virtual-pidfd-signal result=pending");
                    task.awaiting_exit = true;
                    if ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0 {
                        native_fail(domain, pid, b"native pidfd signal resume failed");
                    }
                } else {
                    line(b"deny result=-1 reason=invalid-virtual-pidfd-signal");
                    registers.orig_rax = u64::MAX;
                    task.awaiting_exit = true;
                    task.override_exit = true;
                    task.override_result = -EPERM;
                    if ptrace(PTRACE_SETREGS, host_pid, 0, &registers as *const _ as usize) < 0
                        || ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0
                    {
                        native_fail(domain, pid, b"native pidfd signal denial failed");
                    }
                }
            }
            Action::Mediate
                if mode == 3
                    && syscall == 59
                    && tracee_decimal_fd_path(host_pid, registers.rdi as usize) =>
            {
                let executor = b"/usr/lib/systemd/systemd-executor\0";
                let path_address = (registers.rsp as usize).saturating_sub(256);
                if !write_tracee_bytes(host_pid, path_address, executor) {
                    native_fail(domain, pid, b"native executor path translation failed");
                }
                write_all(b"translate surface=immutable-systemd-executor result=pending\n");
                registers.rdi = path_address as u64;
                task.awaiting_exit = true;
                if ptrace(PTRACE_SETREGS, host_pid, 0, &registers as *const _ as usize) < 0
                    || ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0
                {
                    native_fail(domain, pid, b"native executor resume failed");
                }
            }
            Action::Mediate if mode == 3 && syscall == 5 && registers.rdi <= 2 => {
                let stat = virtual_console_stat();
                let allowed = write_tracee_bytes(host_pid, registers.rsi as usize, &stat);
                write_all(b"emulate surface=virtual-console-stat result=");
                write_i64(if allowed { 0 } else { -EPERM });
                write_all(b"\n");
                registers.orig_rax = u64::MAX;
                task.awaiting_exit = true;
                task.override_exit = true;
                task.override_result = if allowed { 0 } else { -EPERM };
                if ptrace(PTRACE_SETREGS, host_pid, 0, &registers as *const _ as usize) < 0
                    || ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0
                {
                    native_fail(domain, pid, b"native virtual console fstat failed");
                }
            }
            Action::Mediate
                if mode == 3
                    && syscall == 262
                    && tracee_string_equals(host_pid, registers.rsi as usize, b"/dev/console") =>
            {
                let stat = virtual_console_stat();
                let allowed = write_tracee_bytes(host_pid, registers.rdx as usize, &stat);
                write_all(b"emulate surface=virtual-console-stat result=");
                write_i64(if allowed { 0 } else { -EPERM });
                write_all(b"\n");
                registers.orig_rax = u64::MAX;
                task.awaiting_exit = true;
                task.override_exit = true;
                task.override_result = if allowed { 0 } else { -EPERM };
                if ptrace(PTRACE_SETREGS, host_pid, 0, &registers as *const _ as usize) < 0
                    || ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0
                {
                    native_fail(domain, pid, b"native virtual console stat failed");
                }
            }
            Action::Mediate
                if mode == 3
                    && syscall == 89
                    && (tracee_string_equals(
                        host_pid,
                        registers.rdi as usize,
                        b"/proc/self/fd/0",
                    ) || tracee_string_equals(
                        host_pid,
                        registers.rdi as usize,
                        b"/proc/self/fd/1",
                    ) || tracee_string_equals(
                        host_pid,
                        registers.rdi as usize,
                        b"/proc/self/fd/2",
                    )) =>
            {
                let target = b"/dev/console";
                let count = core::cmp::min(target.len(), registers.rdx as usize);
                let allowed =
                    write_tracee_bytes(host_pid, registers.rsi as usize, &target[..count]);
                write_all(b"emulate surface=virtual-console-link result=");
                write_i64(if allowed { count as i64 } else { -EPERM });
                write_all(b"\n");
                registers.orig_rax = u64::MAX;
                task.awaiting_exit = true;
                task.override_exit = true;
                task.override_result = if allowed { count as i64 } else { -EPERM };
                if ptrace(PTRACE_SETREGS, host_pid, 0, &registers as *const _ as usize) < 0
                    || ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0
                {
                    native_fail(domain, pid, b"native virtual console readlink failed");
                }
            }
            Action::Mediate
                if mode == 3
                    && syscall == ginux_core::SYS_READLINKAT
                    && tracee_decimal_fd_path(host_pid, registers.rsi as usize) =>
            {
                let target = b"/memfd:ginux-domain";
                let count = core::cmp::min(target.len(), registers.r10 as usize);
                let allowed =
                    write_tracee_bytes(host_pid, registers.rdx as usize, &target[..count]);
                write_all(b"emulate surface=virtual-proc result=");
                write_i64(if allowed { count as i64 } else { -EPERM });
                write_all(b"\n");
                registers.orig_rax = u64::MAX;
                task.awaiting_exit = true;
                task.override_exit = true;
                task.override_result = if allowed { count as i64 } else { -EPERM };
                if ptrace(PTRACE_SETREGS, host_pid, 0, &registers as *const _ as usize) < 0
                    || ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0
                {
                    native_fail(domain, pid, b"native virtual proc readlink failed");
                }
            }
            Action::Mediate
                if mode == 3
                    && syscall == 16
                    && registers.rdi as i64 == task.console_fd
                    && registers.rsi == 21_505 =>
            {
                line(b"mediate surface=virtual-console-termios result=pending");
                task.awaiting_exit = true;
                if ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0 {
                    native_fail(domain, pid, b"native virtual console termios failed");
                }
            }
            Action::Mediate
                if mode == 3
                    && syscall == 16
                    && registers.rdi as i64 == task.console_fd
                    && registers.rsi == 21_518 =>
            {
                write_all(b"emulate surface=virtual-console-control result=0\n");
                registers.orig_rax = u64::MAX;
                task.awaiting_exit = true;
                task.override_exit = true;
                task.override_result = 0;
                if ptrace(PTRACE_SETREGS, host_pid, 0, &registers as *const _ as usize) < 0
                    || ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0
                {
                    native_fail(domain, pid, b"native virtual console control failed");
                }
            }
            Action::Mediate
                if mode == 3
                    && syscall == 16
                    && (registers.rdi <= 2 || registers.rdi as i64 == task.console_fd)
                    && matches!(registers.rsi, 21_519 | 21_520) =>
            {
                let task_view = processes.view(task.virtual_id).unwrap_or_else(|_| {
                    native_fail(domain, pid, b"native job control view failed")
                });
                let allowed = if registers.rsi == 21_519 {
                    write_tracee_bytes(
                        host_pid,
                        registers.rdx as usize,
                        &task_view.process_group.to_ne_bytes(),
                    )
                } else {
                    read_tracee_u32(host_pid, registers.rdx as usize).is_some_and(|group| {
                        task_view.session != 0 && group == task_view.process_group
                    })
                };
                write_all(b"emulate surface=virtual-console-job-control result=");
                write_i64(if allowed { 0 } else { -EPERM });
                write_all(b"\n");
                registers.orig_rax = u64::MAX;
                task.awaiting_exit = true;
                task.override_exit = true;
                task.override_result = if allowed { 0 } else { -EPERM };
                if ptrace(PTRACE_SETREGS, host_pid, 0, &registers as *const _ as usize) < 0
                    || ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0
                {
                    native_fail(domain, pid, b"native virtual console job control failed");
                }
            }
            Action::Mediate
                if mode == 3 && syscall == 32 && registers.rdi <= DOMAIN_CONSOLE_FD as u64 =>
            {
                line(b"mediate surface=virtual-console-dup result=pending");
                task.awaiting_exit = true;
                if ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0 {
                    native_fail(domain, pid, b"native virtual console dup failed");
                }
            }
            Action::Mediate if mode == 3 && syscall == 32 => {
                line(b"deny result=-1 reason=invalid-console-dup");
                registers.orig_rax = u64::MAX;
                task.awaiting_exit = true;
                task.override_exit = true;
                task.override_result = -EPERM;
                if ptrace(PTRACE_SETREGS, host_pid, 0, &registers as *const _ as usize) < 0
                    || ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0
                {
                    native_fail(domain, pid, b"native invalid console dup denial failed");
                }
            }
            Action::Mediate
                if mode == 3
                    && syscall == 292
                    && registers.rdi <= DOMAIN_CONSOLE_FD as u64
                    && registers.rsi <= 2
                    && registers.rdx & !0x8_0000 == 0 =>
            {
                line(b"mediate surface=virtual-console-dup result=pending");
                task.awaiting_exit = true;
                if ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0 {
                    native_fail(domain, pid, b"native virtual console dup failed");
                }
            }
            Action::Mediate if mode == 3 && syscall == 292 => {
                line(b"deny result=-1 reason=invalid-console-dup");
                registers.orig_rax = u64::MAX;
                task.awaiting_exit = true;
                task.override_exit = true;
                task.override_result = -EPERM;
                if ptrace(PTRACE_SETREGS, host_pid, 0, &registers as *const _ as usize) < 0
                    || ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0
                {
                    native_fail(domain, pid, b"native invalid console dup denial failed");
                }
            }
            Action::Mediate
                if mode == 3
                    && syscall == 33
                    && registers.rdi <= DOMAIN_CONSOLE_FD as u64
                    && registers.rsi <= 2 =>
            {
                line(b"mediate surface=virtual-console-dup result=pending");
                task.awaiting_exit = true;
                if ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0 {
                    native_fail(domain, pid, b"native virtual console dup2 failed");
                }
            }
            Action::Mediate if mode == 3 && syscall == 33 => {
                line(b"deny result=-1 reason=invalid-console-dup");
                registers.orig_rax = u64::MAX;
                task.awaiting_exit = true;
                task.override_exit = true;
                task.override_result = -EPERM;
                if ptrace(PTRACE_SETREGS, host_pid, 0, &registers as *const _ as usize) < 0
                    || ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0
                {
                    native_fail(domain, pid, b"native invalid console dup2 denial failed");
                }
            }
            Action::Mediate
                if mode == 3
                    && syscall == SYS_WRITE
                    && native_fd_contains(&task.cgroup_procs_fds, registers.rdi as usize)
                    && registers.rsi != 0
                    && registers.rdx > 0
                    && registers.rdx <= 64 * 1024 =>
            {
                line(b"emulate surface=virtual-cgroup-membership result=pending");
                registers.orig_rax = u64::MAX;
                task.awaiting_exit = true;
                task.override_exit = true;
                task.override_result = registers.rdx as i64;
                if ptrace(PTRACE_SETREGS, host_pid, 0, &registers as *const _ as usize) < 0
                    || ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0
                {
                    native_fail(domain, pid, b"native cgroup membership emulation failed");
                }
            }
            Action::Mediate
                if mode == 3
                    && matches!(syscall, 1 | 20)
                    && registers.rdi as i64 == task.console_fd
                    && ((syscall == 1 && registers.rdx <= 4096)
                        || (syscall == 20 && registers.rdx <= 16)) =>
            {
                line(b"mediate surface=virtual-console result=pending");
                task.awaiting_exit = true;
                if ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0 {
                    native_fail(domain, pid, b"native virtual console write failed");
                }
            }
            Action::Mediate
                if mode == 3
                    && syscall == 137
                    && (tracee_string_equals(host_pid, registers.rdi as usize, b"/proc")
                        || tracee_string_equals(host_pid, registers.rdi as usize, b"/proc/")
                        || tracee_string_equals(
                            host_pid,
                            registers.rdi as usize,
                            b"/sys/fs/cgroup",
                        )
                        || tracee_string_equals(
                            host_pid,
                            registers.rdi as usize,
                            b"/sys/fs/cgroup/",
                        )) =>
            {
                let mut statfs = [0u8; 120];
                let procfs = tracee_string_equals(host_pid, registers.rdi as usize, b"/proc")
                    || tracee_string_equals(host_pid, registers.rdi as usize, b"/proc/");
                statfs[..8].copy_from_slice(
                    &(if procfs { 0x9fa0u64 } else { 0x6367_7270u64 }).to_ne_bytes(),
                );
                statfs[8..16].copy_from_slice(&4096u64.to_ne_bytes());
                statfs[64..72].copy_from_slice(&255u64.to_ne_bytes());
                statfs[72..80].copy_from_slice(&4096u64.to_ne_bytes());
                let allowed = write_tracee_bytes(host_pid, registers.rsi as usize, &statfs);
                write_all(if procfs {
                    b"emulate surface=virtual-procfs result="
                } else {
                    b"emulate surface=virtual-cgroup result="
                });
                write_i64(if allowed { 0 } else { -EPERM });
                write_all(b"\n");
                registers.orig_rax = u64::MAX;
                task.awaiting_exit = true;
                task.override_exit = true;
                task.override_result = if allowed { 0 } else { -EPERM };
                if ptrace(PTRACE_SETREGS, host_pid, 0, &registers as *const _ as usize) < 0
                    || ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0
                {
                    native_fail(domain, pid, b"native virtual cgroup statfs failed");
                }
            }
            Action::Mediate if mode == 3 && syscall == 138 => {
                let mut target = [0u8; 512];
                let target_len =
                    native_fd_domain_target(host_pid, registers.rdi as usize, &mut target);
                let fs_type = target_len.and_then(|length| {
                    let path = &target[..length];
                    if path == b"/proc" || path.starts_with(b"/proc/") {
                        Some(0x9fa0u64)
                    } else if path == b"/sys/fs/cgroup" || path.starts_with(b"/sys/fs/cgroup/") {
                        Some(0x6367_7270u64)
                    } else {
                        None
                    }
                });
                let Some(fs_type) = fs_type else {
                    line(b"deny result=-1 reason=non-virtual-fstatfs");
                    registers.orig_rax = u64::MAX;
                    task.awaiting_exit = true;
                    task.override_exit = true;
                    task.override_result = -EPERM;
                    if ptrace(PTRACE_SETREGS, host_pid, 0, &registers as *const _ as usize) < 0
                        || ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0
                    {
                        native_fail(domain, pid, b"native invalid fstatfs denial failed");
                    }
                    tasks[task_index] = Some(task);
                    continue;
                };
                let mut statfs = [0u8; 120];
                statfs[..8].copy_from_slice(&fs_type.to_ne_bytes());
                statfs[8..16].copy_from_slice(&4096u64.to_ne_bytes());
                statfs[64..72].copy_from_slice(&255u64.to_ne_bytes());
                statfs[72..80].copy_from_slice(&4096u64.to_ne_bytes());
                let allowed = write_tracee_bytes(host_pid, registers.rsi as usize, &statfs);
                write_all(if fs_type == 0x9fa0 {
                    b"emulate surface=virtual-procfs result="
                } else {
                    b"emulate surface=virtual-cgroup result="
                });
                write_i64(if allowed { 0 } else { -EPERM });
                write_all(b"\n");
                registers.orig_rax = u64::MAX;
                task.awaiting_exit = true;
                task.override_exit = true;
                task.override_result = if allowed { 0 } else { -EPERM };
                if ptrace(PTRACE_SETREGS, host_pid, 0, &registers as *const _ as usize) < 0
                    || ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0
                {
                    native_fail(domain, pid, b"native virtual fstatfs failed");
                }
            }
            Action::Mediate
                if mode == 3
                    && syscall == 3
                    && native_fd_contains(&task.cgroup_procs_fds, registers.rdi as usize) =>
            {
                native_fd_clear(&mut task.cgroup_procs_fds, registers.rdi as usize);
                line(b"mediate surface=virtual-cgroup result=pending");
                task.awaiting_exit = true;
                if ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0 {
                    native_fail(domain, pid, b"native cgroup descriptor close failed");
                }
            }
            Action::Mediate
                if mode == 3 && syscall == 3 && registers.rdi as usize == DOMAIN_NULL_FD =>
            {
                line(b"mediate surface=virtual-device result=protected");
                registers.orig_rax = u64::MAX;
                task.awaiting_exit = true;
                task.override_exit = true;
                task.override_result = 0;
                if ptrace(PTRACE_SETREGS, host_pid, 0, &registers as *const _ as usize) < 0
                    || ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0
                {
                    native_fail(domain, pid, b"native protected device close failed");
                }
            }
            Action::Mediate
                if mode == 3 && syscall == 3 && registers.rdi as usize == DOMAIN_CONSOLE_FD =>
            {
                line(b"mediate surface=virtual-console result=protected");
                registers.orig_rax = u64::MAX;
                task.awaiting_exit = true;
                task.override_exit = true;
                task.override_result = 0;
                if ptrace(PTRACE_SETREGS, host_pid, 0, &registers as *const _ as usize) < 0
                    || ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0
                {
                    native_fail(domain, pid, b"native protected console close failed");
                }
            }
            Action::Mediate
                if mode == 3 && syscall == 3 && registers.rdi as i64 == task.mountinfo_fd =>
            {
                task.mountinfo_fd = -1;
                line(b"mediate surface=virtual-mount-watch result=pending");
                task.awaiting_exit = true;
                if ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0 {
                    native_fail(domain, pid, b"native mount watch close failed");
                }
            }
            Action::Mediate
                if mode == 3 && syscall == 3 && task.pipe_fds.contains(&(registers.rdi as i64)) =>
            {
                if let Some(slot) = task
                    .pipe_fds
                    .iter_mut()
                    .find(|fd| **fd == registers.rdi as i64)
                {
                    *slot = -1;
                }
                line(b"mediate surface=task-pipe result=pending");
                task.awaiting_exit = true;
                if ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0 {
                    native_fail(domain, pid, b"native task pipe close failed");
                }
            }
            Action::Mediate
                if mode == 3
                    && syscall == 257
                    && (tracee_string_equals(host_pid, registers.rsi as usize, b"/dev/null")
                        || tracee_string_equals(
                            host_pid,
                            registers.rsi as usize,
                            b"/dev/console",
                        )) =>
            {
                let null_device =
                    tracee_string_equals(host_pid, registers.rsi as usize, b"/dev/null");
                let source = if null_device {
                    DOMAIN_NULL_FD
                } else {
                    DOMAIN_CONSOLE_FD
                };
                task.pending_console_open = !null_device;
                line(b"mediate surface=virtual-device result=pending");
                registers.orig_rax = SYS_DUP_HOST as u64;
                registers.rdi = source as u64;
                task.awaiting_exit = true;
                if ptrace(PTRACE_SETREGS, host_pid, 0, &registers as *const _ as usize) < 0
                    || ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0
                {
                    native_fail(domain, pid, b"native virtual device open failed");
                }
            }
            Action::Mediate if mode == 3 && syscall == 257 && proc_fd_target_len.is_some() => {
                let path_address = (registers.rsp as usize).saturating_sub(512);
                let path_len = proc_fd_target_len.unwrap_or(0);
                if !write_tracee_bytes(host_pid, path_address, &proc_fd_target[..=path_len]) {
                    native_fail(domain, pid, b"native proc fd target translation failed");
                }
                write_all(b"translate surface=virtual-proc-fd-target result=pending\n");
                registers.rsi = path_address as u64;
                task.awaiting_exit = true;
                if ptrace(PTRACE_SETREGS, host_pid, 0, &registers as *const _ as usize) < 0
                    || ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0
                {
                    native_fail(domain, pid, b"native proc fd target open failed");
                }
            }
            Action::Mediate if mode == 3 && syscall == 257 && proc_self_path_len.is_some() => {
                task.pending_mountinfo_open =
                    tracee_string_equals(host_pid, registers.rsi as usize, b"/proc/self/mountinfo");
                if tracee_string_equals(host_pid, registers.rsi as usize, b"/proc/self/fd")
                    && !native_proc_refresh_fds(
                        root_descriptor,
                        host_pid,
                        task.virtual_id.pid,
                        &mut task.proc_fds,
                    )
                {
                    native_fail(domain, pid, b"native proc fd refresh failed");
                }
                let path_address = (registers.rsp as usize).saturating_sub(512);
                let path_len = proc_self_path_len.unwrap_or(0);
                if !write_tracee_bytes(host_pid, path_address, &proc_self_path[..=path_len]) {
                    native_fail(domain, pid, b"native proc self path translation failed");
                }
                write_all(b"translate surface=virtual-proc-self result=pending\n");
                registers.rsi = path_address as u64;
                task.awaiting_exit = true;
                if ptrace(PTRACE_SETREGS, host_pid, 0, &registers as *const _ as usize) < 0
                    || ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0
                {
                    native_fail(domain, pid, b"native proc self open failed");
                }
            }
            Action::Mediate if mode == 3 && syscall == 293 => {
                line(b"mediate surface=task-pipe result=pending");
                task.pipe2_address = registers.rdi as usize;
                task.awaiting_exit = true;
                if ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0 {
                    native_fail(domain, pid, b"native pipe2 resume failed");
                }
            }
            Action::Mediate if mode == 3 && syscall == 254 && inotify_host_path_len.is_some() => {
                let path_address = (registers.rsp as usize).saturating_sub(512);
                let path_len = inotify_host_path_len.unwrap_or(0);
                if !write_tracee_bytes(host_pid, path_address, &inotify_host_path[..=path_len]) {
                    native_fail(domain, pid, b"native inotify path translation failed");
                }
                line(b"translate surface=virtual-inotify-path result=pending");
                registers.rsi = path_address as u64;
                task.awaiting_exit = true;
                if ptrace(PTRACE_SETREGS, host_pid, 0, &registers as *const _ as usize) < 0
                    || ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0
                {
                    native_fail(domain, pid, b"native inotify resume failed");
                }
            }
            Action::Mediate
                if mode == 3
                    && syscall == 233
                    && task.mountinfo_fd >= 0
                    && registers.rdx as i64 == task.mountinfo_fd
                    && registers.rsi == 1 =>
            {
                line(b"emulate surface=virtual-mount-watch result=0");
                registers.orig_rax = u64::MAX;
                task.awaiting_exit = true;
                task.override_exit = true;
                task.override_result = 0;
                if ptrace(PTRACE_SETREGS, host_pid, 0, &registers as *const _ as usize) < 0
                    || ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0
                {
                    native_fail(domain, pid, b"native mount watch emulation failed");
                }
            }
            Action::Mediate
                if mode == 3
                    && syscall == ginux_core::SYS_CLOSE_RANGE
                    && registers.rdi >= 3
                    && registers.rdi < DOMAIN_NULL_FD as u64
                    && registers.rsi >= registers.rdi
                    && registers.rdx == 0 =>
            {
                let last = core::cmp::min(registers.rsi, DOMAIN_NULL_FD as u64 - 1);
                write_all(b"mediate surface=task-close-range first=");
                write_u64(registers.rdi);
                write_all(b" last=");
                write_u64(last);
                write_all(b" result=pending\n");
                registers.rsi = last;
                task.close_range_first = registers.rdi as i64;
                task.close_range_last = last as i64;
                task.awaiting_exit = true;
                if ptrace(PTRACE_SETREGS, host_pid, 0, &registers as *const _ as usize) < 0
                    || ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0
                {
                    native_fail(domain, pid, b"native close range resume failed");
                }
            }
            Action::Mediate
                if mode == 3
                    && syscall == ginux_core::SYS_LINKAT
                    && registers.rdi as u32 == (-100i32) as u32
                    && registers.rdx as u32 == (-100i32) as u32
                    && registers.r8 == 0x400
                    && proc_link_source_fd.is_some() =>
            {
                let result = native_link_tracee_fd_into_domain(
                    host_pid,
                    proc_link_source_fd.unwrap_or(usize::MAX),
                    registers.r10 as usize,
                );
                write_all(b"emulate surface=virtual-proc-fd-link result=");
                write_i64(result);
                write_all(b"\n");
                registers.orig_rax = u64::MAX;
                task.awaiting_exit = true;
                task.override_exit = true;
                task.override_result = result;
                if ptrace(PTRACE_SETREGS, host_pid, 0, &registers as *const _ as usize) < 0
                    || ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0
                {
                    native_fail(domain, pid, b"native proc fd link emulation failed");
                }
            }
            Action::Mediate if native_arguments_allowed(syscall, &registers, mode) => {
                if cgroup_procs_path {
                    task.pending_cgroup_procs_open = true;
                }
                if mode == 3 && syscall == 47 {
                    task.pending_recvmsg = registers.rsi as usize;
                }
                if mode == 3 && syscall == 55 && registers.rsi == 1 && registers.rdx == 17 {
                    task.pending_peercred = registers.r10 as usize;
                }
                if mode == 3 && syscall == 5 {
                    task.pending_stat = registers.rsi as usize;
                }
                if mode == 3 && syscall == ginux_core::SYS_PIDFD_OPEN {
                    let selected = registers.rdi as u32;
                    let target_host_pid = if selected == task.virtual_id.pid {
                        Some(task.host_pid)
                    } else {
                        tasks
                            .iter()
                            .flatten()
                            .find(|candidate| candidate.virtual_id.pid == selected)
                            .map(|candidate| candidate.host_pid)
                    };
                    let Some(target_host_pid) = target_host_pid else {
                        task.awaiting_exit = true;
                        task.override_exit = true;
                        task.override_result = -3;
                        registers.orig_rax = u64::MAX;
                        tasks[task_index] = Some(task);
                        if ptrace(PTRACE_SETREGS, host_pid, 0, &registers as *const _ as usize) < 0
                            || ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0
                        {
                            native_fail(domain, pid, b"native invalid pidfd denial failed");
                        }
                        continue;
                    };
                    registers.rdi = target_host_pid as u64;
                    if ptrace(PTRACE_SETREGS, host_pid, 0, &registers as *const _ as usize) < 0 {
                        native_fail(domain, pid, b"native pidfd selector translation failed");
                    }
                }
                if mode == 3
                    && syscall == ginux_core::SYS_LINKAT
                    && tracee_decimal_fd_path(host_pid, registers.rsi as usize)
                {
                    let mut target = [0u8; 512];
                    let Some(target_len) = translate_proc_self_fd_target(
                        host_pid,
                        registers.rsi as usize,
                        &mut target,
                    ) else {
                        native_fail(domain, pid, b"native linkat fd target resolution failed");
                    };
                    let path_address = (registers.rsp as usize).saturating_sub(512);
                    if !write_tracee_bytes(host_pid, path_address, &target[..=target_len]) {
                        native_fail(domain, pid, b"native linkat fd target translation failed");
                    }
                    registers.rdi = (-100i32) as u32 as u64;
                    registers.rsi = path_address as u64;
                    if ptrace(PTRACE_SETREGS, host_pid, 0, &registers as *const _ as usize) < 0 {
                        native_fail(domain, pid, b"native linkat register translation failed");
                    }
                }
                if mode == 3
                    && syscall == 257
                    && tracee_string_equals(
                        host_pid,
                        registers.rsi as usize,
                        b"/proc/self/mountinfo",
                    )
                {
                    task.pending_mountinfo_open = true;
                }
                if mode == 3 && syscall == 247 {
                    task.waitid_info = registers.rdx;
                    if registers.rdi == 1 && registers.rsi != 0 {
                        let selected = registers.rsi as u32;
                        let Some(target) = tasks.iter().flatten().find(|candidate| {
                            candidate.virtual_id.pid == selected
                                || candidate.host_pid == selected as usize
                        }) else {
                            task.awaiting_exit = true;
                            task.override_exit = true;
                            task.override_result = -EPERM;
                            registers.orig_rax = u64::MAX;
                            tasks[task_index] = Some(task);
                            if ptrace(PTRACE_SETREGS, host_pid, 0, &registers as *const _ as usize)
                                < 0
                                || ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0
                            {
                                native_fail(domain, pid, b"native invalid waitid denial failed");
                            }
                            continue;
                        };
                        registers.rsi = target.host_pid as u64;
                        if ptrace(PTRACE_SETREGS, host_pid, 0, &registers as *const _ as usize) < 0
                        {
                            native_fail(domain, pid, b"native waitid selector translation failed");
                        }
                    } else if registers.rdi == 3 {
                        task.awaiting_exit = true;
                        task.override_exit = true;
                        task.override_result = -ENOSYS;
                        registers.orig_rax = u64::MAX;
                        tasks[task_index] = Some(task);
                        if ptrace(PTRACE_SETREGS, host_pid, 0, &registers as *const _ as usize) < 0
                            || ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0
                        {
                            native_fail(domain, pid, b"native pidfd wait denial failed");
                        }
                        continue;
                    } else if registers.rdi != 0 {
                        task.awaiting_exit = true;
                        task.override_exit = true;
                        task.override_result = -EPERM;
                        registers.orig_rax = u64::MAX;
                        tasks[task_index] = Some(task);
                        if ptrace(PTRACE_SETREGS, host_pid, 0, &registers as *const _ as usize) < 0
                            || ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0
                        {
                            native_fail(domain, pid, b"native unsupported waitid denial failed");
                        }
                        continue;
                    }
                }
                if mode == 3 && syscall == 61 {
                    let selector = registers.rdi as u32 as i32;
                    if selector == -1 {
                        registers.rdi = u64::MAX;
                    } else if selector > 0 {
                        let selected = selector as u32;
                        let target = tasks.iter().flatten().find(|candidate| {
                            candidate.virtual_id.pid == selected
                                && processes
                                    .view(candidate.virtual_id)
                                    .is_ok_and(|view| view.parent == task.virtual_id.pid)
                        });
                        let Some(target) = target else {
                            write_all(b"mediate surface=task-wait result=-");
                            write_u64(ECHILD as u64);
                            write_all(b"\n");
                            task.awaiting_exit = true;
                            task.override_exit = true;
                            task.override_result = -ECHILD;
                            registers.orig_rax = u64::MAX;
                            tasks[task_index] = Some(task);
                            if ptrace(PTRACE_SETREGS, host_pid, 0, &registers as *const _ as usize)
                                < 0
                                || ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0
                            {
                                native_fail(domain, pid, b"native invalid wait denial failed");
                            }
                            continue;
                        };
                        task.translated_rdi = registers.rdi;
                        registers.rdi = target.host_pid as u64;
                        if ptrace(PTRACE_SETREGS, host_pid, 0, &registers as *const _ as usize) < 0
                        {
                            native_fail(domain, pid, b"native wait selector translation failed");
                        }
                    } else {
                        write_all(b"deny result=-");
                        write_u64(ENOSYS as u64);
                        write_all(b" reason=unsupported-wait-process-group\n");
                        task.awaiting_exit = true;
                        task.override_exit = true;
                        task.override_result = -ENOSYS;
                        registers.orig_rax = u64::MAX;
                        tasks[task_index] = Some(task);
                        if ptrace(PTRACE_SETREGS, host_pid, 0, &registers as *const _ as usize) < 0
                            || ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0
                        {
                            native_fail(domain, pid, b"native process-group wait denial failed");
                        }
                        continue;
                    }
                }
                write_all(b"mediate surface=");
                line(balanced_surface_name(if mode == 3 {
                    systemd_surface(syscall, shared_network)
                } else {
                    balanced_surface(syscall)
                }));
                task.awaiting_exit = true;
                if ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0 {
                    native_fail(domain, pid, b"native mediated syscall failed");
                }
            }
            Action::Mediate => {
                line(b"deny result=-1 reason=invalid-arguments");
                registers.orig_rax = u64::MAX;
                task.awaiting_exit = true;
                task.override_exit = true;
                task.override_result = -EPERM;
                if ptrace(PTRACE_SETREGS, host_pid, 0, &registers as *const _ as usize) < 0
                    || ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0
                {
                    native_fail(domain, pid, b"native argument denial failed");
                }
            }
            Action::Deny(errno) => {
                write_all(b"deny result=-");
                write_u64(errno as u64);
                write_all(b"\n");
                registers.orig_rax = u64::MAX;
                task.awaiting_exit = true;
                task.override_exit = true;
                task.override_result = -(errno as i64);
                if ptrace(PTRACE_SETREGS, host_pid, 0, &registers as *const _ as usize) < 0
                    || ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0
                {
                    native_fail(domain, pid, b"native syscall denial failed");
                }
            }
            _ => native_fail(domain, pid, b"invalid native contract decision"),
        }
        tasks[task_index] = Some(task);
    }
}

fn native_identity_result(syscall: u64, mode: usize) -> Option<i64> {
    if mode == 3 {
        match syscall {
            SYS_GETPID | ginux_core::SYS_GETTID => Some(1),
            ginux_core::SYS_GETPPID => Some(0),
            ginux_core::SYS_GETPGRP => Some(1),
            _ => balanced_identity_result(syscall),
        }
    } else {
        balanced_identity_result(syscall)
    }
}

fn virtual_credential_target(current: u32, values: &[u64]) -> Option<u32> {
    let mut target = None;
    for value in values {
        if *value == u64::MAX || *value == u64::from(u32::MAX) {
            continue;
        }
        let value = u32::try_from(*value).ok()?;
        if target.is_some_and(|selected| selected != value) {
            return None;
        }
        target = Some(value);
    }
    Some(target.unwrap_or(current))
}

fn native_arguments_allowed(syscall: u64, registers: &UserRegs, mode: usize) -> bool {
    if mode == 3 {
        match syscall {
            ginux_core::SYS_PIDFD_OPEN => registers.rdi > 0 && registers.rsi == 0,
            293 => registers.rdi != 0 && registers.rsi & !(0x8_0000 | 0x800) == 0,
            ginux_core::SYS_CLOSE_RANGE => true,
            452 => {
                registers.rdi < DOMAIN_NULL_FD as u64
                    && registers.rsi != 0
                    && registers.rdx & !0o7777 == 0
                    && registers.r10 & !0x1000 == 0
            }
            128 => registers.rdi != 0 && registers.r10 == 8,
            133 => {
                const S_IFMT: u64 = 0o170000;
                const S_IFIFO: u64 = 0o010000;
                registers.rdi != 0
                    && registers.rsi & S_IFMT == S_IFIFO
                    && registers.rsi & !(S_IFMT | 0o7777) == 0
                    && registers.rdx == 0
            }
            259 => {
                const S_IFMT: u64 = 0o170000;
                const S_IFIFO: u64 = 0o010000;
                (registers.rdi as u32 == (-100i32) as u32 || registers.rdi < DOMAIN_NULL_FD as u64)
                    && registers.rsi != 0
                    && registers.rdx & S_IFMT == S_IFIFO
                    && registers.rdx & !(S_IFMT | 0o7777) == 0
                    && registers.r10 == 0
            }
            0 => registers.rdi < DOMAIN_NULL_FD as u64 && registers.rdx <= 64 * 1024 * 1024,
            18 => registers.rdi < DOMAIN_NULL_FD as u64 && registers.rdx <= 1024 * 1024,
            25 => {
                registers.rsi <= 1024 * 1024 * 1024
                    && registers.rdx <= 1024 * 1024 * 1024
                    && registers.r10 & !1 == 0
            }
            73 => registers.rdi < DOMAIN_NULL_FD as u64 && registers.rsi & !15 == 0,
            74 | 75 => registers.rdi < DOMAIN_NULL_FD as u64,
            77 => registers.rdi < DOMAIN_NULL_FD as u64,
            79 => registers.rdi != 0 && registers.rsi <= 4096,
            81 | 306 => registers.rdi < DOMAIN_NULL_FD as u64,
            280 => {
                (registers.rdi as u32 == (-100i32) as u32 || registers.rdi < DOMAIN_NULL_FD as u64)
                    && registers.r10 & !(0x100 | 0x1000) == 0
            }
            285 => {
                registers.rdi < DOMAIN_NULL_FD as u64
                    && registers.rsi == 0
                    && registers.r10 <= 1024 * 1024 * 1024
            }
            ginux_core::SYS_LINKAT => {
                let old_directory = registers.rdi as u32 == (-100i32) as u32
                    || registers.rdi < DOMAIN_NULL_FD as u64;
                let new_directory = registers.rdx as u32 == (-100i32) as u32
                    || registers.rdx < DOMAIN_NULL_FD as u64;
                old_directory && new_directory && registers.r8 & !(0x1000 | 0x400) == 0
            }
            190 => {
                registers.rdi < DOMAIN_NULL_FD as u64
                    && registers.r10 <= 64 * 1024
                    && registers.r8 <= 2
            }
            196 => registers.rdi < DOMAIN_NULL_FD as u64 && registers.rdx <= 64 * 1024,
            199 => registers.rdi < DOMAIN_NULL_FD as u64,
            1
            | 2
            | 7
            | 15
            | 16
            | 20
            | 32
            | 56
            | 57
            | 58
            | 59
            | 61
            | 62
            | 76
            | 80
            | 82
            | 83
            | 84
            | 87
            | 88
            | 90
            | 91
            | 95
            | 112
            | 125
            | 126
            | 135
            | 137
            | 138
            | 157
            | 193
            | 213
            | 232
            | 233
            | 247
            | 253
            | 254
            | 255
            | 257
            | 258
            | ginux_core::SYS_UNLINKAT
            | ginux_core::SYS_RENAMEAT
            | ginux_core::SYS_SYMLINKAT
            | ginux_core::SYS_READLINKAT
            | 270
            | 271
            | 281
            | 283
            | 286
            | 287
            | 289
            | 290
            | 291
            | 294
            | 317
            | ginux_core::SYS_MEMFD_CREATE
            | 435
            | 441 => true,
            _ if ginux_core::is_shared_network_syscall(syscall) => true,
            _ => balanced_arguments_allowed(syscall, registers.rdi, registers.rsi, registers.rdx),
        }
    } else {
        balanced_arguments_allowed(syscall, registers.rdi, registers.rsi, registers.rdx)
    }
}

fn balanced_surface_name(surface: BalancedSurface) -> &'static [u8] {
    match surface {
        BalancedSurface::ImageRead => b"immutable-image result=pending",
        BalancedSurface::DomainWrite => b"disposable-domain-write result=pending",
        BalancedSurface::Memory => b"memory result=pending",
        BalancedSurface::Task => b"task result=pending",
        BalancedSurface::Console => b"bootstrap-console result=pending",
        BalancedSurface::Random => b"random result=pending",
        BalancedSurface::Network => b"shared-network result=pending",
        BalancedSurface::VirtualIdentity | BalancedSurface::Lifecycle | BalancedSurface::Denied => {
            b"invalid result=denied"
        }
    }
}

fn native_fail(domain: &mut Domain, pid: usize, reason: &[u8]) -> ! {
    let _ = ptrace(PTRACE_KILL, pid, 0, 0);
    let mut status = 0;
    let _ = wait(pid, &mut status);
    domain_fail(domain, reason)
}

fn config_main(config_path: *const u8) -> ! {
    let config = load_config(config_path);
    print_effective_config(&config);
    line(b"GINUX_CONFIG: PASS");
    exit_process(0)
}

fn domain_main(config_path: *const u8, fixture_path: *const u8) -> ! {
    let config = load_config(config_path);
    print_effective_config(&config);
    let mut domain = Domain::create(config);
    lifecycle_state(&domain);
    if domain.start().is_err() {
        domain_fail(&mut domain, b"invalid start transition");
    }
    lifecycle_state(&domain);

    let length = read_file(fixture_path);
    let elf =
        unsafe { core::slice::from_raw_parts(core::ptr::addr_of!(ELF_BYTES).cast::<u8>(), length) };
    let mut memory = AddressSpace::new();
    let loaded = match memory.load_elf(elf) {
        Ok(loaded) => loaded,
        Err(_) => domain_fail(&mut domain, b"managed ELF rejected"),
    };

    let mut argv: [&[u8]; 8] = [&[]; 8];
    argv[0] = config.init.as_bytes();
    let mut argc = 1;
    for argument in config.args.as_slice() {
        argv[argc] = argument.as_bytes();
        argc += 1;
    }
    let mut environment: [&[u8]; 8] = [&[]; 8];
    let mut envc = 0;
    for variable in config.environment.as_slice() {
        environment[envc] = variable.as_bytes();
        envc += 1;
    }
    let stack = match memory.build_stack(&argv[..argc], &environment[..envc]) {
        Ok(stack) => stack,
        Err(_) => domain_fail(&mut domain, b"managed stack rejected"),
    };
    let report = match execute(&memory, loaded.entry, stack, 10_000) {
        Ok(report) => report,
        Err(_) => domain_fail(&mut domain, b"managed execution failed"),
    };
    if report.exit_code != 0 {
        domain_fail(&mut domain, b"managed task failed");
    }
    write_all(b"GINUX_DOMAIN: managed-output=");
    write_all(&report.output[..report.output_len]);
    write_all(b"\n");
    if domain.task_exited(true).is_err() {
        domain_fail(&mut domain, b"invalid task-exit transition");
    }
    lifecycle_state(&domain);
    line(b"GINUX_DOMAIN: cleanup resources=false managed-task=false");
    line(b"GATE4: PASS");
    exit_process(0)
}

fn lifecycle_main(config_path: *const u8) -> ! {
    let config = load_config(config_path);
    let mut domain = Domain::create(config);
    lifecycle_state(&domain);
    if domain.start().is_err() {
        domain_fail(&mut domain, b"invalid start transition");
    }

    let mask = 1u64 << (SIGTERM - 1);
    if unsafe {
        syscall4(
            SYS_RT_SIGPROCMASK,
            SIG_BLOCK,
            &mask as *const _ as usize,
            0,
            core::mem::size_of::<u64>(),
        )
    } < 0
    {
        domain_fail(&mut domain, b"signal mask setup failed");
    }
    let signal_fd = unsafe {
        syscall4(
            SYS_SIGNALFD4,
            usize::MAX,
            &mask as *const _ as usize,
            core::mem::size_of::<u64>(),
            0,
        )
    };
    if signal_fd < 0 {
        domain_fail(&mut domain, b"signalfd setup failed");
    }
    let child = unsafe { syscall0(SYS_FORK) };
    if child < 0 {
        let _ = unsafe { syscall1(SYS_CLOSE_HOST, signal_fd as usize) };
        domain_fail(&mut domain, b"helper fork failed");
    }
    if child == 0 {
        loop {
            let _ = unsafe { syscall0(SYS_PAUSE) };
        }
    }

    write_all(b"GINUX_LIFECYCLE: state=running helper-pid=");
    write_u64(child as u64);
    write_all(b" resources=true\n");
    let mut signal_info = [0u8; 128];
    let signal_read = unsafe {
        syscall3(
            SYS_READ_HOST,
            signal_fd as usize,
            signal_info.as_mut_ptr() as usize,
            signal_info.len(),
        )
    };
    if signal_read != signal_info.len() as isize {
        let _ = unsafe { syscall2(SYS_KILL, child as usize, SIGKILL) };
        let mut status = 0;
        let _ = wait(child as usize, &mut status);
        let _ = unsafe { syscall1(SYS_CLOSE_HOST, signal_fd as usize) };
        domain_fail(&mut domain, b"signal wait failed");
    }
    line(b"GINUX_LIFECYCLE: signal=SIGTERM decision=stop");
    let _ = unsafe { syscall2(SYS_KILL, child as usize, SIGKILL) };
    let mut status = 0;
    if wait(child as usize, &mut status) != child {
        let _ = unsafe { syscall1(SYS_CLOSE_HOST, signal_fd as usize) };
        domain_fail(&mut domain, b"helper reap failed");
    }
    let _ = unsafe { syscall1(SYS_CLOSE_HOST, signal_fd as usize) };
    if domain.stop().is_err() {
        domain_fail(&mut domain, b"invalid stop transition");
    }
    line(b"GINUX_LIFECYCLE: helper=reaped zombies=0");
    lifecycle_state(&domain);
    line(b"GINUX_DOMAIN: cleanup resources=false managed-task=false");
    line(b"GATE4_LIFECYCLE: PASS");
    exit_process(0)
}

fn load_config(path: *const u8) -> DomainConfig {
    let length = read_config(path);
    let bytes = unsafe {
        core::slice::from_raw_parts(core::ptr::addr_of!(CONFIG_BYTES).cast::<u8>(), length)
    };
    match DomainConfig::parse(bytes) {
        Ok(config) => config,
        Err(error) => config_rejected(error),
    }
}

fn print_effective_config(config: &DomainConfig) {
    let normalized = match config.normalized() {
        Ok(normalized) => normalized,
        Err(error) => config_rejected(error),
    };
    line(b"GINUX_CONFIG: accepted format=toml schema=1");
    line(b"GINUX_CONFIG_EFFECTIVE_BEGIN");
    write_all(normalized.as_bytes());
    line(b"GINUX_CONFIG_EFFECTIVE_END");
}

fn lifecycle_state(domain: &Domain) {
    write_all(b"GINUX_LIFECYCLE: state=");
    write_all(domain.state().as_str().as_bytes());
    write_all(b" resources=");
    write_all(if domain.resources_acquired() {
        b"true"
    } else {
        b"false"
    });
    write_all(b" managed-task=");
    line(if domain.managed_task() {
        b"true"
    } else {
        b"false"
    });
}

fn config_rejected(error: ConfigError) -> ! {
    write_all(b"GINUX_CONFIG: rejected reason=");
    write_all(error.as_str().as_bytes());
    line(b" managed-started=false");
    exit_process(2)
}

fn domain_fail(domain: &mut Domain, reason: &[u8]) -> ! {
    let _ = domain.task_exited(false);
    write_all(b"GINUX_DOMAIN: failed reason=");
    line(reason);
    lifecycle_state(domain);
    line(b"GINUX_DOMAIN: cleanup resources=false managed-task=false");
    exit_process(1)
}

fn gate1_main() -> ! {
    line(b"GINUX: interception platform=ptrace-sysemu");

    let child = unsafe { syscall0(SYS_FORK) };
    if child < 0 {
        fail(b"fork failed");
    }
    if child == 0 {
        tracee();
    }

    supervise(child as usize)
}

fn runtime_main(path: *const u8) -> ! {
    line(b"GINUX_RUNTIME: backend=x86_64-interpreter");
    let length = read_file(path);
    let elf =
        unsafe { core::slice::from_raw_parts(core::ptr::addr_of!(ELF_BYTES).cast::<u8>(), length) };
    let mut memory = AddressSpace::new();
    let loaded = match memory.load_elf(elf) {
        Ok(loaded) => loaded,
        Err(error) => runtime_load_fail(error),
    };
    let stack = match memory.build_stack(&[b"runtime-fixture", b"fixture-arg"], &[b"GINUX_GATE=3"])
    {
        Ok(stack) => stack,
        Err(_) => runtime_fail(b"initial stack rejected"),
    };
    line(b"GINUX_RUNTIME: elf=loaded class=ELF64 machine=x86_64 type=static");
    line(b"GINUX_RUNTIME: memory=ginux-owned host-pointers=unreachable protections=enforced");
    line(b"GINUX_RUNTIME: stack=linux-x86_64 argc=2 envc=1");

    let report = match execute(&memory, loaded.entry, stack, 10_000) {
        Ok(report) => report,
        Err(_) => runtime_fail(b"interpreter execution failed"),
    };
    if report.exit_code != 0 || &report.output[..report.output_len] != b"fixture-argGINUX_GATE=3" {
        runtime_fail(b"managed fixture reported failure");
    }
    write_all(b"GINUX_RUNTIME: managed-output=");
    write_all(&report.output[..report.output_len]);
    write_all(b"\n");
    for record in report.audit.iter().take(report.audit_len) {
        write_all(b"GINUX_AUDIT: sequence=");
        write_u64(record.sequence as u64);
        write_all(b" syscall=");
        write_u64(record.syscall);
        write_all(b" action=");
        write_all(match record.action {
            AuditAction::Emulate => b"emulate",
            AuditAction::Mediate => b"mediate",
            AuditAction::Deny => b"deny",
        });
        write_all(b" result=");
        write_i64(record.result);
        write_all(b"\n");
    }
    if report.audit_len != 5
        || report.audit[2].syscall != 999
        || report.audit[2].action != AuditAction::Deny
        || report.audit[2].result != -38
    {
        runtime_fail(b"expected deny audit missing");
    }
    line(b"GINUX_RUNTIME: direct-host-syscalls=impossible interpreted-syscall=internal-event");
    line(b"GATE3: PASS");
    exit_process(0)
}

fn read_file(path: *const u8) -> usize {
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

fn read_config(path: *const u8) -> usize {
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

fn read_index(path: *const u8) -> usize {
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

fn runtime_load_fail(error: LoadError) -> ! {
    write_all(b"GATE3: FAIL: elf rejected reason=");
    line(match error {
        LoadError::Truncated => b"truncated",
        LoadError::InvalidMagic => b"invalid-magic",
        LoadError::UnsupportedClass => b"unsupported-class",
        LoadError::UnsupportedEndian => b"unsupported-endian",
        LoadError::UnsupportedType => b"unsupported-type",
        LoadError::UnsupportedMachine => b"unsupported-machine",
        LoadError::InvalidProgramHeaders => b"invalid-program-headers",
        LoadError::UnsupportedInterpreter => b"dynamic-interpreter",
        LoadError::InvalidSegment => b"invalid-segment",
        LoadError::SegmentOverlap => b"segment-overlap",
        LoadError::WriteExecuteSegment => b"write-execute-segment",
        LoadError::InvalidEntry => b"invalid-entry",
    });
    exit_process(1)
}

fn runtime_fail(reason: &[u8]) -> ! {
    write_all(b"GATE3: FAIL: ");
    line(reason);
    exit_process(1)
}

fn c_string_equals(value: *const u8, expected: &[u8]) -> bool {
    for (index, byte) in expected.iter().enumerate() {
        if unsafe { *value.add(index) } != *byte {
            return false;
        }
    }
    unsafe { *value.add(expected.len()) == 0 }
}

fn tracee() -> ! {
    let traced = unsafe { syscall4(SYS_PTRACE, PTRACE_TRACEME, 0, 0, 0) };
    if traced < 0 {
        unsafe { syscall1(SYS_EXIT_HOST, 110) };
        loop {
            core::hint::spin_loop();
        }
    }

    static PATH: &[u8] = b"/var/tmp/ginux-managed-payload\0";
    let argv = [PATH.as_ptr(), core::ptr::null()];
    let envp: [*const u8; 1] = [core::ptr::null()];
    unsafe {
        syscall3(
            SYS_EXECVE,
            PATH.as_ptr() as usize,
            argv.as_ptr() as usize,
            envp.as_ptr() as usize,
        );
        syscall1(SYS_EXIT_HOST, 111);
    }
    loop {
        core::hint::spin_loop();
    }
}

fn supervise(pid: usize) -> ! {
    let mut status = 0i32;
    if wait(pid, &mut status) < 0 || !stopped(status) {
        fail(b"payload did not stop after exec");
    }

    let options = PTRACE_O_TRACESYSGOOD | PTRACE_O_EXITKILL;
    if ptrace(PTRACE_SETOPTIONS, pid, 0, options) < 0 {
        fail(b"PTRACE_SETOPTIONS failed");
    }
    if ptrace(PTRACE_SYSEMU, pid, 0, 0) < 0 {
        fail(b"PTRACE_SYSEMU unavailable");
    }

    let mut saw_getpid = false;
    let mut saw_write = false;
    let mut saw_uname = false;

    loop {
        status = 0;
        if wait(pid, &mut status) < 0 {
            fail(b"wait4 failed");
        }
        if !stopped(status) {
            fail(b"payload exited outside Ginux dispatch");
        }

        let signal = ((status >> 8) & 0xff) as u8;
        if signal != (5 | 0x80) && signal != 5 {
            fail(b"unexpected trace stop");
        }

        let mut regs = core::mem::MaybeUninit::<UserRegs>::uninit();
        if ptrace(PTRACE_GETREGS, pid, 0, regs.as_mut_ptr() as usize) < 0 {
            fail(b"PTRACE_GETREGS failed");
        }
        // PTRACE_GETREGS initializes the complete architecture register set.
        let mut regs = unsafe { regs.assume_init() };

        let decision = decide(regs.orig_rax);
        match decision.action {
            Action::Emulate if decision.syscall == SYS_GETPID => {
                saw_getpid = true;
                regs.rax = VIRTUAL_PID;
                line(b"GINUX: syscall=getpid decision=emulate result=4242");
            }
            Action::Mediate if decision.syscall == SYS_WRITE => {
                saw_write = true;
                if regs.rdi != 1 || regs.rdx == 0 || regs.rdx > 4096 {
                    regs.rax = (-EPERM) as u64;
                    line(b"GINUX: syscall=write decision=deny reason=invalid-arguments");
                } else if !read_tracee_word(pid, regs.rsi as usize) {
                    regs.rax = (-EPERM) as u64;
                    line(b"GINUX: syscall=write decision=deny reason=invalid-memory");
                } else {
                    regs.rax = regs.rdx;
                    line(b"GINUX: syscall=write decision=mediate host-write=not-executed");
                }
            }
            Action::Deny(errno) if decision.syscall == SYS_UNAME => {
                saw_uname = true;
                regs.rax = (-(errno as i64)) as u64;
                line(b"GINUX: syscall=uname decision=deny errno=EPERM");
            }
            Action::Emulate if decision.syscall == SYS_EXIT => {
                let success = regs.rdi == 0 && saw_getpid && saw_write && saw_uname;
                let _ = ptrace(PTRACE_KILL, pid, 0, 0);
                let _ = wait(pid, &mut status);
                if success {
                    line(b"GINUX: direct-host-syscall bypass=not-observed");
                    line(b"GATE1: PASS");
                    exit_process(0);
                }
                fail(b"payload reported failure");
            }
            Action::Deny(errno) => {
                regs.rax = (-(errno as i64)) as u64;
                line(b"GINUX: syscall=unknown decision=deny errno=ENOSYS");
            }
            _ => {
                regs.rax = (-ENOSYS) as u64;
                line(b"GINUX: syscall=unexpected decision=deny errno=ENOSYS");
            }
        }

        if ptrace(PTRACE_SETREGS, pid, 0, &regs as *const _ as usize) < 0 {
            fail(b"PTRACE_SETREGS failed");
        }
        if ptrace(PTRACE_SYSEMU, pid, 0, 0) < 0 {
            fail(b"PTRACE_SYSEMU resume failed");
        }
    }
}

fn wait(pid: usize, status: &mut i32) -> isize {
    unsafe { syscall4(SYS_WAIT4, pid, status as *mut _ as usize, 0, 0) }
}

fn stopped(status: i32) -> bool {
    status & 0xff == 0x7f
}

fn ptrace(request: usize, pid: usize, addr: usize, data: usize) -> isize {
    unsafe { syscall4(SYS_PTRACE, request, pid, addr, data) }
}

fn read_tracee_word(pid: usize, address: usize) -> bool {
    let mut word = 0u64;
    ptrace(PTRACE_PEEKDATA, pid, address, &mut word as *mut _ as usize) >= 0
}

fn read_tracee_u32(pid: usize, address: usize) -> Option<u32> {
    if address == 0 {
        return None;
    }
    let mut word = 0u64;
    if ptrace(PTRACE_PEEKDATA, pid, address, &mut word as *mut _ as usize) < 0 {
        return None;
    }
    Some(u32::from_ne_bytes(word.to_ne_bytes()[..4].try_into().ok()?))
}

fn virtual_console_stat() -> [u8; 144] {
    let mut stat = [0u8; 144];
    stat[0..8].copy_from_slice(&1u64.to_ne_bytes());
    stat[8..16].copy_from_slice(&1u64.to_ne_bytes());
    stat[16..24].copy_from_slice(&1u64.to_ne_bytes());
    stat[24..28].copy_from_slice(&0o20_600u32.to_ne_bytes());
    stat[40..48].copy_from_slice(&0x500u64.to_ne_bytes());
    stat[56..64].copy_from_slice(&4096i64.to_ne_bytes());
    stat
}

fn tracee_string_equals(pid: usize, address: usize, expected: &[u8]) -> bool {
    if address == 0 {
        return false;
    }
    let mut offset = 0usize;
    while offset <= expected.len() {
        let mut word = 0u64;
        if ptrace(
            PTRACE_PEEKDATA,
            pid,
            address.saturating_add(offset),
            &mut word as *mut _ as usize,
        ) < 0
        {
            return false;
        }
        let bytes = word.to_ne_bytes();
        for byte in bytes {
            if offset == expected.len() {
                return byte == 0;
            }
            if byte != expected[offset] {
                return false;
            }
            offset += 1;
        }
    }
    false
}

fn tracee_string_bounded(pid: usize, address: usize, allow_empty: bool) -> bool {
    if address == 0 {
        return false;
    }
    let mut offset = 0usize;
    while offset < 4096 {
        let mut word = 0u64;
        if ptrace(
            PTRACE_PEEKDATA,
            pid,
            address.saturating_add(offset),
            &mut word as *mut _ as usize,
        ) < 0
        {
            return false;
        }
        for byte in word.to_ne_bytes() {
            if byte == 0 {
                return allow_empty || offset != 0;
            }
            offset += 1;
            if offset >= 4096 {
                return false;
            }
        }
    }
    false
}

fn tracee_string_starts_with(pid: usize, address: usize, prefix: &[u8]) -> bool {
    if address == 0 || prefix.is_empty() {
        return false;
    }
    let mut offset = 0usize;
    while offset < prefix.len() {
        let mut word = 0u64;
        if ptrace(
            PTRACE_PEEKDATA,
            pid,
            address.saturating_add(offset),
            &mut word as *mut _ as usize,
        ) < 0
        {
            return false;
        }
        for byte in word.to_ne_bytes() {
            if offset == prefix.len() {
                return true;
            }
            if byte != prefix[offset] {
                return false;
            }
            offset += 1;
        }
    }
    true
}

fn tracee_cgroup_procs_path(pid: usize, address: usize) -> bool {
    const PREFIX: &[u8] = b"/sys/fs/cgroup/";
    const SUFFIX: &[u8] = b"/cgroup.procs";
    if address == 0 {
        return false;
    }
    let mut path = [0u8; 512];
    let mut length = 0usize;
    while length < path.len() {
        let mut word = 0u64;
        if ptrace(
            PTRACE_PEEKDATA,
            pid,
            address.saturating_add(length),
            &mut word as *mut _ as usize,
        ) < 0
        {
            return false;
        }
        for byte in word.to_ne_bytes() {
            if byte == 0 {
                return length > PREFIX.len() + SUFFIX.len()
                    && path[..PREFIX.len()] == *PREFIX
                    && path[..length].ends_with(SUFFIX);
            }
            if length >= path.len() {
                return false;
            }
            path[length] = byte;
            length += 1;
        }
    }
    false
}

fn tracee_single_component_below(pid: usize, address: usize, prefix: &[u8]) -> bool {
    if address == 0 {
        return false;
    }
    let mut offset = 0usize;
    let mut component_length = 0usize;
    while offset < 256 {
        let mut word = 0u64;
        if ptrace(
            PTRACE_PEEKDATA,
            pid,
            address.saturating_add(offset),
            &mut word as *mut _ as usize,
        ) < 0
        {
            return false;
        }
        for byte in word.to_ne_bytes() {
            if offset < prefix.len() {
                if byte != prefix[offset] {
                    return false;
                }
            } else if byte == 0 {
                return component_length > 0;
            } else if byte == b'/' || !byte.is_ascii_graphic() {
                return false;
            } else {
                component_length += 1;
            }
            offset += 1;
            if offset >= 256 {
                return false;
            }
        }
    }
    false
}

fn translate_proc_self_path(
    host_pid: usize,
    address: usize,
    virtual_pid: u32,
    output: &mut [u8],
) -> Option<usize> {
    const PREFIX: &[u8] = b"/proc/self/";
    let mut input = [0u8; 96];
    let mut length = 0usize;
    while length < input.len() {
        let mut word = 0u64;
        if ptrace(
            PTRACE_PEEKDATA,
            host_pid,
            address.saturating_add(length),
            &mut word as *mut _ as usize,
        ) < 0
        {
            return None;
        }
        for byte in word.to_ne_bytes() {
            if byte == 0 {
                if length <= PREFIX.len() || input[..PREFIX.len()] != *PREFIX {
                    return None;
                }
                let mut output_len = 0usize;
                native_buffer_append(output, &mut output_len, b"/proc/")?;
                native_buffer_append_u32(output, &mut output_len, virtual_pid)?;
                native_buffer_append(output, &mut output_len, b"/")?;
                native_buffer_append(output, &mut output_len, &input[PREFIX.len()..length])?;
                if output_len >= output.len() {
                    return None;
                }
                output[output_len] = 0;
                return Some(output_len);
            }
            input[length] = byte;
            length += 1;
            if length >= input.len() {
                return None;
            }
        }
    }
    None
}

fn translate_proc_self_fd_target(
    host_pid: usize,
    address: usize,
    output: &mut [u8],
) -> Option<usize> {
    const PREFIX: &[u8] = b"/proc/self/fd/";
    let mut input = [0u8; 64];
    let mut input_len = 0usize;
    loop {
        let mut word = 0u64;
        if ptrace(
            PTRACE_PEEKDATA,
            host_pid,
            address.saturating_add(input_len),
            &mut word as *mut _ as usize,
        ) < 0
        {
            return None;
        }
        for byte in word.to_ne_bytes() {
            if byte == 0 {
                if input_len <= PREFIX.len() || input[..PREFIX.len()] != *PREFIX {
                    return None;
                }
                let digits = &input[PREFIX.len()..input_len];
                if digits.is_empty() || !digits.iter().all(u8::is_ascii_digit) {
                    return None;
                }

                let mut fd = 0usize;
                for digit in digits {
                    fd = fd.checked_mul(10)?.checked_add((digit - b'0') as usize)?;
                }
                return native_fd_domain_target(host_pid, fd, output);
            }
            if input_len >= input.len() {
                return None;
            }
            input[input_len] = byte;
            input_len += 1;
        }
    }
}

fn read_tracee_c_string(pid: usize, address: usize, output: &mut [u8]) -> Option<usize> {
    if address == 0 || output.is_empty() {
        return None;
    }
    let mut length = 0usize;
    while length < output.len() {
        let mut word = 0u64;
        if ptrace(
            PTRACE_PEEKDATA,
            pid,
            address.saturating_add(length),
            &mut word as *mut _ as usize,
        ) < 0
        {
            return None;
        }
        for byte in word.to_ne_bytes() {
            if byte == 0 {
                if length >= output.len() {
                    return None;
                }
                output[length] = 0;
                return Some(length);
            }
            if length + 1 >= output.len() {
                return None;
            }
            output[length] = byte;
            length += 1;
        }
    }
    None
}

fn native_link_tracee_fd_into_domain(
    host_pid: usize,
    source_fd: usize,
    destination_address: usize,
) -> i64 {
    let mut source = [0u8; 96];
    let mut source_len = 0usize;
    if native_buffer_append(&mut source, &mut source_len, b"/proc/").is_none()
        || native_buffer_append_u32(
            &mut source,
            &mut source_len,
            u32::try_from(host_pid).unwrap_or(u32::MAX),
        )
        .is_none()
        || native_buffer_append(&mut source, &mut source_len, b"/fd/").is_none()
        || native_buffer_append_u32(
            &mut source,
            &mut source_len,
            u32::try_from(source_fd).unwrap_or(u32::MAX),
        )
        .is_none()
        || source_len >= source.len()
    {
        return -22;
    }
    source[source_len] = 0;

    let mut domain_path = [0u8; 384];
    let Some(domain_path_len) =
        read_tracee_c_string(host_pid, destination_address, &mut domain_path)
    else {
        return -14;
    };
    if domain_path_len == 0
        || domain_path[0] != b'/'
        || !domain_path[..domain_path_len].starts_with(b"/run/systemd/propagate/")
    {
        return -1;
    }

    let mut destination = [0u8; 512];
    let mut destination_len = 0usize;
    if native_buffer_append(&mut destination, &mut destination_len, b"/proc/").is_none()
        || native_buffer_append_u32(
            &mut destination,
            &mut destination_len,
            u32::try_from(host_pid).unwrap_or(u32::MAX),
        )
        .is_none()
        || native_buffer_append(&mut destination, &mut destination_len, b"/root").is_none()
        || native_buffer_append(
            &mut destination,
            &mut destination_len,
            &domain_path[..domain_path_len],
        )
        .is_none()
        || destination_len >= destination.len()
    {
        return -22;
    }
    destination[destination_len] = 0;

    unsafe {
        syscall5(
            ginux_core::SYS_LINKAT as usize,
            AT_FDCWD,
            source.as_ptr() as usize,
            AT_FDCWD,
            destination.as_ptr() as usize,
            0x400,
        ) as i64
    }
}

fn native_fd_domain_target(host_pid: usize, fd: usize, output: &mut [u8]) -> Option<usize> {
    let mut root_link = [0u8; 64];
    let mut root_link_len = 0usize;
    native_buffer_append(&mut root_link, &mut root_link_len, b"/proc/")?;
    native_buffer_append_u32(
        &mut root_link,
        &mut root_link_len,
        u32::try_from(host_pid).ok()?,
    )?;
    native_buffer_append(&mut root_link, &mut root_link_len, b"/root")?;
    root_link[root_link_len] = 0;

    let mut fd_link = [0u8; 80];
    let mut fd_link_len = 0usize;
    native_buffer_append(&mut fd_link, &mut fd_link_len, b"/proc/")?;
    native_buffer_append_u32(
        &mut fd_link,
        &mut fd_link_len,
        u32::try_from(host_pid).ok()?,
    )?;
    native_buffer_append(&mut fd_link, &mut fd_link_len, b"/fd/")?;
    native_buffer_append_u32(&mut fd_link, &mut fd_link_len, u32::try_from(fd).ok()?)?;
    fd_link[fd_link_len] = 0;

    let mut root_target = [0u8; 256];
    let root_count = unsafe {
        syscall4(
            ginux_core::SYS_READLINKAT as usize,
            AT_FDCWD,
            root_link.as_ptr() as usize,
            root_target.as_mut_ptr() as usize,
            root_target.len(),
        )
    };
    let mut fd_target = [0u8; 512];
    let fd_count = unsafe {
        syscall4(
            ginux_core::SYS_READLINKAT as usize,
            AT_FDCWD,
            fd_link.as_ptr() as usize,
            fd_target.as_mut_ptr() as usize,
            fd_target.len(),
        )
    };
    if root_count <= 0 || fd_count <= root_count {
        return None;
    }
    let root_count = root_count as usize;
    let fd_count = fd_count as usize;
    if fd_target[..root_count] != root_target[..root_count] || fd_target[root_count] != b'/' {
        return None;
    }
    let suffix = &fd_target[root_count..fd_count];
    if suffix.len() >= output.len() {
        return None;
    }
    output[..suffix.len()].copy_from_slice(suffix);
    output[suffix.len()] = 0;
    Some(suffix.len())
}

fn write_uts_field(output: &mut [u8; 65 * 6], field: usize, value: &[u8]) {
    let start = field * 65;
    let count = core::cmp::min(value.len(), 64);
    output[start..start + count].copy_from_slice(&value[..count]);
}

fn read_tracee_bytes(pid: usize, address: usize, output: &mut [u8]) -> bool {
    if address == 0 {
        return false;
    }
    let mut offset = 0usize;
    while offset < output.len() {
        let mut word = 0u64;
        if ptrace(
            PTRACE_PEEKDATA,
            pid,
            address.saturating_add(offset),
            &mut word as *mut _ as usize,
        ) < 0
        {
            return false;
        }
        let bytes = word.to_ne_bytes();
        let count = core::cmp::min(8, output.len() - offset);
        output[offset..offset + count].copy_from_slice(&bytes[..count]);
        offset += count;
    }
    true
}

fn native_rewrite_stat_identity(tracee_pid: usize, address: usize, uid: u32, gid: u32) -> bool {
    // x86-64 struct stat stores st_uid and st_gid at byte offsets 28 and 32.
    let mut identity = [0u8; 8];
    identity[..4].copy_from_slice(&uid.to_ne_bytes());
    identity[4..].copy_from_slice(&gid.to_ne_bytes());
    write_tracee_bytes(tracee_pid, address.saturating_add(28), &identity)
}

fn native_virtual_ucred(
    sender_host_pid: usize,
    tasks: &[Option<NativeSupervisorTask>; 128],
    current: NativeSupervisorTask,
    processes: &ProcessTable<128>,
) -> Option<[u8; 12]> {
    let sender = if current.host_pid == sender_host_pid {
        Some(current)
    } else {
        tasks
            .iter()
            .flatten()
            .find(|candidate| candidate.host_pid == sender_host_pid)
            .copied()
    };
    let mut encoded = [0u8; 12];
    if let Some(sender) = sender {
        encoded[..4].copy_from_slice(&sender.virtual_id.pid.to_ne_bytes());
        if let Ok(view) = processes.view(sender.virtual_id) {
            encoded[4..8].copy_from_slice(&view.credentials.uid.to_ne_bytes());
            encoded[8..12].copy_from_slice(&view.credentials.gid.to_ne_bytes());
        }
    } else {
        // A queued datagram can outlive its already-reaped sender. Never expose
        // an unmatched host PID or host credentials through the virtual ABI.
        encoded[4..8].copy_from_slice(&65534u32.to_ne_bytes());
        encoded[8..12].copy_from_slice(&65534u32.to_ne_bytes());
    }
    Some(encoded)
}

fn native_rewrite_ucred(
    tracee_pid: usize,
    address: usize,
    tasks: &[Option<NativeSupervisorTask>; 128],
    current: NativeSupervisorTask,
    processes: &ProcessTable<128>,
) -> bool {
    let Some(sender_host_pid) = read_tracee_u32(tracee_pid, address).map(|pid| pid as usize) else {
        return false;
    };
    let Some(encoded) = native_virtual_ucred(sender_host_pid, tasks, current, processes) else {
        return false;
    };
    write_tracee_bytes(tracee_pid, address, &encoded)
}

fn native_rewrite_recvmsg_credentials(
    tracee_pid: usize,
    message_address: usize,
    tasks: &[Option<NativeSupervisorTask>; 128],
    current: NativeSupervisorTask,
    processes: &ProcessTable<128>,
) -> bool {
    let mut message = [0u8; 56];
    if !read_tracee_bytes(tracee_pid, message_address, &mut message) {
        return false;
    }
    let control_address = u64::from_ne_bytes(message[32..40].try_into().unwrap_or([0; 8])) as usize;
    let control_length = u64::from_ne_bytes(message[40..48].try_into().unwrap_or([0; 8])) as usize;
    if control_address == 0 || control_length == 0 {
        return true;
    }
    let bounded_length = core::cmp::min(control_length, 1024);
    let mut control = [0u8; 1024];
    if !read_tracee_bytes(tracee_pid, control_address, &mut control[..bounded_length]) {
        return false;
    }
    let mut offset = 0usize;
    while offset.saturating_add(16) <= bounded_length {
        let length =
            u64::from_ne_bytes(control[offset..offset + 8].try_into().unwrap_or([0; 8])) as usize;
        if length < 16 || offset.saturating_add(length) > bounded_length {
            break;
        }
        let level = u32::from_ne_bytes(
            control[offset + 8..offset + 12]
                .try_into()
                .unwrap_or([0; 4]),
        );
        let kind = u32::from_ne_bytes(
            control[offset + 12..offset + 16]
                .try_into()
                .unwrap_or([0; 4]),
        );
        if level == 1 && kind == 2 && length >= 28 {
            let credential_address = control_address.saturating_add(offset).saturating_add(16);
            if !native_rewrite_ucred(tracee_pid, credential_address, tasks, current, processes) {
                return false;
            }
        }
        offset = offset.saturating_add((length + 7) & !7);
    }
    true
}

fn write_tracee_bytes(pid: usize, address: usize, input: &[u8]) -> bool {
    if address == 0 {
        return false;
    }
    let mut offset = 0usize;
    while offset < input.len() {
        let count = core::cmp::min(8, input.len() - offset);
        let mut bytes = if count == 8 {
            [0u8; 8]
        } else {
            let mut existing = 0u64;
            if ptrace(
                PTRACE_PEEKDATA,
                pid,
                address.saturating_add(offset),
                &mut existing as *mut _ as usize,
            ) < 0
            {
                return false;
            }
            existing.to_ne_bytes()
        };
        bytes[..count].copy_from_slice(&input[offset..offset + count]);
        if ptrace(
            PTRACE_POKEDATA,
            pid,
            address.saturating_add(offset),
            u64::from_ne_bytes(bytes) as usize,
        ) < 0
        {
            return false;
        }
        offset += count;
    }
    true
}

fn read_tracee_u64_pair(pid: usize, address: usize) -> Option<[u64; 2]> {
    if address == 0 {
        return None;
    }
    let mut values = [0u64; 2];
    for (index, value) in values.iter_mut().enumerate() {
        if ptrace(
            PTRACE_PEEKDATA,
            pid,
            address.saturating_add(index * 8),
            value as *mut _ as usize,
        ) < 0
        {
            return None;
        }
    }
    Some(values)
}

fn tracee_decimal_fd_path(pid: usize, address: usize) -> bool {
    const PREFIX: &[u8] = b"/proc/self/fd/";
    if address == 0 {
        return false;
    }
    let mut bytes = [0u8; 32];
    let mut offset = 0usize;
    while offset < bytes.len() {
        let mut word = 0u64;
        if ptrace(
            PTRACE_PEEKDATA,
            pid,
            address.saturating_add(offset),
            &mut word as *mut _ as usize,
        ) < 0
        {
            return false;
        }
        for byte in word.to_ne_bytes() {
            if byte == 0 {
                return offset > PREFIX.len()
                    && bytes[..PREFIX.len()] == *PREFIX
                    && bytes[PREFIX.len()..offset].iter().all(u8::is_ascii_digit);
            }
            bytes[offset] = byte;
            offset += 1;
            if offset >= bytes.len() {
                return false;
            }
        }
    }
    false
}

fn tracee_proc_self_fd_number(pid: usize, address: usize) -> Option<usize> {
    const PREFIX: &[u8] = b"/proc/self/fd/";
    if address == 0 {
        return None;
    }
    let mut bytes = [0u8; 32];
    let mut offset = 0usize;
    while offset < bytes.len() {
        let mut word = 0u64;
        if ptrace(
            PTRACE_PEEKDATA,
            pid,
            address.saturating_add(offset),
            &mut word as *mut _ as usize,
        ) < 0
        {
            return None;
        }
        for byte in word.to_ne_bytes() {
            if byte == 0 {
                if offset <= PREFIX.len()
                    || bytes[..PREFIX.len()] != *PREFIX
                    || !bytes[PREFIX.len()..offset].iter().all(u8::is_ascii_digit)
                {
                    return None;
                }
                let mut fd = 0usize;
                for digit in &bytes[PREFIX.len()..offset] {
                    fd = fd
                        .checked_mul(10)?
                        .checked_add(usize::from(*digit - b'0'))?;
                }
                return Some(fd);
            }
            if offset >= bytes.len() {
                return None;
            }
            bytes[offset] = byte;
            offset += 1;
        }
    }
    None
}

fn tracee_decimal_fdinfo_path(pid: usize, address: usize) -> Option<usize> {
    const PREFIX: &[u8] = b"/proc/self/fdinfo/";
    if address == 0 {
        return None;
    }
    let mut bytes = [0u8; 48];
    let mut offset = 0usize;
    while offset < bytes.len() {
        let mut word = 0u64;
        if ptrace(
            PTRACE_PEEKDATA,
            pid,
            address.saturating_add(offset),
            &mut word as *mut _ as usize,
        ) < 0
        {
            return None;
        }
        for byte in word.to_ne_bytes() {
            if byte == 0 {
                if offset <= PREFIX.len()
                    || bytes[..PREFIX.len()] != *PREFIX
                    || !bytes[PREFIX.len()..offset].iter().all(u8::is_ascii_digit)
                {
                    return None;
                }
                let mut fd = 0usize;
                for digit in &bytes[PREFIX.len()..offset] {
                    fd = fd
                        .checked_mul(10)?
                        .checked_add(usize::from(*digit - b'0'))?;
                }
                return Some(fd);
            }
            if offset >= bytes.len() {
                return None;
            }
            bytes[offset] = byte;
            offset += 1;
        }
    }
    None
}

fn write_tracee_string_to(fd: usize, pid: usize, address: usize) {
    if address == 0 {
        write_all_to(fd, b"(null)");
        return;
    }
    let mut offset = 0usize;
    while offset < 256 {
        let mut word = 0u64;
        if ptrace(
            PTRACE_PEEKDATA,
            pid,
            address.saturating_add(offset),
            &mut word as *mut _ as usize,
        ) < 0
        {
            write_all_to(fd, b"(unreadable)");
            return;
        }
        for byte in word.to_ne_bytes() {
            if byte == 0 {
                return;
            }
            if byte.is_ascii_graphic() || byte == b' ' {
                let printable = [byte];
                write_all_to(fd, &printable);
            } else {
                write_all_to(fd, b"?");
            }
            offset += 1;
            if offset >= 256 {
                return;
            }
        }
    }
}

fn line(message: &[u8]) {
    line_to(1, message);
}

fn line_to(fd: usize, message: &[u8]) {
    write_all_to(fd, message);
    write_all_to(fd, b"\n");
}

fn write_all_to(fd: usize, bytes: &[u8]) {
    let mut offset = 0;
    while offset < bytes.len() {
        let result = unsafe {
            syscall3(
                SYS_WRITE_HOST,
                fd,
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

fn write_all(bytes: &[u8]) {
    write_all_to(1, bytes);
}

fn write_u64_to(fd: usize, mut value: u64) {
    let mut buffer = [0u8; 20];
    let mut index = buffer.len();
    if value == 0 {
        write_all_to(fd, b"0");
        return;
    }
    while value != 0 {
        index -= 1;
        buffer[index] = b'0' + (value % 10) as u8;
        value /= 10;
    }
    write_all_to(fd, &buffer[index..]);
}

fn write_u64(value: u64) {
    write_u64_to(1, value);
}

fn write_i64_to(fd: usize, value: i64) {
    if value < 0 {
        write_all_to(fd, b"-");
        write_u64_to(fd, value.unsigned_abs());
    } else {
        write_u64_to(fd, value as u64);
    }
}

fn write_i64(value: i64) {
    write_i64_to(1, value);
}

fn fail(reason: &[u8]) -> ! {
    write_all(b"GATE1: FAIL: ");
    line(reason);
    exit_process(1)
}

fn exit_process(code: usize) -> ! {
    unsafe { syscall1(SYS_EXIT_HOST, code) };
    loop {
        core::hint::spin_loop();
    }
}

#[inline(always)]
unsafe fn syscall0(number: usize) -> isize {
    let result: isize;
    asm!(
        "syscall",
        inlateout("rax") number as isize => result,
        lateout("rcx") _,
        lateout("r11") _,
        options(nostack)
    );
    result
}

#[inline(always)]
unsafe fn syscall1(number: usize, a1: usize) -> isize {
    let result: isize;
    asm!(
        "syscall",
        inlateout("rax") number as isize => result,
        in("rdi") a1,
        lateout("rcx") _,
        lateout("r11") _,
        options(nostack)
    );
    result
}

#[inline(always)]
unsafe fn syscall2(number: usize, a1: usize, a2: usize) -> isize {
    let result: isize;
    asm!(
        "syscall",
        inlateout("rax") number as isize => result,
        in("rdi") a1,
        in("rsi") a2,
        lateout("rcx") _,
        lateout("r11") _,
        options(nostack)
    );
    result
}

#[inline(always)]
unsafe fn syscall3(number: usize, a1: usize, a2: usize, a3: usize) -> isize {
    let result: isize;
    asm!(
        "syscall",
        inlateout("rax") number as isize => result,
        in("rdi") a1,
        in("rsi") a2,
        in("rdx") a3,
        lateout("rcx") _,
        lateout("r11") _,
        options(nostack)
    );
    result
}

#[inline(always)]
unsafe fn syscall4(number: usize, a1: usize, a2: usize, a3: usize, a4: usize) -> isize {
    let result: isize;
    asm!(
        "syscall",
        inlateout("rax") number as isize => result,
        in("rdi") a1,
        in("rsi") a2,
        in("rdx") a3,
        in("r10") a4,
        lateout("rcx") _,
        lateout("r11") _,
        options(nostack)
    );
    result
}

#[inline(always)]
unsafe fn syscall5(number: usize, a1: usize, a2: usize, a3: usize, a4: usize, a5: usize) -> isize {
    let result: isize;
    asm!(
        "syscall",
        inlateout("rax") number as isize => result,
        in("rdi") a1,
        in("rsi") a2,
        in("rdx") a3,
        in("r10") a4,
        in("r8") a5,
        lateout("rcx") _,
        lateout("r11") _,
        options(nostack)
    );
    result
}
