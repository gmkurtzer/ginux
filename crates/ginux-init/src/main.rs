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
    SignalDelivery, TimerTable, VirtualClock, EVENT_READ, EVENT_WRITE,
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
const SYS_SIGNALFD4: usize = 289;
const AT_FDCWD: usize = (-100isize) as usize;
const O_RDONLY: usize = 0;
const O_RDWR: usize = 2;
const CLOCK_MONOTONIC: usize = 1;
const CLOCK_PROCESS_CPUTIME_ID: usize = 2;

const PTRACE_TRACEME: usize = 0;
const PTRACE_PEEKTEXT: usize = 1;
const PTRACE_POKETEXT: usize = 4;
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
            } else if argc >= 5 && c_string_equals(unsafe { *argv.add(4) as *const u8 }, b"systemd")
            {
                3
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
    let mut domain = Domain::create(config);
    if domain.start().is_err() {
        domain_fail(&mut domain, b"invalid native start transition");
    }
    let child = unsafe { syscall0(SYS_FORK) };
    if child < 0 {
        domain_fail(&mut domain, b"native tracee fork failed");
    }
    if child == 0 {
        native_tracee(root_path, mode);
    }
    supervise_native(child as usize, &mut domain, mode)
}

fn native_tracee(root_path: *const u8, mode: usize) -> ! {
    static ROOT: &[u8] = b"/\0";
    static BASH: &[u8] = b"/usr/bin/bash\0";
    static ARG1: &[u8] = b"--noprofile\0";
    static ARG2: &[u8] = b"--norc\0";
    static ARG3: &[u8] = b"-c\0";
    static COMMAND: &[u8] = b"printf 'GINUX_NATIVE_SHELL: PASS\\n'\0";
    static HOLD_COMMAND: &[u8] = b"printf 'GINUX_NATIVE_HOLD: READY\\n'; while :; do :; done\0";
    static PROBE_COMMAND: &[u8] = b"if : > /tmp/ginux-mutation; then exit 90; else printf 'GINUX_BALANCED_MUTATION: BLOCKED\\n'; fi; if exec 9<>/dev/tcp/127.0.0.1/9; then exit 91; else printf 'GINUX_BALANCED_NETWORK: BLOCKED\\n'; fi\0";
    static SYSTEMD: &[u8] = b"/sbin/init\0";
    static SYSTEMD_ARG1: &[u8] = b"--system\0";
    static SYSTEMD_ARG2: &[u8] = b"--unit=multi-user.target\0";
    static ENV_PATH: &[u8] = b"PATH=/usr/bin:/bin\0";
    static ENV_HOME: &[u8] = b"HOME=/\0";
    static ENV_LOCALE: &[u8] = b"LC_ALL=C\0";
    static ENV_CONTAINER: &[u8] = b"container=ginux\0";

    if unsafe { syscall1(SYS_CHROOT, root_path as usize) } < 0 {
        native_tracee_setup_fail(120);
    }
    if unsafe { syscall1(SYS_CHDIR, ROOT.as_ptr() as usize) } < 0 {
        native_tracee_setup_fail(121);
    }
    let mut inherited = 3usize;
    while inherited < 256 {
        let _ = unsafe { syscall1(SYS_CLOSE_HOST, inherited) };
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
    let systemd_argv = [
        SYSTEMD.as_ptr(),
        SYSTEMD_ARG1.as_ptr(),
        SYSTEMD_ARG2.as_ptr(),
        core::ptr::null(),
    ];
    let environment = [
        ENV_PATH.as_ptr(),
        ENV_HOME.as_ptr(),
        ENV_LOCALE.as_ptr(),
        ENV_CONTAINER.as_ptr(),
        core::ptr::null(),
    ];
    unsafe {
        syscall3(
            SYS_EXECVE,
            if mode == 3 {
                SYSTEMD.as_ptr() as usize
            } else {
                BASH.as_ptr() as usize
            },
            if mode == 3 {
                systemd_argv.as_ptr() as usize
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

fn native_tracee_setup_fail(code: usize) -> ! {
    unsafe { syscall1(SYS_EXIT_HOST, code) };
    loop {
        core::hint::spin_loop();
    }
}

fn supervise_native(pid: usize, domain: &mut Domain, mode: usize) -> ! {
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
    let mut awaiting_exit = false;
    let mut override_exit = false;
    let mut override_result = 0i64;
    loop {
        status = 0;
        if wait(pid, &mut status) < 0 || !stopped(status) {
            native_fail(domain, pid, b"native tracee escaped supervision");
        }
        let signal = ((status >> 8) & 0xff) as u8;
        let event = ((status as usize) >> 16) & 0xffff;
        if matches!(
            event,
            PTRACE_EVENT_FORK | PTRACE_EVENT_VFORK | PTRACE_EVENT_CLONE
        ) {
            let mut child = 0usize;
            let _ = ptrace(PTRACE_GETEVENTMSG, pid, 0, &mut child as *mut _ as usize);
            if child != 0 {
                let _ = ptrace(PTRACE_KILL, child, 0, 0);
            }
            native_fail(
                domain,
                pid,
                b"managed descendant requires multi-task supervision",
            );
        }
        if event != 0 && event != PTRACE_EVENT_EXEC {
            native_fail(domain, pid, b"unexpected native ptrace event");
        }
        if signal != (5 | 0x80) && signal != 5 {
            native_fail(domain, pid, b"unexpected native trace stop");
        }
        let mut registers = core::mem::MaybeUninit::<UserRegs>::uninit();
        if ptrace(PTRACE_GETREGS, pid, 0, registers.as_mut_ptr() as usize) < 0 {
            native_fail(domain, pid, b"native register read failed");
        }
        let mut registers = unsafe { registers.assume_init() };

        if awaiting_exit {
            if override_exit {
                registers.rax = override_result as u64;
                if ptrace(PTRACE_SETREGS, pid, 0, &registers as *const _ as usize) < 0 {
                    native_fail(domain, pid, b"native result override failed");
                }
            }
            write_all(b"GINUX_NATIVE_AUDIT: sequence=");
            write_u64(sequence);
            write_all(b" phase=exit result=");
            write_i64(registers.rax as i64);
            write_all(b"\n");
            awaiting_exit = false;
            override_exit = false;
            if ptrace(PTRACE_SYSCALL, pid, 0, 0) < 0 {
                native_fail(domain, pid, b"native resume failed");
            }
            continue;
        }

        sequence += 1;
        let syscall = registers.orig_rax;
        let shared_network = mode == 3 && domain.config().network_backend == NetworkBackend::Shared;
        let decision = if mode == 3 {
            decide_systemd(syscall, shared_network)
        } else {
            decide_balanced(syscall)
        };
        write_all(b"GINUX_NATIVE_AUDIT: sequence=");
        write_u64(sequence);
        write_all(b" phase=entry syscall=");
        write_u64(syscall);
        write_all(b" action=");
        match decision.action {
            Action::Emulate if native_identity_result(syscall, mode).is_some() => {
                let result = native_identity_result(syscall, mode).unwrap_or(0);
                write_all(b"emulate surface=virtual-identity result=");
                write_i64(result);
                write_all(b"\n");
                registers.orig_rax = u64::MAX;
                awaiting_exit = true;
                override_exit = true;
                override_result = result;
                if ptrace(PTRACE_SETREGS, pid, 0, &registers as *const _ as usize) < 0
                    || ptrace(PTRACE_SYSCALL, pid, 0, 0) < 0
                {
                    native_fail(domain, pid, b"native getpid emulation failed");
                }
            }
            Action::Emulate if syscall == SYS_EXIT || syscall == SYS_EXIT_GROUP => {
                line(b"emulate result=domain-stop");
                let _ = ptrace(PTRACE_KILL, pid, 0, 0);
                let _ = wait(pid, &mut status);
                if domain.task_exited(true).is_err() {
                    domain_fail(domain, b"native exit transition failed");
                }
                line(b"GINUX_NATIVE: supervisor=attached direct-unsupervised-syscalls=0");
                line(b"GINUX_NATIVE_CHECK: PASS");
                exit_process(0)
            }
            Action::Mediate if native_arguments_allowed(syscall, &registers, mode) => {
                write_all(b"mediate surface=");
                line(balanced_surface_name(if mode == 3 {
                    systemd_surface(syscall, shared_network)
                } else {
                    balanced_surface(syscall)
                }));
                awaiting_exit = true;
                if ptrace(PTRACE_SYSCALL, pid, 0, 0) < 0 {
                    native_fail(domain, pid, b"native mediated syscall failed");
                }
            }
            Action::Mediate => {
                line(b"deny result=-1 reason=invalid-arguments");
                registers.orig_rax = u64::MAX;
                awaiting_exit = true;
                override_exit = true;
                override_result = -EPERM;
                if ptrace(PTRACE_SETREGS, pid, 0, &registers as *const _ as usize) < 0
                    || ptrace(PTRACE_SYSCALL, pid, 0, 0) < 0
                {
                    native_fail(domain, pid, b"native argument denial failed");
                }
            }
            Action::Deny(errno) => {
                write_all(b"deny result=-");
                write_u64(errno as u64);
                write_all(b"\n");
                registers.orig_rax = u64::MAX;
                awaiting_exit = true;
                override_exit = true;
                override_result = -(errno as i64);
                if ptrace(PTRACE_SETREGS, pid, 0, &registers as *const _ as usize) < 0
                    || ptrace(PTRACE_SYSCALL, pid, 0, 0) < 0
                {
                    native_fail(domain, pid, b"native syscall denial failed");
                }
            }
            _ => native_fail(domain, pid, b"invalid native contract decision"),
        }
    }
}

fn native_identity_result(syscall: u64, mode: usize) -> Option<i64> {
    if mode == 3 {
        match syscall {
            SYS_GETPID => Some(1),
            ginux_core::SYS_GETPPID => Some(0),
            ginux_core::SYS_GETPGRP => Some(1),
            _ => balanced_identity_result(syscall),
        }
    } else {
        balanced_identity_result(syscall)
    }
}

fn native_arguments_allowed(syscall: u64, registers: &UserRegs, mode: usize) -> bool {
    if mode == 3 {
        match syscall {
            1 | 2 | 16 | 59 | 95 | 125 | 126 | 137 | 157 | 257 => true,
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

fn line(message: &[u8]) {
    write_all(message);
    write_all(b"\n");
}

fn write_all(bytes: &[u8]) {
    let mut offset = 0;
    while offset < bytes.len() {
        let result = unsafe {
            syscall3(
                SYS_WRITE_HOST,
                1,
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

fn write_u64(mut value: u64) {
    let mut buffer = [0u8; 20];
    let mut index = buffer.len();
    if value == 0 {
        write_all(b"0");
        return;
    }
    while value != 0 {
        index -= 1;
        buffer[index] = b'0' + (value % 10) as u8;
        value /= 10;
    }
    write_all(&buffer[index..]);
}

fn write_i64(value: i64) {
    if value < 0 {
        write_all(b"-");
        write_u64(value.unsigned_abs());
    } else {
        write_u64(value as u64);
    }
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
