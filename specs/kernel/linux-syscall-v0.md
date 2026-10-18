# Linux x86-64 Syscall Entry v0

**Status:** Bounded error, I/O, identity, TLS, and termination transitions
verified in QEMU; not a general syscall ABI.

The bounded static ELF proof invokes the architectural x86-64 `SYSCALL`
instruction with Linux register conventions. It submits unsupported call
999 and requires Linux `-ENOSYS` (-38) after resumption. It then uses `rax = 1`
(`__NR_write`) with fd 1, the declared RW segment address, and an eight-byte
length. The kernel accepts 1 through 256 bytes, validates the descriptor and
complete readable VMA range, enables supervisor access only around the bounded
copy-in, emits the exact bytes to the pinned console, and returns the requested
length. A zero-length request or one above the limit returns `-EINVAL`.
The socket profile proves AF_INET TCP creation, ephemeral bind, listen,
`getsockname`, and Linux argument/error precedence. `SO_REUSEADDR`,
`SO_KEEPALIVE`, `IP_RECVERR`, and `TCP_NODELAY` are mutable per-socket boolean
options shared by duplicated descriptors; accepted TCP sockets inherit their
listener's settings. Keepalive and no-delay also configure the underlying TCP
engine. The booted proof sets and reads back each option, while short or
unreadable values and unknown options produce `EINVAL`, `EFAULT`, and
`ENOPROTOOPT` respectively. `SO_TYPE`, `SO_PROTOCOL`, `SO_DOMAIN`, and
`SO_ACCEPTCONN` report live socket state, including the distinct UDP type and
protocol. AF_UNIX `sendmsg` accepts one `SCM_RIGHTS` socket descriptor per
message, retains the underlying open-file reference even if the sender closes
its descriptor, and transfers the retained status flags through `recvmsg`.
`MSG_CMSG_CLOEXEC` marks the received descriptor close-on-exec. A short or
absent receive control buffer consumes the right and reports `MSG_CTRUNC`;
consuming the associated byte through `read` or `recvfrom` discards the right
rather than leaking it into a later message. Malformed control headers and
invalid passed descriptors fail before payload
delivery. The booted proof passes a UDP socket, closes the sender descriptor,
reads the received socket type, verifies close-on-exec, and separately verifies
control truncation. This bounded first profile admits the control record only
when the peer's receive queue is empty. Multiple rights, non-socket
descriptors, `SCM_CREDENTIALS`, and more than one queued ancillary record
remain unsupported. Connected AF_INET datagram and AF_UNIX stream sockets
expose their
peer names through `getpeername`, including Linux address truncation and actual
length reporting, 32-bit descriptor narrowing, `ENOTCONN`, `ENOTSOCK`, `EBADF`,
and `EFAULT` paths. It also proves bidirectional AF_UNIX `socketpair` transport
through `sendto`/`recvfrom`, `sendmsg`/`recvmsg`, and
`sendmmsg`/`recvmmsg`, plus local shutdown, Linux `SIGPIPE`/`MSG_NOSIGNAL`
behavior, nondestructive `MSG_PEEK` reads across local, TCP, and UDP
transports, stream `MSG_WAITALL` accumulation, and descriptor cleanup. The
booted proof submits stream data in separate sends, peeks through both
`recvfrom` and `recvmsg` with `MSG_WAITALL`, verifies the exact combined bytes,
and then consumes the same bytes with a wait-all receive. A stream EOF ends a
wait-all receive with the accumulated byte count. UDP receives copy a short
caller-buffer prefix instead of converting truncation into `EAGAIN`;
`MSG_TRUNC` returns the full datagram length, and `recvmsg` publishes
`MSG_TRUNC` in `msg_flags`. A booted
DNS exchange, when the domain link is up, proves peek-plus-truncation,
subsequent consumption, and the `recvmsg` output flag against the external UDP
transport; a link-down boot skips only that external assertion without a
blocking timeout. Unsupported
families, types, protocols, flags, options, pointers, and oversized message
vectors are rejected with their documented Linux errno. Successful TCP accept,
IPv6, the remaining ancillary-data surface, interruptible blocking socket
waits, and the broader option surface remain Partial and are not promoted by
this proof.
The Linux v6.12 x86-64 table deliberately maps `uselib`, `_sysctl`,
`create_module`, `get_kernel_syms`, `query_module`, `nfsservctl`, `getpmsg`,
`putpmsg`, `afs_syscall`, `tuxcall`, `security`, `set_thread_area`,
`get_thread_area`, `lookup_dcookie`, `epoll_ctl_old`, `epoll_wait_old`, and
`vserver` to the not-implemented entry. Ginux dispatches each of those numbers
explicitly to `-ENOSYS`, independently of its unknown-syscall fallback. The
measured ELF probe calls every entry and requires `-ENOSYS`; Linux's
not-implemented entry does not inspect the argument registers.
Host-global kernel logs, wall-clock adjustment, process accounting, terminal
revocation, and storage-quota administration are outside a domain's authority.
`syslog`, `adjtimex`, `settimeofday`, `clock_settime`, `clock_adjtime`, `acct`,
`vhangup`, `quotactl`, and `quotactl_fd` therefore enter the ordinary
user-namespace privilege gate and return `-EPERM` without touching user memory
or host state. These are explicit security-policy denials, not missing
dispatch.
`rax = 0` (`__NR_read`) exposes descriptor 0 as a currently disconnected
console-input stream. It returns zero for EOF without reading or validating the
caller buffer; output, closed, and unknown descriptors return `-EBADF`. This is
an explicit bootstrap state until an authorized input service and wait path
exist, not an interactive console implementation.
The interpreted proof also admits syscall instruction pointers and copy-in
buffers from the parsed interpreter's RX extent. Its linked interpreter uses
an RX-resident path to inspect `/init`, then builds
`/lib64/<DT_NEEDED-name>` on its mapped stack and inspects the selected
dependency. The name copy is limited to 13 bytes including NUL; an unknown,
empty, or unterminated dependency cannot open. Return RIP outside either the
main or interpreter RX extent still terminates the bounded workload, and
copy-out never accepts an RX page.
`rax = 63`
(`__NR_uname`) and the exact RW segment base request
a fixed 390-byte Linux `utsname`. The kernel zero-initializes every byte, fills
all six 65-byte fields, and enables supervisor access to user memory only for
the copy-out, which is subject to the same declared-extent check. The probe
checks success and the `Ginux` system name.
`rax = 20` (`__NR_writev`) accepts one through four 16-byte x86-64 iovec
records for descriptor 1, with a nonzero aggregate of at most 256 bytes. The
kernel copies the complete vector table and every referenced range into a
bounded staging buffer before emitting anything, so an invalid later vector
returns `-EFAULT` without exposing an earlier prefix. Invalid descriptors return
`-EBADF`; zero-length vectors, excessive vector counts, and excessive aggregate
lengths return `-EINVAL`.
`rax = 7` (`__NR_poll`) accepts zero through four x86-64 `pollfd` records and
atomically replaces every `revents` field after validating the complete table.
The disconnected input console reports `POLLHUP`, writable console or device
endpoints report requested `POLLOUT`, readable devices report requested
`POLLIN`, unknown nonnegative descriptors report `POLLNVAL`, and negative
descriptors are ignored. An invalid table returns `-EFAULT`, and more than four
records returns `-EINVAL`. A zero timeout performs an immediate query; any
ready descriptor also returns immediately regardless of timeout. With one
through four descriptors, no immediate readiness, and another runnable thread,
a positive signed 32-bit millisecond timeout or negative infinite timeout
blocks the caller in its generation-tagged scheduler slot. The wait snapshots
only each descriptor number and requested-event word; descriptor state remains
live. Eventfd writes, pipe reads and writes, endpoint closure, and periodic
timerfd readiness checks recompute all returned-event words and make the waiter
ready when at least one is nonzero. A positive deadline instead atomically
copies zero `revents` words, returns zero, and can preempt the running peer
directly from the local-APIC interrupt. The complete output table is
revalidated on completion; a revoked output range resumes with `-EFAULT`.
Delivery of a caught signal that is unblocked by the caller's ordinary signal
mask cancels the wait, returns `-EINTR`, and leaves the output table unchanged.
With zero descriptors, the table pointer is ignored: a zero timeout returns
immediately, while a positive or infinite timeout uses the same scheduler path
as a sleep. Blocking with no runnable peer retains the explicit `-ENOSYS`
boundary. This is a two-slot, fixed-snapshot proof rather than a general
descriptor wait queue. Blocked-signal delivery, nested delivery, and syscall
restart remain outside this profile.
`rax = 271` (`__NR_ppoll`) uses the same bounded readiness engine. A null
timeout means an infinite wait request; otherwise the complete 16-byte x86-64
`timespec` must contain nonnegative seconds and normalized nanoseconds. An
optional eight-byte signal mask requires the exact signal-set size and is
sanitized to keep `SIGKILL` and `SIGSTOP` unblocked. Zero timeouts and
immediately ready descriptors behave like `poll`. With one through four
descriptors, no readiness, and another runnable thread, the caller blocks with
the temporary mask installed in its per-thread state. Readiness or an
APIC-driven deadline restores the prior mask before making the caller runnable;
failed scheduler handoff rolls the mask and wait record back together. The
measured graph proves an eventfd wake, a positive timeout, zeroed timeout
`revents`, a zero-descriptor timeout with its table pointer ignored, and
restoration of the caller's original mask. Blocking without a runnable peer
returns `-ENOSYS`. Invalid timespecs or mask sizes return `-EINVAL`, invalid
nonempty input or output ranges return `-EFAULT`, and the timeout is not
rewritten. Delivery of a caught signal that the
temporary mask leaves unblocked cancels the wait, restores the caller's prior
mask, returns `-EINTR`, and leaves the output table unchanged. Blocked-signal
delivery, nested delivery, remaining-time reporting, and syscall restart
remain outside this profile.
`rax = 291` (`__NR_epoll_create1`) allocates one process-local epoll instance
in the lowest free auxiliary descriptor, 2 through 5. It accepts only zero or
`EPOLL_CLOEXEC`; a second epoll instance or exhausted auxiliary table returns
`-EMFILE`.
`rax = 233` (`__NR_epoll_ctl`) retains one registration for an open console or
device descriptor. `EPOLL_CTL_ADD`, `EPOLL_CTL_MOD`, and `EPOLL_CTL_DEL` have
their Linux duplicate, missing-registration, invalid-descriptor, and
self-registration errors. ADD and MOD accept only level-triggered `EPOLLIN`
and `EPOLLOUT`, copy the complete packed 12-byte x86-64 `epoll_event` before
changing state, and preserve its 64-bit data word. DEL ignores its event
pointer. `rax = 232` (`__NR_epoll_wait`) accepts exactly one output event and
atomically copies it when the registered descriptor is ready. A zero-timeout
query with no event returns zero. With another runnable thread, a positive
signed 32-bit millisecond timeout or negative infinite timeout blocks the
caller in its generation-tagged scheduler slot. The registration, requested
events, 64-bit data word, and descriptor readiness remain live while blocked.
Eventfd and pipe state changes, endpoint closure, timerfd arming or expiration,
and epoll ADD or MOD operations recompute readiness and wake the waiter. A
positive deadline returns zero without modifying the output event and can
preempt the running peer directly from the local-APIC interrupt. The complete
12-byte output is validated before blocking and revalidated on completion; a
revoked output range resumes with `-EFAULT`. Closing the waited-on epoll
descriptor wakes the waiter with `-EBADF`. Blocking without a runnable peer
retains the explicit `-ENOSYS` boundary. The epoll descriptor participates in
`F_GETFD`, `F_SETFD`, `F_GETFL`, `poll`, and `close`; closing it clears the
registration and releases its descriptor. Delivery of a caught signal that is
unblocked by the caller's ordinary signal mask cancels the wait, returns
`-EINTR`, and leaves the output event unchanged. Blocked-signal delivery,
nested delivery, and syscall restart remain outside this profile.
`rax = 281` (`__NR_epoll_pwait`) uses the same bounded wait operation after
validating an optional eight-byte signal mask and its exact signal-set size.
The mask is sanitized to keep `SIGKILL` and `SIGSTOP` unblocked, installed in
the caller's per-thread state only while blocked, and restored before readiness
or timeout makes the caller runnable. Scheduler-handoff rollback restores the
mask and wait record together. Invalid sizes return `-EINVAL` and invalid mask
ranges return `-EFAULT` before examining readiness. An immediately ready event
or zero timeout behaves like `epoll_wait`; otherwise the same runnable-peer and
output-validation requirements apply. The measured graph proves signal
interruption, a positive APIC-driven timeout, unchanged output, and restoration
of the original mask.
`rax = 441` (`__NR_epoll_pwait2`) replaces the integer timeout with a null or
complete 16-byte x86-64 `timespec`. Negative seconds and non-normalized
nanoseconds return `-EINVAL`; checked conversion overflow also returns
`-EINVAL`, and invalid memory returns `-EFAULT`. A zero timespec queries
immediately, while a nonzero or null timeout uses the same scheduler-backed
masked wait as `epoll_pwait`. For both masked entry points, delivery of a
caught signal that the temporary mask leaves unblocked cancels the wait,
restores the caller's prior mask, returns `-EINTR`, and leaves the output event
unchanged. Blocked-signal delivery, nested delivery, remaining-time reporting,
and syscall restart remain unsupported.
This is a bounded event-notification proof, not a general epoll interest tree.
The scheduler-backed path has two fixed thread slots and one live registration.
`rax = 284` (`__NR_eventfd`) and `rax = 290` (`__NR_eventfd2`) allocate one
process-local event counter in the lowest free auxiliary descriptor. The
initial value is truncated to Linux's unsigned 32-bit input. `eventfd2`
accepts `EFD_CLOEXEC`, `EFD_NONBLOCK`, and `EFD_SEMAPHORE`; other flags return
`-EINVAL`, a second counter or full table returns `-EMFILE`, and `fcntl`
exposes its descriptor and nonblocking flags. Reads and writes require exactly
one eight-byte word and validate the complete user range before changing the
counter. A normal read returns and clears the complete nonzero counter;
semaphore mode returns one and decrements it. Writing adds any value except
`UINT64_MAX`. An empty nonblocking read or addition that would exceed
`UINT64_MAX - 1` returns `-EAGAIN`. A blocking saturated write retains the
already-copied increment in a typed scheduler wait while a peer is runnable.
A read that creates enough capacity wakes it and atomically commits the saved
value; later userspace input mutation cannot redirect the addition. Closing the
descriptor wakes it with `-EBADF`. An admitted caught signal may cancel the
wait with `-EINTR` without changing the counter, and retry may still complete.
Blocking without a runnable peer retains the explicit `-ENOSYS` boundary.
Invalid lengths return
`-EINVAL` and invalid memory returns `-EFAULT` without consuming or adding a
value. `poll` and epoll report `POLLIN`/`EPOLLIN` while nonzero and
`POLLOUT`/`EPOLLOUT` while another value can be added. The proof registers the
counter in the simultaneous epoll descriptor, observes readiness appear after
a write, preserves readiness across a failed read, drains it, verifies
full-counter backpressure, and closes both descriptors. With another runnable
thread, a blocking read on an empty eventfd suspends in its generation-tagged
scheduler slot. A write recomputes descriptor readiness, atomically copies and
consumes the counter on behalf of the waiter, and makes that thread ready;
closing the eventfd instead resumes it with `-EBADF`. The eight-byte output is
validated before blocking and revalidated on completion, so a revoked range
resumes with `-EFAULT` without consuming the counter. Blocking without a
runnable peer retains the explicit `-ENOSYS` boundary. The measured two-thread
graph also proves saturated-write interruption, saved-value isolation, read
wake, and descriptor-close wake.
`rax = 283` (`__NR_timerfd_create`) allocates one process-local
`CLOCK_MONOTONIC` timer in the lowest free auxiliary descriptor. It accepts
`TFD_CLOEXEC` and `TFD_NONBLOCK`; unsupported clocks or flags return `-EINVAL`,
and a second timer or full table returns `-EMFILE`. `rax = 286`
(`__NR_timerfd_settime`) accepts a complete 32-byte x86-64 `itimerspec`,
normalized nonnegative times, and relative or `TFD_TIMER_ABSTIME` arming.
Zero initial expiration disarms the timer. The optional old value is validated
and copied before state changes. `rax = 287` (`__NR_timerfd_gettime`) reports
the interval and time until the next expiration. Reads require exactly one
eight-byte result, preserve pending expiration state on `-EFAULT`, return the
number of accumulated periodic expirations, disarm a consumed one-shot, and
advance a periodic deadline. A disarmed or unexpired nonblocking read returns
`-EAGAIN`. A corresponding blocking read with a runnable peer stores its exact
eight-byte destination, descriptor, and current deadline in a typed scheduler
wait. `timerfd_settime` refreshes that deadline after an arm or disarm; descriptor
closure wakes the read with `-EBADF`. Once the monotonic deadline elapses, the
periodic APIC handler revalidates the output, consumes the expiration count, and
preempts directly back to the reader. Blocking without a runnable peer returns
`-ENOSYS`. An admitted caught standard signal unblocked by the waiting thread's
ordinary mask cancels the wait through the shared transactional handoff and
returns `-EINTR` without changing the output or timer state. The measured graph
retries against the same deadline and completes through the APIC path. Blocked
signals, nested delivery, and syscall restart remain unsupported. `poll` and epoll expose
`POLLIN`/`EPOLLIN` dynamically once the monotonic deadline has elapsed. The
descriptor also participates in
`F_GETFD`, `F_SETFD`, `F_GETFL`, `lseek`, and `close`. The proof creates timerfd
and epoll together, observes one-shot readiness, verifies failed-read
non-consumption, consumes periodic and already-expired absolute timers, and
releases both descriptors.
`fstat` on an open epoll, eventfd, or timerfd descriptor reports the shared
bounded anonymous-inode identity with mode `0600` and no `S_IF*` file-type
bits. This matches Linux's observable anonymous-inode convention while making
no claim of a general inode namespace. Closed descriptors still return
`-EBADF`, and complete output-range validation remains atomic.
`rax = 22` (`__NR_pipe`) and `rax = 293` (`__NR_pipe2`) allocate one
process-local unidirectional pipe using the two lowest free auxiliary
descriptors and atomically copy the read and write descriptor numbers to an
eight-byte user array. The pipe has a fixed 256-byte FIFO. `pipe2` accepts
`O_CLOEXEC` and `O_NONBLOCK`; unsupported flags return `-EINVAL`, invalid
output memory returns `-EFAULT` without allocating, and an existing pipe or
fewer than two free slots returns `-EMFILE`. Writes of one through 256 bytes
are all-or-nothing: invalid input leaves the FIFO unchanged, insufficient
capacity returns `-EAGAIN` in nonblocking mode. A blocking scalar or vectored
write with a runnable peer stores its bounded request in a typed scheduler
wait. `writev` retains a kernel-owned snapshot of up to four input ranges; a
userspace change to the iovec table while blocked cannot redirect completion.
Every snapshotted range is revalidated and all input is gathered before the
FIFO changes. Copy and enqueue occur atomically only when the complete request
fits. A reader that frees insufficient capacity leaves the writer asleep;
closing the reader wakes it with `-EPIPE`. An admitted caught signal may cancel
either wait with `-EINTR` without changing the input or FIFO, and retry may
complete normally. Blocking without a runnable peer retains `-ENOSYS`. Larger
writes return `-EINVAL`. One proof write spans the final
byte of the linked main's RX area and the first byte of its adjacent RW area;
the pipe is closed and recreated before the FIFO lifecycle continues. Reads
return up to the requested
count and consume only after successful copy-out. An empty live pipe follows
the nonblocking `-EAGAIN` rule. A blocking scalar read with a runnable peer
stores its destination, bounded count, and descriptor in a typed scheduler wait.
A write revalidates the output and completes the read, while closing the writer
wakes it with EOF without modifying the destination. The measured two-thread
graph proves both paths and retires both endpoints before continuing. Blocking
without a runnable peer retains `-ENOSYS`. An
admitted caught standard signal unblocked by the waiting thread's ordinary mask
may cancel a scalar read through the shared transactional handoff. It returns
`-EINTR` through `rt_sigreturn` without modifying the destination or FIFO; the
measured graph retries that read, consumes the sibling's byte, and then proves
writer-close EOF. Blocked signals, nested delivery, and syscall restart remain
unsupported. Closing the writer makes any later empty read return EOF and
reports `POLLHUP`, while buffered bytes remain readable.
Closing the reader makes writes return `-EPIPE` and reports `POLLERR`; `SIGPIPE`
is generated with `SI_KERNEL` before a scalar or vectored write returns. An
ignored disposition leaves the `-EPIPE` result unchanged. A caught disposition
commits the ordinary signal frame and restores `-EPIPE` through
`rt_sigreturn`; when the final reader close completes an already-blocked write,
the frame is installed against that writer's saved thread context before it
resumes. A blocked `SIGPIPE` is coalesced into the pending standard-signal set
while the write still completes with `-EPIPE`. The default disposition closes
the bounded process resources, retires every Linux thread, records termination
by signal 13, and returns control to the kernel. The dedicated measured signal
runtime proves that terminal path after its other signal checks. The endpoints participate in `F_GETFD`,
`F_SETFD`, `F_GETFL`, `lseek`, `poll`, epoll, and `close`. The ring-3 proof
checks FIFO ordering across partial reads, failed-read non-consumption, full
buffer backpressure, epoll readiness, EOF, broken-peer handling, and legacy
`pipe` creation.
`fstat` reports either pipe endpoint as a fixed `S_IFIFO | 0600` object with a
4096-byte preferred block size. `ioctl(FIONREAD)` copies the current 32-bit
FIFO byte count for either endpoint; invalid output memory returns `-EFAULT`
without changing the pipe, and other pipe ioctls return `-ENOTTY`. Other open
anonymous descriptors now also return `-ENOTTY`, rather than `-EBADF`, for an
unsupported ioctl. This metadata is process-local compatibility state, not a
general inode or ioctl implementation.
`readv` and `writev` extend the same pipe semantics to one through four
nonempty iovecs with an aggregate maximum of 256 bytes. `writev` gathers and
validates every input range before appending any byte; `readv` validates every
output range before scattering or consuming data. The proof injects an invalid
later vector in each direction, then verifies that the complete original FIFO
sequence remains available to a valid retry. On an empty blocking pipe,
`readv` snapshots the complete validated iovec table into the waiting thread's
kernel state before selecting its peer. Completion uses only that snapshot,
revalidates every output range before copying or consuming, and clears the
snapshot on wake, close, rollback, signal interruption, or thread retirement.
The measured graph changes the shared userspace table while the reader sleeps
and proves that the original one-byte target still receives the writer's byte.
`rax = 78` (`__NR_getdents`) enumerates the same admitted per-domain
directories as `getdents64` using the native x86-64 legacy `linux_dirent`
layout: 64-bit inode and offset fields, a 16-bit record length, the terminated
name at offset 18, and `d_type` in the final record byte. Records remain
eight-byte aligned, descriptor offsets advance only after a successful complete
copy, and the proof covers a full `/dev` enumeration, EOF, rewind, names, and
record typing. Directory coverage remains bounded by the current VFS profile.
`rax = 79` (`__NR_getcwd`) copies the process's indexed working-directory path
and terminator to a writable VMA and returns the required byte count. The
initial directory is `/`; measured directories such as `/var/lib` retain their
published root-index identity. Output is bounded to 64 bytes. An undersized
buffer returns `-ERANGE`, and an invalid destination returns `-EFAULT`.
`rax = 80` (`__NR_chdir`) accepts `/`, `/dev`, or an indexed measured
directory. Absolute names resolve from the container root; relative names
resolve beneath the current measured directory. Input is bounded to 31 bytes,
and empty, `.`, `..`, empty-component, missing, and non-directory paths are
rejected without changing the working directory.
`rax = 81` (`__NR_fchdir`) selects the indexed directory held by an open
bounded descriptor. A closed or unknown descriptor returns `-EBADF`, and a
regular-file descriptor returns `-ENOTDIR`. Closing the descriptor afterward
does not change the retained working-directory identity. These calls provide
bounded immutable-root directory state, not a mutable or general VFS.
`rax = 89` (`__NR_readlink`) recognizes exactly `/proc/self/exe`, copied as a
bounded NUL-terminated pathname. It returns `/init` without a terminator before
replacement, remains `/init` after interpreted self-replacement, and reports
`/bin/signal-probe` or `/bin/exec-second` after a static `execve` commit. Each
rejected exec leaves the original identity visible. A zero output size returns
`-EINVAL`; invalid pathname or output ranges return `-EFAULT`. Other paths
return `-ENOENT`. This is not a general procfs.
`rax = 267` (`__NR_readlinkat`) exposes the same exact link when the directory
descriptor is `AT_FDCWD`. Its pathname, output, length, and errors use the same
bounded implementation. Other directory descriptors return `-EBADF`; this is
not a general directory-descriptor or `*at` interface.
`rax = 95` (`__NR_umask`) atomically replaces the single process's permission
mask, returns the previous value, and ignores bits outside `0777`. The initial
value is `022`; the proof changes it twice and restores it before exit.
`rax = 5` (`__NR_fstat`) reports descriptors 0, 1, and an open bounded null device as fixed character devices
with mode `S_IFCHR | 0600`, one link, and a 4096-byte preferred block size. The
144-byte x86-64 `stat` result is zero-initialized and copied only after its full
destination range is validated. Unknown descriptors return `-EBADF`; an invalid
range returns `-EFAULT` without changing its valid prefix. This is descriptor
metadata for the bounded console profile, not a general inode or VFS query.
`rax = 16` (`__NR_ioctl`) accepts `TCGETS` and `TIOCGWINSZ` on descriptors 0 and
1. `TCGETS` copies a zero-initialized 36-byte x86-64 Linux `termios`, explicitly
describing the current console as having no kernel line discipline. `TIOCGWINSZ`
copies an eight-byte Linux `winsize` reporting 24 rows and 80 columns, with
pixel dimensions zero. Unknown descriptors return `-EBADF`, unsupported
requests return `-ENOTTY`, and invalid destinations return `-EFAULT`. This is a
fixed early-console contract, not mutable terminal state or a general ioctl
mechanism.
`rax = 8` (`__NR_lseek`) reports `-ESPIPE` for either open console endpoint,
zero for the null device, and `-EBADF` for a closed or unknown descriptor.
The bounded console is a stream and never claims a mutable file position.
`rax = 3` (`__NR_close`) irreversibly clears descriptor 0 or 1 from the bounded
process-local console table or closes an auxiliary descriptor. Closing an unknown or already closed descriptor
returns `-EBADF`; subsequent `fstat` and `ioctl` calls observe the closure. No
file, pipe, or socket allocation, inheritance, or general table exists yet.
`rax = 32` (`__NR_dup`) copies an open console descriptor into the lowest of
the same two slots, preserving whether it is the input or output endpoint. An
invalid source returns `-EBADF`; no free slot returns `-EMFILE`. The probe
closes fd 0, duplicates fd 1 into it, writes through the new fd 0, and closes
both slots. There is still no general allocation or inherited table.
`rax = 33` (`__NR_dup2`) atomically replaces either bounded target slot with an
open source's endpoint type. Equal source and target descriptors return the
target unchanged. A closed source or a descriptor outside the two-slot limit
returns `-EBADF`.
`rax = 72` (`__NR_fcntl`) supports `F_DUPFD`, `F_DUPFD_CLOEXEC`, `F_GETFD`,
`F_SETFD`, and `F_GETFL`. The duplication commands copy an open endpoint into
the lowest available slot at or above their nonnegative minimum; a minimum
outside the two-slot table returns `-EINVAL`, and exhaustion returns `-EMFILE`.
`F_DUPFD_CLOEXEC` sets the new slot's `FD_CLOEXEC` bit, while `F_DUPFD`, `dup`,
and a replacing `dup2` clear it. `F_SETFD` retains only `FD_CLOEXEC`; `F_GETFD`
reports it. The current profile has no second `exec` operation yet, so the flag
is process-local observable state rather than an exercised inheritance action.
`F_GETFL` reports `O_RDONLY` for the input endpoint and `O_WRONLY` for output or
duplicated-output endpoints. Closed or unknown descriptors return `-EBADF`;
other mutation and duplication commands return `-EINVAL`. On the null
descriptor, `F_GETFD`, `F_SETFD`, and `F_GETFL` expose its close-on-exec bit and
selected access mode; duplication remains outside this bounded node profile.
`rax = 436` (`__NR_close_range`) closes every open descriptor in an inclusive
range while ignoring holes, or marks each open descriptor with
`CLOSE_RANGE_CLOEXEC`. `CLOSE_RANGE_UNSHARE` is accepted independently or with
`CLOEXEC`; Ginux process descriptor tables are already private, so no table
copy is necessary. Reversed ranges and unknown flags return `-EINVAL`; ranges
wholly beyond the table are successful no-ops. Arguments use their Linux
32-bit widths. Runtime probes cover closure, close-on-exec marking, unsharing,
combined flags, high register bits, reversed ranges, unknown flags, and holes.
`F_SETFL` can toggle only `O_NONBLOCK` on eventfd, timerfd, and each pipe
endpoint; it ignores the supplied access-mode bits and rejects other status
flags with `-EINVAL`. Pipe read and write endpoints retain independent status
flags. The proof clears nonblocking mode on each descriptor class, observes
the explicit `-ENOSYS` would-block boundary, restores it, and verifies
`-EAGAIN`; changing the pipe reader leaves its writer nonblocking.
`rax = 257` (`__NR_openat`) recognizes only the absolute paths `/dev/null`,
`/dev/zero`, `/dev/console`, and `/dev/tty`, and
allocates the lowest free auxiliary descriptor, 2 through 5. It accepts `O_RDONLY`, `O_WRONLY`, or
`O_RDWR`, optionally with `O_CLOEXEC`; other paths return `-ENOENT`, unsupported
flags return `-EINVAL`, bad path memory returns `-EFAULT`, and an open with all
four auxiliary slots occupied returns `-EMFILE`. `/dev/null` reads return EOF and writes discard their requested byte
count without touching the caller buffer. This is a fixed device namespace,
not directory traversal or a general VFS. `/dev/zero` reads up to 256 validated
bytes as zeros and discards writes when its selected access mode permits them.
`/dev/console` reopens the bounded serial stream with the same write limit,
EOF input, terminal ioctls, and non-seekable behavior as the inherited console.
`/dev/tty` is a strict alias of that endpoint; controlling-terminal ownership,
session attachment, and terminal reassignment remain unsupported.
`rax = 19` (`__NR_readv`) accepts one to four nonempty vectors totaling at most
256 bytes. It returns EOF for readable console and null endpoints. For
`/dev/zero`, every destination is validated before any is cleared, so an
invalid later vector returns `-EFAULT` without partially changing earlier
buffers; a valid request returns the total zeroed length.
`writev` applies the same four-vector and 256-byte aggregate limits to the
fixed devices. Console vectors use the atomic serial-copy path; null and zero
devices validate the vector table and lengths, then discard the total without
touching payload memory, matching their scalar-write contract.
`openat` also opens immutable `/dev` in the bounded auxiliary table, optionally requiring
`O_DIRECTORY`; writable access returns `-EISDIR`, while `O_DIRECTORY` on a
device node returns `-ENOTDIR`. `rax = 217` (`__NR_getdents64`) atomically
returns six aligned records for `.`, `..`, `null`, `zero`, `console`, and `tty`
in a 152-byte-or-larger buffer. An undersized buffer returns `-EINVAL`, invalid
memory returns `-EFAULT` without advancing, the next call returns EOF, and
`lseek(fd, 0, SEEK_SET)` resets the bounded cursor. `fstat`, `newfstatat`,
`statx`, and access checks report `/dev` as immutable mode `0555` directory
metadata. No other directory or mutable namespace exists.
`rax = 332` (`__NR_statx`) returns a zero-initialized 256-byte result for these
four paths, including `STATX_BASIC_STATS`, character-device mode `0600`, stable
inode numbers, block size 4096, and conventional device minors. The reserved
mask bit or nonzero flags return `-EINVAL`; unknown paths and invalid memory use
the same `-ENOENT` and `-EFAULT` boundaries as the older metadata call.
`rax = 262` (`__NR_newfstatat`) returns the same fixed metadata for a recognized
device, directory, or immutable runtime file without opening it. Only zero
flags are accepted; bad path or output memory returns
`-EFAULT`, unsupported flags return `-EINVAL`, and other paths return `-ENOENT`.
`access`, `faccessat`, and `faccessat2` report read/write access for either fixed node
and return `-EACCES` for execute access. `faccessat2` accepts only zero flags;
unknown mode bits or flags return `-EINVAL`, with the same path errors as
`openat`.

`rax = 139` (`__NR_sysfs`) exposes the four filesystem types implemented by
the current Linux personality in stable registration order: `rootfs`, `proc`,
`tmpfs`, and `devtmpfs`. Operation 1 maps an exact NUL-terminated name to its
zero-based index, operation 2 copies the indexed name including its terminator,
and operation 3 returns the count. Unknown names, out-of-range indexes, and
unknown operations return `-EINVAL`; inaccessible name or output memory returns
`-EFAULT`. This legacy syscall enumerates filesystem types and does not imply a
mounted Linux-compatible `/sys` filesystem.

`rax = 136` (`__NR_ustat`) reports the free-block and free-inode counts for the
initial namespace's root filesystem device `0:1`, using the native x86-64
32-byte `ustat` layout and zeroed legacy name fields. An unknown device returns
`-EINVAL`, and inaccessible output memory returns `-EFAULT` without a partial
copy.

The immutable runtime namespace also recognizes `/init`,
`/lib64/ld-ginux.so`, `/lib64/libginux.so`, `/lib64/libginux-aux.so`,
`/etc/hostname`, `/etc/os-release`, and `/etc/motd`. These resolve through the retained, independently measured
initramfs index. The first four are aliases for internal `linux/*` entries that
must byte-match the build-authenticated ELF artifacts used by the bounded
execution plan; they are no longer served directly from the kernel's embedded
fixture slices. `/etc/hostname` resolves to exact `ginux\n` bytes and
`/etc/os-release` to exact `NAME=Ginux\nID=ginux\n` bytes; `/etc/motd` contains
exact `Welcome to Ginux\n` bytes. Each descriptor
stores a bounded one-based measured-root node selector separately from its
file role; reads and metadata recover the indexed immutable bytes instead of
dispatching content through a per-file kernel case. `openat`
accepts only `O_RDONLY` with optional
`O_CLOEXEC`; writable access returns `-EACCES` and `O_DIRECTORY` returns
`-ENOTDIR`. Each open has an independent 64-bit cursor. `read` and bounded
`readv` advance it, `pread64` leaves it unchanged, and `lseek` implements
`SEEK_SET`, `SEEK_CUR`, and `SEEK_END` with checked nonnegative results.
`fstat`, `newfstatat`, and `statx` report immutable regular mode `0555`, stable
inodes, and the exact artifact length; access checks deny writes. The ring-3
proof reads ELF magic from the main and auxiliary dependency, performs a
positioned read of the interpreter entry,
seeks and reads it again, verifies metadata, rejects writes and writable opens,
maps the interpreter through the bounded read-only file-mapping profile below,
and closes the descriptor. Before that executable discovery, the interpreter
opens `/etc/hostname`, validates all six bytes, mode `0555`, size 6, and
`SEEK_END`, then requires a write to return `-EBADF` and a writable open to
return `-EACCES`. It also opens `os-release` relative to `/etc`, validates all
20 bytes, mode `0555`, size 20, and writable-open denial through the generic
indexed-file role. These measured-root files are not executable or mappable.
The proof also resolves `motd` relative to the open `/etc` descriptor and
validates immutable mode `0555` and size 17 without a kernel filename case.

The same profile opens `/etc` read-only with `O_DIRECTORY`. Its `getdents64`
result is staged atomically and contains `.`, `..`, `hostname`, `os-release`,
and `motd` regular files derived from the retained measured-root index. A second call returns EOF;
`lseek(fd, 0, SEEK_SET)` rewinds the cursor. A buffer below the first complete
24-byte record returns `-EINVAL` without advancing that cursor; larger bounded
buffers return every whole record that fits and retain the next cursor. `fstat`,
`newfstatat`, and `statx` expose immutable directory mode `0555`; writable
opens return `-EISDIR`, and `O_DIRECTORY` against `/etc/hostname` returns
`-ENOTDIR`.

Opening `/` with `O_DIRECTORY` exposes `.`, `..`, `dev`, `etc`, `proc`, `sys`,
`tmp`, `var`, `init`, `bin`, and `sbin`, with all non-dot entries obtained from
the measured-root index in archive order. Each result page is staged in a fixed
256-byte kernel buffer before copy-out. The first page contains the dots and
children through `bin`; the retained opaque cursor returns `sbin` on a second
page. A buffer smaller than the complete next staged page returns `-EINVAL`
without advancing. Measured directory descriptors retain a bounded
one-based index selector and recover their path from the same immutable
64-node index. The current 23-entry image explicitly crosses the former
16-node publication limit.
The measured `/proc`, `/sys`, `/tmp`, and `/var` nodes open without individual
pathname cases. The ring-3 proof enumerates the otherwise unlisted `/var`,
opens its `lib` child relative to that descriptor, and rejects `../lib` before
lookup. `/dev` remains the separately populated
synthetic device directory. Directory descriptor metadata and pathname
metadata use stable profile-local inode numbers. There is no `/lib64` directory
enumeration. Relative lookup constructs a bounded path from an open measured
directory and a normalized relative name, then consults the retained index;
`AT_FDCWD` uses the same lookup. Empty, `.`, and `..` components are rejected.
`openat`, `newfstatat`, `faccessat`, `faccessat2`, and `statx` share the result.
There is no symlink traversal, mount crossing, or component-by-component VFS
walk. A closed or
unknown base returns `-EBADF`, a regular-file base returns `-ENOTDIR`, and other
relative names return `-ENOENT`. This is not general component traversal, a
general VFS, or general userspace executable lookup; the kernel authenticates
and plans the fixed interpreter, main, and two-DSO executable graph. The kernel
installs the interpreter needed to bootstrap ring 3; that interpreter installs
the main and both dependencies' admitted segments through the fixed mapping
contract below.

The measured `/proc` directory is the immutable mount-point anchor for a
synthetic bounded process view. Before fork, its `getdents64` result contains
`.`, `..`, and the directory `1`; `/proc/1` likewise contains `.`, `..`, and the read-only
regular files `status`, `mountinfo`, `uid_map`, `gid_map`, and `setgroups`.
`/proc/1/status` has fixed mode `0444` and exact bytes
`Name:\tinit\nState:\tR (running)\nPid:\t1\nPPid:\t0\n`. PID 1 enumerates both
directories, validates their child records, reads all 45 bytes, and copies the
snapshot to the measured console before readiness. `/proc/1/mountinfo` has
fixed mode `0444` and exposes the exact 51-byte record
`1 0 0:1 / / ro,nosuid,nodev - ginux-root rootfs ro\n` while the namespace is
active. The three user-namespace records have the exact contents specified in
[Linux User Namespace v0](linux-user-namespace-v0.md). Writes, execute access,
and file mapping are denied for all five records. This is a boot-state identity,
mount, and namespace snapshot, not general procfs: no other
While the fork child is a zombie, `/proc` additionally contains directory `2`.
`/proc/2` contains one read-only `status` file whose exact 44 bytes report
`init`, `Z (zombie)`, PID 2, and PPID 1. The directory and file resolve only
while the generation-bound process record exists; successful `wait4` removes
that record and subsequent lookup returns `-ENOENT`. PID 1 proves appearance,
content, and removal around the real fork/wait lifecycle. This is not general
procfs: arbitrary process counts and metadata, threads, fd links, sysctl, and
retained post-reap records remain absent.

The measured `/tmp` directory anchors one synthetic mutable regular file,
`/tmp/ginux-ready`. The directory begins empty. Absolute `openat` with
`O_CREAT | O_EXCL` creates the one admitted file with mode `0600`, zero length,
and a new nonzero generation.
Descriptors retain that generation and have independent offsets. Scalar
`read`, `write`, and `lseek` use the existing checked offset rules; writes are
fully staged before committing any byte and backing frames are allocated on
demand.
`getdents64` reflects creation and removal. `unlinkat` accepts the closed file,
scrubs all backing bytes, and makes later lookup return `-ENOENT`; an open file
is rejected with `-EBUSY`. Final process teardown performs the same scrub if
the file remains. The in-kernel contract also creates a replacement generation
and rejects the retired handle. The backing RAM store allocates owned pages on
demand and separately proves multiple named objects, cross-page I/O, hidden
staging, atomic publish, growth beyond the former 32 KiB aggregate ceiling,
abort, frame scrubbing, and complete lease recovery; those objects are not
exposed in this Linux view. `readv`, `writev`, file mapping, truncation,
rename, subdirectories, relative creation/unlink, additional files, and
unlink-while-open Linux semantics remain outside this legacy synthetic-file
profile. General runtime writes use the demand-allocated per-domain overlay.

`rax = 59` (`__NR_execve`) provides bounded replacement transitions for the
single-threaded proof process. Absolute and current-directory-relative
normalized paths resolve through the retained measured-root index. The selected
node must be an executable regular file and must match the admitted static ELF
profile. The independently built `bin/signal-probe` and `bin/exec-second`
fixtures both execute, proving selection is based on the measured bytes and
profile rather than equality with one kernel-compiled artifact or one virtual
base. Both fixtures use bounded four-page layouts containing one header page,
two executable pages, and one writable page; the second uses an independent
virtual base. `argv`
admits one through eight pointers and `envp` zero
through eight, each terminated by a null pointer. Each string is limited to 63
bytes plus its terminator, and the complete rebuilt initial stack is limited to
512 bytes. The proof supplies
`["/bin/signal-probe", "signal-probe", NULL]` and
`["GINUX_EXEC=1", NULL]`, while also rejecting an empty argument vector and a
ninth argument. Unknown paths return `-ENOENT`, non-regular or non-executable
nodes return `-EACCES`, executable files outside the admitted ELF profile return
`-ENOEXEC`, inaccessible input returns `-EFAULT`, and count, string, or
aggregate-stack overflow returns `-E2BIG`
without changing the running image. A multithreaded caller returns `-EAGAIN`.
On success the kernel clears the clear-child-TID and robust-list registrations,
constructs a fresh bounded initial stack, allocates and materializes the
selected measured ELF pages, derives their entry and mapping addresses without
a predeclared per-target template, atomically replaces the authenticated image
mappings, and never returns to the old program. Allocation or pre-commit
failure scrubs and releases all new generation-tagged frame leases before
returning. The process owns a bounded active-image record containing each old
address, exact frame generation, and current W^X permissions. RELRO sealing
updates that record with the leaf. Commit accepts only matching live leaves,
replaces them with the new plan, and scrubs and releases every exact old lease
before publishing success. Failed mapping verification preserves the old
leaves and ownership; a stale generation and a second release are rejected
without mutation.
Descriptors marked
`FD_CLOEXEC` close only after the mapping commit, while ordinary descriptors
survive. Caught signal dispositions reset to default, ignored dispositions
remain ignored, the alternate signal stack becomes disabled, the signal mask
and `no_new_privs` remain unchanged, and the process name becomes the selected
node's bounded basename. The canonical measured-root node is retained as
process state, so `/proc/self/exe` reports `/bin/signal-probe` or
`/bin/exec-second` independently of the accepted pathname form. Each
replacement validates these transitions, reopens `/dev/console`, validates its
complete initial stack, including target-specific program-header, entry,
executable-name, platform, credential, clock, and hardware-capability auxiliary
vector entries, emits its distinct proof marker, closes
all surviving proof descriptors, and exits with status 42.

The same syscall admits one interpreted target: Linux-visible `/init`, backed
by measured-root `linux/init`. It accepts one through eight arguments and zero
through eight environment entries under the same 63-byte per-string and
512-byte aggregate-stack limits as static replacement. The replacement proof
passes `/init` plus `dynamic-exec` and the two entries `GINUX=1` and
`GINUX_MODE=exec` through the interpreter to the main and both DSO
constructors. It authenticates the fixed interpreter/main/two-DSO graph,
atomically installs fresh interpreter pages, and retires the complete previous
graph with exact generation-tagged leases. It then resets the main and DSO
mapping contracts so the replacement interpreter can install the authenticated
main and dependencies again. Both static and interpreted exec pre-materialize
the complete bounded stack in a fresh zeroed RW/NX frame and replace the old
primary-stack leaf in the rollback-capable image transaction.
The replacement main validates every ordered auxiliary-vector pair, including
its dynamic program headers, entry, interpreter base, executable name,
platform, credentials, clock tick, conservative hardware masks, and terminator.
Commit requires
the old stack's exact live generation and permissions, then
scrubs and releases that lease; teardown reclaims the replacement stack after
the process root is inactive. The same commit installs a fresh zero-filled
RW/NX heap frame, retires the old exact lease, and resets the process break to
the heap base. Static exec applies the same fresh-heap transaction. The
replacement heap is reclaimed only after the process root is inactive. Fresh primary and sibling TLS frames are fully materialized from the
authenticated templates before commit and replace both exact old leases in the
same transaction. The initial FS base is restored against the fresh primary
mapping. The guarded sibling stack is replaced by a fresh zeroed RW/NX lease in
the same transaction; both adjacent guard leaves stay unmapped, the old exact
lease is retired at commit, and the replacement is reclaimed after the process
root is inactive. Ignored signal dispositions are preserved from the observed
pre-exec set, caught dispositions are cleared, and RELRO is applied again.
The replacement main emits the kernel-observed `DYNEXEC` completion by exiting
with status 42. Other dynamic targets, general dependency resolution, and
shebangs are outside this version. Both exec profiles replace the complete process
page-table hierarchy: static exec constructs the new image in an inactive
five-frame clone and interpreted exec uses a six-frame clone. Each rejects
forced verification failure and stale root generations without changing live
CR3, activates the verified clone, and retires the old exact leases. Both exec
profiles replace the inactive
anonymous-mapping backing frames with fresh zeroed generation-bound leases
after the image commit; the old exact leases are scrubbed and retired, and
teardown reclaims the replacements.

`rax = 322` (`__NR_execveat`) enters the same transaction and accepts flags zero
or `AT_EMPTY_PATH`. An absolute path ignores `dirfd`; a relative path resolves beneath
`AT_FDCWD` or an open measured-root directory descriptor. Unknown descriptors
return `-EBADF`, non-directory descriptors return `-ENOTDIR`, and other flag bits
return `-EINVAL` before image state changes. With `AT_EMPTY_PATH`, an empty
pathname selects an already-open executable measured-root regular file;
directories and non-executable objects return `-EACCES`. The proof closes an
ordinary descriptor, reopens `/bin` in that slot, restores cwd to `/`, and
validates directory-relative resolution. It then rejects empty-path execution
of that directory, reopens the selected measured executable in the same slot,
and executes it through the descriptor while retaining canonical
`/proc/self/exe` identity.
`AT_SYMLINK_NOFOLLOW` behavior remains unsupported.

`rax = 97` (`__NR_getrlimit`) and `rax = 302` (`__NR_prlimit64`) expose immutable
limits for `RLIMIT_STACK` and `RLIMIT_NOFILE`. The probe retains its one-page
stack and bounded descriptor profile. Runtime domains report an unlimited
stack because their initially mapped page grows downward on demand, and report
the runtime descriptor-table limit.
`prlimit64` accepts PID 0 or 1 and an optional old-limit destination. Other
PIDs return `-ESRCH`, unsupported resources return `-EINVAL`, and any non-null
new-limit pointer returns `-EPERM`. Output is copied only after the full
16-byte `rlimit` range passes writable-VMA validation, so a range crossing the
declared ELF extent returns `-EFAULT` without a partial write. The syscall
entry uses an internal value outside Linux's `-1..-4095` errno range to return
to the kernel, keeping Linux `-EPERM` observable in ring 3.
`rax = 204` (`__NR_sched_getaffinity`) accepts any live process visible in the
domain, requires a nonzero, eight-byte-aligned caller size large enough for the
single active CPU, writes the one-bit CPU 0 mask, and returns 8. An unknown or
negative PID returns `-ESRCH`, an invalid size returns `-EINVAL` before process
lookup, and an invalid output range returns `-EFAULT` without a partial write.
`sched_setaffinity` copies up to the kernel's eight-byte CPU mask before process
lookup, accepts every caller size whose supplied mask includes active CPU 0,
and ignores unavailable higher CPU bits as Linux does. A mask excluding CPU 0
returns `-EINVAL`; invalid input returns `-EFAULT`, including before an unknown
PID is reported. Both calls preserve Linux's 32-bit `cpusetsize` argument
conversion. `rax = 309`
(`__NR_getcpu`) reports CPU 0 and NUMA node 0 through independently optional
four-byte pointers. Both destinations are validated before either is written;
the obsolete cache pointer is ignored as on Linux. These results describe the
single-bootstrap-processor profile and do not claim SMP scheduling support.
`rax = 24` (`__NR_sched_yield`) selects the sole ready sibling when one exists
and otherwise succeeds as a no-op. The runtime proof covers both paths and
repeated handoffs across sibling retirement and slot reuse. `sched_getscheduler` reports
`SCHED_OTHER` for any live process visible in the domain, and `sched_getparam`
copies its priority of zero. Both implement Linux's negative-PID, null-pointer,
retired-PID, and output-fault error precedence. `sched_get_priority_min` and
`sched_get_priority_max` report 1 and 99 for `SCHED_FIFO`/`SCHED_RR`, and zero
for `SCHED_OTHER`, `SCHED_BATCH`, `SCHED_IDLE`, and `SCHED_DEADLINE`; unknown
policies return `-EINVAL`. Unknown PIDs return `-ESRCH`, and an invalid output
returns `-EFAULT`. `sched_setparam` and `sched_setscheduler` accept only an
idempotent priority-zero `SCHED_OTHER` request; invalid pointers return
`-EFAULT`, and other policies return `-EPERM`. `rax = 140`
(`__NR_getpriority`) returns Linux's raw `20 - nice` value for the
highest-priority process selected by `PRIO_PROCESS`, `PRIO_PGRP`, or
`PRIO_USER`. A zero selector means the calling process, process group, or real
UID respectively. Nice state is per process, is preserved across context
switches, and is inherited by fork. `setpriority` stores values in Linux's
`-20..=19` range, silently clamps requests outside that range, applies
group/user selections only after permission validation, and enforces Linux
ownership and privilege rules. Unknown selectors return `-EINVAL`; an empty
selection returns `-ESRCH`. The booted proof covers all three selector classes,
signed range clamping, 32-bit argument narrowing, and invalid/empty selectors.
The fork proof also covers inheritance and a child-local priority change.
Competing live multi-process selector results and non-root permission paths
remain to be exercised before these calls are classified Complete.
`rax = 251` (`__NR_ioprio_set`) and `rax = 252` (`__NR_ioprio_get`) maintain
per-process Linux I/O-priority class/data values. Process, process-group, and
real-UID selectors share the same visibility rules as `getpriority`; zero
selects the caller's corresponding identity. The default `IOPRIO_CLASS_NONE`
value is zero. `IOPRIO_CLASS_RT` and `IOPRIO_CLASS_BE` accept data values
0 through 7, `IOPRIO_CLASS_IDLE` is admitted, invalid class/data encodings
return `-EINVAL`, and an empty selection returns `-ESRCH`. Group and user reads
return the best explicit class/data value, ignoring default members when an
explicit member exists. Updates validate the complete selection before
mutation, enforce ownership and real-time-class privilege, persist across
process switches, and are inherited by fork. The booted proof covers the three
selector classes, default and explicit values, invalid encodings, empty
selection, Linux 32-bit argument narrowing, reset, and fork inheritance.
Non-root permission paths, competing simultaneously live aggregate members,
and influence on physical I/O scheduling remain pending.
`rax = 228` (`__NR_clock_gettime`) exposes `CLOCK_REALTIME`,
`CLOCK_MONOTONIC`, and `CLOCK_MONOTONIC_RAW` from an ordered
bootstrap-processor TSC. The conversion
uses a CPUID-reported frequency when available and the PIT-calibrated fallback
otherwise. Both clocks share a private process epoch, return a normalized
16-byte x86-64 `timespec`, and cannot move backward during this single-CPU
proof. Realtime adds elapsed TSC time since the validated UEFI wall-clock
handoff. Other clock IDs return `-EINVAL`; an invalid output range returns
`-EFAULT` without changing its valid prefix. Clock adjustment and a vDSO
remain unsupported.
`rax = 96` (`__NR_gettimeofday`) exposes the same realtime source as a
normalized 16-byte x86-64 `timeval`. A non-null legacy timezone pointer receives
two zero 32-bit fields. Null outputs are accepted. Linux's ordered `put_user`
behavior is preserved: seconds precede microseconds, and the timeval precedes
the timezone, so a later `-EFAULT` retains any earlier completed field writes.
`rax = 201` (`__NR_time`) returns that source's whole realtime seconds and
optionally stores the same 64-bit `time_t`. A bad non-null output returns
`-EFAULT`.
`rax = 229` (`__NR_clock_getres`) exposes the corresponding single-tick
resolution, rounded up to a normalized `timespec`. A null result pointer is a
valid support query. Clock selection and copy errors match `clock_gettime`, and
the complete result is validated before copy-out.
`rax = 35` (`__NR_nanosleep`) accepts one normalized relative timespec. A zero
duration returns immediately. A positive duration requires the peer thread to
be ready, converts the duration to a checked monotonic deadline, stores that
deadline in a typed sleep wait, and selects the peer. The first Linux syscall or
scheduling boundary at or after expiry can clear the wait. Independently, a
periodic local-APIC tick clears an expired wait in interrupt context and marks
the sleeper ready with zero. It saves the interrupted peer's complete integer,
instruction, stack, flags, and FS-base context and dispatches the sleeper before
returning to ring 3. The optional remainder range is prevalidated but remains untouched
on successful expiry. An admitted caught standard signal that is unblocked by
the waiting thread's ordinary mask cancels the wait and returns `-EINTR`
through `rt_sigreturn`. Before the signal frame commits, a non-null remainder
receives the current deadline minus monotonic time as a normalized relative
timespec. The remainder, signal frame, saved contexts, wait record, thread
states, and live return form one rollback transaction; failure restores the
prior user bytes and blocked wait. The measured two-thread graph interrupts a
five-second sleep with targeted `SIGUSR2` and validates the remainder. Invalid
request or remainder ranges return `-EFAULT`, invalid fields or deadline
overflow return `-EINVAL`, and a positive sleep with no runnable peer returns
`-ENOSYS`. Blocked signals, nested delivery, and syscall restart remain
unsupported.
`rax = 230` (`__NR_clock_nanosleep`) accepts `CLOCK_MONOTONIC` with either zero
flags or `TIMER_ABSTIME`. A relative request uses the same checked deadline,
scheduler handoff, APIC expiry, signal interruption, and optional remainder
transaction as `nanosleep`. An absolute request names the private monotonic
epoch directly; an elapsed deadline returns immediately, while a future
deadline uses the same sleep wait. Linux ignores the remainder argument for an
absolute request, including on signal interruption, so Ginux neither validates
nor writes it. Other clocks or flag bits return `-EINVAL`. The measured graph
proves a relative APIC expiry and an already elapsed absolute request with an
invalid ignored remainder pointer. Realtime clocks remain unsupported.
The same two calls accept `CLOCK_PROCESS_CPUTIME_ID` and
`CLOCK_THREAD_CPUTIME_ID`. In this proof the process has one thread, never
deschedules, and owns the processor from its private epoch, so both clocks use
the same elapsed counter. `rax = 100` (`__NR_times`) converts that interval to
the advertised 100 ticks per second, returns it, and optionally copies a
zero-initialized 32-byte x86-64 `tms` with the value in `tms_utime`. System and
child fields remain zero. An invalid complete output returns `-EFAULT` without
a partial write. This is bounded single-task accounting, not scheduler CPU
charging or child aggregation.
`rax = 98` (`__NR_getrusage`) accepts `RUSAGE_SELF`, `RUSAGE_THREAD`, and
`RUSAGE_CHILDREN`. Self and thread queries return a zero-initialized 144-byte
x86-64 `rusage` with the same elapsed interval in `ru_utime`; system time and
all resource counters remain zero. The no-child proof returns an all-zero
structure for `RUSAGE_CHILDREN`. Other selectors return `-EINVAL`, and an
invalid complete output returns `-EFAULT` without a partial write. This is
single-task compatibility accounting, not scheduler-derived usage accounting.
`sysinfo` publishes the same one-GiB, no-swap memory profile exposed through
the current procfs compatibility files plus monotonic uptime and one process.
`sched_rr_get_interval` reports the bounded scheduler's 100 ms quantum for any
live process visible in the domain and matches Linux's negative-PID,
retired-PID, and output-fault ordering.
`sched_setparam` and `sched_setscheduler` admit idempotent `SCHED_OTHER` writes
for any live process in the domain. Unknown policies return `-EINVAL`, valid
privileged policies return `-EPERM`, retired processes return `-ESRCH`, and the
parameter is copied only after process and policy validation. Runtime probes
cover success, invalid priority and policy, bad pointers, negative PIDs, and
retired PIDs.
`sched_setattr` and `sched_getattr` expose the same default-policy contract
through Linux's extensible 56-byte attribute. Size and flag arguments use their
Linux 32-bit widths; getters zero supported extensions and setters accept only
zero-filled extensions. The conformance probe covers current and retired
processes, negative PIDs, short structures, bad pointers, unknown and
privileged policies, nonzero nice values, and invalid flags.
Because Ginux has no swap or page reclaim path, every current and future
userspace mapping is resident. `mlock`, `mlock2`, `munlock`, `mlockall`, and
`munlockall` therefore implement the complete Linux-visible contract without
additional residency state. They validate mapped ranges, Linux integer
narrowing, `MLOCK_ONFAULT`, and the `MCL_CURRENT`, `MCL_FUTURE`, and
`MCL_ONFAULT` combinations; zero-length ranges are successful and unmapped
nonempty ranges return `-ENOMEM`. One runtime probe covers all five calls,
valid and invalid flags, mapped and unmapped ranges, and unaligned range
rounding. `setrlimit` accepts an
idempotent write of the current bounded stack or descriptor limit; changing a
limit remains denied. `sync` has a direct userspace conformance probe and
succeeds after synchronizing every filesystem in the current RAM-backed and
immutable-storage profile. `syncfs` has runtime success and bad-descriptor
assertions and synchronizes the filesystem associated with any open descriptor.
`fsync` and `fdatasync` complete immediately for regular immutable OCI and
RAM-backed overlay files because those objects are already coherent with their
authoritative backing store. Directories use the same filesystem path; pipes,
sockets, consoles, and other nonsynchronizable descriptors return `-EINVAL`.
Closed and `O_PATH` descriptors return `-EBADF`, and descriptor arguments use
Linux's 32-bit width. Runtime probes cover both calls, a regular memfd, a pipe,
a console, a closed descriptor, and high register bits.
`sync_file_range` applies Linux's signed range validation and all combinations
of `SYNC_FILE_RANGE_WAIT_BEFORE`, `WRITE`, and `WAIT_AFTER` to writable regular
files. RAM-backed data is already coherent, so successful requests need no
additional writeback. The runtime probe covers every flag bit, zero length,
Linux 32-bit flag and descriptor narrowing, negative and overflowing ranges,
unknown flags, and invalid descriptors.
`fadvise64` accepts all six Linux advice values for non-pipe open descriptors;
resident immutable and RAM-backed data already satisfies each hint. It applies
Linux integer narrowing, rejects negative offsets and lengths or unknown advice
with `-EINVAL`, invalid descriptors with `-EBADF`, and pipes with `-ESPIPE`
before range or advice validation. Runtime tests cover all advice values,
overflowing positive ranges, negative values, high register bits, invalid
descriptors, and pipe error precedence.
`readahead` is successful for readable regular OCI, overlay, and tmpfs files
because their data is already resident. It implements Linux descriptor
narrowing and distinguishes bad or write-only descriptors (`-EBADF`), negative
offsets (`-EINVAL`), and inapplicable file kinds (`-EINVAL`). Count does not cap
the operation. Runtime tests cover zero and maximum counts, high register bits,
negative offsets, write-only files, directories, and invalid descriptors. Legacy
`epoll_create` delegates to `epoll_create1(0)`. The current process, group, or
user may query the default I/O priority or idempotently set it to default.
`membarrier` implements a complete single-CPU advertised profile: global,
global-expedited, private-expedited, both expedited registration commands, and
registration queries. Registrations belong to the process memory context, are
inherited by fork, and are cleared by exec. Private-expedited execution before
registration returns `-EPERM`; unsupported commands and invalid flags return
`-EINVAL`. Successful barriers issue a sequentially consistent kernel fence.
The syscall narrows its command, flags, and CPU arguments to their Linux UAPI
types, and runtime tests cover query, registration, repeat registration,
execution, rejection, and error precedence. Sync-core and rseq commands are not
advertised and consistently return `-EINVAL`.
`preadv`, `pwritev`, `preadv2`, and `pwritev2` apply the validated bounded
iovec profile to regular files without changing the descriptor offset. The
current profile accepts a zero high offset and, for the v2 calls, zero flags.
Regular-file `readv` and `writev` also cover mutable overlay and tmpfs files.
`ftruncate` accepts a writable regular-file descriptor, copies an immutable OCI
file into the per-domain overlay when necessary, and grows or shrinks it with
demand-allocated pages. Negative lengths return `-EINVAL`. `fallocate` admits
mode zero for writable regular files and ensures that `offset + length` bytes
are allocated without shrinking an existing file. It uses the same copy-up and
demand-allocation path. Negative inputs and zero lengths return `-EINVAL`,
range overflow returns `-EFBIG`, unavailable memory returns `-ENOSPC`, and
allocation modes such as hole punching remain explicitly `-EOPNOTSUPP`.
`copy_file_range` copies an arbitrary requested length between distinct regular
files through a page-sized kernel working buffer, so the working buffer does
not cap the transfer. It supports immutable OCI inputs, demand-allocated
overlay files, and the current tmpfs files. Null offset pointers advance the
corresponding descriptor offset; non-null signed offsets are validated
and updated without changing the descriptor offset. Zero flags are required.
The same backing file, negative offsets, non-regular descriptors, and aliased
offset pointers return `-EINVAL`; inaccessible offsets return `-EFAULT`, and an
append-only output returns `-EBADF`.
`sendfile` streams a regular OCI, overlay, or tmpfs input through the same
page-sized kernel working buffer to a writable regular file or connected
IPv4/Unix socket. A null input-offset pointer advances the input descriptor;
an explicit nonnegative offset is updated instead. Regular output offsets
advance normally. Transfers may exceed one page, short socket or filesystem
writes return the completed byte count, and a nonblocking socket preserves its
normal `-EAGAIN` behavior. Same-file and append-file output remain rejected.
`bind` admits IPv4 TCP or UDP sockets on the configured host address or
`INADDR_ANY`. Port zero selects an ephemeral port; conflicting bindings return
`-EADDRINUSE`, and nonlocal addresses return `-EADDRNOTAVAIL`. `listen` turns a
bound, or automatically bound, TCP socket into a listener. The backlog argument
is validated but the current smoltcp-backed listener holds one pending
connection. `accept` and `accept4` return a new descriptor for that connection,
copy its peer address transactionally when requested, and immediately rearm the
original descriptor with a fresh TCP socket and buffer so it can accept again.
Connections to a listener on the domain's own IPv4 address are paired inside
the kernel without traversing the physical NIC; they retain TCP endpoint names,
stream ordering, readiness, EOF, duplication, and descriptor cleanup behavior.
The listener keeps a pending local-connection queue capped by its requested
backlog and the available socket slots. The booted proof queues two local
clients before accepting one with `accept4` and one with `accept`, verifies
peer addressing and descriptor flags, and transfers a payload through the
accepted stream. `accept4` supports
`SOCK_CLOEXEC` and `SOCK_NONBLOCK`; a nonblocking listener returns `-EAGAIN`
when no connection is pending. A blocking accept polls the network until a
connection arrives or the listener becomes invalid. IPv6, Unix pathname
listeners, signal-interruptible scheduler wait queues, and multi-entry external
network listen backlogs remain outside this profile.
`shutdown` tracks independent read and write closure for AF_UNIX and
kernel-local TCP streams. Read shutdown discards queued local data and makes
subsequent reads return EOF; write shutdown makes subsequent sends return
`-EPIPE`, while the peer drains queued data and then observes EOF. The reverse
direction remains usable after a one-sided write shutdown. External TCP write
shutdown emits a FIN through the network stack, and all socket kinds implement
Linux descriptor-width, `ENOTSOCK`, `EBADF`, `ENOTCONN`, and invalid-`how`
errors. External TCP read shutdown drains and discards transport data after
each network poll so the peer continues to receive acknowledgements while
userspace consistently observes EOF. `poll` reports that EOF as readable and
suppresses writability after write shutdown; the booted AF_UNIX and local-TCP
proofs cover both transitions. A failed `sendto`, `sendmsg`, or `sendmmsg` on a write-shut stream
generates kernel-origin `SIGPIPE` while returning `-EPIPE`; `MSG_NOSIGNAL`
suppresses the signal without changing that error. The booted proof blocks and
consumes each generated signal, verifies its `SI_KERNEL` source, and verifies
that all three suppression paths leave no pending signal. External transport
shutdown under concurrent traffic and concurrent half-close races remain
pending, so syscall 48 is classified Partial rather than Complete.
`openat2` accepts the 24-byte version-zero `open_how` structure and delegates
zero-resolve requests to the same per-domain VFS path as `openat`. Larger
structures up to one page are accepted only when every extension byte is zero;
nonzero extensions and larger structures return `-E2BIG`. Undersized
structures, unsupported resolve policies, modes outside `07777`, and a nonzero
mode without `O_CREAT` return `-EINVAL`. This profile does not yet implement
the `RESOLVE_*` lookup constraints.
`fchmodat2` delegates to the admitted zero-flag `fchmodat` profile.
Legacy `utime`, `utimes`, and `futimesat` convert their second-resolution or
microsecond-resolution inputs to overlay timestamps; null time pointers select
the current realtime value, matching the existing bounded `utimensat` policy.
`rax = 36` (`__NR_getitimer`) reports each of `ITIMER_REAL`, `ITIMER_VIRTUAL`,
and `ITIMER_PROF` as a zero-initialized, disabled 32-byte x86-64 `itimerval`.
`alarm(0)` idempotently disarms the same currently disabled `ITIMER_REAL`
profile and returns zero. Arming through `alarm` remains unsupported until
asynchronous `SIGALRM` delivery exists. A direct `restart_syscall` with no
installed restart block returns `-EINTR`, matching Linux's initial
`do_no_restart_syscall` state; resumable interrupted operations are not yet
represented by a restart block.
`rax = 38` (`__NR_setitimer`) accepts an all-zero value to idempotently disarm
one of those timers and optionally returns its previous disabled value. Arming
a timer returns `-ENOSYS` until asynchronous signal delivery exists. Invalid
selectors return `-EINVAL`; invalid complete input or output ranges return
`-EFAULT`, and rejected requests do not partially change the old-value output.
`rax = 9` (`__NR_mmap`) has a scratch-window profile and one authenticated
executable-install profile. The scratch window has four page slots over
`0x40b000..0x40f000` and admits private extents of one through four pages. A
null hint selects the lowest contiguous free run. Alternatively, anonymous
`MAP_FIXED_NOREPLACE` requires an exact page-aligned free subrange wholly inside
that window and returns `-EEXIST` without mutation if any selected slot is
occupied, including a `PROT_NONE` reservation. Anonymous `MAP_FIXED` accepts
the same bounded placement and atomically fills holes or reservations, or
replaces active pages with fresh zeroed RW/NX generation leases.
It preflights every old leaf and lease before changing the page table, then
scrubs and releases every displaced lease before returning. Its anonymous form
otherwise requires a nonzero length that rounds up to no more than four pages,
`PROT_NONE`, `PROT_READ`, or `PROT_READ | PROT_WRITE`,
`MAP_PRIVATE | MAP_ANONYMOUS`,
fd -1, and offset zero. `PROT_NONE` publishes an occupied virtual reservation
without allocating a frame or installing a leaf. Read-only and RW requests
allocate and install zero-filled generation-bound R/NX or RW/NX frame leases.
`MAP_FIXED` preserves the requested protection when filling or replacing a
selected range. `MAP_FIXED` with
`PROT_NONE` preflights every selected leaf and exact old lease, clears the
leaves as one bounded transaction, publishes reservation metadata, and then
scrubs and releases the displaced leases. It therefore discards mapped
contents rather than preserving them as a future `mprotect` transition would.
The immutable-file form requires `PROT_READ`, `MAP_PRIVATE`, a readable
descriptor for `/init`, `/lib64/ld-ginux.so`, `/lib64/libginux.so`, or
`/lib64/libginux-aux.so`, and a
page-aligned offset. Its complete one- through four-page window must fit within
the file length rounded up to a page; offsets at or beyond EOF are rejected rather
than deferred to a `SIGBUS` path. The kernel copies the authenticated bytes
into allocator-owned read-only/NX frames and zero-fills a partial final page.
Mapping failure scrubs and releases every provisional lease before returning.
Descriptor cursors are unaffected. Unsupported arguments return `-EINVAL`, a
non-file descriptor returns `-ENODEV`, an invalid descriptor returns `-EBADF`,
and a first-fit request without enough contiguous free entries returns
`-ENOMEM`. A fixed request outside the window returns `-EINVAL`.
The executable-install profile accepts only open `/init`,
`/lib64/libginux.so`, or `/lib64/libginux-aux.so` descriptors and exact one-page
`MAP_PRIVATE | MAP_FIXED_NOREPLACE` requests already authenticated by the kernel plan. For
`/init`, those contracts are offset `0` at `0x3ff000` with `PROT_READ`, offsets
`0x1000..0x5000` at `0x400000..0x404000` with `PROT_READ | PROT_EXEC`, and
offsets `0x6000..0x7000` at `0x405000..0x406000` with `PROT_READ |
PROT_WRITE`. For `libginux.so`, the kernel admits a page-aligned base in the
half-open range `0x610000..0x620000`. Offset `0x1000` must be RX at relative
address zero, offset `0x2000` must be RX at relative address `0x1000`, and
offset `0x3000` must be R/NX at relative address `0x2000`; offset `0x4000` must
be RW/NX at relative address `0x3000`. The current
interpreter selects base `0x618000`, producing mappings at `0x618000`,
`0x619000`, `0x61a000`, and `0x61b000`.
For `libginux-aux.so`, the independent half-open base range is
`0x630000..0x640000`. Offset `0x1000` must be RX at relative address zero and
offset `0x2000` must be RW/NX at relative address `0x3000`. The interpreter
selects base `0x638000`, producing mappings at `0x638000` and `0x63b000`.

The interpreter derives these requests from each authenticated file's bounded
`PT_LOAD` table rather than embedding the valid page sequence. It supplies the
main load bias and selects the dependency bias, translates ELF permissions,
and issues one request per derived page. The kernel represents up to eight
dependency pages as authenticated relative-address, file-offset, file-size,
and protection contracts rather than special-casing RX and RW segments. Two
ordered dependency slots retain independent base windows, selected biases,
mapping masks, eight-entry contract tables, and generation-tracked leases. The
current executable proof populates and validates both slots. The
first valid dependency request
atomically binds its candidate bias; later requests must produce that same
bias from their authenticated relative addresses. The kernel contract remains
authoritative: a derived address, offset, relative layout, or protection that
does not match the authenticated plan is rejected. The runtime proof
deliberately submits a wrong first-page offset, a W+X executable request, a
repeated first-page request, and a dependency page at a different in-window
bias and requires `-EINVAL`, `-EINVAL`, `-EEXIST`, and `-EINVAL`, respectively.
The common file walker repeats its bad-offset, W+X, and duplicate-placement
checks while loading the auxiliary DSO. Exit validates generation ownership,
scrubs and releases every lease in both slots, and restores the kernel-owned
live-frame count.

Each admitted main or dependency page receives a generation-tracked kernel
frame on demand. The kernel copies only its authenticated file extent,
zero-fills the remainder, requires the leaf to remain absent, and enforces W^X
while installing it. The dependency window is
preflighted against all other admitted mappings before execution. A duplicate
first-page request for each image exercises allocate-then-release rollback
before returning `-EEXIST`. Main and dependency mappings persist through
process exit; the admitted RELRO operation may only remove write permission
from the second main data page. Every required contract must be consumed once
before successful process completion. A duplicate returns `-EEXIST`; another
file, address, offset, size, flag, or permission set is rejected. Descriptor
cursors remain unchanged. On exit the inactive process root is retained while
all executable payload frames are scrubbed and their exact generation leases
are released; the kernel-owned live-frame count must return to its
pre-execution value. This is bounded userspace-owned loading for two
preauthenticated images and one fixed dependency window, not ASLR, a general
executable allocator, or authority to map unauthenticated bytes.
`rax = 10` (`__NR_mprotect`) activates a complete anonymous `PROT_NONE` range
as fresh zeroed `PROT_READ` or `PROT_READ | PROT_WRITE` pages after one
all-leaves-absent preflight. It can also remove write permission from a complete
RW anonymous range. Activation publishes frame leases only after every new
page is installed; failure scrubs and releases all provisional leases.
Read-only-to-writable escalation, transitions from mapped pages back to
`PROT_NONE`, executable protection, mixed-source transitions, and partial
transitions spanning reserved plus mapped pages return `-EINVAL`.
Requesting the protection already recorded across the complete range is an
idempotent success after validating each present generation lease and PTE, or
each reservation's empty lease and absent leaf. This includes `PROT_NONE` on an
already-reserved range; it does not turn a mapped range into a reservation.
`rax = 11` (`__NR_munmap`) accepts any page-aligned address and nonzero length
that rounds to a subrange of the four-page scratch window, including mapped
pages, holes, or both. It clears the selected
hardware leaves, invalidates their TLB entries, and scrubs the physical frames
before returning their exact generation leases to the allocator. Reserved
pages instead require an absent leaf and empty lease slot and are cleared
without physical work. A partial
removal rebuilds the registry around the holes while preserving every remaining
page's write and immutable-file attributes. Repeating a supported exact unmap
succeeds. A range can remove independent adjacent areas and already-unmapped
holes in one transaction. The anonymous
proof keeps two one-page areas active, queries and removes the second
independently, and proves the first remains intact. It then
removes both the head and tail of separate two-page areas and validates the
surviving area's residency, access, and contents. It also crosses a two-page
area boundary with syscall copy-out, while the file proof maps the complete two-page interpreter,
observes ELF magic and its entry byte, checks the zero-filled tail, proves
syscall copy-out is denied, and rejects an unaligned file offset. Both paths
exit with the selected leaves absent. A four-page proof seals its middle two
pages, creates separated holes, reuses the lowest hole, rejects a fragmented
two-page allocation with `-ENOMEM`, and releases the complete sparse range.
This is not a general VMA allocator: arbitrary
hints, fixed shared or file replacement mappings, writable or shared file
mappings outside the admitted executable pages, ranges outside the bounded
window, arbitrary files, demand paging, and mapping growth are unsupported.
Outside the scratch window, fixed placement is available only for the seven
admitted main pages and two admitted dependency pages described above.
For runtime domains, `rax = 25` (`__NR_mremap`) accepts a complete mapped
page-aligned range in the dynamic mapping region. Shrink releases trailing
pages; growth extends an anonymous mapping in place when the following range
is free. `MREMAP_MAYMOVE` relocates the same generation-bound frames and
preserves page protections when in-place growth is impossible, while
`MREMAP_FIXED` additionally selects a non-overlapping free destination.
Relocation rollback restores every page to its source range on a failed move.
File-backed growth and `MREMAP_DONTUNMAP` remain unsupported. The boot probe
profile continues to reject `mremap` with `-ENOSYS`.
`rax = 29`, `30`, `31`, and `67` (`__NR_shmget`, `shmat`, `shmctl`, and
`shmdt`) provide the first System V shared-memory profile in runtime domains.
Segments and their frame inventories grow from allocator-owned metadata pages
rather than a fixed object or size table. Keyed creation implements
`IPC_CREAT`/`IPC_EXCL`; `IPC_PRIVATE` always creates a new segment. `shmat`
installs the segment's existing frames at a free dynamic address, supports
read-only and read-execute attachment under W^X, and records an independent
attachment. `shmdt` removes that mapping without releasing object-owned data.
`shmctl(IPC_STAT)` returns the native x86-64 112-byte `shmid_ds` subset,
`IPC_SET` updates its mode, and `IPC_RMID` hides the key immediately while deferring destruction until the
last recorded attachment leaves. Process hierarchy cloning preserves the
shared PTE marker, aliases the same writable frames instead of copying them,
and transactionally inherits attachment records. Child exit drops those
records after its page tables retire, and domain teardown releases remaining
segments. `SHM_REMAP`, timestamps, owner/group updates, permission enforcement,
and information commands remain unsupported.
`rax = 64`, `65`, `66`, and `220` (`__NR_semget`, `semop`, `semctl`, and
`semtimedop`) provide dynamically allocated per-domain System V semaphore
sets. Set metadata and values extend through generation-bound pages; the
implementation enforces Linux's default `SEMMSL=32000` semaphores per set and
`SEMOPM=500` operations per call rather than a compile-time storage array.
Keyed `IPC_CREAT`/`IPC_EXCL` lookup and `IPC_PRIVATE` creation are supported.
Multi-operation calls use a complete, non-mutating preflight before committing
any value, so range and would-block failures preserve every semaphore.
`semctl` implements `IPC_RMID`, mode-only `IPC_SET`, `IPC_STAT`, `GETPID`,
`GETVAL`, `GETALL`, `GETNCNT`, `GETZCNT`, `SETVAL`, and atomic `SETALL`.
Valid `semtimedop` timespecs use the same immediate operation engine.
Operations that would sleep return `EAGAIN`; wait scheduling, `SEM_UNDO`,
timestamps, owner/group updates, permission enforcement, and Linux information
commands remain unsupported.
`rax = 68`, `69`, `70`, and `71` (`__NR_msgget`, `msgsnd`, `msgrcv`, and
`msgctl`) provide dynamically allocated per-domain System V message queues.
Queue metadata and message payloads extend through generation-bound pages;
individual messages are admitted up to Linux's default `MSGMAX=8192`, while
each queue begins with `MSGMNB=16384` bytes and can change that limit through
`IPC_SET`. Keyed `IPC_CREAT`/`IPC_EXCL` lookup and `IPC_PRIVATE` creation are
supported. Receives implement FIFO selection, exact positive types, Linux's
lowest-type negative selection, `MSG_EXCEPT`, nondestructive ordinal
`MSG_COPY`, oversized-message preservation, and `MSG_NOERROR` truncation.
`msgctl` implements `IPC_RMID`, mode/byte-limit `IPC_SET`, and the native
x86-64 120-byte `IPC_STAT` layout. Empty/full operations return `EAGAIN`;
blocking wait scheduling, timestamps, owner/group updates, credential
enforcement, and Linux information commands remain unsupported.
`rax = 240` through `245` provide per-domain POSIX message queues.
`mq_open` accepts one-component slash-prefixed names, Linux access/create,
exclusive, nonblocking, and close-on-exec flags, and native 64-byte `mq_attr`
records. Named objects and payload chains allocate pages on demand rather than
using a fixed queue or byte array. Descriptors participate in dup, fork,
close-on-exec, close, and unlink reference accounting. `mq_timedsend` validates
priorities below 32768 and orders equal-priority messages FIFO;
`mq_timedreceive` returns the highest priority first and requires Linux's full
configured receive-buffer size. `mq_getsetattr` reports limits and current
depth and changes only `O_NONBLOCK`; `mq_notify` accepts unregister and
`SIGEV_NONE`. Full/empty queues return `EAGAIN`. Scheduler-backed absolute
deadline waits, `SIGEV_SIGNAL`, and `SIGEV_THREAD` delivery remain unsupported.
`rax = 206` through `210` and `333` provide a dynamic per-domain legacy Linux
AIO profile. `io_setup` allocates an mmap-backed ring, publishes the native
header and context identifier atomically, and creates a page-allocated kernel
completion queue. `io_submit` accepts native 64-byte `iocb` records and
executes `IOCB_CMD_PREAD`, `PWRITE`, `FSYNC`, and `FDSYNC`, retaining the user
data and request address in 32-byte `io_event` completions. `io_getevents` and
`io_pgetevents` drain completions in order, validate timeout and optional
signal-mask records, and honor zero-duration polling. `io_destroy` unmaps the
ring and releases queued records. Work currently completes synchronously, so
`io_cancel` reports `EINVAL`; scheduler-backed waits, in-flight cancellation,
vectored and poll requests, eventfd signaling, and userspace ring consumption
remain unsupported.
`rax = 459`, `460`, and `461` expose the Linux v6.12 LSM userspace query
surface without conflating it with Ginux's native domain security boundary.
`lsm_list_modules` validates its flags and size pointer, writes a required size
of zero, and returns zero because no Linux LSM module is active.
`lsm_get_self_attr` validates Linux attribute IDs, `LSM_FLAG_SINGLE`, and the
size record, resets the required size to zero, then returns `EOPNOTSUPP` because
there is no Linux security context provider. `lsm_set_self_attr` similarly
validates its selector and reserved flags before returning `EOPNOTSUPP`.
Native domain policy is neither enumerated nor mutable through this ABI.
`rax = 453` (`__NR_map_shadow_stack`) reports `EOPNOTSUPP` because Ginux does
not yet enable x86 userspace CET shadow stacks. This matches Linux x86's first
operation check when shadow stacks are unavailable, including its precedence
over address, length, and flag validation. CET enable/status controls,
shadow-stack VMAs, restore tokens, guard gaps, fork integration, and positive
mapping behavior remain pending, so the call remains Partial.
`rax = 303` and `304` (`__NR_name_to_handle_at` and
`__NR_open_by_handle_at`) expose generation-stable handles for regular files,
directories, and symbolic links in the OCI lower root and mutable overlay.
The name operation supports relative/absolute lookup, `AT_EMPTY_PATH`,
`AT_HANDLE_FID`, Linux's size-discovery `EOVERFLOW`
contract, legacy 32-bit mount IDs, and v6.12 unique 64-bit mount IDs. Handles
remain valid across overlay renames. Opening validates the mount anchor,
handle type and length, stale generations, access flags, directory type, and
symlink `O_PATH` requirement; returned descriptors support close-on-exec and
normal duplication/lifecycle rules. `O_PATH` descriptors reject data I/O.
OCI lower files currently reopen read-only, symlink-following handle lookup
returns `EOPNOTSUPP`, connectable-handle semantics are outside the v6.12
baseline, and procfs, devfs, and tmpfs objects do not export handles, so both
calls remain Partial.
`rax = 457` and `458` (`__NR_statmount` and `__NR_listmount`) expose the
read-only Linux v6.12 mount-query ABI for the current domain. Version-zero and
version-one `mnt_id_req` records are accepted, zero-filled forward extensions
are tolerated, and a version-one namespace identifier must select the active
domain namespace. `listmount` enumerates stable unique identifiers for the
domain root and its automatic proc child, supports cursors and reverse order,
and reports unknown parents with `ENOENT`. `statmount` fills the 512-byte
v6.12 fixed record and requested superblock, mount, propagation, root,
mountpoint, filesystem-type, namespace-ID, and option fields, with string
offsets relative to `str[]`; undersized output returns `EOVERFLOW`. The virtual
dev and tmp trees are not distinct VFS mounts yet, namespace selection by file
descriptor is outside the v6.12 baseline, and arbitrary mount trees remain
unavailable, so both calls remain Partial.
`rax = 447` (`__NR_memfd_secret`) creates an anonymous read/write file with an
initial size of zero and accepts only Linux's `FD_CLOEXEC` flag. `ftruncate`
sets its elastic overlay-backed size and `MAP_SHARED` read/write mappings are
usable directly by the owning userspace. Those pages carry page-table metadata
that survives `mprotect`; every normal kernel user-copy path rejects them, so
passing a secret pointer to another syscall returns `EFAULT`. Descriptor
`read`, `write`, positioned I/O, and kernel-mediated range copies reject the
file with `EINVAL`, and duplication preserves its secret identity. Private or
executable mappings and unknown flags return `EINVAL`. The current bounded
profile eagerly allocates mapping pages and does not yet remove them from the
kernel direct map, maintain coherence between repeated mappings, charge locked
memory, prevent hibernation exposure, or preserve secret metadata through
`fork`, so the call remains Partial.
`rax = 462` (`__NR_mseal`) irreversibly seals complete page-rounded ranges in
the bounded anonymous scratch window and the runtime dynamic mapping region.
The operation preflights every page before changing state, is idempotent, and
requires zero flags plus a page-aligned start. A gap returns `-ENOMEM`; invalid
flags, alignment, or overflow return `-EINVAL`. A sealed range rejects
`mprotect`, `pkey_mprotect`, `munmap`, `mremap`, and `MAP_FIXED` replacement
with `-EPERM`. `MADV_DONTNEED` is rejected for sealed read-only anonymous pages
and remains permitted for sealed writable pages, following Linux 6.12's
destructive-advice rule. Sealing executable-image, stack, heap, and other
non-`mmap` VMAs is not yet supported.
`rax = 28` (`__NR_madvise`) accepts only `MADV_DONTNEED` for a fully mapped
nonzero range rounding to one through four anonymous pages. The range may cross adjacent registry
records. Immutable-file mappings and `PROT_NONE` reservations reject advice
with `-EINVAL`. For an anonymous
range it validates that every leaf still maps its allocator-owned frame, then
zeros the frames through the supervisor direct map without changing their
present, user, NX, or writable state. The proof also discards a protected one-page
read-only mapping, observes zeroes from ring 3, and unmaps it normally. Other
advice, ranges, or inactive mappings return `-EINVAL` without changing the
contents. There is no paging policy, reclamation queue, or lazy refill.
`rax = 440` (`__NR_process_madvise`) accepts up to Linux's `IOV_MAX=1024`
native iovecs through a live pidfd for the calling process. It applies the same
`MADV_DONTNEED` operation to each nonempty range and returns the total advised
byte count. A later failing vector returns the successfully processed prefix,
matching the syscall's partial-progress contract. Unknown flags or advice
return `-EINVAL`, invalid vector memory returns `-EFAULT`, and a closed or
foreign pidfd returns `-EBADF` or `-ESRCH`. Advising another process and all
other advice values remain unsupported.
`rax = 448` (`__NR_process_mrelease`) validates flags and a live pidfd using
the same generation-bound descriptor contract. Every pidfd currently creatable
by `pidfd_open` names the live caller, for which Linux requires `-EINVAL`
because memory release is permitted only after exit begins. Closed descriptors
return `-EBADF`, foreign generations return `-ESRCH`, and nonzero flags return
`-EINVAL`. Reclaiming an exiting child remains pending until pidfds can retain
zombie process identities.
`rax = 27` (`__NR_mincore`) accepts any fully occupied nonzero scratch range
rounding to one through four pages, including a range crossing adjacent
registry records, and atomically
writes one byte per page to a fully validated vector. Present generation-owned
pages report one; frame-less `PROT_NONE` reservations report zero. It returns
`-ENOMEM` when any requested page is unmapped. Misaligned or out-of-window
ranges return `-EINVAL`; an invalid vector returns `-EFAULT`. It exposes only
the fixed window, not arbitrary page-table state or reclaim information.
The probe uses `rax = 12` (`__NR_brk`) to query an initial break at `0x407000`,
grow within one separately owned zero-filled RW/NX page, write through that
mapping, and shrink the break. A request beyond `0x408000` follows Linux syscall
semantics by returning the unchanged current break rather than a negative
error. The page remains mapped for the lifetime of this bounded process and is
scrubbed during teardown. A valid `write` copies from the grown heap. A second
eight-byte write crossing the current break returns `-EFAULT`, proving syscall
copy ranges do not expand to the whole mapped page.
The probe also performs a valid `write` from unused stack space and requires
`-EFAULT` for an eight-byte range crossing the stack's upper guard boundary.
Another valid `write` copies an embedded payload from the declared RX segment,
while `uname` copy-out to that segment returns `-EFAULT`. Copy direction thus
respects VMA permissions rather than treating every mapped page as writable.
`rax = 14` (`__NR_rt_sigprocmask`) maintains one eight-byte x86-64 Linux signal
mask per bounded thread. It supports `SIG_BLOCK`, `SIG_UNBLOCK`, `SIG_SETMASK`,
and a null-set query; `SIGKILL` and `SIGSTOP` are always cleared. `clone`
snapshots the parent's mask into the new generation-tagged thread slot. Every
cooperative or blocking context switch saves the outgoing active mask and
installs the selected thread's mask; retirement clears it before slot reuse.
The ring-3 proof has each sequential child observe the parent's blocked
`SIGUSR1`, unblock it locally, and then verifies that the resumed parent still
reports its original mask. The kernel
validates the set size and every complete input/output range before mutation,
so `-EINVAL` and `-EFAULT` leave the prior mask intact. The bounded synchronous
delivery path temporarily adds the delivered signal unless `SA_NODEFER` is
set, always adds the explicit action mask, then restores the exact prior mask
through `rt_sigreturn`. The bounded pending contract is described below;
external asynchronous generation and interrupted-syscall restart behavior are
not implemented yet.
`rax = 13` (`__NR_rt_sigaction`) stores and queries one 32-byte x86-64 action
record for each of signals 1 through 64. Complete action and old-action ranges
are validated before state changes; action masks also clear `SIGKILL` and
`SIGSTOP`. Installing handlers for those two uncatchable signals returns
`-EINVAL`. Other records remain opaque state unless selected by the bounded
synchronous-fault or generated-standard-signal contracts below. The proof exercises default queries,
normalized registration, fault atomicity, and restoration of every action
before exit.

An unmasked ring-3 page fault is delivered as signal 11 when its action has
`SA_RESTORER` and any supported combination of `SA_SIGINFO`, `SA_ONSTACK`,
`SA_NODEFER`, and `SA_RESETHAND`, both handler and restorer point into
a current executable image extent, a Linux thread is current, and no signal
frame is active. The
kernel retains the complete interrupted general-register, RIP, RSP, RFLAGS, and
mask state. It places the 440-byte x86-64 Linux `rt_sigframe` base layout below
the selected writable stack: the restorer is at offset 0, the 304-byte
`ucontext` is at offset 8, and the 128-byte `siginfo_t` is at offset 312. Compile-time
layout assertions bind those sizes and offsets. Normal-stack delivery first
preserves the 128-byte x86-64 red zone; alternate-stack delivery starts from
the registered top. Both paths round the frame base down to 16 bytes and then
subtract eight, leaving ABI-correct handler entry alignment. The context
publishes flags `UC_SIGCONTEXT_SS | UC_STRICT_RESTORE_SS`, the prior alternate
stack, complete integer registers and selectors, the prior signal mask in both
defined locations, zeroed reserved fields, and a null `fpstate` pointer because
floating-point/XSAVE preservation is not implemented yet. Handler entry sets
`rdi = 11`, `rsi = &frame.info`, and `rdx = &frame.uc` even when `SA_SIGINFO` is
clear; generated delivery places its selected signal number in `rdi`. Both
clear DF, RF, and TF as required by the x86 signal ABI before
entering through the exception `IRETQ` path. `SA_ONSTACK` selects the registered
alternate-stack top and publishes `SS_ONSTACK` for an ordinary registration; the syscall gate admits that
range only while both the flag and active frame agree. The proof handler
observes `SS_ONSTACK`, proves replacement returns `-EPERM`, observes the
automatic `SIGSEGV` bit plus its action mask, maps the missing page, and returns
through the registered restorer.
With `SA_SIGINFO`, page-fault delivery publishes `si_signo = SIGSEGV`,
`si_code = SEGV_MAPERR` or `SEGV_ACCERR` from the page-fault present bit,
`si_addr = CR2`, and matching trap, error, and CR2 machine-context fields.
Generated `kill` publishes `SI_USER`, while `tkill` and `tgkill` publish
`SI_TKILL`; their sender PID is the bounded container-local PID 1 and UID is 0.
Without `SA_SIGINFO`, the information record is zeroed rather than exposing
uninitialized kernel data.
`SA_NODEFER` omits the automatic `SIGSEGV` bit while retaining the action mask.
`SA_RESETHAND` clears the complete handler, flags, restorer, and action-mask
record only after the frame and handler entry have committed. The dedicated
runtime combines both flags, observes only its explicit `SIGUSR1` action-mask
bit from inside the handler, returns with the original empty mask, and queries
an already-defaulted action. Unsupported action-flag bits fail delivery with
`-ENOSYS`; nested delivery remains outside this bounded profile even with
`SA_NODEFER`.

`rax = 15` (`__NR_rt_sigreturn`) succeeds only for that one active frame at its
exact post-`ret` stack address with the registered restorer still present at
the frame base. A premature call returns `-EINVAL` without consuming the frame.
Successful return clears the kernel
record and uses a dedicated `IRETQ` syscall epilogue. Before committing that
return, the kernel copies all 440 frame bytes back and requires the restorer,
`uc_flags`, null link, prior alternate-stack record, CS/SS selectors, zero
FS/GS selectors, null FP-state pointer, and reserved machine-context words to
remain valid. It restores all user-edited integer registers, including `rax`,
`rcx`, and `r11`; applies only Linux's `FIX_EFLAGS` set of arithmetic, tracing,
direction, resume, and alignment-check flag bits while retaining the protected
bits from the kernel snapshot; and installs `uc_sigmask` after removing
`SIGKILL` and `SIGSTOP`. Frame information and trap metadata remain
observational. A malformed frame returns `-EINVAL` without consuming the active
context, so the handler may repair it and retry. The proof temporarily supplies
a forbidden non-null FP-state pointer and observes that atomic rejection, then
restores it to zero, changes saved `rax` and `uc_sigmask`, and observes both
changes after return. After a successful restoration, the kernel checks the
unblocked pending set before resuming the restored context. A selected next
signal receives that exact restored context—including user edits—as its
interrupted state. Floating-point/XSAVE restoration, user-edited
alternate-stack restoration, nested delivery, restartable syscalls, and other asynchronous signal sources remain
unsupported. A page fault
that does not meet this contract terminates the bounded Linux execution rather
than entering the native-domain scheduler fault path.
`rax = 62` (`__NR_kill`) accepts PID 1 or process-group selector 0. Signal zero
performs an existence check. An unmasked, explicitly caught standard signal in
the range 1 through 31 constructs the signal frame from the live syscall context,
redirects the pending `SYSRET` into the handler, and records the architectural
post-syscall `rcx` and `r11` values. `rt_sigreturn` then resumes at the
instruction following `kill` with a zero result. A disposition of `SIG_IGN`
discards the signal successfully. The default `SIGPIPE` disposition terminates
the complete bounded process and releases its descriptors, signal state, and
threads. Other default dispositions and real-time signals 32 through 64 remain
outside this bounded generation profile and return `-ENOSYS` instead of
pretending to apply Linux termination, stop, continue, or queueing semantics.
The ring-3 proof covers caught `SIGSEGV` and `SIGUSR1`, inherited ignored
signals, default `SIGPIPE`, and an unsupported default `SIGTERM`. Foreign PIDs
return `-ESRCH`, and invalid signal numbers return `-EINVAL`.
`rax = 200` (`__NR_tkill`) admits positive, in-range container-local TID 1.
Zero, negative, and out-of-range identities return `-EINVAL`; other valid TIDs
return `-ESRCH`. Signal zero probes existence, while any admitted unmasked
caught standard signal uses the same generated-signal and full-context-return path as
`kill`. Although Linux callers should prefer `tgkill`, this bounded legacy
entry point is retained for compatibility and has no independent delivery
implementation.
`rax = 234` (`__NR_tgkill`) admits the positive container-local thread group
and thread pair `(1, 1)`, including signal zero as an existence query. Zero,
negative, or out-of-range identities return `-EINVAL`; well-formed foreign
thread groups or thread IDs return `-ESRCH`. An unmasked admitted caught standard signal
uses the same live-return delivery path as `kill`, and the dedicated measured
`/bin/signal-probe` installs a fresh post-`execve` handler, exercises every
identity class, completes one handler/`rt_sigreturn` lifecycle, then restores
the default action before exit. It also delivers a fresh page fault under
`SA_SIGINFO` and validates the exact handler pointers, `SEGV_MAPERR` address,
trap number, CR2, context flags, disabled alternate-stack record, signal masks,
and null FP-state pointer. The verifier therefore requires ten exact
delivery/return pairs for that measured runtime and two for the other probe
executions.
When an admitted caught standard signal is blocked, `kill`, `tkill`, or
`tgkill` coalesces it into that signal's bounded pending bit and returns success
without constructing a frame.
`rax = 127` (`__NR_rt_sigpending`) requires the exact eight-byte signal-set size
and a writable output range, then reports pending signals intersected with the
current mask. Wrong sizes return `-EINVAL`, and inaccessible output returns
`-EFAULT` without changing pending state. `rt_sigprocmask` selects the
lowest-numbered pending standard signal that is no longer masked, redirects
that syscall's live return into the handler, and clears only its pending bit
after the frame and return context are committed. Pending selection is deferred
while a frame is active. On successful `rt_sigreturn`, another unblocked
pending signal is selected before the restored userspace instruction can run;
otherwise execution resumes after `rt_sigprocmask`. The measured signal
runtime proves the signal remains pending while masked, the prior mask is
reported during unblock, and both mask and pending state are zero at teardown.
It also queues `SIGUSR2` before `SIGUSR1`, duplicates `SIGUSR1` through a
different generator, then proves numeric priority and independent per-signal
source metadata: `SIGUSR1` retains its first `SI_USER`, followed immediately by
`SIGUSR2` with `SI_TKILL`. The first handler edits saved `rax`; the second frame
and final syscall return both observe that value, proving context continuity
across the chain. Process/thread selection among multiple runnable threads,
real-time queue records, and external asynchronous sources remain unsupported.
`rax = 128` (`__NR_rt_sigtimedwait`) accepts the exact eight-byte signal-set
size and selects the lowest-numbered pending signal in the requested set after
removing `SIGKILL` and `SIGSTOP`. If a match already exists, a null, zero, or
positive valid timeout all return immediately with the signal number and
optionally copy its complete 128-byte generated `siginfo_t`; only then are that
pending bit and its source code cleared. Invalid set, timeout, or matching
information-output ranges return `-EFAULT`, invalid sizes or timespec fields
return `-EINVAL`, and none consume pending state. A valid zero timespec with no
match returns `-EAGAIN`. With no pending match, a null timeout can block the
current thread when the other bounded thread is ready. The wait records the
requested standard-signal mask and optional prevalidated information pointer in
typed scheduler state. A matching `kill`, `tkill`, or `tgkill` generation
synchronously consumes that signal before disposition processing, copies its
generated information when requested, installs the signal number in the saved
syscall return context, and marks the waiter ready. The generator then returns
normally in the peer. This profile selects the first matching blocked slot and
supports at most the existing two-thread execution graph. Null waits with no
runnable peer return `-ENOSYS`. A positive timeout with a runnable peer is
converted to a checked monotonic deadline and stored with the typed wait. At
the first Linux syscall or scheduling boundary at or after that deadline, the
kernel clears the wait and makes the saved context ready with `-EAGAIN`. A
periodic local-APIC tick performs the same transition independently in interrupt
context and preemptively dispatches the waiter after saving the running peer.
Interruption by nonmatching signals and general signal wait queues remain unsupported. The measured
runtime proves every immediate rejection, pending `SIGUSR1`/`SI_USER`
consumption, empty polling, mask cleanup, a positive-timeout wake, and a
child-to-parent signal wake. Teardown requires the wait kind, mask, output
pointer, deadline, and saved thread state to be fully reclaimed.
The measured system-container PID 1 adds one native-control boundary to this
contract. After publishing readiness with `SIGTERM` blocked, its null-timeout
wait for exactly that signal may park even though no Linux peer is runnable.
The kernel retains the complete generation-bound thread and process contexts,
marks the thread blocked, and returns to the native control plane. A validated
kernel-origin shutdown request writes the generated `siginfo_t`, wakes the
exact waiter, and resumes it with signal number 15. No other signal set,
process, or workload receives this no-peer admission; those waits continue to
return `-ENOSYS`. `specs/kernel/linux-supervisor-v0.md` defines the complete
bounded lifecycle.
`rax = 34` (`__NR_pause`) and `rax = 130` (`__NR_rt_sigsuspend`) use the same
generation-bound blocked-thread transaction. `pause` retains the current mask;
`rt_sigsuspend` requires an exact eight-byte mask, removes `SIGKILL` and
`SIGSTOP`, and installs it atomically with blocking. A caught, unmasked signal
restores the prior mask into both the saved context and thread record, commits
the handler frame, and leaves the suspended syscall returning `-EINTR` after
`rt_sigreturn`. Invalid mask pointers and sizes fail before state changes. The
current bounded scheduler requires another runnable Linux thread to enter the
wait; no-peer suspension and immediate delivery of a signal already pending at
entry remain unsupported and return `-ENOSYS` rather than falsely succeeding.
`rax = 131` (`__NR_sigaltstack`) reports an initially disabled 24-byte x86-64
alternate-stack record. It accepts `SS_DISABLE`, or a zero-flag or
`SS_AUTODISARM` stack of at least
2048 bytes wholly contained in one currently writable probe VMA. Invalid
records return `-EINVAL`, undersized stacks return `-ENOMEM`, and inaccessible
records or stack ranges return `-EFAULT` before state changes. The proof enables,
queries, rejects invalid replacements, and disables the stack before exit.
While ordinary delivery is using the alternate stack, null-new-stack queries
report `SS_ONSTACK` and any replacement returns `-EPERM`. With
`SS_AUTODISARM`, the frame retains the prior pointer, size, and flag before the
handler-visible configuration is atomically reset to disabled. Syscall stack
admission follows that retained active-frame extent rather than granting access
from the disabled public configuration. The measured handler queries the
disabled state from the autodisarmed stack; successful `rt_sigreturn` restores
the exact prior registration, which is queried before the probe explicitly
disables it. User edits to the frame's alternate-stack record remain rejected.
Before that overwrite, `rax = 158` (`__NR_arch_prctl`) with `ARCH_SET_FS` and
the exact RW segment base establishes a bounded TLS pointer; an FS-relative
load in ring 3 verifies the original payload. `ARCH_GET_FS` copies the active
base to a bounded user pointer. An invalid destination returns `-EFAULT`, and
an unsupported operation returns `-EINVAL`; both errors are recoverable. The
kernel saves the prior supervisor FS-base MSR before entry, restores it after
exit, and reads it back.
Loader-provided static TLS uses a separate checked execution context that binds
the initial FS base to the exact live static-TLS-plus-TCB copy extent. The boot
proof switches that state to an independently materialized sibling instance,
requires syscall-copy admission to follow the switch, and restores the initial
state before entering userspace. The sibling thread-table entry also retains an
exact guarded stack page, while the active initial-thread syscall-copy state
rejects that entire page. This is resource ownership and copy-boundary state,
not protection from direct access within the shared process address space.
An internal constructor additionally validates the exact bounded pthread-style
clone flag set and prepares the sibling's post-syscall RIP, guarded RSP,
RFLAGS, zero child return, FS base, and child-clear-TID registration. Invalid
flags or stack pointers are rejected before mutation, and slot retirement
clears the context. An internal publication proof copies the TID to distinct
parent- and child-TID words only after construction and rolls every child
execution field back when either target is denied. `CLONE_CHILD_SETTID`
publishes the child word, while `CLONE_CHILD_CLEARTID` registers that same word
for exit. The proof restores both words and retires the child before
ring-3 entry. Syscall 56 exposes the same transaction only for the fixed
preallocated sibling TLS and guarded stack. This does not yet make all syscall
state per-thread or connect Linux TLS switching to timer preemption.
The x86-64 `SYSCALL` entry frame now includes `rbx`, `rbp`, and `r12`-`r15` in
addition to `rdi`, `rsi`, `rdx`, `r8`, `r9`, and `r10`; dispatch retains its
original six argument indexes. All twelve values are restored before `SYSRETQ`.
This supplies the preserved general-register input required by a later thread
switch, but the current handler does not select a different return context.
The entry frame has one `repr(C)` Rust representation with the same twelve-word
order as the assembly pushes. Clone construction persists all twelve words plus
RIP, RSP, RFLAGS, RAX, and FS base in the generation-tagged child slot. A
checked lookup reconstructs the state only for a matching live handle and
rejects zeroed, retired, or stale-generation contexts. Cooperative yield and
child exit can install the selected frame for `SYSRETQ`.
An internal installer now couples that checked lookup to the thread-state and
FS/copy-context transition. It performs every fallible lookup and MSR update
before copying the target's registers, RIP, RSP, RFLAGS, and RAX into the output
frame. Child installation moves TLS copy admission to the child; restoring the
parent reverses it. A stale handle leaves both output frame and running identity
untouched. The clone, yield, futex-wait, and exit paths apply the same operation
to the live assembly return frame.
The bounded `sched_yield` proof now exercises the live form of this operation.
The handler receives a mutable pointer to the exact assembly-owned typed frame,
revalidates and reinstalls the current thread's TLS context, rewrites all twelve
registers and the RIP/RSP/RFLAGS return globals while interrupts remain
disabled, verifies the written state, and returns through `SYSRETQ`. The ring-3
workload continuing afterward proves the live round trip. This milestone still
selects only the caller on profiles without sibling resources.
For the interpreted profile, syscall 56 accepts only the exact bounded pthread
flag set, fixed sibling TLS pointer, guarded stack top, and distinct aligned
parent- and child-TID words in the authenticated writable mapping. It publishes
the next monotonic TID to both words and returns that same value to the parent;
the child context retains a zero return. Each selected child verifies that a
live `gettid()` returns its published child-TID value and that `FS:[0]` names
its own sibling TLS/TCB before continuing. A parent-to-child, child-to-parent,
and second parent-to-child `sched_yield` sequence proves bidirectional
cooperative selection before the child's matching `FUTEX_WAIT` saves it and
restores the parent. An explicit wake readies the
child; the parent's matching wait then selects it and resumes the child wait
with zero. Child `exit(42)` clears only the child-TID word, wakes the matching
parent, preserves the parent-TID word, retires and scrubs only the child slot,
and returns the parent through the same live-frame path. The parent wait also
resumes with zero. The complete sequence repeats once through the same slot;
the second reservation requires a scrubbed stack and a larger monotonic TID.
`exit_group(42)` remains final process teardown. More than two sequential
children, parallel execution, timer preemption, and general Linux-thread
scheduling are undefined.
Before its first successful clone, the live syscall path rejects malformed
flags with `-EINVAL`, retires the provisional child transaction, and then
successfully reserves the same slot. After both children retire and the main
RELRO page is sealed, an inaccessible parent-TID target returns `-EFAULT`;
final teardown proves that failure also retires its provisional transaction.
While the first child is ready between its two parent selections, a second
otherwise-valid syscall 56 request returns `-EAGAIN`. That rejection does not
change either published TID word or replace the live child context.
Before `getpid`, the probe also invokes `getuid`, `getgid`, `geteuid`, and
`getegid`; each resolves the current process's live user-namespace handle and
returns zero. The initial namespace maps that container ID to outside ID
100000. These are Linux-container credentials only. They do not select a native
domain, add a capability, or authorize a kernel control operation. `getresuid`
and `getresgid` copy real/effective/saved triples from the same immutable
namespace using Linux's ordered `put_user` semantics: a later pointer fault
returns `-EFAULT` after preserving any earlier successful writes. `rax = 115`
(`__NR_getgroups`)
reports one immutable supplementary group, container-local GID 0. A zero-sized
query returns the required count without accessing its pointer, a negative size
returns `-EINVAL`, and a full four-byte destination is required before copying.
`rax = 116` (`__NR_setgroups`) always returns `-EPERM`, including for an empty
list, because this profile's container credentials are immutable. `rax = 135`
(`__NR_personality`) reports `PER_LINUX` for the query value `0xffffffff` and
accepts an idempotent `PER_LINUX` selection. Every flag and alternate execution
domain returns `-EINVAL`, so a process cannot weaken the fixed execution policy
through personality state. `rax = 125` (`__NR_capget`) accepts Linux capability
ABI v3 for PID 0 or 1 and reports two all-zero capability records. A null data
pointer performs a version/PID query, invalid output returns `-EFAULT`, and an
unknown version is replaced with v3 before returning `-EINVAL`. `rax = 126`
(`__NR_capset`) accepts only an idempotent all-zero v3 set for the current
process; nonzero capabilities or another PID return `-EPERM`. Container UID 0
therefore carries no Linux capabilities and still grants no native kernel
authority. `/proc/1/uid_map` and `/proc/1/gid_map` each contain
`0 100000 1\n`; `/proc/1/setgroups` contains `deny\n`. All are immutable,
read-only, and non-mappable. The fork child inherits the exact generation-bound
namespace handle and verifies UID and GID 0 before exit. The bounded privileged
surface classifies credential mutation, root and mount changes, kernel/module
control, namespace control, raw I/O/performance access, and BPF loading. With
the namespace's empty privilege mask each returns `-EPERM` before inspecting
request pointers. PID 1 proves representative calls fail, an unknown syscall
still returns `-ENOSYS`, and credentials plus mount state remain unchanged;
the complete list is specified in
[Linux Privilege Boundary v0](linux-privilege-boundary-v0.md). The initial
process namespace has one visible process and one
runnable thread. `getpid` returns the fixed process ID 1, while `gettid`
resolves the running entry in the bounded generation-tagged thread table and
returns its TID 1. Internal lifecycle verification temporarily reserves TIDs 2
and 3 but retires them before ring-3 entry, so they are never userspace-visible
or runnable. `getppid` returns 0. `getpgrp`, `getpgid`, and `getsid` query the
live process record. PID 1 starts as process-group and session leader 1, and
forked children inherit both IDs. `setpgid` accepts the caller or its direct
child, applies Linux zero-argument substitutions, rejects negative PID
encodings, requires an existing group in the same session unless the target
creates a group named for itself, and rejects session leaders or cross-session
changes. `setsid` rejects an existing process-group leader and otherwise makes
the caller leader of a new session and process group. Unknown processes return
`-ESRCH`; authority and session violations return `-EPERM`. `rax = 157`
(`__NR_prctl`) supports `PR_SET_NAME` and
`PR_GET_NAME` for this sole thread. The 16-byte name starts as `init`, setters
copy at most 15 bytes and always retain a trailing NUL, and getters atomically
copy all 16 bytes. Invalid user pointers return `-EFAULT`, unknown operations
return `-EINVAL`, and the proof restores `init` before exit so teardown can
verify process-local state. Other `prctl` policy and security controls are not
yet implemented, except for `PR_SET_NO_NEW_PRIVS` and `PR_GET_NO_NEW_PRIVS`.
The former accepts only value 1 with every unused argument zero and
irreversibly sets the process bit; the latter accepts only zero unused
arguments and reports it. Rejected calls return `-EINVAL` without changing the
bit. The proof requires it to remain set through exit. Future exec and
credential-mutation paths must honor this boundary before they are admitted.
Next, `rax = 218` (`__NR_set_tid_address`) registers a
clear-child-TID pointer without dereferencing it and returns the current table
entry's TID 1. The proof registers an unmapped pointer successfully, replaces
it with a writable word, and observes exit zero that word and clear the pointer
from the thread's slot. As on Linux, exit ignores a fault while clearing an
invalid registered pointer. The bounded
clone child uses the same per-slot state; its exit also wakes the one parent
blocked on that exact address. `rax = 273`
(`__NR_set_robust_list`) registers an arbitrary 24-byte robust-list head pointer
in the current generation-tagged thread slot without dereferencing it, matching
Linux; other lengths return `-EINVAL`. `rax = 274`
(`__NR_get_robust_list`) reports the retained head and length for PID 0 or any
live thread visible in the domain; unknown and negative TIDs return `-ESRCH`.
Linux's ordered output behavior is preserved: the eight-byte size is written
before the head pointer, and remains written when a bad head destination then
returns `-EFAULT`. A bad size destination leaves the head untouched. Exit
walks up to Linux's 2048-node limit, handles `list_op_pending`, marks matching
ordinary futex owners with `FUTEX_OWNER_DIED`, preserves `FUTEX_WAITERS`, and
wakes one waiter. User-memory faults terminate cleanup without failing exit.
Priority-inheritance robust futex recovery remains pending, so
`set_robust_list` remains Partial. Internal TID lifecycle testing assigns distinct
clear-child-TID and robust-list sentinels to two slots, proves they follow the
current-thread selection, and proves retirement clears them before slot reuse.

`rax = 334` (`__NR_rseq`) records per-thread registration pointer, length, and
signature. It accepts the original aligned 32-byte ABI and Linux v6.12 extended
lengths with four-byte alignment, validates the entire declared writable range,
and publishes CPU 0, NUMA node 0, and memory-map concurrency ID 0. Repeating an
identical registration returns `-EBUSY`; pointer or length changes return
`-EINVAL`, while a signature mismatch returns `-EPERM`. Matching unregister
resets `cpu_id_start`, `cpu_id`, `node_id`, and `mm_cid` in Linux field order
before clearing the saved registration. Argument narrowing, alignment, range,
and lifecycle cleanup are runtime-tested. Preemption/signal critical-section
fixup and migration behavior remain pending, so the syscall remains Partial.

`rax = 202` (`__NR_futex`) supports a bounded one-waiter lifecycle. Aligned,
readable `FUTEX_WAIT` and `FUTEX_WAIT_PRIVATE` compare one 32-bit word and return
`-EAGAIN` when it differs from the expected value. When the fixed clone child is
ready, a match saves the current frame, records the wait address in that
generation-tagged slot, marks it blocked, and selects the peer. `FUTEX_WAKE`
and `FUTEX_WAKE_PRIVATE` move up to the requested number of exact address
matches back to ready state. A null timeout waits indefinitely within this
bounded lifecycle. A valid zero relative timespec returns `-ETIMEDOUT`
immediately. A positive relative timespec is converted to a checked monotonic
deadline in the typed wait record; the first Linux syscall or scheduling
boundary at or after expiry clears that record and makes the waiter ready with
`-ETIMEDOUT`. A periodic local-APIC tick can perform that transition and dispatch
the waiter in interrupt context. The proof blocks the child and explicitly wakes it, expires a
parent timed wait, then blocks the parent before child clear-child-TID performs
the final wake after zeroing the word. Explicitly woken waits return zero. A
matching wait with no other ready thread returns `-ENOSYS` instead of
deadlocking. Invalid operations, alignment, or timespec fields return
`-EINVAL`, and invalid word or timeout ranges return `-EFAULT`. Bitsets,
requeueing, priority inheritance,
multiple waiters, and cross-process keys remain undefined.

`rax = 10` (`__NR_mprotect`) supports an irreversible transition on either the
page named by the parsed `PT_GNU_RELRO` range or a page-aligned scratch address
plus a nonzero length rounding to one through four active pages. The selected
pages become read-only/NX. Sealing a subrange rebuilds independent registry
runs and preserves every other page's write permission and file-kind metadata.
The RELRO seal lasts for the process lifetime; an anonymous seal
lasts until unmap, and a later mapping starts from freshly scrubbed RW/NX
frames. The rounded range must be present, its address must be page-aligned,
and the only accepted protection is `PROT_READ`. A range can span independent
adjacent writable entries; holes, mixed read-only entries, and all other
requests return `-EINVAL`. Before changing any page, the kernel
validates ownership and writable/NX permissions for every leaf. It then clears
every selected writable bit and invalidates every selected translation before
resuming userspace. The probe confirms that copy-in remains available while
cross-page `uname` copy-out returns `-EFAULT`, repeated sealing is rejected,
head- and tail-page seals preserve write access only on the unselected page, a
read-only area can still be unmapped, and remapping restores writable state
without prior contents. This is a bounded write seal, not a general
memory-management API. The probe
finally uses `rax = 231`
(`__NR_exit_group`) with `rdi = 42` (status).
Final process exit closes every open descriptor in the bounded table before
retiring the last thread. A non-leader thread's `exit` does not close the
process-shared descriptor table.
The separate bounded process-lifecycle contract records generation-checked
process identities, parentage, exclusive address-space-root ownership,
two-phase child publication, exit-to-zombie ordering, and parent-only reap.
Each running record holds an allocator-issued bundle of four through six
hierarchy leases and one through 41 private-page mapping records. Each
record binds a virtual address, generation-checked frame, and W^X permissions;
a complete page-table walk must match the exact inventory. Publication rejects
metadata substitutions and any cross-process frame overlap. Exit enters a
non-runnable intermediate state and cannot publish a zombie until the allocator
no longer tracks any lease in the exact detached bundle. Its dedicated proof
gate resumes the cloned child after validating its
complete saved frame, captures a subsequent ring-3 syscall number 60 and status
42, and uses that captured status for zombie publication and parent wait. The
live runtime now snapshots and completely validates its generation-bound image,
stack, heap, TLS, sibling-stack, and `mmap` ownership before normal exit. After
terminal ring-3 return, it transactionally clones that exact inventory into an
independent child record, checks every copied page byte and hierarchy mapping,
then tears down and reaps the child while restoring the allocator count. One
child is selected through the native scheduler and enters ring 3 under its
cloned root at a private bounded trampoline. Its complete fork-derived context
and FS base are checked before its captured `exit(42)` drives zombie publication
and reap; the parent code remains unchanged. The synthetic process exit proof
remains as an independent lower-level check.
The live exit paths additionally translate the real syscall entry frame into
the process model's complete register order and admit it with the snapshot as
parentless PID 1. A canonical FS base is preserved even when unmapped, matching
Linux `arch_prctl` behavior; page access still requires an ordinary mapping.
The validated PID 1 record is retained in non-stack kernel storage and refreshed
from the exact address-space and return context at every leader syscall entry.
Cleanup requires multiple successful refreshes and refuses any child or
partially retiring record. The exact bounded `clone` below remains thread-only;
the separate process path exposes syscall 57 (`fork`) and syscall 61 (`wait4`)
for the minimal PID 1 profile.
The process model's eager-copy helper can duplicate up to 41
generation-checked private physical pages without aliasing. Its companion
hierarchy transaction requires a complete bounded inventory of user leaves,
copies and rewires four through six generation-checked page-table frames, and
maps each private child page with the parent's W^X permissions. The internal
fork transaction obtains that inventory from the prepared child's recorded
parent, performs both operations, revalidates the resulting child hierarchy,
derives a child register/TLS context with `RAX = 0`, and atomically publishes
the context and child bundle. Context publication requires its RIP, stack, FS
base, and privilege flags to agree with the recorded mappings. A synthetic
proof admits the parent and
child contexts to the native scheduler, selects both under their exact roots,
loads and restores the child's CR3 in kernel mode, and retires their
generation-tagged bindings. The selected child also executes a bounded ring-3
payload that checks `RAX = 0`, writes through FS into private memory, and
returns through the user gate. The entry stub restores all fifteen general
registers and the gate captures and compares the complete frame.

The public `fork` handler accepts only PID 1's leader and defers the expensive
clone until the syscall frame has unwound to the bootstrap stack. The child is
selected through the native scheduler and resumes the real post-syscall RIP
under its private CR3 with `RAX = 0`; PID 1 later resumes with the child PID.
The bounded child observes process identity 2 and parent identity 1, then
`exit(42)` records its eight-bit status and returns to the kernel before its
active address space is released. `wait4` accepts only the exact positive child
PID, zero options, and a null rusage pointer. It validates a non-null four-byte
status output before reaping, writes `42 << 8`, and returns PID 2. Invalid
options return `-EINVAL`, an invalid status pointer returns `-EFAULT`, and a
missing or already reaped child returns `-ECHILD`. `waitid` admits `P_ALL` and
positive `P_PID` with `WEXITED`, optionally combined with `WNOHANG` and
`WNOWAIT`. It produces a fully initialized `SIGCHLD`/`CLD_EXITED` `siginfo_t`,
supports non-consuming observation, zeroes the result for a running child under
`WNOHANG`, and validates optional zeroed rusage before consuming a zombie.
Blocking on a running child, group and pidfd selectors, stopped/continued
states, nonzero accounting, general process-style `clone`, and concurrent
parent/child execution remain unsupported.
Immediately before entry, the kernel:

- saves `EFER`, `STAR`, `LSTAR`, and `FMASK`;
- programs `STAR` for the existing kernel and user GDT selectors;
- points `LSTAR` at a dedicated low-level entry stub;
- configures `FMASK` to clear IF, DF, and AC; and
- sets `EFER.SCE`, reading every programmed MSR back before entering CPL3.

Unlike an interrupt gate, `SYSCALL` does not switch stacks. The entry stub
therefore disables interrupts, clears AC and DF, captures the user RSP, and
switches immediately to the kernel stack saved by the ring-3 entry trampoline
before calling Rust. The bounded handler requires RSP to remain anywhere in
the owned one-page stack mapping between its two unmapped guards, a return RIP
inside the RX extent, and user flags without IOPL or VM while an atomic
boot-probe flag is armed. The proof invokes `getpid` through a nested function
call, so the captured RSP differs from its initial value while remaining in
the owned page. `write` and `getpid` return through
`SYSRETQ`. Before Rust dispatch, the assembly stub saves `rdi`, `rsi`, `rdx`,
`r8`, `r9`, and `r10`; it restores them before resumption, leaving only the
architecturally clobbered `rax`, `rcx`, and `r11` changed. The resumed code
checks the result plus sentinel values in `rdi` and `r10`, then invokes exit.
Exit returns directly to the saved kernel context. The verifier restores and
reads back all four syscall MSRs plus FS base before any later control-plane
workload executes.

This milestone proves the hardware transition and Linux argument registers. It
does not provide persistent per-thread kernel stacks, general signal
interruption, restartable syscalls, scheduler integration, mutable credentials,
general user-namespace creation, credential-based permissions, a general
file-descriptor table, arbitrary
user buffers,
general thread allocation, general TLS state, or any syscall other than the exact bounded
`read`, `pread64`, `write`, `readv`, `writev`, bounded
`poll`/`ppoll`/`epoll`, `close`, `dup`, `dup2`, bounded `fcntl`, `fstat`,
bounded device/runtime-file `openat`, `lseek`, console `ioctl`, `brk`, `uname`,
`getcwd`, `chdir`, `fchdir`, bounded
`readlink`/`readlinkat`,
bounded anonymous and immutable-file `mmap`/`munmap` plus bounded
`madvise`/`mincore`, bounded read-only `mprotect`,
`rt_sigaction`, `rt_sigprocmask`, bounded synchronous `rt_sigreturn`,
`sigaltstack`, `umask`, read-only
`getrlimit`/`prlimit64`,
bounded one-waiter `futex`,
`sched_yield`, bounded scheduler-policy queries, complete scheduler priority
range and single-CPU affinity queries/updates, `getcpu`,
bounded `clock_gettime`/`clock_getres`/`times`/`getrusage`, disabled
`getitimer`/`setitimer`, bounded `nanosleep`,
`arch_prctl(ARCH_SET_FS/ARCH_GET_FS)`,
credential, supplementary-group, and capability queries, fixed `personality`, bounded `prctl` process names, the bounded process/session
identity queries above,
`set_tid_address`, bounded robust-list registration/query, the exact bounded
`clone`, `execve`, `execveat`, `exit`, and `exit_group` calls. `clone` requires
`CLONE_VM|CLONE_FS|CLONE_FILES|CLONE_SIGHAND|CLONE_THREAD|CLONE_SYSVSEM|CLONE_SETTLS|CLONE_PARENT_SETTID|CLONE_CHILD_CLEARTID|CLONE_CHILD_SETTID`,
the fixed sibling stack and TLS resources, and distinct writable aligned parent-
and child-TID words. It publishes the TID to both words and returns it to the
parent. Explicit wake first resumes a blocked child; a second matching wait
runs it again, and child exit clears its word, wakes the blocked parent, and
resumes that wait with zero. This handshake repeats for a second monotonic TID
through the scrubbed slot. There is no general TID allocation, arbitrary or
parallel thread creation, general futex queue, or robust futex recovery yet.
`rax = 335` (`__NR_uretprobe`) follows Linux's direct-call boundary. Ginux does
not install return uprobes yet, so a userspace invocation generates `SIGILL`
with `SI_KERNEL`; a registered Linux signal action can receive it, and the
default action terminates the domain init process. The kernel-only trampoline
path and return-probe consumers remain unsupported.

`rax = 317` (`__NR_seccomp`) supports Linux strict mode. Enabling it is
irreversible for the calling process; afterward only `read`, `write`, `exit`,
and `rt_sigreturn` are admitted, and any other syscall terminates the process
with `SIGKILL`. `SECCOMP_GET_ACTION_AVAIL` reports `SECCOMP_RET_KILL_THREAD`,
the action used by this profile. BPF filter mode, synchronization across a
general thread group, logging, tracing, and user notification remain
unsupported and fail explicitly rather than silently weakening a filter.

`rax = 216` (`__NR_remap_file_pages`) recognizes Linux's deprecated nonlinear
file-remapping ABI and validates its address, size, protection, and page-index
contract. All VMAs Ginux currently creates are ineligible: file mappings are
private and shared mappings are anonymous, so Linux-compatible `EINVAL` is
returned. Successful nonlinear `MAP_SHARED` file remapping remains pending.

`rax = 248..250` (`add_key`, `request_key`, and `keyctl`) provide per-domain,
frame-backed `user` and `logon` keys. Process, session, and user keyring
selectors are recognized; keys receive monotonic serials and support lookup,
payload read/update, description, revoke/invalidate, and ring clear. Payloads
are scrubbed when their domain is torn down. The current one-frame payload
profile is bounded to 3760 bytes. General key types, nested keyring links,
permission mutation, request-key upcalls, quotas, expiry, and asymmetric-key
operations remain unsupported.

`rax = 323` (`userfaultfd`) creates an anonymous nonblocking descriptor when
`UFFD_USER_MODE_ONLY` is requested, with Linux `O_CLOEXEC`/`O_NONBLOCK`
semantics. Calls without that flag fail with `EPERM`, matching a Linux host
that disables unprivileged kernel-fault handling for a caller without ptrace
authority. `UFFDIO_API` negotiates API `0xaa`; the empty event stream returns
`EAGAIN`. Range registration, page-fault events, copy/zeropage/write-protect,
and fork/remap/remove events remain unsupported and their ioctls are rejected.

`rax = 300..301` (`fanotify_init` and `fanotify_mark`) provide one notification
group per domain with close-on-exec/nonblocking descriptor behavior. Exact-file
`FAN_MARK_ADD`, `FAN_MARK_REMOVE`, and `FAN_MARK_FLUSH` operations support
`FAN_MODIFY`; successful overlay writes queue Linux v3 fanotify metadata with
an owned descriptor for the affected file. Reads do not generate modification
events. Multiple groups, directory/mount/filesystem marks, permission events,
FID reporting, unlimited queues/marks, and other event classes remain pending.

`rax = 444..446` (`landlock_create_ruleset`, `landlock_add_rule`, and
`landlock_restrict_self`) expose Linux's disabled-LSM boundary and return
`EOPNOTSUPP`, including the ABI-version query. This matches Linux when the
optional Landlock security module is not enabled at boot. Ginux does not
advertise a Landlock ABI until every access right in that ABI can be enforced;
security-module selection remains an explicit host policy decision.

`rax = 425..427` (`io_uring_setup`, `io_uring_enter`, and
`io_uring_register`) provide a bounded shared-ring ABI. Setup rounds up to at
most 64 submission entries, publishes Linux SQ/CQ offsets, and exposes the
three standard mmap offsets for SQ, CQ, and SQEs. Enter synchronously consumes
SQ indices and publishes CQEs with acquire/release-visible head/tail updates.
`IORING_OP_NOP`, `READ`, `WRITE`, `READV`, `WRITEV`, and `FSYNC` use the same
validated descriptor, VFS, and socket paths as their syscall equivalents;
unsupported operations complete with `-EINVAL`. `IORING_REGISTER_PROBE`
publishes operation records through `IORING_OP_WRITE`. SQPOLL/IOPOLL,
registered resources, linked requests, cancellation, personalities, multiple
rings, positioned I/O, and true asynchronous execution remain pending.

Every other call
returns `-ENOSYS` without disarming
the task, so a caller can handle the error and continue. The entire path
remains disarmed outside the proof. The current copy helpers cover only the
declared main/interpreter, TLS, stack, bounded heap, and active mapping areas.
They walk adjacent areas only when every byte carries the requested permission,
so holes, RELRO, guards, and arithmetic overflow fail closed. General VM-area
registration, arbitrary mappings, and fault-driven copies remain future work.
Invalid `write` descriptors return `-EBADF`, unsupported lengths return
`-EINVAL`, and a `write` or `uname` range outside that segment returns
`-EFAULT`. Boot verification covers the exact lower and upper boundaries, the
first byte beyond the segment, and address-addition overflow. The ring-3 probe
also submits fd 2, a 257-byte write, and `uname` at the first mapped address
beyond `p_memsz`; it requires the three errors above and then completes the
valid syscall sequence, proving errors do not silently terminate the task.
