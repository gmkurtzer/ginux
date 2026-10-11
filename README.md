# Ginux

Ginux is an immutable, capability-secure, container-native operating system for AI infrastructure. The native host is intentionally small: an immutable kernel boots one constrained control-plane domain, which creates isolated services and containers through stable interfaces.

The initial executable milestone is an x86-64 unified EFI image tested under QEMU. Its boot component passes a development-signed authority manifest and measured control-plane bundle, exits firmware boot services, and transfers control to a separate `no_std` kernel. The kernel establishes protected CPU and memory state, seals manifest-derived capabilities into one native domain, and runs the measured W^X control-plane payload at ring 3 through a capability-checked call gate. Calibrated APIC one-shots preempt the control context to run an isolated service and its replacement generation, while bounded IPC carries the request and reply. The control plane can also create, bounded-load, stage, cancel, and retire stop-only auxiliary workloads without placing third-party code in the kernel. A separate isolated service probes and configures link, IPv4, default-gateway, and one static-route record through the first manifest-authorized network-device contract in control ABI v2; a monotonic generation makes effective configuration changes observable and rejects stale whole-state applies. Distinct read-only and write-only services prove that observation and mutation authority can be delegated independently, and a faulted network service is retired without losing mediated state or panicking the kernel. Packet I/O and a general route table remain deliberately unimplemented. On Apple Silicon development hosts it uses QEMU TCG emulation. The same Q35/OVMF profile will use KVM acceleration on x86-64 Linux.

Native scheduler contexts retain a lower-canonical per-thread FS base alongside
registers and the address-space root. Yield, blocking, and timer preemption
capture it; every initial entry and resume programs and reads back the selected
value before ring 3. Native bootstrap payloads currently use zero, while the
Linux TLS contexts use nonzero FS bases outside this scheduler path.

M4 begins with a strict static x86-64 ELF64 parser profile. It scans up to nine
program headers in any order, normalizes executable, optional read-only, and
writable loads,
and accepts validated optional `PT_PHDR`, `PT_GNU_RELRO`, non-executable
`PT_GNU_STACK`, bounded `PT_INTERP`, `PT_TLS`, and one-, two-, three-, four-,
five-, or six-entry
relocation `PT_DYNAMIC` declarations. Three-relocation images may place the eager
`R_X86_64_JUMP_SLOT` record in a separate one-entry `DT_JMPREL` table described
by `DT_PLTRELSZ` and `DT_PLTREL`; the six-relocation runtime main uses two
entries in that split table. The symbol profile validates a two- or three-entry
symbol-table prefix, bounded string table, bounded SysV `DT_HASH` or
one-bucket/one-word-bloom `DT_GNU_HASH`, and up to two global function symbols
used by paired `R_X86_64_GLOB_DAT` and `R_X86_64_JUMP_SLOT` records. The parser admits an ordered list of at most two distinct
`DT_NEEDED` names and rejects overflow or duplicates. A separate LLD-produced
metadata fixture proves both names and the second DSO's `DT_SONAME`. Independent
authority records bind both ordered names to their SHA-256 digests and canonical
load biases, with pairwise overlap rejection. Both dependencies are mapped and
self-relocated by the userspace interpreter. The main image's two undefined
symbols are matched by complete name against one provider in each authenticated
`ET_DYN` dependency.
Images whose expanded
program-header table covers the entry point are rejected. A bounded proof maps
a resolved `ET_EXEC` main image, relocated `ET_DYN` interpreter, and two
relocated dependencies into separate W^X regions, builds the dynamic startup
stack, and enters the interpreter first.
The linker-produced main also carries an authenticated read-only `PT_TLS` view
of its writable load: eight initialized bytes, sixteen total bytes, and
eight-byte alignment. The parser caps both TLS size and alignment at 64 bytes,
requires matching file and virtual displacement inside the writable load, and
rejects malformed or duplicate declarations. Planning preserves all three
templates and applies the main and dependency biases. The interpreted execution
proof allocates a separate owned RW/NX page and constructs a deterministic
three-module static layout: the auxiliary DSO occupies `TP-48..TP-32`, the
primary DSO occupies `TP-32..TP-16`, the main occupies `TP-16..TP`, and all
initialized `.tdata` regions retain zeroed `.tbss` tails. A minimal 16-byte TCB
contains its own address. FS is activated before the interpreter,
and the main preinitializer refuses to advance unless its TLS template, zero
tail, and self pointer are correct. Both DSOs' direct initializers also refuse
to advance unless their initialized and zero-filled regions are correct.
The main accesses name the hidden TLS symbols
through `R_X86_64_TPOFF32`; LLD resolves both offsets without adding a dynamic
relocation class, proving the toolchain layout matches the loader. Teardown
scrubs and releases the TLS frame and restores the supervisor's prior FS base.
The kernel admits only the live static-block-plus-TCB extent to syscall copy
validation; the interpreter writes the initialized `TLS_OK!` value directly
from FS-relative storage before invoking the preinitializer, while one-byte
underflow and overflow ranges are rejected internally.
Each DSO names its hidden TLS template through one toolchain-emitted
`R_X86_64_GOTTPOFF` object reference. LLD converts that reference into a
bounded `R_X86_64_TPOFF64` GOT relocation and marks the object `DF_STATIC_TLS`.
The userspace interpreter validates both exact relocation shapes and installs
`-32` for the primary DSO and `-48` for the auxiliary DSO; each initializer
loads its value from its GOT before using it as an FS-relative offset. Malformed
types, symbol indexes, addends, alignments, and targets are rejected. TLS
modules beyond the bounded graph, dynamic TLS, and per-thread allocation
remain unimplemented.
A reusable bounded planner computes each aligned block, signed thread-pointer
offset, live extent, and TCB fit from the authenticated templates. It rejects
zero or non-power-of-two alignments, undersized declarations, misaligned thread
pointers, arithmetic underflow, and page overflow; the execution path consumes
that plan rather than maintaining separate layout arithmetic.
The same reusable materializer revalidates the plan before mutation, clears the
whole owned page, copies every `.tdata` template, preserves zero-filled tails
and padding, and writes the self-referential TCB. A substituted plan is rejected
without changing the destination page.
The interpreted address space also allocates a second independently owned
RW/NX TLS page from that same authenticated plan. Its identical static blocks
have a distinct self-referential TCB and thread pointer. The active syscall-copy
contract admits only the initial thread's live TLS extent and rejects the
sibling extent even while both mappings exist. Teardown verifies the sibling
page is scrubbed before releasing both frame leases and restoring the allocator
count. This proves reusable per-thread allocation and cleanup prerequisites; it
does not yet create or schedule a second Linux thread, and code in one process
continues to share that process's address space.
FS base and the syscall-copy TLS extent are represented together as a checked
execution context. Before ring-3 entry, the kernel installs the sibling context,
reads back its FS base, validates its self pointer, proves copy admission moved
from the initial extent to the sibling extent, and then restores and revalidates
the initial context. Bootstrap is single-core with interrupts disabled during
this transition; scheduler-driven Linux TLS switching remains future work.
A two-slot Linux thread table now binds monotonically assigned TIDs and
generation-tagged internal handles to those checked TLS contexts. The initial
thread runs as TID 1; the boot proof reserves TID 2 for the sibling context,
switches to it and back, rejects a third live reservation, retires it, reuses
the slot as TID 3 with a new generation, and rejects the stale handle. The
ring-3 `gettid`, `set_tid_address`, and robust-list lookup paths resolve the
current table entry rather than embedding TID 1. Process exit retires the
initial entry and teardown requires the complete table to be empty before
resetting the process-local TID sequence.
The sibling slot also owns a separate, zeroed, RW/NX page between two unmapped
guard pages. Its exact bounds follow the slot across TID 2 retirement and TID 3
reuse. The current-thread syscall-copy contract rejects that whole page, and
teardown verifies it is scrubbed before releasing its frame. This establishes
bounded stack ownership and cleanup, not memory isolation within the shared
process address space.
An internal clone constructor now accepts exactly the bounded shared-VM,
shared-files, shared-signal-handler, thread-group, SysV-semaphore, TLS,
parent-TID, and child-clear-TID flag set. It requires the reserved stack top,
the sibling TLS pointer, aligned TID words inside the parent's writable stack,
and a safe return RIP/RFLAGS pair. It records a child context that resumes after
the eventual syscall with `rax = 0`, while the parent result is the child's TID.
Malformed flags and stack bounds fail without changing the slot, and retirement
clears the saved context before generation reuse. An internal publication proof
then exercises the current-thread copy boundary: denial rolls the entire child
execution state back to empty, while success writes the child TID to the exact
parent-TID word only after the complete context exists. The proof restores the
initial stack word and retires the unpublished child. There is still no
userspace `clone` entry or concurrently schedulable Linux thread.
The Linux `SYSCALL` entry now captures and restores `rbx`, `rbp`, and
`r12`-`r15` alongside all six syscall argument registers. Existing argument
indices remain unchanged for dispatch. This makes the complete preserved user
register set available on the kernel stack for a future thread switch; the
current path still returns to the same thread.
The assembly layout is now represented by one `repr(C)` syscall-frame type
rather than an untyped word slice. Clone construction copies all twelve saved
registers into generation-bound per-thread storage alongside RIP, RSP, RFLAGS,
RAX, and FS base. A checked selector reconstructs the complete return context
only for the matching live handle; retired handles and newly reused slots have
no selectable context. Selection is not yet connected to `SYSRETQ`.
A checked installer now validates the selected child first, switches the
generation-tagged running state and FS/copy context, and then fills an exact
return frame. Because all fallible work precedes the frame writes, a stale
handle leaves every output word and the running parent unchanged. The boot
proof installs the child, observes that copy admission moved to its TLS, and
restores the parent with the reverse admission decision. That cross-thread
installer still targets a local proof frame rather than redirecting `SYSRETQ`.
The real ring-3 `sched_yield` path now round-trips through that typed live
frame. With interrupts disabled, the handler validates and reinstalls the
current TLS context, writes all twelve registers plus RIP/RSP/RFLAGS into the
assembly-owned return state, reads the result back, and resumes successfully
through `SYSRETQ`.
The interpreted proof now takes the next step: its first yield names a bounded
authenticated child trampoline. The kernel saves the complete parent frame,
reserves the sibling TID/TLS/stack resources, installs the child through the
live `SYSRETQ` state, and executes it in ring 3 on the guarded stack. The child
uses `exit` to retire only itself; the kernel restores the parent's saved frame,
FS base, and copy ownership, after which the parent continues. Syscall stack
validation now resolves the running thread's exact stack extent instead of the
initial stack. This is one kernel-orchestrated proof child, not userspace
`clone(2)` or a general scheduler.
Each thread slot also owns its clear-child-TID pointer and robust-list head.
The lifecycle proof assigns different sentinel registrations to TIDs 1 and 2,
switches between them, and proves neither becomes visible through the other.
Retiring TID 2 clears both values before its slot is reused, and the replacement
starts empty. The real TID 1 syscall registrations are resolved through that
slot; exit zeroes the exact registered child-TID word, records and clears the
exact robust-list head, then retires the thread. Post-exit verification requires
both per-thread fields to be zero in every slot.
A compact userspace interpreter assembled from its own source derives its load
bias, reads each admitted `DT_RELA` record, applies its own load-biased relative
relocation plus the main image's relative and symbol-based relocations. The
interpreter independently requires the exact primary and split relocation
sizes, 24-byte `RELA` entries, counted-relative totals, and `DT_RELA` PLT kind
for the main and both dependencies before applying those records. The
symbol relocations resolve to the authenticated dependencies, and the interpreter
also applies both dependencies' own relative and self-symbol relocations. Each
DSO and the main expose one authenticated `DT_INIT` function and one eight-byte
`DT_INIT_ARRAY`. The main additionally exposes one eight-byte
`DT_PREINIT_ARRAY`. The interpreter independently scans the main's dynamic
table for the exact relocation size and entry size, two-entry relative count,
and complete preinit tag pair. It binds that address to the first relative
relocation target, then relocates and calls the preinitializer
before loading either dependency, calls each dependency's direct init function
and constructor in order, resolves the main, and then calls the main's direct
init function and constructor before transferring control to its entry.
Before applying any init-array relative relocation, the interpreter also
requires the runtime `DT_INIT_ARRAY` address to equal that relocation target
for both dependencies and the main.
Each exported function refuses to succeed before its dependency constructor
runs; each constructor requires its image's direct init phase, and the main
constructor calls both exports before recording initialization,
and the main entry refuses to continue without that record. All three
constructors receive the original `argc`, `argv`, and `envp` context and require
the bounded `/init` and `GINUX=1` vectors before recording initialization. The main workload calls through the
resolved jump slots; the loader validates all six dependency relocations and
both dependency calls return checked values before the full
ring-3 probe completes, while the kernel verifies the interpreter's relocated
target. The kernel then restores its root and scrubs every owned frame.
The build rejects unresolved ELF relocation records in all three raw embedded
text payloads before extracting their bytes.
The executable QEMU path now goes beyond the synthetic metadata fixtures: LLD
produces an `ET_DYN` interpreter, PIE main, `libginux.so`, and
`libginux-aux.so`, including
program headers, dynamic tables, GOT/PLT, SysV/GNU hashes, `DT_NEEDED`, and
`DT_SONAME`. The kernel authenticates all four linker outputs, applies fixed
load biases, accepts the dependencies' intentionally unmapped ELF headers, and
executes the complete probe through the linker-generated jump slot.
The authenticated interpreter occupies a bounded two-page RX extent plus one
RW/NX page; its metadata/startup core remains on the first executable page and
its runtime-file discovery path begins on the second. Before relocating itself,
the linked interpreter validates its mapped ELF
header and bounded program-header table and derives its sole `PT_DYNAMIC`
address and entry count; it no longer embeds its linker layout. After applying
that relocation, the interpreter bounds and scans its initial stack,
requires `AT_PHDR`, `AT_PHNUM`, `AT_BASE`, and `AT_ENTRY`, and verifies that
`AT_BASE` identifies its actual load page. It derives the main load bias from
`AT_PHDR`, the authenticated file's `e_phoff`, and matching `e_phnum`. It then
invokes the Linux syscall gate from its own RX range and opens `/init`. After inspecting it through a temporary
read-only window, the interpreter reopens it, bounds its 56-byte program-header
table to nine entries in the two-page window, and derives page-aligned mmap
requests from each readable `PT_LOAD`. It installs the authenticated header,
four RX, and two RW/NX pages through exact `MAP_FIXED_NOREPLACE` contracts. It
then captures the main image's two ordered `DT_NEEDED` strings, constructs each
`/lib64/<needed-name>` on its bounded stack, inspects those derived dependency
paths, walks both files' `PT_LOAD` entries the same way, and installs
RX pages at `0x618000..0x61a000`, an R/NX page at `0x61a000`, and an RW/NX page
at `0x61b000` for the primary DSO. The auxiliary DSO receives RX at `0x638000`
and RW/NX at `0x63b000`. The kernel admits a
page-aligned dependency bias only within `0x610000..0x620000`, binds the first
valid request, and requires every later dependency page to preserve the
authenticated relative layout at that bias. A bounded eight-entry contract
table is derived from the dependency's authenticated page plan; the current
fixture consumes four entries and exercises every admitted permission class.
Kernel execution state now provides two isolated dependency slots, each with
its own base window, selected bias, eight-entry contract table, mapping mask,
generation leases, and cleanup validation. The running interpreter populates
both slots, executes the main's one-entry preinit array, applies all three
admitted relocations in each DSO, executes each
dependency's `DT_INIT` function and one-entry init array, resolves the main, and
executes its `DT_INIT` function and one-entry init array before transferring to
the main entry.
Wrong offsets, duplicate
placement, a mixed bias, and W+X are rejected before it applies
dependency and main relocations. Both dynamic-table addresses and entry counts
come from bounded `PT_DYNAMIC` headers, and final transfer uses `AT_ENTRY`
rather than an embedded main entry address. The kernel installs only the interpreter for
this stage; it authenticates and plans the main and both dependencies and reserves
only the address-space infrastructure and interpreter frames initially. Main
and dependency payload frames are allocated only when their authenticated mmap
requests arrive. All three images' content and leaves are installed by userspace,
and every executable-page lease is scrubbed and released after the process
root is deactivated.
Linker-private metadata remains mapped but outside the probe's bounded
syscall-copy data extent.
Interpreter and dependency selection bind the exact `PT_INTERP` path or
`DT_NEEDED` name and SHA-256 digest before parsing or mapping. The dependency's
own `DT_SONAME` must equal both the requested name and the authority record. A
production authority source and general dynamic linking remain unimplemented.
The admitted linker tag subset also validates `DT_PLTGOT`, zero `DT_DEBUG`, one
counted relative relocation, and `DF_1_PIE`. Malformed dynamic tables,
duplicate tags, unsupported relocations, and relocation targets outside writable memory
are rejected before mapping.
The static planner also rejects any image carrying dynamic relocation metadata,
so unresolved code cannot bypass interpreter-first startup.
The execution-oriented main plan keeps its six-header table in a distinct
read-only load so its `0x400100` entry remains intact.
The profile is validated during boot,
plans and installs up to two RX/RW pages per segment with independently
zero-filled BSS, including ELF and program headers in the first load page. The
runtime proof owns every mapped frame, preserves per-page W^X permissions, and
rejects larger plans. The planner materializes a conventional third read-only
load and non-page-base segments. The mapper owns its frame and a separate leaf
table, activates the root, and executes the toolchain-linked image through the
full syscall workload before tearing every frame down. The synthetic two-load
proof also activates the collision-checked page-table root and
executes a non-page-base verification entry at ring 3. This
entry receives a distinct RW/NX stack containing `argc`, `argv[0]`, `envp[0]`,
and bounded Linux `AT_PHDR`/`AT_PHENT`/`AT_PHNUM`/`AT_PAGESZ`, `AT_CLKTCK`,
`AT_PLATFORM`, conservative `AT_HWCAP`/`AT_HWCAP2`, static-loader,
credential, `AT_SECURE`, `AT_EXECFN`, and `AT_ENTRY` metadata. It validates its
argument, environment, executable-name, secure-mode, and entry metadata from
ring 3 before terminating through the architectural x86-64 `SYSCALL` instruction
after checking that an unsupported call returns `-ENOSYS`, performing bounded
console `write` copy-in and `uname` copy-out operations, and validating a
`getpid()` result returned through `SYSRETQ`. Console writes accept 1 through
256 validated bytes on descriptor 1 and preserve exact byte content. Bounded
`writev` atomically validates and assembles up to four vectors under the same
aggregate limit before emitting any bytes. Bounded `poll` atomically reports
immediate console and device readiness across four descriptors; a call that
would wait returns `ENOSYS` until descriptor wait queues exist. `ppoll` adds
validated timespec and temporary signal-mask inputs without pretending to
sleep. A single bounded epoll instance can register one console or device
descriptor, report immediate level-triggered readiness, and preserve packed
64-bit user data; waits that would block use the same explicit `ENOSYS`
boundary. A bounded eventfd counter provides a signalable source for that
epoll path, including nonblocking saturation and semaphore reads. A bounded
monotonic timerfd adds one-shot and periodic clock-driven readiness with
relative or absolute arming. One bounded nonblocking pipe pair provides a
256-byte FIFO with atomic scalar or vectored I/O, poll/epoll readiness, EOF,
broken-peer behavior, FIFO metadata, and byte-count introspection. A bounded console ioctl reports a fixed 80-by-24 geometry, and `close` irreversibly
retires either initial console descriptor. Bounded `dup` reuses the lowest closed slot and preserves whether
the console endpoint is input or output; `dup2` provides exact-slot replacement.
`fcntl(F_GETFD/F_GETFL)` exposes the bounded descriptor and endpoint flags. It also
configures and queries an FS-relative
TLS base through bounded `arch_prctl(ARCH_SET_FS)` and restores the supervisor's
prior FS base afterward. Pointer-bearing calls are constrained to the RW
segment's declared ELF memory extent, allocated heap bytes below the current
break, the mapped user stack, or the active one- or two-page mapping. They
cannot cross the stack's guard-page boundaries. Invalid ranges and write
arguments return Linux `-EFAULT`, `-EBADF`, or `-EINVAL`. Copy-in may also read
the declared main or interpreter RX ELF extent, while copy-out rejects both.
The ring-3 workload verifies each error and continues into valid calls.
The bounded process reports UID, effective UID, GID, and effective GID zero as
container-local identity only; these values grant no native kernel capability.
Real/effective/saved UID and GID triples are also zero and are copied only
after all three output pointers pass writable-VMA validation.
Its initial single thread reports PID/TID/process-group/session ID 1 and parent
PID 0, matching an initial process namespace. A bounded
`set_tid_address` registration returns TID 1 and clears the registered word as
part of exit.
Bounded `set_robust_list` and `get_robust_list` calls retain and report one
24-byte thread robust-list head, reject foreign process IDs and invalid ranges,
and discard the registration during exit. Nonblocking futex compare and wake
semantics are exposed for the sole thread; a wait that would sleep returns
`ENOSYS` until scheduler wait queues exist. Robust futex recovery is not
implemented.
Bounded `mprotect` can irreversibly seal the page named by the parsed
`PT_GNU_RELRO` range or every page in the active anonymous extent as
read-only/NX. The page-table permissions and syscall copy direction change
together: reads continue, writes fail, and executable permissions are rejected.
One separately owned, zero-filled RW/NX page backs a bounded `brk` heap. The
ring-3 workload grows and shrinks the break, writes through the mapping, and
confirms that an out-of-range request leaves the current break unchanged.
A separate one- or two-page anonymous `mmap` extent is installed RW/NX on
demand, removed from the hardware page table by exact `munmap`, scrubbed, and
proven zero on reuse. Bounded `MADV_DONTNEED` also zeros it in place without
widening a sealed read-only extent, while `mincore` reports each page resident
only while active. The same fixed slot can instead expose a one- or two-page
private read-only/NX window into an immutable runtime ELF file. File offsets
must be page aligned, partial final pages are zero-filled, syscall copy-out
cannot modify the mapping, and unmap scrubs both payload frames.
General mapping placement, shared file mappings, writable mappings outside the
exact authenticated main/dependency data pages, and arbitrary filesystem-backed
mappings are not implemented.
The root view retains a fixed working-directory identity of `/`; bounded
`getcwd` returns it, while undersized and invalid buffers fail.
Its first procfs-compatible identity is `/proc/self/exe`, whose bounded
`readlink` result is `/init`.
An immutable runtime-file view exposes the exact authenticated `/init`,
`/lib64/ld-ginux.so`, `/lib64/libginux.so`, and `/lib64/libginux-aux.so` bytes
as read-only regular files. The linked main opens the auxiliary DSO, reads and
validates its ELF magic, and closes its independent descriptor. The interpreter
also derives, inspects, and executable-maps both DSO paths.
Bounded `openat`, scalar and vectored reads, `pread64`, `lseek`, private
read-only `mmap`, metadata, and access checks operate on those files. This is
not directory traversal, a general VFS, or general `execve` loading. The kernel
authenticates and plans the fixed four-image graph; the interpreter installs
the authenticated main and both dependency segments admitted by that exact plan.
Per-process `umask` state starts at `022`, returns its prior value, and masks
updates to the nine permission bits.
Bounded `fstat` identifies descriptors 0 and 1 as fixed character devices and
copies the exact x86-64 stat layout only after validating the complete output.
Read-only `getrlimit` and `prlimit64` queries expose the bounded stack and file
descriptor ceilings; unsupported resources, foreign PIDs, mutations, and
invalid output ranges fail without changing user memory.
Bounded `sched_getaffinity` and `getcpu` report the current bootstrap topology
as CPU 0 in NUMA node 0, with atomic validation of their output ranges.
Bounded `clock_gettime` reports monotonic and raw-monotonic elapsed time from a
private TSC epoch, using PIT calibration when CPUID has no frequency data;
`clock_getres` reports its rounded single-tick resolution. The sole process and
thread expose that same non-descheduled interval through CPU clocks and
100-Hz `times` accounting; bounded `getrusage` returns it as user time in a
fully initialized x86-64 resource-usage structure. The three interval-timer
classes can be queried and idempotently disarmed; arming remains unsupported
until asynchronous signal delivery exists.
The bounded ELF parser and page-image planner also validate checked,
page-aligned load-bias plans for position-independent `ET_DYN` images. Dynamic
execution planning binds an exact `PT_INTERP` path to a non-overlapping relocated
interpreter image while preserving distinct initial-entry, `AT_ENTRY`, and
`AT_BASE` values. The bounded synthetic interpreter-first execution path is
verified, including rejection of substituted, mutated, and truncated interpreter
bytes. The dependency is independently authenticated by exact name and digest,
requires a matching internal `DT_SONAME`, is mapped W^X at a non-overlapping
bias, and is rejected if it requests another dependency. Synthetic userspace
relative relocations are executed for the interpreter, main image, and
dependency. The dependency also receives its own
eager `GLOB_DAT` and `JUMP_SLOT` self-bindings. The main image proves one
cross-object symbol resolution through `R_X86_64_GLOB_DAT` and eager
`R_X86_64_JUMP_SLOT`; the jump slot is called in ring 3 and returns a checked
value from dependency code. The userspace interpreter verifies that the
relocation's symbol index is the admitted SysV main-image or GNU dependency
hash bucket result, follows each validated symbol-name offset into its
`DT_STRTAB`, and
requires the complete NUL-terminated names to match. Multiple dependencies,
transitive graphs, search paths, alternate SONAME selection,
symbol-version processing, interposition, multi-bucket lookup, lazy binding,
and a real userspace linker remain outside the profile.
This is not yet a general Linux
process or syscall environment; see
[Linux ELF64 Profile v0](specs/kernel/linux-elf-v0.md) and
[Linux Syscall Entry v0](specs/kernel/linux-syscall-v0.md).

## Quick start

Requirements:

- Rust 1.99.0 through `rustup`
- Clang with the `x86_64-none-elf` assembly target
- QEMU with x86-64 and OVMF support
- GNU Make

Check the local environment:

```sh
make doctor
```

Build the bootable EFI filesystem:

```sh
make build
```

`make build` deterministically regenerates the development manifest, bundle
digest, and Ed25519 signature from `boot/manifest-v0.conf` and
`control-plane/bootstrap.hex`; that source declares its authenticated control,
service, and restart entry labels inline. Run `make manifest` directly when
working on those inputs. The kernel build assembles `kernel/linux-probe.S` as
an x86-64 ELF object, extracts its `.text` section for the executable runtime
proof, and also links a complete static ELF with the Rust toolchain's LLD for
parser compatibility verification. The checked-in linker script fixes its
read-only headers at `0x3ff000`, a four-page RX load with entry at `0x400100`,
and RW data/BSS at `0x404000`, matching the ring-3 workload's bounded layout.
This tests ordinary
linker metadata on both macOS and Linux hosts. `make linux-probe` builds the
static artifact at `build/linux-probe.elf` and the dynamic fixtures at
`build/linux-dynamic/ld-ginux.so`,
`build/linux-dynamic/linux-linked-main.elf`, and
`build/linux-dynamic/libginux.so`. It also emits the bounded dependency-list
fixtures `linux-linked-graph-main.elf` and `libginux-aux.so` for inspection
without assembling the EFI
filesystem. Override the tools with `CLANG=/path/to/clang` or
`LLD=/path/to/rust-lld`. Kernel and
proof-workload ABI constants share the machine-readable
`specs/abi/control-v2.rs` source, and the manifest builder expands its symbolic
feature and descriptor words before signing. The checked-in key is an RFC test
identity and must not be treated as a production trust root.

The interpreter and dependency proof payloads are assembled from
`kernel/linux-interpreter.S` and `kernel/linux-dependency.S` under separate
bounded text-size and unresolved-relocation checks.

The result is placed at:

```text
build/esp/EFI/BOOT/BOOTX64.EFI
```

Boot it in headless QEMU:

```sh
make run
```

Run the automated boot assertion:

```sh
make test
```

The test succeeds only after the post-firmware kernel validates the UEFI memory map and emits `GINUX_KERNEL_OK` over the serial console.

Before committing a milestone, run the complete clean gate:

```sh
make gate
```

This formats the workspace, removes prior artifacts, runs lint checks, rebuilds
the EFI image, executes the QEMU boot assertions, and checks the working diff
for whitespace errors.

`make help` lists all supported development targets.

## Virtualization profiles

- **macOS on Apple Silicon:** QEMU x86-64 with TCG. UTM is optional for graphical inspection; the checked-in QEMU runner is authoritative.
- **Linux on x86-64:** QEMU x86-64 with KVM when `/dev/kvm` is available, falling back to TCG otherwise.

Override discovery when necessary:

```sh
QEMU_ACCEL=tcg QEMU_CPU=qemu64,+smep,+smap,+umip OVMF_CODE=/path/to/OVMF_CODE.fd OVMF_VARS=/path/to/OVMF_VARS.fd make run
```

The default virtual CPU is `qemu64,+smep,+smap,+umip`, matching the kernel's
fail-closed supervisor execution/access and user-mode instruction protection
requirements. `QEMU_CPU` may select a different model only if it still
advertises SMEP, SMAP, and UMIP.

See [ARCHITECTURE.md](ARCHITECTURE.md) for the full design, [ADR 0001](docs/decisions/0001-native-domains-and-authority.md) for the native domain and authority model, [Boot Handoff Contract v3](specs/boot/handoff-v3.md) for the firmware-to-kernel boundary, and [Paging v0](specs/kernel/paging-v0.md) for the transitional kernel address space.
