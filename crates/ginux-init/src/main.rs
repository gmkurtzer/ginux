#![no_std]
#![no_main]

use core::arch::global_asm;
use core::cell::Cell;
use core::panic::PanicInfo;
use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use ginux_contract::{Action as ContractAction, AuditBudget, AuditDisposition, EffectiveContract};
use ginux_core::{
    balanced_arguments_allowed, balanced_identity_result, balanced_mediated_result,
    balanced_surface, decide, decide_systemd, systemd_surface, Action, SYS_EXIT, SYS_EXIT_GROUP,
    SYS_GETPID, SYS_UNAME, SYS_WRITE,
};
use ginux_instance::{AuditMode, HookFailurePolicy};

mod channel_hooks;
mod dispatch;
mod fs_hooks;
mod launch;
mod native_clone;
mod process_hooks;
mod supervisor;

use crate::dispatch::{balanced_surface_name, decide_native, process_entry, Lane, NativeDecision};
use crate::launch::{
    c_string_bytes, c_string_equals, config_rejected, launch_main, launch_usage, load_config,
    parse_argv, print_effective_config, read_file, read_index, usage, Command, ELF_BYTES,
    VFS_INDEX_BYTES,
};
use crate::process_hooks::{
    dup3_flags_valid, exec_mediated, fcntl_supported, pipe2_flags_valid, thread_supported,
    FD_CLOEXEC, F_DUPFD, F_DUPFD_CLOEXEC, F_GETFD, F_SETFD, O_CLOEXEC, PTRACE_EVENT_CLONE,
    PTRACE_EVENT_VFORK, PTRACE_O_TRACECLONE, PTRACE_O_TRACEVFORK, SYS_ARCH_PRCTL, SYS_CLOSE,
    SYS_DUP, SYS_DUP2, SYS_DUP3, SYS_FCNTL, SYS_FUTEX, SYS_GETTID, SYS_GET_ROBUST_LIST, SYS_PIPE,
    SYS_PIPE2, SYS_READ, SYS_RSEQ, SYS_SET_ROBUST_LIST, SYS_SET_TID_ADDRESS,
};
use crate::supervisor::{
    exit_process, install_trace_filter, line, managed_host_identity, ptrace, read_tracee_word,
    standard_stream_refusal, stopped, syscall0, syscall1, syscall2, syscall3, syscall4, syscall5,
    syscall6, virtual_sysinfo, wait, write_all, write_i64, write_u64, UserRegs, AT_FDCWD,
    CLOCK_MONOTONIC, CLOCK_PROCESS_CPUTIME_ID, EBADF, ECHILD, EINVAL, EMFILE, ENOSYS, EPERM, ESRCH,
    O_RDONLY, O_RDWR, PR_SET_NO_NEW_PRIVS, PTRACE_CONT, PTRACE_EVENT_EXEC, PTRACE_EVENT_FORK,
    PTRACE_EVENT_SECCOMP, PTRACE_GETEVENTMSG, PTRACE_GETREGS, PTRACE_KILL, PTRACE_O_EXITKILL,
    PTRACE_O_TRACEEXEC, PTRACE_O_TRACEFORK, PTRACE_O_TRACESECCOMP, PTRACE_O_TRACESYSGOOD,
    PTRACE_PEEKTEXT, PTRACE_POKETEXT, PTRACE_SETOPTIONS, PTRACE_SETREGS, PTRACE_SYSCALL,
    PTRACE_SYSEMU, PTRACE_TRACEME, SIGCHLD, SIGCONT, SIGINT, SIGKILL, SIGSTOP, SIGTERM, SIGTSTP,
    SIG_BLOCK, SYS_CHDIR, SYS_CHROOT, SYS_CLOCK_GETTIME, SYS_CLOSE_HOST, SYS_EXECVE, SYS_EXIT_HOST,
    SYS_FORK, SYS_FSTAT, SYS_FSYNC, SYS_KILL, SYS_OPENAT_HOST, SYS_PAUSE, SYS_PIDFD_GETFD,
    SYS_PIDFD_OPEN, SYS_PRCTL, SYS_PREAD64, SYS_PTRACE, SYS_PWRITE64, SYS_READ_HOST,
    SYS_RT_SIGPROCMASK, SYS_SETGID, SYS_SETGROUPS, SYS_SETUID, SYS_SIGNALFD4, SYS_WAIT4,
    SYS_WRITE_HOST, VIRTUAL_PID,
};
use crate::supervisor::{fault_errno, write_tracee_bytes as write_tracee_bytes_result, EFAULT};
use ginux_devices::{
    BlockBackend, DeviceError, DeviceKind, DeviceName, DeviceRegistry, DeviceView, IsolatedNetwork,
    Partition, IOCTL_CONSOLE_GET_SIZE,
};
use ginux_instance::{
    DispatchMode, ExecutionBackend, ExecutionProfile, Instance, InstanceConfig, NetworkBackend,
    NetworkProtocol,
};
use ginux_kernel_views::{
    CgroupFs, IdentityFs, IdentityView, MemoryView, MountView, NamespaceView, ProcFs, SysFs,
    ViewError,
};
use ginux_process::{
    ChildStatus, Credentials, EventFd, EventPoll, FdKind, FutexTable, PipeSpace, ProcessError,
    ProcessTable, SignalAction, SignalDelivery, TaskId, TaskState, TaskView, TimerTable,
    VirtualClock, WaitTarget, EVENT_READ, EVENT_WRITE, WAIT_ALLOWED_OPTIONS, WNOHANG,
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

const SYS_DUP_HOST: usize = 32;
const SYS_FCNTL_HOST: usize = 72;
const SYS_SCHED_SETAFFINITY_HOST: usize = 203;
#[allow(dead_code)]
const SYS_GETDENTS64_HOST: usize = 217;
#[allow(dead_code)]
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
const CLOCK_REALTIME: usize = 0;
const GINUX_ABI_RELEASE: &[u8] = b"6.12.0-ginux-abi1";
const GINUX_VERSION: &str = concat!("Ginux userspace kernel ", env!("CARGO_PKG_VERSION"));
const PTRACE_POKEDATA: usize = 5;
const PTRACE_PEEKDATA: usize = 2;
const SYS_IOCTL_HOST: usize = 16;
const TIOCSCTTY: u64 = 0x540e;
const TIOCGPGRP: u64 = 0x540f;
const TIOCSPGRP: u64 = 0x5410;
const SYS_MMAP_HOST: usize = 9;
const SYS_FUTEX_HOST: usize = 202;
const SYS_CLOSE_RANGE_HOST: usize = 436;
const FUTEX_WAIT: usize = 0;
const FUTEX_WAKE: usize = 1;
const PR_SET_PDEATHSIG: usize = 1;
const SECCOMP_IOCTL_NOTIF_RECV: usize = 0xc050_2100;
const SECCOMP_IOCTL_NOTIF_SEND: usize = 0xc018_2101;
const PROT_READ_WRITE: usize = 1 | 2;
const MAP_SHARED_ANONYMOUS: usize = 1 | 0x20;
const INSTANCE_NULL_FD: usize = 240;
const INSTANCE_CONSOLE_FD: usize = 241;
const NATIVE_PROC_FD_LIMIT: usize = 512;
const NATIVE_SOCKET_FD_LIMIT: usize = 64;
const NATIVE_VIRTUAL_KERNEL_FD_LIMIT: usize = 256;
const NATIVE_VIRTUAL_DIRECTORY_LIMIT: usize = 128;
const VIRTUAL_KERNEL_FD_BASE: i64 = 10_000;
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
unsafe extern "C" fn memmove(
    destination: *mut core::ffi::c_void,
    source: *const core::ffi::c_void,
    length: usize,
) -> *mut core::ffi::c_void {
    let destination_bytes = destination.cast::<u8>();
    let source_bytes = source.cast::<u8>();
    if (destination_bytes as usize) <= (source_bytes as usize) {
        let mut index = 0usize;
        while index < length {
            core::ptr::write_volatile(
                destination_bytes.add(index),
                core::ptr::read_volatile(source_bytes.add(index)),
            );
            index += 1;
        }
    } else {
        let mut index = length;
        while index != 0 {
            index -= 1;
            core::ptr::write_volatile(
                destination_bytes.add(index),
                core::ptr::read_volatile(source_bytes.add(index)),
            );
        }
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
        Some(Command::ConfigMerge(config_path, count, overrides)) => {
            config_merge_main(config_path, count, overrides)
        }
        Some(Command::InstancePlan(config_path)) => instance_plan_main(config_path),
        Some(Command::InstanceValidate(config_path)) => instance_validate_main(config_path),
        Some(Command::InstanceExplain(config_path, operation)) => {
            instance_explain_main(config_path, operation)
        }
        Some(Command::InstanceDiff(left, right)) => instance_diff_main(left, right),
        Some(Command::InstanceInspect(config_path)) => instance_inspect_main(config_path),
        Some(Command::HookPlan(config_path)) => hook_plan_main(config_path),
        Some(Command::Lifecycle(config_path)) => lifecycle_main(config_path),
        Some(Command::Vfs(config_path, rootfs_path, index_path)) => {
            vfs_main(config_path, rootfs_path, index_path)
        }
        Some(Command::Native(config_path, root_path, mode)) => {
            native_main(config_path, root_path, mode, None)
        }
        Some(Command::NativeHook(config_path, root_path, index)) => {
            native_main(config_path, root_path, 3, Some(index))
        }
        Some(Command::Instance(config_path, fixture_path)) => {
            instance_main(config_path, fixture_path)
        }
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
        Some(Command::Usage) => usage(),
        None => gate1_main(),
    }
}

fn kernel_views_main(config_path: *const u8) -> ! {
    let config = load_config(config_path);
    let mut instance = Instance::create(config);
    if instance.start().is_err() {
        kernel_views_fail(b"instance start failed");
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
        hostname: b"gate8-instance",
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
    line(b"GINUX_VIEWS: identity hostname=gate8-instance namespaces=virtual host-id=absent");

    if processes.shutdown(0) != 2
        || processes.reap_all() != 2
        || instance.task_exited(true).is_err()
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
    let mut instance = Instance::create(config);
    if instance.start().is_err() {
        process_fail(b"instance start failed");
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
    if instance.task_exited(true).is_err() {
        process_fail(b"instance cleanup failed");
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
    /// A termination signal interrupted this task's `wait4` (parked or about
    /// to park): the skipped call's exit stop rewinds it to its `syscall`
    /// instruction, so the pending signal is delivered first and an ignored
    /// one leaves the wait blocking, as on Linux. Always restarting is right
    /// only because no handler can exist (`rt_sigaction` is `-ENOSYS` in this
    /// lane); with a handler Linux returns `-EINTR` unless `SA_RESTART` is set.
    wait_restart: bool,
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
    /// The host killed this task while it sat in a reported, unresumed stop
    /// (see [`tracee_killed`]); only its death report may follow.
    killed: bool,
}

fn process_native_main(config_path: *const u8, fixture_path: *const u8) -> ! {
    let config = load_config(config_path);
    let mut instance = Instance::create(config);
    if instance.start().is_err() {
        process_fail(b"native process instance start failed");
    }
    let child = unsafe { syscall0(SYS_FORK) };
    if child < 0 {
        process_fail(b"native process tracee fork failed");
    }
    if child == 0 {
        process_native_tracee(fixture_path);
    }
    supervise_process_native(child as usize, &mut instance)
}

fn process_native_tracee(fixture_path: *const u8) -> ! {
    const SYS_CLOSE_RANGE: usize = 436;
    const SYS_RT_SIGACTION: usize = 13;
    const SYS_SETRLIMIT: usize = 160;
    const RLIMIT_CORE: usize = 4;
    const SIGPIPE: usize = 13;
    if unsafe { syscall3(SYS_CLOSE_RANGE, 3, u32::MAX as usize, 0) } < 0 {
        native_tracee_setup_fail(132);
    }
    // Forwarded core-dumping signals must not make the host write a core
    // of an instance task; the tracee cannot raise the limit (setrlimit and
    // prlimit64 are `-ENOSYS` in this lane). SIGPIPE starts at its default
    // action whatever the launcher ignored, as exec keeps `SIG_IGN`.
    let no_core = [0usize; 2];
    let default_action = [0usize; 4];
    if unsafe { syscall2(SYS_SETRLIMIT, RLIMIT_CORE, no_core.as_ptr() as usize) } < 0
        || unsafe {
            syscall4(
                SYS_RT_SIGACTION,
                SIGPIPE,
                default_action.as_ptr() as usize,
                0,
                8,
            )
        } < 0
        || unsafe { syscall4(SYS_PTRACE, PTRACE_TRACEME, 0, 0, 0) } < 0
    {
        native_tracee_setup_fail(130);
    }
    let host_identity = managed_host_identity();
    if host_identity.drop_root
        && (unsafe { syscall2(SYS_SETGROUPS, 0, 0) } < 0
            || unsafe { syscall1(SYS_SETGID, host_identity.gid) } < 0
            || unsafe { syscall1(SYS_SETUID, host_identity.uid) } < 0)
    {
        native_tracee_setup_fail(133);
    }
    if unsafe { syscall5(SYS_PRCTL, PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) } < 0 {
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

fn supervise_process_native(root_host_pid: usize, instance: &mut Instance) -> ! {
    let mut status = 0i32;
    if wait(root_host_pid, &mut status) < 0 || !stopped(status) {
        write_all(b"GINUX_NATIVE_PROCESS: initial-status=");
        write_i64(status as i64);
        write_all(b"\n");
        native_fail(
            instance,
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
        native_fail(instance, root_host_pid, b"process ptrace options failed");
    }
    let mut processes: ProcessTable<8> = ProcessTable::new();
    // Descriptor mirror: pipe pairs, dups, and close-on-exec ends as
    // supervisor-owned instance objects, keyed by virtual pid. Every mutation
    // below follows a host outcome the loop observed.
    let mut pipes: PipeSpace<8, 16, 16> = PipeSpace::new();
    let root_virtual = processes
        .create_init(Credentials::unprivileged(1000, 1000))
        .unwrap_or_else(|_| process_native_fail(instance, root_host_pid, b"virtual init failed"));
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
        wait_restart: false,
        killed: false,
    });
    let mut root_exited = false;
    let mut root_failed = false;
    if ptrace(PTRACE_SYSCALL, root_host_pid, 0, 0) < 0 {
        process_native_fail(instance, root_host_pid, b"process initial resume failed");
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
            process_native_fail(instance, root_host_pid, b"process wait-any failed");
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
            process_native_fail(instance, root_host_pid, b"exec former tid read failed");
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
                    process_native_fail(instance, root_host_pid, b"newborn stop table full");
                };
                *slot = Some((host_pid, status));
                line(b"GINUX_NATIVE_PROCESS_AUDIT: newborn-stop=deferred");
                continue;
            }
            process_native_fail(instance, root_host_pid, b"unknown host tracee");
        };
        let task = tasks[task_index].unwrap();
        if task.killed && stopped(status) {
            process_native_fail(instance, root_host_pid, b"killed process stopped again");
        }
        // A group exit (a fatal signal or exit_group in a sibling thread) or
        // a sibling's exec SIGKILLs this task even while it sits in the stop
        // handled below, and every later ptrace request on it then fails.
        // Such a failure is no supervision loss: the killed task cannot run
        // workload code or its pending syscall, and its death report follows.
        // Mark it and wait for that report ([`killed_or_fail`] followed by
        // `continue`); any other failure fails closed.
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
            write_all(b"GINUX_NATIVE_TASK: virtual-pid=");
            write_u64(task.virtual_id.pid as u64);
            write_all(b" exit-status=");
            write_i64(exit_status as i64);
            write_all(b"\n");
            let recorded = match child_status {
                ChildStatus::Exited(code) => processes.exit(task.virtual_id, code),
                ChildStatus::Signaled(signal) => processes.signaled(task.virtual_id, signal),
            };
            if recorded.is_err() {
                process_native_fail(instance, root_host_pid, b"virtual exit failed");
            }
            // The host closed every descriptor with the task. Release
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
                    process_native_fail(instance, root_host_pid, b"thread reap failed")
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
                    instance,
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
                process_native_fail(instance, root_host_pid, b"native process fixture failed");
            }
            if root_exited && !tasks.iter().flatten().any(|entry| entry.tracing) {
                // Descriptor leak audit: every end died with its task (last close
                // is the host's), so no pipe object may survive full reap.
                write_all(b"GINUX_NATIVE_PROCESS_AUDIT: descriptors live-pipes=");
                write_u64(pipes.active_pipes() as u64);
                write_all(b"\n");
                if processes.reap_all() == 0 || processes.active() != 0 {
                    process_native_fail(instance, root_host_pid, b"native process cleanup failed");
                }
                if instance.task_exited(true).is_err() {
                    process_native_fail(instance, root_host_pid, b"native instance exit failed");
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
                process_native_fail(instance, root_host_pid, b"fork event read failed");
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
                    process_native_fail(instance, root_host_pid, b"virtual thread failed")
                })
            } else {
                processes.fork(task.virtual_id).unwrap_or_else(|_| {
                    process_native_fail(instance, root_host_pid, b"virtual fork failed")
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
                process_native_fail(instance, root_host_pid, b"descriptor inherit failed");
            }
            let Some(child_slot) = tasks.iter().position(Option::is_none) else {
                process_native_fail(instance, root_host_pid, b"host task table full");
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
                wait_restart: false,
                killed: false,
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
                    killed_or_fail(
                        &mut tasks,
                        task_index,
                        instance,
                        root_host_pid,
                        b"fork breakpoint registers failed",
                    );
                    continue;
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
                    killed_or_fail(
                        &mut tasks,
                        task_index,
                        instance,
                        root_host_pid,
                        b"fork breakpoint read failed",
                    );
                    continue;
                }
                if ptrace(
                    PTRACE_POKETEXT,
                    host_pid,
                    breakpoint_rip,
                    (breakpoint_word & !0xff) | 0xcc,
                ) < 0
                {
                    killed_or_fail(
                        &mut tasks,
                        task_index,
                        instance,
                        root_host_pid,
                        b"fork breakpoint install failed",
                    );
                    continue;
                }
                let parent = tasks[task_index].as_mut().unwrap();
                parent.awaiting_exit = false;
                parent.override_result = false;
                parent.fork_result_pending = child_virtual.pid;
                parent.fork_breakpoint_rip = breakpoint_rip;
                parent.fork_breakpoint_word = breakpoint_word;
                parent.fork_variant_event = event as u32;
                if ptrace(PTRACE_CONT, host_pid, 0, 0) < 0 {
                    killed_or_fail(
                        &mut tasks,
                        task_index,
                        instance,
                        root_host_pid,
                        b"fork parent breakpoint resume failed",
                    );
                    continue;
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
                killed_or_fail(
                    &mut tasks,
                    task_index,
                    instance,
                    root_host_pid,
                    b"fork parent resume failed",
                );
                continue;
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
                process_native_fail(instance, root_host_pid, b"unexpected exec event");
            }
            let group = processes
                .view(task.virtual_id)
                .unwrap_or_else(|_| {
                    process_native_fail(instance, root_host_pid, b"exec tgid lookup failed")
                })
                .tgid;
            pipes.unshare(task.virtual_id.pid).unwrap_or_else(|_| {
                process_native_fail(instance, root_host_pid, b"exec descriptor unshare failed")
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
                process_native_fail(instance, root_host_pid, b"virtual exec failed")
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
                killed_or_fail(
                    &mut tasks,
                    task_index,
                    instance,
                    root_host_pid,
                    b"exec event resume failed",
                );
                continue;
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
                killed_or_fail(
                    &mut tasks,
                    task_index,
                    instance,
                    root_host_pid,
                    b"fork result read failed",
                );
                continue;
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
                killed_or_fail(
                    &mut tasks,
                    task_index,
                    instance,
                    root_host_pid,
                    b"fork breakpoint restore failed",
                );
                continue;
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
                killed_or_fail(
                    &mut tasks,
                    task_index,
                    instance,
                    root_host_pid,
                    b"clone parent tid store failed",
                );
                continue;
            }
            if ptrace(
                PTRACE_SETREGS,
                host_pid,
                0,
                &parent_registers as *const _ as usize,
            ) < 0
            {
                killed_or_fail(
                    &mut tasks,
                    task_index,
                    instance,
                    root_host_pid,
                    b"fork result translation failed",
                );
                continue;
            }
            let parent = tasks[task_index].as_mut().unwrap();
            parent.fork_result_pending = 0;
            parent.fork_breakpoint_rip = 0;
            parent.fork_breakpoint_word = 0;
            parent.fork_variant_event = 0;
            if ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0 {
                killed_or_fail(
                    &mut tasks,
                    task_index,
                    instance,
                    root_host_pid,
                    b"fork parent resume failed",
                );
                continue;
            }
            continue;
        }
        if signal == 19 {
            if task.child_tid != 0 {
                if store_tracee_tid(host_pid, task.child_tid, task.virtual_id.pid).is_err() {
                    killed_or_fail(
                        &mut tasks,
                        task_index,
                        instance,
                        root_host_pid,
                        b"child tid store failed",
                    );
                    continue;
                }
                tasks[task_index].as_mut().unwrap().child_tid = 0;
            }
            let _ = ptrace(PTRACE_SETOPTIONS, host_pid, 0, options);
            if ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0 {
                killed_or_fail(
                    &mut tasks,
                    task_index,
                    instance,
                    root_host_pid,
                    b"fork child resume failed",
                );
                continue;
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
                killed_or_fail(
                    &mut tasks,
                    task_index,
                    instance,
                    root_host_pid,
                    b"SIGCHLD mediation failed",
                );
                continue;
            }
            continue;
        }
        // A forwarded termination signal first arrives as a delivery stop
        // for the task: forward normal termination signals, including
        // self-signals and delivery to a sibling in the target group, and
        // the kernel-generated fatal signals (SIGPIPE, SIGSEGV, ...). No
        // handler can exist, so the default action kills the task and its
        // death reaches the virtual table through the exit path above.
        if fatal_default_signal(signal) {
            write_all(b"GINUX_NATIVE_PROCESS_AUDIT: terminating signal=");
            write_u64(signal as u64);
            write_all(b" virtual-pid=");
            write_u64(task.virtual_id.pid as u64);
            write_all(b"\n");
            if ptrace(PTRACE_SYSCALL, host_pid, 0, signal as usize) < 0 {
                killed_or_fail(
                    &mut tasks,
                    task_index,
                    instance,
                    root_host_pid,
                    b"terminating signal resume failed",
                );
                continue;
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
            process_native_fail(instance, root_host_pid, b"unexpected process trace stop");
        }
        let mut registers = core::mem::MaybeUninit::<UserRegs>::uninit();
        if ptrace(PTRACE_GETREGS, host_pid, 0, registers.as_mut_ptr() as usize) < 0 {
            killed_or_fail(
                &mut tasks,
                task_index,
                instance,
                root_host_pid,
                b"process registers read failed",
            );
            continue;
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
                // Descriptor exit: the host executed, so the tracee
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
                                            instance,
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
                                    killed_or_fail(
                                        &mut tasks,
                                        task_index,
                                        instance,
                                        root_host_pid,
                                        b"pipe result read failed",
                                    );
                                    continue;
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
                                process_native_fail(instance, root_host_pid, b"dup mirror failed");
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
                                        instance,
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
                                process_native_fail(
                                    instance,
                                    root_host_pid,
                                    b"fcntl mirror failed",
                                );
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
                        match store_tracee_tid(host_pid, current.clone_ptid, virtual_pid) {
                            Err(()) => {
                                killed_or_fail(
                                    &mut tasks,
                                    task_index,
                                    instance,
                                    root_host_pid,
                                    b"thread tid store failed",
                                );
                                continue;
                            }
                            Ok(true) => {
                                write_all(b"GINUX_NATIVE_PROCESS_AUDIT: thread-tid virtual-tid=");
                                write_u64(virtual_pid as u64);
                                write_all(b"\n");
                            }
                            Ok(false) => {}
                        }
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
                        process_native_fail(instance, root_host_pid, b"exec event without success");
                    }
                    write_all(b"GINUX_NATIVE_PROCESS_AUDIT: exec-complete virtual-pid=");
                    write_u64(current.virtual_id.pid as u64);
                    write_all(b"\n");
                } else if registers.rax as i64 == 0 {
                    process_native_fail(
                        instance,
                        root_host_pid,
                        b"exec completed without an event",
                    );
                } else {
                    write_all(b"GINUX_NATIVE_PROCESS_AUDIT: exec-failed errno=");
                    write_i64(-(registers.rax as i64));
                    write_all(b" virtual-pid=");
                    write_u64(current.virtual_id.pid as u64);
                    write_all(b"\n");
                }
                current.exec_pending = false;
                current.exec_event_seen = false;
            } else if current.wait_restart {
                // Interrupted wait: back over the two-byte `syscall` with
                // the number restored; the arguments stayed untouched.
                registers.rip -= 2;
                registers.rax = current.syscall;
            } else if current.override_result {
                registers.rax = current.result as u64;
            }
            if restore_translated_argument {
                registers.rdi = current.translated_rdi;
            }
            if (current.override_result || current.wait_restart || restore_translated_argument)
                && ptrace(PTRACE_SETREGS, host_pid, 0, &registers as *const _ as usize) < 0
            {
                killed_or_fail(
                    &mut tasks,
                    task_index,
                    instance,
                    root_host_pid,
                    b"process result translation failed",
                );
                continue;
            }
            // A SIGINT or SIGTERM the host accepted stays pending in a waiter
            // of the target group parked at its entry stop; resume those.
            if current.syscall == 62
                && restore_translated_argument
                && registers.rax == 0
                && (registers.rsi == SIGINT as u64 || registers.rsi == SIGTERM as u64)
            {
                interrupt_parked_waiters(
                    instance,
                    root_host_pid,
                    &processes,
                    &mut tasks,
                    current.translated_rdi as u32,
                );
            }
            current.awaiting_exit = false;
            current.override_result = false;
            current.wait_restart = false;
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
                killed_or_fail(
                    &mut tasks,
                    task_index,
                    instance,
                    root_host_pid,
                    b"process exit resume failed",
                );
                continue;
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
            killed_or_fail(
                &mut tasks,
                task_index,
                instance,
                root_host_pid,
                b"process requires the x86-64 syscall ABI",
            );
            continue;
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
                        process_native_fail(instance, root_host_pid, b"identity lookup failed")
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
                                // The virtual child is reaped: drop its host
                                // slot before the stores, which a killed
                                // caller may skip. As on Linux, the child
                                // stays reaped when a store faults.
                                clear_host_slot(&mut tasks, done.child);
                                match store_wait_results(
                                    host_pid,
                                    status_ptr,
                                    rusage_ptr,
                                    done.status.word(),
                                ) {
                                    Ok(()) => {
                                        write_all(b"GINUX_NATIVE_PROCESS_AUDIT: wait selector=");
                                        write_i64(selector);
                                        write_all(b" virtual-result=");
                                        write_u64(done.child.pid as u64);
                                        write_all(b" status=");
                                        write_i64(done.status.shell_code() as i64);
                                        write_all(b"\n");
                                        current.result = done.child.pid as i64;
                                    }
                                    Err(error) if error == -EFAULT => {
                                        write_wait_fault(current.virtual_id.pid, done.child.pid);
                                        current.result = -EFAULT;
                                    }
                                    Err(_) => {
                                        killed_or_fail(
                                            &mut tasks,
                                            task_index,
                                            instance,
                                            root_host_pid,
                                            b"wait result store failed",
                                        );
                                        continue;
                                    }
                                }
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
                                } else if termination_pending(host_pid) {
                                    // A termination signal is already queued:
                                    // skip and restart the call instead of
                                    // parking, so the signal is delivered.
                                    current.wait_restart = true;
                                    registers.orig_rax = u64::MAX;
                                    write_all(
                                        b"GINUX_NATIVE_PROCESS_AUDIT: wait interrupted caller=",
                                    );
                                    write_u64(current.virtual_id.pid as u64);
                                    write_all(b"\n");
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
                                        killed_or_fail(
                                            &mut tasks,
                                            task_index,
                                            instance,
                                            root_host_pid,
                                            b"wait park failed",
                                        );
                                        continue;
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
                                    instance,
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
                                if !(0..=64).contains(&(signal as i32))
                                    || view.id.pid != view.tgid
                                    || (signal != 0
                                        && signal != SIGINT as u64
                                        && signal != SIGTERM as u64
                                        && signal != SIGKILL as u64) =>
                            {
                                // Numbers outside the signal range are invalid
                                // arguments for any existing target, as on
                                // Linux. Signals to thread-group members would
                                // hit the whole host group, and only the normal
                                // termination signals have a managed meaning.
                                current.result = if !(0..=64).contains(&(signal as i32)) {
                                    -EINVAL
                                } else if view.id.pid != view.tgid {
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
                                            instance,
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
                    // Managed read: pre-supervision stdio and tracked
                    // pipe ends execute on the host; anything else is not a
                    // instance object and fails closed.
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
                    // Managed write: the existing bounded console path
                    // is unchanged; tracked pipe ends execute regardless of
                    // length so shell pipelines move arbitrary bytes. A stdio
                    // alias (dup, dup2, dup3, F_DUPFD) follows the rule of the
                    // stdio it names, so it cannot widen it. Beyond that the
                    // old denial shape stays (ENOSYS for stdio with an
                    // overlong write, EBADF past it).
                    let stdio = match pipes.lookup(current.virtual_id.pid, registers.rdi as u32) {
                        Some(entry) if entry.kind == FdKind::External => Some(entry.origin as u64),
                        Some(_) => None,
                        None => Some(registers.rdi),
                    };
                    let denied = match stdio {
                        Some(1 | 2) if registers.rdx <= 4096 => None,
                        Some(0..=2) => Some(-ENOSYS),
                        Some(_) => Some(-EBADF),
                        None => None,
                    };
                    if let Some(result) = denied {
                        current.result = result;
                        current.override_result = true;
                        registers.orig_rax = u64::MAX;
                    }
                }
                SYS_PIPE | SYS_DUP | SYS_DUP2 | SYS_CLOSE | SYS_FCNTL | SYS_DUP3 | SYS_PIPE2 => {
                    // Descriptor mediation: flag shapes the mirror
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
            killed_or_fail(
                &mut tasks,
                task_index,
                instance,
                root_host_pid,
                b"process syscall dispatch failed",
            );
            continue;
        }
    }
}

/// Signals whose default action terminates the task (with or without a
/// core dump). Forwarding a delivery stop for one runs no workload code
/// only because `rt_sigaction` stays `-ENOSYS` in this lane and exec resets
/// inherited handlers: no handler can exist, so the task dies (or, under an
/// inherited `SIG_IGN`, carries on as on Linux). Mediating `rt_sigaction`
/// must revisit this. SIGKILL never stops; SIGCHLD and SIGSTOP have their
/// own paths. SIGCONT, SIGTSTP, SIGTTIN, SIGTTOU, SIGURG and SIGWINCH are not
/// forwarded and still fail the instance, although Linux stops the task or
/// ignores them by default. SIGTRAP stays out: a plain trap stop may be the supervisor's
/// fork-result breakpoint hit by a sibling thread of the forking parent,
/// which must fail the instance instead of passing as a workload crash.
fn fatal_default_signal(signal: u8) -> bool {
    matches!(signal, 1..=4 | 6..=8 | 10..=16 | 24..=27 | 29..=64)
}

/// Whether a tracee that was in a reported, not yet resumed ptrace stop has
/// been killed out of it. Only SIGKILL ends such a stop without a resume
/// request, and afterwards every ptrace request on the task fails with
/// `ESRCH`: it is no longer stopped (or is already a zombie). A task still in
/// its stop answers the probe, so a genuine request failure stays fatal.
fn tracee_killed(host_pid: usize) -> bool {
    let mut message = 0usize;
    ptrace(
        PTRACE_GETEVENTMSG,
        host_pid,
        0,
        &mut message as *mut _ as usize,
    ) == -ESRCH as isize
}

/// Record a [`tracee_killed`] task so the loop awaits its death report.
fn mark_killed(tasks: &mut [Option<NativeProcessTask>; 8], index: usize) -> bool {
    let Some(task) = tasks[index].as_mut() else {
        return false;
    };
    task.killed = tracee_killed(task.host_pid);
    if task.killed {
        write_all(b"GINUX_NATIVE_PROCESS_AUDIT: killed-in-stop virtual-pid=");
        write_u64(task.virtual_id.pid as u64);
        write_all(b"\n");
    }
    task.killed
}

/// Fail the instance for a ptrace request on the handled stop of task `index`
/// unless [`mark_killed`] finds the task killed out of that stop. Returns only
/// for such a killed task; the caller then goes back to waiting for its death
/// report.
fn killed_or_fail(
    tasks: &mut [Option<NativeProcessTask>; 8],
    index: usize,
    instance: &mut Instance,
    root_host_pid: usize,
    reason: &[u8],
) {
    if !mark_killed(tasks, index) {
        process_native_fail(instance, root_host_pid, reason);
    }
}

/// Whether a signal that is not blocked and whose default action terminates
/// the task is queued for the stopped host task, privately or for its whole
/// thread group. A blocked signal stays queued without interrupting the wait,
/// so counting it would restart the call forever. An unreadable mask keeps
/// the plain park.
fn termination_pending(host_pid: usize) -> bool {
    const PTRACE_PEEKSIGINFO: usize = 0x4209;
    const PTRACE_GETSIGMASK: usize = 0x420a;
    const PTRACE_PEEKSIGINFO_SHARED: u32 = 1;
    #[repr(C)]
    struct PeekSiginfoArgs {
        offset: u64,
        flags: u32,
        count: i32,
    }
    let mut blocked = 0u64;
    if ptrace(
        PTRACE_GETSIGMASK,
        host_pid,
        core::mem::size_of_val(&blocked),
        &mut blocked as *mut _ as usize,
    ) < 0
    {
        return false;
    }
    let mut info = [0i32; 32];
    for flags in [0, PTRACE_PEEKSIGINFO_SHARED] {
        let mut offset = 0;
        loop {
            let arguments = PeekSiginfoArgs {
                offset,
                flags,
                count: 1,
            };
            if ptrace(
                PTRACE_PEEKSIGINFO,
                host_pid,
                &arguments as *const _ as usize,
                info.as_mut_ptr() as usize,
            ) != 1
            {
                break;
            }
            if (1..=64).contains(&info[0])
                && blocked & (1u64 << (info[0] - 1)) == 0
                && fatal_default_signal(info[0] as u8)
            {
                return true;
            }
            offset += 1;
        }
    }
    false
}

/// Resume every `wait4` waiter of thread group `tgid` parked at its entry
/// stop, after a SIGINT or SIGTERM was sent to that group. Only SIGKILL ends
/// a ptrace stop by itself, so a parked waiter would hold any other signal
/// pending forever. Each resumed waiter skips its call, and its exit stop
/// restarts it (see `wait_restart`): the signal is delivered on the way back
/// to user space and kills the task through the usual forwarding, or,
/// ignored, lets the wait park again. Tasks of the group that are not parked
/// are left alone; they take the signal on their own.
fn interrupt_parked_waiters(
    instance: &mut Instance,
    root_host_pid: usize,
    processes: &ProcessTable<8>,
    tasks: &mut [Option<NativeProcessTask>; 8],
    tgid: u32,
) {
    for index in 0..tasks.len() {
        let Some(entry) = tasks[index] else {
            continue;
        };
        if !entry.wait_pending
            || !entry.tracing
            || entry.killed
            || !processes
                .view(entry.virtual_id)
                .is_ok_and(|view| view.tgid == tgid)
        {
            continue;
        }
        if let Some(slot) = tasks[index].as_mut() {
            slot.wait_pending = false;
            slot.wait_restart = true;
        }
        write_all(b"GINUX_NATIVE_PROCESS_AUDIT: wait interrupted caller=");
        write_u64(entry.virtual_id.pid as u64);
        write_all(b"\n");
        if ptrace(PTRACE_SYSCALL, entry.host_pid, 0, 0) < 0 && !mark_killed(tasks, index) {
            process_native_fail(instance, root_host_pid, b"wait interrupt resume failed");
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
    instance: &mut Instance,
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
        if !entry.wait_pending || !entry.tracing || entry.killed {
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
                Err(_) => process_native_fail(instance, root_host_pid, b"wait wakeup failed"),
            };
            clear_host_slot(tasks, done.child);
            match store_wait_results(
                entry.host_pid,
                entry.wait_status,
                entry.wait_rusage,
                done.status.word(),
            ) {
                Ok(()) => {
                    write_all(b"GINUX_NATIVE_PROCESS_AUDIT: wait wakeup caller=");
                    write_u64(entry.virtual_id.pid as u64);
                    write_all(b" child=");
                    write_u64(done.child.pid as u64);
                    write_all(b" status=");
                    write_i64(done.status.shell_code() as i64);
                    write_all(b"\n");
                    done.child.pid as i64
                }
                Err(error) if error == -EFAULT => {
                    write_wait_fault(entry.virtual_id.pid, done.child.pid);
                    -EFAULT
                }
                Err(_) => {
                    // A parked waiter killed by its own group exit only
                    // awaits its death report now, like a killed task in the
                    // main loop.
                    if !mark_killed(tasks, index) {
                        process_native_fail(instance, root_host_pid, b"wait result store failed");
                    }
                    index += 1;
                    continue;
                }
            }
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
        if ptrace(PTRACE_SYSCALL, entry.host_pid, 0, SIGCHLD) < 0 && !mark_killed(tasks, index) {
            process_native_fail(instance, root_host_pid, b"wait wakeup resume failed");
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

/// Store a completed wait into the caller's `wait4` buffers as the kernel
/// does after reaping the child: the full status word first, then the
/// `rusage`, each skipped for a null pointer. Exited children report the code
/// in bits 8-15; signaled children report the signal number in bits 0-6 with
/// no core-dump bit. Without usage accounting the honest bounded `rusage`
/// content is zeros. Both stores go through [`write_tracee_bytes`] and so
/// honor the caller's page protections (except on its ptrace fallback, where
/// only unmapped memory faults): a buffer the caller cannot write fails with
/// `-EFAULT`, and a failed status store skips the `rusage`. Like the
/// kernel's copy, a faulting `rusage` keeps the bytes before the fault.
fn store_wait_results(
    host_pid: usize,
    user_status: usize,
    user_rusage: usize,
    status: u32,
) -> Result<(), i64> {
    const RUSAGE_BYTES: usize = 144;
    if user_status != 0 {
        store_tracee_u32(host_pid, user_status, status)?;
    }
    if user_rusage != 0 {
        write_tracee_bytes_result(host_pid, user_rusage, &[0; RUSAGE_BYTES])?;
    }
    Ok(())
}

/// Store a 4-byte word (a wait status or a clone tid) all or nothing, as
/// the kernel's single 4-byte `put_user` does. A word that crosses a page
/// boundary is written in two parts, the second page first: if that part
/// faults nothing is written, and if the first page then faults, the second
/// part gets its previous bytes back.
fn store_tracee_u32(host_pid: usize, address: usize, value: u32) -> Result<(), i64> {
    const PAGE_SIZE: usize = 4096;
    let bytes = value.to_le_bytes();
    let head = PAGE_SIZE - (address & (PAGE_SIZE - 1));
    if head >= bytes.len() {
        return write_tracee_bytes_result(host_pid, address, &bytes);
    }
    let Some(tail) = address.checked_add(head) else {
        return Err(-EFAULT);
    };
    // The page-aligned tail starts a ptrace word; a page that cannot be read
    // cannot be written either.
    let mut previous = 0usize;
    let read = ptrace(
        PTRACE_PEEKTEXT,
        host_pid,
        tail,
        &mut previous as *mut _ as usize,
    );
    if read < 0 {
        return Err(fault_errno(read as i64));
    }
    write_tracee_bytes_result(host_pid, tail, &bytes[head..])?;
    if let Err(error) = write_tracee_bytes_result(host_pid, address, &bytes[..head]) {
        let _ = write_tracee_bytes_result(
            host_pid,
            tail,
            &previous.to_le_bytes()[..bytes.len() - head],
        );
        return Err(error);
    }
    Ok(())
}

fn write_wait_fault(caller: u32, child: u32) {
    write_all(b"GINUX_NATIVE_PROCESS_AUDIT: wait store fault caller=");
    write_u64(caller as u64);
    write_all(b" reaped=");
    write_u64(child as u64);
    write_all(b"\n");
}

/// Publish the virtual tid into a `clone` tid cell: the parent's for
/// `CLONE_PARENT_SETTID`, the child's for `CLONE_CHILD_SETTID`. The host
/// kernel stored the host tid there; thread identity requires the virtual
/// tid instead. The store goes through [`store_tracee_u32`] and so honors the
/// page protections like the kernel's `put_user` (except on the ptrace
/// fallback of [`write_tracee_bytes`], where only unmapped memory faults).
/// The kernel ignores a failed tid store and `clone` still succeeds, so a
/// fault is only audited here and returns `Ok(false)`; any other error, such
/// as `ESRCH` for a tracee that is gone, is reported.
fn store_tracee_tid(host_pid: usize, user_tid: usize, tid: u32) -> Result<bool, ()> {
    match store_tracee_u32(host_pid, user_tid, tid) {
        Ok(()) => Ok(true),
        Err(error) if error == -EFAULT => {
            write_all(b"GINUX_NATIVE_PROCESS_AUDIT: tid store fault virtual-tid=");
            write_u64(tid as u64);
            write_all(b"\n");
            Ok(false)
        }
        Err(_) => Err(()),
    }
}

fn process_native_fail(instance: &mut Instance, _root_pid: usize, reason: &[u8]) -> ! {
    // Every caller runs after PTRACE_SETOPTIONS set PTRACE_O_EXITKILL, which
    // fork, vfork and clone children inherit, so the exit below kills every
    // tracee. No kill or reap of its own: the root may already be reaped (its
    // pid free for reuse), and traced threads may still hold its group open.
    instance_fail(instance, reason)
}

struct HostBlock {
    descriptor: usize,
    len: u64,
}

/// Partition the devices gate and the block benchmark place on the backing file.
const BLOCK_PARTITION_START: u64 = 1024;
const BLOCK_PARTITION_LEN: u64 = 2048;

impl HostBlock {
    /// Wraps an open backing file, taking its length from `fstat`. Fails with
    /// `stat_reason` if the size cannot be read and with `size_reason` if the
    /// file does not cover the partition, before anything is written to it.
    fn open(descriptor: usize, stat_reason: &[u8], size_reason: &[u8]) -> Self {
        // struct stat on x86_64 is 144 bytes; st_size is the 64-bit field at
        // byte offset 48.
        let mut stat = [0u64; 18];
        if unsafe { syscall2(SYS_FSTAT, descriptor, stat.as_mut_ptr() as usize) } != 0 {
            device_fail(stat_reason);
        }
        let len = stat[6];
        if len < BLOCK_PARTITION_START + BLOCK_PARTITION_LEN {
            device_fail(size_reason);
        }
        Self { descriptor, len }
    }
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
    let instance_id = 6u64;
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
                    instance_id,
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
        || config.network_backend != ginux_instance::NetworkBackend::Mediated
    {
        device_fail(b"required normalized assignment missing");
    }
    let mut visible = [DeviceName::new(b"unused").unwrap(); 8];
    let visible_count = registry.visible(instance_id, &mut visible);
    if visible_count != 6 {
        device_fail(b"unexpected device projection");
    }
    line(b"GINUX_DEVICES: visible=console,null,zero,random,block0,net0 unassigned=hidden");

    let zero = registry
        .open(instance_id, b"zero")
        .unwrap_or_else(|_| device_fail(b"zero open failed"));
    let mut zeros = [1u8; 16];
    if registry.read(instance_id, zero, &mut zeros) != Ok(16) || zeros != [0; 16] {
        device_fail(b"zero read failed");
    }
    let console = registry
        .open(instance_id, b"console")
        .unwrap_or_else(|_| device_fail(b"console open failed"));
    let mut window = [0u8; 8];
    if registry
        .ioctl(instance_id, console, IOCTL_CONSOLE_GET_SIZE, &mut window)
        .is_err()
        || registry
            .ioctl(instance_id, console, 0xdead, &mut [])
            .is_ok()
        || registry.map(instance_id, console).is_ok()
    {
        device_fail(b"ioctl or mapping policy failed");
    }
    line(b"GINUX_DEVICES: handles=virtual pseudo=PASS ioctl-unknown=denied mmap=denied");

    let host_descriptor =
        unsafe { syscall4(SYS_OPENAT_HOST, AT_FDCWD, block_path as usize, O_RDWR, 0) };
    if host_descriptor < 0 {
        device_fail(b"block backend open failed");
    }
    let backend = HostBlock::open(
        host_descriptor as usize,
        b"block backend stat failed",
        b"block backend smaller than partition",
    );
    let mut partition = Partition::new(backend, BLOCK_PARTITION_START, BLOCK_PARTITION_LEN, false)
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
    if network.send(b"instance-packet").is_err() {
        device_fail(b"network send failed");
    }
    let mut packet = [0u8; 64];
    if network.receive(&mut packet) != Ok(15)
        || &packet[..15] != b"instance-packet"
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
    registry.revoke(instance_id);
    if registry
        .assign(
            7,
            100,
            DeviceName::new(b"otherdisk").unwrap(),
            DeviceKind::Block,
        )
        .is_err()
        || registry.write(instance_id, console, b"revoked").is_ok()
    {
        device_fail(b"revocation failed");
    }
    line(b"GINUX_DEVICES: conflict=denied teardown=flushed,released,reusable");
    line(b"GATE6_DEVICES: PASS");
    exit_process(0)
}

fn text_list_contains<const N: usize>(values: &[ginux_instance::Text<N>], expected: &[u8]) -> bool {
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
        let backend = HostBlock::open(
            descriptor as usize,
            b"benchmark block stat failed",
            b"benchmark block backend smaller than partition",
        );
        let mut partition =
            Partition::new(backend, BLOCK_PARTITION_START, BLOCK_PARTITION_LEN, false)
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
        // Stop the clocks before close(), which flushes: the fsync is not one
        // of the measured read/write operations.
        let wall_ns = clock_nanoseconds(CLOCK_MONOTONIC) - wall_start;
        let cpu_ns = clock_nanoseconds(CLOCK_PROCESS_CPUTIME_ID) - cpu_start;
        if partition.close().is_err() {
            device_fail(b"benchmark block close failed");
        }
        (repetitions * 512, wall_ns, cpu_ns)
    } else if c_string_equals(kind, b"network") {
        if config.network_backend != ginux_instance::NetworkBackend::Mediated {
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
    const OVERLAY_VALUE: &[u8] = b"instance-local";
    let config = load_config(config_path);
    let mut instance = Instance::create(config);
    if instance.start().is_err() {
        instance_fail(&mut instance, b"invalid start transition");
    }
    let index_length =
        read_index(index_path).unwrap_or_else(|reason| instance_fail(&mut instance, reason));
    let index_bytes = unsafe {
        core::slice::from_raw_parts(
            core::ptr::addr_of!(VFS_INDEX_BYTES).cast::<u8>(),
            index_length,
        )
    };
    let index = match Index::new(index_bytes) {
        Ok(index) => index,
        Err(_) => instance_fail(&mut instance, b"invalid VFS index"),
    };
    let descriptor =
        unsafe { syscall4(SYS_OPENAT_HOST, AT_FDCWD, rootfs_path as usize, O_RDONLY, 0) };
    if descriptor < 0 {
        instance_fail(&mut instance, b"rootfs open failed");
    }
    let reader = HostRootfs {
        descriptor: descriptor as usize,
    };
    let mut vfs: VirtualFs<'_, HostRootfs, 8, 4096, 8> = VirtualFs::new(index, reader);

    let os_fd = match vfs.open(b"/etc/os-release", O_READ) {
        Ok(value) => value,
        Err(_) => vfs_fail(
            &mut instance,
            &mut vfs,
            descriptor as usize,
            b"os-release lookup failed",
        ),
    };
    let mut os_release = [0u8; 512];
    let os_length = match vfs.read(os_fd, &mut os_release) {
        Ok(value) => value,
        Err(_) => vfs_fail(
            &mut instance,
            &mut vfs,
            descriptor as usize,
            b"os-release read failed",
        ),
    };
    let _ = vfs.close(os_fd);
    if !contains(&os_release[..os_length], b"Rocky Linux") {
        vfs_fail(
            &mut instance,
            &mut vfs,
            descriptor as usize,
            b"Rocky identity missing",
        );
    }

    let bash_fd = match vfs.open(b"/usr/bin/bash", O_READ) {
        Ok(value) => value,
        Err(_) => vfs_fail(
            &mut instance,
            &mut vfs,
            descriptor as usize,
            b"bash lookup failed",
        ),
    };
    let mut magic = [0u8; 4];
    if vfs.read(bash_fd, &mut magic) != Ok(4) || &magic != b"\x7fELF" {
        vfs_fail(
            &mut instance,
            &mut vfs,
            descriptor as usize,
            b"bash ELF read failed",
        );
    }
    let _ = vfs.close(bash_fd);

    let overlay_fd = match vfs.open(b"/ginux-overlay-proof", O_READ | O_WRITE | O_CREATE) {
        Ok(value) => value,
        Err(_) => vfs_fail(
            &mut instance,
            &mut vfs,
            descriptor as usize,
            b"overlay create failed",
        ),
    };
    if vfs.write(overlay_fd, OVERLAY_VALUE) != Ok(OVERLAY_VALUE.len()) {
        vfs_fail(
            &mut instance,
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
    write_all(b"GINUX_VFS: overlay=/ginux-overlay-proof value=");
    line(OVERLAY_VALUE);
    line(b"GINUX_VFS: managed-host-paths=unexposed managed-host-fds=unexposed");
    vfs.teardown();
    let _ = unsafe { syscall1(SYS_CLOSE_HOST, descriptor as usize) };
    if instance.task_exited(true).is_err() {
        instance_fail(&mut instance, b"invalid task-exit transition");
    }
    line(b"GINUX_VFS: teardown overlay-files=0 descriptors=0");
    line(b"GINUX_VFS_CHECK: PASS");
    exit_process(0)
}

fn vfs_fail(
    instance: &mut Instance,
    vfs: &mut VirtualFs<'_, HostRootfs, 8, 4096, 8>,
    descriptor: usize,
    reason: &[u8],
) -> ! {
    vfs.teardown();
    let _ = unsafe { syscall1(SYS_CLOSE_HOST, descriptor) };
    instance_fail(instance, reason)
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    needle.is_empty()
        || haystack
            .windows(needle.len())
            .any(|window| window == needle)
}

fn contract_label(config: &ginux_instance::InstanceConfig) -> &[u8] {
    if config.contract_recipe_version == 0 {
        config.contract_profile.as_bytes()
    } else {
        b"self-contained-v1"
    }
}

fn native_main(
    config_path: *const u8,
    root_path: *const u8,
    mode: usize,
    hook_index: Option<usize>,
) -> ! {
    let config = load_config(config_path);
    let contract = config
        .effective_contract()
        .unwrap_or_else(|error| config_rejected(error));
    if hook_index.is_some_and(|index| index >= config.pre_boot_hooks.as_slice().len()) {
        config_rejected(ginux_instance::ConfigError::InvalidValue);
    }
    print_instance_start_summary(&config);
    let systemd_contract = matches!(
        config.contract_profile.as_bytes(),
        b"gate9-systemd-v1" | b"strict-v1"
    ) || (config.contract_recipe_version != 0
        && config.init.as_bytes() == b"/sbin/init");
    let mode = if mode == 0 && systemd_contract {
        3
    } else {
        mode
    };
    let direct_yield = config.dispatch_mode == DispatchMode::Selective
        && contract.syscall_rule(24).is_some_and(|rule| {
            rule.operation.direct_safe && rule.action == ContractAction::Mediate
        });
    let mut instance = Instance::create(config);
    if instance.start().is_err() {
        instance_fail(&mut instance, b"invalid native start transition");
    }
    // Before any descriptor is opened here, which would take the number of
    // a closed standard stream.
    if let Some(reason) = standard_stream_refusal() {
        instance_fail(&mut instance, reason);
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
        instance_fail(&mut instance, b"native root open failed");
    }
    let child = unsafe { syscall0(SYS_FORK) };
    if child < 0 {
        instance_fail(&mut instance, b"native tracee fork failed");
    }
    if child == 0 {
        native_tracee(root_path, mode, hook_index, &config, direct_yield);
    }
    if let Some(index) = hook_index {
        write_all(b"GINUX_HOOK_AUDIT: action=start index=");
        write_u64(index as u64);
        write_all(b" context=instance mediated=true uid=");
        write_u64(config.access_uid);
        write_all(b" gid=");
        write_u64(config.access_gid);
        write_all(b"\n");
    }
    supervise_native(
        child as usize,
        &mut instance,
        mode,
        root_descriptor as usize,
        contract,
        hook_index.is_some(),
        direct_yield,
    )
}

fn native_tracee(
    root_path: *const u8,
    mode: usize,
    hook_index: Option<usize>,
    config: &InstanceConfig,
    direct_yield: bool,
) -> ! {
    static ROOT: &[u8] = b"/\0";
    static BASH: &[u8] = b"/usr/bin/bash\0";
    static ARG1: &[u8] = b"--noprofile\0";
    static ARG2: &[u8] = b"--norc\0";
    static ARG3: &[u8] = b"-c\0";
    static SH: &[u8] = b"/bin/sh\0";
    static COMMAND: &[u8] = b"printf 'GINUX_NATIVE_SHELL: PASS\\n'\0";
    static HOLD_COMMAND: &[u8] = b"printf 'GINUX_NATIVE_HOLD: READY\\n'; while :; do :; done\0";
    static PROBE_COMMAND: &[u8] = b"if : > /tmp/ginux-mutation; then exit 90; else printf 'GINUX_BALANCED_MUTATION: BLOCKED\\n'; fi; if exec 9<>/dev/tcp/127.0.0.1/9; then exit 91; else printf 'GINUX_BALANCED_NETWORK: BLOCKED\\n'; fi\0";
    static ENV_PATH: &[u8] = b"PATH=/usr/bin:/bin\0";
    static ENV_HOME: &[u8] = b"HOME=/\0";
    static ENV_LOCALE: &[u8] = b"LC_ALL=C\0";
    static HOST_DEV_NULL: &[u8] = b"/dev/null\0";

    // Inherit nothing beyond the standard streams, at any descriptor number:
    // a descriptor left open to a host directory would anchor paths outside
    // the root. The protected null and console descriptors are made after.
    if unsafe {
        syscall3(
            ginux_core::SYS_CLOSE_RANGE as usize,
            3,
            u32::MAX as usize,
            0,
        )
    } < 0
    {
        native_tracee_setup_fail(127);
    }
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
    let protected_null = unsafe { syscall3(SYS_FCNTL_HOST, null_fd as usize, 0, INSTANCE_NULL_FD) };
    let _ = unsafe { syscall1(SYS_CLOSE_HOST, null_fd as usize) };
    if protected_null != INSTANCE_NULL_FD as isize {
        native_tracee_setup_fail(119);
    }
    let protected_console = unsafe { syscall3(SYS_FCNTL_HOST, 0, 0, INSTANCE_CONSOLE_FD) };
    if protected_console != INSTANCE_CONSOLE_FD as isize {
        native_tracee_setup_fail(119);
    }

    if unsafe { syscall1(SYS_CHROOT, root_path as usize) } < 0 {
        native_tracee_setup_fail(120);
    }
    if unsafe { syscall1(SYS_CHDIR, ROOT.as_ptr() as usize) } < 0 {
        native_tracee_setup_fail(121);
    }
    if unsafe { syscall4(SYS_PTRACE, PTRACE_TRACEME, 0, 0, 0) } < 0 {
        native_tracee_setup_fail(122);
    }
    let host_identity = managed_host_identity();
    if host_identity.drop_root {
        if unsafe { syscall2(SYS_SETGROUPS, 0, 0) } < 0 {
            native_tracee_setup_fail(123);
        }
        if unsafe { syscall1(SYS_SETGID, host_identity.gid) } < 0 {
            native_tracee_setup_fail(124);
        }
        if unsafe { syscall1(SYS_SETUID, host_identity.uid) } < 0 {
            native_tracee_setup_fail(125);
        }
    }
    if unsafe { syscall5(SYS_PRCTL, PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) } < 0 {
        native_tracee_setup_fail(126);
    }
    // Give the parent a stop at which it can enable PTRACE_O_TRACESECCOMP
    // before this process installs the mandatory all-syscall trace filter.
    let self_pid = unsafe { syscall0(39) };
    if self_pid <= 0 || unsafe { syscall2(SYS_KILL, self_pid as usize, SIGSTOP) } < 0 {
        native_tracee_setup_fail(127);
    }
    let filter_result = install_trace_filter(direct_yield);
    if (direct_yield && filter_result != 0) || (!direct_yield && filter_result != 3) {
        native_tracee_setup_fail(128);
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
    let mut hook_command = [0u8; 513];
    let hook_argv = if let Some(index) = hook_index {
        copy_c_string(
            config.pre_boot_hooks.as_slice()[index].as_bytes(),
            &mut hook_command,
        );
        [
            SH.as_ptr(),
            ARG3.as_ptr(),
            hook_command.as_ptr(),
            core::ptr::null(),
        ]
    } else {
        [core::ptr::null(); 4]
    };
    unsafe {
        syscall3(
            SYS_EXECVE,
            if hook_index.is_some() {
                SH.as_ptr() as usize
            } else if mode == 3 {
                configured_init.as_ptr() as usize
            } else {
                BASH.as_ptr() as usize
            },
            if hook_index.is_some() {
                hook_argv.as_ptr() as usize
            } else if mode == 3 {
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
    clone_call: native_clone::Pending,
    lifetime_pidfd: isize,
    virtual_id: TaskId,
    start_ticks: u64,
    awaiting_exit: bool,
    override_exit: bool,
    override_result: i64,
    pending_console_open: bool,
    console_fd: i64,
    console_fds: [u64; NATIVE_PROC_FD_LIMIT / 64],
    pending_console_duplicate: i8,
    pipe2_address: usize,
    pipe_fds: [i64; 8],
    socket_fds: [NativeSocketFd; NATIVE_SOCKET_FD_LIMIT],
    pending_socket_type: u8,
    pending_sockaddr: usize,
    pending_sockaddr_word: u64,
    pending_getsockname: usize,
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
    wait_status: u64,
    waitid_info: u64,
    waitid_selector: u64,
    initial_stop_pending: bool,
    initial_exec_pending: bool,
    job_stop_pending: bool,
    job_stopped: bool,
    tracing: bool,
    limits: [[u64; 2]; 16],
}

#[derive(Clone, Copy)]
struct NativeRetiredCredential {
    host_pid: usize,
    virtual_pid: u32,
    uid: u32,
    gid: u32,
    lifetime_pidfd: isize,
}

const EMPTY_RETIRED_CREDENTIAL: NativeRetiredCredential = NativeRetiredCredential {
    host_pid: 0,
    virtual_pid: 0,
    uid: 0,
    gid: 0,
    lifetime_pidfd: -1,
};

#[derive(Clone, Copy)]
struct NativeSocketFd {
    fd: i64,
    protocol: u8,
    source_bound: bool,
    internal_port: u16,
    external_port: u16,
}

#[derive(Clone, Copy)]
struct NativeVirtualKernelFd {
    owner: usize,
    fd: i64,
    path: [u8; 192],
    path_len: usize,
    offset: u64,
    flags: u64,
    close_on_exec: bool,
}

#[derive(Clone, Copy)]
struct NativeVirtualDirectory {
    path: [u8; 192],
    path_len: usize,
}

const EMPTY_VIRTUAL_DIRECTORY: NativeVirtualDirectory = NativeVirtualDirectory {
    path: [0; 192],
    path_len: 0,
};

struct NativeVirtualKernelState {
    directories: [NativeVirtualDirectory; NATIVE_VIRTUAL_DIRECTORY_LIMIT],
}

impl NativeVirtualKernelState {
    const fn new() -> Self {
        Self {
            directories: [EMPTY_VIRTUAL_DIRECTORY; NATIVE_VIRTUAL_DIRECTORY_LIMIT],
        }
    }

    fn cgroup_directory_exists(&self, path: &[u8]) -> bool {
        path == b"/sys/fs/cgroup"
            || self.directories.iter().any(|directory| {
                directory.path_len == path.len() && directory.path[..directory.path_len] == *path
            })
    }

    fn create_cgroup_directory(&mut self, path: &[u8]) -> i64 {
        if path == b"/sys/fs/cgroup" || self.cgroup_directory_exists(path) {
            return -17;
        }
        if !path.starts_with(b"/sys/fs/cgroup/") || path.len() > 192 {
            return -EPERM;
        }
        let Some(separator) = path.iter().rposition(|byte| *byte == b'/') else {
            return -2;
        };
        if !self.cgroup_directory_exists(&path[..separator]) {
            return -2;
        }
        let Some(slot) = self
            .directories
            .iter_mut()
            .find(|directory| directory.path_len == 0)
        else {
            return -28;
        };
        slot.path[..path.len()].copy_from_slice(path);
        slot.path_len = path.len();
        0
    }

    fn remove_cgroup_directory(&mut self, path: &[u8]) -> i64 {
        if path == b"/sys/fs/cgroup" || !path.starts_with(b"/sys/fs/cgroup/") {
            return -EPERM;
        }
        let Some(index) = self.directories.iter().position(|directory| {
            directory.path_len == path.len() && directory.path[..directory.path_len] == *path
        }) else {
            return -2;
        };
        if self.directories.iter().any(|directory| {
            directory.path_len > path.len()
                && directory.path[..path.len()] == *path
                && directory.path[path.len()] == b'/'
        }) {
            return -39;
        }
        self.directories[index] = EMPTY_VIRTUAL_DIRECTORY;
        0
    }
}

const EMPTY_VIRTUAL_KERNEL_FD: NativeVirtualKernelFd = NativeVirtualKernelFd {
    owner: 0,
    fd: -1,
    path: [0; 192],
    path_len: 0,
    offset: 0,
    flags: 0,
    close_on_exec: false,
};

const EMPTY_SOCKET_FD: NativeSocketFd = NativeSocketFd {
    fd: -1,
    protocol: 0,
    source_bound: false,
    internal_port: 0,
    external_port: 0,
};

struct NativeProcSample {
    name: [u8; 64],
    name_len: usize,
    cmdline: [u8; 256],
    cmdline_len: usize,
    state: u8,
    minflt: u64,
    cminflt: u64,
    majflt: u64,
    cmajflt: u64,
    utime: u64,
    stime: u64,
    cutime: i64,
    cstime: i64,
    priority: i64,
    nice: i64,
    threads: u64,
    starttime: u64,
    vsize: u64,
    rss_pages: i64,
}

impl NativeProcSample {
    fn name(&self) -> &[u8] {
        &self.name[..self.name_len]
    }

    fn cmdline(&self) -> &[u8] {
        &self.cmdline[..self.cmdline_len]
    }
}

fn native_virtual_kernel_fd(
    entries: &[NativeVirtualKernelFd; NATIVE_VIRTUAL_KERNEL_FD_LIMIT],
    owner: usize,
    fd: i64,
) -> Option<&NativeVirtualKernelFd> {
    entries
        .iter()
        .find(|entry| entry.owner == owner && entry.fd == fd)
}

fn native_virtual_kernel_fd_mut(
    entries: &mut [NativeVirtualKernelFd; NATIVE_VIRTUAL_KERNEL_FD_LIMIT],
    owner: usize,
    fd: i64,
) -> Option<&mut NativeVirtualKernelFd> {
    entries
        .iter_mut()
        .find(|entry| entry.owner == owner && entry.fd == fd)
}

fn native_virtual_kernel_close(
    entries: &mut [NativeVirtualKernelFd; NATIVE_VIRTUAL_KERNEL_FD_LIMIT],
    owner: usize,
    fd: i64,
) -> bool {
    let Some(entry) = native_virtual_kernel_fd_mut(entries, owner, fd) else {
        return false;
    };
    *entry = EMPTY_VIRTUAL_KERNEL_FD;
    true
}

fn native_virtual_kernel_clear_owner(
    entries: &mut [NativeVirtualKernelFd; NATIVE_VIRTUAL_KERNEL_FD_LIMIT],
    owner: usize,
) {
    for entry in entries.iter_mut().filter(|entry| entry.owner == owner) {
        *entry = EMPTY_VIRTUAL_KERNEL_FD;
    }
}

fn native_virtual_kernel_clear_range(
    entries: &mut [NativeVirtualKernelFd; NATIVE_VIRTUAL_KERNEL_FD_LIMIT],
    owner: usize,
    first: i64,
    last: i64,
) {
    for entry in entries
        .iter_mut()
        .filter(|entry| entry.owner == owner && entry.fd >= first && entry.fd <= last)
    {
        *entry = EMPTY_VIRTUAL_KERNEL_FD;
    }
}

fn native_virtual_kernel_fork(
    entries: &mut [NativeVirtualKernelFd; NATIVE_VIRTUAL_KERNEL_FD_LIMIT],
    parent: usize,
    child: usize,
) -> bool {
    let snapshot = *entries;
    for inherited in snapshot.iter().filter(|entry| entry.owner == parent) {
        let Some(slot) = entries.iter_mut().find(|entry| entry.owner == 0) else {
            return false;
        };
        *slot = *inherited;
        slot.owner = child;
    }
    true
}

fn native_virtual_kernel_exec(
    entries: &mut [NativeVirtualKernelFd; NATIVE_VIRTUAL_KERNEL_FD_LIMIT],
    owner: usize,
) {
    for entry in entries
        .iter_mut()
        .filter(|entry| entry.owner == owner && entry.close_on_exec)
    {
        *entry = EMPTY_VIRTUAL_KERNEL_FD;
    }
}

fn native_virtual_kernel_allocate(
    entries: &mut [NativeVirtualKernelFd; NATIVE_VIRTUAL_KERNEL_FD_LIMIT],
    owner: usize,
    path: &[u8],
    flags: u64,
    close_on_exec: bool,
) -> Option<i64> {
    if path.len() > 192 {
        return None;
    }
    let mut candidate = VIRTUAL_KERNEL_FD_BASE;
    while entries
        .iter()
        .any(|entry| entry.owner == owner && entry.fd == candidate)
    {
        candidate = candidate.checked_add(1)?;
    }
    let slot = entries.iter_mut().find(|entry| entry.owner == 0)?;
    *slot = EMPTY_VIRTUAL_KERNEL_FD;
    slot.owner = owner;
    slot.fd = candidate;
    slot.path[..path.len()].copy_from_slice(path);
    slot.path_len = path.len();
    slot.flags = flags;
    slot.close_on_exec = close_on_exec;
    Some(candidate)
}

fn native_virtual_kernel_duplicate(
    entries: &mut [NativeVirtualKernelFd; NATIVE_VIRTUAL_KERNEL_FD_LIMIT],
    owner: usize,
    old_fd: i64,
    requested: Option<i64>,
    close_on_exec: bool,
) -> Option<i64> {
    let original = *native_virtual_kernel_fd(entries, owner, old_fd)?;
    let new_fd = requested.unwrap_or_else(|| {
        let mut candidate = VIRTUAL_KERNEL_FD_BASE;
        while entries
            .iter()
            .any(|entry| entry.owner == owner && entry.fd == candidate)
        {
            candidate += 1;
        }
        candidate
    });
    if new_fd == old_fd {
        return Some(new_fd);
    }
    if let Some(existing) = native_virtual_kernel_fd_mut(entries, owner, new_fd) {
        *existing = EMPTY_VIRTUAL_KERNEL_FD;
    }
    let slot = entries.iter_mut().find(|entry| entry.owner == 0)?;
    *slot = original;
    slot.fd = new_fd;
    slot.close_on_exec = close_on_exec;
    Some(new_fd)
}

fn native_virtual_kernel_duplicate_at_least(
    entries: &mut [NativeVirtualKernelFd; NATIVE_VIRTUAL_KERNEL_FD_LIMIT],
    owner: usize,
    old_fd: i64,
    minimum: i64,
    close_on_exec: bool,
) -> Option<i64> {
    let mut candidate = core::cmp::max(minimum, VIRTUAL_KERNEL_FD_BASE);
    while entries
        .iter()
        .any(|entry| entry.owner == owner && entry.fd == candidate)
    {
        candidate = candidate.checked_add(1)?;
    }
    native_virtual_kernel_duplicate(entries, owner, old_fd, Some(candidate), close_on_exec)
}

fn native_virtual_kernel_path(
    host_pid: usize,
    directory_fd: i64,
    address: usize,
    entries: &[NativeVirtualKernelFd; NATIVE_VIRTUAL_KERNEL_FD_LIMIT],
    output: &mut [u8; 192],
) -> Option<usize> {
    let mut input = [0u8; 192];
    let input_len = read_tracee_c_string(host_pid, address, &mut input)?;
    let mut length = 0usize;
    if input.first() != Some(&b'/') {
        let directory = native_virtual_kernel_fd(entries, host_pid, directory_fd)?;
        native_buffer_append(output, &mut length, &directory.path[..directory.path_len])?;
        if length > 1 && output[length - 1] != b'/' {
            native_buffer_append(output, &mut length, b"/")?;
        }
    }
    for component in input[..input_len].split(|byte| *byte == b'/') {
        if component.is_empty() || component == b"." {
            continue;
        }
        if component == b".." {
            if length <= 1 {
                length = 1;
                continue;
            }
            while length > 1 && output[length - 1] == b'/' {
                length -= 1;
            }
            while length > 1 && output[length - 1] != b'/' {
                length -= 1;
            }
            if length > 1 {
                length -= 1;
            }
            continue;
        }
        if length == 0 || output[length - 1] != b'/' {
            native_buffer_append(output, &mut length, b"/")?;
        }
        native_buffer_append(output, &mut length, component)?;
    }
    if length == 0 {
        output[0] = b'/';
        length = 1;
    }
    if matches!(
        output.get(..length),
        Some(path) if path == b"/proc" || path.starts_with(b"/proc/")
            || path == b"/sys" || path.starts_with(b"/sys/")
    ) {
        Some(length)
    } else {
        None
    }
}

fn native_virtual_task<'p, 'a>(
    path: &'p [u8],
    current: TaskId,
    processes: &'a ProcessTable<128>,
    tasks: &'a [Option<NativeSupervisorTask>; 128],
) -> Option<(TaskView, &'a NativeSupervisorTask, &'p [u8])> {
    let rest = path.strip_prefix(b"/proc/")?;
    let slash = rest
        .iter()
        .position(|byte| *byte == b'/')
        .unwrap_or(rest.len());
    let virtual_pid = if &rest[..slash] == b"self" {
        current.pid
    } else {
        u32::try_from(native_parse_u64(&rest[..slash])?).ok()?
    };
    let supervisor = tasks
        .iter()
        .flatten()
        .find(|task| task.tracing && task.virtual_id.pid == virtual_pid)?;
    let view = processes.view(supervisor.virtual_id).ok()?;
    let leaf = if slash == rest.len() {
        b"".as_slice()
    } else {
        &rest[slash + 1..]
    };
    Some((view, supervisor, leaf))
}

fn native_virtual_kernel_directory(
    path: &[u8],
    current: TaskId,
    processes: &ProcessTable<128>,
    tasks: &[Option<NativeSupervisorTask>; 128],
    state: &NativeVirtualKernelState,
) -> bool {
    if matches!(
        path,
        b"/proc"
            | b"/proc/pressure"
            | b"/proc/sys"
            | b"/proc/sys/fs"
            | b"/proc/sys/kernel"
            | b"/proc/sys/kernel/random"
            | b"/sys"
            | b"/sys/kernel"
            | b"/sys/kernel/security"
            | b"/sys/fs"
            | b"/sys/class"
            | b"/sys/class/block"
            | b"/sys/class/net"
            | b"/sys/class/tty"
            | b"/sys/class/misc"
            | b"/sys/devices"
            | b"/sys/devices/system"
            | b"/sys/devices/system/cpu"
    ) {
        return true;
    }
    if path == b"/sys/fs/cgroup" || path.starts_with(b"/sys/fs/cgroup/") {
        return state.cgroup_directory_exists(path);
    }
    native_virtual_task(path, current, processes, tasks)
        .is_some_and(|(_, _, leaf)| leaf.is_empty() || matches!(leaf, b"fd" | b"fdinfo" | b"ns"))
}

fn native_virtual_kernel_exists(
    path: &[u8],
    current: TaskId,
    processes: &ProcessTable<128>,
    tasks: &[Option<NativeSupervisorTask>; 128],
    state: &NativeVirtualKernelState,
    virtual_fds: &[NativeVirtualKernelFd; NATIVE_VIRTUAL_KERNEL_FD_LIMIT],
) -> bool {
    if native_virtual_kernel_directory(path, current, processes, tasks, state) {
        return true;
    }
    if matches!(
        path,
        b"/proc/stat"
            | b"/proc/uptime"
            | b"/proc/meminfo"
            | b"/proc/mounts"
            | b"/proc/loadavg"
            | b"/proc/cpuinfo"
            | b"/proc/version"
            | b"/proc/pressure/cpu"
            | b"/proc/sys/fs/file-max"
            | b"/proc/sys/fs/nr_open"
            | b"/proc/sys/kernel/cap_last_cap"
            | b"/proc/sys/kernel/hostname"
            | b"/proc/sys/kernel/ostype"
            | b"/proc/sys/kernel/osrelease"
            | b"/proc/sys/kernel/version"
            | b"/proc/sys/kernel/random/boot_id"
            | b"/sys/kernel/security/lsm"
            | b"/sys/devices/system/cpu/online"
            | b"/sys/fs/cgroup/cgroup.controllers"
            | b"/sys/fs/cgroup/cgroup.procs"
            | b"/sys/fs/cgroup/cgroup.subtree_control"
    ) {
        return true;
    }
    if path.starts_with(b"/sys/fs/cgroup/") {
        let Some(separator) = path.iter().rposition(|byte| *byte == b'/') else {
            return false;
        };
        if state.cgroup_directory_exists(&path[..separator])
            && matches!(
                &path[separator + 1..],
                b"cgroup.controllers"
                    | b"cgroup.procs"
                    | b"cgroup.subtree_control"
                    | b"cgroup.events"
                    | b"cgroup.kill"
                    | b"cpu.max"
                    | b"cpu.weight"
                    | b"memory.current"
                    | b"memory.events"
                    | b"memory.high"
                    | b"memory.low"
                    | b"memory.max"
                    | b"memory.min"
                    | b"pids.current"
                    | b"pids.events"
                    | b"pids.max"
            )
        {
            return true;
        }
    }
    if path.starts_with(b"/sys/class/") {
        return matches!(
            path.rsplit(|byte| *byte == b'/').next().unwrap_or_default(),
            b"dev" | b"type" | b"uevent"
        );
    }
    native_virtual_task(path, current, processes, tasks).is_some_and(|(_, supervisor, leaf)| {
        if let Some(digits) = leaf
            .strip_prefix(b"fd/")
            .or_else(|| leaf.strip_prefix(b"fdinfo/"))
        {
            let Some(fd) = native_parse_u64(digits).and_then(|value| usize::try_from(value).ok())
            else {
                return false;
            };
            if native_virtual_kernel_fd(virtual_fds, supervisor.host_pid, fd as i64).is_some() {
                return true;
            }
            let mut live = [0u64; NATIVE_PROC_FD_LIMIT / 64];
            return native_tracee_fd_snapshot(supervisor.host_pid, &mut live)
                && native_fd_contains(&live, fd);
        }
        matches!(
            leaf,
            b"stat"
                | b"statm"
                | b"comm"
                | b"cgroup"
                | b"status"
                | b"cmdline"
                | b"oom_score_adj"
                | b"mountinfo"
                | b"root"
                | b"ns/mnt"
                | b"ns/pid"
                | b"ns/uts"
                | b"ns/cgroup"
        )
    })
}

fn native_virtual_kernel_symlink(
    path: &[u8],
    current: TaskId,
    processes: &ProcessTable<128>,
    tasks: &[Option<NativeSupervisorTask>; 128],
) -> bool {
    path == b"/proc/self"
        || native_virtual_task(path, current, processes, tasks).is_some_and(|(_, _, leaf)| {
            leaf == b"root" || leaf.starts_with(b"ns/") || leaf.starts_with(b"fd/")
        })
}

fn native_default_limits() -> [[u64; 2]; 16] {
    let mut limits = [[u64::MAX; 2]; 16];
    limits[3] = [8 * 1024 * 1024, u64::MAX];
    limits[6] = [4096, 4096];
    limits[7] = [4096, 4096];
    limits[8] = [64 * 1024, 64 * 1024];
    limits
}

fn native_pidfd_reaped(fd: isize) -> Option<bool> {
    #[repr(C)]
    struct PollFd {
        fd: i32,
        events: i16,
        revents: i16,
    }
    let mut descriptor = PollFd {
        fd: fd as i32,
        events: 1,
        revents: 0,
    };
    let result = unsafe { syscall3(7, &mut descriptor as *mut _ as usize, 1, 0) };
    if result < 0 || descriptor.revents & 32 != 0 {
        return None;
    }
    Some(descriptor.revents & 16 != 0)
}

fn native_retire_credential(
    task: NativeSupervisorTask,
    processes: &ProcessTable<128>,
    retired: &mut [NativeRetiredCredential; 128],
    next: &mut usize,
) -> bool {
    // Reclaim the full task/process slot while retaining the small identity
    // that queued UNIX messages and connected peers may still report. Keeping
    // the pidfd open pins the numeric host PID, so the bounded cache cannot
    // translate a reused PID as an older virtual sender.
    if task.lifetime_pidfd < 0 {
        return true;
    }
    let Ok(view) = processes.view(task.virtual_id) else {
        return false;
    };
    let slot = retired
        .iter()
        .position(|credential| credential.lifetime_pidfd < 0)
        .unwrap_or_else(|| {
            let slot = *next % retired.len();
            *next = next.wrapping_add(1);
            slot
        });
    if retired[slot].lifetime_pidfd >= 0 {
        let _ = unsafe { syscall1(SYS_CLOSE_HOST, retired[slot].lifetime_pidfd as usize) };
    }
    retired[slot] = NativeRetiredCredential {
        host_pid: task.host_pid,
        virtual_pid: task.virtual_id.pid,
        uid: view.credentials.uid,
        gid: view.credentials.gid,
        lifetime_pidfd: task.lifetime_pidfd,
    };
    true
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

fn native_tracee_fd_snapshot(
    host_pid: usize,
    entries: &mut [u64; NATIVE_PROC_FD_LIMIT / 64],
) -> bool {
    entries.fill(0);
    let mut host_path = [0u8; 64];
    let mut host_path_len = 0usize;
    if native_buffer_append(&mut host_path, &mut host_path_len, b"/proc/").is_none()
        || native_buffer_append_u64(&mut host_path, &mut host_path_len, host_pid as u64).is_none()
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
    let mut buffer = [0u8; 4096];
    let mut success = true;
    loop {
        let count = unsafe {
            syscall3(
                SYS_GETDENTS64_HOST,
                directory as usize,
                buffer.as_mut_ptr() as usize,
                buffer.len(),
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
                u16::from_ne_bytes([buffer[offset + 16], buffer[offset + 17]]) as usize;
            if record_length < 20 || offset.saturating_add(record_length) > count as usize {
                success = false;
                break;
            }
            let mut fd = 0usize;
            let mut digits = 0usize;
            for byte in &buffer[offset + 19..offset + record_length] {
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
                && !matches!(fd, INSTANCE_NULL_FD | INSTANCE_CONSOLE_FD)
            {
                native_fd_set(entries, fd);
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

fn native_host_proc_read(host_pid: usize, leaf: &[u8], output: &mut [u8]) -> Option<usize> {
    let mut path = [0u8; 96];
    let mut length = 0usize;
    native_buffer_append(&mut path, &mut length, b"/proc/")?;
    native_buffer_append_u64(&mut path, &mut length, host_pid as u64)?;
    native_buffer_append(&mut path, &mut length, b"/")?;
    native_buffer_append(&mut path, &mut length, leaf)?;
    if length >= path.len() {
        return None;
    }
    path[length] = 0;
    native_host_file_read(&path, output)
}

fn native_host_file_read(path: &[u8], output: &mut [u8]) -> Option<usize> {
    let descriptor = unsafe {
        syscall4(
            SYS_OPENAT_HOST,
            AT_FDCWD,
            path.as_ptr() as usize,
            O_RDONLY,
            0,
        )
    };
    if descriptor < 0 {
        return None;
    }
    let mut length = 0usize;
    while length < output.len() {
        let count = unsafe {
            syscall3(
                SYS_READ_HOST,
                descriptor as usize,
                output.as_mut_ptr().add(length) as usize,
                output.len() - length,
            )
        };
        if count < 0 {
            let _ = unsafe { syscall1(SYS_CLOSE_HOST, descriptor as usize) };
            return None;
        }
        if count == 0 {
            break;
        }
        length = length.checked_add(count as usize)?;
    }
    let _ = unsafe { syscall1(SYS_CLOSE_HOST, descriptor as usize) };
    Some(length)
}

fn native_parse_u64(input: &[u8]) -> Option<u64> {
    if input.is_empty() || !input.iter().all(u8::is_ascii_digit) {
        return None;
    }
    input.iter().try_fold(0u64, |value, byte| {
        value.checked_mul(10)?.checked_add(u64::from(*byte - b'0'))
    })
}

fn native_parse_i64(input: &[u8]) -> Option<i64> {
    let (negative, digits) = match input.first() {
        Some(b'-') => (true, &input[1..]),
        Some(b'+') => (false, &input[1..]),
        Some(_) => (false, input),
        None => return None,
    };
    let value = native_parse_u64(digits)?;
    if negative {
        i64::try_from(value).ok()?.checked_neg()
    } else {
        i64::try_from(value).ok()
    }
}

fn native_proc_sample(host_pid: usize) -> Option<NativeProcSample> {
    let mut stat = [0u8; 1024];
    let stat_len = native_host_proc_read(host_pid, b"stat", &mut stat)?;
    let stat = &stat[..stat_len];
    let name_start = stat.iter().position(|byte| *byte == b'(')?.checked_add(1)?;
    let name_end = stat.iter().rposition(|byte| *byte == b')')?;
    if name_end <= name_start || name_end.checked_add(3)? > stat.len() {
        return None;
    }
    let tail = &stat[name_end + 2..];
    let state = *tail.first()?;
    if tail.get(1) != Some(&b' ') {
        return None;
    }
    let mut fields = [&[][..]; 21];
    let mut count = 0usize;
    for field in tail[2..]
        .split(|byte| *byte == b' ' || *byte == b'\n')
        .filter(|field| !field.is_empty())
    {
        if count == fields.len() {
            break;
        }
        fields[count] = field;
        count += 1;
    }
    if count != fields.len() {
        return None;
    }

    let source_name = &stat[name_start..name_end];
    let mut name = [0u8; 64];
    let name_len = core::cmp::min(source_name.len(), name.len());
    name[..name_len].copy_from_slice(&source_name[..name_len]);

    let mut cmdline = [0u8; 256];
    let mut cmdline_len = native_host_proc_read(host_pid, b"cmdline", &mut cmdline)?;
    if cmdline_len == 0 {
        cmdline[..name_len].copy_from_slice(&name[..name_len]);
        cmdline_len = name_len;
        if cmdline_len < cmdline.len() {
            cmdline[cmdline_len] = 0;
            cmdline_len += 1;
        }
    }

    Some(NativeProcSample {
        name,
        name_len,
        cmdline,
        cmdline_len,
        state,
        minflt: native_parse_u64(fields[6])?,
        cminflt: native_parse_u64(fields[7])?,
        majflt: native_parse_u64(fields[8])?,
        cmajflt: native_parse_u64(fields[9])?,
        utime: native_parse_u64(fields[10])?,
        stime: native_parse_u64(fields[11])?,
        cutime: native_parse_i64(fields[12])?,
        cstime: native_parse_i64(fields[13])?,
        priority: native_parse_i64(fields[14])?,
        nice: native_parse_i64(fields[15])?,
        threads: native_parse_u64(fields[16])?,
        starttime: native_parse_u64(fields[18])?,
        vsize: native_parse_u64(fields[19])?,
        rss_pages: native_parse_i64(fields[20])?,
    })
}

fn native_proc_stat(task: TaskView, sample: &NativeProcSample, output: &mut [u8]) -> Option<usize> {
    let mut length = 0usize;
    native_buffer_append_u32(output, &mut length, task.id.pid)?;
    native_buffer_append(output, &mut length, b" (")?;
    native_buffer_append(output, &mut length, sample.name())?;
    native_buffer_append(output, &mut length, b") ")?;
    native_buffer_append(output, &mut length, &[sample.state])?;
    native_buffer_append(output, &mut length, b" ")?;
    native_buffer_append_u32(output, &mut length, task.parent)?;
    native_buffer_append(output, &mut length, b" ")?;
    native_buffer_append_u32(output, &mut length, task.process_group)?;
    native_buffer_append(output, &mut length, b" ")?;
    native_buffer_append_u32(output, &mut length, task.session)?;
    native_buffer_append(output, &mut length, b" 0 -1 0 ")?;
    for value in [sample.minflt, sample.cminflt, sample.majflt, sample.cmajflt] {
        native_buffer_append_u64(output, &mut length, value)?;
        native_buffer_append(output, &mut length, b" ")?;
    }
    native_buffer_append_u64(output, &mut length, sample.utime)?;
    native_buffer_append(output, &mut length, b" ")?;
    native_buffer_append_u64(output, &mut length, sample.stime)?;
    native_buffer_append(output, &mut length, b" ")?;
    native_buffer_append_i64(output, &mut length, sample.cutime)?;
    native_buffer_append(output, &mut length, b" ")?;
    native_buffer_append_i64(output, &mut length, sample.cstime)?;
    native_buffer_append(output, &mut length, b" ")?;
    native_buffer_append_i64(output, &mut length, sample.priority)?;
    native_buffer_append(output, &mut length, b" ")?;
    native_buffer_append_i64(output, &mut length, sample.nice)?;
    native_buffer_append(output, &mut length, b" ")?;
    native_buffer_append_u64(output, &mut length, sample.threads)?;
    native_buffer_append(output, &mut length, b" 0 ")?;
    native_buffer_append_u64(output, &mut length, sample.starttime)?;
    native_buffer_append(output, &mut length, b" ")?;
    native_buffer_append_u64(output, &mut length, sample.vsize)?;
    native_buffer_append(output, &mut length, b" ")?;
    native_buffer_append_i64(output, &mut length, sample.rss_pages)?;
    native_buffer_append(
        output,
        &mut length,
        b" 0 0 0 0 0 0 0 0 0 0 0 0 0 0 17 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0\n",
    )?;
    Some(length)
}

fn native_proc_status(
    task: TaskView,
    sample: &NativeProcSample,
    output: &mut [u8],
) -> Option<usize> {
    let mut length = 0usize;
    native_buffer_append(output, &mut length, b"Name:\t")?;
    native_buffer_append(output, &mut length, sample.name())?;
    native_buffer_append(output, &mut length, b"\nState:\t")?;
    native_buffer_append(output, &mut length, &[sample.state])?;
    native_buffer_append(output, &mut length, b"\nTgid:\t")?;
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
    native_buffer_append(output, &mut length, b"\nThreads:\t")?;
    native_buffer_append_u64(output, &mut length, sample.threads)?;
    native_buffer_append(output, &mut length, b"\nVmSize:\t")?;
    native_buffer_append_u64(output, &mut length, sample.vsize / 1024)?;
    native_buffer_append(output, &mut length, b" kB\nVmRSS:\t")?;
    native_buffer_append_i64(
        output,
        &mut length,
        sample.rss_pages.saturating_mul(4).max(0),
    )?;
    native_buffer_append(output, &mut length, b" kB\n\n")?;
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

fn native_buffer_append_u64(output: &mut [u8], length: &mut usize, mut value: u64) -> Option<()> {
    let mut digits = [0u8; 20];
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

fn native_buffer_append_i64(output: &mut [u8], length: &mut usize, value: i64) -> Option<()> {
    if value < 0 {
        native_buffer_append(output, length, b"-")?;
    }
    native_buffer_append_u64(output, length, value.unsigned_abs())
}

fn native_buffer_append_hundredths(
    output: &mut [u8],
    length: &mut usize,
    value: u64,
) -> Option<()> {
    native_buffer_append(output, length, &[b'0' + ((value / 10) % 10) as u8])?;
    native_buffer_append(output, length, &[b'0' + (value % 10) as u8])
}

fn native_identity_bytes(instance_id: &[u8]) -> [u8; 16] {
    let mut first = 0xcbf2_9ce4_8422_2325u64;
    let mut second = 0x8422_2325_cbf2_9ce4u64;
    for byte in instance_id {
        first ^= u64::from(*byte);
        first = first.wrapping_mul(0x100_0000_01b3);
        second ^= u64::from(*byte).wrapping_add(0x9d);
        second = second.wrapping_mul(0x100_0000_01b3);
    }
    let mut identity = [0u8; 16];
    identity[..8].copy_from_slice(&first.to_be_bytes());
    identity[8..].copy_from_slice(&second.to_be_bytes());
    // Present the stable per-instance value as an RFC 4122 variant/version UUID.
    identity[6] = (identity[6] & 0x0f) | 0x40;
    identity[8] = (identity[8] & 0x3f) | 0x80;
    identity
}

fn native_buffer_append_identity_uuid(
    output: &mut [u8],
    length: &mut usize,
    instance_id: &[u8],
) -> Option<()> {
    let identity = native_identity_bytes(instance_id);
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for (index, byte) in identity.iter().enumerate() {
        if matches!(index, 4 | 6 | 8 | 10) {
            native_buffer_append(output, length, b"-")?;
        }
        native_buffer_append(output, length, &[HEX[(byte >> 4) as usize]])?;
        native_buffer_append(output, length, &[HEX[(byte & 0x0f) as usize]])?;
    }
    Some(())
}

struct NativeVirtualIdentity<'a> {
    instance_id: &'a [u8],
    hostname: &'a [u8],
    contract_profile: &'a [u8],
}

fn native_virtual_kernel_read(
    path: &[u8],
    current: TaskId,
    processes: &ProcessTable<128>,
    tasks: &[Option<NativeSupervisorTask>; 128],
    global: [u64; 4],
    identity: NativeVirtualIdentity<'_>,
    output: &mut [u8],
) -> Option<usize> {
    let mut length = 0usize;
    match path {
        b"/proc/version" => {
            native_buffer_append(output, &mut length, GINUX_VERSION.as_bytes())?;
            native_buffer_append(output, &mut length, b" (Linux ABI personality ")?;
            native_buffer_append(output, &mut length, GINUX_ABI_RELEASE)?;
            native_buffer_append(output, &mut length, b"; contract ")?;
            native_buffer_append(output, &mut length, identity.contract_profile)?;
            native_buffer_append(output, &mut length, b"; x86_64)\n")?;
        }
        b"/proc/mounts" => native_buffer_append(
            output,
            &mut length,
            b"ginux-root / ginuxfs rw 0 0\nproc /proc proc rw,nosuid,nodev,noexec 0 0\nsysfs /sys sysfs ro,nosuid,nodev,noexec 0 0\n",
        )?,
        b"/proc/pressure/cpu" => native_buffer_append(
            output,
            &mut length,
            b"some avg10=0.00 avg60=0.00 avg300=0.00 total=0\n",
        )?,
        b"/proc/sys/fs/file-max" => {
            native_buffer_append(output, &mut length, b"9223372036854775807\n")?
        }
        b"/proc/sys/fs/nr_open" => native_buffer_append(output, &mut length, b"1048576\n")?,
        b"/proc/sys/kernel/cap_last_cap" => {
            native_buffer_append(output, &mut length, b"40\n")?
        }
        b"/proc/sys/kernel/hostname" => {
            native_buffer_append(output, &mut length, identity.hostname)?;
            native_buffer_append(output, &mut length, b"\n")?;
        }
        b"/proc/sys/kernel/ostype" => native_buffer_append(output, &mut length, b"Linux\n")?,
        b"/proc/sys/kernel/osrelease" => {
            native_buffer_append(output, &mut length, GINUX_ABI_RELEASE)?;
            native_buffer_append(output, &mut length, b"\n")?;
        }
        b"/proc/sys/kernel/version" => {
            native_buffer_append(output, &mut length, GINUX_VERSION.as_bytes())?;
            native_buffer_append(output, &mut length, b"\n")?;
        }
        b"/proc/sys/kernel/random/boot_id" => {
            native_buffer_append_identity_uuid(output, &mut length, identity.instance_id)?;
            native_buffer_append(output, &mut length, b"\n")?;
        }
        b"/sys/kernel/security/lsm" => {}
        b"/sys/devices/system/cpu/online" => {
            native_buffer_append(output, &mut length, b"0")?;
            if global[2] > 1 {
                native_buffer_append(output, &mut length, b"-")?;
                native_buffer_append_u64(output, &mut length, global[2] - 1)?;
            }
            native_buffer_append(output, &mut length, b"\n")?;
        }
        b"/sys/fs/cgroup/cgroup.controllers" => {
            native_buffer_append(output, &mut length, b"cpu memory pids\n")?
        }
        b"/sys/fs/cgroup/cgroup.procs" | b"/sys/fs/cgroup/cgroup.subtree_control" => {}
        b"/proc/cpuinfo" => {
            return native_virtual_cpuinfo(global[2], output);
        }
        b"/proc/stat" | b"/proc/uptime" | b"/proc/meminfo" | b"/proc/loadavg" => {
            return native_virtual_proc_global(path, processes, tasks, global, output);
        }
        _ if path.starts_with(b"/sys/fs/cgroup/") => {
            match path.rsplit(|byte| *byte == b'/').next().unwrap_or_default() {
                b"cgroup.controllers" => {
                    native_buffer_append(output, &mut length, b"cpu memory pids\n")?
                }
                b"cgroup.procs" => {}
                b"cgroup.subtree_control" => {}
                b"cgroup.events" => {
                    native_buffer_append(output, &mut length, b"populated 1\nfrozen 0\n")?
                }
                b"cgroup.kill" => {}
                b"cpu.max" => native_buffer_append(output, &mut length, b"max 100000\n")?,
                b"cpu.weight" => native_buffer_append(output, &mut length, b"100\n")?,
                b"memory.current" => native_buffer_append(output, &mut length, b"0\n")?,
                b"memory.events" => native_buffer_append(
                    output,
                    &mut length,
                    b"low 0\nhigh 0\nmax 0\noom 0\noom_kill 0\n",
                )?,
                b"memory.high" | b"memory.max" => {
                    native_buffer_append(output, &mut length, b"max\n")?
                }
                b"memory.low" | b"memory.min" | b"pids.current" => {
                    native_buffer_append(output, &mut length, b"0\n")?
                }
                b"pids.events" => {
                    native_buffer_append(output, &mut length, b"max 0\n")?
                }
                b"pids.max" => native_buffer_append(output, &mut length, b"max\n")?,
                _ => return None,
            }
        }
        _ if path.starts_with(b"/sys/class/") => {
            let leaf = path.rsplit(|byte| *byte == b'/').next().unwrap_or_default();
            match leaf {
                b"dev" => native_buffer_append(output, &mut length, b"1:3\n")?,
                b"type" => native_buffer_append(output, &mut length, b"1\n")?,
                b"uevent" => native_buffer_append(output, &mut length, b"DEVTYPE=virtual\n")?,
                _ => return None,
            }
        }
        _ => {
            let (view, supervisor, leaf) =
                native_virtual_task(path, current, processes, tasks)?;
            let mut sample = native_proc_sample(supervisor.host_pid)?;
            sample.starttime = supervisor.start_ticks;
            match leaf {
                b"stat" => return native_proc_stat(view, &sample, output),
                b"status" => return native_proc_status(view, &sample, output),
                b"statm" => {
                    let size_pages = sample.vsize / 4096;
                    let resident_pages = sample.rss_pages.max(0) as u64;
                    native_buffer_append_u64(output, &mut length, size_pages)?;
                    native_buffer_append(output, &mut length, b" ")?;
                    native_buffer_append_u64(output, &mut length, resident_pages)?;
                    native_buffer_append(output, &mut length, b" 0 0 0 ")?;
                    native_buffer_append_u64(
                        output,
                        &mut length,
                        size_pages.saturating_sub(resident_pages),
                    )?;
                    native_buffer_append(output, &mut length, b" 0\n")?;
                }
                b"comm" => {
                    native_buffer_append(output, &mut length, sample.name())?;
                    native_buffer_append(output, &mut length, b"\n")?;
                }
                b"cmdline" => native_buffer_append(output, &mut length, sample.cmdline())?,
                b"cgroup" => native_buffer_append(output, &mut length, b"0::/\n")?,
                b"oom_score_adj" => native_buffer_append(output, &mut length, b"0\n")?,
                b"mountinfo" => native_buffer_append(
                    output,
                    &mut length,
                    b"1 0 0:1 / / rw,nosuid,nodev - ginuxfs ginux-root rw\n2 1 0:2 / /proc rw,nosuid,nodev,noexec - proc proc rw\n3 1 0:3 / /sys ro,nosuid,nodev,noexec - sysfs sysfs ro\n",
                )?,
                _ if leaf.starts_with(b"fdinfo/") => {
                    native_buffer_append(
                        output,
                        &mut length,
                        b"pos:\t0\nflags:\t02000002\nmnt_id:\t1\nino:\t1\n",
                    )?;
                    let descriptor = usize::try_from(native_parse_u64(
                        leaf.strip_prefix(b"fdinfo/")?,
                    )?)
                    .ok()?;
                    if let Some(target_host_pid) =
                        native_pidfd_host_target(supervisor.host_pid, descriptor)
                    {
                        native_buffer_append(output, &mut length, b"Pid:\t")?;
                        let target_virtual_pid = tasks
                            .iter()
                            .flatten()
                            .find(|task| task.tracing && task.host_pid == target_host_pid)
                            .map(|task| task.virtual_id.pid);
                        if let Some(target_virtual_pid) = target_virtual_pid {
                            native_buffer_append_u32(output, &mut length, target_virtual_pid)?;
                        } else {
                            native_buffer_append(output, &mut length, b"-1")?;
                        }
                        native_buffer_append(output, &mut length, b"\nNSpid:\t")?;
                        if let Some(target_virtual_pid) = target_virtual_pid {
                            native_buffer_append_u32(output, &mut length, target_virtual_pid)?;
                        } else {
                            native_buffer_append(output, &mut length, b"-1")?;
                        }
                        native_buffer_append(output, &mut length, b"\n")?;
                    }
                }
                _ => return None,
            }
        }
    }
    Some(length)
}

fn native_virtual_cpuinfo(vcpus: u64, output: &mut [u8]) -> Option<usize> {
    let mut source = [0u8; 256 * 1024];
    let input_len = native_host_file_read(b"/proc/cpuinfo\0", &mut source)?;
    let mut input_offset = 0usize;
    let mut output_len = 0usize;
    let mut processors = 0u64;
    while input_offset < input_len && processors < vcpus {
        let remaining = &source[input_offset..input_len];
        let block_len = remaining
            .windows(2)
            .position(|window| window == b"\n\n")
            .map_or(remaining.len(), |offset| offset + 2);
        if source[input_offset..input_offset + block_len].starts_with(b"processor") {
            native_buffer_append(
                output,
                &mut output_len,
                &source[input_offset..input_offset + block_len],
            )?;
            processors += 1;
        }
        input_offset += block_len;
    }
    (processors == vcpus && output_len > 0).then_some(output_len)
}

fn native_virtual_proc_global(
    path: &[u8],
    processes: &ProcessTable<128>,
    tasks: &[Option<NativeSupervisorTask>; 128],
    global: [u64; 4],
    output: &mut [u8],
) -> Option<usize> {
    let now_ns = clock_nanoseconds(CLOCK_MONOTONIC);
    let elapsed_ns = now_ns.saturating_sub(global[0]);
    let elapsed_ticks = elapsed_ns / 10_000_000;
    let mut user_ticks = 0u64;
    let mut system_ticks = 0u64;
    let mut rss_pages = 0u64;
    let mut active = 0u64;
    let mut last_pid = 1u32;
    for task in tasks.iter().flatten().filter(|task| task.tracing) {
        let view = processes.view(task.virtual_id).ok()?;
        let sample = native_proc_sample(task.host_pid)?;
        user_ticks = user_ticks.saturating_add(sample.utime);
        system_ticks = system_ticks.saturating_add(sample.stime);
        if view.tgid == view.id.pid {
            rss_pages = rss_pages.saturating_add(sample.rss_pages.max(0) as u64);
        }
        active += 1;
        last_pid = core::cmp::max(last_pid, view.id.pid);
    }
    let mut length = 0usize;
    match path {
        b"/proc/stat" => {
            let total = elapsed_ticks.saturating_mul(core::cmp::max(global[2], 1));
            native_buffer_append(output, &mut length, b"cpu  ")?;
            native_buffer_append_u64(output, &mut length, user_ticks)?;
            native_buffer_append(output, &mut length, b" 0 ")?;
            native_buffer_append_u64(output, &mut length, system_ticks)?;
            native_buffer_append(output, &mut length, b" ")?;
            native_buffer_append_u64(
                output,
                &mut length,
                total.saturating_sub(user_ticks.saturating_add(system_ticks)),
            )?;
            native_buffer_append(output, &mut length, b" 0 0 0 0 0 0\nbtime ")?;
            native_buffer_append_u64(output, &mut length, global[1])?;
            native_buffer_append(output, &mut length, b"\nprocesses ")?;
            native_buffer_append_u32(output, &mut length, last_pid)?;
            native_buffer_append(output, &mut length, b"\nprocs_running ")?;
            native_buffer_append_u64(output, &mut length, active)?;
            native_buffer_append(output, &mut length, b"\nprocs_blocked 0\n")?;
        }
        b"/proc/uptime" => {
            let hundredths = elapsed_ns / 10_000_000;
            native_buffer_append_u64(output, &mut length, hundredths / 100)?;
            native_buffer_append(output, &mut length, b".")?;
            native_buffer_append_hundredths(output, &mut length, hundredths % 100)?;
            native_buffer_append(output, &mut length, b" 0.00\n")?;
        }
        b"/proc/meminfo" => {
            let total = global[3].saturating_mul(1024);
            let available = total.saturating_sub(rss_pages.saturating_mul(4));
            for (label, value) in [
                (b"MemTotal:       ".as_slice(), total),
                (b"MemFree:        ".as_slice(), available),
                (b"MemAvailable:   ".as_slice(), available),
                (b"Buffers:        ".as_slice(), 0),
                (b"Cached:         ".as_slice(), 0),
                (b"SwapTotal:      ".as_slice(), 0),
                (b"SwapFree:       ".as_slice(), 0),
            ] {
                native_buffer_append(output, &mut length, label)?;
                native_buffer_append_u64(output, &mut length, value)?;
                native_buffer_append(output, &mut length, b" kB\n")?;
            }
        }
        b"/proc/loadavg" => {
            native_buffer_append(output, &mut length, b"0.00 0.00 0.00 ")?;
            native_buffer_append_u64(output, &mut length, active)?;
            native_buffer_append(output, &mut length, b"/")?;
            native_buffer_append_u64(output, &mut length, active)?;
            native_buffer_append(output, &mut length, b" ")?;
            native_buffer_append_u32(output, &mut length, last_pid)?;
            native_buffer_append(output, &mut length, b"\n")?;
        }
        _ => return None,
    }
    Some(length)
}

fn native_virtual_kernel_stat(
    path: &[u8],
    directory: bool,
    symbolic_link: bool,
    size: u64,
) -> [u8; 144] {
    let mut stat = [0u8; 144];
    stat[0..8].copy_from_slice(&1u64.to_ne_bytes());
    stat[8..16].copy_from_slice(&1u64.to_ne_bytes());
    stat[16..24].copy_from_slice(&(if directory { 2u64 } else { 1u64 }).to_ne_bytes());
    let mode = if symbolic_link {
        0o120_777u32
    } else if directory {
        0o040_555u32
    } else {
        0o100_444u32
    };
    stat[24..28].copy_from_slice(&mode.to_ne_bytes());
    stat[28..32].copy_from_slice(&0u32.to_ne_bytes());
    stat[32..36].copy_from_slice(&0u32.to_ne_bytes());
    stat[48..56].copy_from_slice(&size.to_ne_bytes());
    stat[56..64].copy_from_slice(&4096i64.to_ne_bytes());
    stat[64..72].copy_from_slice(&size.div_ceil(512).to_ne_bytes());
    let mut hash = 1u64;
    for byte in path {
        hash = hash
            .wrapping_mul(1099511628211)
            .wrapping_add(u64::from(*byte));
    }
    stat[8..16].copy_from_slice(&hash.to_ne_bytes());
    stat
}

fn native_virtual_dirent(
    output: &mut [u8],
    length: &mut usize,
    index: u64,
    name: &[u8],
    file_type: u8,
) -> Option<()> {
    let record_len = (19usize.checked_add(name.len())?.checked_add(1)? + 7) & !7;
    let end = length.checked_add(record_len)?;
    if end > output.len() {
        return None;
    }
    output[*length..end].fill(0);
    output[*length..*length + 8].copy_from_slice(&(index + 1).to_ne_bytes());
    output[*length + 8..*length + 16].copy_from_slice(&((index + 1) as i64).to_ne_bytes());
    output[*length + 16..*length + 18].copy_from_slice(&(record_len as u16).to_ne_bytes());
    output[*length + 18] = file_type;
    output[*length + 19..*length + 19 + name.len()].copy_from_slice(name);
    *length = end;
    Some(())
}

#[allow(clippy::too_many_arguments)]
fn native_virtual_kernel_getdents(
    path: &[u8],
    current: TaskId,
    processes: &ProcessTable<128>,
    tasks: &[Option<NativeSupervisorTask>; 128],
    state: &NativeVirtualKernelState,
    virtual_fds: &[NativeVirtualKernelFd; NATIVE_VIRTUAL_KERNEL_FD_LIMIT],
    start: u64,
    output: &mut [u8],
) -> Option<(usize, u64)> {
    let mut length = 0usize;
    let mut index = 0u64;
    let mut add = |name: &[u8], file_type: u8| -> Option<()> {
        let selected = index >= start;
        if selected && native_virtual_dirent(output, &mut length, index, name, file_type).is_none()
        {
            return None;
        }
        index += 1;
        Some(())
    };
    add(b".", 4)?;
    add(b"..", 4)?;
    match path {
        b"/proc" => {
            for (name, directory) in [
                (b"self".as_slice(), 10),
                (b"stat", 8),
                (b"uptime", 8),
                (b"meminfo", 8),
                (b"mounts", 8),
                (b"loadavg", 8),
                (b"cpuinfo", 8),
                (b"version", 8),
                (b"pressure", 4),
                (b"sys", 4),
            ] {
                add(name, directory)?;
            }
            for task in tasks.iter().flatten().filter(|task| task.tracing) {
                let mut name = [0u8; 10];
                let mut name_len = 0usize;
                native_buffer_append_u32(&mut name, &mut name_len, task.virtual_id.pid)?;
                add(&name[..name_len], 4)?;
            }
        }
        b"/proc/pressure" => add(b"cpu", 8)?,
        b"/proc/sys" => {
            add(b"fs", 4)?;
            add(b"kernel", 4)?;
        }
        b"/proc/sys/fs" => {
            add(b"file-max", 8)?;
            add(b"nr_open", 8)?;
        }
        b"/proc/sys/kernel" => {
            for (name, directory) in [
                (b"cap_last_cap".as_slice(), 8),
                (b"hostname", 8),
                (b"ostype", 8),
                (b"osrelease", 8),
                (b"version", 8),
                (b"random", 4),
            ] {
                add(name, directory)?;
            }
        }
        b"/proc/sys/kernel/random" => add(b"boot_id", 8)?,
        b"/sys" => {
            add(b"kernel", 4)?;
            add(b"fs", 4)?;
            add(b"class", 4)?;
            add(b"devices", 4)?;
        }
        b"/sys/kernel" => add(b"security", 4)?,
        b"/sys/kernel/security" => add(b"lsm", 8)?,
        b"/sys/fs" => add(b"cgroup", 4)?,
        b"/sys/fs/cgroup" => {
            add(b"cgroup.controllers", 8)?;
            add(b"cgroup.procs", 8)?;
            add(b"cgroup.subtree_control", 8)?;
            for directory in state.directories.iter().filter(|directory| {
                directory.path_len > path.len()
                    && directory.path[..path.len()] == *path
                    && directory.path[path.len()] == b'/'
                    && !directory.path[path.len() + 1..directory.path_len].contains(&b'/')
            }) {
                add(&directory.path[path.len() + 1..directory.path_len], 4)?;
            }
        }
        _ if path.starts_with(b"/sys/fs/cgroup/") => {
            add(b"cgroup.controllers", 8)?;
            add(b"cgroup.procs", 8)?;
            add(b"cgroup.subtree_control", 8)?;
            add(b"cgroup.events", 8)?;
            add(b"cgroup.kill", 8)?;
            add(b"cpu.max", 8)?;
            add(b"cpu.weight", 8)?;
            add(b"memory.current", 8)?;
            add(b"memory.events", 8)?;
            add(b"memory.high", 8)?;
            add(b"memory.low", 8)?;
            add(b"memory.max", 8)?;
            add(b"memory.min", 8)?;
            add(b"pids.current", 8)?;
            add(b"pids.events", 8)?;
            add(b"pids.max", 8)?;
            for directory in state.directories.iter().filter(|directory| {
                directory.path_len > path.len()
                    && directory.path[..path.len()] == *path
                    && directory.path[path.len()] == b'/'
                    && !directory.path[path.len() + 1..directory.path_len].contains(&b'/')
            }) {
                add(&directory.path[path.len() + 1..directory.path_len], 4)?;
            }
        }
        b"/sys/class" => {
            for class in [b"block".as_slice(), b"net", b"tty", b"misc"] {
                add(class, 4)?;
            }
        }
        b"/sys/devices" => add(b"system", 4)?,
        b"/sys/devices/system" => add(b"cpu", 4)?,
        b"/sys/devices/system/cpu" => add(b"online", 8)?,
        _ => {
            let (_, supervisor, leaf) = native_virtual_task(path, current, processes, tasks)?;
            match leaf {
                b"fd" | b"fdinfo" => {
                    let mut live = [0u64; NATIVE_PROC_FD_LIMIT / 64];
                    if !native_tracee_fd_snapshot(supervisor.host_pid, &mut live) {
                        return None;
                    }
                    for fd in 0..NATIVE_PROC_FD_LIMIT {
                        if native_fd_contains(&live, fd) {
                            let mut name = [0u8; 20];
                            let mut name_len = 0usize;
                            native_buffer_append_u64(&mut name, &mut name_len, fd as u64)?;
                            add(&name[..name_len], if leaf == b"fd" { 10 } else { 8 })?;
                        }
                    }
                    for descriptor in virtual_fds
                        .iter()
                        .filter(|descriptor| descriptor.owner == supervisor.host_pid)
                    {
                        let mut name = [0u8; 20];
                        let mut name_len = 0usize;
                        native_buffer_append_u64(&mut name, &mut name_len, descriptor.fd as u64)?;
                        add(&name[..name_len], if leaf == b"fd" { 10 } else { 8 })?;
                    }
                    return Some((length, index));
                }
                b"ns" => {
                    for name in [b"mnt".as_slice(), b"pid", b"uts", b"cgroup"] {
                        add(name, 10)?;
                    }
                    return Some((length, index));
                }
                b"" => {}
                _ => return Some((length, index)),
            }
            for (name, directory) in [
                (b"stat".as_slice(), 8),
                (b"statm", 8),
                (b"comm", 8),
                (b"cgroup", 8),
                (b"status", 8),
                (b"cmdline", 8),
                (b"oom_score_adj", 8),
                (b"mountinfo", 8),
                (b"fd", 4),
                (b"fdinfo", 4),
                (b"ns", 4),
                (b"root", 10),
            ] {
                add(name, directory)?;
            }
        }
    }
    Some((length, index))
}

fn native_virtual_kernel_link(path: &[u8], current: TaskId, output: &mut [u8]) -> Option<usize> {
    let mut length = 0usize;
    if path == b"/proc/self" {
        native_buffer_append_u32(output, &mut length, current.pid)?;
    } else if path.ends_with(b"/root") {
        native_buffer_append(output, &mut length, b"/")?;
    } else if path.contains(&b'/') && path.windows(4).any(|part| part == b"/ns/") {
        let namespace = path.rsplit(|byte| *byte == b'/').next()?;
        native_buffer_append(output, &mut length, namespace)?;
        native_buffer_append(output, &mut length, b":[1]")?;
    } else if path.windows(4).any(|part| part == b"/fd/") {
        native_buffer_append(output, &mut length, b"/dev/console")?;
    } else {
        return None;
    }
    Some(length)
}

fn native_virtual_proc_fd_number(path: &[u8], current: TaskId) -> Option<usize> {
    let rest = path.strip_prefix(b"/proc/")?;
    let separator = rest.iter().position(|byte| *byte == b'/')?;
    let pid = &rest[..separator];
    if pid != b"self" && native_parse_u64(pid)? != u64::from(current.pid) {
        return None;
    }
    let digits = rest[separator + 1..].strip_prefix(b"fd/")?;
    usize::try_from(native_parse_u64(digits)?).ok()
}

fn native_virtual_kernel_override(host_pid: usize, registers: &mut UserRegs, result: i64) -> bool {
    registers.orig_rax = u64::MAX;
    ptrace(PTRACE_SETREGS, host_pid, 0, registers as *const _ as usize) >= 0
        && ptrace(PTRACE_SYSCALL, host_pid, 0, 0) >= 0
        && {
            registers.rax = result as u64;
            true
        }
}

#[repr(C)]
struct SeccompData {
    nr: i32,
    arch: u32,
    instruction_pointer: u64,
    args: [u64; 6],
}

#[repr(C)]
struct SeccompNotification {
    id: u64,
    pid: u32,
    flags: u32,
    data: SeccompData,
}

#[repr(C)]
struct SeccompNotificationResponse {
    id: u64,
    value: i64,
    error: i32,
    flags: u32,
}

struct BrokerPidMap {
    audit_lock: AtomicU32,
    foreground_group: AtomicU32,
    entries: [AtomicU64; 128],
    process_groups: [AtomicU32; 128],
    suspend_requests: [AtomicU32; 128],
}

impl BrokerPidMap {
    fn new() -> Self {
        Self {
            audit_lock: AtomicU32::new(0),
            foreground_group: AtomicU32::new(0),
            entries: [const { AtomicU64::new(0) }; 128],
            process_groups: [const { AtomicU32::new(0) }; 128],
            suspend_requests: [const { AtomicU32::new(0) }; 128],
        }
    }

    fn lock_audit(&self) {
        if self
            .audit_lock
            .compare_exchange(0, 1, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
        {
            return;
        }
        loop {
            if self.audit_lock.swap(2, Ordering::Acquire) == 0 {
                return;
            }
            let _ = unsafe {
                syscall6(
                    SYS_FUTEX_HOST,
                    &self.audit_lock as *const AtomicU32 as usize,
                    FUTEX_WAIT,
                    2,
                    0,
                    0,
                    0,
                )
            };
        }
    }

    fn unlock_audit(&self) {
        if self.audit_lock.fetch_sub(1, Ordering::Release) == 1 {
            return;
        }
        self.audit_lock.store(0, Ordering::Release);
        let _ = unsafe {
            syscall6(
                SYS_FUTEX_HOST,
                &self.audit_lock as *const AtomicU32 as usize,
                FUTEX_WAKE,
                1,
                0,
                0,
                0,
            )
        };
    }

    fn set(&self, host_pid: usize, virtual_pid: u32, process_group: u32) -> bool {
        let encoded = ((host_pid as u64) << 32) | u64::from(virtual_pid);
        for (index, entry) in self.entries.iter().enumerate() {
            let current = entry.load(Ordering::Acquire);
            if current >> 32 == host_pid as u64 {
                self.suspend_requests[index].store(0, Ordering::Release);
                self.process_groups[index].store(process_group, Ordering::Release);
                entry.store(encoded, Ordering::Release);
                return true;
            }
            if current == 0
                && entry
                    .compare_exchange(0, encoded, Ordering::AcqRel, Ordering::Acquire)
                    .is_ok()
            {
                self.suspend_requests[index].store(0, Ordering::Release);
                self.process_groups[index].store(process_group, Ordering::Release);
                return true;
            }
        }
        false
    }

    fn remove(&self, host_pid: usize) {
        for (index, entry) in self.entries.iter().enumerate() {
            let current = entry.load(Ordering::Acquire);
            if current >> 32 == host_pid as u64 {
                entry.store(0, Ordering::Release);
                self.process_groups[index].store(0, Ordering::Release);
                self.suspend_requests[index].store(0, Ordering::Release);
                return;
            }
        }
    }

    fn update_process_group(&self, host_pid: usize, process_group: u32) -> bool {
        for (index, entry) in self.entries.iter().enumerate() {
            if entry.load(Ordering::Acquire) >> 32 == host_pid as u64 {
                self.process_groups[index].store(process_group, Ordering::Release);
                return true;
            }
        }
        false
    }

    fn set_foreground_group(&self, process_group: u32) {
        self.foreground_group
            .store(process_group, Ordering::Release);
    }

    fn foreground_group(&self) -> u32 {
        self.foreground_group.load(Ordering::Acquire)
    }

    fn signal_foreground(&self, signal: usize) -> bool {
        let foreground = self.foreground_group.load(Ordering::Acquire);
        let mut delivered = false;
        for (index, entry) in self.entries.iter().enumerate() {
            let encoded = entry.load(Ordering::Acquire);
            if encoded != 0 && self.process_groups[index].load(Ordering::Acquire) == foreground {
                let host_pid = (encoded >> 32) as usize;
                if signal == SIGTSTP {
                    self.suspend_requests[index].store(1, Ordering::Release);
                }
                if unsafe { syscall2(SYS_KILL, host_pid, signal) } == 0 {
                    delivered = true;
                } else if signal == SIGTSTP {
                    self.suspend_requests[index].store(0, Ordering::Release);
                }
            }
        }
        delivered
    }

    fn take_suspend_request(&self, host_pid: usize) -> bool {
        for (index, entry) in self.entries.iter().enumerate() {
            if entry.load(Ordering::Acquire) >> 32 == host_pid as u64 {
                return self.suspend_requests[index].swap(0, Ordering::AcqRel) != 0;
            }
        }
        false
    }

    fn virtual_pid(&self, host_pid: u32) -> Option<u32> {
        self.entries.iter().find_map(|entry| {
            let encoded = entry.load(Ordering::Acquire);
            (encoded >> 32 == u64::from(host_pid)).then_some(encoded as u32)
        })
    }
}

fn native_shared_pid_map() -> Option<&'static BrokerPidMap> {
    let address = unsafe {
        syscall6(
            SYS_MMAP_HOST,
            0,
            core::mem::size_of::<BrokerPidMap>(),
            PROT_READ_WRITE,
            MAP_SHARED_ANONYMOUS,
            usize::MAX,
            0,
        )
    };
    if address < 0 {
        return None;
    }
    let map = address as *mut BrokerPidMap;
    unsafe { map.write(BrokerPidMap::new()) };
    Some(unsafe { &*map })
}

fn native_console_control_broker(pid_map: &'static BrokerPidMap, root_descriptor: usize) -> isize {
    const PR_SET_PDEATHSIG: usize = 1;
    const CONTROL_FD: usize = 10;
    let child = unsafe { syscall0(SYS_FORK) };
    if child != 0 {
        return child;
    }
    if unsafe { syscall2(SYS_PRCTL, PR_SET_PDEATHSIG, SIGKILL) } < 0 {
        exit_process(130);
    }
    let _ = unsafe { syscall1(SYS_CLOSE_HOST, root_descriptor) };
    let mut command = 0u8;
    loop {
        let read = unsafe {
            syscall3(
                SYS_READ_HOST,
                CONTROL_FD,
                &mut command as *mut u8 as usize,
                1,
            )
        };
        if read != 1 {
            exit_process(131);
        }
        if command == b'I' {
            let _ = pid_map.signal_foreground(SIGINT);
        } else if command == b'S' {
            let _ = pid_map.signal_foreground(SIGTSTP);
        }
    }
}

fn native_duplicate_listener(pid: usize, descriptor: usize) -> isize {
    let pidfd = unsafe { syscall2(SYS_PIDFD_OPEN, pid, 0) };
    if pidfd < 0 {
        return pidfd;
    }
    let listener = unsafe { syscall3(SYS_PIDFD_GETFD, pidfd as usize, descriptor, 0) };
    let _ = unsafe { syscall1(SYS_CLOSE_HOST, pidfd as usize) };
    listener
}

fn native_yield_broker(
    listener: usize,
    audit_mode: AuditMode,
    instance_id: &[u8],
    contract: &EffectiveContract,
    pid_map: &BrokerPidMap,
) -> ! {
    const AUDIT_EVENT_LIMIT: u64 = 8192;
    let _ = unsafe { syscall5(SYS_PRCTL, PR_SET_PDEATHSIG, SIGKILL, 0, 0, 0) };
    let rule = contract
        .syscall_rule(24)
        .filter(|rule| matches!(rule.action, ContractAction::Mediate))
        .filter(|_| matches!(decide_systemd(24, false).action, Action::Mediate))
        .unwrap_or_else(|| exit_process(130));
    let mut budget = AuditBudget::new(AUDIT_EVENT_LIMIT);
    loop {
        let mut notification = SeccompNotification {
            id: 0,
            pid: 0,
            flags: 0,
            data: SeccompData {
                nr: 0,
                arch: 0,
                instruction_pointer: 0,
                args: [0; 6],
            },
        };
        let received = unsafe {
            syscall3(
                SYS_IOCTL_HOST,
                listener,
                SECCOMP_IOCTL_NOTIF_RECV,
                &mut notification as *mut _ as usize,
            )
        };
        if received < 0 {
            exit_process(0);
        }
        if notification.data.nr != 24 {
            exit_process(130);
        }
        let Some(virtual_pid) = pid_map.virtual_pid(notification.pid) else {
            exit_process(133);
        };
        match if audit_mode == AuditMode::Disabled {
            AuditDisposition::Drop
        } else {
            budget.admit()
        } {
            AuditDisposition::Emit => {
                let mut record = [0u8; 512];
                let mut length = 0usize;
                if native_buffer_append(&mut record, &mut length, b"GINUX_NATIVE_AUDIT: sequence=")
                    .is_none()
                    || native_buffer_append_u64(&mut record, &mut length, notification.id).is_none()
                    || native_buffer_append(&mut record, &mut length, b" instance=").is_none()
                    || native_buffer_append(&mut record, &mut length, instance_id).is_none()
                    || native_buffer_append(&mut record, &mut length, b" virtual-pid=").is_none()
                    || native_buffer_append_u64(&mut record, &mut length, u64::from(virtual_pid))
                        .is_none()
                    || native_buffer_append(&mut record, &mut length, b" contract=").is_none()
                    || native_buffer_append(&mut record, &mut length, contract.digest()).is_none()
                    || native_buffer_append(
                        &mut record,
                        &mut length,
                        b" operation=syscall.sched_yield policy-source=",
                    )
                    .is_none()
                    || native_buffer_append(&mut record, &mut length, rule.source.as_bytes())
                        .is_none()
                    || native_buffer_append(&mut record, &mut length, b" policy-rule=").is_none()
                    || native_buffer_append_u64(&mut record, &mut length, rule.source_rule as u64)
                        .is_none()
                    || native_buffer_append(
                        &mut record,
                        &mut length,
                        b" phase=entry syscall=24 action=mediate surface=task-yield result=0\n",
                    )
                    .is_none()
                {
                    exit_process(131);
                }
                pid_map.lock_audit();
                write_all_to(2, &record[..length]);
                pid_map.unlock_audit();
            }
            AuditDisposition::Drop => {}
            AuditDisposition::DropAndReport(dropped) => {
                pid_map.lock_audit();
                write_all_to(2, b"GINUX_AUDIT_DROPPED: instance=");
                write_all_to(2, instance_id);
                write_all_to(2, b" contract=");
                write_all_to(2, contract.digest());
                write_all_to(2, b" limit=");
                write_u64_to(2, AUDIT_EVENT_LIMIT);
                write_all_to(2, b" dropped=");
                write_u64_to(2, dropped);
                write_all_to(2, b"\n");
                pid_map.unlock_audit();
            }
        }
        let response = SeccompNotificationResponse {
            id: notification.id,
            value: 0,
            error: 0,
            // Receiving the notification has already descheduled the caller
            // and let other runnable work proceed. Return Linux's defined
            // success result directly instead of executing a second host
            // sched_yield after the mediated yield has completed.
            flags: 0,
        };
        if unsafe {
            syscall3(
                SYS_IOCTL_HOST,
                listener,
                SECCOMP_IOCTL_NOTIF_SEND,
                &response as *const _ as usize,
            )
        } < 0
        {
            exit_process(132);
        }
    }
}

fn supervise_native(
    pid: usize,
    instance: &mut Instance,
    mode: usize,
    root_descriptor: usize,
    contract: EffectiveContract,
    bootstrap_phase: bool,
    direct_yield: bool,
) -> ! {
    // The host pseudo-terminal process group also contains this supervisor.
    // Ctrl-C must interrupt the attached instance task without terminating
    // Ginux itself. The tracee was forked before this point, so blocking
    // SIGINT here affects only the supervisor; instance tasks retain normal
    // terminal interrupt semantics.
    let interrupt_mask = (1u64 << (SIGINT - 1)) | (1u64 << (SIGTSTP - 1));
    if unsafe {
        syscall4(
            SYS_RT_SIGPROCMASK,
            SIG_BLOCK,
            &interrupt_mask as *const _ as usize,
            0,
            core::mem::size_of::<u64>(),
        )
    } < 0
    {
        native_fail(instance, pid, b"native supervisor SIGINT mask failed");
    }
    const AUDIT_EVENT_LIMIT: u64 = 8192;
    const AUDIT_PREFIX: &[u8] = b"GINUX_NATIVE_AUDIT: sequence=";
    let audit_mode = instance.config().audit_mode;
    let audit_instance_id = instance.config().id;
    let audit_budget = Cell::new(AuditBudget::new(AUDIT_EVENT_LIMIT));
    let suppress_audit_record = Cell::new(false);
    let audit_locked = Cell::new(false);
    let pid_map = native_shared_pid_map()
        .unwrap_or_else(|| native_fail(instance, pid, b"native broker PID map failed"));
    let write_all = |bytes: &[u8]| {
        if bytes == AUDIT_PREFIX {
            let disposition = if audit_mode == AuditMode::Disabled {
                AuditDisposition::Drop
            } else {
                let mut budget = audit_budget.get();
                let disposition = budget.admit();
                audit_budget.set(budget);
                disposition
            };
            match disposition {
                AuditDisposition::Emit => {
                    pid_map.lock_audit();
                    audit_locked.set(true);
                    suppress_audit_record.set(false);
                }
                AuditDisposition::Drop => suppress_audit_record.set(true),
                AuditDisposition::DropAndReport(dropped) => {
                    suppress_audit_record.set(true);
                    pid_map.lock_audit();
                    write_all_to(2, b"GINUX_AUDIT_DROPPED: instance=");
                    write_all_to(2, audit_instance_id.as_bytes());
                    write_all_to(2, b" contract=");
                    write_all_to(2, contract.digest());
                    write_all_to(2, b" limit=");
                    write_u64_to(2, AUDIT_EVENT_LIMIT);
                    write_all_to(2, b" dropped=");
                    write_u64_to(2, dropped);
                    write_all_to(2, b"\n");
                    pid_map.unlock_audit();
                }
            }
        }
        if !suppress_audit_record.get() {
            write_all_to(2, bytes);
        }
        if bytes.ends_with(b"\n") {
            if audit_locked.replace(false) {
                pid_map.unlock_audit();
            }
            suppress_audit_record.set(false);
        }
    };
    let write_u64 = |value: u64| {
        if !suppress_audit_record.get() {
            write_u64_to(2, value);
        }
    };
    let write_i64 = |value: i64| {
        if !suppress_audit_record.get() {
            write_i64_to(2, value);
        }
    };
    let write_tracee_string = |pid: usize, address: usize| {
        if !suppress_audit_record.get() {
            write_tracee_string_to(2, pid, address);
        }
    };
    let line = |message: &[u8]| {
        write_all(message);
        write_all(b"\n");
    };
    let instance_start_ns = clock_nanoseconds(CLOCK_MONOTONIC);
    let instance_start_realtime_seconds = clock_nanoseconds(CLOCK_REALTIME) / 1_000_000_000;

    let mut status = 0i32;
    if wait(pid, &mut status) < 0 {
        native_fail(instance, pid, b"native tracee wait after exec failed");
    }
    if !stopped(status) || ((status >> 8) & 0xff) as usize != SIGSTOP {
        write_all(b"GINUX_NATIVE: pre-filter-status=");
        write_i64(status as i64);
        write_all(b"\n");
        native_fail(
            instance,
            pid,
            b"native tracee did not stop before filter install",
        );
    }
    if mode == 3 && !native_limit_cpus(pid, instance.config().vcpus) {
        native_fail(instance, pid, b"native vcpu affinity failed");
    }
    let options = PTRACE_O_TRACESYSGOOD
        | PTRACE_O_TRACEFORK
        | PTRACE_O_TRACEVFORK
        | PTRACE_O_TRACECLONE
        | PTRACE_O_TRACEEXEC
        | PTRACE_O_TRACESECCOMP
        | PTRACE_O_EXITKILL;
    if ptrace(PTRACE_SETOPTIONS, pid, 0, options) < 0 {
        native_fail(instance, pid, b"native ptrace setup failed");
    }
    if ptrace(PTRACE_CONT, pid, 0, 0) < 0 {
        native_fail(instance, pid, b"native ptrace resume failed");
    }
    if mode == 3 {
        write_all(
            b"GINUX_NATIVE: backend=ptrace-native profile=balanced elf-loader=host contract=",
        );
        line(contract_label(instance.config()));
        line(b"GINUX_NATIVE: virtual-pid=1 virtual-ppid=0 root=oci-overlay");
    } else {
        write_all(
            b"GINUX_NATIVE: backend=ptrace-native profile=balanced elf-loader=host contract=",
        );
        line(contract_label(instance.config()));
        // The same decision `native_tracee` applied before exec.
        let identity = managed_host_identity();
        write_all(b"GINUX_NATIVE: tracee-uid=");
        write_u64(identity.uid as u64);
        write_all(b" tracee-gid=");
        write_u64(identity.gid as u64);
        line(b" root=oci-read-only");
    }

    let mut sequence = 0u64;
    let mut processes: ProcessTable<128> = ProcessTable::new();
    let virtual_credentials = if mode == 3 {
        if instance.config().access_uid == 0 && instance.config().access_gid == 0 {
            Credentials::root()
        } else {
            Credentials::unprivileged(
                instance.config().access_uid as u32,
                instance.config().access_gid as u32,
            )
        }
    } else {
        Credentials::unprivileged(1000, 1000)
    };
    let root_virtual = processes
        .create_init(virtual_credentials)
        .unwrap_or_else(|_| native_fail(instance, pid, b"native virtual init failed"));
    let root_view = processes
        .view(root_virtual)
        .unwrap_or_else(|_| native_fail(instance, pid, b"native root process view failed"));
    if !pid_map.set(pid, root_virtual.pid, root_view.process_group) {
        native_fail(instance, pid, b"native broker PID map full");
    }
    pid_map.set_foreground_group(root_view.process_group);
    let control_available = unsafe { syscall2(SYS_FCNTL as usize, 10, F_GETFD as usize) } >= 0;
    let mut control_broker_pid = 0isize;
    let mut tasks: [Option<NativeSupervisorTask>; 128] = [None; 128];
    let mut retired_credentials = [EMPTY_RETIRED_CREDENTIAL; 128];
    let mut next_retired_credential = 0usize;
    let mut virtual_kernel_fds = [EMPTY_VIRTUAL_KERNEL_FD; NATIVE_VIRTUAL_KERNEL_FD_LIMIT];
    let mut virtual_kernel_state = NativeVirtualKernelState::new();
    let mut pending_child_stops = [0usize; 16];
    let mut peak_managed_tasks = 1usize;
    let mut yield_broker_pid = 0usize;
    let mut root_console_fds = [0u64; NATIVE_PROC_FD_LIMIT / 64];
    for descriptor in [0, 1, 2, INSTANCE_CONSOLE_FD] {
        if !native_fd_set(&mut root_console_fds, descriptor) {
            native_fail(instance, pid, b"native console descriptor table full");
        }
    }
    tasks[0] = Some(NativeSupervisorTask {
        host_pid: pid,
        clone_call: native_clone::Pending::empty(),
        lifetime_pidfd: -1,
        virtual_id: root_virtual,
        start_ticks: 0,
        awaiting_exit: false,
        override_exit: false,
        override_result: 0,
        pending_console_open: false,
        console_fd: INSTANCE_CONSOLE_FD as i64,
        console_fds: root_console_fds,
        pending_console_duplicate: 0,
        pipe2_address: 0,
        pipe_fds: [-1; 8],
        socket_fds: [EMPTY_SOCKET_FD; NATIVE_SOCKET_FD_LIMIT],
        pending_socket_type: 0,
        pending_sockaddr: 0,
        pending_sockaddr_word: 0,
        pending_getsockname: 0,
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
        wait_status: 0,
        waitid_info: 0,
        waitid_selector: 0,
        initial_stop_pending: false,
        initial_exec_pending: true,
        job_stop_pending: false,
        job_stopped: false,
        tracing: true,
        limits: native_default_limits(),
    });
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
            native_fail(instance, pid, b"native wait-any failed");
        }
        let host_pid = waited as usize;
        if host_pid == yield_broker_pid {
            let exit_status = if status & 0x7f == 0 {
                status >> 8
            } else {
                128 + (status & 0x7f)
            };
            yield_broker_pid = 0;
            if exit_status != 0 {
                native_fail(instance, pid, b"native yield broker failed");
            }
            continue;
        }
        if control_broker_pid > 0 && host_pid == control_broker_pid as usize {
            native_fail(instance, pid, b"native console control broker exited");
        }
        let task_index = tasks
            .iter()
            .position(|task| task.is_some_and(|task| task.host_pid == host_pid));
        let Some(task_index) = task_index else {
            let initial_child_stop = stopped(status) && ((status >> 8) & 0xff) as u8 == 19;
            let Some(pending_slot) = pending_child_stops.iter_mut().find(|child| **child == 0)
            else {
                native_fail(instance, pid, b"pending native child table full");
            };
            if initial_child_stop {
                *pending_slot = host_pid;
                continue;
            }
            native_fail(instance, pid, b"unknown native tracee");
        };
        let mut task = tasks[task_index].unwrap();
        if !stopped(status) {
            if task.clone_call.phase != native_clone::Phase::Idle {
                // A dead caller cannot finish its private protocol. Terminate
                // the instance rather than leave a shared-VM snapshot behind.
                native_fail(
                    instance,
                    pid,
                    b"native clone interrupted during private setup",
                );
            }
            let exit_status = if status & 0x7f == 0 {
                status >> 8
            } else {
                128 + (status & 0x7f)
            };
            if processes.exit(task.virtual_id, exit_status).is_err() {
                native_fail(instance, pid, b"native virtual exit failed");
            }
            let parent_virtual_pid = processes
                .view(task.virtual_id)
                .map(|view| view.parent)
                .unwrap_or(0);
            pid_map.remove(task.host_pid);
            native_virtual_kernel_clear_owner(&mut virtual_kernel_fds, task.host_pid);
            task.tracing = false;
            tasks[task_index] = Some(task);
            // Linux may report the parent's SIGCHLD delivery stop before the
            // tracer receives the child's final exit status. That early
            // notification is suppressed below because the virtual child is
            // not reportable yet. Queue a replacement only after recording
            // the exit, so an asynchronous shell handler can reap it.
            if parent_virtual_pid != 0 {
                if let Some(parent) = tasks.iter().flatten().find(|candidate| {
                    candidate.virtual_id.pid == parent_virtual_pid && candidate.tracing
                }) {
                    let _ = unsafe { syscall2(SYS_KILL, parent.host_pid, SIGCHLD) };
                }
            }
            if task.virtual_id == root_virtual {
                for descendant in tasks.iter().flatten() {
                    if descendant.tracing {
                        let _ = ptrace(PTRACE_KILL, descendant.host_pid, 0, 0);
                    }
                }
                if yield_broker_pid != 0 {
                    let _ = unsafe { syscall2(SYS_KILL, yield_broker_pid, SIGKILL) };
                    let mut broker_status = 0i32;
                    let _ = wait(yield_broker_pid, &mut broker_status);
                }
                if control_broker_pid > 0 {
                    let _ = unsafe { syscall2(SYS_KILL, control_broker_pid as usize, SIGKILL) };
                    let mut control_status = 0i32;
                    let _ = wait(control_broker_pid as usize, &mut control_status);
                }
                if instance.task_exited(exit_status == 0).is_err() {
                    instance_fail(instance, b"native exit transition failed");
                }
                if direct_yield {
                    line(b"GINUX_NATIVE: supervisor=attached dispatch=selective direct-safe-syscalls=1");
                } else {
                    line(b"GINUX_NATIVE: supervisor=attached dispatch=paranoid direct-safe-syscalls=0");
                }
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
        // With PTRACE_O_TRACESYSGOOD every syscall stop reports SIGTRAP | 0x80.
        // Any other stop, such as a plain SIGTRAP from int3, would otherwise
        // be taken as a syscall stop and swap entry and exit tracking.
        let signal = ((status >> 8) & 0xff) as u8;
        let event = ((status as usize) >> 16) & 0xffff;
        let seccomp_entry = event == PTRACE_EVENT_SECCOMP;
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
                native_fail(instance, pid, b"native child event read failed");
            }
            let child_virtual = if event == PTRACE_EVENT_CLONE {
                processes.clone_thread(task.virtual_id)
            } else {
                processes.fork(task.virtual_id)
            }
            .unwrap_or_else(|_| native_fail(instance, pid, b"native virtual child failed"));
            let child_view = processes.view(child_virtual).unwrap_or_else(|_| {
                native_fail(instance, pid, b"native virtual child view failed")
            });
            if !pid_map.set(child_host_pid, child_virtual.pid, child_view.process_group) {
                native_fail(instance, pid, b"native broker PID map full");
            }
            let child_start_ticks =
                clock_nanoseconds(CLOCK_MONOTONIC).saturating_sub(instance_start_ns) / 10_000_000;
            if mode == 3
                && !native_virtual_kernel_fork(&mut virtual_kernel_fds, host_pid, child_host_pid)
            {
                native_fail(instance, pid, b"native virtual descriptor table full");
            }
            let Some(child_slot) = tasks.iter().position(Option::is_none) else {
                native_fail(instance, pid, b"native task table full");
            };
            let pending_child_slot = pending_child_stops
                .iter()
                .position(|child| *child == child_host_pid);
            // Pin this exact host task before it can leave its initial stop.
            // POLLHUP distinguishes final reaping from an exited zombie and
            // remains meaningful even if the numeric host PID is reused.
            let mut lifetime_pidfd = unsafe { syscall2(SYS_PIDFD_OPEN, child_host_pid, 0) };
            // A non-leader thread is not waitid-reapable and older pidfd
            // ABIs cannot open it. Preserve that existing thread lane.
            if event == PTRACE_EVENT_CLONE && matches!(lifetime_pidfd, -22 | -3) {
                lifetime_pidfd = -1;
            } else if lifetime_pidfd < 0 {
                native_fail(instance, pid, b"native child lifetime pin failed");
            }
            tasks[child_slot] = Some(NativeSupervisorTask {
                host_pid: child_host_pid,
                clone_call: if task.clone_call.phase == native_clone::Phase::Call {
                    task.clone_call.child()
                } else {
                    native_clone::Pending::empty()
                },
                lifetime_pidfd,
                virtual_id: child_virtual,
                start_ticks: child_start_ticks,
                awaiting_exit: false,
                override_exit: false,
                override_result: 0,
                pending_console_open: false,
                console_fd: task.console_fd,
                console_fds: task.console_fds,
                pending_console_duplicate: 0,
                pipe2_address: 0,
                pipe_fds: task.pipe_fds,
                socket_fds: task.socket_fds,
                pending_socket_type: 0,
                pending_sockaddr: 0,
                pending_sockaddr_word: 0,
                pending_getsockname: 0,
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
                wait_status: 0,
                waitid_info: 0,
                waitid_selector: 0,
                initial_stop_pending: pending_child_slot.is_none(),
                initial_exec_pending: false,
                job_stop_pending: false,
                job_stopped: false,
                tracing: true,
                limits: task.limits,
            });
            peak_managed_tasks = core::cmp::max(peak_managed_tasks, processes.active());
            if let Some(pending_slot) = pending_child_slot {
                pending_child_stops[pending_slot] = 0;
                let _ = ptrace(PTRACE_SETOPTIONS, child_host_pid, 0, options);
                if let Some(child) = tasks[child_slot].as_mut() {
                    native_clone_start_child(child).unwrap_or_else(|| {
                        native_fail(instance, pid, b"native child clone cleanup failed")
                    });
                }
                if ptrace(PTRACE_CONT, child_host_pid, 0, 0) < 0 {
                    native_fail(instance, pid, b"pending native child resume failed");
                }
            }
            // Translate the host child ID at the parent's syscall-exit stop.
            // A software breakpoint in the parent's text is unsafe here:
            // multithreaded workloads share that text, so an unrelated
            // sibling can hit the breakpoint and bring down the instance.
            task.awaiting_exit = true;
            task.override_exit = true;
            task.override_result = child_virtual.pid as i64;
            tasks[task_index] = Some(task);
            if ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0 {
                native_fail(instance, pid, b"native parent resume after child failed");
            }
            continue;
        }
        if event == PTRACE_EVENT_EXEC {
            let _ = processes.exec(task.virtual_id);
            if mode == 3 {
                native_virtual_kernel_exec(&mut virtual_kernel_fds, host_pid);
            }
            tasks[task_index] = Some(task);
            if ptrace(PTRACE_CONT, host_pid, 0, 0) < 0 {
                native_fail(instance, pid, b"native resume after exec failed");
            }
            continue;
        }
        if event != 0 && !seccomp_entry {
            native_fail(instance, pid, b"unexpected native ptrace event");
        }
        if signal == 19 {
            if task.job_stop_pending {
                task.job_stop_pending = false;
                task.job_stopped = true;
                tasks[task_index] = Some(task);
                continue;
            }
            if !task.initial_stop_pending {
                native_fail(instance, pid, b"unexpected native trace stop");
            }
            task.initial_stop_pending = false;
            native_clone_start_child(&mut task).unwrap_or_else(|| {
                native_fail(instance, pid, b"native child clone cleanup failed")
            });
            tasks[task_index] = Some(task);
            let _ = ptrace(PTRACE_SETOPTIONS, host_pid, 0, options);
            if ptrace(PTRACE_CONT, host_pid, 0, 0) < 0 {
                native_fail(instance, pid, b"native child initial resume failed");
            }
            continue;
        }
        if signal == 17 && task.clone_call.phase != native_clone::Phase::Idle {
            task.clone_call.sigchld |= tasks.iter().flatten().any(|candidate| {
                !candidate.tracing
                    && processes.view(candidate.virtual_id).is_ok_and(|view| {
                        view.parent == task.virtual_id.pid
                            && matches!(view.state, TaskState::Exited(_) | TaskState::Signaled(_))
                    })
            });
            let request = if task.awaiting_exit {
                PTRACE_SYSCALL
            } else {
                PTRACE_CONT
            };
            tasks[task_index] = Some(task);
            if ptrace(request, host_pid, 0, 0) < 0 {
                native_fail(instance, pid, b"native clone signal deferral failed");
            }
            continue;
        }
        if signal == 17 {
            let reportable_child = tasks.iter().flatten().any(|candidate| {
                !candidate.tracing
                    && processes.view(candidate.virtual_id).is_ok_and(|view| {
                        view.parent == task.virtual_id.pid
                            && matches!(view.state, TaskState::Exited(_) | TaskState::Signaled(_))
                    })
            });
            // Suppress a host notification that raced ahead of Ginux's
            // virtual exit record. The exit path queues a replacement after
            // the child is reportable.
            let delivered_signal = if reportable_child { SIGCHLD } else { 0 };
            // Preserve the hybrid loop's phase: a userspace delivery stop
            // has no syscall exit to collect, while a blocked call does.
            let request = if task.awaiting_exit {
                PTRACE_SYSCALL
            } else {
                PTRACE_CONT
            };
            if ptrace(request, host_pid, 0, delivered_signal) < 0 {
                native_fail(instance, pid, b"native SIGCHLD resume failed");
            }
            continue;
        }
        if signal as usize == SIGTSTP && pid_map.take_suspend_request(host_pid) {
            task.job_stop_pending = true;
            tasks[task_index] = Some(task);
            let request = if task.awaiting_exit {
                PTRACE_SYSCALL
            } else {
                PTRACE_CONT
            };
            if ptrace(request, host_pid, 0, SIGSTOP) < 0 {
                native_fail(instance, pid, b"native foreground suspend failed");
            }
            continue;
        }
        if signal as usize == SIGCONT && task.job_stopped {
            task.job_stopped = false;
            tasks[task_index] = Some(task);
            let request = if task.awaiting_exit {
                PTRACE_SYSCALL
            } else {
                PTRACE_CONT
            };
            if ptrace(request, host_pid, 0, SIGCONT) < 0 {
                native_fail(instance, pid, b"native foreground resume failed");
            }
            continue;
        }
        if signal == 5 && !seccomp_entry {
            native_fail(instance, pid, b"unexpected native trace stop");
        }
        if !seccomp_entry && signal != (5 | 0x80) {
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
            if ptrace(PTRACE_CONT, host_pid, 0, signal as usize) < 0 {
                native_fail(instance, pid, b"native signal delivery failed");
            }
            continue;
        }
        let mut entry_registers = None;
        let entry_syscall = if seccomp_entry {
            let mut number = 0usize;
            if ptrace(
                PTRACE_GETEVENTMSG,
                host_pid,
                0,
                &mut number as *mut _ as usize,
            ) < 0
                || number == 0xffff
            {
                native_fail(instance, pid, b"native requires the x86-64 syscall ABI");
            }
            let mut registers = core::mem::MaybeUninit::<UserRegs>::uninit();
            if ptrace(PTRACE_GETREGS, host_pid, 0, registers.as_mut_ptr() as usize) < 0 {
                native_fail(instance, pid, b"native register read failed");
            }
            let registers = unsafe { registers.assume_init() };
            let syscall = registers.orig_rax;
            entry_registers = Some(registers);
            Some(syscall)
        } else {
            None
        };

        if task.initial_exec_pending && entry_syscall == Some(59) {
            let mut listener = -1isize;
            if !direct_yield {
                listener = native_duplicate_listener(host_pid, 3);
                if listener < 0 {
                    native_fail(instance, pid, b"native seccomp listener duplication failed");
                }
            }
            reduce_native_supervisor_authority(
                instance,
                pid,
                root_descriptor,
                if listener >= 0 {
                    listener as usize
                } else {
                    usize::MAX
                },
            );
            if control_available {
                control_broker_pid = native_console_control_broker(pid_map, root_descriptor);
                if control_broker_pid < 0 {
                    native_fail(instance, pid, b"native console control broker failed");
                }
            }
            if !direct_yield {
                let broker = unsafe { syscall0(SYS_FORK) };
                if broker < 0 {
                    let _ = unsafe { syscall1(SYS_CLOSE_HOST, listener as usize) };
                    native_fail(instance, pid, b"native yield broker fork failed");
                }
                if broker == 0 {
                    let _ = unsafe { syscall1(SYS_CLOSE_HOST, root_descriptor) };
                    let _ = unsafe { syscall1(SYS_CLOSE_HOST, 10) };
                    native_yield_broker(
                        listener as usize,
                        audit_mode,
                        audit_instance_id.as_bytes(),
                        &contract,
                        pid_map,
                    );
                }
                yield_broker_pid = broker as usize;
                let _ = unsafe { syscall1(SYS_CLOSE_HOST, listener as usize) };
            }
            sequence += 1;
            task.initial_exec_pending = false;
            write_all(b"GINUX_NATIVE_AUDIT: sequence=");
            write_u64(sequence);
            write_all(b" instance=");
            write_all(audit_instance_id.as_bytes());
            write_all(b" virtual-pid=");
            write_u64(task.virtual_id.pid as u64);
            write_all(b" contract=");
            write_all(contract.digest());
            line(b" operation=launch.configured-init policy-source=launch phase=entry syscall=59 action=mediate surface=configured-init result=pending");
            tasks[task_index] = Some(task);
            if ptrace(PTRACE_CONT, host_pid, 0, 0) < 0 {
                native_fail(instance, pid, b"native configured init resume failed");
            }
            continue;
        }

        if entry_syscall == Some(24) {
            let rule = contract
                .syscall_rule(24)
                .filter(|rule| matches!(rule.action, ContractAction::Mediate));
            if rule.is_none() || !matches!(decide_systemd(24, false).action, Action::Mediate) {
                native_fail(instance, pid, b"native yield contract mismatch");
            }
            sequence += 1;
            write_all(AUDIT_PREFIX);
            let mut record = [0u8; 512];
            let mut length = 0usize;
            let rule = rule.unwrap();
            if native_buffer_append_u64(&mut record, &mut length, sequence).is_none()
                || native_buffer_append(&mut record, &mut length, b" instance=").is_none()
                || native_buffer_append(&mut record, &mut length, audit_instance_id.as_bytes())
                    .is_none()
                || native_buffer_append(&mut record, &mut length, b" virtual-pid=").is_none()
                || native_buffer_append_u64(&mut record, &mut length, task.virtual_id.pid as u64)
                    .is_none()
                || native_buffer_append(&mut record, &mut length, b" contract=").is_none()
                || native_buffer_append(&mut record, &mut length, contract.digest()).is_none()
                || native_buffer_append(
                    &mut record,
                    &mut length,
                    b" operation=syscall.sched_yield policy-source=",
                )
                .is_none()
                || native_buffer_append(&mut record, &mut length, rule.source.as_bytes()).is_none()
                || native_buffer_append(&mut record, &mut length, b" policy-rule=").is_none()
                || native_buffer_append_u64(&mut record, &mut length, rule.source_rule as u64)
                    .is_none()
                || native_buffer_append(
                    &mut record,
                    &mut length,
                    b" phase=entry syscall=24 action=mediate surface=task-yield result=0\n",
                )
                .is_none()
            {
                native_fail(instance, pid, b"native yield audit overflow");
            }
            write_all(&record[..length]);
            tasks[task_index] = Some(task);
            if ptrace(PTRACE_CONT, host_pid, 0, 0) < 0 {
                native_fail(instance, pid, b"native yield resume failed");
            }
            continue;
        }
        let mut registers = if let Some(registers) = entry_registers {
            registers
        } else {
            let mut registers = core::mem::MaybeUninit::<UserRegs>::uninit();
            if ptrace(PTRACE_GETREGS, host_pid, 0, registers.as_mut_ptr() as usize) < 0 {
                native_fail(instance, pid, b"native register read failed");
            }
            unsafe { registers.assume_init() }
        };

        if seccomp_entry
            && matches!(
                task.clone_call.phase,
                native_clone::Phase::Call | native_clone::Phase::Unmap
            )
        {
            if !task.clone_call.matches_entry(&registers) {
                native_fail(instance, pid, b"native clone replay mismatch");
            }
            task.awaiting_exit = true;
            tasks[task_index] = Some(task);
            if ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0 {
                native_fail(instance, pid, b"native clone replay resume failed");
            }
            continue;
        }
        if !seccomp_entry && !task.awaiting_exit && registers.orig_rax == u64::MAX {
            tasks[task_index] = Some(task);
            if ptrace(PTRACE_CONT, host_pid, 0, 0) < 0 {
                native_fail(instance, pid, b"native child phase synchronization failed");
            }
            continue;
        }

        if !seccomp_entry && task.awaiting_exit {
            use native_clone::Phase;
            match task.clone_call.phase {
                Phase::Map if registers.rax as i64 >= 0 => {
                    task.clone_call
                        .mapped(host_pid, &mut registers)
                        .unwrap_or_else(|_| {
                            native_fail(instance, pid, b"native clone snapshot write failed")
                        });
                    task.awaiting_exit = false;
                    tasks[task_index] = Some(task);
                    if ptrace(PTRACE_SETREGS, host_pid, 0, &registers as *const _ as usize) < 0
                        || ptrace(PTRACE_CONT, host_pid, 0, 0) < 0
                    {
                        native_fail(instance, pid, b"native clone snapshot resume failed");
                    }
                    continue;
                }
                Phase::Call => {
                    let result = if task.override_exit {
                        task.override_result
                    } else {
                        registers.rax as i64
                    };
                    task.clone_call.unmap(&mut registers, result);
                    task.awaiting_exit = false;
                    task.override_exit = false;
                    tasks[task_index] = Some(task);
                    if ptrace(PTRACE_SETREGS, host_pid, 0, &registers as *const _ as usize) < 0
                        || ptrace(PTRACE_CONT, host_pid, 0, 0) < 0
                    {
                        native_fail(instance, pid, b"native clone cleanup resume failed");
                    }
                    continue;
                }
                Phase::Map | Phase::Unmap => {
                    if task.clone_call.phase == Phase::Map {
                        task.clone_call.result = registers.rax as i64;
                    } else if registers.rax as i64 != 0 {
                        native_fail(instance, pid, b"native clone snapshot cleanup failed");
                    }
                    task.clone_call.restore(&mut registers);
                    task.override_exit = true;
                    task.override_result = registers.rax as i64;
                }
                _ => {}
            }
            if task.syscall == 247 && registers.rax as i64 == 0 && task.waitid_info != 0 {
                let mut identity = 0u64;
                if ptrace(
                    PTRACE_PEEKDATA,
                    host_pid,
                    task.waitid_info as usize + 16,
                    &mut identity as *mut _ as usize,
                ) < 0
                {
                    native_fail(instance, pid, b"native waitid result read failed");
                }
                let waited_host = identity as u32 as usize;
                if waited_host != 0 {
                    let Some(waited_task) = tasks
                        .iter()
                        .flatten()
                        .find(|candidate| candidate.host_pid == waited_host)
                        .copied()
                    else {
                        native_fail(instance, pid, b"native waitid result unknown");
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
                        native_fail(instance, pid, b"native waitid result translation failed");
                    }
                }
            }
            if task.syscall == 247 {
                for slot in 0..tasks.len() {
                    let Some(child) = tasks[slot] else {
                        continue;
                    };
                    if child.tracing || child.lifetime_pidfd < 0 {
                        continue;
                    }
                    let state = native_pidfd_reaped(child.lifetime_pidfd).unwrap_or_else(|| {
                        native_fail(instance, pid, b"native child lifetime check failed")
                    });
                    if !state {
                        continue;
                    }
                    let parent = processes
                        .view(child.virtual_id)
                        .ok()
                        .and_then(|view| {
                            tasks
                                .iter()
                                .flatten()
                                .find(|candidate| candidate.virtual_id.pid == view.parent)
                        })
                        .map(|parent| parent.virtual_id)
                        .unwrap_or_else(|| {
                            native_fail(instance, pid, b"native waitid parent unknown")
                        });
                    if !native_retire_credential(
                        child,
                        &processes,
                        &mut retired_credentials,
                        &mut next_retired_credential,
                    ) {
                        native_fail(instance, pid, b"native waitid credential retirement failed");
                    }
                    if processes
                        .wait(parent, WaitTarget::Specific(child.virtual_id.pid), 0)
                        .is_err()
                    {
                        native_fail(instance, pid, b"native virtual waitid result invalid");
                    }
                    tasks[slot] = None;
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
                    native_fail(instance, pid, b"native wait result unknown");
                };
                let wait_status = if task.wait_status == 0 {
                    0
                } else {
                    read_tracee_u32(host_pid, task.wait_status as usize).unwrap_or_else(|| {
                        native_fail(instance, pid, b"native wait status read failed")
                    })
                };
                let child_state_only =
                    waited_task.job_stopped || wait_status & 0xff == 0x7f || wait_status == 0xffff;
                if !child_state_only
                    && !native_retire_credential(
                        waited_task,
                        &processes,
                        &mut retired_credentials,
                        &mut next_retired_credential,
                    )
                {
                    native_fail(instance, pid, b"native wait credential retirement failed");
                }
                if !child_state_only
                    && processes
                        .wait(
                            task.virtual_id,
                            WaitTarget::Specific(waited_task.virtual_id.pid),
                            0,
                        )
                        .is_err()
                {
                    native_fail(instance, pid, b"native virtual wait result invalid");
                }
                registers.rax = waited_task.virtual_id.pid as u64;
                task.override_exit = true;
                if !child_state_only {
                    if let Some(waited_slot) = tasks.iter_mut().find(|candidate| {
                        candidate.is_some_and(|candidate| candidate.host_pid == waited_host)
                    }) {
                        *waited_slot = None;
                    }
                }
            }
            if task.syscall == 247 {
                // P_PID translates RSI internally; return its original ABI
                // value even on errors/restarts, rather than a host PID.
                registers.rsi = task.waitid_selector;
                if !task.override_exit {
                    task.override_exit = true;
                    task.override_result = registers.rax as i64;
                }
            }
            if task.translated_rdi != 0 {
                // Like RSI above: the guest keeps its virtual selector, and a
                // restarted call is translated again at its next entry.
                registers.rdi = task.translated_rdi;
                if !task.override_exit {
                    task.override_exit = true;
                    task.override_result = registers.rax as i64;
                }
            }
            if task.override_exit {
                if task.syscall != 61 || registers.rax as i64 <= 0 {
                    registers.rax = task.override_result as u64;
                }
                if ptrace(PTRACE_SETREGS, host_pid, 0, &registers as *const _ as usize) < 0 {
                    native_fail(instance, pid, b"native result override failed");
                }
            }
            write_all(b"GINUX_NATIVE_AUDIT: sequence=");
            write_u64(sequence);
            write_all(b" instance=");
            write_all(audit_instance_id.as_bytes());
            write_all(b" virtual-pid=");
            write_u64(task.virtual_id.pid as u64);
            write_all(b" contract=");
            write_all(contract.digest());
            write_all(b" phase=exit result=");
            write_i64(registers.rax as i64);
            write_all(b"\n");
            if task.pending_console_open {
                task.console_fd = registers.rax as i64;
                if registers.rax as i64 >= 0
                    && !native_fd_set(&mut task.console_fds, registers.rax as usize)
                {
                    native_fail(instance, pid, b"native console descriptor table full");
                }
                task.pending_console_open = false;
            }
            if task.pending_console_duplicate != 0 {
                if registers.rax as i64 >= 0 {
                    let duplicated = registers.rax as usize;
                    native_fd_clear(&mut task.console_fds, duplicated);
                    if task.pending_console_duplicate > 0
                        && !native_fd_set(&mut task.console_fds, duplicated)
                    {
                        native_fail(instance, pid, b"native console descriptor table full");
                    }
                }
                task.pending_console_duplicate = 0;
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
                    native_fail(instance, pid, b"native cgroup descriptor table full");
                }
                task.pending_cgroup_procs_open = false;
            }
            if task.pending_recvmsg != 0 {
                if registers.rax as i64 >= 0
                    && !native_rewrite_recvmsg_credentials(
                        host_pid,
                        task.pending_recvmsg,
                        &tasks,
                        &retired_credentials,
                        task,
                        &processes,
                    )
                {
                    native_fail(
                        instance,
                        pid,
                        b"native recvmsg credential translation failed",
                    );
                }
                task.pending_recvmsg = 0;
            }
            if task.pending_peercred != 0 {
                if registers.rax as i64 == 0
                    && !native_rewrite_ucred(
                        host_pid,
                        task.pending_peercred,
                        &tasks,
                        &retired_credentials,
                        task,
                        &processes,
                    )
                {
                    native_fail(instance, pid, b"native peer credential translation failed");
                }
                task.pending_peercred = 0;
            }
            if task.pending_stat != 0 {
                if registers.rax as i64 == 0 {
                    let view = processes.view(task.virtual_id).unwrap_or_else(|_| {
                        native_fail(instance, pid, b"native stat identity failed")
                    });
                    if !native_rewrite_stat_identity(
                        host_pid,
                        task.pending_stat,
                        view.credentials.uid,
                        view.credentials.gid,
                    ) {
                        native_fail(instance, pid, b"native stat identity translation failed");
                    }
                }
                task.pending_stat = 0;
            }
            if task.syscall == 41 && registers.rax as i64 >= 0 && task.pending_socket_type != 0 {
                let Some(slot) = task.socket_fds.iter_mut().find(|entry| entry.fd < 0) else {
                    native_fail(instance, pid, b"native socket descriptor table full");
                };
                *slot = NativeSocketFd {
                    fd: registers.rax as i64,
                    protocol: task.pending_socket_type,
                    source_bound: false,
                    internal_port: 0,
                    external_port: 0,
                };
            }
            if task.syscall == 3 && registers.rax as i64 == 0 {
                native_fd_clear(&mut task.console_fds, registers.rdi as usize);
                if let Some(slot) = task
                    .socket_fds
                    .iter_mut()
                    .find(|entry| entry.fd == registers.rdi as i64)
                {
                    *slot = EMPTY_SOCKET_FD;
                }
            }
            if task.syscall == 49 && registers.rax as i64 == 0 {
                if let Some(socket) = task
                    .socket_fds
                    .iter_mut()
                    .find(|entry| entry.fd == registers.rdi as i64)
                {
                    socket.source_bound = true;
                }
            }
            if task.pending_sockaddr != 0 {
                if ptrace(
                    PTRACE_POKEDATA,
                    host_pid,
                    task.pending_sockaddr,
                    task.pending_sockaddr_word as usize,
                ) < 0
                {
                    native_fail(instance, pid, b"native translated sockaddr restore failed");
                }
                task.pending_sockaddr = 0;
                task.pending_sockaddr_word = 0;
            }
            if task.pending_getsockname != 0 && registers.rax as i64 == 0 {
                let internal_port = task
                    .socket_fds
                    .iter()
                    .find(|entry| entry.fd == registers.rdi as i64)
                    .map_or(0, |entry| entry.internal_port);
                if internal_port != 0 {
                    let mut word = 0u64;
                    if ptrace(
                        PTRACE_PEEKDATA,
                        host_pid,
                        task.pending_getsockname,
                        &mut word as *mut _ as usize,
                    ) < 0
                        || ptrace(
                            PTRACE_POKEDATA,
                            host_pid,
                            task.pending_getsockname,
                            native_sockaddr_with_port(word, internal_port) as usize,
                        ) < 0
                    {
                        native_fail(instance, pid, b"native getsockname translation failed");
                    }
                }
                task.pending_getsockname = 0;
            }
            if task.syscall == 293 && registers.rax as i64 == 0 && task.pipe2_address != 0 {
                let read_fd = read_tracee_u32(host_pid, task.pipe2_address);
                let write_fd = read_tracee_u32(host_pid, task.pipe2_address + 4);
                let Some((read_fd, write_fd)) = read_fd.zip(write_fd) else {
                    native_fail(instance, pid, b"native pipe2 descriptor read failed");
                };
                for pipe_fd in [i64::from(read_fd), i64::from(write_fd)] {
                    let Some(slot) = task.pipe_fds.iter_mut().find(|fd| **fd < 0) else {
                        native_fail(instance, pid, b"native pipe descriptor table full");
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
                for socket_fd in &mut task.socket_fds {
                    if socket_fd.fd >= task.close_range_first
                        && socket_fd.fd <= task.close_range_last
                    {
                        *socket_fd = EMPTY_SOCKET_FD;
                    }
                }
                if task.close_range_first >= 0 && task.close_range_last >= 0 {
                    native_fd_clear_range(
                        &mut task.console_fds,
                        task.close_range_first as usize,
                        task.close_range_last as usize,
                    );
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
            task.wait_status = 0;
            task.waitid_info = 0;
            task.pipe2_address = 0;
            task.pending_socket_type = 0;
            task.pending_getsockname = 0;
            task.close_range_first = -1;
            task.close_range_last = -1;
            tasks[task_index] = Some(task);
            let signal = if task.clone_call.sigchld { 17 } else { 0 };
            task.clone_call.sigchld = false;
            tasks[task_index] = Some(task);
            if ptrace(PTRACE_CONT, host_pid, 0, signal) < 0 {
                native_fail(instance, pid, b"native resume failed");
            }
            continue;
        }

        if !seccomp_entry {
            native_fail(instance, pid, b"unexpected native syscall-exit stop");
        }

        sequence += 1;
        let syscall = entry_syscall.unwrap_or_else(|| {
            native_fail(instance, pid, b"native syscall entry missing seccomp event")
        });
        if registers.orig_rax != syscall {
            native_fail(instance, pid, b"native seccomp syscall identity mismatch");
        }
        task.syscall = syscall;
        let transfer_limit = if mode != 3 {
            match syscall {
                0 | 17 => Some(1024 * 1024),
                1 if matches!(registers.rdi, 1 | 2) => Some(4096),
                _ => None,
            }
        } else {
            None
        };
        if transfer_limit.is_some_and(|limit| registers.rdx > limit) {
            registers.rdx = transfer_limit.unwrap_or(registers.rdx);
            if ptrace(PTRACE_SETREGS, host_pid, 0, &registers as *const _ as usize) < 0 {
                native_fail(instance, pid, b"native transfer clamp failed");
            }
        }
        if syscall == 318 && registers.rsi > 256 {
            registers.rsi = 256;
            if ptrace(PTRACE_SETREGS, host_pid, 0, &registers as *const _ as usize) < 0 {
                native_fail(instance, pid, b"native getrandom clamp failed");
            }
        }
        if syscall == 247 {
            task.waitid_selector = registers.rsi;
        }
        let shared_network =
            mode == 3 && instance.config().network_backend == NetworkBackend::Shared;
        let socket_table_full =
            shared_network && syscall == 41 && !task.socket_fds.iter().any(|entry| entry.fd < 0);
        if shared_network && syscall == 41 && !socket_table_full {
            task.pending_socket_type = (registers.rsi & 0xf) as u8;
        }
        if shared_network
            && syscall == 51
            && task
                .socket_fds
                .iter()
                .any(|entry| entry.fd == registers.rdi as i64 && entry.external_port != 0)
        {
            task.pending_getsockname = registers.rsi as usize;
        }
        if shared_network && matches!(syscall, 42 | 49) {
            let protocol = native_socket_protocol(&task, registers.rdi);
            if let Some((word, port)) =
                native_sockaddr_port(host_pid, registers.rsi as usize, registers.rdx)
            {
                let ingress = syscall == 49;
                let network_protocol = match protocol {
                    1 => Some(NetworkProtocol::Tcp),
                    2 => Some(NetworkProtocol::Udp),
                    _ => None,
                };
                if network_protocol.is_none()
                    || !instance.config().network_allows(
                        ingress,
                        network_protocol.unwrap_or(NetworkProtocol::Tcp),
                        port,
                    )
                {
                    write_all(b"GINUX_NETWORK_AUDIT: action=deny direction=");
                    write_all(if ingress { b"ingress" } else { b"egress" });
                    write_all(b" port=");
                    write_u64(port as u64);
                    write_all(b" result=-");
                    write_u64(EPERM as u64);
                    write_all(b"\n");
                    registers.orig_rax = u64::MAX;
                    task.awaiting_exit = true;
                    task.override_exit = true;
                    task.override_result = -EPERM;
                    tasks[task_index] = Some(task);
                    if ptrace(PTRACE_SETREGS, host_pid, 0, &registers as *const _ as usize) < 0
                        || ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0
                    {
                        native_fail(instance, pid, b"native network policy denial failed");
                    }
                    continue;
                }
                if !ingress {
                    if let Some(source) = instance.config().network_source_ipv4() {
                        let socket_index = task
                            .socket_fds
                            .iter()
                            .position(|entry| entry.fd == registers.rdi as i64);
                        let source_bound =
                            socket_index.is_some_and(|index| task.socket_fds[index].source_bound);
                        if !source_bound {
                            if !native_bind_socket_source(host_pid, registers.rdi as i64, source) {
                                native_fail(instance, pid, b"native source-address bind failed");
                            }
                            if let Some(index) = socket_index {
                                task.socket_fds[index].source_bound = true;
                            }
                            write_all(b"GINUX_NETWORK_AUDIT: action=source-bind address=");
                            write_all(instance.config().network_source.as_bytes());
                            write_all(b"\n");
                        }
                    }
                }
                if ingress {
                    let mut translated = word;
                    if let Some(source) = instance.config().network_source_ipv4() {
                        if word >> 32 == 0 {
                            translated = native_sockaddr_with_source(translated, source);
                        }
                    }
                    if let Some(external) = instance
                        .config()
                        .translated_port(network_protocol.unwrap_or(NetworkProtocol::Tcp), port)
                    {
                        translated = native_sockaddr_with_port(translated, external);
                        if let Some(socket) = task
                            .socket_fds
                            .iter_mut()
                            .find(|entry| entry.fd == registers.rdi as i64)
                        {
                            socket.internal_port = port;
                            socket.external_port = external;
                        }
                        write_all(b"GINUX_NETWORK_AUDIT: action=translate protocol=");
                        write_all(if protocol == 1 { b"tcp" } else { b"udp" });
                        write_all(b" internal=");
                        write_u64(port as u64);
                        write_all(b" external=");
                        write_u64(external as u64);
                        write_all(b"\n");
                    }
                    if translated != word {
                        if ptrace(
                            PTRACE_POKEDATA,
                            host_pid,
                            registers.rsi as usize,
                            translated as usize,
                        ) < 0
                        {
                            native_fail(instance, pid, b"native network translation failed");
                        }
                        task.pending_sockaddr = registers.rsi as usize;
                        task.pending_sockaddr_word = word;
                    }
                }
            }
        }
        let implementation_decision = if mode == 3 {
            decide_systemd(syscall, shared_network)
        } else {
            match decide_native(syscall, registers.rdi, registers.rsi, registers.rdx) {
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
                    task.awaiting_exit = true;
                    task.override_exit = true;
                    task.override_result = result;
                    tasks[task_index] = Some(task);
                    if ptrace(PTRACE_SETREGS, host_pid, 0, &registers as *const _ as usize) < 0
                        || ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0
                    {
                        native_fail(instance, pid, b"native lane emulation failed");
                    }
                    continue;
                }
            }
        };
        let path_policy_denied = mode == 3
            && !bootstrap_phase
            && match syscall {
                2 if registers.rsi & 3 != 0
                    || registers.rsi & (O_CREATE_HOST | O_TRUNC_HOST) as u64 != 0 =>
                {
                    native_write_path_denied(
                        instance.config(),
                        root_descriptor,
                        host_pid,
                        AT_FDCWD as u64,
                        registers.rdi as usize,
                    )
                }
                76 | 83 | 84 | 85 | 87 | 90 | 133 => native_write_path_denied(
                    instance.config(),
                    root_descriptor,
                    host_pid,
                    AT_FDCWD as u64,
                    registers.rdi as usize,
                ),
                82 | 86 => {
                    native_write_path_denied(
                        instance.config(),
                        root_descriptor,
                        host_pid,
                        AT_FDCWD as u64,
                        registers.rdi as usize,
                    ) || native_write_path_denied(
                        instance.config(),
                        root_descriptor,
                        host_pid,
                        AT_FDCWD as u64,
                        registers.rsi as usize,
                    )
                }
                88 => native_write_path_denied(
                    instance.config(),
                    root_descriptor,
                    host_pid,
                    AT_FDCWD as u64,
                    registers.rsi as usize,
                ),
                91 => {
                    if registers.rdi <= 2 {
                        false
                    } else {
                        let mut target = [0u8; 512];
                        native_fd_instance_target(host_pid, registers.rdi as usize, &mut target)
                            .is_none_or(|length| {
                                instance.config().filesystem_write_denied(&target[..length])
                            })
                    }
                }
                257 if registers.rdx & 3 != 0
                    || registers.rdx & (O_CREATE_HOST | O_TRUNC_HOST) as u64 != 0 =>
                {
                    native_write_path_denied(
                        instance.config(),
                        root_descriptor,
                        host_pid,
                        registers.rdi,
                        registers.rsi as usize,
                    )
                }
                258 | 259 | 263 | 268 | 452 => native_write_path_denied(
                    instance.config(),
                    root_descriptor,
                    host_pid,
                    registers.rdi,
                    registers.rsi as usize,
                ),
                264 | 316 => {
                    native_write_path_denied(
                        instance.config(),
                        root_descriptor,
                        host_pid,
                        registers.rdi,
                        registers.rsi as usize,
                    ) || native_write_path_denied(
                        instance.config(),
                        root_descriptor,
                        host_pid,
                        registers.rdx,
                        registers.r10 as usize,
                    )
                }
                265 => {
                    native_write_path_denied(
                        instance.config(),
                        root_descriptor,
                        host_pid,
                        registers.rdi,
                        registers.rsi as usize,
                    ) || native_write_path_denied(
                        instance.config(),
                        root_descriptor,
                        host_pid,
                        registers.rdx,
                        registers.r10 as usize,
                    )
                }
                266 => native_write_path_denied(
                    instance.config(),
                    root_descriptor,
                    host_pid,
                    registers.rsi,
                    registers.rdx as usize,
                ),
                _ => false,
            };
        let setid_policy_denied = mode == 3
            && match syscall {
                90 => native_setid_path_denied(
                    instance.config(),
                    root_descriptor,
                    host_pid,
                    AT_FDCWD as u64,
                    registers.rdi as usize,
                    registers.rsi,
                ),
                91 => native_setid_fd_denied(
                    instance.config(),
                    host_pid,
                    registers.rdi as usize,
                    registers.rsi,
                ),
                268 | 452 => native_setid_path_denied(
                    instance.config(),
                    root_descriptor,
                    host_pid,
                    registers.rdi,
                    registers.rsi as usize,
                    registers.rdx,
                ),
                _ => false,
            };
        let device_node_policy_denied = mode == 3
            && match syscall {
                133 if matches!(registers.rsi & 0o170000, 0o020000 | 0o060000) => {
                    native_device_create_denied(
                        instance.config(),
                        root_descriptor,
                        host_pid,
                        AT_FDCWD as u64,
                        registers.rdi as usize,
                    )
                }
                259 if matches!(registers.rdx & 0o170000, 0o020000 | 0o060000) => {
                    native_device_create_denied(
                        instance.config(),
                        root_descriptor,
                        host_pid,
                        registers.rdi,
                        registers.rsi as usize,
                    )
                }
                _ => false,
            };
        let clone_memory_denied = mode == 3 && native_clone::changes_snapshot(syscall, &registers);
        let local_policy_denied = clone_memory_denied
            || path_policy_denied
            || setid_policy_denied
            || device_node_policy_denied;
        if local_policy_denied {
            write_all(b"GINUX_POLICY_AUDIT: action=deny policy=");
            write_all(if clone_memory_denied {
                b"clone-arguments"
            } else if path_policy_denied {
                b"filesystem-write"
            } else if setid_policy_denied {
                b"setid"
            } else {
                b"device-node"
            });
            write_all(b" result=-1\n");
        }

        let semantic_operation = if mode == 3 && syscall == 257 {
            if tracee_string_starts_with(host_pid, registers.rsi as usize, b"/proc") {
                Some("proc.process.read")
            } else if tracee_string_starts_with(host_pid, registers.rsi as usize, b"/sys/fs/cgroup")
            {
                Some("cgroup.virtual.manage")
            } else if tracee_string_starts_with(host_pid, registers.rsi as usize, b"/sys") {
                Some("sys.device.read")
            } else if registers.rdx & 3 != 0
                || registers.rdx & (O_CREATE_HOST | O_TRUNC_HOST) as u64 != 0
            {
                Some("filesystem.path.write")
            } else {
                Some("filesystem.path.read")
            }
        } else if syscall == 0 && registers.rdi as i64 == task.console_fd {
            Some("device.console.read")
        } else if syscall == 1 && registers.rdi as i64 == task.console_fd {
            Some("device.console.write")
        } else if mode == 3 && matches!(syscall, 42 | 49) {
            match (native_socket_protocol(&task, registers.rdi), syscall) {
                (1, 42) => Some("network.inet.tcp.connect"),
                (1, 49) => Some("network.inet.tcp.bind"),
                (2, 42) => Some("network.inet.udp.connect"),
                (2, 49) => Some("network.inet.udp.bind"),
                _ => None,
            }
        } else {
            ginux_contract::syscall_semantic_operation(syscall, registers.rsi)
        };
        let contract_rule = contract.runtime_rule(syscall, semantic_operation);
        let contract_action = contract_rule
            .map(|rule| rule.action)
            .unwrap_or(ContractAction::Deny(38));
        let decision = if local_policy_denied {
            ginux_core::Decision {
                syscall,
                action: Action::Deny(EPERM as i32),
            }
        } else {
            match contract_action {
                ContractAction::Deny(errno) => ginux_core::Decision {
                    syscall,
                    action: Action::Deny(errno),
                },
                ContractAction::Emulate | ContractAction::Mediate => implementation_decision,
            }
        };
        write_all(b"GINUX_NATIVE_AUDIT: sequence=");
        write_u64(sequence);
        write_all(b" instance=");
        write_all(audit_instance_id.as_bytes());
        write_all(b" virtual-pid=");
        write_u64(task.virtual_id.pid as u64);
        write_all(b" contract=");
        write_all(contract.digest());
        write_all(b" operation=");
        if let Some(rule) = contract_rule {
            write_all(rule.operation.id.as_bytes());
            write_all(b" policy-source=");
            write_all(rule.source.as_bytes());
            write_all(b" policy-rule=");
            write_u64(rule.source_rule as u64);
        } else {
            write_all(b"unknown");
            write_all(b" policy-source=default-deny");
        }
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
        } else if mode == 3 && syscall == 98 {
            write_all(b" who=");
            write_i64(registers.rdi as i64);
            write_all(b" usage=");
            write_u64(registers.rsi);
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
        let cgroup_procs_path = mode == 3
            && syscall == 257
            && tracee_cgroup_procs_path(host_pid, registers.rsi as usize);
        let proc_link_source_fd = if mode == 3 && syscall == ginux_core::SYS_LINKAT {
            tracee_proc_self_fd_number(host_pid, registers.rsi as usize)
        } else {
            None
        };
        let mut virtual_path = [0u8; 192];
        let virtual_path_len = if mode != 3 {
            None
        } else {
            match syscall {
                2 | 21 | 83 | 84 | 89 => native_virtual_kernel_path(
                    host_pid,
                    AT_FDCWD as i64,
                    registers.rdi as usize,
                    &virtual_kernel_fds,
                    &mut virtual_path,
                ),
                257 | 258 | 262 | 263 | 267 | 269 | 332 | 439 => native_virtual_kernel_path(
                    host_pid,
                    registers.rdi as i32 as i64,
                    registers.rsi as usize,
                    &virtual_kernel_fds,
                    &mut virtual_path,
                ),
                _ => None,
            }
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
        let virtual_inotify_watch = if mode == 3 && syscall == 254 {
            tracee_proc_self_fd_number(host_pid, registers.rsi as usize).is_some_and(|fd| {
                native_virtual_kernel_fd(&virtual_kernel_fds, host_pid, fd as i64).is_some()
            })
        } else {
            false
        };
        let mut proc_fd_host_path = [0u8; 512];
        let proc_fd_host_path_len = if mode == 3
            && matches!(syscall, 2 | 257)
            && tracee_decimal_fd_path(
                host_pid,
                if syscall == 2 {
                    registers.rdi as usize
                } else {
                    registers.rsi as usize
                },
            ) {
            translate_proc_self_fd_target(
                host_pid,
                if syscall == 2 {
                    registers.rdi as usize
                } else {
                    registers.rsi as usize
                },
                &mut proc_fd_host_path,
            )
        } else {
            None
        };
        write_all(b" action=");
        match decision.action {
            Action::Mediate if syscall == 99 => {
                let result = write_tracee_bytes_result(
                    host_pid,
                    registers.rdi as usize,
                    &virtual_sysinfo(instance.config().memory_mib),
                )
                .map_or_else(|error| error, |_| 0);
                write_all(b"emulate surface=virtual-system result=");
                write_i64(result);
                write_all(b"\n");
                registers.orig_rax = u64::MAX;
                task.awaiting_exit = true;
                task.override_exit = true;
                task.override_result = result;
                if ptrace(PTRACE_SETREGS, host_pid, 0, &registers as *const _ as usize) < 0
                    || ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0
                {
                    native_fail(instance, pid, b"native sysinfo emulation failed");
                }
            }
            Action::Mediate if socket_table_full => {
                write_all(b"deny surface=virtual-network result=-");
                write_u64(EMFILE as u64);
                write_all(b" reason=descriptor-limit\n");
                registers.orig_rax = u64::MAX;
                task.awaiting_exit = true;
                task.override_exit = true;
                task.override_result = -EMFILE;
                if ptrace(PTRACE_SETREGS, host_pid, 0, &registers as *const _ as usize) < 0
                    || ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0
                {
                    native_fail(instance, pid, b"native socket limit denial failed");
                }
            }
            Action::Mediate if proc_fd_host_path_len.is_some() => {
                let length = proc_fd_host_path_len.unwrap_or(0);
                let path_address = (registers.rsp as usize).saturating_sub(640);
                let translated =
                    write_tracee_bytes(host_pid, path_address, &proc_fd_host_path[..=length]);
                if !translated {
                    native_fail(instance, pid, b"native proc fd open translation failed");
                }
                if syscall == 2 {
                    registers.rdi = path_address as u64;
                } else {
                    registers.rsi = path_address as u64;
                }
                write_all(b"mediate surface=virtual-proc-fd result=pending\n");
                task.awaiting_exit = true;
                if ptrace(PTRACE_SETREGS, host_pid, 0, &registers as *const _ as usize) < 0
                    || ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0
                {
                    native_fail(instance, pid, b"native proc fd open resume failed");
                }
            }
            Action::Mediate
                if mode == 3 && matches!(syscall, 2 | 257) && virtual_path_len.is_some() =>
            {
                let path = &virtual_path[..virtual_path_len.unwrap_or(0)];
                let flags = if syscall == 2 {
                    registers.rsi
                } else {
                    registers.rdx
                };
                let directory = native_virtual_kernel_directory(
                    path,
                    task.virtual_id,
                    &processes,
                    &tasks,
                    &virtual_kernel_state,
                );
                let exists = native_virtual_kernel_exists(
                    path,
                    task.virtual_id,
                    &processes,
                    &tasks,
                    &virtual_kernel_state,
                    &virtual_kernel_fds,
                );
                let result = if !exists {
                    -2
                } else if flags & O_DIRECTORY_HOST as u64 != 0 && !directory {
                    -20
                } else {
                    native_virtual_kernel_allocate(
                        &mut virtual_kernel_fds,
                        host_pid,
                        path,
                        flags,
                        flags & O_CLOEXEC != 0,
                    )
                    .unwrap_or(-EMFILE)
                };
                if result >= 0
                    && (path == b"/proc/self/mountinfo"
                        || path
                            .strip_prefix(b"/proc/")
                            .and_then(|rest| rest.strip_suffix(b"/mountinfo"))
                            .and_then(native_parse_u64)
                            == Some(u64::from(task.virtual_id.pid)))
                {
                    task.mountinfo_fd = result;
                }
                write_all(b"emulate surface=virtual-kernel-open result=");
                write_i64(result);
                write_all(b"\n");
                task.awaiting_exit = true;
                task.override_exit = true;
                task.override_result = result;
                if !native_virtual_kernel_override(host_pid, &mut registers, result) {
                    native_fail(instance, pid, b"native virtual open emulation failed");
                }
            }
            Action::Mediate
                if mode == 3 && matches!(syscall, 21 | 269 | 439) && virtual_path_len.is_some() =>
            {
                let path = &virtual_path[..virtual_path_len.unwrap_or(0)];
                let result = if native_virtual_kernel_exists(
                    path,
                    task.virtual_id,
                    &processes,
                    &tasks,
                    &virtual_kernel_state,
                    &virtual_kernel_fds,
                ) {
                    0
                } else {
                    -2
                };
                write_all(b"emulate surface=virtual-kernel-access result=");
                write_i64(result);
                write_all(b"\n");
                task.awaiting_exit = true;
                task.override_exit = true;
                task.override_result = result;
                if !native_virtual_kernel_override(host_pid, &mut registers, result) {
                    native_fail(instance, pid, b"native virtual access emulation failed");
                }
            }
            Action::Mediate
                if mode == 3
                    && matches!(syscall, 83 | 84 | 258 | 263)
                    && virtual_path_len.is_some() =>
            {
                let path = &virtual_path[..virtual_path_len.unwrap_or(0)];
                let creating = matches!(syscall, 83 | 258);
                let result = if creating {
                    virtual_kernel_state.create_cgroup_directory(path)
                } else {
                    virtual_kernel_state.remove_cgroup_directory(path)
                };
                write_all(b"emulate surface=virtual-kernel-tree result=");
                write_i64(result);
                write_all(b"\n");
                task.awaiting_exit = true;
                task.override_exit = true;
                task.override_result = result;
                if !native_virtual_kernel_override(host_pid, &mut registers, result) {
                    native_fail(instance, pid, b"native virtual tree emulation failed");
                }
            }
            Action::Mediate
                if mode == 3
                    && syscall == SYS_READ
                    && native_virtual_kernel_fd(
                        &virtual_kernel_fds,
                        host_pid,
                        registers.rdi as i64,
                    )
                    .is_some() =>
            {
                let descriptor =
                    *native_virtual_kernel_fd(&virtual_kernel_fds, host_pid, registers.rdi as i64)
                        .unwrap();
                let path = &descriptor.path[..descriptor.path_len];
                let mut content = [0u8; 256 * 1024];
                let generated = if native_virtual_kernel_directory(
                    path,
                    task.virtual_id,
                    &processes,
                    &tasks,
                    &virtual_kernel_state,
                ) {
                    None
                } else {
                    native_virtual_kernel_read(
                        path,
                        task.virtual_id,
                        &processes,
                        &tasks,
                        [
                            instance_start_ns,
                            instance_start_realtime_seconds,
                            instance.config().vcpus,
                            instance.config().memory_mib,
                        ],
                        NativeVirtualIdentity {
                            instance_id: instance.config().id.as_bytes(),
                            hostname: instance.config().hostname.as_bytes(),
                            contract_profile: contract_label(instance.config()),
                        },
                        &mut content,
                    )
                };
                let result = if native_virtual_kernel_directory(
                    path,
                    task.virtual_id,
                    &processes,
                    &tasks,
                    &virtual_kernel_state,
                ) {
                    -21
                } else if let Some(length) = generated {
                    let start = core::cmp::min(descriptor.offset as usize, length);
                    let count = core::cmp::min(registers.rdx as usize, length - start);
                    if write_tracee_bytes(
                        host_pid,
                        registers.rsi as usize,
                        &content[start..start + count],
                    ) {
                        if let Some(entry) = native_virtual_kernel_fd_mut(
                            &mut virtual_kernel_fds,
                            host_pid,
                            registers.rdi as i64,
                        ) {
                            entry.offset = entry.offset.saturating_add(count as u64);
                        }
                        count as i64
                    } else {
                        -EFAULT
                    }
                } else {
                    -ENOSYS
                };
                write_all(b"emulate surface=virtual-kernel-read result=");
                write_i64(result);
                write_all(b"\n");
                task.awaiting_exit = true;
                task.override_exit = true;
                task.override_result = result;
                if !native_virtual_kernel_override(host_pid, &mut registers, result) {
                    native_fail(instance, pid, b"native virtual read emulation failed");
                }
            }
            Action::Mediate
                if mode == 3
                    && syscall == SYS_WRITE
                    && native_virtual_kernel_fd(
                        &virtual_kernel_fds,
                        host_pid,
                        registers.rdi as i64,
                    )
                    .is_some() =>
            {
                let descriptor =
                    *native_virtual_kernel_fd(&virtual_kernel_fds, host_pid, registers.rdi as i64)
                        .unwrap();
                let path = &descriptor.path[..descriptor.path_len];
                let writable = path.ends_with(b"/cgroup.procs")
                    || path.ends_with(b"/cgroup.subtree_control")
                    || path.ends_with(b"/cgroup.kill")
                    || path.ends_with(b"/oom_score_adj");
                let result = if writable {
                    registers.rdx as i64
                } else {
                    -EPERM
                };
                write_all(b"emulate surface=virtual-kernel-write result=");
                write_i64(result);
                write_all(b"\n");
                task.awaiting_exit = true;
                task.override_exit = true;
                task.override_result = result;
                if !native_virtual_kernel_override(host_pid, &mut registers, result) {
                    native_fail(instance, pid, b"native virtual write emulation failed");
                }
            }
            Action::Mediate
                if mode == 3
                    && syscall == SYS_GETDENTS64_HOST as u64
                    && native_virtual_kernel_fd(
                        &virtual_kernel_fds,
                        host_pid,
                        registers.rdi as i64,
                    )
                    .is_some() =>
            {
                let descriptor =
                    *native_virtual_kernel_fd(&virtual_kernel_fds, host_pid, registers.rdi as i64)
                        .unwrap();
                let path = &descriptor.path[..descriptor.path_len];
                let mut entries = [0u8; 32 * 1024];
                let limit = core::cmp::min(registers.rdx as usize, entries.len());
                let generated = native_virtual_kernel_getdents(
                    path,
                    task.virtual_id,
                    &processes,
                    &tasks,
                    &virtual_kernel_state,
                    &virtual_kernel_fds,
                    descriptor.offset,
                    &mut entries[..limit],
                );
                let result = if let Some((length, next)) = generated {
                    if write_tracee_bytes(host_pid, registers.rsi as usize, &entries[..length]) {
                        if let Some(entry) = native_virtual_kernel_fd_mut(
                            &mut virtual_kernel_fds,
                            host_pid,
                            registers.rdi as i64,
                        ) {
                            entry.offset = next;
                        }
                        length as i64
                    } else {
                        -EFAULT
                    }
                } else {
                    -20
                };
                write_all(b"emulate surface=virtual-kernel-directory result=");
                write_i64(result);
                write_all(b"\n");
                task.awaiting_exit = true;
                task.override_exit = true;
                task.override_result = result;
                if !native_virtual_kernel_override(host_pid, &mut registers, result) {
                    native_fail(instance, pid, b"native virtual directory emulation failed");
                }
            }
            Action::Mediate
                if mode == 3
                    && syscall == 16
                    && (registers.rsi as u32) & 0xc000_ffff == 0xc000_ff0b =>
            {
                // PIDFD_GET_INFO (Linux 6.13; the struct size in the request
                // grows with the kernel) reports host pid, tgid, ppid and
                // credentials. The 6.12 ABI personality has no such ioctl;
                // callers (systemd 258+) fall back to /proc/self/fdinfo,
                // which Ginux virtualizes.
                write_all(b"emulate surface=virtual-pidfd-info result=-25\n");
                registers.orig_rax = u64::MAX;
                task.awaiting_exit = true;
                task.override_exit = true;
                task.override_result = -25;
                if ptrace(PTRACE_SETREGS, host_pid, 0, &registers as *const _ as usize) < 0
                    || ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0
                {
                    native_fail(instance, pid, b"native pidfd info denial failed");
                }
            }
            Action::Mediate
                if mode == 3
                    && syscall == 16
                    && native_virtual_kernel_fd(
                        &virtual_kernel_fds,
                        host_pid,
                        registers.rdi as i64,
                    )
                    .is_some() =>
            {
                let result = -25;
                write_all(b"emulate surface=virtual-kernel-ioctl result=-25\n");
                task.awaiting_exit = true;
                task.override_exit = true;
                task.override_result = result;
                if !native_virtual_kernel_override(host_pid, &mut registers, result) {
                    native_fail(instance, pid, b"native virtual ioctl emulation failed");
                }
            }
            Action::Mediate
                if mode == 3
                    && syscall == 5
                    && native_virtual_kernel_fd(
                        &virtual_kernel_fds,
                        host_pid,
                        registers.rdi as i64,
                    )
                    .is_some() =>
            {
                let descriptor =
                    *native_virtual_kernel_fd(&virtual_kernel_fds, host_pid, registers.rdi as i64)
                        .unwrap();
                let path = &descriptor.path[..descriptor.path_len];
                let stat = native_virtual_kernel_stat(
                    path,
                    native_virtual_kernel_directory(
                        path,
                        task.virtual_id,
                        &processes,
                        &tasks,
                        &virtual_kernel_state,
                    ),
                    false,
                    0,
                );
                let result = if write_tracee_bytes(host_pid, registers.rsi as usize, &stat) {
                    0
                } else {
                    -EFAULT
                };
                write_all(b"emulate surface=virtual-kernel-stat result=");
                write_i64(result);
                write_all(b"\n");
                task.awaiting_exit = true;
                task.override_exit = true;
                task.override_result = result;
                if !native_virtual_kernel_override(host_pid, &mut registers, result) {
                    native_fail(instance, pid, b"native virtual fstat emulation failed");
                }
            }
            Action::Mediate if mode == 3 && syscall == 262 && virtual_path_len.is_some() => {
                let path = &virtual_path[..virtual_path_len.unwrap_or(0)];
                let exists = native_virtual_kernel_exists(
                    path,
                    task.virtual_id,
                    &processes,
                    &tasks,
                    &virtual_kernel_state,
                    &virtual_kernel_fds,
                );
                let symbolic_link = registers.r10 & 0x100 != 0
                    && native_virtual_kernel_symlink(path, task.virtual_id, &processes, &tasks);
                let stat = native_virtual_kernel_stat(
                    path,
                    !symbolic_link
                        && native_virtual_kernel_directory(
                            path,
                            task.virtual_id,
                            &processes,
                            &tasks,
                            &virtual_kernel_state,
                        ),
                    symbolic_link,
                    0,
                );
                let result = if !exists {
                    -2
                } else if write_tracee_bytes(host_pid, registers.rdx as usize, &stat) {
                    0
                } else {
                    -EFAULT
                };
                write_all(b"emulate surface=virtual-kernel-stat result=");
                write_i64(result);
                write_all(b"\n");
                task.awaiting_exit = true;
                task.override_exit = true;
                task.override_result = result;
                if !native_virtual_kernel_override(host_pid, &mut registers, result) {
                    native_fail(instance, pid, b"native virtual stat emulation failed");
                }
            }
            Action::Mediate if mode == 3 && syscall == 332 && virtual_path_len.is_some() => {
                let path = &virtual_path[..virtual_path_len.unwrap_or(0)];
                let exists = native_virtual_kernel_exists(
                    path,
                    task.virtual_id,
                    &processes,
                    &tasks,
                    &virtual_kernel_state,
                    &virtual_kernel_fds,
                );
                let mut statx = [0u8; 256];
                let symbolic_link = registers.rdx & 0x100 != 0
                    && native_virtual_kernel_symlink(path, task.virtual_id, &processes, &tasks);
                statx[0..4].copy_from_slice(&0x7fffu32.to_ne_bytes());
                statx[4..8].copy_from_slice(&4096u32.to_ne_bytes());
                statx[16..20].copy_from_slice(&1u32.to_ne_bytes());
                statx[28..30].copy_from_slice(
                    &(if symbolic_link {
                        0o120_777u16
                    } else if native_virtual_kernel_directory(
                        path,
                        task.virtual_id,
                        &processes,
                        &tasks,
                        &virtual_kernel_state,
                    ) {
                        0o040_555u16
                    } else {
                        0o100_444u16
                    })
                    .to_ne_bytes(),
                );
                statx[32..40].copy_from_slice(&1u64.to_ne_bytes());
                let result = if !exists {
                    -2
                } else if write_tracee_bytes(host_pid, registers.r8 as usize, &statx) {
                    0
                } else {
                    -EFAULT
                };
                write_all(b"emulate surface=virtual-kernel-statx result=");
                write_i64(result);
                write_all(b"\n");
                task.awaiting_exit = true;
                task.override_exit = true;
                task.override_result = result;
                if !native_virtual_kernel_override(host_pid, &mut registers, result) {
                    native_fail(instance, pid, b"native virtual statx emulation failed");
                }
            }
            Action::Mediate
                if mode == 3
                    && matches!(syscall, 89 | ginux_core::SYS_READLINKAT)
                    && virtual_path_len.is_some() =>
            {
                let path = &virtual_path[..virtual_path_len.unwrap_or(0)];
                let mut target = [0u8; 192];
                let target_len = native_virtual_proc_fd_number(path, task.virtual_id)
                    .and_then(|fd| {
                        native_virtual_kernel_fd(&virtual_kernel_fds, host_pid, fd as i64)
                            .map(|descriptor| {
                                let count = core::cmp::min(descriptor.path_len, target.len());
                                target[..count].copy_from_slice(&descriptor.path[..count]);
                                count
                            })
                            .or_else(|| native_fd_instance_target(host_pid, fd, &mut target))
                    })
                    .or_else(|| native_virtual_kernel_link(path, task.virtual_id, &mut target));
                let (address, capacity) = if syscall == 89 {
                    (registers.rsi as usize, registers.rdx as usize)
                } else {
                    (registers.rdx as usize, registers.r10 as usize)
                };
                let result = if let Some(length) = target_len {
                    let count = core::cmp::min(length, capacity);
                    if write_tracee_bytes(host_pid, address, &target[..count]) {
                        count as i64
                    } else {
                        -EFAULT
                    }
                } else {
                    -22
                };
                write_all(b"emulate surface=virtual-kernel-link result=");
                write_i64(result);
                write_all(b"\n");
                task.awaiting_exit = true;
                task.override_exit = true;
                task.override_result = result;
                if !native_virtual_kernel_override(host_pid, &mut registers, result) {
                    native_fail(instance, pid, b"native virtual link emulation failed");
                }
            }
            Action::Mediate
                if mode == 3
                    && syscall == 8
                    && native_virtual_kernel_fd(
                        &virtual_kernel_fds,
                        host_pid,
                        registers.rdi as i64,
                    )
                    .is_some() =>
            {
                let entry = native_virtual_kernel_fd_mut(
                    &mut virtual_kernel_fds,
                    host_pid,
                    registers.rdi as i64,
                )
                .unwrap();
                let result = match registers.rdx {
                    0 => {
                        entry.offset = registers.rsi;
                        entry.offset as i64
                    }
                    1 => {
                        entry.offset = entry.offset.saturating_add(registers.rsi);
                        entry.offset as i64
                    }
                    _ => -22,
                };
                write_all(b"emulate surface=virtual-kernel-seek result=");
                write_i64(result);
                write_all(b"\n");
                task.awaiting_exit = true;
                task.override_exit = true;
                task.override_result = result;
                if !native_virtual_kernel_override(host_pid, &mut registers, result) {
                    native_fail(instance, pid, b"native virtual seek emulation failed");
                }
            }
            Action::Mediate
                if mode == 3
                    && syscall == 3
                    && native_virtual_kernel_fd(
                        &virtual_kernel_fds,
                        host_pid,
                        registers.rdi as i64,
                    )
                    .is_some() =>
            {
                if registers.rdi as i64 == task.mountinfo_fd {
                    task.mountinfo_fd = -1;
                }
                let result = if native_virtual_kernel_close(
                    &mut virtual_kernel_fds,
                    host_pid,
                    registers.rdi as i64,
                ) {
                    0
                } else {
                    -EBADF
                };
                write_all(b"emulate surface=virtual-kernel-close result=");
                write_i64(result);
                write_all(b"\n");
                task.awaiting_exit = true;
                task.override_exit = true;
                task.override_result = result;
                if !native_virtual_kernel_override(host_pid, &mut registers, result) {
                    native_fail(instance, pid, b"native virtual close emulation failed");
                }
            }
            Action::Mediate
                if mode == 3
                    && matches!(syscall, 32 | 33 | 292)
                    && native_virtual_kernel_fd(
                        &virtual_kernel_fds,
                        host_pid,
                        registers.rdi as i64,
                    )
                    .is_some() =>
            {
                let requested = (syscall != 32).then_some(registers.rsi as i64);
                let close_on_exec = syscall == 292 && registers.rdx & O_CLOEXEC != 0;
                let result = native_virtual_kernel_duplicate(
                    &mut virtual_kernel_fds,
                    host_pid,
                    registers.rdi as i64,
                    requested,
                    close_on_exec,
                )
                .unwrap_or(-EMFILE);
                write_all(b"emulate surface=virtual-kernel-duplicate result=");
                write_i64(result);
                write_all(b"\n");
                task.awaiting_exit = true;
                task.override_exit = true;
                task.override_result = result;
                if !native_virtual_kernel_override(host_pid, &mut registers, result) {
                    native_fail(instance, pid, b"native virtual duplicate emulation failed");
                }
            }
            Action::Mediate
                if mode == 3
                    && syscall == 72
                    && native_virtual_kernel_fd(
                        &virtual_kernel_fds,
                        host_pid,
                        registers.rdi as i64,
                    )
                    .is_some() =>
            {
                let descriptor =
                    *native_virtual_kernel_fd(&virtual_kernel_fds, host_pid, registers.rdi as i64)
                        .unwrap();
                let result = match registers.rsi {
                    F_GETFD => i64::from(descriptor.close_on_exec) * FD_CLOEXEC as i64,
                    3 => descriptor.flags as i64,
                    4 => {
                        if let Some(entry) = native_virtual_kernel_fd_mut(
                            &mut virtual_kernel_fds,
                            host_pid,
                            registers.rdi as i64,
                        ) {
                            entry.flags = (entry.flags & 3) | (registers.rdx & !3);
                        }
                        0
                    }
                    F_SETFD => {
                        if let Some(entry) = native_virtual_kernel_fd_mut(
                            &mut virtual_kernel_fds,
                            host_pid,
                            registers.rdi as i64,
                        ) {
                            entry.close_on_exec = registers.rdx & FD_CLOEXEC != 0;
                        }
                        0
                    }
                    F_DUPFD | F_DUPFD_CLOEXEC => native_virtual_kernel_duplicate_at_least(
                        &mut virtual_kernel_fds,
                        host_pid,
                        registers.rdi as i64,
                        registers.rdx as i64,
                        registers.rsi == F_DUPFD_CLOEXEC,
                    )
                    .unwrap_or(-EMFILE),
                    _ => -22,
                };
                write_all(b"emulate surface=virtual-kernel-fcntl result=");
                write_i64(result);
                write_all(b"\n");
                task.awaiting_exit = true;
                task.override_exit = true;
                task.override_result = result;
                if !native_virtual_kernel_override(host_pid, &mut registers, result) {
                    native_fail(instance, pid, b"native virtual fcntl emulation failed");
                }
            }
            Action::Emulate if native_identity_result(syscall, mode).is_some() => {
                let result = if mode == 3 {
                    let view = processes
                        .view(task.virtual_id)
                        .unwrap_or_else(|_| native_fail(instance, pid, b"native identity failed"));
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
                    native_fail(instance, pid, b"native getpid emulation failed");
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
                    native_fail(instance, pid, b"native priority emulation failed");
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
                    native_fail(instance, pid, b"native priority update emulation failed");
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
                    native_fail(instance, pid, b"native rlimit emulation failed");
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
                    native_fail(instance, pid, b"native virtual capset failed");
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
                    native_fail(instance, pid, b"native virtual capability prctl failed");
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
                    native_fail(instance, pid, b"native virtual personality failed");
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
                    native_fail(instance, pid, b"native virtual seccomp failed");
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
                    write_all(b"translate surface=virtual-poweroff result=instance-shutdown\n");
                    line_to(
                        2,
                        b"GINUX_LIFECYCLE_AUDIT: action=translate surface=virtual-poweroff result=instance-shutdown",
                    );
                    registers.orig_rax = ginux_core::SYS_EXIT_GROUP;
                    registers.rdi = 0;
                    task.awaiting_exit = false;
                    task.override_exit = false;
                } else {
                    write_all(b"deny result=-1 reason=invalid-virtual-poweroff\n");
                    line_to(
                        2,
                        b"GINUX_LIFECYCLE_AUDIT: action=deny result=-1 reason=invalid-virtual-poweroff",
                    );
                    registers.orig_rax = u64::MAX;
                    task.awaiting_exit = true;
                    task.override_exit = true;
                    task.override_result = -EPERM;
                }
                if ptrace(PTRACE_SETREGS, host_pid, 0, &registers as *const _ as usize) < 0
                    || ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0
                {
                    native_fail(instance, pid, b"native virtual poweroff failed");
                }
            }
            Action::Emulate if mode == 3 && syscall == 63 => {
                let mut uts = [0u8; 65 * 6];
                write_uts_field(&mut uts, 0, b"Linux");
                write_uts_field(&mut uts, 1, instance.config().hostname.as_bytes());
                write_uts_field(&mut uts, 2, GINUX_ABI_RELEASE);
                write_uts_field(&mut uts, 3, GINUX_VERSION.as_bytes());
                write_uts_field(&mut uts, 4, b"x86_64");
                write_uts_field(&mut uts, 5, b"localinstance");
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
                    native_fail(instance, pid, b"native uname emulation failed");
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
                    native_fail(instance, pid, b"native virtual mount emulation failed");
                }
            }
            Action::Emulate if mode == 3 && syscall == 112 => {
                let result = processes
                    .create_session(task.virtual_id)
                    .map(i64::from)
                    .unwrap_or(-EPERM);
                if result >= 0 {
                    let task_view = processes.view(task.virtual_id).unwrap_or_else(|_| {
                        native_fail(instance, pid, b"native session view failed")
                    });
                    if !pid_map.update_process_group(host_pid, task_view.process_group) {
                        native_fail(instance, pid, b"native session PID map update failed");
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
                    native_fail(instance, pid, b"native virtual setsid failed");
                }
            }
            Action::Emulate if mode == 3 && syscall == 124 => {
                let target = u32::try_from(registers.rdi).ok();
                let result = match target {
                    Some(0) => processes
                        .view(task.virtual_id)
                        .map(|view| i64::from(view.session))
                        .unwrap_or(-ESRCH),
                    Some(target) => processes
                        .lookup(target)
                        .map(|view| i64::from(view.session))
                        .unwrap_or(-ESRCH),
                    None => -ESRCH,
                };
                write_all(b"emulate surface=virtual-session-query result=");
                write_i64(result);
                write_all(b"\n");
                registers.orig_rax = u64::MAX;
                task.awaiting_exit = true;
                task.override_exit = true;
                task.override_result = result;
                if ptrace(PTRACE_SETREGS, host_pid, 0, &registers as *const _ as usize) < 0
                    || ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0
                {
                    native_fail(instance, pid, b"native virtual getsid failed");
                }
            }
            Action::Emulate if mode == 3 && syscall == 121 => {
                let target = u32::try_from(registers.rdi).ok();
                let result = match target {
                    Some(0) => processes
                        .view(task.virtual_id)
                        .map(|view| i64::from(view.process_group))
                        .unwrap_or(-ESRCH),
                    Some(target) => processes
                        .lookup(target)
                        .map(|view| i64::from(view.process_group))
                        .unwrap_or(-ESRCH),
                    None => -ESRCH,
                };
                write_all(b"emulate surface=virtual-process-group-query result=");
                write_i64(result);
                write_all(b"\n");
                registers.orig_rax = u64::MAX;
                task.awaiting_exit = true;
                task.override_exit = true;
                task.override_result = result;
                if ptrace(PTRACE_SETREGS, host_pid, 0, &registers as *const _ as usize) < 0
                    || ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0
                {
                    native_fail(instance, pid, b"native virtual getpgid failed");
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
                        native_fail(instance, pid, b"native process group view failed")
                    });
                    let changed_supervisor = tasks
                        .iter()
                        .flatten()
                        .find(|candidate| candidate.virtual_id == changed_task)
                        .copied()
                        .unwrap_or_else(|| {
                            native_fail(instance, pid, b"native process group task missing")
                        });
                    if !pid_map
                        .update_process_group(changed_supervisor.host_pid, task_view.process_group)
                    {
                        native_fail(instance, pid, b"native process group PID map update failed");
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
                    native_fail(instance, pid, b"native virtual setpgid failed");
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
                let current = processes.view(task.virtual_id).unwrap_or_else(|_| {
                    native_fail(instance, pid, b"native credential view failed")
                });
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
                    native_fail(instance, pid, b"native virtual credential update failed");
                }
            }
            Action::Emulate if mode == 3 && matches!(syscall, 92 | 93 | 94 | 260) => {
                let root = processes
                    .view(task.virtual_id)
                    .is_ok_and(|view| view.credentials.uid == 0);
                let allowed = match syscall {
                    92 | 94 => tracee_string_bounded(host_pid, registers.rdi as usize, false),
                    93 => registers.rdi < INSTANCE_NULL_FD as u64,
                    260 => {
                        let directory = registers.rdi as u32 == (-100i32) as u32
                            || registers.rdi < INSTANCE_NULL_FD as u64;
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
                    native_fail(instance, pid, b"native virtual file ownership failed");
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
                    native_fail(instance, pid, b"native virtual cgroup removal failed");
                }
            }
            Action::Mediate
                if mode == 3
                    && syscall == 90
                    && registers.rsi & !0o7777 == 0
                    && tracee_string_equals(host_pid, registers.rdi as usize, b"/dev/console") =>
            {
                write_all(b"emulate surface=virtual-console-mode result=0\n");
                registers.orig_rax = u64::MAX;
                task.awaiting_exit = true;
                task.override_exit = true;
                task.override_result = 0;
                if ptrace(PTRACE_SETREGS, host_pid, 0, &registers as *const _ as usize) < 0
                    || ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0
                {
                    native_fail(instance, pid, b"native virtual console chmod failed");
                }
            }
            Action::Mediate
                if mode == 3
                    && syscall == 268
                    && registers.rdx & !0o7777 == 0
                    && registers.r10 & !0x100 == 0
                    && tracee_string_equals(host_pid, registers.rsi as usize, b"/dev/console") =>
            {
                write_all(b"emulate surface=virtual-console-mode result=0\n");
                registers.orig_rax = u64::MAX;
                task.awaiting_exit = true;
                task.override_exit = true;
                task.override_result = 0;
                if ptrace(PTRACE_SETREGS, host_pid, 0, &registers as *const _ as usize) < 0
                    || ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0
                {
                    native_fail(instance, pid, b"native virtual console fchmodat failed");
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
                        native_fd_instance_target(host_pid, registers.rdi as usize, &mut target)
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
                    native_fail(instance, pid, b"native virtual console fchmodat2 failed");
                }
            }
            Action::Mediate
                if mode == 3
                    && syscall == 91
                    && (registers.rdi <= 2 || {
                        let mut target = [0u8; 64];
                        native_fd_instance_target(host_pid, registers.rdi as usize, &mut target)
                            .is_some_and(|length| &target[..length] == b"/dev/console")
                    }) =>
            {
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
                    native_fail(instance, pid, b"native virtual console fchmod failed");
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
                    native_fail(instance, pid, b"native virtual console vhangup failed");
                }
            }
            Action::Emulate if syscall == SYS_EXIT || syscall == SYS_EXIT_GROUP => {
                line(b"mediate surface=lifecycle result=pending");
                task.awaiting_exit = false;
                task.override_exit = false;
                if ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0 {
                    native_fail(instance, pid, b"native lifecycle resume failed");
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
                        native_fail(instance, pid, b"native systemd poweroff signal failed");
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
                        native_fail(instance, pid, b"native group signal emulation failed");
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
                            native_fail(instance, pid, b"native task signal translation failed");
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
                            native_fail(
                                instance,
                                pid,
                                b"native unknown signal target denial failed",
                            );
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
                        native_fail(instance, pid, b"native virtual signal denial failed");
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
                        native_fail(instance, pid, b"native pidfd continue emulation failed");
                    }
                } else if allowed {
                    line(b"mediate surface=virtual-pidfd-signal result=pending");
                    task.awaiting_exit = true;
                    if ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0 {
                        native_fail(instance, pid, b"native pidfd signal resume failed");
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
                        native_fail(instance, pid, b"native pidfd signal denial failed");
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
                    native_fail(instance, pid, b"native executor path translation failed");
                }
                write_all(b"translate surface=immutable-systemd-executor result=pending\n");
                registers.rdi = path_address as u64;
                task.awaiting_exit = true;
                if ptrace(PTRACE_SETREGS, host_pid, 0, &registers as *const _ as usize) < 0
                    || ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0
                {
                    native_fail(instance, pid, b"native executor resume failed");
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
                    native_fail(instance, pid, b"native virtual console fstat failed");
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
                    native_fail(instance, pid, b"native virtual console stat failed");
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
                    native_fail(instance, pid, b"native virtual console readlink failed");
                }
            }
            Action::Mediate
                if mode == 3
                    && syscall == ginux_core::SYS_READLINKAT
                    && tracee_decimal_fd_path(host_pid, registers.rsi as usize) =>
            {
                let target: &[u8] = tracee_proc_self_fd_number(host_pid, registers.rsi as usize)
                    .filter(|fd| native_fd_contains(&task.console_fds, *fd))
                    .map_or(b"/memfd:ginux-instance", |_| b"/dev/console");
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
                    native_fail(instance, pid, b"native virtual proc readlink failed");
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
                    native_fail(instance, pid, b"native virtual console termios failed");
                }
            }
            Action::Mediate
                if mode == 3
                    && syscall == 16
                    && native_fd_contains(&task.console_fds, registers.rdi as usize)
                    && registers.rsi == TIOCSCTTY =>
            {
                let task_view = processes.view(task.virtual_id).unwrap_or_else(|_| {
                    native_fail(instance, pid, b"native controlling terminal view failed")
                });
                pid_map.set_foreground_group(task_view.process_group);
                write_all(b"emulate surface=virtual-console-control result=0\n");
                registers.orig_rax = u64::MAX;
                task.awaiting_exit = true;
                task.override_exit = true;
                task.override_result = 0;
                if ptrace(PTRACE_SETREGS, host_pid, 0, &registers as *const _ as usize) < 0
                    || ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0
                {
                    native_fail(instance, pid, b"native virtual console control failed");
                }
            }
            Action::Mediate
                if mode == 3
                    && syscall == 16
                    && native_fd_contains(&task.console_fds, registers.rdi as usize)
                    && matches!(registers.rsi, TIOCGPGRP | TIOCSPGRP) =>
            {
                let task_view = processes.view(task.virtual_id).unwrap_or_else(|_| {
                    native_fail(instance, pid, b"native job control view failed")
                });
                let allowed = if registers.rsi == TIOCGPGRP {
                    write_tracee_bytes(
                        host_pid,
                        registers.rdx as usize,
                        &pid_map.foreground_group().to_ne_bytes(),
                    )
                } else {
                    read_tracee_u32(host_pid, registers.rdx as usize).is_some_and(|group| {
                        if task_view.session != 0
                            && processes
                                .process_group_in_session(task.virtual_id, group)
                                .unwrap_or(false)
                        {
                            pid_map.set_foreground_group(group);
                            true
                        } else {
                            false
                        }
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
                    native_fail(instance, pid, b"native virtual console job control failed");
                }
            }
            Action::Mediate if mode == 3 && syscall == 32 => {
                line(b"mediate surface=managed-descriptor-dup result=pending");
                task.pending_console_duplicate =
                    if native_fd_contains(&task.console_fds, registers.rdi as usize) {
                        1
                    } else {
                        -1
                    };
                task.awaiting_exit = true;
                if ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0 {
                    native_fail(instance, pid, b"native managed descriptor dup failed");
                }
            }
            Action::Mediate if mode == 3 && syscall == 292 && registers.rdx & !0x8_0000 == 0 => {
                line(b"mediate surface=managed-descriptor-dup result=pending");
                task.pending_console_duplicate =
                    if native_fd_contains(&task.console_fds, registers.rdi as usize) {
                        1
                    } else {
                        -1
                    };
                task.awaiting_exit = true;
                if ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0 {
                    native_fail(instance, pid, b"native managed descriptor dup3 failed");
                }
            }
            Action::Mediate if mode == 3 && syscall == 292 => {
                line(b"emulate surface=managed-descriptor-dup result=-22");
                registers.orig_rax = u64::MAX;
                task.awaiting_exit = true;
                task.override_exit = true;
                task.override_result = -EINVAL;
                if ptrace(PTRACE_SETREGS, host_pid, 0, &registers as *const _ as usize) < 0
                    || ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0
                {
                    native_fail(instance, pid, b"native invalid dup3 flags denial failed");
                }
            }
            Action::Mediate if mode == 3 && syscall == 33 => {
                line(b"mediate surface=managed-descriptor-dup result=pending");
                task.pending_console_duplicate =
                    if native_fd_contains(&task.console_fds, registers.rdi as usize) {
                        1
                    } else {
                        -1
                    };
                task.awaiting_exit = true;
                if ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0 {
                    native_fail(instance, pid, b"native managed descriptor dup2 failed");
                }
            }
            Action::Mediate
                if mode == 3
                    && syscall == SYS_FCNTL
                    && matches!(registers.rsi, F_DUPFD | F_DUPFD_CLOEXEC) =>
            {
                line(b"mediate surface=managed-descriptor-fcntl-dup result=pending");
                task.pending_console_duplicate =
                    if native_fd_contains(&task.console_fds, registers.rdi as usize) {
                        1
                    } else {
                        -1
                    };
                task.awaiting_exit = true;
                if ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0 {
                    native_fail(instance, pid, b"native managed descriptor fcntl dup failed");
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
                    native_fail(instance, pid, b"native cgroup membership emulation failed");
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
                    native_fail(instance, pid, b"native virtual console write failed");
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
                    native_fail(instance, pid, b"native virtual cgroup statfs failed");
                }
            }
            Action::Mediate if mode == 3 && syscall == 138 => {
                let virtual_path =
                    native_virtual_kernel_fd(&virtual_kernel_fds, host_pid, registers.rdi as i64)
                        .map(|entry| &entry.path[..entry.path_len]);
                let mut target = [0u8; 512];
                let target_len = if virtual_path.is_none() {
                    native_fd_instance_target(host_pid, registers.rdi as usize, &mut target)
                } else {
                    None
                };
                let fs_type = virtual_path
                    .or_else(|| target_len.map(|length| &target[..length]))
                    .and_then(|path| {
                        if path == b"/proc" || path.starts_with(b"/proc/") {
                            Some(0x9fa0u64)
                        } else if path == b"/sys/fs/cgroup" || path.starts_with(b"/sys/fs/cgroup/")
                        {
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
                        native_fail(instance, pid, b"native invalid fstatfs denial failed");
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
                    native_fail(instance, pid, b"native virtual fstatfs failed");
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
                    native_fail(instance, pid, b"native cgroup descriptor close failed");
                }
            }
            Action::Mediate
                if mode == 3 && syscall == 3 && registers.rdi as usize == INSTANCE_NULL_FD =>
            {
                line(b"mediate surface=virtual-device result=protected");
                registers.orig_rax = u64::MAX;
                task.awaiting_exit = true;
                task.override_exit = true;
                task.override_result = 0;
                if ptrace(PTRACE_SETREGS, host_pid, 0, &registers as *const _ as usize) < 0
                    || ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0
                {
                    native_fail(instance, pid, b"native protected device close failed");
                }
            }
            Action::Mediate
                if mode == 3 && syscall == 3 && registers.rdi as usize == INSTANCE_CONSOLE_FD =>
            {
                line(b"mediate surface=virtual-console result=protected");
                registers.orig_rax = u64::MAX;
                task.awaiting_exit = true;
                task.override_exit = true;
                task.override_result = 0;
                if ptrace(PTRACE_SETREGS, host_pid, 0, &registers as *const _ as usize) < 0
                    || ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0
                {
                    native_fail(instance, pid, b"native protected console close failed");
                }
            }
            Action::Mediate
                if mode == 3 && syscall == 3 && registers.rdi as i64 == task.mountinfo_fd =>
            {
                task.mountinfo_fd = -1;
                line(b"mediate surface=virtual-mount-watch result=pending");
                task.awaiting_exit = true;
                if ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0 {
                    native_fail(instance, pid, b"native mount watch close failed");
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
                    native_fail(instance, pid, b"native task pipe close failed");
                }
            }
            Action::Mediate
                if mode == 3
                    && matches!(syscall, 2 | 257)
                    && (tracee_string_equals(
                        host_pid,
                        if syscall == 2 {
                            registers.rdi as usize
                        } else {
                            registers.rsi as usize
                        },
                        b"/dev/null",
                    ) || tracee_string_equals(
                        host_pid,
                        if syscall == 2 {
                            registers.rdi as usize
                        } else {
                            registers.rsi as usize
                        },
                        b"/dev/console",
                    ) || tracee_string_equals(
                        host_pid,
                        if syscall == 2 {
                            registers.rdi as usize
                        } else {
                            registers.rsi as usize
                        },
                        b"/dev/tty",
                    )) =>
            {
                let path_address = if syscall == 2 {
                    registers.rdi as usize
                } else {
                    registers.rsi as usize
                };
                let null_device = tracee_string_equals(host_pid, path_address, b"/dev/null");
                let source = if null_device {
                    INSTANCE_NULL_FD
                } else {
                    INSTANCE_CONSOLE_FD
                };
                task.pending_console_open = !null_device;
                line(b"mediate surface=virtual-device result=pending");
                registers.orig_rax = SYS_DUP_HOST as u64;
                registers.rdi = source as u64;
                task.awaiting_exit = true;
                if ptrace(PTRACE_SETREGS, host_pid, 0, &registers as *const _ as usize) < 0
                    || ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0
                {
                    native_fail(instance, pid, b"native virtual device open failed");
                }
            }
            Action::Mediate if mode == 3 && syscall == 293 => {
                line(b"mediate surface=task-pipe result=pending");
                task.pipe2_address = registers.rdi as usize;
                task.awaiting_exit = true;
                if ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0 {
                    native_fail(instance, pid, b"native pipe2 resume failed");
                }
            }
            Action::Mediate if virtual_inotify_watch => {
                line(b"emulate surface=virtual-inotify-watch result=1");
                task.awaiting_exit = true;
                task.override_exit = true;
                task.override_result = 1;
                if !native_virtual_kernel_override(host_pid, &mut registers, 1) {
                    native_fail(
                        instance,
                        pid,
                        b"native virtual inotify watch emulation failed",
                    );
                }
            }
            Action::Mediate if mode == 3 && syscall == 254 && inotify_host_path_len.is_some() => {
                let path_address = (registers.rsp as usize).saturating_sub(512);
                let path_len = inotify_host_path_len.unwrap_or(0);
                if !write_tracee_bytes(host_pid, path_address, &inotify_host_path[..=path_len]) {
                    native_fail(instance, pid, b"native inotify path translation failed");
                }
                line(b"translate surface=virtual-inotify-path result=pending");
                registers.rsi = path_address as u64;
                task.awaiting_exit = true;
                if ptrace(PTRACE_SETREGS, host_pid, 0, &registers as *const _ as usize) < 0
                    || ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0
                {
                    native_fail(instance, pid, b"native inotify resume failed");
                }
            }
            Action::Mediate
                if mode == 3
                    && syscall == 233
                    && native_virtual_kernel_fd(
                        &virtual_kernel_fds,
                        host_pid,
                        registers.rdx as i64,
                    )
                    .is_some()
                    && matches!(registers.rsi, 1..=3) =>
            {
                line(b"emulate surface=virtual-kernel-watch result=0");
                registers.orig_rax = u64::MAX;
                task.awaiting_exit = true;
                task.override_exit = true;
                task.override_result = 0;
                if ptrace(PTRACE_SETREGS, host_pid, 0, &registers as *const _ as usize) < 0
                    || ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0
                {
                    native_fail(
                        instance,
                        pid,
                        b"native virtual kernel watch emulation failed",
                    );
                }
            }
            Action::Mediate
                if mode == 3
                    && syscall == ginux_core::SYS_CLOSE_RANGE
                    && registers.rdi >= 3
                    && registers.rdi < INSTANCE_NULL_FD as u64
                    && registers.rsi >= registers.rdi
                    && registers.rdx == 0 =>
            {
                native_virtual_kernel_clear_range(
                    &mut virtual_kernel_fds,
                    host_pid,
                    registers.rdi as i64,
                    core::cmp::min(registers.rsi, i64::MAX as u64) as i64,
                );
                let last = core::cmp::min(registers.rsi, INSTANCE_NULL_FD as u64 - 1);
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
                    native_fail(instance, pid, b"native close range resume failed");
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
                let result = native_link_tracee_fd_into_instance(
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
                    native_fail(instance, pid, b"native proc fd link emulation failed");
                }
            }
            Action::Mediate if mode == 3 && syscall == 435 => {
                let snapshot = native_clone::Pending::snapshot(host_pid, task_index, &registers)
                    .and_then(|mut snapshot| {
                        if snapshot.flags() & 0x2_0000_0000 != 0 {
                            let owned = native_virtual_kernel_fd(
                                &virtual_kernel_fds,
                                host_pid,
                                snapshot.cgroup() as i64,
                            )
                            .is_some_and(|entry| {
                                virtual_kernel_state
                                    .cgroup_directory_exists(&entry.path[..entry.path_len])
                            });
                            if !owned {
                                return Err(-EBADF);
                            }
                            snapshot.clear_cgroup();
                        }
                        Ok(snapshot)
                    });
                task.awaiting_exit = true;
                match snapshot {
                    Ok(snapshot) => {
                        task.clone_call = snapshot;
                        task.clone_call.map(&mut registers);
                        line(b"mediate surface=immutable-clone-arguments result=pending");
                    }
                    Err(error) => {
                        write_all(b"emulate surface=immutable-clone-arguments result=");
                        write_i64(error);
                        write_all(b"\n");
                        registers.orig_rax = u64::MAX;
                        task.override_exit = true;
                        task.override_result = error;
                    }
                }
                if ptrace(PTRACE_SETREGS, host_pid, 0, &registers as *const _ as usize) < 0
                    || ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0
                {
                    native_fail(instance, pid, b"native clone snapshot setup failed");
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
                        // Finish the record before the early continue: the
                        // audit lock is released by its newline.
                        write_all(b"deny result=-3 reason=unknown-process\n");
                        registers.orig_rax = u64::MAX;
                        tasks[task_index] = Some(task);
                        if ptrace(PTRACE_SETREGS, host_pid, 0, &registers as *const _ as usize) < 0
                            || ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0
                        {
                            native_fail(instance, pid, b"native invalid pidfd denial failed");
                        }
                        continue;
                    };
                    task.translated_rdi = registers.rdi;
                    registers.rdi = target_host_pid as u64;
                    if ptrace(PTRACE_SETREGS, host_pid, 0, &registers as *const _ as usize) < 0 {
                        native_fail(instance, pid, b"native pidfd selector translation failed");
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
                        native_fail(instance, pid, b"native linkat fd target resolution failed");
                    };
                    let path_address = (registers.rsp as usize).saturating_sub(512);
                    if !write_tracee_bytes(host_pid, path_address, &target[..=target_len]) {
                        native_fail(instance, pid, b"native linkat fd target translation failed");
                    }
                    registers.rdi = (-100i32) as u32 as u64;
                    registers.rsi = path_address as u64;
                    if ptrace(PTRACE_SETREGS, host_pid, 0, &registers as *const _ as usize) < 0 {
                        native_fail(instance, pid, b"native linkat register translation failed");
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
                            // Finish the record before the early continue: the
                            // audit lock is released by its newline.
                            write_all(b"deny result=-");
                            write_u64(EPERM as u64);
                            write_all(b" reason=unknown-process\n");
                            registers.orig_rax = u64::MAX;
                            tasks[task_index] = Some(task);
                            if ptrace(PTRACE_SETREGS, host_pid, 0, &registers as *const _ as usize)
                                < 0
                                || ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0
                            {
                                native_fail(instance, pid, b"native invalid waitid denial failed");
                            }
                            continue;
                        };
                        registers.rsi = target.host_pid as u64;
                        if ptrace(PTRACE_SETREGS, host_pid, 0, &registers as *const _ as usize) < 0
                        {
                            native_fail(
                                instance,
                                pid,
                                b"native waitid selector translation failed",
                            );
                        }
                    } else if registers.rdi == 3 {
                        // Linux consumes id_t as 32 bits, including for P_PIDFD.
                        let selected_fd = registers.rsi as u32 as usize;
                        let target_host_pid = native_pidfd_host_target(host_pid, selected_fd);
                        let managed = target_host_pid.is_some_and(|selected| {
                            tasks
                                .iter()
                                .flatten()
                                .any(|candidate| candidate.host_pid == selected)
                        });
                        if !managed {
                            task.awaiting_exit = true;
                            task.override_exit = true;
                            task.override_result = if target_host_pid.is_some() {
                                -ECHILD
                            } else {
                                -EBADF
                            };
                            // Finish the record before the early continue: the
                            // audit lock is released by its newline.
                            write_all(b"emulate surface=virtual-pidfd-wait result=");
                            write_i64(task.override_result);
                            write_all(b"\n");
                            registers.orig_rax = u64::MAX;
                            tasks[task_index] = Some(task);
                            if ptrace(PTRACE_SETREGS, host_pid, 0, &registers as *const _ as usize)
                                < 0
                                || ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0
                            {
                                native_fail(
                                    instance,
                                    pid,
                                    b"native invalid pidfd wait denial failed",
                                );
                            }
                            continue;
                        }
                    } else if registers.rdi != 0 {
                        task.awaiting_exit = true;
                        task.override_exit = true;
                        task.override_result = -EPERM;
                        // Finish the record before the early continue: the
                        // audit lock is released by its newline.
                        write_all(b"deny result=-");
                        write_u64(EPERM as u64);
                        write_all(b" reason=unsupported-wait-id-type\n");
                        registers.orig_rax = u64::MAX;
                        tasks[task_index] = Some(task);
                        if ptrace(PTRACE_SETREGS, host_pid, 0, &registers as *const _ as usize) < 0
                            || ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0
                        {
                            native_fail(instance, pid, b"native unsupported waitid denial failed");
                        }
                        continue;
                    }
                }
                if mode == 3 && syscall == 61 {
                    task.wait_status = registers.rsi;
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
                                native_fail(instance, pid, b"native invalid wait denial failed");
                            }
                            continue;
                        };
                        task.translated_rdi = registers.rdi;
                        registers.rdi = target.host_pid as u64;
                        if ptrace(PTRACE_SETREGS, host_pid, 0, &registers as *const _ as usize) < 0
                        {
                            native_fail(instance, pid, b"native wait selector translation failed");
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
                            native_fail(instance, pid, b"native process-group wait denial failed");
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
                if mode != 3 {
                    if let Some(result) = balanced_mediated_result(syscall) {
                        task.override_exit = true;
                        task.override_result = result;
                    }
                }
                if ptrace(PTRACE_SYSCALL, host_pid, 0, 0) < 0 {
                    native_fail(instance, pid, b"native mediated syscall failed");
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
                    native_fail(instance, pid, b"native argument denial failed");
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
                    native_fail(instance, pid, b"native syscall denial failed");
                }
            }
            _ => native_fail(instance, pid, b"invalid native contract decision"),
        }
        tasks[task_index] = Some(task);
    }
}

fn native_clone_start_child(task: &mut NativeSupervisorTask) -> Option<()> {
    if task.clone_call.phase != native_clone::Phase::Child {
        return Some(());
    }
    let mut registers = core::mem::MaybeUninit::<UserRegs>::uninit();
    if ptrace(
        PTRACE_GETREGS,
        task.host_pid,
        0,
        registers.as_mut_ptr() as usize,
    ) < 0
    {
        return None;
    }
    let mut registers = unsafe { registers.assume_init() };
    task.clone_call.unmap(&mut registers, 0);
    if ptrace(
        PTRACE_SETREGS,
        task.host_pid,
        0,
        &registers as *const _ as usize,
    ) < 0
    {
        return None;
    }
    Some(())
}

/// Bootstrap opens the instance root and establishes tracing while privileged.
/// Once the tracee is stopped under `PTRACE_O_EXITKILL`, the supervisor needs
/// only stdio, the optional controller channel (fd 10), and its already-open
/// instance-root descriptor. Drop every other inherited descriptor, host root
/// identity, supplementary group, and privilege-gain path before resuming any
/// managed instruction.
fn reduce_native_supervisor_authority(
    instance: &mut Instance,
    pid: usize,
    root_descriptor: usize,
    preserved_descriptor: usize,
) {
    let mut descriptor = 3usize;
    while descriptor < 256 {
        if descriptor != root_descriptor && descriptor != preserved_descriptor && descriptor != 10 {
            let _ = unsafe { syscall1(SYS_CLOSE_HOST, descriptor) };
        }
        descriptor += 1;
    }
    let _ = unsafe { syscall3(SYS_CLOSE_RANGE_HOST, 256, u32::MAX as usize, 0) };

    let host_identity = managed_host_identity();
    if host_identity.drop_root
        && (unsafe { syscall2(SYS_SETGROUPS, 0, 0) } < 0
            || unsafe { syscall1(SYS_SETGID, host_identity.gid) } < 0
            || unsafe { syscall1(SYS_SETUID, host_identity.uid) } < 0)
    {
        native_fail(
            instance,
            pid,
            b"native supervisor identity reduction failed",
        );
    }
    if unsafe { syscall5(SYS_PRCTL, PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) } < 0 {
        native_fail(instance, pid, b"native supervisor no-new-privileges failed");
    }
    write_all(b"GINUX_NATIVE_AUTHORITY: supervisor-uid=");
    write_u64(host_identity.uid as u64);
    write_all(b" supervisor-gid=");
    write_u64(host_identity.gid as u64);
    line(b" capabilities=none no-new-privileges=1 inherited-fds=bounded root=instance-only");
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
    if ginux_core::signal_driven_io(syscall, registers.rsi, registers.rdx) {
        return false;
    }
    if mode == 3 {
        match syscall {
            ginux_core::SYS_PIDFD_OPEN => registers.rdi > 0 && registers.rsi == 0,
            293 => registers.rdi != 0 && registers.rsi & !(0x8_0000 | 0x800) == 0,
            ginux_core::SYS_CLOSE_RANGE => true,
            452 => {
                registers.rdi < INSTANCE_NULL_FD as u64
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
                (registers.rdi as u32 == (-100i32) as u32
                    || registers.rdi < INSTANCE_NULL_FD as u64)
                    && registers.rsi != 0
                    && registers.rdx & S_IFMT == S_IFIFO
                    && registers.rdx & !(S_IFMT | 0o7777) == 0
                    && registers.r10 == 0
            }
            0 => registers.rdi < VIRTUAL_KERNEL_FD_BASE as u64 && registers.rdx <= 64 * 1024 * 1024,
            18 => registers.rdi < VIRTUAL_KERNEL_FD_BASE as u64 && registers.rdx <= 1024 * 1024,
            25 => {
                registers.rsi <= 1024 * 1024 * 1024
                    && registers.rdx <= 1024 * 1024 * 1024
                    && registers.r10 & !1 == 0
            }
            73 => registers.rdi < INSTANCE_NULL_FD as u64 && registers.rsi & !15 == 0,
            74 | 75 => registers.rdi < INSTANCE_NULL_FD as u64,
            77 => registers.rdi < INSTANCE_NULL_FD as u64,
            98 => {
                (registers.rdi <= 1 || registers.rdi == u64::from(u32::MAX)) && registers.rsi != 0
            }
            79 => registers.rdi != 0 && registers.rsi <= 4096,
            81 | 306 => registers.rdi < INSTANCE_NULL_FD as u64,
            280 => {
                (registers.rdi as u32 == (-100i32) as u32
                    || registers.rdi < INSTANCE_NULL_FD as u64)
                    && registers.r10 & !(0x100 | 0x1000) == 0
            }
            285 => {
                registers.rdi < INSTANCE_NULL_FD as u64
                    && registers.rsi == 0
                    && registers.r10 <= 1024 * 1024 * 1024
            }
            ginux_core::SYS_LINKAT => {
                let old_directory = registers.rdi as u32 == (-100i32) as u32
                    || registers.rdi < INSTANCE_NULL_FD as u64;
                let new_directory = registers.rdx as u32 == (-100i32) as u32
                    || registers.rdx < INSTANCE_NULL_FD as u64;
                old_directory && new_directory && registers.r8 & !(0x1000 | 0x400) == 0
            }
            190 => {
                registers.rdi < INSTANCE_NULL_FD as u64
                    && registers.r10 <= 64 * 1024
                    && registers.r8 <= 2
            }
            196 => registers.rdi < INSTANCE_NULL_FD as u64 && registers.rdx <= 64 * 1024,
            199 => registers.rdi < INSTANCE_NULL_FD as u64,
            1
            | 2
            | 7
            | 15
            | 16
            | 20
            | 24
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

fn native_socket_protocol(task: &NativeSupervisorTask, fd: u64) -> u8 {
    task.socket_fds
        .iter()
        .find(|entry| entry.fd == fd as i64)
        .map_or(0, |entry| entry.protocol)
}

fn native_bind_socket_source(pid: usize, fd: i64, source: [u8; 4]) -> bool {
    const SYS_PIDFD_OPEN: usize = 434;
    const SYS_PIDFD_GETFD: usize = 438;
    const SYS_BIND: usize = 49;
    let pidfd = unsafe { syscall2(SYS_PIDFD_OPEN, pid, 0) };
    if pidfd < 0 {
        return false;
    }
    let duplicate = unsafe { syscall3(SYS_PIDFD_GETFD, pidfd as usize, fd as usize, 0) };
    if duplicate < 0 {
        let _ = unsafe { syscall1(SYS_CLOSE_HOST, pidfd as usize) };
        return false;
    }
    let mut address = [0u8; 16];
    address[0] = 2;
    address[4..8].copy_from_slice(&source);
    let result = unsafe {
        syscall3(
            SYS_BIND,
            duplicate as usize,
            address.as_ptr() as usize,
            address.len(),
        )
    };
    let _ = unsafe { syscall1(SYS_CLOSE_HOST, duplicate as usize) };
    let _ = unsafe { syscall1(SYS_CLOSE_HOST, pidfd as usize) };
    result == 0
}

fn native_sockaddr_port(pid: usize, address: usize, length: u64) -> Option<(u64, u16)> {
    if address == 0 || length < 4 {
        return None;
    }
    let mut word = 0u64;
    if ptrace(PTRACE_PEEKDATA, pid, address, &mut word as *mut _ as usize) < 0 {
        return None;
    }
    let family = word & 0xffff;
    if family != 2 && family != 10 {
        return None;
    }
    let port = ((((word >> 16) & 0xff) << 8) | ((word >> 24) & 0xff)) as u16;
    Some((word, port))
}

fn native_sockaddr_with_port(word: u64, port: u16) -> u64 {
    let network = port.to_be_bytes();
    (word & !0xffff_0000) | (u64::from(network[0]) << 16) | (u64::from(network[1]) << 24)
}

fn native_sockaddr_with_source(word: u64, source: [u8; 4]) -> u64 {
    let mut value = word & 0x0000_0000_ffff_ffff;
    value |= u64::from(source[0]) << 32;
    value |= u64::from(source[1]) << 40;
    value |= u64::from(source[2]) << 48;
    value |= u64::from(source[3]) << 56;
    value
}

fn native_fail(instance: &mut Instance, pid: usize, reason: &[u8]) -> ! {
    kill_tracee(pid);
    instance_fail(instance, reason)
}

/// Ends a traced child with `kill(2)` and `SIGKILL` and reaps it, as
/// `launch_fail` does. `PTRACE_KILL` only as a fallback: a seccomp policy that
/// refuses ptrace requests may refuse it as well, and the reap would then block
/// with the tracee left in its stop (and running on, untraced, once the
/// supervisor is gone before `PTRACE_O_EXITKILL` is set).
///
/// Some callers have already reaped the child with the wait that reported it
/// gone. A nonblocking wait first tells them apart: it fails once the child is
/// reaped and reaps it if it has exited, so `kill(2)` only targets a child that
/// is still ours and whose pid cannot have been reused.
fn kill_tracee(pid: usize) {
    let mut status = 0i32;
    let reaped = unsafe {
        syscall4(
            SYS_WAIT4,
            pid,
            &mut status as *mut _ as usize,
            WNOHANG as usize,
            0,
        )
    };
    if reaped < 0 || (reaped as usize == pid && !stopped(status)) {
        return;
    }
    if unsafe { syscall2(SYS_KILL, pid, SIGKILL) } < 0 {
        let _ = ptrace(PTRACE_KILL, pid, 0, 0);
    }
    let _ = wait(pid, &mut status);
}

fn config_main(config_path: *const u8) -> ! {
    let config = load_config(config_path);
    print_effective_config(&config);
    line(b"GINUX_CONFIG: PASS");
    exit_process(0)
}

fn config_merge_main(config_path: *const u8, count: usize, overrides: *const usize) -> ! {
    let mut config = load_config(config_path);
    let mut index = 0usize;
    while index < count {
        let argument = unsafe { *overrides.add(index) as *const u8 };
        let bytes = c_string_bytes(argument, 1024).unwrap_or_else(|error| config_rejected(error));
        let equals = bytes
            .iter()
            .position(|byte| *byte == b'=')
            .unwrap_or_else(|| config_rejected(ginux_instance::ConfigError::InvalidSyntax));
        let key = &bytes[..equals];
        let value = &bytes[equals + 1..];
        if key.is_empty() || value.is_empty() {
            config_rejected(ginux_instance::ConfigError::InvalidSyntax);
        }
        config
            .apply_override(key, value)
            .unwrap_or_else(|error| config_rejected(error));
        index += 1;
    }
    let normalized = config
        .normalized()
        .unwrap_or_else(|error| config_rejected(error));
    write_all(normalized.as_bytes());
    exit_process(0)
}

fn instance_plan_main(config_path: *const u8) -> ! {
    let config = load_config(config_path);
    let contract = config
        .effective_contract()
        .unwrap_or_else(|error| config_rejected(error));
    write_all(b"GINUX_CONTRACT: catalog=");
    write_u64(ginux_contract::CATALOG_VERSION as u64);
    write_all(b" instance=");
    write_all(config.id.as_bytes());
    write_all(b" profile=");
    if config.contract_recipe_version == 0 {
        write_all(config.contract_profile.as_bytes());
    } else {
        write_all(b"self-contained-v1");
    }
    write_all(b" digest=");
    write_all(contract.digest());
    write_all(b" operations=");
    write_u64(contract.rules().len() as u64);
    write_all(b"\n");
    for rule in contract.rules() {
        write_all(b"GINUX_CONTRACT_RULE: operation=");
        write_all(rule.operation.id.as_bytes());
        write_all(b" action=");
        write_all(rule.action.name().as_bytes());
        write_all(b" provenance=");
        write_all(rule.provenance.name().as_bytes());
        write_all(b" source=");
        write_all(rule.source.as_bytes());
        write_all(b" source-rule=");
        write_u64(rule.source_rule as u64);
        write_all(b" status=");
        write_all(rule.operation.status.name().as_bytes());
        write_all(b"\n");
    }
    line(b"GINUX_CONTRACT: PASS");
    exit_process(0)
}

fn instance_validate_main(config_path: *const u8) -> ! {
    let config = load_config(config_path);
    let contract = config
        .effective_contract()
        .unwrap_or_else(|error| config_rejected(error));
    write_all(b"Contract valid: instance=");
    write_all(config.id.as_bytes());
    write_all(b" catalog=");
    write_u64(ginux_contract::CATALOG_VERSION as u64);
    write_all(b" digest=");
    write_all(contract.digest());
    write_all(b" operations=");
    write_u64(contract.rules().len() as u64);
    write_all(b"\n");
    exit_process(0)
}

fn instance_explain_main(config_path: *const u8, operation: *const u8) -> ! {
    let config = load_config(config_path);
    let operation = c_string_bytes(operation, 128).unwrap_or_else(|error| config_rejected(error));
    let contract = config
        .effective_contract()
        .unwrap_or_else(|error| config_rejected(error));
    let Some(rule) = contract
        .rules()
        .iter()
        .find(|rule| rule.operation.id.as_bytes() == operation)
    else {
        write_all(b"Contract operation is not present in catalog version ");
        write_u64(ginux_contract::CATALOG_VERSION as u64);
        line(b".");
        exit_process(2)
    };
    write_all(b"Operation: ");
    line(rule.operation.id.as_bytes());
    write_all(b"Decision: ");
    line(rule.action.name().as_bytes());
    write_all(b"Rule: ");
    write_all(rule.provenance.name().as_bytes());
    write_all(b" source=");
    write_all(rule.source.as_bytes());
    write_all(b" index=");
    write_u64(rule.source_rule as u64);
    write_all(b"\nStatus: ");
    line(rule.operation.status.name().as_bytes());
    write_all(b"Constraint: ");
    line(rule.operation.constraint.as_bytes());
    write_all(b"Limitation: ");
    line(rule.operation.limitation.as_bytes());
    exit_process(0)
}

fn instance_diff_main(left_path: *const u8, right_path: *const u8) -> ! {
    let left = load_config(left_path)
        .effective_contract()
        .unwrap_or_else(|error| config_rejected(error));
    let right = load_config(right_path)
        .effective_contract()
        .unwrap_or_else(|error| config_rejected(error));
    let mut changes = 0u64;
    for (left_rule, right_rule) in left.rules().iter().zip(right.rules()) {
        if left_rule.action != right_rule.action {
            changes += 1;
            write_all(b"Changed: ");
            write_all(left_rule.operation.id.as_bytes());
            write_all(b" ");
            write_all(left_rule.action.name().as_bytes());
            write_all(b" -> ");
            line(right_rule.action.name().as_bytes());
        }
    }
    write_all(b"Contract differences: ");
    write_u64(changes);
    write_all(b"\n");
    exit_process(0)
}

fn instance_inspect_main(config_path: *const u8) -> ! {
    let config = load_config(config_path);
    let contract = config
        .effective_contract()
        .unwrap_or_else(|error| config_rejected(error));
    let mut emulate = 0u64;
    let mut mediate = 0u64;
    let mut deny = 0u64;
    for rule in contract.rules() {
        match rule.action {
            ContractAction::Emulate => emulate += 1,
            ContractAction::Mediate => mediate += 1,
            ContractAction::Deny(_) => deny += 1,
        }
    }
    write_all(b"Contract: instance=");
    write_all(config.id.as_bytes());
    write_all(b" catalog=");
    write_u64(ginux_contract::CATALOG_VERSION as u64);
    write_all(b" digest=");
    write_all(contract.digest());
    write_all(b"\nDecisions: emulate=");
    write_u64(emulate);
    write_all(b" mediate=");
    write_u64(mediate);
    write_all(b" deny=");
    write_u64(deny);
    write_all(b" total=");
    write_u64(contract.rules().len() as u64);
    write_all(b"\n");
    exit_process(0)
}

fn hook_plan_main(config_path: *const u8) -> ! {
    let config = load_config(config_path);
    write_u64(config.pre_boot_hooks.as_slice().len() as u64);
    write_all(b" ");
    write_u64(config.hook_timeout_seconds);
    write_all(b" ");
    line(match config.hook_failure_policy {
        HookFailurePolicy::Stop => b"stop",
        HookFailurePolicy::Continue => b"continue",
    });
    exit_process(0)
}

fn instance_main(config_path: *const u8, fixture_path: *const u8) -> ! {
    let config = load_config(config_path);
    print_effective_config(&config);
    let mut instance = Instance::create(config);
    lifecycle_state(&instance);
    if instance.start().is_err() {
        instance_fail(&mut instance, b"invalid start transition");
    }
    lifecycle_state(&instance);

    let length =
        read_file(fixture_path).unwrap_or_else(|reason| instance_fail(&mut instance, reason));
    let elf =
        unsafe { core::slice::from_raw_parts(core::ptr::addr_of!(ELF_BYTES).cast::<u8>(), length) };
    let mut memory = AddressSpace::new();
    let loaded = match memory.load_elf(elf) {
        Ok(loaded) => loaded,
        Err(_) => instance_fail(&mut instance, b"managed ELF rejected"),
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
        Err(_) => instance_fail(&mut instance, b"managed stack rejected"),
    };
    let report = match execute(&memory, loaded.entry, stack, 10_000) {
        Ok(report) => report,
        Err(_) => instance_fail(&mut instance, b"managed execution failed"),
    };
    if report.exit_code != 0 {
        instance_fail(&mut instance, b"managed task failed");
    }
    write_all(b"GINUX_INSTANCE: managed-output=");
    write_all(&report.output[..report.output_len]);
    write_all(b"\n");
    if instance.task_exited(true).is_err() {
        instance_fail(&mut instance, b"invalid task-exit transition");
    }
    lifecycle_state(&instance);
    line(b"GINUX_INSTANCE: cleanup resources=false managed-task=false");
    line(b"GATE4: PASS");
    exit_process(0)
}

fn lifecycle_main(config_path: *const u8) -> ! {
    let config = load_config(config_path);
    let mut instance = Instance::create(config);
    lifecycle_state(&instance);
    if instance.start().is_err() {
        instance_fail(&mut instance, b"invalid start transition");
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
        instance_fail(&mut instance, b"signal mask setup failed");
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
        instance_fail(&mut instance, b"signalfd setup failed");
    }
    let child = unsafe { syscall0(SYS_FORK) };
    if child < 0 {
        let _ = unsafe { syscall1(SYS_CLOSE_HOST, signal_fd as usize) };
        instance_fail(&mut instance, b"helper fork failed");
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
        instance_fail(&mut instance, b"signal wait failed");
    }
    line(b"GINUX_LIFECYCLE: signal=SIGTERM decision=stop");
    let _ = unsafe { syscall2(SYS_KILL, child as usize, SIGKILL) };
    let mut status = 0;
    if wait(child as usize, &mut status) != child {
        let _ = unsafe { syscall1(SYS_CLOSE_HOST, signal_fd as usize) };
        instance_fail(&mut instance, b"helper reap failed");
    }
    let _ = unsafe { syscall1(SYS_CLOSE_HOST, signal_fd as usize) };
    if instance.stop().is_err() {
        instance_fail(&mut instance, b"invalid stop transition");
    }
    line(b"GINUX_LIFECYCLE: helper=reaped zombies=0");
    lifecycle_state(&instance);
    line(b"GINUX_INSTANCE: cleanup resources=false managed-task=false");
    line(b"GATE4_LIFECYCLE: PASS");
    exit_process(0)
}

fn print_instance_start_summary(config: &InstanceConfig) {
    write_all(b"GINUX_INSTANCE_START: id=");
    write_all(config.id.as_bytes());
    write_all(b" init=\"");
    write_all(config.init.as_bytes());
    for argument in config.args.as_slice() {
        write_all(b" ");
        write_all(argument.as_bytes());
    }
    write_all(b"\" resources=");
    write_u64(config.vcpus);
    write_all(b"-vcpu,");
    write_u64(config.memory_mib);
    write_all(b"-MiB execution=");
    write_all(match config.execution_profile {
        ExecutionProfile::Security => b"security",
        ExecutionProfile::Balanced => b"balanced",
        ExecutionProfile::Performance => b"performance",
    });
    write_all(b"/");
    write_all(match config.execution_backend {
        ExecutionBackend::Interpreter => b"interpreter",
        ExecutionBackend::PtraceSysemu => b"ptrace-sysemu",
        ExecutionBackend::PtraceNative => b"ptrace-native",
    });
    write_all(b" dispatch=");
    write_all(match config.dispatch_mode {
        DispatchMode::Paranoid => b"paranoid",
        DispatchMode::Selective => b"selective",
    });
    write_all(b" network=");
    line(match config.network_backend {
        NetworkBackend::None => b"none",
        NetworkBackend::Mediated => b"mediated",
        NetworkBackend::Shared => b"shared",
    });
}
fn lifecycle_state(instance: &Instance) {
    write_all(b"GINUX_LIFECYCLE: state=");
    write_all(instance.state().as_str().as_bytes());
    write_all(b" resources=");
    write_all(if instance.resources_acquired() {
        b"true"
    } else {
        b"false"
    });
    write_all(b" managed-task=");
    line(if instance.managed_task() {
        b"true"
    } else {
        b"false"
    });
}

fn instance_fail(instance: &mut Instance, reason: &[u8]) -> ! {
    let _ = instance.task_exited(false);
    write_all(b"GINUX_INSTANCE: failed reason=");
    line(reason);
    lifecycle_state(instance);
    line(b"GINUX_INSTANCE: cleanup resources=false managed-task=false");
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
    let length = read_file(path).unwrap_or_else(|reason| runtime_fail(reason));
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

    let host_identity = managed_host_identity();
    if host_identity.drop_root
        && (unsafe { syscall2(SYS_SETGROUPS, 0, 0) } < 0
            || unsafe { syscall1(SYS_SETGID, host_identity.gid) } < 0
            || unsafe { syscall1(SYS_SETUID, host_identity.uid) } < 0)
    {
        unsafe { syscall1(SYS_EXIT_HOST, 112) };
        loop {
            core::hint::spin_loop();
        }
    }
    if unsafe { syscall5(SYS_PRCTL, PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) } < 0 {
        unsafe { syscall1(SYS_EXIT_HOST, 113) };
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

fn supervise_setup_fail(pid: usize, reason: &[u8]) -> ! {
    kill_tracee(pid);
    fail(reason)
}

fn supervise(pid: usize) -> ! {
    let mut status = 0i32;
    if wait(pid, &mut status) < 0 || !stopped(status) {
        fail(b"payload did not stop after exec");
    }

    // Until PTRACE_O_EXITKILL is in place, exiting would detach the stopped
    // payload and let it run untraced. Both setup failures kill and reap it
    // before reporting, so it never runs past its stop after execve.
    let options = PTRACE_O_TRACESYSGOOD | PTRACE_O_EXITKILL;
    if ptrace(PTRACE_SETOPTIONS, pid, 0, options) < 0 {
        supervise_setup_fail(pid, b"PTRACE_SETOPTIONS failed");
    }
    if ptrace(PTRACE_SYSEMU, pid, 0, 0) < 0 {
        supervise_setup_fail(pid, b"PTRACE_SYSEMU unavailable");
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
                kill_tracee(pid);
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

fn native_limit_cpus(pid: usize, vcpus: u64) -> bool {
    if vcpus == 0 || vcpus > 64 {
        return false;
    }
    let mask = if vcpus == 64 {
        u64::MAX
    } else {
        (1u64 << vcpus) - 1
    };
    unsafe {
        syscall3(
            SYS_SCHED_SETAFFINITY_HOST,
            pid,
            core::mem::size_of::<u64>(),
            &mask as *const _ as usize,
        ) >= 0
    }
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

#[allow(dead_code)]
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
                return native_fd_instance_target(host_pid, fd, output);
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

fn native_proc_link_instance_target(
    host_pid: usize,
    suffix: &[u8],
    output: &mut [u8],
) -> Option<usize> {
    let mut root_link = [0u8; 64];
    let mut root_len = 0usize;
    native_buffer_append(&mut root_link, &mut root_len, b"/proc/")?;
    native_buffer_append_u32(&mut root_link, &mut root_len, u32::try_from(host_pid).ok()?)?;
    native_buffer_append(&mut root_link, &mut root_len, b"/root")?;
    root_link[root_len] = 0;

    let mut target_link = [0u8; 96];
    let mut target_len = 0usize;
    native_buffer_append(&mut target_link, &mut target_len, b"/proc/")?;
    native_buffer_append_u32(
        &mut target_link,
        &mut target_len,
        u32::try_from(host_pid).ok()?,
    )?;
    native_buffer_append(&mut target_link, &mut target_len, suffix)?;
    target_link[target_len] = 0;

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
    let mut target = [0u8; 512];
    let target_count = unsafe {
        syscall4(
            ginux_core::SYS_READLINKAT as usize,
            AT_FDCWD,
            target_link.as_ptr() as usize,
            target.as_mut_ptr() as usize,
            target.len(),
        )
    };
    if root_count <= 0 || target_count < root_count {
        return None;
    }
    let root_count = root_count as usize;
    let target_count = target_count as usize;
    if target[..root_count] != root_target[..root_count] {
        return None;
    }
    let suffix = if target_count == root_count {
        b"/".as_slice()
    } else {
        if target[root_count] != b'/' {
            return None;
        }
        &target[root_count..target_count]
    };
    if suffix.len() >= output.len() {
        return None;
    }
    output[..suffix.len()].copy_from_slice(suffix);
    Some(suffix.len())
}

fn native_instance_path(
    host_pid: usize,
    dirfd: u64,
    address: usize,
    output: &mut [u8],
) -> Option<usize> {
    let mut raw = [0u8; 512];
    let raw_len = read_tracee_c_string(host_pid, address, &mut raw)?;
    if raw_len == 0 {
        return None;
    }
    let mut joined = [0u8; 768];
    let mut joined_len = 0usize;
    if raw[0] != b'/' {
        let mut base = [0u8; 512];
        let base_len = if dirfd as u32 == (-100i32) as u32 {
            native_proc_link_instance_target(host_pid, b"/cwd", &mut base)?
        } else {
            native_fd_instance_target(host_pid, dirfd as usize, &mut base)?
        };
        native_buffer_append(&mut joined, &mut joined_len, &base[..base_len])?;
        if joined_len == 0 || joined[joined_len - 1] != b'/' {
            native_buffer_append(&mut joined, &mut joined_len, b"/")?;
        }
    }
    native_buffer_append(&mut joined, &mut joined_len, &raw[..raw_len])?;

    native_normalize_instance_path(&joined[..joined_len], output)
}

fn native_normalize_instance_path(input: &[u8], output: &mut [u8]) -> Option<usize> {
    let mut length = 1usize;
    if output.is_empty() {
        return None;
    }
    output[0] = b'/';
    for component in input.split(|byte| *byte == b'/') {
        if component.is_empty() || component == b"." {
            continue;
        }
        if component == b".." {
            if length > 1 {
                length -= 1;
                while length > 1 && output[length - 1] != b'/' {
                    length -= 1;
                }
            }
            continue;
        }
        if length > 1 {
            if length >= output.len() {
                return None;
            }
            output[length] = b'/';
            length += 1;
        }
        if length + component.len() >= output.len() {
            return None;
        }
        output[length..length + component.len()].copy_from_slice(component);
        length += component.len();
    }
    Some(length)
}

fn native_resolve_instance_path(
    root_descriptor: usize,
    path: &[u8],
    output: &mut [u8],
) -> Option<usize> {
    const SYS_NEWFSTATAT_HOST: usize = 262;
    const AT_SYMLINK_NOFOLLOW: usize = 0x100;
    const S_IFMT: u32 = 0o170000;
    const S_IFLNK: u32 = 0o120000;
    const MAX_SYMLINKS: usize = 16;

    let mut pending = [0u8; 1024];
    let mut pending_len = native_normalize_instance_path(path, &mut pending)?;
    for _ in 0..MAX_SYMLINKS {
        let mut current = [0u8; 512];
        let mut current_len = 0usize;
        let mut offset = 1usize;
        let mut followed = false;
        while offset < pending_len {
            let component_end = pending[offset..pending_len]
                .iter()
                .position(|byte| *byte == b'/')
                .map_or(pending_len, |position| offset + position);
            let component = &pending[offset..component_end];
            if current_len != 0 {
                current[current_len] = b'/';
                current_len += 1;
            }
            if current_len + component.len() + 1 > current.len() {
                return None;
            }
            current[current_len..current_len + component.len()].copy_from_slice(component);
            current_len += component.len();
            current[current_len] = 0;

            let mut stat = [0u8; 144];
            let result = unsafe {
                syscall4(
                    SYS_NEWFSTATAT_HOST,
                    root_descriptor,
                    current.as_ptr() as usize,
                    stat.as_mut_ptr() as usize,
                    AT_SYMLINK_NOFOLLOW,
                )
            };
            if result < 0 {
                if pending_len > output.len() {
                    return None;
                }
                output[..pending_len].copy_from_slice(&pending[..pending_len]);
                return Some(pending_len);
            }
            let mode = u32::from_ne_bytes([stat[24], stat[25], stat[26], stat[27]]);
            if mode & S_IFMT == S_IFLNK {
                let mut target = [0u8; 512];
                let target_len = unsafe {
                    syscall4(
                        ginux_core::SYS_READLINKAT as usize,
                        root_descriptor,
                        current.as_ptr() as usize,
                        target.as_mut_ptr() as usize,
                        target.len(),
                    )
                };
                if target_len <= 0 {
                    return None;
                }
                let target_len = target_len as usize;
                let mut joined = [0u8; 1024];
                let mut joined_len = 0usize;
                if target[0] != b'/' {
                    native_buffer_append(&mut joined, &mut joined_len, b"/")?;
                    if let Some(separator) = current[..current_len]
                        .iter()
                        .rposition(|byte| *byte == b'/')
                    {
                        native_buffer_append(
                            &mut joined,
                            &mut joined_len,
                            &current[..separator + 1],
                        )?;
                    }
                }
                native_buffer_append(&mut joined, &mut joined_len, &target[..target_len])?;
                native_buffer_append(
                    &mut joined,
                    &mut joined_len,
                    &pending[component_end..pending_len],
                )?;
                let mut next = [0u8; 1024];
                pending_len = native_normalize_instance_path(&joined[..joined_len], &mut next)?;
                pending[..pending_len].copy_from_slice(&next[..pending_len]);
                followed = true;
                break;
            }
            offset = component_end.saturating_add(1);
        }
        if !followed {
            if pending_len > output.len() {
                return None;
            }
            output[..pending_len].copy_from_slice(&pending[..pending_len]);
            return Some(pending_len);
        }
    }
    None
}

fn native_write_path_denied(
    config: &InstanceConfig,
    root_descriptor: usize,
    host_pid: usize,
    dirfd: u64,
    address: usize,
) -> bool {
    if config.filesystem_write_deny.as_slice().is_empty() {
        return false;
    }
    let mut path = [0u8; 512];
    let Some(length) = native_instance_path(host_pid, dirfd, address, &mut path) else {
        return true;
    };
    let mut resolved = [0u8; 512];
    let Some(resolved_len) =
        native_resolve_instance_path(root_descriptor, &path[..length], &mut resolved)
    else {
        return true;
    };
    let denied = config.filesystem_write_denied(&resolved[..resolved_len]);
    if denied {
        write_all(b"GINUX_POLICY_PATH: requested=");
        write_all(&path[..length]);
        write_all(b" resolved=");
        write_all(&resolved[..resolved_len]);
        write_all(b"\n");
    }
    denied
}

fn native_setid_path_denied(
    config: &InstanceConfig,
    root_descriptor: usize,
    host_pid: usize,
    dirfd: u64,
    address: usize,
    mode: u64,
) -> bool {
    if mode & 0o6000 == 0 {
        return false;
    }
    let mut path = [0u8; 512];
    let Some(length) = native_instance_path(host_pid, dirfd, address, &mut path) else {
        return true;
    };
    let mut resolved = [0u8; 512];
    let Some(resolved_len) =
        native_resolve_instance_path(root_descriptor, &path[..length], &mut resolved)
    else {
        return true;
    };
    config.privilege_setid_denied(
        &resolved[..resolved_len],
        mode & 0o4000 != 0,
        mode & 0o2000 != 0,
    )
}

fn native_setid_fd_denied(config: &InstanceConfig, host_pid: usize, fd: usize, mode: u64) -> bool {
    if mode & 0o6000 == 0 {
        return false;
    }
    let mut path = [0u8; 512];
    let Some(length) = native_fd_instance_target(host_pid, fd, &mut path) else {
        return true;
    };
    config.privilege_setid_denied(&path[..length], mode & 0o4000 != 0, mode & 0o2000 != 0)
}

fn native_device_create_denied(
    config: &InstanceConfig,
    root_descriptor: usize,
    host_pid: usize,
    dirfd: u64,
    address: usize,
) -> bool {
    let mut path = [0u8; 512];
    let Some(length) = native_instance_path(host_pid, dirfd, address, &mut path) else {
        return true;
    };
    let mut resolved = [0u8; 512];
    let Some(resolved_len) =
        native_resolve_instance_path(root_descriptor, &path[..length], &mut resolved)
    else {
        return true;
    };
    config.device_create_denied(&resolved[..resolved_len])
}

fn native_link_tracee_fd_into_instance(
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

    let mut instance_path = [0u8; 384];
    let Some(instance_path_len) =
        read_tracee_c_string(host_pid, destination_address, &mut instance_path)
    else {
        return -14;
    };
    if instance_path_len == 0
        || instance_path[0] != b'/'
        || !instance_path[..instance_path_len].starts_with(b"/run/systemd/propagate/")
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
            &instance_path[..instance_path_len],
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

fn native_fd_instance_target(host_pid: usize, fd: usize, output: &mut [u8]) -> Option<usize> {
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
    if root_count <= 0 || fd_count < root_count {
        return None;
    }
    let root_count = root_count as usize;
    let fd_count = fd_count as usize;
    if fd_target[..root_count] != root_target[..root_count] {
        return None;
    }
    if fd_count == root_count {
        output[0] = b'/';
        output[1] = 0;
        return Some(1);
    }
    if fd_target[root_count] != b'/' {
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
    retired: &[NativeRetiredCredential; 128],
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
    } else if let Some(sender) = retired
        .iter()
        .find(|candidate| candidate.lifetime_pidfd >= 0 && candidate.host_pid == sender_host_pid)
    {
        encoded[..4].copy_from_slice(&sender.virtual_pid.to_ne_bytes());
        encoded[4..8].copy_from_slice(&sender.uid.to_ne_bytes());
        encoded[8..12].copy_from_slice(&sender.gid.to_ne_bytes());
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
    retired: &[NativeRetiredCredential; 128],
    current: NativeSupervisorTask,
    processes: &ProcessTable<128>,
) -> bool {
    let Some(sender_host_pid) = read_tracee_u32(tracee_pid, address).map(|pid| pid as usize) else {
        return false;
    };
    let Some(encoded) = native_virtual_ucred(sender_host_pid, tasks, retired, current, processes)
    else {
        return false;
    };
    write_tracee_bytes(tracee_pid, address, &encoded)
}

fn native_rewrite_recvmsg_credentials(
    tracee_pid: usize,
    message_address: usize,
    tasks: &[Option<NativeSupervisorTask>; 128],
    retired: &[NativeRetiredCredential; 128],
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
            if !native_rewrite_ucred(
                tracee_pid,
                credential_address,
                tasks,
                retired,
                current,
                processes,
            ) {
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

#[allow(dead_code)]
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

fn write_i64_to(fd: usize, value: i64) {
    if value < 0 {
        write_all_to(fd, b"-");
        write_u64_to(fd, value.unsigned_abs());
    } else {
        write_u64_to(fd, value as u64);
    }
}
fn fail(reason: &[u8]) -> ! {
    write_all(b"GATE1: FAIL: ");
    line(reason);
    exit_process(1)
}
