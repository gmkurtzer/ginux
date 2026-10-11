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
length. A zero-length request or one above the limit returns `-EINVAL`. Next,
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
ready descriptor also returns immediately regardless of timeout. A query with
no ready descriptors and a nonzero or infinite timeout returns `-ENOSYS`
without changing the table because scheduler-backed descriptor wait queues do
not exist yet.
`rax = 271` (`__NR_ppoll`) uses the same bounded readiness engine. A null
timeout means an infinite wait request; otherwise the complete 16-byte x86-64
`timespec` must contain nonnegative seconds and normalized nanoseconds. An
optional eight-byte signal mask and exact signal-set size are validated before
the query, but no persistent mask change occurs because this implementation
never blocks. Zero timeouts and immediately ready descriptors behave like
`poll`; a call that would wait returns `-ENOSYS`. Invalid timespecs or mask
sizes return `-EINVAL`, invalid input ranges return `-EFAULT`, and the timeout
is not rewritten.
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
atomically copies it only when the registered descriptor is immediately
ready. A zero-timeout query with no event returns zero; a nonzero or infinite
wait returns `-ENOSYS` until scheduler-backed wait queues exist. Invalid output
memory returns `-EFAULT` without consuming the level-triggered event. The epoll
descriptor participates in `F_GETFD`, `F_SETFD`, `F_GETFL`, `poll`, and
`close`; closing it clears the registration and releases its descriptor.
`rax = 281` (`__NR_epoll_pwait`) uses the same bounded wait operation after
validating an optional eight-byte signal mask and its exact signal-set size.
The temporary mask is not installed because this implementation never blocks;
invalid sizes return `-EINVAL` and invalid mask ranges return `-EFAULT` before
examining readiness.
`rax = 441` (`__NR_epoll_pwait2`) replaces the integer timeout with a null or
complete 16-byte x86-64 `timespec`. Negative seconds and non-normalized
nanoseconds return `-EINVAL`; invalid memory returns `-EFAULT`. A zero timespec
queries immediately, while a nonzero or null timeout that finds no ready event
retains the explicit `-ENOSYS` blocking boundary.
This is a bounded event-notification proof, not a general epoll interest tree.
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
`UINT64_MAX - 1` returns `-EAGAIN`; the corresponding blocking operation
returns `-ENOSYS` until descriptor wait queues exist. Invalid lengths return
`-EINVAL` and invalid memory returns `-EFAULT` without consuming or adding a
value. `poll` and epoll report `POLLIN`/`EPOLLIN` while nonzero and
`POLLOUT`/`EPOLLOUT` while another value can be added. The proof registers the
counter in the simultaneous epoll descriptor, observes readiness appear after
a write, preserves readiness across a failed read, drains it, verifies
full-counter backpressure, and closes both descriptors.
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
`-EAGAIN`; a corresponding blocking read returns `-ENOSYS` until descriptor
wait queues exist. `poll` and epoll expose `POLLIN`/`EPOLLIN` dynamically once
the monotonic deadline has elapsed. The descriptor also participates in
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
capacity returns `-EAGAIN` in nonblocking mode or `-ENOSYS` without wait
queues, and larger writes return `-EINVAL`. Reads return up to the requested
count and consume only after successful copy-out. An empty live pipe follows
the same nonblocking or would-block errors; closing the writer makes an empty
read return EOF and reports `POLLHUP`, while buffered bytes remain readable.
Closing the reader makes writes return `-EPIPE` and reports `POLLERR`; SIGPIPE
delivery is not yet implemented. The endpoints participate in `F_GETFD`,
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
sequence remains available to a valid retry.
`rax = 79` (`__NR_getcwd`) copies the initial container working directory `/`
and its terminator to a writable VMA and returns 2. A one-byte buffer returns
`-ERANGE`; an out-of-range destination returns `-EFAULT`. This is a bounded
identity for the initial root view, not a general VFS or directory state.
`rax = 80` (`__NR_chdir`) accepts exactly `/`, preserving that fixed root view.
Other paths return `-ENOENT`, and an invalid or unterminated pathname returns
the bounded pathname-copy error. No general directory lookup or mutable current
directory state is implied.
`rax = 89` (`__NR_readlink`) recognizes exactly `/proc/self/exe`, copied as a
bounded NUL-terminated pathname, and returns `/init` without a terminator. A
zero output size returns `-EINVAL`; invalid pathname or output ranges return
`-EFAULT`. Other paths return `-ENOENT`. This is not a general procfs.
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
`faccessat` and `faccessat2` report read/write access for either fixed node
and return `-EACCES` for execute access. `faccessat2` accepts only zero flags;
unknown mode bits or flags return `-EINVAL`, with the same path errors as
`openat`.

The immutable runtime namespace also recognizes `/init`,
`/lib64/ld-ginux.so`, `/lib64/libginux.so`, and `/lib64/libginux-aux.so`.
These names expose the exact build-authenticated ELF byte slices used by the
execution or bounded dependency-graph proofs, not
copies or mutable storage. `openat` accepts only `O_RDONLY` with optional
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
and closes the descriptor. There is no `/lib64` directory enumeration,
pathname traversal, general VFS, or userspace `execve`; the kernel authenticates
and plans the fixed interpreter, main, and two-DSO executable graph. The kernel
installs the interpreter needed to bootstrap ring 3; that interpreter installs
the main and both dependencies' admitted segments through the fixed mapping
contract below.
`rax = 97` (`__NR_getrlimit`) and `rax = 302` (`__NR_prlimit64`) expose immutable
limits for `RLIMIT_STACK` and `RLIMIT_NOFILE`. The stack soft and hard limits
are one page, matching the bounded mapped stack; the descriptor limits are 4,
matching the highest descriptor number the profile can accept plus one.
`prlimit64` accepts PID 0 or 1 and an optional old-limit destination. Other
PIDs return `-ESRCH`, unsupported resources return `-EINVAL`, and any non-null
new-limit pointer returns `-EPERM`. Output is copied only after the full
16-byte `rlimit` range passes writable-VMA validation, so a range crossing the
declared ELF extent returns `-EFAULT` without a partial write. The syscall
entry uses an internal value outside Linux's `-1..-4095` errno range to return
to the kernel, keeping Linux `-EPERM` observable in ring 3.
`rax = 204` (`__NR_sched_getaffinity`) accepts PID 0 or 1, requires an
eight-byte-or-larger caller mask, writes the one-bit CPU 0 mask, and returns 8.
An unknown PID returns `-ESRCH`, an undersized mask returns `-EINVAL`, and an
invalid output range returns `-EFAULT` without a partial write.
`sched_setaffinity` accepts only an idempotent CPU-0 mask with the same PID and
size rules; a different mask returns `-EINVAL`, and an invalid input returns
`-EFAULT`. `rax = 309`
(`__NR_getcpu`) reports CPU 0 and NUMA node 0 through independently optional
four-byte pointers. Both destinations are validated before either is written;
the obsolete cache pointer is ignored as on Linux. These results describe the
single-bootstrap-processor profile and do not claim SMP scheduling support.
`rax = 24` (`__NR_sched_yield`) selects the sole ready sibling when one exists
and otherwise succeeds as a no-op. `sched_getscheduler` reports
`SCHED_OTHER` for PID 0 or 1, and `sched_getparam` copies its priority of zero
only after validating the complete output. `sched_get_priority_min` and
`sched_get_priority_max` both report zero for `SCHED_OTHER`. Unknown PIDs
return `-ESRCH`, an invalid output returns `-EFAULT`, and unsupported policies
return `-EINVAL`. `sched_setparam` and `sched_setscheduler` accept only an
idempotent priority-zero `SCHED_OTHER` request; invalid pointers return
`-EFAULT`, and other policies return `-EPERM`. `rax = 140`
(`__NR_getpriority`) reports nice value zero for the sole process by returning
the raw Linux syscall value 20. It accepts only `PRIO_PROCESS` and PID 0 or 1;
other selectors return `-EINVAL` and an unknown PID returns `-ESRCH`.
`setpriority` accepts only an idempotent request for nice value zero; an actual
priority change returns `-EPERM`.
`rax = 228` (`__NR_clock_gettime`) exposes `CLOCK_MONOTONIC` and
`CLOCK_MONOTONIC_RAW` from an ordered bootstrap-processor TSC. The conversion
uses a CPUID-reported frequency when available and the PIT-calibrated fallback
otherwise. Both clocks share a private process epoch, return a normalized
16-byte x86-64 `timespec`, and cannot move backward during this single-CPU
proof. Other clock IDs return `-EINVAL`; an invalid output range returns
`-EFAULT` without changing its valid prefix. Realtime, CPU clocks, sleeping,
adjustment, and a vDSO remain unsupported.
`rax = 229` (`__NR_clock_getres`) exposes the corresponding single-tick
resolution, rounded up to a normalized `timespec`. A null result pointer is a
valid support query. Clock selection and copy errors match `clock_gettime`, and
the complete result is validated before copy-out.
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
`rax = 36` (`__NR_getitimer`) reports each of `ITIMER_REAL`, `ITIMER_VIRTUAL`,
and `ITIMER_PROF` as a zero-initialized, disabled 32-byte x86-64 `itimerval`.
`rax = 38` (`__NR_setitimer`) accepts an all-zero value to idempotently disarm
one of those timers and optionally returns its previous disabled value. Arming
a timer returns `-ENOSYS` until asynchronous signal delivery exists. Invalid
selectors return `-EINVAL`; invalid complete input or output ranges return
`-EFAULT`, and rejected requests do not partially change the old-value output.
`rax = 9` (`__NR_mmap`) has a scratch-window profile and one authenticated
executable-install profile. The scratch window admits one private one- or
two-page extent at kernel-selected address `0x40b000`. Its anonymous form requires a null hint, an
exact 4096- or 8192-byte length, `PROT_READ | PROT_WRITE`, `MAP_PRIVATE |
MAP_ANONYMOUS`, fd -1, and offset zero, and installs zero-filled RW/NX frames.
The immutable-file form requires `PROT_READ`, `MAP_PRIVATE`, a readable
descriptor for `/init`, `/lib64/ld-ginux.so`, `/lib64/libginux.so`, or
`/lib64/libginux-aux.so`, and a
page-aligned offset. Its complete one- or two-page window must fit within the
file length rounded up to a page; offsets at or beyond EOF are rejected rather
than deferred to a `SIGBUS` path. The kernel copies the authenticated bytes
into allocator-owned read-only/NX frames and zero-fills a partial final page.
Descriptor cursors are unaffected. Unsupported arguments return `-EINVAL`, a
non-file descriptor returns `-ENODEV`, an invalid descriptor returns `-EBADF`,
and a request while the sole extent is occupied returns `-ENOMEM`.
The executable-install profile accepts only open `/init`,
`/lib64/libginux.so`, or `/lib64/libginux-aux.so` descriptors and exact one-page `MAP_PRIVATE |
MAP_FIXED_NOREPLACE` requests already authenticated by the kernel plan. For
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
`rax = 11` (`__NR_munmap`) accepts only the complete active scratch extent, clears its
hardware leaves, invalidates their TLB entries, and scrubs the physical frames
before reuse. Repeating a supported exact unmap succeeds. The anonymous probe
crosses the two-page boundary with syscall copy-out, while the file proof maps
the complete two-page interpreter, observes ELF magic and its entry byte,
checks the zero-filled tail, proves syscall copy-out is denied, and rejects an
unaligned file offset. Both paths exit with both leaves absent. This is not a
general VMA allocator: hints, fixed mappings, writable or shared file
mappings outside the admitted executable pages, partial ranges, arbitrary
files, demand paging, and mapping growth are unsupported. Fixed placement is
available only for the seven admitted main pages and two admitted dependency
pages described above.
`rax = 28` (`__NR_madvise`) accepts only `MADV_DONTNEED` for the exact active
anonymous extent. Immutable-file mappings reject advice with `-EINVAL`. For an
anonymous mapping it validates that every leaf still maps its allocator-owned
frame, then zeros the frames through the supervisor direct map without changing
their present, user, NX, or writable state. The proof also discards a sealed
one-page read-only mapping, observes zeroes from ring 3, and unmaps it normally.
Other advice, ranges, or inactive mappings return `-EINVAL` without changing
the contents. There is no paging policy, reclamation queue, or lazy refill.
`rax = 27` (`__NR_mincore`) accepts either kind of exact active extent and
atomically writes one resident byte per page to a fully validated vector. It returns
`-ENOMEM` after unmap. Misaligned, partial, or different ranges return
`-EINVAL`; an invalid vector returns `-EFAULT`. It exposes only the fixed slot,
not arbitrary page-table state or reclaim information.
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
mask for the sole thread. It supports `SIG_BLOCK`, `SIG_UNBLOCK`, `SIG_SETMASK`,
and a null-set query; `SIGKILL` and `SIGSTOP` are always cleared. The kernel
validates the set size and every complete input/output range before mutation,
so `-EINVAL` and `-EFAULT` leave the prior mask intact. The proof restores an
empty mask before exit. This is process state only: signal actions, generation,
delivery, pending sets, alternate stacks, and interrupted-syscall behavior are
not implemented yet.
`rax = 13` (`__NR_rt_sigaction`) stores and queries one 32-byte x86-64 action
record for each of signals 1 through 64. Complete action and old-action ranges
are validated before state changes; action masks also clear `SIGKILL` and
`SIGSTOP`. Installing handlers for those two uncatchable signals returns
`-EINVAL`. Handler, flags, and restorer values are retained as opaque ABI state
until delivery is implemented. The proof exercises default queries, normalized
registration, fault atomicity, and restoration of every action before exit.
`rax = 131` (`__NR_sigaltstack`) reports an initially disabled 24-byte x86-64
alternate-stack record. It accepts `SS_DISABLE` or a zero-flag stack of at least
2048 bytes wholly contained in one currently writable probe VMA. Invalid
records return `-EINVAL`, undersized stacks return `-ENOMEM`, and inaccessible
records or stack ranges return `-EFAULT` before state changes. The proof enables,
queries, rejects invalid replacements, and disables the stack before exit.
`SS_ONSTACK`, automatic disarming, and actual signal-stack entry remain
undefined until signal delivery exists.
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
While the first child is ready between its two parent selections, a second
otherwise-valid syscall 56 request returns `-EAGAIN`. That rejection does not
change either published TID word or replace the live child context.
Before `getpid`, the probe also invokes `getuid`, `getgid`, `geteuid`, and
`getegid`; each returns zero. These are Linux-container credentials only. They
do not select a native domain, add a capability, or authorize a kernel control
operation. `getresuid` and `getresgid` copy real/effective/saved triples of
zero. All three destinations are validated before any write, so an invalid
third pointer cannot leave a partial result. `rax = 115` (`__NR_getgroups`)
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
authority. The initial process namespace has one visible process and one
runnable thread. `getpid` returns the fixed process ID 1, while `gettid`
resolves the running entry in the bounded generation-tagged thread table and
returns its TID 1. Internal lifecycle verification temporarily reserves TIDs 2
and 3 but retires them before ring-3 entry, so they are never userspace-visible
or runnable. `getppid` returns 0, and `getpgrp`,
`getpgid(0)`, and `getsid(0)` return 1. `getpgid` and `getsid` return `-ESRCH`
for an unknown PID. `rax = 157` (`__NR_prctl`) supports `PR_SET_NAME` and
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
four-byte word inside the declared RW segment and returns the current table
entry's TID 1. The bounded
proof rejects an out-of-segment registration with `-EFAULT`. Exit zeroes the
registered word and clears the pointer from that thread's slot. The bounded
clone child uses the same per-slot state; its exit also wakes the one parent
blocked on that exact address. `rax = 273`
(`__NR_set_robust_list`) registers one writable 24-byte
robust-list head in the current generation-tagged thread slot. Other lengths return `-EINVAL`, and a head
crossing the declared writable extent returns `-EFAULT`. `rax = 274`
(`__NR_get_robust_list`) reports the retained head and length for PID 0 or the
current table entry's TID;
foreign PIDs return `-ESRCH`. Both eight-byte output ranges are validated before
either is changed. Exit discards the registration, but no robust futex traversal
or recovery is attempted. Internal TID lifecycle testing assigns distinct
clear-child-TID and robust-list sentinels to two slots, proves they follow the
current-thread selection, and proves retirement clears them before slot reuse.

`rax = 202` (`__NR_futex`) supports a bounded one-waiter lifecycle. Aligned,
readable `FUTEX_WAIT` and `FUTEX_WAIT_PRIVATE` compare one 32-bit word and return
`-EAGAIN` when it differs from the expected value. When the fixed clone child is
ready, a match saves the current frame, records the wait address in that
generation-tagged slot, marks it blocked, and selects the peer. `FUTEX_WAKE`
and `FUTEX_WAKE_PRIVATE` move up to the requested number of exact address
matches back to ready state. The proof blocks the child and explicitly wakes
it, then blocks the parent before child clear-child-TID performs the second wake
after zeroing the word. Both resumed waits return zero. A matching wait with no
other ready thread returns `-ENOSYS` instead of
deadlocking. Invalid operations or alignment return `-EINVAL`, and an invalid
word returns `-EFAULT`. Timeouts, bitsets, requeueing, priority inheritance,
multiple waiters, and cross-process keys remain undefined.

`rax = 10` (`__NR_mprotect`) supports one irreversible transition on either the
page named by the parsed `PT_GNU_RELRO` range or the complete active anonymous
extent: one or two exact pages may become read-only/NX. The RELRO seal lasts for
the process lifetime; an anonymous seal lasts until unmap, and a later mapping
starts from freshly scrubbed RW/NX frames. The normalized range must be
present, page-aligned, and exact, and the only accepted protection is
`PROT_READ`. All other requests return `-EINVAL`. Before changing any page, the
kernel validates ownership and writable/NX permissions for every leaf. It then
clears every writable bit and invalidates every translation before resuming
userspace. The probe confirms that copy-in remains available while cross-page
`uname` copy-out returns `-EFAULT`, repeated and partial sealing are rejected, a
read-only extent can still be unmapped, and remapping restores writable state
without prior contents. This is a bounded write seal, not a general
memory-management API. The probe
finally uses `rax = 231`
(`__NR_exit_group`) with `rdi = 42` (status).
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
does not provide persistent per-thread kernel stacks, `IRETQ` fallback, signal
interruption, restartable syscalls, scheduler integration, mutable credentials
or credential-based permissions, a general file-descriptor table, arbitrary
user buffers,
general thread allocation, general TLS state, or any syscall other than the exact bounded
`read`, `pread64`, `write`, `readv`, `writev`, bounded
`poll`/`ppoll`/`epoll`, `close`, `dup`, `dup2`, bounded `fcntl`, `fstat`,
bounded device/runtime-file `openat`, `lseek`, console `ioctl`, `brk`, `uname`,
`getcwd`, `chdir`, bounded
`readlink`/`readlinkat`,
bounded anonymous and immutable-file `mmap`/`munmap` plus bounded
`madvise`/`mincore`, bounded read-only `mprotect`,
`rt_sigaction`, `rt_sigprocmask`, `sigaltstack`, `umask`, read-only
`getrlimit`/`prlimit64`,
bounded one-waiter `futex`,
`sched_yield`, bounded scheduler-policy and priority queries, `sched_getaffinity`, `getcpu`,
bounded `clock_gettime`/`clock_getres`/`times`/`getrusage`, disabled
`getitimer`/`setitimer`,
`arch_prctl(ARCH_SET_FS/ARCH_GET_FS)`,
credential, supplementary-group, and capability queries, fixed `personality`, bounded `prctl` process names, the bounded process/session
identity queries above,
`set_tid_address`, bounded robust-list registration/query, the exact bounded
`clone`, `exit`, and `exit_group` calls. `clone` requires
`CLONE_VM|CLONE_FS|CLONE_FILES|CLONE_SIGHAND|CLONE_THREAD|CLONE_SYSVSEM|CLONE_SETTLS|CLONE_PARENT_SETTID|CLONE_CHILD_CLEARTID|CLONE_CHILD_SETTID`,
the fixed sibling stack and TLS resources, and distinct writable aligned parent-
and child-TID words. It publishes the TID to both words and returns it to the
parent. Explicit wake first resumes a blocked child; a second matching wait
runs it again, and child exit clears its word, wakes the blocked parent, and
resumes that wait with zero. This handshake repeats for a second monotonic TID
through the scrubbed slot. There is no general TID allocation, arbitrary or
parallel thread creation, general futex queue, or robust futex recovery yet.
Every other call
returns `-ENOSYS` without disarming
the task, so a caller can handle the error and continue. The entire path
remains disarmed outside the proof. The current copy helpers cover only the one
declared RW segment and the bounded heap below its current break; general
stack-page ranges are also accepted. General VM-area lookup and cross-area
copies remain future work.
Invalid `write` descriptors return `-EBADF`, unsupported lengths return
`-EINVAL`, and a `write` or `uname` range outside that segment returns
`-EFAULT`. Boot verification covers the exact lower and upper boundaries, the
first byte beyond the segment, and address-addition overflow. The ring-3 probe
also submits fd 2, a 257-byte write, and `uname` at the first mapped address
beyond `p_memsz`; it requires the three errors above and then completes the
valid syscall sequence, proving errors do not silently terminate the task.
