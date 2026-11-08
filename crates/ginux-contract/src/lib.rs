#![no_std]

use core::fmt;

pub const CATALOG_VERSION: u32 = 2;
pub const MAX_OPERATIONS: usize = 192;
pub const DIGEST_HEX_BYTES: usize = 64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuditDisposition {
    Emit,
    Drop,
    DropAndReport(u64),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuditBudget {
    limit: u64,
    emitted: u64,
    dropped: u64,
}

impl AuditBudget {
    pub const fn new(limit: u64) -> Self {
        Self {
            limit,
            emitted: 0,
            dropped: 0,
        }
    }

    pub fn admit(&mut self) -> AuditDisposition {
        if self.emitted < self.limit {
            self.emitted += 1;
            return AuditDisposition::Emit;
        }
        self.dropped = self.dropped.saturating_add(1);
        if self.dropped.is_power_of_two() {
            AuditDisposition::DropAndReport(self.dropped)
        } else {
            AuditDisposition::Drop
        }
    }

    pub const fn emitted(self) -> u64 {
        self.emitted
    }

    pub const fn dropped(self) -> u64 {
        self.dropped
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Action {
    Emulate,
    Mediate,
    Deny(i32),
}

impl Action {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Emulate => "emulate",
            Self::Mediate => "mediate",
            Self::Deny(_) => "deny",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Status {
    Complete,
    Partial,
    Denied,
    Planned,
    OutOfScope,
}

impl Status {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::Partial => "partial",
            Self::Denied => "denied",
            Self::Planned => "planned",
            Self::OutOfScope => "out-of-scope",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Kind {
    Syscall,
    Ioctl,
    Network,
    Device,
    Filesystem,
    Proc,
    Sys,
    Cgroup,
    Credential,
    Signal,
    Time,
    Memory,
}

impl Kind {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Syscall => "syscall",
            Self::Ioctl => "ioctl",
            Self::Network => "network",
            Self::Device => "device",
            Self::Filesystem => "filesystem",
            Self::Proc => "proc",
            Self::Sys => "sys",
            Self::Cgroup => "cgroup",
            Self::Credential => "credential",
            Self::Signal => "signal",
            Self::Time => "time",
            Self::Memory => "memory",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Operation {
    pub id: &'static str,
    pub kind: Kind,
    pub abi_number: Option<u64>,
    pub status: Status,
    pub action: Action,
    pub direct_safe: bool,
    pub constraint: &'static str,
    pub evidence: &'static str,
    pub limitation: &'static str,
}

macro_rules! syscall {
    ($name:literal, $number:literal, $status:ident, $action:expr, $constraint:literal, $evidence:literal) => {
        Operation {
            id: concat!("syscall.", $name),
            kind: Kind::Syscall,
            abi_number: Some($number),
            status: Status::$status,
            action: $action,
            direct_safe: false,
            constraint: $constraint,
            evidence: $evidence,
            limitation: "x86-64 Linux ABI only",
        }
    };
}

macro_rules! direct_syscall {
    ($name:literal, $number:literal, $status:ident, $action:expr, $constraint:literal, $evidence:literal) => {
        Operation {
            id: concat!("syscall.", $name),
            kind: Kind::Syscall,
            abi_number: Some($number),
            status: Status::$status,
            action: $action,
            direct_safe: true,
            constraint: $constraint,
            evidence: $evidence,
            limitation: "x86-64 Linux ABI only",
        }
    };
}

macro_rules! operation {
    ($id:literal, $kind:ident, $status:ident, $action:expr, $constraint:literal, $evidence:literal, $limitation:literal) => {
        Operation {
            id: $id,
            kind: Kind::$kind,
            abi_number: None,
            status: Status::$status,
            action: $action,
            direct_safe: false,
            constraint: $constraint,
            evidence: $evidence,
            limitation: $limitation,
        }
    };
}

pub static CATALOG: &[Operation] = &[
    syscall!(
        "read",
        0,
        Partial,
        Action::Mediate,
        "bounded descriptor and length",
        "test-systemd"
    ),
    syscall!(
        "write",
        1,
        Partial,
        Action::Mediate,
        "bounded descriptor and length",
        "test-systemd"
    ),
    syscall!(
        "open",
        2,
        Partial,
        Action::Mediate,
        "instance path and validated flags",
        "test-native-vfs"
    ),
    syscall!(
        "close",
        3,
        Complete,
        Action::Mediate,
        "instance descriptor",
        "test-systemd"
    ),
    syscall!(
        "stat",
        4,
        Partial,
        Action::Mediate,
        "instance path",
        "test-systemd"
    ),
    syscall!(
        "fstat",
        5,
        Partial,
        Action::Mediate,
        "instance descriptor",
        "test-systemd"
    ),
    syscall!(
        "poll",
        7,
        Partial,
        Action::Mediate,
        "bounded descriptor set",
        "test-systemd"
    ),
    syscall!(
        "lseek",
        8,
        Complete,
        Action::Mediate,
        "instance descriptor",
        "test-systemd"
    ),
    syscall!(
        "mmap",
        9,
        Partial,
        Action::Mediate,
        "W^X and bounded mapping",
        "test-runtime"
    ),
    syscall!(
        "mprotect",
        10,
        Partial,
        Action::Mediate,
        "W^X",
        "test-runtime"
    ),
    syscall!(
        "munmap",
        11,
        Complete,
        Action::Mediate,
        "managed mapping",
        "test-runtime"
    ),
    syscall!(
        "brk",
        12,
        Complete,
        Action::Mediate,
        "managed address space",
        "test-runtime"
    ),
    syscall!(
        "rt_sigaction",
        13,
        Partial,
        Action::Mediate,
        "virtual signal state",
        "test-process"
    ),
    syscall!(
        "rt_sigprocmask",
        14,
        Partial,
        Action::Mediate,
        "virtual signal state",
        "test-process"
    ),
    syscall!(
        "rt_sigreturn",
        15,
        Partial,
        Action::Mediate,
        "supervised frame",
        "test-process"
    ),
    syscall!(
        "ioctl",
        16,
        Partial,
        Action::Mediate,
        "cataloged request families only",
        "test-devices"
    ),
    syscall!(
        "pread64",
        17,
        Partial,
        Action::Mediate,
        "bounded instance descriptor",
        "test-native-vfs"
    ),
    syscall!(
        "pwrite64",
        18,
        Partial,
        Action::Mediate,
        "bounded instance descriptor",
        "test-systemd"
    ),
    syscall!(
        "writev",
        20,
        Partial,
        Action::Mediate,
        "bounded vectors",
        "test-systemd"
    ),
    syscall!(
        "access",
        21,
        Partial,
        Action::Mediate,
        "instance path",
        "test-native-vfs"
    ),
    direct_syscall!(
        "sched_yield",
        24,
        Complete,
        Action::Mediate,
        "pre-effect contract approval",
        "bench"
    ),
    syscall!(
        "mremap",
        25,
        Partial,
        Action::Mediate,
        "managed mapping",
        "test-systemd"
    ),
    syscall!(
        "madvise",
        28,
        Partial,
        Action::Mediate,
        "managed mapping",
        "test-runtime"
    ),
    syscall!(
        "dup",
        32,
        Complete,
        Action::Mediate,
        "instance descriptor",
        "test-systemd"
    ),
    syscall!(
        "dup2",
        33,
        Complete,
        Action::Mediate,
        "instance descriptor",
        "test-systemd"
    ),
    syscall!(
        "getpid",
        39,
        Complete,
        Action::Emulate,
        "virtual identity",
        "test-process"
    ),
    syscall!(
        "socket",
        41,
        Partial,
        Action::Mediate,
        "configured families and protocols",
        "test-gate975-network"
    ),
    syscall!(
        "connect",
        42,
        Partial,
        Action::Mediate,
        "egress policy",
        "test-gate975-network"
    ),
    syscall!(
        "accept",
        43,
        Partial,
        Action::Mediate,
        "ingress policy",
        "test-gate975-network"
    ),
    syscall!(
        "sendto",
        44,
        Partial,
        Action::Mediate,
        "egress policy and bounded payload",
        "test-gate975-network"
    ),
    syscall!(
        "recvfrom",
        45,
        Partial,
        Action::Mediate,
        "ingress policy and bounded payload",
        "test-gate975-network"
    ),
    syscall!(
        "sendmsg",
        46,
        Partial,
        Action::Mediate,
        "bounded message",
        "test-systemd"
    ),
    syscall!(
        "recvmsg",
        47,
        Partial,
        Action::Mediate,
        "bounded message",
        "test-systemd"
    ),
    syscall!(
        "shutdown",
        48,
        Partial,
        Action::Mediate,
        "socket owned by instance",
        "test-gate975-network"
    ),
    syscall!(
        "bind",
        49,
        Partial,
        Action::Mediate,
        "ingress policy and translation",
        "test-gate975-network"
    ),
    syscall!(
        "listen",
        50,
        Partial,
        Action::Mediate,
        "socket owned by instance",
        "test-gate975-network"
    ),
    syscall!(
        "getsockname",
        51,
        Partial,
        Action::Mediate,
        "virtual translated address",
        "test-gate975-network"
    ),
    syscall!(
        "getpeername",
        52,
        Partial,
        Action::Mediate,
        "socket owned by instance",
        "test-gate975-network"
    ),
    syscall!(
        "socketpair",
        53,
        Partial,
        Action::Mediate,
        "local instance only",
        "test-systemd"
    ),
    syscall!(
        "setsockopt",
        54,
        Partial,
        Action::Mediate,
        "cataloged options",
        "test-systemd"
    ),
    syscall!(
        "getsockopt",
        55,
        Partial,
        Action::Mediate,
        "cataloged options",
        "test-systemd"
    ),
    syscall!(
        "clone",
        56,
        Partial,
        Action::Mediate,
        "validated process/thread flags",
        "test-process"
    ),
    syscall!(
        "fork",
        57,
        Partial,
        Action::Mediate,
        "supervised child",
        "test-process"
    ),
    syscall!(
        "vfork",
        58,
        Partial,
        Action::Mediate,
        "supervised child",
        "test-process"
    ),
    syscall!(
        "execve",
        59,
        Partial,
        Action::Mediate,
        "instance path and environment",
        "test-systemd"
    ),
    syscall!(
        "exit",
        60,
        Complete,
        Action::Emulate,
        "virtual lifecycle",
        "test-process"
    ),
    syscall!(
        "wait4",
        61,
        Partial,
        Action::Mediate,
        "virtual child selector",
        "test-process"
    ),
    syscall!(
        "kill",
        62,
        Partial,
        Action::Mediate,
        "virtual task selector",
        "test-process"
    ),
    syscall!(
        "uname",
        63,
        Complete,
        Action::Emulate,
        "virtual kernel identity",
        "test-systemd-interactive"
    ),
    syscall!(
        "fcntl",
        72,
        Partial,
        Action::Mediate,
        "cataloged commands",
        "test-systemd"
    ),
    syscall!(
        "flock",
        73,
        Partial,
        Action::Mediate,
        "instance descriptor",
        "test-systemd"
    ),
    syscall!(
        "fsync",
        74,
        Complete,
        Action::Mediate,
        "instance descriptor",
        "test-systemd"
    ),
    syscall!(
        "fdatasync",
        75,
        Complete,
        Action::Mediate,
        "instance descriptor",
        "test-systemd"
    ),
    syscall!(
        "truncate",
        76,
        Partial,
        Action::Mediate,
        "instance path",
        "test-systemd"
    ),
    syscall!(
        "ftruncate",
        77,
        Partial,
        Action::Mediate,
        "instance descriptor",
        "test-systemd"
    ),
    syscall!(
        "getcwd",
        79,
        Complete,
        Action::Mediate,
        "instance path",
        "test-systemd"
    ),
    syscall!(
        "chdir",
        80,
        Partial,
        Action::Mediate,
        "instance path",
        "test-systemd"
    ),
    syscall!(
        "fchdir",
        81,
        Partial,
        Action::Mediate,
        "instance descriptor",
        "test-systemd"
    ),
    syscall!(
        "rename",
        82,
        Partial,
        Action::Mediate,
        "instance paths",
        "test-systemd"
    ),
    syscall!(
        "mkdir",
        83,
        Partial,
        Action::Mediate,
        "instance path",
        "test-systemd"
    ),
    syscall!(
        "rmdir",
        84,
        Partial,
        Action::Mediate,
        "instance path",
        "test-systemd"
    ),
    syscall!(
        "unlink",
        87,
        Partial,
        Action::Mediate,
        "instance path",
        "test-systemd"
    ),
    syscall!(
        "symlink",
        88,
        Partial,
        Action::Mediate,
        "instance paths",
        "test-systemd"
    ),
    syscall!(
        "readlink",
        89,
        Partial,
        Action::Mediate,
        "instance path",
        "test-native-vfs"
    ),
    syscall!(
        "chmod",
        90,
        Partial,
        Action::Mediate,
        "instance path and bounded mode",
        "test-systemd"
    ),
    syscall!(
        "fchmod",
        91,
        Partial,
        Action::Mediate,
        "instance descriptor and bounded mode",
        "test-systemd"
    ),
    syscall!(
        "chown",
        92,
        Partial,
        Action::Emulate,
        "virtual credentials",
        "test-systemd"
    ),
    syscall!(
        "fchown",
        93,
        Partial,
        Action::Emulate,
        "virtual credentials",
        "test-systemd"
    ),
    syscall!(
        "lchown",
        94,
        Partial,
        Action::Emulate,
        "virtual credentials",
        "test-systemd"
    ),
    syscall!(
        "umask",
        95,
        Complete,
        Action::Mediate,
        "instance process",
        "test-systemd"
    ),
    syscall!(
        "getrusage",
        98,
        Partial,
        Action::Mediate,
        "virtual task",
        "test-process"
    ),
    syscall!(
        "sysinfo",
        99,
        Partial,
        Action::Mediate,
        "virtual system view",
        "test-systemd"
    ),
    syscall!(
        "times",
        100,
        Partial,
        Action::Mediate,
        "virtual task",
        "test-process"
    ),
    syscall!(
        "getuid",
        102,
        Complete,
        Action::Emulate,
        "virtual credentials",
        "test-systemd"
    ),
    syscall!(
        "getgid",
        104,
        Complete,
        Action::Emulate,
        "virtual credentials",
        "test-systemd"
    ),
    syscall!(
        "setuid",
        105,
        Partial,
        Action::Emulate,
        "virtual credentials only",
        "test-systemd"
    ),
    syscall!(
        "setgid",
        106,
        Partial,
        Action::Emulate,
        "virtual credentials only",
        "test-systemd"
    ),
    syscall!(
        "geteuid",
        107,
        Complete,
        Action::Emulate,
        "virtual credentials",
        "test-systemd"
    ),
    syscall!(
        "getegid",
        108,
        Complete,
        Action::Emulate,
        "virtual credentials",
        "test-systemd"
    ),
    syscall!(
        "setpgid",
        109,
        Partial,
        Action::Emulate,
        "virtual process group",
        "test-systemd"
    ),
    syscall!(
        "getppid",
        110,
        Complete,
        Action::Emulate,
        "virtual identity",
        "test-process"
    ),
    syscall!(
        "getpgrp",
        111,
        Complete,
        Action::Emulate,
        "virtual process group",
        "test-systemd"
    ),
    syscall!(
        "setsid",
        112,
        Partial,
        Action::Emulate,
        "virtual session",
        "test-systemd"
    ),
    syscall!(
        "setreuid",
        113,
        Partial,
        Action::Emulate,
        "virtual credentials only",
        "test-systemd"
    ),
    syscall!(
        "setregid",
        114,
        Partial,
        Action::Emulate,
        "virtual credentials only",
        "test-systemd"
    ),
    syscall!(
        "setgroups",
        116,
        Partial,
        Action::Emulate,
        "virtual credentials only",
        "test-systemd"
    ),
    syscall!(
        "setresuid",
        117,
        Partial,
        Action::Emulate,
        "virtual credentials only",
        "test-systemd"
    ),
    syscall!(
        "setresgid",
        119,
        Partial,
        Action::Emulate,
        "virtual credentials only",
        "test-systemd"
    ),
    syscall!(
        "capget",
        125,
        Partial,
        Action::Mediate,
        "virtual capability view",
        "test-systemd"
    ),
    syscall!(
        "capset",
        126,
        Partial,
        Action::Mediate,
        "authority reduction only",
        "test-systemd"
    ),
    syscall!(
        "rt_sigtimedwait",
        128,
        Partial,
        Action::Mediate,
        "virtual signal state",
        "test-systemd"
    ),
    syscall!(
        "mknod",
        133,
        Partial,
        Action::Mediate,
        "FIFO only",
        "test-systemd"
    ),
    syscall!(
        "personality",
        135,
        Partial,
        Action::Mediate,
        "bounded personality",
        "test-systemd"
    ),
    syscall!(
        "statfs",
        137,
        Partial,
        Action::Mediate,
        "instance path",
        "test-systemd"
    ),
    syscall!(
        "fstatfs",
        138,
        Partial,
        Action::Mediate,
        "instance descriptor",
        "test-systemd"
    ),
    syscall!(
        "getpriority",
        140,
        Partial,
        Action::Emulate,
        "virtual task",
        "test-systemd"
    ),
    syscall!(
        "setpriority",
        141,
        Partial,
        Action::Emulate,
        "virtual task",
        "test-systemd"
    ),
    syscall!(
        "vhangup",
        153,
        Partial,
        Action::Emulate,
        "virtual console",
        "test-systemd"
    ),
    syscall!(
        "prctl",
        157,
        Partial,
        Action::Mediate,
        "cataloged commands",
        "test-systemd"
    ),
    syscall!(
        "arch_prctl",
        158,
        Partial,
        Action::Mediate,
        "TLS operations",
        "test-systemd"
    ),
    syscall!(
        "mount",
        165,
        Partial,
        Action::Emulate,
        "systemd virtual mount subset",
        "test-systemd"
    ),
    syscall!(
        "umount2",
        166,
        Partial,
        Action::Emulate,
        "systemd virtual unmount subset",
        "test-systemd"
    ),
    syscall!(
        "reboot",
        169,
        Partial,
        Action::Emulate,
        "instance shutdown only",
        "test-systemd"
    ),
    syscall!(
        "gettid",
        186,
        Complete,
        Action::Emulate,
        "virtual identity",
        "test-process"
    ),
    syscall!(
        "fsetxattr",
        190,
        Partial,
        Action::Mediate,
        "instance descriptor",
        "test-systemd"
    ),
    syscall!(
        "fgetxattr",
        193,
        Partial,
        Action::Mediate,
        "instance descriptor",
        "test-systemd"
    ),
    syscall!(
        "flistxattr",
        196,
        Partial,
        Action::Mediate,
        "instance descriptor",
        "test-systemd"
    ),
    syscall!(
        "fremovexattr",
        199,
        Partial,
        Action::Mediate,
        "instance descriptor",
        "test-systemd"
    ),
    syscall!(
        "futex",
        202,
        Partial,
        Action::Mediate,
        "non-PI operations",
        "test-process"
    ),
    syscall!(
        "epoll_create",
        213,
        Partial,
        Action::Mediate,
        "instance descriptors",
        "test-systemd"
    ),
    syscall!(
        "getdents64",
        217,
        Partial,
        Action::Mediate,
        "instance descriptor",
        "test-systemd"
    ),
    syscall!(
        "set_tid_address",
        218,
        Partial,
        Action::Mediate,
        "virtual tid result",
        "test-process"
    ),
    syscall!(
        "clock_gettime",
        228,
        Partial,
        Action::Mediate,
        "read-only time",
        "test-process"
    ),
    syscall!(
        "clock_nanosleep",
        230,
        Partial,
        Action::Mediate,
        "bounded wait",
        "test-process"
    ),
    syscall!(
        "exit_group",
        231,
        Complete,
        Action::Emulate,
        "virtual lifecycle",
        "test-process"
    ),
    syscall!(
        "epoll_wait",
        232,
        Partial,
        Action::Mediate,
        "instance descriptors",
        "test-systemd"
    ),
    syscall!(
        "epoll_ctl",
        233,
        Partial,
        Action::Mediate,
        "instance descriptors",
        "test-systemd"
    ),
    syscall!(
        "waitid",
        247,
        Partial,
        Action::Mediate,
        "virtual child selector",
        "test-process"
    ),
    syscall!(
        "inotify_init",
        253,
        Partial,
        Action::Mediate,
        "instance filesystem",
        "test-systemd"
    ),
    syscall!(
        "inotify_add_watch",
        254,
        Partial,
        Action::Mediate,
        "instance path",
        "test-systemd"
    ),
    syscall!(
        "inotify_rm_watch",
        255,
        Partial,
        Action::Mediate,
        "instance descriptor",
        "test-systemd"
    ),
    syscall!(
        "openat",
        257,
        Partial,
        Action::Mediate,
        "instance path and validated flags",
        "test-native-vfs"
    ),
    syscall!(
        "mkdirat",
        258,
        Partial,
        Action::Mediate,
        "instance path",
        "test-systemd"
    ),
    syscall!(
        "mknodat",
        259,
        Partial,
        Action::Mediate,
        "FIFO only",
        "test-systemd"
    ),
    syscall!(
        "fchownat",
        260,
        Partial,
        Action::Emulate,
        "virtual credentials",
        "test-systemd"
    ),
    syscall!(
        "newfstatat",
        262,
        Partial,
        Action::Mediate,
        "instance path",
        "test-systemd"
    ),
    syscall!(
        "unlinkat",
        263,
        Partial,
        Action::Mediate,
        "instance path",
        "test-systemd"
    ),
    syscall!(
        "renameat",
        264,
        Partial,
        Action::Mediate,
        "instance paths",
        "test-systemd"
    ),
    syscall!(
        "linkat",
        265,
        Partial,
        Action::Mediate,
        "instance paths",
        "test-systemd"
    ),
    syscall!(
        "symlinkat",
        266,
        Partial,
        Action::Mediate,
        "instance paths",
        "test-systemd"
    ),
    syscall!(
        "readlinkat",
        267,
        Partial,
        Action::Mediate,
        "instance path",
        "test-systemd"
    ),
    syscall!(
        "pselect6",
        270,
        Partial,
        Action::Mediate,
        "bounded descriptor set",
        "test-systemd"
    ),
    syscall!(
        "ppoll",
        271,
        Partial,
        Action::Mediate,
        "bounded descriptor set",
        "test-systemd"
    ),
    syscall!(
        "set_robust_list",
        273,
        Partial,
        Action::Mediate,
        "virtual task",
        "test-process"
    ),
    syscall!(
        "utimensat",
        280,
        Partial,
        Action::Mediate,
        "instance path",
        "test-systemd"
    ),
    syscall!(
        "epoll_pwait",
        281,
        Partial,
        Action::Mediate,
        "instance descriptors",
        "test-systemd"
    ),
    syscall!(
        "timerfd_create",
        283,
        Partial,
        Action::Mediate,
        "virtual timer",
        "test-process"
    ),
    syscall!(
        "fallocate",
        285,
        Partial,
        Action::Mediate,
        "instance descriptor",
        "test-systemd"
    ),
    syscall!(
        "timerfd_settime",
        286,
        Partial,
        Action::Mediate,
        "virtual timer",
        "test-process"
    ),
    syscall!(
        "timerfd_gettime",
        287,
        Partial,
        Action::Mediate,
        "virtual timer",
        "test-process"
    ),
    syscall!(
        "accept4",
        288,
        Partial,
        Action::Mediate,
        "ingress policy",
        "test-gate975-network"
    ),
    syscall!(
        "signalfd4",
        289,
        Partial,
        Action::Mediate,
        "virtual signal state",
        "test-systemd"
    ),
    syscall!(
        "eventfd2",
        290,
        Partial,
        Action::Mediate,
        "virtual event state",
        "test-process"
    ),
    syscall!(
        "epoll_create1",
        291,
        Partial,
        Action::Mediate,
        "instance descriptors",
        "test-systemd"
    ),
    syscall!(
        "dup3",
        292,
        Complete,
        Action::Mediate,
        "instance descriptor",
        "test-systemd"
    ),
    syscall!(
        "pipe2",
        293,
        Partial,
        Action::Mediate,
        "validated flags",
        "test-process"
    ),
    syscall!(
        "inotify_init1",
        294,
        Partial,
        Action::Mediate,
        "instance filesystem",
        "test-systemd"
    ),
    syscall!(
        "recvmmsg",
        299,
        Partial,
        Action::Mediate,
        "bounded messages",
        "test-gate975-network"
    ),
    syscall!(
        "prlimit64",
        302,
        Partial,
        Action::Mediate,
        "virtual resource limits",
        "test-systemd"
    ),
    syscall!(
        "syncfs",
        306,
        Complete,
        Action::Mediate,
        "instance descriptor",
        "test-systemd"
    ),
    syscall!(
        "sendmmsg",
        307,
        Partial,
        Action::Mediate,
        "bounded messages",
        "test-gate975-network"
    ),
    syscall!(
        "seccomp",
        317,
        Partial,
        Action::Mediate,
        "authority reduction only",
        "test-systemd"
    ),
    syscall!(
        "getrandom",
        318,
        Partial,
        Action::Mediate,
        "bounded bytes",
        "test-systemd"
    ),
    syscall!(
        "memfd_create",
        319,
        Partial,
        Action::Mediate,
        "instance memory",
        "test-systemd"
    ),
    syscall!(
        "statx",
        332,
        Partial,
        Action::Mediate,
        "instance path",
        "test-native-vfs"
    ),
    syscall!(
        "rseq",
        334,
        Partial,
        Action::Mediate,
        "virtual task",
        "test-process"
    ),
    syscall!(
        "pidfd_send_signal",
        424,
        Partial,
        Action::Mediate,
        "virtual task selector",
        "test-process"
    ),
    syscall!(
        "pidfd_open",
        434,
        Partial,
        Action::Mediate,
        "virtual task selector",
        "test-process"
    ),
    syscall!(
        "clone3",
        435,
        Partial,
        Action::Mediate,
        "validated process/thread flags",
        "test-process"
    ),
    syscall!(
        "close_range",
        436,
        Partial,
        Action::Mediate,
        "bounded instance descriptors",
        "test-systemd"
    ),
    syscall!(
        "faccessat2",
        439,
        Partial,
        Action::Mediate,
        "instance path",
        "test-native-vfs"
    ),
    syscall!(
        "epoll_pwait2",
        441,
        Partial,
        Action::Mediate,
        "instance descriptors",
        "test-systemd"
    ),
    syscall!(
        "fchmodat2",
        452,
        Partial,
        Action::Mediate,
        "instance path and bounded mode",
        "test-systemd"
    ),
    operation!(
        "ioctl.tty.get_winsize",
        Ioctl,
        Partial,
        Action::Emulate,
        "virtual console",
        "test-devices",
        "other tty ioctls denied"
    ),
    operation!(
        "network.inet.tcp.bind",
        Network,
        Partial,
        Action::Mediate,
        "ingress policy and translation",
        "test-gate975-network",
        "host-shared backend"
    ),
    operation!(
        "network.inet.tcp.connect",
        Network,
        Partial,
        Action::Mediate,
        "egress policy",
        "test-gate975-network",
        "host-shared backend"
    ),
    operation!(
        "network.inet.udp.bind",
        Network,
        Partial,
        Action::Mediate,
        "ingress policy and translation",
        "test-gate975-network",
        "host-shared backend"
    ),
    operation!(
        "network.inet.udp.connect",
        Network,
        Partial,
        Action::Mediate,
        "egress policy",
        "test-gate975-network",
        "host-shared backend"
    ),
    operation!(
        "device.console.read",
        Device,
        Partial,
        Action::Mediate,
        "assigned console",
        "test-devices",
        "bootstrap console"
    ),
    operation!(
        "device.console.write",
        Device,
        Partial,
        Action::Mediate,
        "assigned console",
        "test-devices",
        "bootstrap console"
    ),
    operation!(
        "filesystem.path.read",
        Filesystem,
        Partial,
        Action::Mediate,
        "instance root only",
        "test-native-vfs",
        "subset of Linux VFS"
    ),
    operation!(
        "filesystem.path.write",
        Filesystem,
        Partial,
        Action::Mediate,
        "instance overlay only",
        "test-systemd",
        "subset of Linux VFS"
    ),
    operation!(
        "proc.process.read",
        Proc,
        Partial,
        Action::Emulate,
        "virtual tasks only",
        "test-kernel-views",
        "synthetic subset"
    ),
    operation!(
        "proc.system.read",
        Proc,
        Partial,
        Action::Emulate,
        "virtual system values",
        "test-kernel-views",
        "synthetic subset"
    ),
    operation!(
        "sys.device.read",
        Sys,
        Partial,
        Action::Emulate,
        "assigned devices only",
        "test-kernel-views",
        "synthetic subset"
    ),
    operation!(
        "cgroup.virtual.manage",
        Cgroup,
        Partial,
        Action::Emulate,
        "instance hierarchy only",
        "test-kernel-views",
        "synthetic cgroup v2 subset"
    ),
    operation!(
        "credential.virtual.set",
        Credential,
        Partial,
        Action::Emulate,
        "no host privilege increase",
        "test-systemd",
        "virtual credentials"
    ),
    operation!(
        "signal.virtual.deliver",
        Signal,
        Partial,
        Action::Mediate,
        "virtual task selector",
        "test-process",
        "supported signal subset"
    ),
    operation!(
        "time.clock.read",
        Time,
        Partial,
        Action::Mediate,
        "read-only",
        "test-process",
        "host clock source"
    ),
    operation!(
        "memory.virtual.manage",
        Memory,
        Partial,
        Action::Mediate,
        "W^X",
        "test-runtime",
        "host VM backing"
    ),
    operation!(
        "device.mmio.direct",
        Device,
        Denied,
        Action::Deny(1),
        "never exposed to an instance",
        "contract-deny-negative",
        "direct hardware authority is prohibited"
    ),
    operation!(
        "network.packet.raw",
        Network,
        Planned,
        Action::Deny(38),
        "not admitted by catalog version 1",
        "catalog-planned-deny",
        "future design requires a mediated packet contract"
    ),
    operation!(
        "device.dma.direct",
        Device,
        OutOfScope,
        Action::Deny(1),
        "never exposed to an instance",
        "catalog-out-of-scope-deny",
        "userspace Ginux does not own host DMA programming"
    ),
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContractError {
    UnknownProfile,
    UnknownTemplate,
    InvalidSelector,
    UnmatchedSelector,
    TooManyOperations,
    DuplicateOperation,
    UnsupportedPromotion,
}

impl fmt::Display for ContractError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::UnknownProfile => "unknown-contract-profile",
            Self::UnknownTemplate => "unknown-contract-template",
            Self::InvalidSelector => "invalid-contract-selector",
            Self::UnmatchedSelector => "unmatched-contract-selector",
            Self::TooManyOperations => "contract-operation-capacity",
            Self::DuplicateOperation => "duplicate-catalog-operation",
            Self::UnsupportedPromotion => "unsupported-contract-promotion",
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Provenance {
    DefaultDeny,
    BaseProfile,
    Template,
    InstanceAllow,
    InstanceDeny,
    RecipeAllow,
    RecipeDeny,
    ExactAllow,
    ExactDeny,
}

impl Provenance {
    pub const fn name(self) -> &'static str {
        match self {
            Self::DefaultDeny => "default-deny",
            Self::BaseProfile => "base-profile",
            Self::Template => "template",
            Self::InstanceAllow => "instance-allow",
            Self::InstanceDeny => "instance-deny",
            Self::RecipeAllow => "recipe-allow",
            Self::RecipeDeny => "recipe-deny",
            Self::ExactAllow => "exact-allow",
            Self::ExactDeny => "exact-deny",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AccessVerb {
    Read,
    Write,
    Execute,
    Connect,
    Listen,
    Process,
    System,
}

impl AccessVerb {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::Write => "write",
            Self::Execute => "execute",
            Self::Connect => "connect",
            Self::Listen => "listen",
            Self::Process => "process",
            Self::System => "system",
        }
    }
}

#[derive(Clone, Copy)]
pub struct Recipe<'a> {
    pub read: &'a [&'a [u8]],
    pub write: &'a [&'a [u8]],
    pub execute: &'a [&'a [u8]],
    pub connect: &'a [&'a [u8]],
    pub listen: &'a [&'a [u8]],
    pub process: &'a [&'a [u8]],
    pub system: &'a [&'a [u8]],
    pub deny: &'a [&'a [u8]],
    pub exact_allow: &'a [&'a [u8]],
    pub exact_deny: &'a [&'a [u8]],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EffectiveRule {
    pub operation: &'static Operation,
    pub action: Action,
    pub provenance: Provenance,
    pub source: &'static str,
    pub source_rule: u16,
}

const EMPTY_OPERATION: Operation = Operation {
    id: "",
    kind: Kind::Syscall,
    abi_number: None,
    status: Status::Denied,
    action: Action::Deny(38),
    direct_safe: false,
    constraint: "",
    evidence: "",
    limitation: "",
};

const EMPTY_RULE: EffectiveRule = EffectiveRule {
    operation: &EMPTY_OPERATION,
    action: Action::Deny(38),
    provenance: Provenance::DefaultDeny,
    source: "default-deny",
    source_rule: 0,
};

const SYSCALL_RULE_SLOTS: usize = 512;
const NO_SYSCALL_RULE: u16 = u16::MAX;

#[derive(Clone, Copy)]
pub struct EffectiveContract {
    rules: [EffectiveRule; MAX_OPERATIONS],
    syscall_rules: [u16; SYSCALL_RULE_SLOTS],
    len: usize,
    digest: [u8; DIGEST_HEX_BYTES],
}

impl EffectiveContract {
    pub const fn empty() -> Self {
        Self {
            rules: [EMPTY_RULE; MAX_OPERATIONS],
            syscall_rules: [NO_SYSCALL_RULE; SYSCALL_RULE_SLOTS],
            len: 0,
            digest: [0; DIGEST_HEX_BYTES],
        }
    }

    pub fn rules(&self) -> &[EffectiveRule] {
        &self.rules[..self.len]
    }

    pub const fn digest(&self) -> &[u8; DIGEST_HEX_BYTES] {
        &self.digest
    }

    pub fn syscall_action(&self, number: u64) -> Action {
        self.syscall_rule(number)
            .map(|rule| rule.action)
            .unwrap_or(Action::Deny(38))
    }

    pub fn syscall_rule(&self, number: u64) -> Option<&EffectiveRule> {
        if let Ok(number) = usize::try_from(number) {
            if let Some(index) = self.syscall_rules.get(number).copied() {
                if index != NO_SYSCALL_RULE {
                    return self.rules.get(usize::from(index));
                }
            }
        }
        self.rules()
            .iter()
            .find(|rule| rule.operation.abi_number == Some(number))
    }

    pub fn operation_rule(&self, id: &str) -> Option<&EffectiveRule> {
        self.rules().iter().find(|rule| rule.operation.id == id)
    }

    fn index_syscalls(&mut self) {
        for index in 0..self.len {
            let Some(number) = self.rules[index].operation.abi_number else {
                continue;
            };
            let Ok(number) = usize::try_from(number) else {
                continue;
            };
            if let Some(slot) = self.syscall_rules.get_mut(number) {
                *slot = index as u16;
            }
        }
    }
}

pub fn validate_catalog() -> Result<(), ContractError> {
    if CATALOG.len() > MAX_OPERATIONS {
        return Err(ContractError::TooManyOperations);
    }
    for (index, operation) in CATALOG.iter().enumerate() {
        if CATALOG[..index].iter().any(|existing| {
            existing.id == operation.id
                || (operation.abi_number.is_some() && existing.abi_number == operation.abi_number)
        }) {
            return Err(ContractError::DuplicateOperation);
        }
    }
    Ok(())
}

pub fn resolve(
    profile: &[u8],
    templates: &[&[u8]],
    allow: &[&[u8]],
    deny: &[&[u8]],
) -> Result<EffectiveContract, ContractError> {
    validate_catalog()?;
    let mut effective = EffectiveContract::empty();
    for operation in CATALOG {
        effective.rules[effective.len] = EffectiveRule {
            operation,
            action: Action::Deny(38),
            provenance: Provenance::DefaultDeny,
            source: "default-deny",
            source_rule: 0,
        };
        effective.len += 1;
    }
    effective.index_syscalls();
    apply_profile(&mut effective, profile)?;
    for template in templates {
        apply_template(&mut effective, template)?;
    }
    for selector in allow {
        apply_selector(
            &mut effective,
            selector,
            None,
            Provenance::InstanceAllow,
            "instance-allow",
        )?;
    }
    for selector in deny {
        apply_selector(
            &mut effective,
            selector,
            Some(Action::Deny(1)),
            Provenance::InstanceDeny,
            "instance-deny",
        )?;
    }
    effective.digest = contract_digest(&effective);
    Ok(effective)
}

pub fn resolve_recipe(recipe: Recipe<'_>) -> Result<EffectiveContract, ContractError> {
    validate_catalog()?;
    let mut effective = EffectiveContract::empty();
    for operation in CATALOG {
        effective.rules[effective.len] = EffectiveRule {
            operation,
            action: Action::Deny(38),
            provenance: Provenance::DefaultDeny,
            source: "default-deny",
            source_rule: 0,
        };
        effective.len += 1;
    }
    effective.index_syscalls();
    for (verb, selectors) in [
        (AccessVerb::Read, recipe.read),
        (AccessVerb::Write, recipe.write),
        (AccessVerb::Execute, recipe.execute),
        (AccessVerb::Connect, recipe.connect),
        (AccessVerb::Listen, recipe.listen),
        (AccessVerb::Process, recipe.process),
        (AccessVerb::System, recipe.system),
    ] {
        for (index, selector) in selectors.iter().enumerate() {
            apply_recipe_selector(&mut effective, verb, selector, index, false)?;
        }
    }
    for (index, selector) in recipe.exact_allow.iter().enumerate() {
        apply_exact_selector(&mut effective, selector, index, false)?;
    }
    for (index, selector) in recipe.deny.iter().enumerate() {
        apply_recipe_deny(&mut effective, selector, index, Provenance::RecipeDeny)?;
    }
    for (index, selector) in recipe.exact_deny.iter().enumerate() {
        apply_exact_selector(&mut effective, selector, index, true)?;
    }
    effective.digest = contract_digest(&effective);
    Ok(effective)
}

fn apply_recipe_selector(
    effective: &mut EffectiveContract,
    verb: AccessVerb,
    selector: &[u8],
    index: usize,
    deny: bool,
) -> Result<(), ContractError> {
    validate_resource_selector(selector)?;
    let mut matched = 0usize;
    let mut unsupported = 0usize;
    for rule in &mut effective.rules[..effective.len] {
        if resource_selector_matches(selector, rule.operation.id.as_bytes())
            && operation_access(rule.operation) == verb
        {
            if !deny && !matches!(rule.operation.status, Status::Complete | Status::Partial) {
                unsupported += 1;
                continue;
            }
            matched += 1;
            rule.action = if deny {
                Action::Deny(1)
            } else {
                rule.operation.action
            };
            rule.provenance = if deny {
                Provenance::RecipeDeny
            } else {
                Provenance::RecipeAllow
            };
            rule.source = verb.name();
            rule.source_rule = index as u16;
        }
    }
    if matched == 0 {
        return Err(if unsupported != 0 {
            ContractError::UnsupportedPromotion
        } else {
            ContractError::UnmatchedSelector
        });
    }
    Ok(())
}

fn apply_recipe_deny(
    effective: &mut EffectiveContract,
    selector: &[u8],
    index: usize,
    provenance: Provenance,
) -> Result<(), ContractError> {
    validate_resource_selector(selector)?;
    let mut matched = 0usize;
    for rule in &mut effective.rules[..effective.len] {
        if resource_selector_matches(selector, rule.operation.id.as_bytes()) {
            matched += 1;
            rule.action = Action::Deny(1);
            rule.provenance = provenance;
            rule.source = "deny";
            rule.source_rule = index as u16;
        }
    }
    if matched == 0 {
        return Err(ContractError::UnmatchedSelector);
    }
    Ok(())
}

fn apply_exact_selector(
    effective: &mut EffectiveContract,
    selector: &[u8],
    index: usize,
    deny: bool,
) -> Result<(), ContractError> {
    validate_selector(selector)?;
    if selector.contains(&b'*') {
        return Err(ContractError::InvalidSelector);
    }
    let Some(rule) = effective.rules[..effective.len]
        .iter_mut()
        .find(|rule| rule.operation.id.as_bytes() == selector)
    else {
        return Err(ContractError::UnmatchedSelector);
    };
    if !deny && !matches!(rule.operation.status, Status::Complete | Status::Partial) {
        return Err(ContractError::UnsupportedPromotion);
    }
    rule.action = if deny {
        Action::Deny(1)
    } else {
        rule.operation.action
    };
    rule.provenance = if deny {
        Provenance::ExactDeny
    } else {
        Provenance::ExactAllow
    };
    rule.source = if deny { "exact-deny" } else { "exact-allow" };
    rule.source_rule = index as u16;
    Ok(())
}

pub fn validate_resource_selector(selector: &[u8]) -> Result<(), ContractError> {
    if selector.is_empty()
        || selector[0] == b'.'
        || selector[selector.len() - 1] == b'.'
        || selector.windows(2).any(|pair| pair == b"..")
        || selector.iter().any(|byte| {
            !byte.is_ascii_lowercase()
                && !byte.is_ascii_digit()
                && !matches!(*byte, b'.' | b'_' | b'-' | b'*')
        })
    {
        return Err(ContractError::InvalidSelector);
    }
    for component in selector.split(|byte| *byte == b'.') {
        if component.contains(&b'*') && component != b"*" && component != b"**" {
            return Err(ContractError::InvalidSelector);
        }
    }
    Ok(())
}

pub fn resource_selector_matches(selector: &[u8], operation: &[u8]) -> bool {
    resource_components_match(selector, operation, true)
}

fn resource_components_match(pattern: &[u8], value: &[u8], first: bool) -> bool {
    let (pattern_head, pattern_tail) = split_component(pattern);
    let (value_head, value_tail) = split_component(value);
    if pattern_head == b"**" {
        if pattern_tail.is_empty() {
            return true;
        }
        return resource_components_match(pattern_tail, value, false)
            || (!value_tail.is_empty() && resource_components_match(pattern, value_tail, false));
    }
    if value_head.is_empty() {
        return false;
    }
    let head_matches = pattern_head == b"*"
        || pattern_head == value_head
        || (first && pattern_head == b"fs" && value_head == b"filesystem")
        || (first && pattern_head == b"dev" && value_head == b"device");
    head_matches
        && if pattern_tail.is_empty() {
            value_tail.is_empty()
        } else {
            !value_tail.is_empty() && resource_components_match(pattern_tail, value_tail, false)
        }
}

fn split_component(value: &[u8]) -> (&[u8], &[u8]) {
    match value.iter().position(|byte| *byte == b'.') {
        Some(index) => (&value[..index], &value[index + 1..]),
        None => (value, &[]),
    }
}

pub fn operation_access(operation: &Operation) -> AccessVerb {
    let id = operation.id.as_bytes();
    if operation.kind == Kind::Proc
        || operation.kind == Kind::Sys
        || id.ends_with(b".read")
        || id.starts_with(b"syscall.read")
        || id.starts_with(b"syscall.pread")
        || id.starts_with(b"syscall.stat")
        || id.starts_with(b"syscall.fstat")
        || id.starts_with(b"syscall.newfstat")
        || id.starts_with(b"syscall.getdents")
        || id.starts_with(b"syscall.access")
        || id.starts_with(b"syscall.faccess")
    {
        AccessVerb::Read
    } else if id.ends_with(b".write")
        || id.starts_with(b"syscall.write")
        || id.starts_with(b"syscall.pwrite")
        || id.starts_with(b"syscall.truncate")
        || id.starts_with(b"syscall.ftruncate")
        || id.starts_with(b"syscall.mkdir")
        || id.starts_with(b"syscall.unlink")
        || id.starts_with(b"syscall.rename")
        || id.starts_with(b"syscall.link")
        || id.starts_with(b"syscall.symlink")
        || id.starts_with(b"syscall.chmod")
        || id.starts_with(b"syscall.fchmod")
        || id.starts_with(b"syscall.fallocate")
        || id.starts_with(b"syscall.fsync")
        || id.starts_with(b"syscall.fdatasync")
        || id.starts_with(b"syscall.syncfs")
    {
        AccessVerb::Write
    } else if id == b"syscall.execve" {
        AccessVerb::Execute
    } else if id.ends_with(b".connect")
        || id.starts_with(b"syscall.connect")
        || id.starts_with(b"syscall.send")
        || id.starts_with(b"syscall.recv")
        || id.starts_with(b"syscall.getsock")
        || id.starts_with(b"syscall.getpeer")
        || id.starts_with(b"syscall.setsock")
        || id.starts_with(b"syscall.shutdown")
    {
        AccessVerb::Connect
    } else if id.ends_with(b".bind")
        || id == b"syscall.bind"
        || id == b"syscall.listen"
        || id.starts_with(b"syscall.accept")
    {
        AccessVerb::Listen
    } else if operation.kind == Kind::Signal
        || operation.kind == Kind::Credential
        || id.starts_with(b"syscall.clone")
        || id == b"syscall.fork"
        || id == b"syscall.vfork"
        || id.starts_with(b"syscall.wait")
        || id.starts_with(b"syscall.kill")
        || id.starts_with(b"syscall.pidfd")
        || id.starts_with(b"syscall.rt_sig")
    {
        AccessVerb::Process
    } else {
        AccessVerb::System
    }
}

fn apply_profile(effective: &mut EffectiveContract, profile: &[u8]) -> Result<(), ContractError> {
    match profile {
        b"minimal-v1" | b"strict-v1" => {
            for selector in [
                b"syscall.getpid".as_slice(),
                b"syscall.getppid".as_slice(),
                b"syscall.gettid".as_slice(),
                b"syscall.write".as_slice(),
                b"syscall.exit".as_slice(),
                b"syscall.exit_group".as_slice(),
            ] {
                apply_selector(
                    effective,
                    selector,
                    None,
                    Provenance::BaseProfile,
                    if profile == b"strict-v1" {
                        "strict-v1"
                    } else {
                        "minimal-v1"
                    },
                )?;
            }
        }
        b"gate5-balanced-v1" | b"gate9-systemd-v1" => {
            let source = if profile == b"gate5-balanced-v1" {
                "gate5-balanced-v1"
            } else {
                "gate9-systemd-v1"
            };
            for rule in &mut effective.rules[..effective.len] {
                if rule.operation.status != Status::Denied {
                    rule.action = rule.operation.action;
                    rule.provenance = Provenance::BaseProfile;
                    rule.source = source;
                }
            }
        }
        _ => return Err(ContractError::UnknownProfile),
    }
    Ok(())
}

fn apply_template(effective: &mut EffectiveContract, template: &[u8]) -> Result<(), ContractError> {
    let selectors: &[&[u8]] = match template {
        b"network-client-v1" => &[
            b"syscall.socket",
            b"syscall.connect",
            b"syscall.send*",
            b"syscall.recv*",
            b"syscall.getsock*",
            b"syscall.getpeername",
            b"syscall.setsockopt",
            b"network.inet.*.connect",
        ],
        b"read-only-time-v1" => &[b"syscall.clock_*", b"time.clock.read"],
        b"virtual-kernel-views-v1" => &[b"proc.*", b"sys.*", b"cgroup.*"],
        _ => return Err(ContractError::UnknownTemplate),
    };
    let source = match template {
        b"network-client-v1" => "network-client-v1",
        b"read-only-time-v1" => "read-only-time-v1",
        b"virtual-kernel-views-v1" => "virtual-kernel-views-v1",
        _ => return Err(ContractError::UnknownTemplate),
    };
    for selector in selectors {
        apply_selector(effective, selector, None, Provenance::Template, source)?;
    }
    Ok(())
}

fn apply_selector(
    effective: &mut EffectiveContract,
    selector: &[u8],
    forced: Option<Action>,
    provenance: Provenance,
    source: &'static str,
) -> Result<(), ContractError> {
    validate_selector(selector)?;
    let exact = !selector.contains(&b'*');
    let mut matches = 0;
    for rule in &mut effective.rules[..effective.len] {
        if selector_matches(selector, rule.operation.id.as_bytes()) {
            matches += 1;
            rule.action = forced.unwrap_or(rule.operation.action);
            rule.provenance = provenance;
            rule.source = source;
        }
    }
    if matches == 0 {
        return Err(ContractError::UnmatchedSelector);
    }
    if exact && matches != 1 {
        return Err(ContractError::InvalidSelector);
    }
    Ok(())
}

pub fn validate_selector(selector: &[u8]) -> Result<(), ContractError> {
    if selector.is_empty()
        || selector[0] == b'.'
        || selector[selector.len() - 1] == b'.'
        || selector.windows(2).any(|pair| pair == b"..")
        || selector.iter().any(|byte| {
            !byte.is_ascii_lowercase()
                && !byte.is_ascii_digit()
                && !matches!(*byte, b'.' | b'_' | b'-' | b'*')
        })
    {
        return Err(ContractError::InvalidSelector);
    }
    let stars = selector.iter().filter(|byte| **byte == b'*').count();
    if stars > 1 {
        return Err(ContractError::InvalidSelector);
    }
    Ok(())
}

pub fn selector_matches(selector: &[u8], operation: &[u8]) -> bool {
    let Some(star) = selector.iter().position(|byte| *byte == b'*') else {
        return selector == operation;
    };
    let prefix = &selector[..star];
    let suffix = &selector[star + 1..];
    operation.len() >= prefix.len() + suffix.len()
        && operation.starts_with(prefix)
        && operation.ends_with(suffix)
}

fn contract_digest(contract: &EffectiveContract) -> [u8; DIGEST_HEX_BYTES] {
    // FNV-1a is used only as a stable identity, not as a trust primitive. The
    // catalog and normalized configuration remain the authoritative inputs.
    let mut hash = 0xcbf29ce484222325u64;
    hash = hash_bytes(hash, &CATALOG_VERSION.to_le_bytes());
    for rule in contract.rules() {
        hash = hash_bytes(hash, rule.operation.id.as_bytes());
        hash = hash_bytes(hash, &[rule.operation.direct_safe as u8]);
        hash = hash_bytes(hash, rule.action.name().as_bytes());
        hash = hash_bytes(hash, rule.provenance.name().as_bytes());
        hash = hash_bytes(hash, rule.source.as_bytes());
        hash = hash_bytes(hash, &rule.source_rule.to_le_bytes());
    }
    let mut digest = [b'0'; DIGEST_HEX_BYTES];
    // Four independently salted FNV lanes produce a fixed 256-bit identity.
    for lane in 0usize..4 {
        let value = hash_bytes(
            hash ^ (lane as u64).wrapping_mul(0x9e3779b97f4a7c15),
            &[lane as u8],
        );
        for nibble in 0..16 {
            let shift = (15 - nibble) * 4;
            digest[lane * 16 + nibble] = b"0123456789abcdef"[((value >> shift) & 0xf) as usize];
        }
    }
    digest
}

const fn hash_bytes(mut hash: u64, bytes: &[u8]) -> u64 {
    let mut index = 0;
    while index < bytes.len() {
        hash ^= bytes[index] as u64;
        hash = hash.wrapping_mul(0x100000001b3);
        index += 1;
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_is_unique_and_bounded() {
        validate_catalog().unwrap();
        assert!(CATALOG.len() <= MAX_OPERATIONS);
        assert!(CATALOG.iter().any(|entry| entry.id == "syscall.ioctl"));
        assert!(CATALOG.iter().any(|entry| entry.id == "proc.process.read"));
    }

    #[test]
    fn minimal_is_deny_by_default() {
        let contract = resolve(b"minimal-v1", &[], &[], &[]).unwrap();
        assert_eq!(contract.syscall_action(39), Action::Emulate);
        assert_eq!(contract.syscall_action(41), Action::Deny(38));
    }

    #[test]
    fn strict_profile_keeps_the_small_bootstrap_surface() {
        let contract = resolve(b"strict-v1", &[], &[], &[]).unwrap();
        assert_eq!(contract.syscall_action(39), Action::Emulate);
        assert_eq!(contract.syscall_action(228), Action::Deny(38));
        assert_eq!(contract.syscall_action(41), Action::Deny(38));
    }

    #[test]
    fn systemd_profile_mediates_sched_yield_through_the_index() {
        let contract = resolve(b"gate9-systemd-v1", &[], &[], &[]).unwrap();
        let rule = contract.syscall_rule(24).unwrap();
        assert_eq!(rule.operation.id, "syscall.sched_yield");
        assert_eq!(rule.action, Action::Mediate);
        assert!(rule.operation.direct_safe);
        assert_eq!(contract.syscall_action(24), Action::Mediate);
    }

    #[test]
    fn templates_and_overrides_are_deterministic() {
        let first = resolve(
            b"minimal-v1",
            &[b"network-client-v1"],
            &[b"syscall.clock_*"],
            &[b"syscall.connect"],
        )
        .unwrap();
        let second = resolve(
            b"minimal-v1",
            &[b"network-client-v1"],
            &[b"syscall.clock_*"],
            &[b"syscall.connect"],
        )
        .unwrap();
        assert_eq!(first.digest(), second.digest());
        assert_eq!(first.syscall_action(42), Action::Deny(1));
        assert_eq!(first.syscall_action(228), Action::Mediate);
    }

    #[test]
    fn semantic_deny_does_not_rewrite_unrelated_syscall_authority() {
        let contract = resolve(
            b"gate9-systemd-v1",
            &[],
            &[],
            &[b"network.inet.tcp.connect"],
        )
        .unwrap();
        assert_eq!(contract.syscall_action(42), Action::Mediate);
        assert_eq!(
            contract
                .operation_rule("network.inet.tcp.connect")
                .unwrap()
                .action,
            Action::Deny(1)
        );
        assert_eq!(
            contract
                .operation_rule("network.inet.udp.connect")
                .unwrap()
                .action,
            Action::Mediate
        );
    }

    #[test]
    fn malformed_and_unmatched_selectors_fail_closed() {
        assert_eq!(
            resolve(b"minimal-v1", &[], &[b"syscall:nope"], &[])
                .err()
                .unwrap(),
            ContractError::InvalidSelector
        );
        assert_eq!(
            resolve(b"minimal-v1", &[], &[b"syscall.nope*"], &[])
                .err()
                .unwrap(),
            ContractError::UnmatchedSelector
        );
        assert!(resolve(b"unversioned", &[], &[], &[]).is_err());
    }

    #[test]
    fn old_selector_does_not_expand_without_reresolution() {
        let contract = resolve(b"minimal-v1", &[], &[b"syscall.clock_*"], &[]).unwrap();
        let selected = contract
            .rules()
            .iter()
            .filter(|rule| rule.provenance == Provenance::InstanceAllow)
            .count();
        assert_eq!(selected, 2);
        assert_eq!(CATALOG_VERSION, 2);
    }

    #[test]
    fn systemd_dispatch_and_catalog_are_consistent() {
        for number in 0..=512 {
            let implementation = ginux_core::decide_systemd(number, true).action;
            let catalog = CATALOG
                .iter()
                .find(|operation| operation.abi_number == Some(number));
            match (implementation, catalog) {
                (ginux_core::Action::Deny(_), None) => {}
                (ginux_core::Action::Emulate, Some(operation)) => {
                    assert_eq!(operation.action, Action::Emulate, "syscall {number}");
                }
                (ginux_core::Action::Mediate, Some(operation)) => {
                    assert_eq!(operation.action, Action::Mediate, "syscall {number}");
                }
                (ginux_core::Action::Deny(errno), Some(operation)) => {
                    assert_eq!(operation.action, Action::Deny(errno), "syscall {number}");
                }
                (_, None) => panic!("implemented syscall {number} is missing from the catalog"),
            }
        }
    }

    #[test]
    fn resource_globs_are_component_bounded_and_support_aliases() {
        assert!(resource_selector_matches(b"fs.**", b"filesystem.path.read"));
        assert!(resource_selector_matches(
            b"network.*.tcp.connect",
            b"network.inet.tcp.connect"
        ));
        assert!(!resource_selector_matches(
            b"network.*.connect",
            b"network.inet.tcp.connect"
        ));
        assert_eq!(
            validate_resource_selector(b"network.in*.connect"),
            Err(ContractError::InvalidSelector)
        );
    }

    #[test]
    fn self_contained_recipe_matches_the_systemd_authority_surface() {
        let all = &[b"**".as_slice()];
        let deny = &[
            b"dev.mmio.direct".as_slice(),
            b"dev.dma.direct".as_slice(),
            b"network.packet.raw".as_slice(),
        ];
        let recipe = resolve_recipe(Recipe {
            read: all,
            write: all,
            execute: all,
            connect: all,
            listen: all,
            process: all,
            system: all,
            deny,
            exact_allow: &[],
            exact_deny: &[],
        })
        .unwrap();
        let legacy = resolve(b"gate9-systemd-v1", &[], &[], &[]).unwrap();
        for rule in recipe.rules() {
            let legacy_action = legacy.operation_rule(rule.operation.id).unwrap().action;
            assert_eq!(
                matches!(rule.action, Action::Deny(_)),
                matches!(legacy_action, Action::Deny(_)),
                "{}",
                rule.operation.id
            );
            if !matches!(rule.action, Action::Deny(_)) {
                assert_eq!(rule.action, legacy_action, "{}", rule.operation.id);
            }
        }
    }

    #[test]
    fn recipe_denies_win_and_unsupported_authority_cannot_be_promoted() {
        let recipe = resolve_recipe(Recipe {
            read: &[b"fs.**"],
            write: &[b"fs.**"],
            execute: &[],
            connect: &[],
            listen: &[],
            process: &[],
            system: &[],
            deny: &[b"fs.path.write"],
            exact_allow: &[],
            exact_deny: &[],
        })
        .unwrap();
        let denied = recipe.operation_rule("filesystem.path.write").unwrap();
        assert_eq!(denied.action, Action::Deny(1));
        assert_eq!(denied.provenance, Provenance::RecipeDeny);
        assert_eq!(denied.source_rule, 0);
        assert_eq!(
            resolve_recipe(Recipe {
                read: &[],
                write: &[],
                execute: &[],
                connect: &[],
                listen: &[],
                process: &[],
                system: &[],
                deny: &[],
                exact_allow: &[b"device.mmio.direct"],
                exact_deny: &[],
            })
            .err(),
            Some(ContractError::UnsupportedPromotion)
        );
    }

    #[test]
    fn audit_budget_is_bounded_and_reports_drops_logarithmically() {
        let mut budget = AuditBudget::new(2);
        assert_eq!(budget.admit(), AuditDisposition::Emit);
        assert_eq!(budget.admit(), AuditDisposition::Emit);
        assert_eq!(budget.admit(), AuditDisposition::DropAndReport(1));
        assert_eq!(budget.admit(), AuditDisposition::DropAndReport(2));
        assert_eq!(budget.admit(), AuditDisposition::Drop);
        assert_eq!(budget.admit(), AuditDisposition::DropAndReport(4));
        assert_eq!(budget.emitted(), 2);
        assert_eq!(budget.dropped(), 4);
    }
}
