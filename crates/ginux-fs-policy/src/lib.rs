#![no_std]

use ginux_contract::Action;

pub const X86_64_AUDIT_ARCH: u32 = 0xc000_003e;
pub const ENOSYS: i32 = 38;
pub const ABI_REFERENCE: &str =
    "Linux v6.12 arch/x86/entry/syscalls/syscall_64.tbl plus Linux man-pages semantics";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContractAuthorization {
    Emulate,
    Mediate,
    Deny(i32),
}

impl From<Action> for ContractAuthorization {
    fn from(action: Action) -> Self {
        match action {
            Action::Emulate => Self::Emulate,
            Action::Mediate => Self::Mediate,
            Action::Deny(errno) => Self::Deny(errno),
        }
    }
}

impl ContractAuthorization {
    pub const fn permits(self) -> bool {
        matches!(self, Self::Emulate | Self::Mediate)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ObjectId {
    pub device: u64,
    pub inode: u64,
}

impl ObjectId {
    pub const UNAVAILABLE: Self = Self {
        device: 0,
        inode: 0,
    };

    pub const fn is_available(self) -> bool {
        self.device != 0 || self.inode != 0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CapabilityKind {
    Root,
    WorkingDirectory,
    DescriptorTable,
    DirectoryDescriptor,
    Descriptor,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Rights(u16);

impl Rights {
    pub const LOOKUP: Self = Self(1 << 0);
    pub const READ: Self = Self(1 << 1);
    pub const WRITE: Self = Self(1 << 2);
    pub const CREATE: Self = Self(1 << 3);
    pub const METADATA: Self = Self(1 << 4);
    pub const EXECUTE: Self = Self(1 << 5);

    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    pub const fn contains(self, required: Self) -> bool {
        self.0 & required.0 == required.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Capability {
    pub kind: CapabilityKind,
    pub instance: u64,
    pub lifetime: u64,
    pub generation: u64,
    pub object: ObjectId,
    pub rights: Rights,
    available: bool,
}

impl Capability {
    pub const fn new(
        kind: CapabilityKind,
        instance: u64,
        lifetime: u64,
        generation: u64,
        object: ObjectId,
        rights: Rights,
    ) -> Self {
        Self {
            kind,
            instance,
            lifetime,
            generation,
            object,
            rights,
            available: object.is_available(),
        }
    }

    pub const fn unavailable(kind: CapabilityKind, instance: u64, lifetime: u64) -> Self {
        Self {
            kind,
            instance,
            lifetime,
            generation: 0,
            object: ObjectId::UNAVAILABLE,
            rights: Rights(0),
            available: false,
        }
    }

    pub const fn is_available(self) -> bool {
        self.available
    }

    pub const fn authorizes(
        self,
        instance: u64,
        lifetime: u64,
        generation: u64,
        required: Rights,
    ) -> bool {
        self.available
            && self.instance == instance
            && self.lifetime == lifetime
            && self.generation == generation
            && self.rights.contains(required)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TaskContext {
    pub host: u64,
    pub virtual_id: u64,
    pub generation: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InstanceContext {
    pub identity: u64,
    pub lifetime: u64,
    pub root_generation: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthorityContext {
    pub root: Capability,
    pub cwd: Capability,
    pub descriptor_table: Capability,
    pub cwd_generation: u64,
    pub descriptor_generation: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CallContext {
    pub abi: u32,
    pub syscall: u64,
    pub arguments: [u64; 6],
    pub task: TaskContext,
    pub instance: InstanceContext,
    pub authority: AuthorityContext,
    pub contract: ContractAuthorization,
}

impl CallContext {
    pub const fn x86_64(
        syscall: u64,
        arguments: [u64; 6],
        task: TaskContext,
        instance: InstanceContext,
        authority: AuthorityContext,
        contract: ContractAuthorization,
    ) -> Self {
        Self {
            abi: X86_64_AUDIT_ARCH,
            syscall,
            arguments,
            task,
            instance,
            authority,
            contract,
        }
    }

    pub fn complete_for_path(self) -> bool {
        self.abi == X86_64_AUDIT_ARCH
            && self.task.host != 0
            && self.task.virtual_id != 0
            && self.task.generation != 0
            && self.instance.identity != 0
            && self.instance.lifetime != 0
            && self.authority.root.kind == CapabilityKind::Root
            && self.authority.root.authorizes(
                self.instance.identity,
                self.instance.lifetime,
                self.instance.root_generation,
                Rights::LOOKUP,
            )
            && self.authority.cwd.kind == CapabilityKind::WorkingDirectory
            && self.authority.cwd.authorizes(
                self.instance.identity,
                self.instance.lifetime,
                self.authority.cwd_generation,
                Rights::LOOKUP,
            )
            && self.authority.descriptor_table.kind == CapabilityKind::DescriptorTable
            && self.authority.descriptor_table.authorizes(
                self.instance.identity,
                self.instance.lifetime,
                self.authority.descriptor_generation,
                Rights::LOOKUP,
            )
            && self.contract.permits()
    }
}

pub const fn identity_token(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    let mut index = 0;
    while index < bytes.len() {
        hash ^= bytes[index] as u64;
        hash = hash.wrapping_mul(0x100_0000_01b3);
        index += 1;
    }
    if hash == 0 {
        1
    } else {
        hash
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Owner {
    FilesystemLane,
    DescriptorCore,
    ProcessLane,
    VirtualKernel,
    ExplicitDeny,
}

impl Owner {
    pub const fn name(self) -> &'static str {
        match self {
            Self::FilesystemLane => "filesystem-lane",
            Self::DescriptorCore => "descriptor-core",
            Self::ProcessLane => "process-lane",
            Self::VirtualKernel => "virtual-kernel",
            Self::ExplicitDeny => "explicit-deny",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Operation {
    pub syscall: u64,
    pub name: &'static str,
    pub owner: Owner,
    pub arguments: &'static str,
    pub flags: &'static str,
    pub authority: &'static str,
    pub executor: &'static str,
    pub unsupported_errno: i32,
}

macro_rules! op {
    ($number:literal, $name:literal, $owner:ident, $arguments:literal, $flags:literal, $authority:literal, $executor:literal) => {
        Operation {
            syscall: $number,
            name: $name,
            owner: Owner::$owner,
            arguments: $arguments,
            flags: $flags,
            authority: $authority,
            executor: $executor,
            unsupported_errno: ENOSYS,
        }
    };
}

// Pinned Linux x86-64 filesystem-capable operation inventory. An operation may
// remain on a reviewed legacy route, but it cannot be absent or gain a host
// continuation merely because the filesystem lane declines it.
pub static OPERATIONS: &[Operation] = &[
    op!(
        0,
        "read",
        DescriptorCore,
        "fd,buf,count",
        "none",
        "descriptor:read",
        "legacy descriptor mediation"
    ),
    op!(
        1,
        "write",
        DescriptorCore,
        "fd,buf,count",
        "none",
        "descriptor:write",
        "legacy descriptor mediation"
    ),
    op!(
        2,
        "open",
        FilesystemLane,
        "path,flags,mode",
        "open flags",
        "root+cwd+contract",
        "legacy native path route"
    ),
    op!(
        3,
        "close",
        DescriptorCore,
        "fd",
        "none",
        "descriptor",
        "descriptor table"
    ),
    op!(
        4,
        "stat",
        FilesystemLane,
        "path,stat",
        "none",
        "root+cwd+contract",
        "legacy native path route"
    ),
    op!(
        5,
        "fstat",
        DescriptorCore,
        "fd,stat",
        "none",
        "descriptor:metadata",
        "descriptor table"
    ),
    op!(
        6,
        "lstat",
        FilesystemLane,
        "path,stat",
        "no-follow-final",
        "root+cwd+contract",
        "legacy native path route"
    ),
    op!(
        7,
        "poll",
        DescriptorCore,
        "fds,count,timeout",
        "poll events",
        "descriptor set",
        "descriptor mediation"
    ),
    op!(
        8,
        "lseek",
        DescriptorCore,
        "fd,offset,whence",
        "whence",
        "descriptor:read|write",
        "descriptor table"
    ),
    op!(
        9,
        "mmap",
        DescriptorCore,
        "addr,len,prot,flags,fd,offset",
        "map flags",
        "descriptor:mmap",
        "memory mediation"
    ),
    op!(
        16,
        "ioctl",
        DescriptorCore,
        "fd,request,arg",
        "request-specific",
        "descriptor+ioctl contract",
        "descriptor mediation"
    ),
    op!(
        17,
        "pread64",
        DescriptorCore,
        "fd,buf,count,offset",
        "none",
        "descriptor:read",
        "descriptor mediation"
    ),
    op!(
        18,
        "pwrite64",
        DescriptorCore,
        "fd,buf,count,offset",
        "none",
        "descriptor:write",
        "descriptor mediation"
    ),
    op!(
        19,
        "readv",
        DescriptorCore,
        "fd,iov,count",
        "none",
        "descriptor:read",
        "descriptor mediation"
    ),
    op!(
        20,
        "writev",
        DescriptorCore,
        "fd,iov,count",
        "none",
        "descriptor:write",
        "descriptor mediation"
    ),
    op!(
        21,
        "access",
        FilesystemLane,
        "path,mode",
        "access mode",
        "root+cwd+contract",
        "legacy native path route"
    ),
    op!(
        22,
        "pipe",
        DescriptorCore,
        "pipefd",
        "none",
        "descriptor table:create",
        "process descriptor mediation"
    ),
    op!(
        23,
        "select",
        DescriptorCore,
        "nfds,readfds,writefds,exceptfds,timeout",
        "descriptor sets",
        "descriptor set",
        "descriptor mediation"
    ),
    op!(
        32,
        "dup",
        DescriptorCore,
        "oldfd",
        "none",
        "descriptor",
        "descriptor table"
    ),
    op!(
        33,
        "dup2",
        DescriptorCore,
        "oldfd,newfd",
        "none",
        "descriptor",
        "descriptor table"
    ),
    op!(
        40,
        "sendfile",
        DescriptorCore,
        "outfd,infd,offset,count",
        "none",
        "two descriptors",
        "legacy descriptor mediation"
    ),
    op!(
        59,
        "execve",
        ProcessLane,
        "path,argv,envp",
        "none",
        "root+cwd+execute contract",
        "process lane"
    ),
    op!(
        72,
        "fcntl",
        DescriptorCore,
        "fd,command,arg",
        "command",
        "descriptor",
        "descriptor table"
    ),
    op!(
        73,
        "flock",
        DescriptorCore,
        "fd,operation",
        "lock operation",
        "descriptor:metadata",
        "descriptor mediation"
    ),
    op!(
        74,
        "fsync",
        DescriptorCore,
        "fd",
        "none",
        "descriptor:write",
        "descriptor mediation"
    ),
    op!(
        75,
        "fdatasync",
        DescriptorCore,
        "fd",
        "none",
        "descriptor:write",
        "descriptor mediation"
    ),
    op!(
        76,
        "truncate",
        FilesystemLane,
        "path,length",
        "none",
        "root+cwd+write contract",
        "legacy native path route"
    ),
    op!(
        77,
        "ftruncate",
        DescriptorCore,
        "fd,length",
        "none",
        "descriptor:write",
        "descriptor mediation"
    ),
    op!(
        78,
        "getdents",
        DescriptorCore,
        "fd,dirents,count",
        "none",
        "descriptor:read",
        "descriptor mediation"
    ),
    op!(
        79,
        "getcwd",
        FilesystemLane,
        "buffer,size",
        "none",
        "cwd",
        "legacy native path route"
    ),
    op!(
        80,
        "chdir",
        FilesystemLane,
        "path",
        "none",
        "root+cwd+lookup contract",
        "legacy native path route"
    ),
    op!(
        81,
        "fchdir",
        DescriptorCore,
        "fd",
        "none",
        "descriptor:lookup",
        "descriptor table"
    ),
    op!(
        82,
        "rename",
        FilesystemLane,
        "oldpath,newpath",
        "none",
        "two roots+parents+write contract",
        "legacy native path route"
    ),
    op!(
        83,
        "mkdir",
        FilesystemLane,
        "path,mode",
        "none",
        "parent:create contract",
        "legacy native path route"
    ),
    op!(
        84,
        "rmdir",
        FilesystemLane,
        "path",
        "none",
        "parent:write contract",
        "legacy native path route"
    ),
    op!(
        85,
        "creat",
        FilesystemLane,
        "path,mode",
        "create/truncate",
        "parent:create contract",
        "legacy native path route"
    ),
    op!(
        86,
        "link",
        FilesystemLane,
        "oldpath,newpath",
        "none",
        "source+destination parent",
        "legacy native path route"
    ),
    op!(
        87,
        "unlink",
        FilesystemLane,
        "path",
        "none",
        "parent:write contract",
        "legacy native path route"
    ),
    op!(
        88,
        "symlink",
        FilesystemLane,
        "target,linkpath",
        "none",
        "destination parent:create",
        "legacy native path route"
    ),
    op!(
        89,
        "readlink",
        FilesystemLane,
        "path,buffer,size",
        "no-follow-final",
        "root+cwd+read contract",
        "legacy native path route"
    ),
    op!(
        90,
        "chmod",
        FilesystemLane,
        "path,mode",
        "mode",
        "resolved object:metadata",
        "legacy native path route"
    ),
    op!(
        91,
        "fchmod",
        DescriptorCore,
        "fd,mode",
        "mode",
        "descriptor:metadata",
        "descriptor mediation"
    ),
    op!(
        92,
        "chown",
        FilesystemLane,
        "path,uid,gid",
        "none",
        "resolved object:metadata",
        "legacy native path route"
    ),
    op!(
        93,
        "fchown",
        DescriptorCore,
        "fd,uid,gid",
        "none",
        "descriptor:metadata",
        "descriptor mediation"
    ),
    op!(
        94,
        "lchown",
        FilesystemLane,
        "path,uid,gid",
        "no-follow-final",
        "resolved object:metadata",
        "legacy native path route"
    ),
    op!(
        95,
        "umask",
        ProcessLane,
        "mask",
        "mode",
        "task filesystem state",
        "process lane"
    ),
    op!(
        132,
        "utime",
        FilesystemLane,
        "path,times",
        "none",
        "resolved object:metadata",
        "legacy native path route"
    ),
    op!(
        133,
        "mknod",
        FilesystemLane,
        "path,mode,device",
        "node type",
        "parent:create+device contract",
        "legacy native path route"
    ),
    op!(
        137,
        "statfs",
        FilesystemLane,
        "path,statfs",
        "none",
        "root+cwd+read contract",
        "legacy native path route"
    ),
    op!(
        138,
        "fstatfs",
        DescriptorCore,
        "fd,statfs",
        "none",
        "descriptor:metadata",
        "descriptor mediation"
    ),
    op!(
        155,
        "pivot_root",
        ExplicitDeny,
        "new_root,put_old",
        "none",
        "mount capability",
        "explicit denial"
    ),
    op!(
        161,
        "chroot",
        ExplicitDeny,
        "path",
        "none",
        "instance root authority",
        "explicit denial"
    ),
    op!(
        162,
        "sync",
        ExplicitDeny,
        "none",
        "none",
        "host-global filesystem authority",
        "explicit denial"
    ),
    op!(
        163,
        "acct",
        ExplicitDeny,
        "path",
        "none",
        "host-global authority",
        "explicit denial"
    ),
    op!(
        165,
        "mount",
        ExplicitDeny,
        "source,target,type,flags,data",
        "mount flags",
        "mount capability",
        "explicit denial"
    ),
    op!(
        166,
        "umount2",
        ExplicitDeny,
        "target,flags",
        "unmount flags",
        "mount capability",
        "explicit denial"
    ),
    op!(
        167,
        "swapon",
        ExplicitDeny,
        "path,flags",
        "swap flags",
        "host-global authority",
        "explicit denial"
    ),
    op!(
        168,
        "swapoff",
        ExplicitDeny,
        "path",
        "none",
        "host-global authority",
        "explicit denial"
    ),
    op!(
        179,
        "quotactl",
        ExplicitDeny,
        "command,special,id,address",
        "quota command",
        "host-global filesystem authority",
        "explicit denial"
    ),
    op!(
        180,
        "nfsservctl",
        ExplicitDeny,
        "command,arg,response",
        "command-specific",
        "host-global filesystem authority",
        "explicit denial"
    ),
    op!(
        187,
        "readahead",
        DescriptorCore,
        "fd,offset,count",
        "none",
        "descriptor:read",
        "descriptor mediation"
    ),
    op!(
        188,
        "setxattr",
        FilesystemLane,
        "path,name,value,size,flags",
        "xattr flags",
        "resolved object:metadata",
        "legacy native path route"
    ),
    op!(
        189,
        "lsetxattr",
        FilesystemLane,
        "path,name,value,size,flags",
        "xattr flags",
        "resolved object:metadata",
        "legacy native path route"
    ),
    op!(
        190,
        "fsetxattr",
        DescriptorCore,
        "fd,name,value,size,flags",
        "xattr flags",
        "descriptor:metadata",
        "descriptor mediation"
    ),
    op!(
        191,
        "getxattr",
        FilesystemLane,
        "path,name,value,size",
        "none",
        "resolved object:read",
        "legacy native path route"
    ),
    op!(
        192,
        "lgetxattr",
        FilesystemLane,
        "path,name,value,size",
        "none",
        "resolved object:read",
        "legacy native path route"
    ),
    op!(
        193,
        "fgetxattr",
        DescriptorCore,
        "fd,name,value,size",
        "none",
        "descriptor:read",
        "descriptor mediation"
    ),
    op!(
        194,
        "listxattr",
        FilesystemLane,
        "path,list,size",
        "none",
        "resolved object:read",
        "legacy native path route"
    ),
    op!(
        195,
        "llistxattr",
        FilesystemLane,
        "path,list,size",
        "none",
        "resolved object:read",
        "legacy native path route"
    ),
    op!(
        196,
        "flistxattr",
        DescriptorCore,
        "fd,list,size",
        "none",
        "descriptor:read",
        "descriptor mediation"
    ),
    op!(
        197,
        "removexattr",
        FilesystemLane,
        "path,name",
        "none",
        "resolved object:metadata",
        "legacy native path route"
    ),
    op!(
        198,
        "lremovexattr",
        FilesystemLane,
        "path,name",
        "none",
        "resolved object:metadata",
        "legacy native path route"
    ),
    op!(
        199,
        "fremovexattr",
        DescriptorCore,
        "fd,name",
        "none",
        "descriptor:metadata",
        "descriptor mediation"
    ),
    op!(
        206,
        "io_setup",
        ExplicitDeny,
        "events,context",
        "none",
        "asynchronous I/O authority",
        "explicit denial"
    ),
    op!(
        207,
        "io_destroy",
        ExplicitDeny,
        "context",
        "none",
        "asynchronous I/O authority",
        "explicit denial"
    ),
    op!(
        208,
        "io_getevents",
        ExplicitDeny,
        "context,min,count,events,timeout",
        "none",
        "asynchronous I/O authority",
        "explicit denial"
    ),
    op!(
        209,
        "io_submit",
        ExplicitDeny,
        "context,count,requests",
        "request-specific",
        "asynchronous descriptor rights",
        "explicit denial"
    ),
    op!(
        210,
        "io_cancel",
        ExplicitDeny,
        "context,request,result",
        "none",
        "asynchronous I/O authority",
        "explicit denial"
    ),
    op!(
        213,
        "epoll_create",
        DescriptorCore,
        "size",
        "none",
        "descriptor table:create",
        "descriptor mediation"
    ),
    op!(
        217,
        "getdents64",
        DescriptorCore,
        "fd,dirents,count",
        "none",
        "descriptor:read",
        "descriptor mediation"
    ),
    op!(
        221,
        "fadvise64",
        DescriptorCore,
        "fd,offset,length,advice",
        "advice",
        "descriptor",
        "descriptor mediation"
    ),
    op!(
        232,
        "epoll_wait",
        DescriptorCore,
        "epfd,events,count,timeout",
        "none",
        "descriptor set",
        "descriptor mediation"
    ),
    op!(
        233,
        "epoll_ctl",
        DescriptorCore,
        "epfd,operation,fd,event",
        "control operation",
        "two descriptors",
        "descriptor mediation"
    ),
    op!(
        253,
        "inotify_init",
        DescriptorCore,
        "none",
        "none",
        "instance notification table",
        "legacy descriptor mediation"
    ),
    op!(
        254,
        "inotify_add_watch",
        DescriptorCore,
        "fd,path,mask",
        "watch mask",
        "descriptor+root+cwd",
        "legacy descriptor mediation"
    ),
    op!(
        255,
        "inotify_rm_watch",
        DescriptorCore,
        "fd,watch",
        "none",
        "descriptor",
        "legacy descriptor mediation"
    ),
    op!(
        257,
        "openat",
        FilesystemLane,
        "dirfd,path,flags,mode",
        "open flags",
        "root+cwd+dirfd+contract",
        "legacy native path route"
    ),
    op!(
        258,
        "mkdirat",
        FilesystemLane,
        "dirfd,path,mode",
        "none",
        "parent:create contract",
        "legacy native path route"
    ),
    op!(
        259,
        "mknodat",
        FilesystemLane,
        "dirfd,path,mode,device",
        "node type",
        "parent:create+device contract",
        "legacy native path route"
    ),
    op!(
        260,
        "fchownat",
        FilesystemLane,
        "dirfd,path,uid,gid,flags",
        "at flags",
        "resolved object:metadata",
        "legacy native path route"
    ),
    op!(
        261,
        "futimesat",
        FilesystemLane,
        "dirfd,path,times",
        "none",
        "resolved object:metadata",
        "legacy native path route"
    ),
    op!(
        262,
        "newfstatat",
        FilesystemLane,
        "dirfd,path,stat,flags",
        "at flags",
        "root+cwd+dirfd+read",
        "legacy native path route"
    ),
    op!(
        263,
        "unlinkat",
        FilesystemLane,
        "dirfd,path,flags",
        "at flags",
        "parent:write contract",
        "legacy native path route"
    ),
    op!(
        264,
        "renameat",
        FilesystemLane,
        "olddirfd,oldpath,newdirfd,newpath",
        "none",
        "two roots+parents+write contract",
        "legacy native path route"
    ),
    op!(
        265,
        "linkat",
        FilesystemLane,
        "olddirfd,oldpath,newdirfd,newpath,flags",
        "at flags",
        "source+destination parent",
        "legacy native path route"
    ),
    op!(
        266,
        "symlinkat",
        FilesystemLane,
        "target,newdirfd,linkpath",
        "none",
        "destination parent:create",
        "legacy native path route"
    ),
    op!(
        267,
        "readlinkat",
        FilesystemLane,
        "dirfd,path,buffer,size",
        "at flags",
        "root+cwd+dirfd+read",
        "legacy native path route"
    ),
    op!(
        268,
        "fchmodat",
        FilesystemLane,
        "dirfd,path,mode",
        "at flags",
        "resolved object:metadata",
        "legacy native path route"
    ),
    op!(
        269,
        "faccessat",
        FilesystemLane,
        "dirfd,path,mode",
        "access mode",
        "root+cwd+dirfd+read",
        "legacy native path route"
    ),
    op!(
        275,
        "splice",
        DescriptorCore,
        "infd,inoff,outfd,outoff,len,flags",
        "splice flags",
        "two descriptors",
        "legacy descriptor mediation"
    ),
    op!(
        276,
        "tee",
        DescriptorCore,
        "infd,outfd,len,flags",
        "splice flags",
        "two descriptors",
        "legacy descriptor mediation"
    ),
    op!(
        277,
        "sync_file_range",
        DescriptorCore,
        "fd,offset,count,flags",
        "sync flags",
        "descriptor:write",
        "descriptor mediation"
    ),
    op!(
        278,
        "vmsplice",
        DescriptorCore,
        "fd,iov,count,flags",
        "splice flags",
        "descriptor:write",
        "legacy descriptor mediation"
    ),
    op!(
        280,
        "utimensat",
        FilesystemLane,
        "dirfd,path,times,flags",
        "at flags",
        "resolved object:metadata",
        "legacy native path route"
    ),
    op!(
        281,
        "epoll_pwait",
        DescriptorCore,
        "epfd,events,count,timeout,sigmask,size",
        "none",
        "descriptor set",
        "descriptor mediation"
    ),
    op!(
        285,
        "fallocate",
        DescriptorCore,
        "fd,mode,offset,length",
        "allocate mode",
        "descriptor:write",
        "descriptor mediation"
    ),
    op!(
        291,
        "epoll_create1",
        DescriptorCore,
        "flags",
        "create flags",
        "descriptor table:create",
        "descriptor mediation"
    ),
    op!(
        292,
        "dup3",
        DescriptorCore,
        "oldfd,newfd,flags",
        "duplicate flags",
        "descriptor table",
        "descriptor mediation"
    ),
    op!(
        293,
        "pipe2",
        DescriptorCore,
        "pipefd,flags",
        "create flags",
        "descriptor table:create",
        "process descriptor mediation"
    ),
    op!(
        294,
        "inotify_init1",
        DescriptorCore,
        "flags",
        "create flags",
        "instance notification table",
        "legacy descriptor mediation"
    ),
    op!(
        295,
        "preadv",
        DescriptorCore,
        "fd,iov,count,offset",
        "none",
        "descriptor:read",
        "descriptor mediation"
    ),
    op!(
        296,
        "pwritev",
        DescriptorCore,
        "fd,iov,count,offset",
        "none",
        "descriptor:write",
        "descriptor mediation"
    ),
    op!(
        300,
        "fanotify_init",
        ExplicitDeny,
        "flags,event_flags",
        "notification flags",
        "filesystem observation authority",
        "explicit denial"
    ),
    op!(
        301,
        "fanotify_mark",
        ExplicitDeny,
        "fd,flags,mask,dirfd,path",
        "mark flags",
        "descriptor+root+cwd",
        "explicit denial"
    ),
    op!(
        303,
        "name_to_handle_at",
        ExplicitDeny,
        "dirfd,path,handle,mount,flags",
        "at flags",
        "opaque handle authority",
        "explicit denial"
    ),
    op!(
        304,
        "open_by_handle_at",
        ExplicitDeny,
        "mountfd,handle,flags",
        "open flags",
        "opaque handle authority",
        "explicit denial"
    ),
    op!(
        306,
        "syncfs",
        DescriptorCore,
        "fd",
        "none",
        "descriptor:write",
        "descriptor mediation"
    ),
    op!(
        313,
        "finit_module",
        ExplicitDeny,
        "fd,params,flags",
        "module flags",
        "host kernel authority",
        "explicit denial"
    ),
    op!(
        316,
        "renameat2",
        FilesystemLane,
        "olddirfd,oldpath,newdirfd,newpath,flags",
        "rename flags",
        "two roots+parents+write contract",
        "legacy native path route"
    ),
    op!(
        319,
        "memfd_create",
        DescriptorCore,
        "name,flags",
        "create flags",
        "instance memory file",
        "descriptor mediation"
    ),
    op!(
        322,
        "execveat",
        ProcessLane,
        "dirfd,path,argv,envp,flags",
        "at flags",
        "root+cwd+dirfd+execute",
        "process lane"
    ),
    op!(
        326,
        "copy_file_range",
        DescriptorCore,
        "infd,inoff,outfd,outoff,len,flags",
        "copy flags",
        "two descriptors",
        "legacy descriptor mediation"
    ),
    op!(
        327,
        "preadv2",
        DescriptorCore,
        "fd,iov,count,offset,flags",
        "rw flags",
        "descriptor:read",
        "descriptor mediation"
    ),
    op!(
        328,
        "pwritev2",
        DescriptorCore,
        "fd,iov,count,offset,flags",
        "rw flags",
        "descriptor:write",
        "descriptor mediation"
    ),
    op!(
        332,
        "statx",
        FilesystemLane,
        "dirfd,path,flags,mask,statx",
        "at flags",
        "root+cwd+dirfd+read",
        "legacy native path route"
    ),
    op!(
        333,
        "io_pgetevents",
        ExplicitDeny,
        "context,min,count,events,timeout,signal",
        "none",
        "asynchronous I/O authority",
        "explicit denial"
    ),
    op!(
        425,
        "io_uring_setup",
        ExplicitDeny,
        "entries,parameters",
        "setup flags",
        "asynchronous I/O authority",
        "explicit denial"
    ),
    op!(
        426,
        "io_uring_enter",
        ExplicitDeny,
        "fd,submit,min_complete,flags,sigmask,size",
        "enter flags",
        "asynchronous descriptor rights",
        "explicit denial"
    ),
    op!(
        427,
        "io_uring_register",
        ExplicitDeny,
        "fd,opcode,arg,count",
        "opcode-specific",
        "asynchronous descriptor rights",
        "explicit denial"
    ),
    op!(
        428,
        "open_tree",
        ExplicitDeny,
        "dirfd,path,flags",
        "mount flags",
        "mount capability",
        "explicit denial"
    ),
    op!(
        429,
        "move_mount",
        ExplicitDeny,
        "fromdir,from,todir,to,flags",
        "mount flags",
        "mount capability",
        "explicit denial"
    ),
    op!(
        430,
        "fsopen",
        ExplicitDeny,
        "filesystem,flags",
        "mount flags",
        "mount capability",
        "explicit denial"
    ),
    op!(
        431,
        "fsconfig",
        ExplicitDeny,
        "fd,command,key,value,aux",
        "command",
        "mount capability",
        "explicit denial"
    ),
    op!(
        432,
        "fsmount",
        ExplicitDeny,
        "fd,flags,attributes",
        "mount flags",
        "mount capability",
        "explicit denial"
    ),
    op!(
        433,
        "fspick",
        ExplicitDeny,
        "dirfd,path,flags",
        "mount flags",
        "mount capability",
        "explicit denial"
    ),
    op!(
        436,
        "close_range",
        DescriptorCore,
        "first,last,flags",
        "close flags",
        "descriptor table",
        "descriptor table"
    ),
    op!(
        437,
        "openat2",
        FilesystemLane,
        "dirfd,path,open_how,size",
        "open_how",
        "root+cwd+dirfd+contract",
        "legacy native path route"
    ),
    op!(
        438,
        "pidfd_getfd",
        ExplicitDeny,
        "pidfd,targetfd,flags",
        "none",
        "external descriptor import authority",
        "explicit denial"
    ),
    op!(
        439,
        "faccessat2",
        FilesystemLane,
        "dirfd,path,mode,flags",
        "at flags",
        "root+cwd+dirfd+read",
        "legacy native path route"
    ),
    op!(
        441,
        "epoll_pwait2",
        DescriptorCore,
        "epfd,events,count,timeout,sigmask,size",
        "none",
        "descriptor set",
        "descriptor mediation"
    ),
    op!(
        442,
        "mount_setattr",
        ExplicitDeny,
        "dirfd,path,flags,attributes,size",
        "mount flags",
        "mount capability",
        "explicit denial"
    ),
    op!(
        444,
        "landlock_create_ruleset",
        ExplicitDeny,
        "attribute,size,flags",
        "ruleset flags",
        "nested filesystem-policy authority",
        "explicit denial"
    ),
    op!(
        445,
        "landlock_add_rule",
        ExplicitDeny,
        "ruleset,rule_type,attribute,flags",
        "rule flags",
        "nested filesystem-policy authority",
        "explicit denial"
    ),
    op!(
        446,
        "landlock_restrict_self",
        ExplicitDeny,
        "ruleset,flags",
        "restriction flags",
        "nested filesystem-policy authority",
        "explicit denial"
    ),
    op!(
        451,
        "cachestat",
        DescriptorCore,
        "fd,range,output,flags",
        "query flags",
        "descriptor:metadata",
        "descriptor mediation"
    ),
    op!(
        452,
        "fchmodat2",
        FilesystemLane,
        "dirfd,path,mode,flags",
        "at flags",
        "resolved object:metadata",
        "legacy native path route"
    ),
    op!(
        457,
        "statmount",
        ExplicitDeny,
        "request,buffer,size,flags",
        "query flags",
        "mount capability",
        "explicit denial"
    ),
    op!(
        458,
        "listmount",
        ExplicitDeny,
        "request,mounts,count,flags",
        "query flags",
        "mount capability",
        "explicit denial"
    ),
];

pub fn operation(syscall: u64) -> Option<&'static Operation> {
    OPERATIONS
        .iter()
        .find(|operation| operation.syscall == syscall)
}

/// Fail-closed inventory lookup for the filesystem lane. Unknown ABI entries
/// cannot acquire a default owner or host executor.
pub fn filesystem_operation(context: &CallContext) -> Result<&'static Operation, i32> {
    if context.abi != X86_64_AUDIT_ARCH || !context.contract.permits() {
        return Err(ENOSYS);
    }
    operation(context.syscall)
        .filter(|operation| operation.owner == Owner::FilesystemLane)
        .ok_or(ENOSYS)
}

/// Shared fail-closed guard for every future filesystem hook. Inventory
/// ownership, contract permission, capability completeness, and route-local
/// confinement must all agree before a hook may perform or emulate an effect.
pub fn filesystem_hook_authorized(context: &CallContext, confinement_permits: bool) -> bool {
    confinement_permits && context.complete_for_path() && filesystem_operation(context).is_ok()
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;

    #[test]
    fn context_keeps_all_arguments_and_authority_dimensions() {
        let root = Capability::new(
            CapabilityKind::Root,
            7,
            11,
            13,
            ObjectId {
                device: 17,
                inode: 19,
            },
            Rights::LOOKUP.union(Rights::READ),
        );
        let context = CallContext::x86_64(
            264,
            [1, 2, 3, 4, 5, 6],
            TaskContext {
                host: 23,
                virtual_id: 29,
                generation: 31,
            },
            InstanceContext {
                identity: 7,
                lifetime: 11,
                root_generation: 13,
            },
            AuthorityContext {
                root,
                cwd: Capability::new(
                    CapabilityKind::WorkingDirectory,
                    7,
                    11,
                    37,
                    ObjectId {
                        device: 17,
                        inode: 41,
                    },
                    Rights::LOOKUP,
                ),
                descriptor_table: Capability::new(
                    CapabilityKind::DescriptorTable,
                    7,
                    11,
                    43,
                    ObjectId {
                        device: 7,
                        inode: 43,
                    },
                    Rights::LOOKUP.union(Rights::READ),
                ),
                cwd_generation: 37,
                descriptor_generation: 43,
            },
            ContractAuthorization::Mediate,
        );
        assert_eq!(context.arguments, [1, 2, 3, 4, 5, 6]);
        assert!(context.complete_for_path());

        let mut changed_tail = context;
        changed_tail.arguments[5] = 7;
        assert_ne!(context, changed_tail);

        let mut stale_root = context;
        stale_root.instance.root_generation += 1;
        assert!(!stale_root.complete_for_path());

        let mut stale_cwd = context;
        stale_cwd.authority.cwd_generation += 1;
        assert!(!stale_cwd.complete_for_path());

        let mut stale_descriptors = context;
        stale_descriptors.authority.descriptor_generation += 1;
        assert!(!stale_descriptors.complete_for_path());
    }

    #[test]
    fn capability_is_bound_to_instance_lifetime_and_generation() {
        let capability = Capability::new(
            CapabilityKind::Descriptor,
            2,
            3,
            5,
            ObjectId {
                device: 7,
                inode: 11,
            },
            Rights::READ,
        );
        assert!(capability.authorizes(2, 3, 5, Rights::READ));
        assert!(!capability.authorizes(9, 3, 5, Rights::READ));
        assert!(!capability.authorizes(2, 4, 5, Rights::READ));
        assert!(!capability.authorizes(2, 3, 6, Rights::READ));
        assert!(!capability.authorizes(2, 3, 5, Rights::WRITE));
    }

    #[test]
    fn incomplete_or_denied_context_cannot_admit_paths() {
        let missing = Capability::unavailable(CapabilityKind::Root, 2, 3);
        let context = CallContext::x86_64(
            2,
            [0; 6],
            TaskContext {
                host: 5,
                virtual_id: 7,
                generation: 11,
            },
            InstanceContext {
                identity: 2,
                lifetime: 3,
                root_generation: 0,
            },
            AuthorityContext {
                root: missing,
                cwd: missing,
                descriptor_table: missing,
                cwd_generation: 0,
                descriptor_generation: 0,
            },
            ContractAuthorization::Deny(1),
        );
        assert!(!context.complete_for_path());
    }

    #[test]
    fn inventory_is_unique_and_complete_per_row() {
        assert!(OPERATIONS.len() >= 100);
        for (index, operation) in OPERATIONS.iter().enumerate() {
            assert!(!operation.name.is_empty());
            assert!(!operation.arguments.is_empty());
            assert!(!operation.flags.is_empty());
            assert!(!operation.authority.is_empty());
            assert!(!operation.executor.is_empty());
            assert_eq!(operation.unsupported_errno, ENOSYS);
            assert!(!OPERATIONS[..index]
                .iter()
                .any(|other| other.syscall == operation.syscall));
        }
        assert!(OPERATIONS
            .windows(2)
            .all(|pair| pair[0].syscall < pair[1].syscall));
        for syscall in [2, 82, 264, 316, 332, 437, 452] {
            assert_eq!(
                operation(syscall).map(|entry| entry.owner),
                Some(Owner::FilesystemLane)
            );
        }
        for syscall in [
            155, 161, 162, 165, 179, 206, 209, 300, 304, 313, 425, 426, 427, 428, 438, 442, 444,
            445, 446, 457, 458,
        ] {
            assert_eq!(
                operation(syscall).map(|entry| entry.owner),
                Some(Owner::ExplicitDeny)
            );
        }
    }

    #[test]
    fn identity_tokens_are_stable_and_separate_instances() {
        assert_eq!(identity_token(b"rocky"), identity_token(b"rocky"));
        assert_ne!(identity_token(b"rocky"), identity_token(b"peer"));
        assert_ne!(identity_token(b""), 0);
    }

    #[test]
    fn filesystem_lookup_fails_closed_for_denial_unknown_abi_and_wrong_owner() {
        let capability = Capability::new(
            CapabilityKind::Root,
            2,
            3,
            5,
            ObjectId {
                device: 7,
                inode: 11,
            },
            Rights::LOOKUP,
        );
        let context = CallContext::x86_64(
            2,
            [0; 6],
            TaskContext {
                host: 13,
                virtual_id: 17,
                generation: 19,
            },
            InstanceContext {
                identity: 2,
                lifetime: 3,
                root_generation: 5,
            },
            AuthorityContext {
                root: capability,
                cwd: Capability {
                    kind: CapabilityKind::WorkingDirectory,
                    ..capability
                },
                descriptor_table: Capability {
                    kind: CapabilityKind::DescriptorTable,
                    ..capability
                },
                cwd_generation: 5,
                descriptor_generation: 5,
            },
            ContractAuthorization::Mediate,
        );
        assert_eq!(filesystem_operation(&context).unwrap().name, "open");

        let mut denied = context;
        denied.contract = ContractAuthorization::Deny(1);
        assert_eq!(filesystem_operation(&denied), Err(ENOSYS));

        let mut unknown_abi = context;
        unknown_abi.abi = 0;
        assert_eq!(filesystem_operation(&unknown_abi), Err(ENOSYS));

        let mut wrong_owner = context;
        wrong_owner.syscall = 0;
        assert_eq!(filesystem_operation(&wrong_owner), Err(ENOSYS));

        let mut unknown = context;
        unknown.syscall = u64::MAX;
        assert_eq!(filesystem_operation(&unknown), Err(ENOSYS));

        assert!(filesystem_hook_authorized(&context, true));
        assert!(!filesystem_hook_authorized(&context, false));
        assert!(!filesystem_hook_authorized(&denied, true));
        assert!(!filesystem_hook_authorized(&unknown_abi, true));
        assert!(!filesystem_hook_authorized(&wrong_owner, true));
        assert!(!filesystem_hook_authorized(&unknown, true));

        let mut stale = context;
        stale.authority.descriptor_generation += 1;
        assert!(!filesystem_hook_authorized(&stale, true));
    }
}
