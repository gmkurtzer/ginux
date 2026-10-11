# Linux ELF64 Profile v0

**Status:** Strict static x86-64 parser, mapping, and one-shot ring-3 entry
profile verified in QEMU.

The first Linux-compatibility boundary accepts a deliberately bounded ELF64
profile. Files must be little-endian ELF class 64, current ELF version,
System-V or Linux ABI, `ET_EXEC`, and machine `EM_X86_64`. The ELF header and
program-header sizes must match the published 64-bit layout. Section-header
metadata is not consulted by the runtime loader; processor flags and extended
program-header counts are rejected.

Two or three `PT_LOAD` program headers are accepted. Exactly one must be
executable and exactly one writable; an optional third load must be read-only.
One `PT_GNU_STACK` header
is optional and must request read/write, non-executable memory with no file or
virtual range. One `PT_PHDR` header is also optional; it must be read-only and
must exactly describe the in-image program-header table at its derived mapped
address. One `PT_GNU_RELRO` header is optional. It must be read-only, start on
the final page of the writable load, end exactly with that load, and cover no
more than one page. This profile rounds the declaration out to its containing
page when userspace seals it with `mprotect`. Up to six recognized headers may
appear in any order. The parser
normalizes load segments by executable, read-only, and writable role before
planning. The parser accepts images without RELRO, while the current executable
mapping proof requires it so the memory transition has an authenticated target.
Unknown and duplicate metadata headers are rejected. Each load
segment must be readable, nonempty in the
file, contained within the supplied image, aligned to a nonzero power of two
with congruent file and virtual addresses, and mapped wholly inside the lower
canonical user range beginning at `0x10000`. File size may be smaller than
memory size to represent zero-filled data. Physical addresses may be zero or
equal to the corresponding virtual address, matching conventional linkers;
they are otherwise rejected and never used for mapping.
The segments must not overlap.

The profile requires exactly one executable segment and one writable segment.
Writable-executable mappings are rejected. The entry point must lie inside the
executable segment. The boot verifier accepts a synthetic image whose bounded
RX segment starts at file offset zero, includes its ELF/program headers and
code, and uses a non-page-base entry. Its RW segment has eight file bytes and a
4608-byte memory extent spanning two pages. The verifier then rejects
bad magic, wrong class, non-executable file type, wrong machine, an out-of-range
entry point, W+X flags, overflowing file size, overlapping virtual ranges, an
executable GNU stack, malformed or duplicate metadata, unsupported header
types, and truncation. Images with neither metadata header, either one, or both
are accepted when their declarations are valid.

The build also feeds the same assembly through the Rust toolchain's LLD to
produce a genuine static ELF with separate read-only header, RX text, and RW
data/BSS loads, populated section-header metadata, `PT_PHDR`, and
`PT_GNU_RELRO` plus `PT_GNU_STACK`. The boot verifier parses that linked
artifact, checks its entry,
header-table address/count, exact code and initialized data, 4608-byte data/BSS
extent, and maps all three loads with role-derived permissions. The checked-in
linker script fixes the read-only header load at
`0x3ff000`, the non-page-base text entry at `0x400100`, and the data/BSS load at
`0x401000`, with the final 512 bytes declared RELRO at `0x402000`; this is the
stable input contract for linked-image execution work.

The page-image planner represents executable, optional read-only, and writable
loads explicitly. It aligns each mapping downward, retains the declared memory
start, accounts for the in-page prefix when bounding page count, and copies file
bytes at the correct page offset while zeroing the prefix, BSS, and page tails.
The runtime mapper owns a separate leaf table and payload frame for the third
read-only load, installs it read-only/NX, activates the root, and executes the
toolchain-linked image. It passes the normalized RELRO range into the execution
contract, so the bounded `mprotect` path can seal only the declared page.
Syscall return validation remains bounded to its mapped
RX page while readable-code copy bounds begin at the exact non-page-base
`0x400100` memory extent. The proof restores the kernel root and scrubs every
owned frame after exit.

The strict profile prevents unsupported features from being silently
interpreted with weaker semantics. It does not accept position-independent
`ET_DYN`, load `PT_INTERP`, accept arbitrary program-header types, perform
relocations, or support glibc.

## Page-image plan

The bounded page-image planner accepts page-aligned segments of at most two
4096-byte pages. It preserves ELF virtual addresses and derives separate
executable/read-only and writable/non-executable metadata. Each page is first
zeroed and then receives only its intersecting file bytes, so a second BSS-only
page is entirely zero. A request for a third page is rejected. A load segment
may begin
at file offset zero, as real ELF images commonly do, so the first mapped page
preserves the ELF and program-header tables and places instructions at their
file-relative offset. The writable segment's file tail through `p_memsz` and
all remaining page bytes are zero. The QEMU verifier checks the ELF magic,
exact RX instructions, RW data bytes, W^X metadata, and every zero-filled tail
byte. Runtime address-space installation owns and tears down every planned
page, including a second BSS-only RW/NX page.

QEMU maps the toolchain-linked read-only header at `0x3ff000` through a
separately cloned 2 MiB leaf table, validates it beside the RX and RW/NX loads,
and runs the linked entry. The synthetic proof also installs an RX page at
`0x400000` and two RW/NX pages at
`0x401000` and `0x402000` into a newly allocated x86-64 root. It privately clones
the affected PML4, PDPT, directory, and 2 MiB leaf region; replaces only the two
segment ranges; retains the supervisor-only higher-half direct map; and verifies
RX versus RW/NX permissions plus exact materialized contents through the direct
map. An owned page at `0x403000` supplies one zero-filled RW/NX heap page. The
page at `0x404000` remains an unmapped lower stack guard, and an owned page at
`0x405000` supplies a RW/NX stack; `0x406000` remains an upper guard. Before
changing CR3, the proof rejects either
segment, heap, or stack target page if it overlaps the loaded kernel PE image.
It then activates the root, observes all three ELF pages through
their declared user virtual addresses under SMAP, and enters CPL3 at the ELF
entry at `0x400100`, while syscall return validation remains bounded to its
`0x400000` executable page. The proof instructions live in checked-in x86-64
assembly; the build uses Clang to produce an ELF relocatable and extracts its
`.text` section, so branch layout is assembler-validated instead of
hand-encoded. The instruction stream first verifies that unsupported call
999 returns Linux `-ENOSYS` without terminating it. It then issues a bounded
Linux `write(1, 0x401000, 8)` whose kernel copy-in verifies the RW payload. A
bounded `arch_prctl(ARCH_SET_FS, 0x401000)` then makes the payload available
through an FS-relative TLS read. `ARCH_GET_FS` copies the active base back to
the declared data segment; invalid operation and destination tests require
`-EINVAL` and `-EFAULT` without terminating the task. A bounded
`uname(0x401000)` subsequently
copies a complete zero-initialized `utsname` into
the declared data segment, where the probe validates the system name. Both
copy directions are checked against the segment's 4608-byte `p_memsz`, rather
than the rest of its final mapped page. Copy helpers also accept heap bytes
strictly below the current break, not the entire mapped heap page, and ranges
wholly inside the user stack page. Copy-in additionally permits the declared
RX ELF memory extent; copy-out does not. The proof reads an embedded RX payload
through `write` and requires `-EFAULT` when `uname` targets that read-only page.
Ranges crossing a stack guard page are rejected before SMAP access. Bounded
`getcwd` exposes the initial container working directory as `/`, and bounded
`readlink` identifies `/proc/self/exe` as `/init`. Per-process `umask` starts at
`022` and masks updates to nine bits. It next
issues process identity queries, verifies PID, TID, process-group, and session
ID 1 plus parent PID 0 after the kernel resumes it through `SYSRETQ`,
registers a bounded clear-child-TID word, verifies preserved first and fourth
argument registers, and issues Linux `exit_group(42)`. The kernel clears the
registered word before completing exit and verifies it after returning from
ring 3. Those calls are
accepted only while an atomic probe flag is armed; exit returns directly to the
saved kernel stack. The kernel verifies a ring-3 heap write after exit. The
proof restores and reads back the kernel root, then clears and releases all
thirteen root/table/reserved-segment/heap/stack frames.

This proves instruction fetch and controlled return, not a Linux process. The
root retains supervisor-only low identity leaves around the ELF pages so the
current low-linked kernel can survive the proof transition. The initial stack
pointer is 16-byte aligned and names `argc = 1`, an `argv[0]` pointer to the
NUL-terminated `/init` string, a null `argv` terminator, an `envp[0]` pointer
to `GINUX=1`, a null `envp` terminator,
`AT_PHDR` pointing at the mapped program-header table, `AT_PHENT = 56`,
`AT_PHNUM` matching the parsed header count, `AT_PAGESZ = 4096`, `AT_BASE = 0`
for the static image, `AT_FLAGS = 0`, `AT_UID = 0`, `AT_EUID = 0`, `AT_GID = 0`,
`AT_EGID = 0`, `AT_SECURE = 0`, `AT_EXECFN` pointing to `/init`, `AT_ENTRY`
equal to the ELF entry, `AT_CLKTCK = 100`, and `AT_NULL`. These credentials are
container-local
and do not grant native kernel capabilities. The probe dereferences and validates
`argv[0]`, `envp[0]`, `AT_BASE`, `AT_FLAGS`, `AT_SECURE`, `AT_EXECFN`, and
`AT_ENTRY` from CPL3 before its syscall sequence. Credential queries include
atomic copy-out of real/effective/saved UID and GID triples, all zero in the
container namespace. `AT_RANDOM` remains absent
until the boot path supplies a cryptographically suitable entropy source.
Multiple arguments or environment entries, the complete Linux auxiliary
vector, scheduler ownership, the broader syscall
surface, and general collision policy beyond the loaded kernel image are not
defined yet. The loader does not relocate `ET_EXEC` images to the existing high
test address, because doing so would violate ELF semantics and hide these
remaining prerequisites.

The syscall transition is specified separately in
[Linux Syscall Entry v0](linux-syscall-v0.md).
