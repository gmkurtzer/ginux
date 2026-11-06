//! Name-matched Linux syscall numbers for the two daemon architectures.
//!
//! The domain contract in this crate is written with x86_64 numbers.
//! An aarch64 supervisor translates at the ptrace boundary so the
//! same decisions apply to the generic aarch64 table. Numbers that
//! exist on only one architecture are absent and must fail closed.

#[derive(Clone, Copy)]
struct Pair {
    aarch64: u16,
    x86_64: u16,
}

static PAIRS: &[Pair] = &[
    Pair {
        aarch64: 0,
        x86_64: 206,
    }, // io_setup
    Pair {
        aarch64: 1,
        x86_64: 207,
    }, // io_destroy
    Pair {
        aarch64: 2,
        x86_64: 209,
    }, // io_submit
    Pair {
        aarch64: 3,
        x86_64: 210,
    }, // io_cancel
    Pair {
        aarch64: 4,
        x86_64: 208,
    }, // io_getevents
    Pair {
        aarch64: 5,
        x86_64: 188,
    }, // setxattr
    Pair {
        aarch64: 6,
        x86_64: 189,
    }, // lsetxattr
    Pair {
        aarch64: 7,
        x86_64: 190,
    }, // fsetxattr
    Pair {
        aarch64: 8,
        x86_64: 191,
    }, // getxattr
    Pair {
        aarch64: 9,
        x86_64: 192,
    }, // lgetxattr
    Pair {
        aarch64: 10,
        x86_64: 193,
    }, // fgetxattr
    Pair {
        aarch64: 11,
        x86_64: 194,
    }, // listxattr
    Pair {
        aarch64: 12,
        x86_64: 195,
    }, // llistxattr
    Pair {
        aarch64: 13,
        x86_64: 196,
    }, // flistxattr
    Pair {
        aarch64: 14,
        x86_64: 197,
    }, // removexattr
    Pair {
        aarch64: 15,
        x86_64: 198,
    }, // lremovexattr
    Pair {
        aarch64: 16,
        x86_64: 199,
    }, // fremovexattr
    Pair {
        aarch64: 17,
        x86_64: 79,
    }, // getcwd
    Pair {
        aarch64: 18,
        x86_64: 212,
    }, // lookup_dcookie
    Pair {
        aarch64: 19,
        x86_64: 290,
    }, // eventfd2
    Pair {
        aarch64: 20,
        x86_64: 291,
    }, // epoll_create1
    Pair {
        aarch64: 21,
        x86_64: 233,
    }, // epoll_ctl
    Pair {
        aarch64: 22,
        x86_64: 281,
    }, // epoll_pwait
    Pair {
        aarch64: 23,
        x86_64: 32,
    }, // dup
    Pair {
        aarch64: 24,
        x86_64: 292,
    }, // dup3
    Pair {
        aarch64: 25,
        x86_64: 72,
    }, // fcntl
    Pair {
        aarch64: 26,
        x86_64: 294,
    }, // inotify_init1
    Pair {
        aarch64: 27,
        x86_64: 254,
    }, // inotify_add_watch
    Pair {
        aarch64: 28,
        x86_64: 255,
    }, // inotify_rm_watch
    Pair {
        aarch64: 29,
        x86_64: 16,
    }, // ioctl
    Pair {
        aarch64: 30,
        x86_64: 251,
    }, // ioprio_set
    Pair {
        aarch64: 31,
        x86_64: 252,
    }, // ioprio_get
    Pair {
        aarch64: 32,
        x86_64: 73,
    }, // flock
    Pair {
        aarch64: 33,
        x86_64: 259,
    }, // mknodat
    Pair {
        aarch64: 34,
        x86_64: 258,
    }, // mkdirat
    Pair {
        aarch64: 35,
        x86_64: 263,
    }, // unlinkat
    Pair {
        aarch64: 36,
        x86_64: 266,
    }, // symlinkat
    Pair {
        aarch64: 37,
        x86_64: 265,
    }, // linkat
    Pair {
        aarch64: 38,
        x86_64: 264,
    }, // renameat
    Pair {
        aarch64: 39,
        x86_64: 166,
    }, // umount2
    Pair {
        aarch64: 40,
        x86_64: 165,
    }, // mount
    Pair {
        aarch64: 41,
        x86_64: 155,
    }, // pivot_root
    Pair {
        aarch64: 42,
        x86_64: 180,
    }, // nfsservctl
    Pair {
        aarch64: 43,
        x86_64: 137,
    }, // statfs
    Pair {
        aarch64: 44,
        x86_64: 138,
    }, // fstatfs
    Pair {
        aarch64: 45,
        x86_64: 76,
    }, // truncate
    Pair {
        aarch64: 46,
        x86_64: 77,
    }, // ftruncate
    Pair {
        aarch64: 47,
        x86_64: 285,
    }, // fallocate
    Pair {
        aarch64: 48,
        x86_64: 269,
    }, // faccessat
    Pair {
        aarch64: 49,
        x86_64: 80,
    }, // chdir
    Pair {
        aarch64: 50,
        x86_64: 81,
    }, // fchdir
    Pair {
        aarch64: 51,
        x86_64: 161,
    }, // chroot
    Pair {
        aarch64: 52,
        x86_64: 91,
    }, // fchmod
    Pair {
        aarch64: 53,
        x86_64: 268,
    }, // fchmodat
    Pair {
        aarch64: 54,
        x86_64: 260,
    }, // fchownat
    Pair {
        aarch64: 55,
        x86_64: 93,
    }, // fchown
    Pair {
        aarch64: 56,
        x86_64: 257,
    }, // openat
    Pair {
        aarch64: 57,
        x86_64: 3,
    }, // close
    Pair {
        aarch64: 58,
        x86_64: 153,
    }, // vhangup
    Pair {
        aarch64: 59,
        x86_64: 293,
    }, // pipe2
    Pair {
        aarch64: 60,
        x86_64: 179,
    }, // quotactl
    Pair {
        aarch64: 61,
        x86_64: 217,
    }, // getdents64
    Pair {
        aarch64: 62,
        x86_64: 8,
    }, // lseek
    Pair {
        aarch64: 63,
        x86_64: 0,
    }, // read
    Pair {
        aarch64: 64,
        x86_64: 1,
    }, // write
    Pair {
        aarch64: 65,
        x86_64: 19,
    }, // readv
    Pair {
        aarch64: 66,
        x86_64: 20,
    }, // writev
    Pair {
        aarch64: 67,
        x86_64: 17,
    }, // pread64
    Pair {
        aarch64: 68,
        x86_64: 18,
    }, // pwrite64
    Pair {
        aarch64: 69,
        x86_64: 295,
    }, // preadv
    Pair {
        aarch64: 70,
        x86_64: 296,
    }, // pwritev
    Pair {
        aarch64: 71,
        x86_64: 40,
    }, // sendfile
    Pair {
        aarch64: 72,
        x86_64: 270,
    }, // pselect6
    Pair {
        aarch64: 73,
        x86_64: 271,
    }, // ppoll
    Pair {
        aarch64: 74,
        x86_64: 289,
    }, // signalfd4
    Pair {
        aarch64: 75,
        x86_64: 278,
    }, // vmsplice
    Pair {
        aarch64: 76,
        x86_64: 275,
    }, // splice
    Pair {
        aarch64: 77,
        x86_64: 276,
    }, // tee
    Pair {
        aarch64: 78,
        x86_64: 267,
    }, // readlinkat
    Pair {
        aarch64: 79,
        x86_64: 262,
    }, // newfstatat
    Pair {
        aarch64: 80,
        x86_64: 5,
    }, // fstat
    Pair {
        aarch64: 81,
        x86_64: 162,
    }, // sync
    Pair {
        aarch64: 82,
        x86_64: 74,
    }, // fsync
    Pair {
        aarch64: 83,
        x86_64: 75,
    }, // fdatasync
    Pair {
        aarch64: 84,
        x86_64: 277,
    }, // sync_file_range
    Pair {
        aarch64: 85,
        x86_64: 283,
    }, // timerfd_create
    Pair {
        aarch64: 86,
        x86_64: 286,
    }, // timerfd_settime
    Pair {
        aarch64: 87,
        x86_64: 287,
    }, // timerfd_gettime
    Pair {
        aarch64: 88,
        x86_64: 280,
    }, // utimensat
    Pair {
        aarch64: 89,
        x86_64: 163,
    }, // acct
    Pair {
        aarch64: 90,
        x86_64: 125,
    }, // capget
    Pair {
        aarch64: 91,
        x86_64: 126,
    }, // capset
    Pair {
        aarch64: 92,
        x86_64: 135,
    }, // personality
    Pair {
        aarch64: 93,
        x86_64: 60,
    }, // exit
    Pair {
        aarch64: 94,
        x86_64: 231,
    }, // exit_group
    Pair {
        aarch64: 95,
        x86_64: 247,
    }, // waitid
    Pair {
        aarch64: 96,
        x86_64: 218,
    }, // set_tid_address
    Pair {
        aarch64: 97,
        x86_64: 272,
    }, // unshare
    Pair {
        aarch64: 98,
        x86_64: 202,
    }, // futex
    Pair {
        aarch64: 99,
        x86_64: 273,
    }, // set_robust_list
    Pair {
        aarch64: 100,
        x86_64: 274,
    }, // get_robust_list
    Pair {
        aarch64: 101,
        x86_64: 35,
    }, // nanosleep
    Pair {
        aarch64: 102,
        x86_64: 36,
    }, // getitimer
    Pair {
        aarch64: 103,
        x86_64: 38,
    }, // setitimer
    Pair {
        aarch64: 104,
        x86_64: 246,
    }, // kexec_load
    Pair {
        aarch64: 105,
        x86_64: 175,
    }, // init_module
    Pair {
        aarch64: 106,
        x86_64: 176,
    }, // delete_module
    Pair {
        aarch64: 107,
        x86_64: 222,
    }, // timer_create
    Pair {
        aarch64: 108,
        x86_64: 224,
    }, // timer_gettime
    Pair {
        aarch64: 109,
        x86_64: 225,
    }, // timer_getoverrun
    Pair {
        aarch64: 110,
        x86_64: 223,
    }, // timer_settime
    Pair {
        aarch64: 111,
        x86_64: 226,
    }, // timer_delete
    Pair {
        aarch64: 112,
        x86_64: 227,
    }, // clock_settime
    Pair {
        aarch64: 113,
        x86_64: 228,
    }, // clock_gettime
    Pair {
        aarch64: 114,
        x86_64: 229,
    }, // clock_getres
    Pair {
        aarch64: 115,
        x86_64: 230,
    }, // clock_nanosleep
    Pair {
        aarch64: 116,
        x86_64: 103,
    }, // syslog
    Pair {
        aarch64: 117,
        x86_64: 101,
    }, // ptrace
    Pair {
        aarch64: 118,
        x86_64: 142,
    }, // sched_setparam
    Pair {
        aarch64: 119,
        x86_64: 144,
    }, // sched_setscheduler
    Pair {
        aarch64: 120,
        x86_64: 145,
    }, // sched_getscheduler
    Pair {
        aarch64: 121,
        x86_64: 143,
    }, // sched_getparam
    Pair {
        aarch64: 122,
        x86_64: 203,
    }, // sched_setaffinity
    Pair {
        aarch64: 123,
        x86_64: 204,
    }, // sched_getaffinity
    Pair {
        aarch64: 124,
        x86_64: 24,
    }, // sched_yield
    Pair {
        aarch64: 125,
        x86_64: 146,
    }, // sched_get_priority_max
    Pair {
        aarch64: 126,
        x86_64: 147,
    }, // sched_get_priority_min
    Pair {
        aarch64: 127,
        x86_64: 148,
    }, // sched_rr_get_interval
    Pair {
        aarch64: 128,
        x86_64: 219,
    }, // restart_syscall
    Pair {
        aarch64: 129,
        x86_64: 62,
    }, // kill
    Pair {
        aarch64: 130,
        x86_64: 200,
    }, // tkill
    Pair {
        aarch64: 131,
        x86_64: 234,
    }, // tgkill
    Pair {
        aarch64: 132,
        x86_64: 131,
    }, // sigaltstack
    Pair {
        aarch64: 133,
        x86_64: 130,
    }, // rt_sigsuspend
    Pair {
        aarch64: 134,
        x86_64: 13,
    }, // rt_sigaction
    Pair {
        aarch64: 135,
        x86_64: 14,
    }, // rt_sigprocmask
    Pair {
        aarch64: 136,
        x86_64: 127,
    }, // rt_sigpending
    Pair {
        aarch64: 137,
        x86_64: 128,
    }, // rt_sigtimedwait
    Pair {
        aarch64: 138,
        x86_64: 129,
    }, // rt_sigqueueinfo
    Pair {
        aarch64: 139,
        x86_64: 15,
    }, // rt_sigreturn
    Pair {
        aarch64: 140,
        x86_64: 141,
    }, // setpriority
    Pair {
        aarch64: 141,
        x86_64: 140,
    }, // getpriority
    Pair {
        aarch64: 142,
        x86_64: 169,
    }, // reboot
    Pair {
        aarch64: 143,
        x86_64: 114,
    }, // setregid
    Pair {
        aarch64: 144,
        x86_64: 106,
    }, // setgid
    Pair {
        aarch64: 145,
        x86_64: 113,
    }, // setreuid
    Pair {
        aarch64: 146,
        x86_64: 105,
    }, // setuid
    Pair {
        aarch64: 147,
        x86_64: 117,
    }, // setresuid
    Pair {
        aarch64: 148,
        x86_64: 118,
    }, // getresuid
    Pair {
        aarch64: 149,
        x86_64: 119,
    }, // setresgid
    Pair {
        aarch64: 150,
        x86_64: 120,
    }, // getresgid
    Pair {
        aarch64: 151,
        x86_64: 122,
    }, // setfsuid
    Pair {
        aarch64: 152,
        x86_64: 123,
    }, // setfsgid
    Pair {
        aarch64: 153,
        x86_64: 100,
    }, // times
    Pair {
        aarch64: 154,
        x86_64: 109,
    }, // setpgid
    Pair {
        aarch64: 155,
        x86_64: 121,
    }, // getpgid
    Pair {
        aarch64: 156,
        x86_64: 124,
    }, // getsid
    Pair {
        aarch64: 157,
        x86_64: 112,
    }, // setsid
    Pair {
        aarch64: 158,
        x86_64: 115,
    }, // getgroups
    Pair {
        aarch64: 159,
        x86_64: 116,
    }, // setgroups
    Pair {
        aarch64: 160,
        x86_64: 63,
    }, // uname
    Pair {
        aarch64: 161,
        x86_64: 170,
    }, // sethostname
    Pair {
        aarch64: 162,
        x86_64: 171,
    }, // setdomainname
    Pair {
        aarch64: 163,
        x86_64: 97,
    }, // getrlimit
    Pair {
        aarch64: 164,
        x86_64: 160,
    }, // setrlimit
    Pair {
        aarch64: 165,
        x86_64: 98,
    }, // getrusage
    Pair {
        aarch64: 166,
        x86_64: 95,
    }, // umask
    Pair {
        aarch64: 167,
        x86_64: 157,
    }, // prctl
    Pair {
        aarch64: 168,
        x86_64: 309,
    }, // getcpu
    Pair {
        aarch64: 169,
        x86_64: 96,
    }, // gettimeofday
    Pair {
        aarch64: 170,
        x86_64: 164,
    }, // settimeofday
    Pair {
        aarch64: 171,
        x86_64: 159,
    }, // adjtimex
    Pair {
        aarch64: 172,
        x86_64: 39,
    }, // getpid
    Pair {
        aarch64: 173,
        x86_64: 110,
    }, // getppid
    Pair {
        aarch64: 174,
        x86_64: 102,
    }, // getuid
    Pair {
        aarch64: 175,
        x86_64: 107,
    }, // geteuid
    Pair {
        aarch64: 176,
        x86_64: 104,
    }, // getgid
    Pair {
        aarch64: 177,
        x86_64: 108,
    }, // getegid
    Pair {
        aarch64: 178,
        x86_64: 186,
    }, // gettid
    Pair {
        aarch64: 179,
        x86_64: 99,
    }, // sysinfo
    Pair {
        aarch64: 180,
        x86_64: 240,
    }, // mq_open
    Pair {
        aarch64: 181,
        x86_64: 241,
    }, // mq_unlink
    Pair {
        aarch64: 182,
        x86_64: 242,
    }, // mq_timedsend
    Pair {
        aarch64: 183,
        x86_64: 243,
    }, // mq_timedreceive
    Pair {
        aarch64: 184,
        x86_64: 244,
    }, // mq_notify
    Pair {
        aarch64: 185,
        x86_64: 245,
    }, // mq_getsetattr
    Pair {
        aarch64: 186,
        x86_64: 68,
    }, // msgget
    Pair {
        aarch64: 187,
        x86_64: 71,
    }, // msgctl
    Pair {
        aarch64: 188,
        x86_64: 70,
    }, // msgrcv
    Pair {
        aarch64: 189,
        x86_64: 69,
    }, // msgsnd
    Pair {
        aarch64: 190,
        x86_64: 64,
    }, // semget
    Pair {
        aarch64: 191,
        x86_64: 66,
    }, // semctl
    Pair {
        aarch64: 192,
        x86_64: 220,
    }, // semtimedop
    Pair {
        aarch64: 193,
        x86_64: 65,
    }, // semop
    Pair {
        aarch64: 194,
        x86_64: 29,
    }, // shmget
    Pair {
        aarch64: 195,
        x86_64: 31,
    }, // shmctl
    Pair {
        aarch64: 196,
        x86_64: 30,
    }, // shmat
    Pair {
        aarch64: 197,
        x86_64: 67,
    }, // shmdt
    Pair {
        aarch64: 198,
        x86_64: 41,
    }, // socket
    Pair {
        aarch64: 199,
        x86_64: 53,
    }, // socketpair
    Pair {
        aarch64: 200,
        x86_64: 49,
    }, // bind
    Pair {
        aarch64: 201,
        x86_64: 50,
    }, // listen
    Pair {
        aarch64: 202,
        x86_64: 43,
    }, // accept
    Pair {
        aarch64: 203,
        x86_64: 42,
    }, // connect
    Pair {
        aarch64: 204,
        x86_64: 51,
    }, // getsockname
    Pair {
        aarch64: 205,
        x86_64: 52,
    }, // getpeername
    Pair {
        aarch64: 206,
        x86_64: 44,
    }, // sendto
    Pair {
        aarch64: 207,
        x86_64: 45,
    }, // recvfrom
    Pair {
        aarch64: 208,
        x86_64: 54,
    }, // setsockopt
    Pair {
        aarch64: 209,
        x86_64: 55,
    }, // getsockopt
    Pair {
        aarch64: 210,
        x86_64: 48,
    }, // shutdown
    Pair {
        aarch64: 211,
        x86_64: 46,
    }, // sendmsg
    Pair {
        aarch64: 212,
        x86_64: 47,
    }, // recvmsg
    Pair {
        aarch64: 213,
        x86_64: 187,
    }, // readahead
    Pair {
        aarch64: 214,
        x86_64: 12,
    }, // brk
    Pair {
        aarch64: 215,
        x86_64: 11,
    }, // munmap
    Pair {
        aarch64: 216,
        x86_64: 25,
    }, // mremap
    Pair {
        aarch64: 217,
        x86_64: 248,
    }, // add_key
    Pair {
        aarch64: 218,
        x86_64: 249,
    }, // request_key
    Pair {
        aarch64: 219,
        x86_64: 250,
    }, // keyctl
    Pair {
        aarch64: 220,
        x86_64: 56,
    }, // clone
    Pair {
        aarch64: 221,
        x86_64: 59,
    }, // execve
    Pair {
        aarch64: 222,
        x86_64: 9,
    }, // mmap
    Pair {
        aarch64: 223,
        x86_64: 221,
    }, // fadvise64
    Pair {
        aarch64: 224,
        x86_64: 167,
    }, // swapon
    Pair {
        aarch64: 225,
        x86_64: 168,
    }, // swapoff
    Pair {
        aarch64: 226,
        x86_64: 10,
    }, // mprotect
    Pair {
        aarch64: 227,
        x86_64: 26,
    }, // msync
    Pair {
        aarch64: 228,
        x86_64: 149,
    }, // mlock
    Pair {
        aarch64: 229,
        x86_64: 150,
    }, // munlock
    Pair {
        aarch64: 230,
        x86_64: 151,
    }, // mlockall
    Pair {
        aarch64: 231,
        x86_64: 152,
    }, // munlockall
    Pair {
        aarch64: 232,
        x86_64: 27,
    }, // mincore
    Pair {
        aarch64: 233,
        x86_64: 28,
    }, // madvise
    Pair {
        aarch64: 234,
        x86_64: 216,
    }, // remap_file_pages
    Pair {
        aarch64: 235,
        x86_64: 237,
    }, // mbind
    Pair {
        aarch64: 236,
        x86_64: 239,
    }, // get_mempolicy
    Pair {
        aarch64: 237,
        x86_64: 238,
    }, // set_mempolicy
    Pair {
        aarch64: 238,
        x86_64: 256,
    }, // migrate_pages
    Pair {
        aarch64: 239,
        x86_64: 279,
    }, // move_pages
    Pair {
        aarch64: 240,
        x86_64: 297,
    }, // rt_tgsigqueueinfo
    Pair {
        aarch64: 241,
        x86_64: 298,
    }, // perf_event_open
    Pair {
        aarch64: 242,
        x86_64: 288,
    }, // accept4
    Pair {
        aarch64: 243,
        x86_64: 299,
    }, // recvmmsg
    Pair {
        aarch64: 260,
        x86_64: 61,
    }, // wait4
    Pair {
        aarch64: 261,
        x86_64: 302,
    }, // prlimit64
    Pair {
        aarch64: 262,
        x86_64: 300,
    }, // fanotify_init
    Pair {
        aarch64: 263,
        x86_64: 301,
    }, // fanotify_mark
    Pair {
        aarch64: 264,
        x86_64: 303,
    }, // name_to_handle_at
    Pair {
        aarch64: 265,
        x86_64: 304,
    }, // open_by_handle_at
    Pair {
        aarch64: 266,
        x86_64: 305,
    }, // clock_adjtime
    Pair {
        aarch64: 267,
        x86_64: 306,
    }, // syncfs
    Pair {
        aarch64: 268,
        x86_64: 308,
    }, // setns
    Pair {
        aarch64: 269,
        x86_64: 307,
    }, // sendmmsg
    Pair {
        aarch64: 270,
        x86_64: 310,
    }, // process_vm_readv
    Pair {
        aarch64: 271,
        x86_64: 311,
    }, // process_vm_writev
    Pair {
        aarch64: 272,
        x86_64: 312,
    }, // kcmp
    Pair {
        aarch64: 273,
        x86_64: 313,
    }, // finit_module
    Pair {
        aarch64: 274,
        x86_64: 314,
    }, // sched_setattr
    Pair {
        aarch64: 275,
        x86_64: 315,
    }, // sched_getattr
    Pair {
        aarch64: 276,
        x86_64: 316,
    }, // renameat2
    Pair {
        aarch64: 277,
        x86_64: 317,
    }, // seccomp
    Pair {
        aarch64: 278,
        x86_64: 318,
    }, // getrandom
    Pair {
        aarch64: 279,
        x86_64: 319,
    }, // memfd_create
    Pair {
        aarch64: 280,
        x86_64: 321,
    }, // bpf
    Pair {
        aarch64: 281,
        x86_64: 322,
    }, // execveat
    Pair {
        aarch64: 282,
        x86_64: 323,
    }, // userfaultfd
    Pair {
        aarch64: 283,
        x86_64: 324,
    }, // membarrier
    Pair {
        aarch64: 284,
        x86_64: 325,
    }, // mlock2
    Pair {
        aarch64: 285,
        x86_64: 326,
    }, // copy_file_range
    Pair {
        aarch64: 286,
        x86_64: 327,
    }, // preadv2
    Pair {
        aarch64: 287,
        x86_64: 328,
    }, // pwritev2
    Pair {
        aarch64: 288,
        x86_64: 329,
    }, // pkey_mprotect
    Pair {
        aarch64: 289,
        x86_64: 330,
    }, // pkey_alloc
    Pair {
        aarch64: 290,
        x86_64: 331,
    }, // pkey_free
    Pair {
        aarch64: 291,
        x86_64: 332,
    }, // statx
    Pair {
        aarch64: 292,
        x86_64: 333,
    }, // io_pgetevents
    Pair {
        aarch64: 293,
        x86_64: 334,
    }, // rseq
    Pair {
        aarch64: 294,
        x86_64: 320,
    }, // kexec_file_load
    Pair {
        aarch64: 424,
        x86_64: 424,
    }, // pidfd_send_signal
    Pair {
        aarch64: 425,
        x86_64: 425,
    }, // io_uring_setup
    Pair {
        aarch64: 426,
        x86_64: 426,
    }, // io_uring_enter
    Pair {
        aarch64: 427,
        x86_64: 427,
    }, // io_uring_register
    Pair {
        aarch64: 428,
        x86_64: 428,
    }, // open_tree
    Pair {
        aarch64: 429,
        x86_64: 429,
    }, // move_mount
    Pair {
        aarch64: 430,
        x86_64: 430,
    }, // fsopen
    Pair {
        aarch64: 431,
        x86_64: 431,
    }, // fsconfig
    Pair {
        aarch64: 432,
        x86_64: 432,
    }, // fsmount
    Pair {
        aarch64: 433,
        x86_64: 433,
    }, // fspick
    Pair {
        aarch64: 434,
        x86_64: 434,
    }, // pidfd_open
    Pair {
        aarch64: 435,
        x86_64: 435,
    }, // clone3
    Pair {
        aarch64: 436,
        x86_64: 436,
    }, // close_range
    Pair {
        aarch64: 437,
        x86_64: 437,
    }, // openat2
    Pair {
        aarch64: 438,
        x86_64: 438,
    }, // pidfd_getfd
    Pair {
        aarch64: 439,
        x86_64: 439,
    }, // faccessat2
    Pair {
        aarch64: 440,
        x86_64: 440,
    }, // process_madvise
    Pair {
        aarch64: 441,
        x86_64: 441,
    }, // epoll_pwait2
    Pair {
        aarch64: 442,
        x86_64: 442,
    }, // mount_setattr
    Pair {
        aarch64: 443,
        x86_64: 443,
    }, // quotactl_fd
    Pair {
        aarch64: 444,
        x86_64: 444,
    }, // landlock_create_ruleset
    Pair {
        aarch64: 445,
        x86_64: 445,
    }, // landlock_add_rule
    Pair {
        aarch64: 446,
        x86_64: 446,
    }, // landlock_restrict_self
    Pair {
        aarch64: 447,
        x86_64: 447,
    }, // memfd_secret
    Pair {
        aarch64: 448,
        x86_64: 448,
    }, // process_mrelease
    Pair {
        aarch64: 449,
        x86_64: 449,
    }, // futex_waitv
    Pair {
        aarch64: 450,
        x86_64: 450,
    }, // set_mempolicy_home_node
    Pair {
        aarch64: 451,
        x86_64: 451,
    }, // cachestat
    Pair {
        aarch64: 452,
        x86_64: 452,
    }, // fchmodat2
    Pair {
        aarch64: 453,
        x86_64: 453,
    }, // map_shadow_stack
    Pair {
        aarch64: 454,
        x86_64: 454,
    }, // futex_wake
    Pair {
        aarch64: 455,
        x86_64: 455,
    }, // futex_wait
    Pair {
        aarch64: 456,
        x86_64: 456,
    }, // futex_requeue
    Pair {
        aarch64: 457,
        x86_64: 457,
    }, // statmount
    Pair {
        aarch64: 458,
        x86_64: 458,
    }, // listmount
    Pair {
        aarch64: 459,
        x86_64: 459,
    }, // lsm_get_self_attr
    Pair {
        aarch64: 460,
        x86_64: 460,
    }, // lsm_set_self_attr
    Pair {
        aarch64: 461,
        x86_64: 461,
    }, // lsm_list_modules
];

pub fn to_x86(aarch64: u64) -> Option<u64> {
    let number = u16::try_from(aarch64).ok()?;
    PAIRS
        .iter()
        .find(|pair| pair.aarch64 == number)
        .map(|pair| u64::from(pair.x86_64))
}

pub fn to_aarch64(x86_64: u64) -> Option<u64> {
    let number = u16::try_from(x86_64).ok()?;
    PAIRS
        .iter()
        .find(|pair| pair.x86_64 == number)
        .map(|pair| u64::from(pair.aarch64))
}

#[cfg(test)]
mod tests {
    use super::{to_aarch64, to_x86, PAIRS};

    #[test]
    fn names_do_not_follow_numeric_collisions() {
        assert_eq!(to_x86(63), Some(0));
        assert_eq!(to_x86(160), Some(63));
        assert_eq!(to_x86(64), Some(1));
        assert_eq!(to_x86(220), Some(56));
        assert_eq!(to_x86(56), Some(257));
        assert_eq!(to_aarch64(57), None);
    }

    #[test]
    fn every_pair_round_trips() {
        for pair in PAIRS {
            assert_eq!(
                to_x86(u64::from(pair.aarch64)),
                Some(u64::from(pair.x86_64))
            );
            assert_eq!(
                to_aarch64(u64::from(pair.x86_64)),
                Some(u64::from(pair.aarch64))
            );
        }
    }
}
