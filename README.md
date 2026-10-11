# Ginux

Ginux is an immutable, capability-secure, container-native operating system for AI infrastructure. The native host is intentionally small: an immutable kernel boots one constrained control-plane domain, which creates isolated services and containers through stable interfaces.

The initial executable milestone is an x86-64 unified EFI image tested under QEMU. Its boot component passes a development-signed authority manifest and measured control-plane bundle, exits firmware boot services, and transfers control to a separate `no_std` kernel. The kernel establishes protected CPU and memory state, seals manifest-derived capabilities into one native domain, and runs the measured W^X control-plane payload at ring 3 through a capability-checked call gate. Calibrated APIC one-shots preempt the control context to run an isolated service and its replacement generation, while bounded IPC carries the request and reply. The control plane can also create, bounded-load, stage, cancel, and retire stop-only auxiliary workloads without placing third-party code in the kernel. A separate isolated service probes and configures link, IPv4, default-gateway, and one static-route record through the first manifest-authorized network-device contract in control ABI v2; a monotonic generation makes effective configuration changes observable and rejects stale whole-state applies. Distinct read-only and write-only services prove that observation and mutation authority can be delegated independently, and a faulted network service is retired without losing mediated state or panicking the kernel. Packet I/O and a general route table remain deliberately unimplemented. On Apple Silicon development hosts it uses QEMU TCG emulation. The same Q35/OVMF profile will use KVM acceleration on x86-64 Linux.

M4 begins with a strict static x86-64 ELF64 parser profile. It scans up to eight
program headers in any order, normalizes executable, optional read-only, and
writable loads,
and accepts validated optional `PT_PHDR`, `PT_GNU_RELRO`, non-executable
`PT_GNU_STACK`, bounded `PT_INTERP`, and one- to three-entry relocation
`PT_DYNAMIC` declarations. Three-relocation images may place the eager
`R_X86_64_JUMP_SLOT` record in a separate one-entry `DT_JMPREL` table described
by `DT_PLTRELSZ` and `DT_PLTREL`; the current interpreted fixtures use that
conventional split. The symbol profile validates a two-entry symbol-table
prefix, bounded string table, either a one-bucket legacy SysV `DT_HASH` or a
one-bucket/one-word-bloom `DT_GNU_HASH`, and one
global function symbol used by `R_X86_64_GLOB_DAT` and
`R_X86_64_JUMP_SLOT`. The interpreted proof admits exactly one `DT_NEEDED`
name. Its undefined main-image symbol is matched to the same defined symbol in
one authenticated `ET_DYN` dependency.
Images whose expanded
program-header table covers the entry point are rejected. A bounded proof maps
a resolved `ET_EXEC` main image, relocated `ET_DYN` interpreter, and one
relocated dependency into separate W^X regions, builds the dynamic startup
stack, and enters the interpreter first.
A compact userspace interpreter assembled from its own source derives its load
bias, reads each admitted `DT_RELA` record, applies its own load-biased relative
relocation plus the main image's relative and symbol-based relocations. The
symbol relocations resolve to the authenticated dependency, and the interpreter
also applies the dependency's own relative and self-symbol relocations before
transferring control to the main entry. The main workload calls through the
resolved jump slot; the loader validates all three dependency relocations and
dependency code returns a checked value before the full
ring-3 probe completes, while the kernel verifies the interpreter's relocated
target. The kernel then restores its root and scrubs every owned frame.
The build rejects unresolved ELF relocation records in all three raw embedded
text payloads before extracting their bytes.
The executable QEMU path now goes beyond the synthetic metadata fixtures: LLD
produces an `ET_DYN` interpreter, PIE main, and separate `libginux.so`, including
program headers, dynamic tables, GOT/PLT, SysV/GNU hashes, `DT_NEEDED`, and
`DT_SONAME`. The kernel authenticates all three linker outputs, applies fixed
load biases, accepts the dependency's intentionally unmapped ELF headers, and
executes the complete probe through the linker-generated jump slot.
Before relocating itself, the linked interpreter validates its mapped ELF
header and bounded program-header table and derives its sole `PT_DYNAMIC`
address and entry count; it no longer embeds its linker layout. After applying
that relocation, the interpreter bounds and scans its initial stack,
requires `AT_PHDR`, `AT_PHNUM`, `AT_BASE`, and `AT_ENTRY`, and verifies that
`AT_BASE` identifies its actual load page. It derives the main load bias from
`AT_PHDR`, the authenticated file's `e_phoff`, and matching `e_phnum`. It then
invokes the Linux syscall gate from its own RX range and opens `/init`. After inspecting it through a temporary
read-only window, the interpreter reopens it, bounds its 56-byte program-header
table to eight entries in the two-page window, and derives page-aligned mmap
requests from each readable `PT_LOAD`. It installs the authenticated header,
four RX, and two RW/NX pages through exact `MAP_FIXED_NOREPLACE` contracts. It
then captures the main image's sole `DT_NEEDED` string,
constructs `/lib64/<needed-name>` on its bounded stack, inspects that derived
dependency path, walks that file's `PT_LOAD` entries the same way, and installs
RX pages at `0x618000..0x61a000` and an RW/NX page at `0x61b000`. The kernel admits a
page-aligned dependency bias only within `0x610000..0x620000`, binds the first
valid request, and requires every later dependency page to preserve the
authenticated relative layout at that bias. A bounded eight-entry contract
table is derived from the dependency's authenticated page plan; the current
fixture consumes three entries. Wrong offsets, duplicate
placement, a mixed bias, and W+X are rejected before it applies
dependency and main relocations. Both dynamic-table addresses and entry counts
come from bounded `PT_DYNAMIC` headers, and final transfer uses `AT_ENTRY`
rather than an embedded main entry address. The kernel installs only the interpreter for
this stage; it authenticates and plans the main and dependency and reserves
only the address-space infrastructure and interpreter frames initially. Main
and dependency payload frames are allocated only when their authenticated mmap
requests arrive. Both images' content and leaves are installed by userspace,
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
`/lib64/ld-ginux.so`, and `/lib64/libginux.so` bytes as read-only regular files.
Bounded `openat`, scalar and vectored reads, `pread64`, `lseek`, private
read-only `mmap`, metadata, and access checks operate on those files. This is
not directory traversal, a general VFS, or general `execve` loading. The kernel
still authenticates and plans the fixed runtime trio; the interpreter installs
the authenticated main and dependency segments admitted by that exact plan.
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
static artifact at `build/linux-probe.elf` and the executable dynamic trio at
`build/linux-dynamic/ld-ginux.so`,
`build/linux-dynamic/linux-linked-main.elf`, and
`build/linux-dynamic/libginux.so` for inspection without assembling the EFI
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
