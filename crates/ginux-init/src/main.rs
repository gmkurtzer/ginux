#![no_std]
#![no_main]

use core::arch::global_asm;
use core::panic::PanicInfo;
use ginux_core::{
    balanced_identity_result, balanced_surface, decide, Action, SYS_EXIT, SYS_EXIT_GROUP,
    SYS_GETPID, SYS_UNAME, SYS_WRITE,
};

mod channel_hooks;
mod dispatch;
mod fs_hooks;
mod launch;
mod process_hooks;
mod supervisor;

use crate::dispatch::{
    balanced_surface_name, decide_native, native_arguments_allowed, process_entry, Lane,
    NativeDecision,
};
use crate::launch::{
    c_string_equals, launch_main, launch_usage, load_config, parse_argv, print_effective_config,
    read_file, read_index, Command, ELF_BYTES, VFS_INDEX_BYTES,
};
use crate::process_hooks::{
    dup3_flags_valid, exec_mediated, fcntl_supported, pipe2_flags_valid, thread_supported,
    FD_CLOEXEC, F_DUPFD, F_DUPFD_CLOEXEC, F_GETFD, F_SETFD, O_CLOEXEC, PTRACE_EVENT_CLONE,
    PTRACE_EVENT_VFORK, PTRACE_O_TRACECLONE, PTRACE_O_TRACEVFORK, SYS_ARCH_PRCTL, SYS_CLOSE,
    SYS_DUP, SYS_DUP2, SYS_DUP3, SYS_FCNTL, SYS_FUTEX, SYS_GETTID, SYS_GET_ROBUST_LIST, SYS_PIPE,
    SYS_PIPE2, SYS_READ, SYS_RSEQ, SYS_SET_ROBUST_LIST, SYS_SET_TID_ADDRESS,
};
use crate::supervisor::{
    exit_process, line, ptrace, read_tracee_word, stopped, syscall0, syscall1, syscall2, syscall3,
    syscall4, syscall5, wait, write_all, write_i64, write_u64, UserRegs, AT_FDCWD, CLOCK_MONOTONIC,
    CLOCK_PROCESS_CPUTIME_ID, DOMAIN_GID, DOMAIN_UID, EBADF, ECHILD, EINVAL, ENOSYS, EPERM, ESRCH,
    O_RDONLY, O_RDWR, PR_SET_NO_NEW_PRIVS, PTRACE_CONT, PTRACE_EVENT_EXEC, PTRACE_EVENT_FORK,
    PTRACE_GETEVENTMSG, PTRACE_GETREGS, PTRACE_KILL, PTRACE_O_EXITKILL, PTRACE_O_TRACEEXEC,
    PTRACE_O_TRACEFORK, PTRACE_O_TRACESYSGOOD, PTRACE_PEEKTEXT, PTRACE_POKETEXT, PTRACE_SETOPTIONS,
    PTRACE_SETREGS, PTRACE_SYSCALL, PTRACE_SYSEMU, PTRACE_TRACEME, SIGCHLD, SIGINT, SIGKILL,
    SIGTERM, SIG_BLOCK, SYS_CHDIR, SYS_CHROOT, SYS_CLOCK_GETTIME, SYS_CLOSE_HOST, SYS_EXECVE,
    SYS_EXIT_HOST, SYS_FORK, SYS_FSYNC, SYS_KILL, SYS_OPENAT_HOST, SYS_PAUSE, SYS_PRCTL,
    SYS_PREAD64, SYS_PTRACE, SYS_PWRITE64, SYS_READ_HOST, SYS_RT_SIGPROCMASK, SYS_SETGID,
    SYS_SETGROUPS, SYS_SETUID, SYS_SIGNALFD4, SYS_WAIT4, VIRTUAL_PID,
};
use ginux_devices::{
    BlockBackend, DeviceError, DeviceKind, DeviceName, DeviceRegistry, DeviceView, IsolatedNetwork,
    Partition, IOCTL_CONSOLE_GET_SIZE,
};
use ginux_domain::Domain;
use ginux_kernel_views::{
    CgroupFs, IdentityFs, IdentityView, MemoryView, MountView, NamespaceView, ProcFs, SysFs,
    ViewError,
};
use ginux_process::{
    ChildStatus, Credentials, EventFd, EventPoll, FutexTable, PipeSpace, ProcessError,
    ProcessTable, SignalAction, SignalDelivery, TaskState, TimerTable, VirtualClock, WaitTarget,
    EVENT_READ, EVENT_WRITE, WAIT_ALLOWED_OPTIONS, WNOHANG,
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
    match parse_argv(argc, argv) {
        Some(Command::Runtime(path)) => runtime_main(path),
        Some(Command::Config(config_path)) => config_main(config_path),
        Some(Command::Lifecycle(config_path)) => lifecycle_main(config_path),
        Some(Command::Vfs(config_path, rootfs_path, index_path)) => {
            vfs_main(config_path, rootfs_path, index_path)
        }
        Some(Command::Native(config_path, root_path, mode)) => {
            native_main(config_path, root_path, mode)
        }
        Some(Command::Domain(config_path, fixture_path)) => domain_main(config_path, fixture_path),
        Some(Command::Process(config_path)) => process_main(config_path),
        Some(Command::ProcessNative(config_path, fixture_path)) => {
            process_native_main(config_path, fixture_path)
        }
        Some(Command::Devices(config_path, block_path)) => devices_main(config_path, block_path),
        Some(Command::DeviceBench(config_path, block_path, kind)) => {
            device_bench_main(config_path, block_path, kind)
        }
        Some(Command::KernelViews(config_path)) => kernel_views_main(config_path),
        Some(Command::Launch(config_path, root_path, result_path)) => {
            launch_main(config_path, root_path, result_path)
        }
        Some(Command::LaunchUsage) => launch_usage(),
        None => gate1_main(),
    }
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
        // Thread-group members are joined over a futex, never reaped: a
        // wait naming the thread must fail while the leader reaps normally.
        if processes
            .wait(init, WaitTarget::Specific(thread.pid), 0)
            .is_ok()
            || processes
                .wait(init, WaitTarget::Specific(child.pid), 0)
                .map(|result| result.status)
                != Ok(ChildStatus::Exited(iteration as i32))
        {
            process_fail(b"wait or reap failed");
        }
        // Thread-group members are never reaped by `wait`; reclaim the
        // joined thread the way the supervision loop's final cleanup does,
        // or the bounded table would fill before iteration 256.
        if processes.reap_all() != 1 {
            process_fail(b"thread reap failed");
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
    /// A blocking `wait4` with nothing reportable parks the caller at its
    /// entry stop (the host call never executes) until a matching child
    /// state change wakes it. The selector/options/pointers snapshot the
    /// parked request; registers stay untouched while parked.
    wait_pending: bool,
    wait_selector: i64,
    wait_options: u32,
    wait_status: usize,
    wait_rusage: usize,
    fork_result_pending: u32,
    fork_breakpoint_rip: usize,
    fork_breakpoint_word: usize,
    fork_variant_event: u32,
    exec_pending: bool,
    exec_event_seen: bool,
    /// A mediated descriptor entry stashed for its exit stop: the syscall
    /// number plus its arguments (pipefd pointer or oldfd, newfd or flags,
    /// flags). Zero when no descriptor call is in flight.
    desc_syscall: u64,
    desc_arg0: u64,
    desc_arg1: u64,
    desc_arg2: u64,
    tracing: bool,
    thread_pending: bool,
    clone_ptid: usize,
    set_tid_pending: bool,
    clone_files: bool,
    clone_ctid: usize,
    child_tid: usize,
    held_status: i32,
    created_virtual: u32,
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
    const SYS_CLOSE_RANGE: usize = 436;
    if unsafe { syscall3(SYS_CLOSE_RANGE, 3, u32::MAX as usize, 0) } < 0 {
        native_tracee_setup_fail(132);
    }
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
    let options = PTRACE_O_TRACESYSGOOD
        | PTRACE_O_TRACEFORK
        | PTRACE_O_TRACEVFORK
        | PTRACE_O_TRACECLONE
        | PTRACE_O_TRACEEXEC
        | PTRACE_O_EXITKILL;
    if ptrace(PTRACE_SETOPTIONS, root_host_pid, 0, options) < 0 {
        native_fail(domain, root_host_pid, b"process ptrace options failed");
    }
    let mut processes: ProcessTable<8> = ProcessTable::new();
    // G05a descriptor mirror: pipe pairs, dups, and close-on-exec ends as
    // supervisor-owned domain objects, keyed by virtual pid. Every mutation
    // below follows a host outcome the loop observed.
    let mut pipes: PipeSpace<8, 16, 16> = PipeSpace::new();
    let root_virtual = processes
        .create_init(Credentials::unprivileged(1000, 1000))
        .unwrap_or_else(|_| process_native_fail(domain, root_host_pid, b"virtual init failed"));
    let mut tasks: [Option<NativeProcessTask>; 8] = [None; 8];
    // wait4 may report a new tracee's initial SIGSTOP before its parent's
    // creation event. Keep it stopped until GETEVENTMSG identifies its owner.
    let mut newborn_stops: [Option<(usize, i32)>; 8] = [None; 8];
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
        fork_variant_event: 0,
        exec_pending: false,
        exec_event_seen: false,
        desc_syscall: 0,
        desc_arg0: 0,
        desc_arg1: 0,
        desc_arg2: 0,
        tracing: true,
        thread_pending: false,
        clone_ptid: 0,
        set_tid_pending: false,
        clone_files: false,
        clone_ctid: 0,
        child_tid: 0,
        held_status: 0,
        created_virtual: 0,
        wait_pending: false,
        wait_selector: 0,
        wait_options: 0,
        wait_status: 0,
        wait_rusage: 0,
    });
    let mut root_exited = false;
    let mut root_failed = false;
    if ptrace(PTRACE_SYSCALL, root_host_pid, 0, 0) < 0 {
        process_native_fail(domain, root_host_pid, b"process initial resume failed");
    }
    line(b"GINUX_NATIVE_PROCESS: virtual-init=1 host-pid=unexposed ptrace=attached");

    loop {
        status = 0;
        // A sibling exec can kill a task even while it is ptrace-stopped.
        // Hold its stop until exec succeeds (and removes it) or fails; on
        // failure process the saved stop normally before executing anything.
        let ready = tasks.iter().position(|entry| {
            entry.is_some_and(|task| {
                task.held_status != 0 && !group_exec_pending(&tasks, &processes, task.virtual_id)
            })
        });
        let waited = if let Some(index) = ready {
            let held = tasks[index].as_mut().unwrap();
            status = held.held_status;
            held.held_status = 0;
            held.host_pid as isize
        } else {
            unsafe {
                syscall4(
                    SYS_WAIT4,
                    (-1isize) as usize,
                    &mut status as *mut _ as usize,
                    0,
                    0,
                )
            }
        };
        if waited <= 0 {
            process_native_fail(domain, root_host_pid, b"process wait-any failed");
        }
        let host_pid = waited as usize;
        let mut former_tid = host_pid;
        if stopped(status)
            && (status as usize) >> 16 == PTRACE_EVENT_EXEC
            && ptrace(
                PTRACE_GETEVENTMSG,
                host_pid,
                0,
                &mut former_tid as *mut _ as usize,
            ) < 0
        {
            process_native_fail(domain, root_host_pid, b"exec former tid read failed");
        }
        let Some(task_index) = tasks
            .iter()
            .position(|task| task.is_some_and(|task| task.host_pid == former_tid))
        else {
            if status == (19 << 8) | 0x7f
                && tasks.iter().flatten().any(|task| {
                    task.tracing
                        && task.awaiting_exit
                        && !task.override_result
                        && matches!(task.syscall, 56..=58)
                })
                && !newborn_stops
                    .iter()
                    .flatten()
                    .any(|&(pid, _)| pid == host_pid)
            {
                let Some(slot) = newborn_stops.iter_mut().find(|slot| slot.is_none()) else {
                    process_native_fail(domain, root_host_pid, b"newborn stop table full");
                };
                *slot = Some((host_pid, status));
                line(b"GINUX_NATIVE_PROCESS_AUDIT: newborn-stop=deferred");
                continue;
            }
            process_native_fail(domain, root_host_pid, b"unknown host tracee");
        };
        let task = tasks[task_index].unwrap();
        if stopped(status)
            && (status as usize) >> 16 == 0
            && group_exec_pending(&tasks, &processes, task.virtual_id)
        {
            tasks[task_index].as_mut().unwrap().held_status = status;
            continue;
        }

        if !stopped(status) {
            // A zero low byte is a normal exit; anything else is death by
            // that signal number (core-dump bit excluded). Both outcomes
            // live in the virtual table so managed waits report them; the
            // shell-style code below is diagnostics only.
            let child_status = if status & 0x7f == 0 {
                ChildStatus::Exited(status >> 8)
            } else {
                ChildStatus::Signaled((status & 0x7f) as u8)
            };
            let exit_status = child_status.shell_code();
            let recorded = match child_status {
                ChildStatus::Exited(code) => processes.exit(task.virtual_id, code),
                ChildStatus::Signaled(signal) => processes.signaled(task.virtual_id, signal),
            };
            if recorded.is_err() {
                process_native_fail(domain, root_host_pid, b"virtual exit failed");
            }
            // G05a: the host closed every descriptor with the task. Release
            // the mirror's ends now, so the pipe objects die their last
            // close here and no stale write end can hide an EOF elsewhere.
            pipes.release_task(task.virtual_id.pid);
            // Thread exit with cleanup: a task whose tgid differs from its
            // pid is a thread-group member. Threads are joined over a futex
            // on shared memory, never reaped by `wait4`, so the host slot is
            // dropped and its managed slot reclaimed now. The host kernel
            // already performed the
            // `CLONE_CHILD_CLEARTID` clear and wakeup as part of this exit.
            let thread_exit = processes
                .view(task.virtual_id)
                .is_ok_and(|view| view.tgid != view.id.pid);
            if thread_exit {
                write_all(b"GINUX_NATIVE_PROCESS_AUDIT: thread-exit virtual-tid=");
                write_u64(task.virtual_id.pid as u64);
                write_all(b" status=");
                write_i64(exit_status as i64);
                write_all(b"\n");
                processes.reap_thread(task.virtual_id).unwrap_or_else(|_| {
                    process_native_fail(domain, root_host_pid, b"thread reap failed")
                });
                tasks[task_index] = None;
            } else if let Some(entry) = tasks[task_index].as_mut() {
                entry.tracing = false;
                // A parked waiter can never complete its own wait once dead.
                entry.wait_pending = false;
            }
            if !thread_exit {
                // Queue virtual SIGCHLD for the parent and wake every
                // parked wait this state change satisfies. Thread exits
                // queue nothing: the host sends no SIGCHLD for them and no
                // wait consumes them.
                wake_waiters_for_child(
                    domain,
                    root_host_pid,
                    &mut processes,
                    &mut tasks,
                    task.virtual_id,
                );
            }
            if task.virtual_id == root_virtual {
                root_exited = true;
                if exit_status != 0 {
                    root_failed = true;
                }
            }
            // The main thread may exit while joined-or-detached siblings are
            // still draining (thread death reports arrive in kernel order, not
            // creation order), so a zero root exit waits for the remaining
            // traced tasks instead of failing. A nonzero root exit fails fast.
            if root_failed {
                write_all(b"GINUX_NATIVE_PROCESS: root-exit=");
                write_i64(exit_status as i64);
                write_all(b" tracing-remaining=");
                write_u64(tasks.iter().flatten().filter(|entry| entry.tracing).count() as u64);
                write_all(b"\n");
                process_native_fail(domain, root_host_pid, b"native process fixture failed");
            }
            if root_exited && !tasks.iter().flatten().any(|entry| entry.tracing) {
                // G05a leak audit: every end died with its task (last close
                // is the host's), so no pipe object may survive full reap.
                write_all(b"GINUX_NATIVE_PROCESS_AUDIT: descriptors live-pipes=");
                write_u64(pipes.active_pipes() as u64);
                write_all(b"\n");
                if processes.reap_all() == 0 || processes.active() != 0 {
                    process_native_fail(domain, root_host_pid, b"native process cleanup failed");
                }
                if domain.task_exited(true).is_err() {
                    process_native_fail(domain, root_host_pid, b"native domain exit failed");
                }
                line(b"GINUX_NATIVE_PROCESS: fork=mediated child=ptrace-attached pid-translation=PASS");
                line(
                    b"GINUX_NATIVE_PROCESS: wait=translated child-status=real tasks=0 cleanup=PASS",
                );
                line(b"GATE7_NATIVE_PROCESS: PASS");
                exit_process(0);
            }
            continue;
        }

        let event = (status as usize) >> 16;
        if event == PTRACE_EVENT_FORK || event == PTRACE_EVENT_VFORK || event == PTRACE_EVENT_CLONE
        {
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
            // A creation whose entry carried a supported `CLONE_THREAD`
            // shape shares the existing managed-task path as a thread-group
            // member (same tgid, fresh virtual tid); every other shape is an
            // ordinary parent/child process pair. The entry arm recorded
            // which one this is before the host call executed.
            let is_thread = tasks[task_index]
                .as_ref()
                .is_some_and(|parent| parent.thread_pending);
            let child_virtual = if is_thread {
                processes.clone_thread(task.virtual_id).unwrap_or_else(|_| {
                    process_native_fail(domain, root_host_pid, b"virtual thread failed")
                })
            } else {
                processes.fork(task.virtual_id).unwrap_or_else(|_| {
                    process_native_fail(domain, root_host_pid, b"virtual fork failed")
                })
            };
            // Mirror the actual CLONE_FILES choice, including non-thread
            // processes that share files and threads that do not.
            let inheritance = if task.clone_files {
                pipes.share(child_virtual.pid, task.virtual_id.pid)
            } else {
                pipes.inherit(child_virtual.pid, task.virtual_id.pid)
            };
            if inheritance.is_err() {
                process_native_fail(domain, root_host_pid, b"descriptor inherit failed");
            }
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
                fork_variant_event: 0,
                exec_pending: false,
                exec_event_seen: false,
                desc_syscall: 0,
                desc_arg0: 0,
                desc_arg1: 0,
                desc_arg2: 0,
                tracing: true,
                thread_pending: false,
                clone_ptid: 0,
                set_tid_pending: false,
                clone_files: false,
                clone_ctid: 0,
                child_tid: 0,
                held_status: 0,
                created_virtual: 0,
                wait_pending: false,
                wait_selector: 0,
                wait_options: 0,
                wait_status: 0,
                wait_rusage: 0,
            });
            tasks[child_slot].as_mut().unwrap().child_tid = task.clone_ctid;
            if let Some(slot) = newborn_stops
                .iter_mut()
                .find(|slot| slot.is_some_and(|(pid, _)| pid == child_host_pid))
            {
                tasks[child_slot].as_mut().unwrap().held_status = slot.take().unwrap().1;
                line(b"GINUX_NATIVE_PROCESS_AUDIT: newborn-stop=registered");
            }
            if event == PTRACE_EVENT_FORK {
                // Separate address space (raw fork, or clone without address
                // sharing, which the lane entry already enforced): the
                // breakpoint below is invisible to the child, so translate
                // the parent's observed result when it fires.
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
                parent.fork_variant_event = event as u32;
                if ptrace(PTRACE_CONT, host_pid, 0, 0) < 0 {
                    process_native_fail(
                        domain,
                        root_host_pid,
                        b"fork parent breakpoint resume failed",
                    );
                }
                continue;
            }
            // vfork/clone children may share the parent's text, where a
            // breakpoint would also trap the child: resume with SYSCALL and
            // translate the host child pid at the creation exit stop below.
            // The entry left `awaiting_exit` set, and `fork_result_pending`
            // stays clear so the breakpoint branch above is skipped.
            let parent = tasks[task_index].as_mut().unwrap();
            parent.fork_variant_event = event as u32;
            parent.created_virtual = child_virtual.pid;
            if ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0 {
                process_native_fail(domain, root_host_pid, b"fork parent resume failed");
            }
            continue;
        }
        if event == PTRACE_EVENT_EXEC {
            // Executable replacement: the host kernel already performed the
            // shebang/loader transition. The virtual task keeps its identity
            // (same host pid, same virtual pid) with a bumped exec epoch and
            // reset signal state, and stays attached: `awaiting_exit` remains
            // set so the execve exit stop below only consumes the pending
            // entry instead of looking like a fresh entry.
            if !task.exec_pending {
                process_native_fail(domain, root_host_pid, b"unexpected exec event");
            }
            let group = processes
                .view(task.virtual_id)
                .unwrap_or_else(|_| {
                    process_native_fail(domain, root_host_pid, b"exec tgid lookup failed")
                })
                .tgid;
            pipes.unshare(task.virtual_id.pid).unwrap_or_else(|_| {
                process_native_fail(domain, root_host_pid, b"exec descriptor unshare failed")
            });
            let scrubbed = pipes.exec_scrub(task.virtual_id.pid);
            for entry in &mut tasks {
                if let Some(sibling) = *entry {
                    if sibling.virtual_id != task.virtual_id
                        && processes
                            .view(sibling.virtual_id)
                            .is_ok_and(|view| view.tgid == group)
                    {
                        pipes.release_task(sibling.virtual_id.pid);
                        *entry = None;
                    }
                }
            }
            let (identity, epoch) = processes.exec_group(task.virtual_id).unwrap_or_else(|_| {
                process_native_fail(domain, root_host_pid, b"virtual exec failed")
            });
            pipes.rename_task(task.virtual_id.pid, identity.pid);
            let current = tasks[task_index].as_mut().unwrap();
            current.host_pid = host_pid;
            current.virtual_id = identity;
            current.tracing = true;
            if identity == root_virtual {
                root_exited = false;
            }
            write_all(b"GINUX_NATIVE_PROCESS_AUDIT: exec syscall=");
            write_u64(task.syscall);
            write_all(b" host-pid=");
            write_u64(host_pid as u64);
            write_all(b" virtual-pid=");
            write_u64(identity.pid as u64);
            write_all(b" epoch=");
            write_u64(epoch as u64);
            write_all(b" cloexec-dropped=");
            write_u64(scrubbed as u64);
            write_all(b"\n");
            tasks[task_index].as_mut().unwrap().exec_event_seen = true;
            if ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0 {
                process_native_fail(domain, root_host_pid, b"exec event resume failed");
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
            write_all(b"GINUX_NATIVE_PROCESS_AUDIT: fork variant=");
            if pending.fork_variant_event == PTRACE_EVENT_VFORK as u32 {
                write_all(b"vfork");
            } else if pending.fork_variant_event == PTRACE_EVENT_CLONE as u32 {
                write_all(b"clone");
            } else {
                write_all(b"fork");
            }
            write_all(b" host-result=");
            write_i64(parent_registers.rax as i64);
            write_all(b" virtual-result=");
            write_u64(virtual_pid as u64);
            write_all(b"\n");
            parent_registers.rip = pending.fork_breakpoint_rip as u64;
            parent_registers.rax = virtual_pid as u64;
            if pending.clone_ptid != 0
                && store_tracee_tid(host_pid, pending.clone_ptid, virtual_pid).is_err()
            {
                process_native_fail(domain, root_host_pid, b"clone parent tid store failed");
            }
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
            parent.fork_variant_event = 0;
            if ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0 {
                process_native_fail(domain, root_host_pid, b"fork parent resume failed");
            }
            continue;
        }
        if signal == 19 {
            if task.child_tid != 0 {
                if store_tracee_tid(host_pid, task.child_tid, task.virtual_id.pid).is_err() {
                    process_native_fail(domain, root_host_pid, b"child tid store failed");
                }
                tasks[task_index].as_mut().unwrap().child_tid = 0;
            }
            let _ = ptrace(PTRACE_SETOPTIONS, host_pid, 0, options);
            if ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0 {
                process_native_fail(domain, root_host_pid, b"fork child resume failed");
            }
            continue;
        }
        if signal == SIGCHLD as u8 {
            // The host kernel queued SIGCHLD for a managed child state
            // change: deliver it so the parent's disposition observes the
            // notification. The default disposition discards it, and no
            // handler installation is mediated yet, so delivery is safe.
            write_all(b"GINUX_NATIVE_PROCESS_AUDIT: sigchld delivered parent=");
            write_u64(task.virtual_id.pid as u64);
            write_all(b"\n");
            if ptrace(PTRACE_SYSCALL, host_pid, 0, SIGCHLD) < 0 {
                process_native_fail(domain, root_host_pid, b"SIGCHLD mediation failed");
            }
            continue;
        }
        // A forwarded termination signal first arrives as a delivery stop
        // for the task: forward normal termination signals, including
        // self-signals and delivery to a sibling in the target group.
        if signal == SIGINT as u8 || signal == SIGTERM as u8 {
            write_all(b"GINUX_NATIVE_PROCESS_AUDIT: terminating signal=");
            write_u64(signal as u64);
            write_all(b" virtual-pid=");
            write_u64(task.virtual_id.pid as u64);
            write_all(b"\n");
            if ptrace(PTRACE_SYSCALL, host_pid, 0, signal as usize) < 0 {
                process_native_fail(domain, root_host_pid, b"terminating signal resume failed");
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
            // Only mediated kills rewrite the pid argument now; waits are
            // fully emulated at entry, so the host wait result never appears
            // here. A woken parked waiter lands on `override_result` below
            // with its status word already stored.
            let restore_translated_argument =
                matches!(current.syscall, 62 | 274) && current.translated_rdi != 0;
            if current.desc_syscall != 0 && !current.override_result {
                // G05a descriptor exit: the host executed, so the tracee
                // keeps the real host result. The mirror follows the
                // outcome; failures leave it untouched.
                let desc_result = registers.rax as i64;
                match current.desc_syscall {
                    SYS_PIPE | SYS_PIPE2 => {
                        if desc_result == 0 {
                            match read_tracee_pipefds(host_pid, current.desc_arg0 as usize) {
                                Ok((read_fd, write_fd)) => {
                                    let cloexec = current.desc_syscall == SYS_PIPE2
                                        && current.desc_arg1 & O_CLOEXEC != 0;
                                    if pipes
                                        .create_pipe(
                                            current.virtual_id.pid,
                                            read_fd,
                                            write_fd,
                                            cloexec,
                                        )
                                        .is_err()
                                    {
                                        process_native_fail(
                                            domain,
                                            root_host_pid,
                                            b"pipe mirror full",
                                        );
                                    }
                                    write_all(b"GINUX_NATIVE_PROCESS_AUDIT: pipe task=");
                                    write_u64(current.virtual_id.pid as u64);
                                    write_all(b" read=");
                                    write_u64(read_fd as u64);
                                    write_all(b" write=");
                                    write_u64(write_fd as u64);
                                    write_all(b" cloexec=");
                                    write_u64(if cloexec { 1 } else { 0 });
                                    write_all(b"\n");
                                }
                                Err(()) => {
                                    process_native_fail(
                                        domain,
                                        root_host_pid,
                                        b"pipe result read failed",
                                    );
                                }
                            }
                        }
                    }
                    SYS_DUP => {
                        if desc_result >= 0 {
                            if pipes
                                .duplicate(
                                    current.virtual_id.pid,
                                    current.desc_arg0 as u32,
                                    desc_result as u32,
                                    false,
                                )
                                .is_err()
                            {
                                process_native_fail(domain, root_host_pid, b"dup mirror failed");
                            }
                            write_all(b"GINUX_NATIVE_PROCESS_AUDIT: dup task=");
                            write_u64(current.virtual_id.pid as u64);
                            write_all(b" old=");
                            write_u64(current.desc_arg0);
                            write_all(b" new=");
                            write_u64(desc_result as u64);
                            write_all(b"\n");
                        }
                    }
                    SYS_DUP2 | SYS_DUP3 => {
                        if desc_result >= 0 {
                            // `old == new` is a successful no-op: the host
                            // changed nothing, so the mirror keeps its entry.
                            if current.desc_arg0 as u32 != desc_result as u32 {
                                let cloexec = current.desc_syscall == SYS_DUP3
                                    && current.desc_arg2 & O_CLOEXEC != 0;
                                if pipes
                                    .duplicate(
                                        current.virtual_id.pid,
                                        current.desc_arg0 as u32,
                                        desc_result as u32,
                                        cloexec,
                                    )
                                    .is_err()
                                {
                                    process_native_fail(
                                        domain,
                                        root_host_pid,
                                        b"dup mirror failed",
                                    );
                                }
                            }
                            write_all(b"GINUX_NATIVE_PROCESS_AUDIT: dup task=");
                            write_u64(current.virtual_id.pid as u64);
                            write_all(b" old=");
                            write_u64(current.desc_arg0);
                            write_all(b" new=");
                            write_u64(desc_result as u64);
                            write_all(b"\n");
                        }
                    }
                    SYS_CLOSE => {
                        if desc_result == 0
                            && pipes
                                .close(current.virtual_id.pid, current.desc_arg0 as u32)
                                .unwrap_or(false)
                        {
                            write_all(b"GINUX_NATIVE_PROCESS_AUDIT: close task=");
                            write_u64(current.virtual_id.pid as u64);
                            write_all(b" fd=");
                            write_u64(current.desc_arg0);
                            write_all(b"\n");
                        }
                    }
                    SYS_FCNTL if desc_result >= 0 => {
                        if current.desc_arg1 == F_DUPFD || current.desc_arg1 == F_DUPFD_CLOEXEC {
                            let cloexec = current.desc_arg1 == F_DUPFD_CLOEXEC;
                            if pipes
                                .duplicate(
                                    current.virtual_id.pid,
                                    current.desc_arg0 as u32,
                                    desc_result as u32,
                                    cloexec,
                                )
                                .is_err()
                            {
                                process_native_fail(domain, root_host_pid, b"fcntl mirror failed");
                            }
                            write_all(b"GINUX_NATIVE_PROCESS_AUDIT: fcntl-dup task=");
                            write_u64(current.virtual_id.pid as u64);
                            write_all(b" old=");
                            write_u64(current.desc_arg0);
                            write_all(b" new=");
                            write_u64(desc_result as u64);
                            write_all(b"\n");
                        } else if current.desc_arg1 == F_SETFD {
                            let _ = pipes.set_cloexec(
                                current.virtual_id.pid,
                                current.desc_arg0 as u32,
                                current.desc_arg2 & FD_CLOEXEC != 0,
                            );
                        } else if current.desc_arg1 == F_GETFD {
                            // Observed only: the host already reported
                            // the live flag, and the mirror holds no
                            // copy of its own to update.
                        }
                    }
                    _ => {}
                }
                current.desc_syscall = 0;
                current.desc_arg0 = 0;
                current.desc_arg1 = 0;
                current.desc_arg2 = 0;
            } else if current.syscall >= 56
                && current.syscall <= 58
                && registers.rax as i64 > 0
                && current.fork_variant_event != 0
            {
                // Creation exit on the SYSCALL-resumed vfork/clone path (no
                // breakpoint could be planted in shared text): the kernel
                // reports the host child pid, which maps to the virtual
                // child created at the fork-event stop. Thread creation
                // shares this path: the parent observes the virtual tid, and
                // the parent-tid cell (which the host kernel filled with the
                // host tid) is republished with the virtual tid so every tid
                // observation agrees.
                // The child may have exited and been retired before the
                // parent reaches this stop; retain the event's identity.
                if current.created_virtual != 0 {
                    let virtual_pid = current.created_virtual;
                    write_all(b"GINUX_NATIVE_PROCESS_AUDIT: fork variant=");
                    if current.thread_pending {
                        write_all(b"thread");
                    } else if current.fork_variant_event == PTRACE_EVENT_VFORK as u32 {
                        write_all(b"vfork");
                    } else {
                        write_all(b"clone");
                    }
                    write_all(b" host-result=");
                    write_i64(registers.rax as i64);
                    write_all(b" virtual-result=");
                    write_u64(virtual_pid as u64);
                    write_all(b"\n");
                    registers.rax = virtual_pid as u64;
                    current.override_result = true;
                    if current.clone_ptid != 0 {
                        if store_tracee_tid(host_pid, current.clone_ptid, virtual_pid).is_err() {
                            process_native_fail(domain, root_host_pid, b"thread tid store failed");
                        }
                        write_all(b"GINUX_NATIVE_PROCESS_AUDIT: thread-tid virtual-tid=");
                        write_u64(virtual_pid as u64);
                        write_all(b"\n");
                    }
                }
                current.thread_pending = false;
                current.clone_ptid = 0;
            } else if current.syscall == SYS_SET_TID_ADDRESS
                && current.set_tid_pending
                && registers.rax as i64 > 0
            {
                // `set_tid_address` returns the caller's tid: publish the
                // virtual tid instead of the host one.
                write_all(b"GINUX_NATIVE_PROCESS_AUDIT: set-tid virtual-tid=");
                write_u64(current.virtual_id.pid as u64);
                write_all(b"\n");
                registers.rax = current.virtual_id.pid as u64;
                current.override_result = true;
                current.set_tid_pending = false;
            } else if (current.syscall == 59 || current.syscall == 322) && current.exec_pending {
                // Replacement exit: a recorded EXEC event means the new image
                // is already running under supervision and this stop (return
                // 0) only consumes the pending entry. A negative return is an
                // explicit failure such as ENOENT and the old image continues
                // under supervision. Success without a prior event is handled
                // defensively the same way the event handles it.
                if current.exec_event_seen {
                    if registers.rax as i64 != 0 {
                        process_native_fail(domain, root_host_pid, b"exec event without success");
                    }
                    write_all(b"GINUX_NATIVE_PROCESS_AUDIT: exec-complete virtual-pid=");
                    write_u64(current.virtual_id.pid as u64);
                    write_all(b"\n");
                } else if registers.rax as i64 == 0 {
                    process_native_fail(domain, root_host_pid, b"exec completed without an event");
                } else {
                    write_all(b"GINUX_NATIVE_PROCESS_AUDIT: exec-failed errno=");
                    write_i64(-(registers.rax as i64));
                    write_all(b" virtual-pid=");
                    write_u64(current.virtual_id.pid as u64);
                    write_all(b"\n");
                }
                current.exec_pending = false;
                current.exec_event_seen = false;
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
            current.fork_variant_event = 0;
            current.created_virtual = 0;
            current.exec_pending = false;
            current.exec_event_seen = false;
            current.desc_syscall = 0;
            current.desc_arg0 = 0;
            current.desc_arg1 = 0;
            current.desc_arg2 = 0;
            tasks[task_index] = Some(current);
            if ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0 {
                process_native_fail(domain, root_host_pid, b"process exit resume failed");
            }
            continue;
        }

        const PTRACE_GET_SYSCALL_INFO: usize = 0x420e;
        let mut syscall_info = [0u64; 11];
        if ptrace(
            PTRACE_GET_SYSCALL_INFO,
            host_pid,
            core::mem::size_of_val(&syscall_info),
            syscall_info.as_mut_ptr() as usize,
        ) < 24
            || syscall_info[0] as u8 != 1
            || (syscall_info[0] >> 32) as u32 != 0xc000003e
        {
            process_native_fail(
                domain,
                root_host_pid,
                b"process requires the x86-64 syscall ABI",
            );
        }
        current.syscall = registers.orig_rax;
        current.awaiting_exit = true;
        if let Some(result) = process_entry(
            registers.orig_rax,
            registers.rdi,
            registers.rsi,
            registers.rdx,
        ) {
            current.result = result;
            current.override_result = true;
            registers.orig_rax = u64::MAX;
        } else {
            match registers.orig_rax {
                39 | 110 | SYS_GETTID => {
                    let view = processes.view(current.virtual_id).unwrap_or_else(|_| {
                        process_native_fail(domain, root_host_pid, b"identity lookup failed")
                    });
                    // `getpid` reports the thread-group id (the leader's pid
                    // for every member), `gettid` the virtual tid (leader pid
                    // for the main thread, the fresh virtual tid for created
                    // threads), `getppid` the virtual parent. Host tids never
                    // leak.
                    current.result = if registers.orig_rax == 110 {
                        view.parent as i64
                    } else if registers.orig_rax == SYS_GETTID {
                        view.id.pid as i64
                    } else {
                        view.tgid as i64
                    };
                    current.override_result = true;
                    registers.orig_rax = u64::MAX;
                }
                56..=58 => {
                    // A supported thread creation records its parent-tid cell
                    // so the creation exit stop can publish the virtual tid
                    // there. Unsupported thread shapes never reach this arm:
                    // the lane entry already emulated `-ENOSYS` for them, so
                    // they never execute. Any other creation shape resets the
                    // pending thread state.
                    current.thread_pending =
                        registers.orig_rax == 56 && thread_supported(registers.rdi);
                    current.clone_files = registers.orig_rax == 56
                        && registers.rdi & crate::process_hooks::CLONE_FILES != 0;
                    current.clone_ptid = if registers.orig_rax == 56
                        && registers.rdi & crate::process_hooks::CLONE_PARENT_SETTID != 0
                    {
                        registers.rdx as usize
                    } else {
                        0
                    };
                    current.clone_ctid = if registers.orig_rax == 56
                        && registers.rdi & crate::process_hooks::CLONE_CHILD_SETTID != 0
                    {
                        registers.r10 as usize
                    } else {
                        0
                    };
                    if processes.active() == tasks.len() || tasks.iter().all(Option::is_some) {
                        current.result = -11;
                        current.override_result = true;
                        registers.orig_rax = u64::MAX;
                    }
                }
                SYS_ARCH_PRCTL | SYS_SET_ROBUST_LIST | SYS_RSEQ => {}
                SYS_FUTEX => {
                    // PI futexes encode host tids in workload memory and need
                    // translation that this process contract does not provide.
                    if matches!(registers.rsi & 0x7f, 6..=8 | 11..=13) {
                        current.result = -ENOSYS;
                        current.override_result = true;
                        registers.orig_rax = u64::MAX;
                    }
                }
                SYS_GET_ROBUST_LIST => {
                    if registers.rdi == current.virtual_id.pid as u64 {
                        current.translated_rdi = registers.rdi;
                        registers.rdi = 0;
                    } else if registers.rdi != 0 {
                        current.result = -ENOSYS;
                        current.override_result = true;
                        registers.orig_rax = u64::MAX;
                    }
                }
                SYS_SET_TID_ADDRESS => {
                    // Per-thread exit-clear registration; the returned caller
                    // tid is translated to the virtual tid at the exit stop.
                    current.set_tid_pending = true;
                }
                59 | 322 => {
                    // Executable replacement: the mediated shapes execute on
                    // the host with the tracee still attached, and the EXEC
                    // event records the virtual transition. Shapes needing
                    // descriptor-table plumbing never execute and fail here.
                    if exec_mediated(registers.orig_rax, registers.rdi, registers.r8) {
                        current.exec_pending = true;
                        current.exec_event_seen = false;
                    } else {
                        current.result = -ENOSYS;
                        current.override_result = true;
                        registers.orig_rax = u64::MAX;
                    }
                }
                61 => {
                    // Managed wait4: fully emulated, so the host call never
                    // executes and supervisor-side reaping can never race
                    // the tracee for a child. Selectors follow wait4 pid
                    // semantics; unknown option bits and stale selectors
                    // fail explicitly with errno, never with lost children.
                    let selector = registers.rdi as i32 as i64;
                    let status_ptr = registers.rsi as usize;
                    let options = registers.rdx as u32;
                    let rusage_ptr = registers.r10 as usize;
                    if options & !WAIT_ALLOWED_OPTIONS != 0 {
                        current.result = -EINVAL;
                        current.override_result = true;
                        registers.orig_rax = u64::MAX;
                    } else if let Some(target) =
                        resolve_wait_target(selector, &processes, current.virtual_id)
                    {
                        match processes.wait(current.virtual_id, target, options) {
                            Ok(done) => {
                                if status_ptr != 0
                                    && store_tracee_status(host_pid, status_ptr, done.status.word())
                                        .is_err()
                                {
                                    process_native_fail(
                                        domain,
                                        root_host_pid,
                                        b"wait status store failed",
                                    );
                                }
                                if rusage_ptr != 0
                                    && zero_tracee_rusage(host_pid, rusage_ptr).is_err()
                                {
                                    process_native_fail(
                                        domain,
                                        root_host_pid,
                                        b"wait rusage store failed",
                                    );
                                }
                                clear_host_slot(&mut tasks, done.child);
                                write_all(b"GINUX_NATIVE_PROCESS_AUDIT: wait selector=");
                                write_i64(selector);
                                write_all(b" virtual-result=");
                                write_u64(done.child.pid as u64);
                                write_all(b" status=");
                                write_i64(done.status.shell_code() as i64);
                                write_all(b"\n");
                                current.result = done.child.pid as i64;
                                current.override_result = true;
                                registers.orig_rax = u64::MAX;
                            }
                            Err(ProcessError::NotChild) => {
                                current.result = -ECHILD;
                                current.override_result = true;
                                registers.orig_rax = u64::MAX;
                            }
                            Err(ProcessError::NoExitedChild) => {
                                if options & WNOHANG != 0 {
                                    current.result = 0;
                                    current.override_result = true;
                                    registers.orig_rax = u64::MAX;
                                } else {
                                    // Blocking wait with nothing reportable
                                    // yet: park the caller at this entry stop
                                    // until a matching child state change
                                    // wakes it. The host call never runs.
                                    current.wait_pending = true;
                                    current.wait_selector = selector;
                                    current.wait_options = options;
                                    current.wait_status = status_ptr;
                                    current.wait_rusage = rusage_ptr;
                                    registers.orig_rax = u64::MAX;
                                    tasks[task_index] = Some(current);
                                    if ptrace(
                                        PTRACE_SETREGS,
                                        host_pid,
                                        0,
                                        &registers as *const _ as usize,
                                    ) < 0
                                    {
                                        process_native_fail(
                                            domain,
                                            root_host_pid,
                                            b"wait park failed",
                                        );
                                    }
                                    write_all(b"GINUX_NATIVE_PROCESS_AUDIT: wait parked caller=");
                                    write_u64(current.virtual_id.pid as u64);
                                    write_all(b" selector=");
                                    write_i64(selector);
                                    write_all(b"\n");
                                    continue;
                                }
                            }
                            Err(_) => {
                                process_native_fail(
                                    domain,
                                    root_host_pid,
                                    b"wait selector invalid",
                                );
                            }
                        }
                    } else {
                        current.result = -ECHILD;
                        current.override_result = true;
                        registers.orig_rax = u64::MAX;
                    }
                }
                62 => {
                    // Managed kill: the existence probe (signal 0) and the
                    // normal termination signals are decided against the
                    // virtual table. Supported terminators execute on the
                    // host with the pid translated; the exit stop restores
                    // the virtual argument. Anything else fails explicitly
                    // and never executes, so no task escapes supervision.
                    let selector = registers.rdi as i32 as i64;
                    let signal = registers.rsi;
                    if selector <= 0 {
                        // Broadcast and group selectors have no managed
                        // delivery path yet.
                        current.result = -EINVAL;
                        current.override_result = true;
                        registers.orig_rax = u64::MAX;
                    } else {
                        let pid = selector as u32;
                        match processes.lookup(pid) {
                            None => {
                                current.result = -ESRCH;
                                current.override_result = true;
                                registers.orig_rax = u64::MAX;
                            }
                            Some(view)
                                if view.id.pid != view.tgid
                                    || (signal != 0
                                        && signal != SIGINT as u64
                                        && signal != SIGTERM as u64
                                        && signal != SIGKILL as u64) =>
                            {
                                // Signals to thread-group members would hit
                                // the whole host group, and only the normal
                                // termination signals have a managed meaning.
                                current.result = if view.id.pid != view.tgid {
                                    -EPERM
                                } else {
                                    -ENOSYS
                                };
                                current.override_result = true;
                                registers.orig_rax = u64::MAX;
                            }
                            Some(view) => {
                                let live = view.state == TaskState::Running;
                                if !live || signal == 0 {
                                    // Zombies and existence probes need no
                                    // host effect: the kernel reports success
                                    // without delivering anything.
                                    current.result = 0;
                                    current.override_result = true;
                                    registers.orig_rax = u64::MAX;
                                } else {
                                    let Some(host) = tasks
                                        .iter()
                                        .flatten()
                                        .find(|entry| entry.virtual_id.pid == pid)
                                        .map(|entry| entry.host_pid)
                                    else {
                                        process_native_fail(
                                            domain,
                                            root_host_pid,
                                            b"kill host pid unknown",
                                        );
                                    };
                                    current.translated_rdi = registers.rdi;
                                    registers.rdi = host as u64;
                                }
                            }
                        }
                    }
                }
                SYS_READ => {
                    // G05a managed read: pre-supervision stdio and tracked
                    // pipe ends execute on the host; anything else is not a
                    // domain object and fails closed.
                    let tracked = pipes
                        .lookup(current.virtual_id.pid, registers.rdi as u32)
                        .is_some();
                    if !tracked && !matches!(registers.rdi, 0..=2) {
                        current.result = -EBADF;
                        current.override_result = true;
                        registers.orig_rax = u64::MAX;
                    }
                }
                1 => {
                    // G05a managed write: the existing bounded console path
                    // is unchanged; tracked pipe ends execute regardless of
                    // length so shell pipelines move arbitrary bytes. Beyond
                    // that the old denial shape stays (ENOSYS for stdio with
                    // an overlong write, EBADF past it).
                    if !(matches!(registers.rdi, 1 | 2) && registers.rdx <= 4096
                        || pipes
                            .lookup(current.virtual_id.pid, registers.rdi as u32)
                            .is_some())
                    {
                        current.result = if matches!(registers.rdi, 0..=2) {
                            -ENOSYS
                        } else {
                            -EBADF
                        };
                        current.override_result = true;
                        registers.orig_rax = u64::MAX;
                    }
                }
                SYS_PIPE | SYS_DUP | SYS_DUP2 | SYS_CLOSE | SYS_FCNTL | SYS_DUP3 | SYS_PIPE2 => {
                    // G05a descriptor mediation: flag shapes the mirror
                    // cannot represent fail here before the host runs.
                    // Everything else executes on the host and the exit stop
                    // below mirrors the outcome.
                    let duplicates = matches!(
                        registers.orig_rax,
                        SYS_DUP | SYS_DUP2 | SYS_DUP3 | SYS_FCNTL
                    );
                    if duplicates
                        && registers.rdi > 2
                        && pipes
                            .lookup(current.virtual_id.pid, registers.rdi as u32)
                            .is_none()
                    {
                        current.result = -EBADF;
                        current.override_result = true;
                        registers.orig_rax = u64::MAX;
                    } else if (registers.orig_rax == SYS_PIPE2 && !pipe2_flags_valid(registers.rsi))
                        || (registers.orig_rax == SYS_DUP3 && !dup3_flags_valid(registers.rdx))
                    {
                        current.result = -EINVAL;
                        current.override_result = true;
                        registers.orig_rax = u64::MAX;
                    } else if registers.orig_rax == SYS_FCNTL && !fcntl_supported(registers.rsi) {
                        current.result = -ENOSYS;
                        current.override_result = true;
                        registers.orig_rax = u64::MAX;
                    } else {
                        current.desc_syscall = registers.orig_rax;
                        current.desc_arg0 = registers.rdi;
                        current.desc_arg1 = registers.rsi;
                        current.desc_arg2 = registers.rdx;
                    }
                }
                60 | 231 => {}
                _ => {
                    current.result = -ENOSYS;
                    current.override_result = true;
                    registers.orig_rax = u64::MAX;
                }
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

fn group_exec_pending(
    tasks: &[Option<NativeProcessTask>; 8],
    processes: &ProcessTable<8>,
    caller: ginux_process::TaskId,
) -> bool {
    let Ok(view) = processes.view(caller) else {
        return false;
    };
    tasks.iter().flatten().any(|other| {
        other.virtual_id != caller
            && other.exec_pending
            && processes
                .view(other.virtual_id)
                .is_ok_and(|other_view| other_view.tgid == view.tgid)
    })
}

/// Resolve a `wait4` pid selector against the virtual table. `None` means
/// the caller has no matching unwaited-for child, so the entry fails with
/// `-ECHILD`, matching the kernel even for blocking waits.
fn resolve_wait_target(
    selector: i64,
    processes: &ProcessTable<8>,
    caller: ginux_process::TaskId,
) -> Option<WaitTarget> {
    let target = if selector == -1 {
        WaitTarget::Any
    } else if selector == 0 {
        WaitTarget::Group(processes.view(caller).ok()?.process_group)
    } else if selector < -1 {
        WaitTarget::Group(selector.checked_neg()? as u32)
    } else {
        WaitTarget::Specific(selector as u32)
    };
    if processes.has_waitable_child(caller, target).ok()? {
        Some(target)
    } else {
        None
    }
}

/// Drop the supervisor's host-side slot for a virtually reaped child. The
/// host child was already reaped by the supervisor's own wait loop when its
/// outcome was recorded; the slot only lingered so exits could be attributed.
fn clear_host_slot(tasks: &mut [Option<NativeProcessTask>; 8], child: ginux_process::TaskId) {
    if let Some(position) = tasks
        .iter()
        .position(|entry| entry.is_some_and(|entry| entry.virtual_id == child))
    {
        tasks[position] = None;
    }
}

/// Complete one managed child state change for every parked `wait4` waiter
/// it satisfies, and queue virtual SIGCHLD for the parent. Called from the
/// supervisor's child-reaping path after the virtual outcome is recorded.
/// Each resumed waiter carries the real host SIGCHLD queued for this death,
/// so the parent's disposition observes the notification (ignored by
/// default); the virtual pending bit is bookkeeping for future disposition
/// mediation.
fn wake_waiters_for_child(
    domain: &mut Domain,
    root_host_pid: usize,
    processes: &mut ProcessTable<8>,
    tasks: &mut [Option<NativeProcessTask>; 8],
    dead: ginux_process::TaskId,
) {
    if let Ok(view) = processes.view(dead) {
        // The root task has no managed parent; only real children queue.
        if view.parent != 0 {
            if let Some(parent) = tasks
                .iter()
                .flatten()
                .find(|entry| entry.virtual_id.pid == view.parent)
                .map(|entry| entry.virtual_id)
            {
                let _ = processes.signal(parent, SIGCHLD as u8);
            }
            write_all(b"GINUX_NATIVE_PROCESS_AUDIT: sigchld queued parent=");
            write_u64(view.parent as u64);
            write_all(b" child=");
            write_u64(dead.pid as u64);
            write_all(b"\n");
        }
    }
    let mut index = 0;
    while index < tasks.len() {
        let waiter = tasks[index];
        let Some(entry) = waiter else {
            index += 1;
            continue;
        };
        if !entry.wait_pending || !entry.tracing {
            index += 1;
            continue;
        }
        // Another waiter may have consumed the last matching child during
        // this wakeup pass. That ends this wait with ECHILD; only a wait
        // with a remaining, non-reportable child should stay parked.
        let result = if let Some(target) =
            resolve_wait_target(entry.wait_selector, processes, entry.virtual_id)
        {
            let done = match processes.wait(entry.virtual_id, target, entry.wait_options) {
                Ok(done) => done,
                Err(ProcessError::NoExitedChild) => {
                    index += 1;
                    continue;
                }
                Err(_) => process_native_fail(domain, root_host_pid, b"wait wakeup failed"),
            };
            if entry.wait_status != 0
                && store_tracee_status(entry.host_pid, entry.wait_status, done.status.word())
                    .is_err()
            {
                process_native_fail(domain, root_host_pid, b"wait status store failed");
            }
            if entry.wait_rusage != 0
                && zero_tracee_rusage(entry.host_pid, entry.wait_rusage).is_err()
            {
                process_native_fail(domain, root_host_pid, b"wait rusage store failed");
            }
            clear_host_slot(tasks, done.child);
            write_all(b"GINUX_NATIVE_PROCESS_AUDIT: wait wakeup caller=");
            write_u64(entry.virtual_id.pid as u64);
            write_all(b" child=");
            write_u64(done.child.pid as u64);
            write_all(b" status=");
            write_i64(done.status.shell_code() as i64);
            write_all(b"\n");
            done.child.pid as i64
        } else {
            -ECHILD
        };
        if let Some(slot) = tasks[index].as_mut() {
            slot.wait_pending = false;
            slot.override_result = true;
            slot.result = result;
        }
        // The exit stop queued for this parked entry applies the pid result;
        // resume carrying the host SIGCHLD so delivery is observed.
        write_all(b"GINUX_NATIVE_PROCESS_AUDIT: sigchld delivered parent=");
        write_u64(entry.virtual_id.pid as u64);
        write_all(b"\n");
        if ptrace(PTRACE_SYSCALL, entry.host_pid, 0, SIGCHLD) < 0 {
            process_native_fail(domain, root_host_pid, b"wait wakeup resume failed");
        }
        index += 1;
    }
}

/// Read the two `int` fds the host `pipe`/`pipe2` just stored at the
/// tracee's `pipefd` pointer, without touching neighboring tracee memory.
/// Each byte is fetched with its own aligned `PTRACE_PEEKTEXT` read, so an
/// unaligned two-word array is assembled correctly. Any unreadable word
/// fails explicitly; the caller treats that as supervision failure, never
/// as invented descriptors.
fn read_tracee_pipefds(host_pid: usize, user_pipefd: usize) -> Result<(u32, u32), ()> {
    let mut bytes = [0u8; 8];
    let mut index = 0;
    while index < 8 {
        let address = user_pipefd.wrapping_add(index);
        let aligned = address & !0x7;
        let shift = (address & 0x7) * 8;
        let mut word = 0usize;
        if ptrace(
            PTRACE_PEEKTEXT,
            host_pid,
            aligned,
            &mut word as *mut _ as usize,
        ) < 0
        {
            return Err(());
        }
        bytes[index] = ((word >> shift) & 0xff) as u8;
        index += 1;
    }
    Ok((
        u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
        u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]),
    ))
}

/// Store the full Linux wait status word for the reaped virtual child into
/// the parent's `wait4` status slot, without touching neighboring tracee
/// memory. Exited children report the code in bits 8-15; signaled children
/// report the signal number in bits 0-6 with no core-dump bit.
fn store_tracee_status(host_pid: usize, user_status: usize, value: u32) -> Result<(), ()> {
    // A valid int pointer may straddle two ptrace words. Preserve every
    // neighboring byte and write all four bytes, including that second word.
    for (offset, byte) in value.to_le_bytes().iter().enumerate() {
        let address = user_status.checked_add(offset).ok_or(())?;
        let aligned = address & !0x7;
        let shift = (address & 0x7) * 8;
        let mut word = 0usize;
        if ptrace(
            PTRACE_PEEKTEXT,
            host_pid,
            aligned,
            &mut word as *mut _ as usize,
        ) < 0
        {
            return Err(());
        }
        let merged = (word & !(0xffusize << shift)) | ((*byte as usize) << shift);
        if ptrace(PTRACE_POKETEXT, host_pid, aligned, merged) < 0 {
            return Err(());
        }
    }
    Ok(())
}

/// Zero the caller's `rusage` buffer after a managed wait. The kernel would
/// fill resource usage here; without usage accounting the honest bounded
/// content is zeros. `rusage` is 8-byte aligned in practice; an unaligned
/// buffer fails explicitly rather than tearing neighbors.
fn zero_tracee_rusage(host_pid: usize, user_rusage: usize) -> Result<(), ()> {
    const RUSAGE_BYTES: usize = 144;
    let mut offset = 0;
    while offset < RUSAGE_BYTES {
        let address = user_rusage + offset;
        if address & 0x7 != 0 {
            return Err(());
        }
        if ptrace(PTRACE_POKETEXT, host_pid, address, 0) < 0 {
            return Err(());
        }
        offset += 8;
    }
    Ok(())
}

/// Publish the virtual tid into the tracee's parent-tid cell. The host
/// kernel stored the host tid there during `clone` with
/// `CLONE_PARENT_SETTID`; thread identity requires the parent to observe the
/// virtual tid instead. Same aligned-word technique as
/// [`store_tracee_status`], but for one 4-byte tid cell.
fn store_tracee_tid(host_pid: usize, user_tid: usize, tid: u32) -> Result<(), ()> {
    store_tracee_status(host_pid, user_tid, tid)
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
    supervise_native(child as usize, &mut domain)
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
    let argv = [
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
            BASH.as_ptr() as usize,
            argv.as_ptr() as usize,
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

fn supervise_native(pid: usize, domain: &mut Domain) -> ! {
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
    let options = PTRACE_O_TRACESYSGOOD | PTRACE_O_EXITKILL;
    if ptrace(PTRACE_SETOPTIONS, pid, 0, options) < 0 || ptrace(PTRACE_SYSCALL, pid, 0, 0) < 0 {
        native_fail(domain, pid, b"native ptrace setup failed");
    }
    line(b"GINUX_NATIVE: backend=ptrace-native profile=balanced elf-loader=host contract=gate5-balanced");
    line(b"GINUX_NATIVE: tracee-uid=1000 tracee-gid=1000 root=oci-read-only");

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
        let decision = match decide_native(syscall, registers.rdi, registers.rsi, registers.rdx) {
            NativeDecision::Contract(decision) => decision,
            NativeDecision::Hooked { lane, result } => {
                let lane_name: &[u8] = match lane {
                    Lane::Process => b"process",
                    Lane::Filesystem => b"filesystem",
                    Lane::Channel => b"channel",
                    Lane::Contract => b"contract",
                };
                write_all(b"emulate surface=lane-");
                write_all(lane_name);
                write_all(b" result=");
                write_i64(result);
                write_all(b"\n");
                registers.orig_rax = u64::MAX;
                awaiting_exit = true;
                override_exit = true;
                override_result = result;
                if ptrace(PTRACE_SETREGS, pid, 0, &registers as *const _ as usize) < 0
                    || ptrace(PTRACE_SYSCALL, pid, 0, 0) < 0
                {
                    native_fail(domain, pid, b"native lane emulation failed");
                }
                continue;
            }
        };
        write_all(b"GINUX_NATIVE_AUDIT: sequence=");
        write_u64(sequence);
        write_all(b" phase=entry syscall=");
        write_u64(syscall);
        write_all(b" action=");
        match decision.action {
            Action::Emulate if balanced_identity_result(syscall).is_some() => {
                let result = balanced_identity_result(syscall).unwrap_or(0);
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
            Action::Mediate if native_arguments_allowed(syscall, &registers) => {
                write_all(b"mediate surface=");
                line(balanced_surface_name(balanced_surface(syscall)));
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

fn fail(reason: &[u8]) -> ! {
    write_all(b"GATE1: FAIL: ");
    line(reason);
    exit_process(1)
}
