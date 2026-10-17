# Linux x86-64 Userspace Compatibility Matrix

This document is the compatibility ledger for running unmodified Linux x86-64
userspace on Ginux. It records the target contract, the current implementation,
and the remaining work. It is an index, not a substitute for the detailed
behavioral specifications and tests.

## Contract and authorities

There is no single document that defines the complete Linux userspace ABI.
Ginux uses this authority order:

1. The pinned [Linux v6.12 x86-64 syscall table](https://github.com/torvalds/linux/blob/v6.12/arch/x86/entry/syscalls/syscall_64.tbl)
   defines native x86-64 syscall names and numbers.
2. Linux [UAPI headers](https://github.com/torvalds/linux/tree/v6.12/include/uapi)
   define userspace-visible constants, flags, structures, ioctl encodings, and
   filesystem interfaces.
3. The [Linux man-pages project](https://www.kernel.org/doc/man-pages/)
   defines expected syscall behavior, errors, and observable semantics.
4. The [x86-64 psABI](https://gitlab.com/x86-psABIs/x86-64-ABI)
   and System V generic ABI define ELF, relocation, process-entry, calling,
   register, stack, and TLS conventions.
5. Linux's documented [stable ABI](https://www.kernel.org/doc/html/latest/admin-guide/abi-stable.html),
   procfs/sysfs documentation, and behavior verified by Linux selftests define
   the remaining observable kernel/userspace interfaces.

The baseline is native 64-bit x86-64 Linux v6.12. The x32 and i386 compatibility
ABIs are out of scope. Moving the baseline requires an explicit review of the
new syscall table and UAPI changes.

## Status rules

| Status | Meaning |
|---|---|
| Complete | Matches the pinned Linux contract and has positive, negative, and edge-case conformance tests. |
| Partial | Dispatched and useful, but intentionally bounded or not yet proven equivalent to Linux. |
| Policy-denied | Recognized as a privileged operation and rejected by the container authority policy. This is not full syscall support. |
| Missing | Returns `-ENOSYS` through the default dispatcher. |

Detailed bounded behavior for implemented calls lives in
[Linux Syscall Entry v0](../specs/kernel/linux-syscall-v0.md).
The matrix must be updated in the same commit whenever syscall dispatch or its
documented behavior changes.

Current totals: **57 Complete**, **270 Partial**, **48 Policy-denied**, **0 Missing**.

## Non-syscall ABI surfaces

| Surface | Status | Current boundary |
|---|---|---|
| x86-64 `SYSCALL` entry and return | Partial | Native register convention and `-errno` returns; bounded context validation. |
| ELF64 loading | Partial | `ET_EXEC` and dynamically interpreted `ET_DYN` use bounded program-header and relocation profiles. Linux static PIE (`ET_DYN` without `PT_INTERP`, including Rocky's `ldconfig`) remains unsupported and is rejected with `ENOEXEC`; see [Linux ELF64 Profile v0](../specs/kernel/linux-elf-v0.md). |
| Initial stack and auxiliary vector | Partial | Bounded argv/envp, selected `AT_*` entries, kernel-provided `AT_RANDOM`, and a one-page initial runtime stack that grows downward on demand; no vDSO. |
| Dynamic loader and shared objects | Partial | Unmodified Alpine/musl and Rocky 10/glibc shells load. Rocky's dynamic coreutils `cat` and `ls` execute from the OCI root. Broader dependency, relocation, locale, and NSS coverage remains incomplete. |
| TLS | Partial | Bounded static TLS and `arch_prctl` FS-base support. |
| Process and thread model | Partial | Repeated fork/exec/exit/wait works for Alpine and Rocky shells. Fork preserves parent memory and descriptor state; exec privately replaces shared image pages. `process_vm_readv`/`process_vm_writev` support validated same-process transfers and reject crossing the process/domain boundary. `kcmp` identifies same-process VM/resource tables and known shared descriptors; cross-process comparison remains unavailable. Per-thread `rseq` registration supports original and extended lengths, Linux error precedence, CPU/node/mm-CID publication, matching unregister, and lifecycle cleanup; migration and critical-section abort semantics remain pending. Copy-on-write, larger process tables, and general Linux scheduling semantics remain incomplete. |
| Signals | Partial | Selected standard signal, mask, alternate-stack, queued self-delivery with preserved `sigval`, delivery, and return behavior; no complete real-time queueing, cross-process delivery, or restart/nesting semantics. |
| VFS and path lookup | Partial | Immutable OCI lower root with a demand-allocated, per-domain RAM overlay, directories, symlinks, FIFO metadata, cwd, tmpfs, devfs, procfs subsets, and frame-chained extended attributes. Overlay file and xattr data grow by allocating frames rather than stopping at a fixed aggregate cache ceiling; open-unlinked objects live through their descriptors, directory renames include descendants, and permission/ownership operations cover the package-manager path. Generation-stable OCI and overlay file handles support size discovery, mount IDs, rename-stable reopening, and `O_PATH`; procfs and other virtual filesystems do not export handles. `cachestat` reports RAM-resident pages for regular OCI/overlay files. `memfd_create` provides elastic anonymous files and `MFD_CLOEXEC`; sealing operations remain pending. `memfd_secret` provides anonymous shared mappings that reject kernel user-copy and descriptor I/O, but direct-map removal, coherent repeated mappings, demand allocation, locked-memory accounting, and fork inheritance remain pending. `mknod`/`mknodat` create regular files and FIFOs; opening named FIFOs and creating socket nodes are pending, while device nodes are denied by domain policy. This is not yet a complete Linux VFS. |
| Filesystem notifications | Partial | `inotify_init[1]`, one instance with up to 16 watches per domain, watch replacement/addition, removal with `IN_IGNORED`, descriptor duplication, readiness, event reads, and coalesced `IN_MODIFY` events for overlay writes are supported. Bounded `fanotify_init`/`fanotify_mark` groups deliver Linux v3 metadata and an owned event descriptor for marked-file writes. Multiple fanotify groups, directory/mount/filesystem marks, permission events, child names, blocking reads, and the remaining mutation event classes are pending. |
| `/proc` | Partial | Automatically mounted per domain with domain-local PID directories, `status`, `stat`, `statm`, `mountinfo`, namespace maps, `cpuinfo`, `meminfo`, `stat`, `uptime`, `loadavg`, `filesystems`, and `cmdline`. Broader Linux procfs coverage and unconstrained process enumeration remain incomplete. |
| IPv4 sockets | Partial | Domain descriptors expose repeated client TCP and UDP connections plus TCP bind/listen/accept over the shared host NIC with IOMMU-confined virtio queues; AF_UNIX stream socket pairs cover local runtime coordination. Ancillary data, IPv6, multi-entry listen backlogs, and complete option/error semantics remain pending. |
| OCI overlay root | Partial | Verified layers are assembled in manifest order with replacements and OCI whiteouts. Broader overlay conformance remains under test. |
| Runtime `mmap` | Partial | Page-table-backed anonymous and OCI file mappings, fixed placement, in-place growth/shrink and movable anonymous `mremap`, protection changes, unmap, `madvise`, and `mincore` support Alpine and the validated Rocky glibc/coreutils path. SysV shared-memory pages remain shared across process hierarchy cloning. NUMA policy calls expose the current single node, validate node-0 masks, and make node-0 placement/migration operations explicit no-ops. Process snapshot metadata is still bounded and COW, general file-backed `MAP_SHARED`, `MREMAP_DONTUNMAP`, movable file mappings, multi-node NUMA policy, and full Linux semantics remain incomplete. |
| `/dev` and terminal ABI | Partial | Selected console/device nodes and bounded termios/winsize ioctls. |
| `/sys` | Missing | No Linux-compatible sysfs contract. |
| Credentials and user namespaces | Partial | Container-scoped identity and hard privilege boundary; many mutation calls are policy-denied. |
| Mount and namespace APIs | Partial | Container roots are kernel-created; `listmount` and `statmount` enumerate and describe the domain root and automatic proc mount. General mount, chroot, pivot, unshare, and setns are policy-denied. |
| Socket API | Partial | Linux TCP/UDP sockets use the kernel network stack and survive the connection churn of OCI and DNF downloads; TCP server sockets and local `socketpair` are supported. IPv6, ancillary data, interruptible socket waits, and the full option surface remain pending. |
| cgroups | Missing | No Linux cgroup filesystem or ABI. |
| seccomp, audit, LSM ABI | Partial | Strict seccomp mode and the Linux LSM query API are implemented; BPF seccomp filters, audit, and active Linux security modules remain missing. |
| vDSO/vvar | Missing | Time calls currently cross the syscall boundary. |
| Linux module userspace ABI | Policy-denied | Module management is owned by the trusted control plane. |
| ioctl families | Partial | Only explicitly admitted console, pipe, and device requests; ioctl is a family of ABIs, not one completed interface. |

The current POSIX timer profile provides one monotonic `SIGEV_NONE` timer per
domain, including relative/absolute arm, query, overrun, and
delete operations. Signal/thread notification, realtime clocks, multiple timer
objects, and overrun accumulation remain pending.

System V shared memory now uses domain-owned, dynamically allocated frame
objects whose lifetime is independent of any one mapping. `shmget`, `shmat`,
`shmdt`, and `shmctl(IPC_STAT/IPC_RMID)` provide keyed creation, shared page
mapping, attachment reference counts, and deferred destruction after
`IPC_RMID`. `IPC_SET` updates the permission mode; fork preserves shared page
identity and inherits attachment accounting; process/domain teardown releases
the corresponding references and objects. `SHM_REMAP`, timestamps, owner/group
updates, permission enforcement, and Linux's
information commands remain pending, so these calls remain Partial.

System V semaphores use dynamically allocated, per-domain sets and chained
value pages. `semget`, `semop`, `semtimedop`, and the principal `semctl`
commands support keyed creation, atomic multi-operation preflight and commit,
mode/stat queries, individual and whole-set values, and immediate removal.
The Linux default `SEMMSL=32000` and `SEMOPM=500` limits are enforced.
Operations that would sleep currently return `EAGAIN`; waiter scheduling,
`SEM_UNDO`, timestamps, permission enforcement, and information commands are
pending, so the family remains Partial.

System V message queues use dynamically allocated, per-domain queue and
payload objects. `msgget`, `msgsnd`, `msgrcv`, and
`msgctl(IPC_STAT/IPC_SET/IPC_RMID)` support keyed creation, ordered messages,
Linux type selection, `MSG_EXCEPT`, nondestructive `MSG_COPY`, and
`MSG_NOERROR` truncation. Queue byte limits are configurable and payloads are
not constrained to a static aggregate cache. Blocking wait/wakeup, timestamps,
credential enforcement, and Linux's information commands remain pending, so
the family remains Partial.

POSIX message queues use named per-domain objects backed by the same dynamic
page allocator. `mq_open`, `mq_unlink`, `mq_timedsend`, `mq_timedreceive`,
`mq_notify`, and `mq_getsetattr` cover create/exclusive lookup, descriptor
duplication and close, unlink lifetime, priority ordering, attributes,
`O_NONBLOCK`, and `SIGEV_NONE`. Blocking deadline waits and signal/thread
notification delivery remain pending, so this family remains Partial.

Legacy Linux AIO uses dynamically allocated per-domain contexts and completion
records. `io_setup`, `io_submit`, `io_getevents`, `io_pgetevents`,
`io_cancel`, and `io_destroy` support synchronous `PREAD`, `PWRITE`, `FSYNC`,
and `FDSYNC` submission with Linux `iocb`/`io_event` layouts and an mmap-backed
ring header. True asynchronous execution, blocking waits, cancellation of
in-flight work, vectored requests, poll, and eventfd completion remain pending.

## Syscall index

The table includes every native `common` or `64` entry in Linux v6.12's
x86-64 syscall table. Number gaps reserved by Linux are not syscalls and are
therefore omitted.

| Number | Syscall | Status |
|---:|---|---|
| 0 | read | Partial |
| 1 | write | Partial |
| 2 | open | Partial |
| 3 | close | Partial |
| 4 | stat | Partial |
| 5 | fstat | Partial |
| 6 | lstat | Partial |
| 7 | poll | Partial |
| 8 | lseek | Partial |
| 9 | mmap | Partial |
| 10 | mprotect | Partial |
| 11 | munmap | Partial |
| 12 | brk | Partial |
| 13 | rt_sigaction | Partial |
| 14 | rt_sigprocmask | Partial |
| 15 | rt_sigreturn | Partial |
| 16 | ioctl | Partial |
| 17 | pread64 | Partial |
| 18 | pwrite64 | Partial |
| 19 | readv | Partial |
| 20 | writev | Partial |
| 21 | access | Partial |
| 22 | pipe | Partial |
| 23 | select | Partial |
| 24 | sched_yield | Complete |
| 25 | mremap | Partial |
| 26 | msync | Partial |
| 27 | mincore | Partial |
| 28 | madvise | Partial |
| 29 | shmget | Partial |
| 30 | shmat | Partial |
| 31 | shmctl | Partial |
| 32 | dup | Partial |
| 33 | dup2 | Partial |
| 34 | pause | Partial |
| 35 | nanosleep | Partial |
| 36 | getitimer | Partial |
| 37 | alarm | Partial |
| 38 | setitimer | Partial |
| 39 | getpid | Complete |
| 40 | sendfile | Partial |
| 41 | socket | Partial |
| 42 | connect | Partial |
| 43 | accept | Partial |
| 44 | sendto | Partial |
| 45 | recvfrom | Partial |
| 46 | sendmsg | Partial |
| 47 | recvmsg | Partial |
| 48 | shutdown | Partial |
| 49 | bind | Partial |
| 50 | listen | Partial |
| 51 | getsockname | Partial |
| 52 | getpeername | Partial |
| 53 | socketpair | Partial |
| 54 | setsockopt | Partial |
| 55 | getsockopt | Partial |
| 56 | clone | Partial |
| 57 | fork | Partial |
| 58 | vfork | Partial |
| 59 | execve | Partial |
| 60 | exit | Partial |
| 61 | wait4 | Partial |
| 62 | kill | Partial |
| 63 | uname | Complete |
| 64 | semget | Partial |
| 65 | semop | Partial |
| 66 | semctl | Partial |
| 67 | shmdt | Partial |
| 68 | msgget | Partial |
| 69 | msgsnd | Partial |
| 70 | msgrcv | Partial |
| 71 | msgctl | Partial |
| 72 | fcntl | Partial |
| 73 | flock | Partial |
| 74 | fsync | Partial |
| 75 | fdatasync | Partial |
| 76 | truncate | Partial |
| 77 | ftruncate | Partial |
| 78 | getdents | Partial |
| 79 | getcwd | Partial |
| 80 | chdir | Partial |
| 81 | fchdir | Partial |
| 82 | rename | Partial |
| 83 | mkdir | Partial |
| 84 | rmdir | Partial |
| 85 | creat | Partial |
| 86 | link | Partial |
| 87 | unlink | Partial |
| 88 | symlink | Partial |
| 89 | readlink | Partial |
| 90 | chmod | Partial |
| 91 | fchmod | Partial |
| 92 | chown | Partial |
| 93 | fchown | Partial |
| 94 | lchown | Partial |
| 95 | umask | Complete |
| 96 | gettimeofday | Complete |
| 97 | getrlimit | Partial |
| 98 | getrusage | Partial |
| 99 | sysinfo | Partial |
| 100 | times | Partial |
| 101 | ptrace | Policy-denied |
| 102 | getuid | Complete |
| 103 | syslog | Policy-denied |
| 104 | getgid | Complete |
| 105 | setuid | Policy-denied |
| 106 | setgid | Policy-denied |
| 107 | geteuid | Complete |
| 108 | getegid | Complete |
| 109 | setpgid | Partial |
| 110 | getppid | Complete |
| 111 | getpgrp | Complete |
| 112 | setsid | Partial |
| 113 | setreuid | Policy-denied |
| 114 | setregid | Policy-denied |
| 115 | getgroups | Complete |
| 116 | setgroups | Partial |
| 117 | setresuid | Policy-denied |
| 118 | getresuid | Complete |
| 119 | setresgid | Policy-denied |
| 120 | getresgid | Complete |
| 121 | getpgid | Complete |
| 122 | setfsuid | Policy-denied |
| 123 | setfsgid | Policy-denied |
| 124 | getsid | Complete |
| 125 | capget | Partial |
| 126 | capset | Partial |
| 127 | rt_sigpending | Partial |
| 128 | rt_sigtimedwait | Partial |
| 129 | rt_sigqueueinfo | Partial |
| 130 | rt_sigsuspend | Partial |
| 131 | sigaltstack | Partial |
| 132 | utime | Partial |
| 133 | mknod | Partial |
| 134 | uselib | Complete |
| 135 | personality | Partial |
| 136 | ustat | Complete |
| 137 | statfs | Partial |
| 138 | fstatfs | Partial |
| 139 | sysfs | Complete |
| 140 | getpriority | Partial |
| 141 | setpriority | Partial |
| 142 | sched_setparam | Partial |
| 143 | sched_getparam | Complete |
| 144 | sched_setscheduler | Partial |
| 145 | sched_getscheduler | Complete |
| 146 | sched_get_priority_max | Complete |
| 147 | sched_get_priority_min | Complete |
| 148 | sched_rr_get_interval | Complete |
| 149 | mlock | Complete |
| 150 | munlock | Complete |
| 151 | mlockall | Complete |
| 152 | munlockall | Complete |
| 153 | vhangup | Policy-denied |
| 154 | modify_ldt | Partial |
| 155 | pivot_root | Policy-denied |
| 156 | _sysctl | Complete |
| 157 | prctl | Partial |
| 158 | arch_prctl | Partial |
| 159 | adjtimex | Policy-denied |
| 160 | setrlimit | Partial |
| 161 | chroot | Policy-denied |
| 162 | sync | Complete |
| 163 | acct | Policy-denied |
| 164 | settimeofday | Policy-denied |
| 165 | mount | Policy-denied |
| 166 | umount2 | Policy-denied |
| 167 | swapon | Policy-denied |
| 168 | swapoff | Policy-denied |
| 169 | reboot | Policy-denied |
| 170 | sethostname | Policy-denied |
| 171 | setdomainname | Policy-denied |
| 172 | iopl | Policy-denied |
| 173 | ioperm | Policy-denied |
| 174 | create_module | Complete |
| 175 | init_module | Policy-denied |
| 176 | delete_module | Policy-denied |
| 177 | get_kernel_syms | Complete |
| 178 | query_module | Complete |
| 179 | quotactl | Policy-denied |
| 180 | nfsservctl | Complete |
| 181 | getpmsg | Complete |
| 182 | putpmsg | Complete |
| 183 | afs_syscall | Complete |
| 184 | tuxcall | Complete |
| 185 | security | Complete |
| 186 | gettid | Complete |
| 187 | readahead | Complete |
| 188 | setxattr | Partial |
| 189 | lsetxattr | Partial |
| 190 | fsetxattr | Partial |
| 191 | getxattr | Partial |
| 192 | lgetxattr | Partial |
| 193 | fgetxattr | Partial |
| 194 | listxattr | Partial |
| 195 | llistxattr | Partial |
| 196 | flistxattr | Partial |
| 197 | removexattr | Partial |
| 198 | lremovexattr | Partial |
| 199 | fremovexattr | Partial |
| 200 | tkill | Partial |
| 201 | time | Complete |
| 202 | futex | Partial |
| 203 | sched_setaffinity | Complete |
| 204 | sched_getaffinity | Complete |
| 205 | set_thread_area | Complete |
| 206 | io_setup | Partial |
| 207 | io_destroy | Partial |
| 208 | io_getevents | Partial |
| 209 | io_submit | Partial |
| 210 | io_cancel | Partial |
| 211 | get_thread_area | Complete |
| 212 | lookup_dcookie | Complete |
| 213 | epoll_create | Partial |
| 214 | epoll_ctl_old | Complete |
| 215 | epoll_wait_old | Complete |
| 216 | remap_file_pages | Partial |
| 217 | getdents64 | Partial |
| 218 | set_tid_address | Complete |
| 219 | restart_syscall | Partial |
| 220 | semtimedop | Partial |
| 221 | fadvise64 | Complete |
| 222 | timer_create | Partial |
| 223 | timer_settime | Partial |
| 224 | timer_gettime | Partial |
| 225 | timer_getoverrun | Partial |
| 226 | timer_delete | Partial |
| 227 | clock_settime | Policy-denied |
| 228 | clock_gettime | Partial |
| 229 | clock_getres | Partial |
| 230 | clock_nanosleep | Partial |
| 231 | exit_group | Partial |
| 232 | epoll_wait | Partial |
| 233 | epoll_ctl | Partial |
| 234 | tgkill | Partial |
| 235 | utimes | Partial |
| 236 | vserver | Complete |
| 237 | mbind | Partial |
| 238 | set_mempolicy | Partial |
| 239 | get_mempolicy | Partial |
| 240 | mq_open | Partial |
| 241 | mq_unlink | Partial |
| 242 | mq_timedsend | Partial |
| 243 | mq_timedreceive | Partial |
| 244 | mq_notify | Partial |
| 245 | mq_getsetattr | Partial |
| 246 | kexec_load | Policy-denied |
| 247 | waitid | Partial |
| 248 | add_key | Partial |
| 249 | request_key | Partial |
| 250 | keyctl | Partial |
| 251 | ioprio_set | Partial |
| 252 | ioprio_get | Partial |
| 253 | inotify_init | Partial |
| 254 | inotify_add_watch | Partial |
| 255 | inotify_rm_watch | Partial |
| 256 | migrate_pages | Partial |
| 257 | openat | Partial |
| 258 | mkdirat | Partial |
| 259 | mknodat | Partial |
| 260 | fchownat | Partial |
| 261 | futimesat | Partial |
| 262 | newfstatat | Partial |
| 263 | unlinkat | Partial |
| 264 | renameat | Partial |
| 265 | linkat | Partial |
| 266 | symlinkat | Partial |
| 267 | readlinkat | Partial |
| 268 | fchmodat | Partial |
| 269 | faccessat | Partial |
| 270 | pselect6 | Partial |
| 271 | ppoll | Partial |
| 272 | unshare | Policy-denied |
| 273 | set_robust_list | Partial |
| 274 | get_robust_list | Complete |
| 275 | splice | Partial |
| 276 | tee | Partial |
| 277 | sync_file_range | Partial |
| 278 | vmsplice | Partial |
| 279 | move_pages | Partial |
| 280 | utimensat | Partial |
| 281 | epoll_pwait | Partial |
| 282 | signalfd | Partial |
| 283 | timerfd_create | Partial |
| 284 | eventfd | Partial |
| 285 | fallocate | Partial |
| 286 | timerfd_settime | Partial |
| 287 | timerfd_gettime | Partial |
| 288 | accept4 | Partial |
| 289 | signalfd4 | Partial |
| 290 | eventfd2 | Partial |
| 291 | epoll_create1 | Partial |
| 292 | dup3 | Partial |
| 293 | pipe2 | Partial |
| 294 | inotify_init1 | Partial |
| 295 | preadv | Partial |
| 296 | pwritev | Partial |
| 297 | rt_tgsigqueueinfo | Partial |
| 298 | perf_event_open | Policy-denied |
| 299 | recvmmsg | Partial |
| 300 | fanotify_init | Partial |
| 301 | fanotify_mark | Partial |
| 302 | prlimit64 | Partial |
| 303 | name_to_handle_at | Partial |
| 304 | open_by_handle_at | Partial |
| 305 | clock_adjtime | Policy-denied |
| 306 | syncfs | Complete |
| 307 | sendmmsg | Partial |
| 308 | setns | Policy-denied |
| 309 | getcpu | Complete |
| 310 | process_vm_readv | Partial |
| 311 | process_vm_writev | Partial |
| 312 | kcmp | Partial |
| 313 | finit_module | Policy-denied |
| 314 | sched_setattr | Partial |
| 315 | sched_getattr | Partial |
| 316 | renameat2 | Partial |
| 317 | seccomp | Partial |
| 318 | getrandom | Partial |
| 319 | memfd_create | Partial |
| 320 | kexec_file_load | Policy-denied |
| 321 | bpf | Policy-denied |
| 322 | execveat | Partial |
| 323 | userfaultfd | Partial |
| 324 | membarrier | Complete |
| 325 | mlock2 | Complete |
| 326 | copy_file_range | Partial |
| 327 | preadv2 | Partial |
| 328 | pwritev2 | Partial |
| 329 | pkey_mprotect | Partial |
| 330 | pkey_alloc | Partial |
| 331 | pkey_free | Partial |
| 332 | statx | Partial |
| 333 | io_pgetevents | Partial |
| 334 | rseq | Partial |
| 335 | uretprobe | Partial |
| 424 | pidfd_send_signal | Partial |
| 425 | io_uring_setup | Partial |
| 426 | io_uring_enter | Partial |
| 427 | io_uring_register | Partial |
| 428 | open_tree | Policy-denied |
| 429 | move_mount | Policy-denied |
| 430 | fsopen | Policy-denied |
| 431 | fsconfig | Policy-denied |
| 432 | fsmount | Policy-denied |
| 433 | fspick | Policy-denied |
| 434 | pidfd_open | Partial |
| 435 | clone3 | Partial |
| 436 | close_range | Partial |
| 437 | openat2 | Partial |
| 438 | pidfd_getfd | Partial |
| 439 | faccessat2 | Partial |
| 440 | process_madvise | Partial |
| 441 | epoll_pwait2 | Partial |
| 442 | mount_setattr | Policy-denied |
| 443 | quotactl_fd | Policy-denied |
| 444 | landlock_create_ruleset | Policy-denied |
| 445 | landlock_add_rule | Policy-denied |
| 446 | landlock_restrict_self | Policy-denied |
| 447 | memfd_secret | Partial |
| 448 | process_mrelease | Partial |
| 449 | futex_waitv | Partial |
| 450 | set_mempolicy_home_node | Partial |
| 451 | cachestat | Partial |
| 452 | fchmodat2 | Partial |
| 453 | map_shadow_stack | Partial |
| 454 | futex_wake | Partial |
| 455 | futex_wait | Partial |
| 456 | futex_requeue | Partial |
| 457 | statmount | Partial |
| 458 | listmount | Partial |
| 459 | lsm_get_self_attr | Partial |
| 460 | lsm_set_self_attr | Partial |
| 461 | lsm_list_modules | Partial |
| 462 | mseal | Partial |

## Completion requirements

A syscall may move from Partial to Complete only when its supported flags,
structures, success behavior, Linux error precedence, blocking/interruption
behavior, concurrency, and filesystem or descriptor side effects are covered by
tests. libc startup success alone is not conformance.

The practical compatibility gate remains workload-driven:

1. Trace the next target workload on Linux.
2. Add every observed syscall and non-syscall ABI dependency to this ledger.
3. Implement the behavior against the pinned sources.
4. Add focused kernel tests and a real OCI userspace regression.
5. Change status only after those tests prove the documented boundary.

The controller exposes the complete compiled ledger through `test linux all`;
see [Linux Userspace Interface Test v0](../specs/kernel/linux-abi-test-v0.md).
`test linux` prints its summary, while `interfaces`, `syscalls`, and `syscall
NAME|NUMBER` select detailed results. The command name is intentionally
environment-specific so future compatibility profiles, such as BSD, can have
independent tests and contracts.

For grouped interfaces, PASS reports that the current bounded Ginux profile was
verified during boot; it does not upgrade that interface to complete Linux
compatibility. For individual syscalls, only a complete runtime conformance
probe may print PASS. Partial dispatch prints UNVERIFIED, deliberate authority
policy prints POLICY-DENIED, and absent dispatch prints UNSUPPORTED. The runtime
catalog is generated from every syscall row in this file so documentation and
the executable checklist cannot silently diverge.
