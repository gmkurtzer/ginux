# Linux ELF64 Profile v0

**Status:** Strict static x86-64 parser, mapping, and one-shot ring-3 entry
profile verified in QEMU.

The first Linux-compatibility boundary accepts a deliberately bounded ELF64
profile. Files must be little-endian ELF class 64, current ELF version,
System-V or Linux ABI, `ET_EXEC` or `ET_DYN`, and machine `EM_X86_64`. The ELF header and
program-header sizes must match the published 64-bit layout. Section-header
metadata is not consulted by the runtime loader; processor flags and extended
program-header counts are rejected.
The entry point may not fall within the mapped program-header table. This is
checked after deriving the table's runtime address, so adding metadata cannot
silently overwrite entry instructions while leaving an apparently valid entry
inside the executable load.

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
canonical user range. Static `ET_EXEC` virtual addresses begin at `0x10000`;
`ET_DYN` addresses may be relative to zero but are subject to that lower bound
after applying a load bias. File size may be smaller than
memory size to represent zero-filled data. Physical addresses may be zero or
equal to the corresponding virtual address, matching conventional linkers;
they are otherwise rejected and never used for mapping.
The segments must not overlap.
Planning additionally rejects distinct segments whose byte ranges do not
overlap but whose rounded mappings share a page. This prevents one segment's
page-table entry from replacing another segment or weakening its permissions;
the rule applies equally before static mapping and after `ET_DYN` relocation.

The profile requires exactly one executable segment and one writable segment.
Writable-executable mappings are rejected. The entry point must lie inside the
executable segment. The boot verifier accepts a synthetic image whose bounded
RX segment starts at file offset zero, includes its ELF/program headers and
code through file offset `0x4000`, and uses a non-page-base entry. Its RW
segment has eight file bytes and a
4608-byte memory extent spanning two pages. The verifier then rejects
bad magic, wrong class, non-executable file type, wrong machine, an out-of-range
entry point, W+X flags, overflowing file size, overlapping virtual ranges, an
executable GNU stack, malformed or duplicate metadata, unsupported header
types, and truncation. Images with neither metadata header, either one, or both
are accepted when their declarations are valid. The boot verifier reuses one
mutation buffer across metadata cases and runs the metadata and RELRO mutation
suites sequentially. At most two 16 KiB-class fixture images are therefore
live on the 64 KiB bootstrap kernel stack. This keeps verification bounded
while reserving the complete four-page executable profile for proof
instructions.

The build also feeds the same assembly through the Rust toolchain's LLD to
produce a genuine static ELF with separate read-only header, RX text, and RW
data/BSS loads, populated section-header metadata, `PT_PHDR`, and
`PT_GNU_RELRO` plus `PT_GNU_STACK`. The boot verifier parses that linked
artifact, checks its entry,
header-table address/count, exact code and initialized data, 4608-byte data/BSS
extent, and maps all three loads with role-derived permissions. The checked-in
linker script fixes the read-only header load at
`0x3ff000`, the non-page-base text entry at `0x400100`, and the data/BSS load at
`0x404000`, with the final 512 bytes declared RELRO at `0x405000`; this is the
stable input contract for linked-image execution work. Both the assembly build
and linker reject text larger than 16128 bytes, the exact space from the
non-page-base entry through the end of its four-page executable mapping. Probe
growth therefore fails at build time instead of overlapping the writable load
or silently exceeding the runtime loader profile.

The page-image planner represents executable, optional read-only, and writable
loads explicitly. It aligns each mapping downward, retains the declared memory
start, accounts for the in-page prefix when bounding page count, and copies file
bytes at the correct page offset while zeroing the prefix, BSS, and page tails.
The runtime mapper owns a separate leaf table and payload frame for the third
read-only load, installs it read-only/NX, activates the root, and executes the
toolchain-linked image. It passes the normalized RELRO range into the execution
contract, so the bounded `mprotect` path can seal only the declared page.
Syscall return validation remains bounded to its mapped
RX extent while readable-code copy bounds begin at the exact non-page-base
`0x400100` memory extent. The proof restores the kernel root and scrubs every
owned frame after exit.

The strict profile prevents unsupported features from being silently
interpreted with weaker semantics. It recognizes at most one `PT_INTERP`
record with read-only flags, zero virtual addresses, unit alignment, matching
file/memory sizes, and a bounded absolute NUL-terminated path without embedded
NUL bytes. Duplicate, relative, unterminated, writable, oversized, and
out-of-file records are rejected. An image carrying valid interpreter metadata
is represented by the parser but deliberately rejected by the page-image
planner, so the kernel cannot enter its main executable at the wrong entry
point. Position-independent `ET_DYN` images use a separate planner that requires
a nonzero, page-aligned load bias, relocates every segment, entry, program-header,
and RELRO address with checked arithmetic, and rejects mappings outside the user
range. The static planner rejects `ET_DYN`, and the position-independent planner
rejects interpreter-bearing images. A separate interpreted-image planner binds an
`ET_EXEC` main image to an exactly matching resolved interpreter path and a
non-interpreted `ET_DYN` image. It preserves the main entry for `AT_ENTRY`, uses
the relocated interpreter entry as the initial instruction pointer, records the
interpreter bias for `AT_BASE`, and rejects page-granular overlap between every
main and interpreter load before mapping. The resolver still must authenticate
the selected interpreter bytes. The execution-oriented planning fixture uses a
five-header main image: a dedicated read-only header load at `0x3ff000`, RX text
stored from file offset `0x200` with entry `0x400100`, RW/RELRO data, and
`PT_INTERP`. Its program-header table therefore remains readable at `0x3ff040`
without occupying entry bytes. Static and interpreted plans produce the same
startup-metadata type, so initial RIP and the `AT_PHDR`, `AT_PHNUM`, `AT_BASE`,
and `AT_ENTRY` values cannot silently fall back to static defaults. The existing
static runtime stack is now populated by a single bounded auxiliary-vector
encoder using that common contract, and the interpreted proof checks the exact
word positions of its dynamic values. A bounded execution proof installs the
main image's read-only/NX header page, four RX pages, and two RW/NX pages plus
the interpreter's RX and RW/NX pages in three independently cloned 2 MiB leaf
regions. It also owns bounded heap, stack, and two `mmap` payload frames. The
proof validates permissions, the intact main entry, and image contents through
the activated user root, constructs the dynamic-process stack with the common
auxiliary-vector encoder, and enters the relocated interpreter at `0x6000f0`.
That synthetic interpreter is a fixed trampoline to the main entry at
`0x400100`; the complete ring-3 probe therefore runs with dynamic `AT_BASE`,
main-image `AT_ENTRY`, and main-image `AT_PHDR`/`AT_PHNUM` values. The kernel
restores its own root and scrubs and releases all nineteen page-table and
payload frames. The profile does not apply interpreter relocations, resolve
dynamic symbols, execute a real dynamic linker, or support glibc.

## Page-image plan

The bounded page-image planner accepts executable segments of at most four
4096-byte pages and all other segments of at most two pages. It preserves ELF virtual addresses and derives separate
executable/read-only and writable/non-executable metadata. Each page is first
zeroed and then receives only its intersecting file bytes, so a second BSS-only
page is entirely zero. A third non-executable page or a fifth executable page
is rejected. A load segment
may begin
at file offset zero, as real ELF images commonly do, so the first mapped page
preserves the ELF and program-header tables and places instructions at their
file-relative offset. The writable segment's file tail through `p_memsz` and
all remaining page bytes are zero. The QEMU verifier checks the ELF magic,
exact RX instructions, RW data bytes, W^X metadata, and every zero-filled tail
byte. Runtime address-space installation owns and tears down every planned
page, including a second BSS-only RW/NX page.

QEMU maps the toolchain-linked read-only header at `0x3ff000` through a
separately cloned 2 MiB leaf table, validates it beside the four-page RX and
RW/NX loads, and runs the linked entry. The synthetic proof also installs one
four-page RX load at `0x400000` and two RW/NX pages at
`0x404000` and `0x405000` into a newly allocated x86-64 root. It privately clones
the affected PML4, PDPT, directory, and 2 MiB leaf region; replaces only the two
segment ranges; retains the supervisor-only higher-half direct map; and verifies
RX versus RW/NX permissions plus exact materialized contents through the direct
map. An owned page at `0x406000` supplies one zero-filled RW/NX heap page. The
page at `0x407000` remains an unmapped lower stack guard, and an owned page at
`0x408000` supplies a RW/NX stack; `0x409000` remains an upper guard. Two final
owned frames are initially absent at `0x40a000` and `0x40b000` and are
installed only for the bounded one- or two-page anonymous `mmap` lifecycle. Before
changing CR3, the proof rejects either
segment, heap, or stack target page if it overlaps the loaded kernel PE image.
It then activates the root, observes the declared ELF mappings through their
user virtual addresses under SMAP, and enters CPL3 at the ELF
entry at `0x400100`, while syscall return validation remains bounded to its
declared executable extent. The proof instructions live in checked-in x86-64
assembly; the build uses Clang to produce an ELF relocatable and extracts its
`.text` section, so branch layout is assembler-validated instead of
hand-encoded. Repeated success and errno assertions use callable assembly
helpers. Each helper executes `SYSCALL` from a nested return frame inside the
owned stack page, preserving every individual call-site assertion while
avoiding duplicated branch sequences. Helpers cover common errno results and
the repeated positive return values 1, 2, 3, 5, 8, 152, and 256. Together they
recover more than 3.2 KiB of the
fixed four-page executable budget without removing compatibility cases. The
instruction stream first verifies that unsupported call
999 returns Linux `-ENOSYS` without terminating it. It then issues a bounded
Linux `write(1, 0x404000, 8)` whose kernel copy-in verifies the RW payload. A
bounded `arch_prctl(ARCH_SET_FS, 0x404000)` then makes the payload available
through an FS-relative TLS read. `ARCH_GET_FS` copies the active base back to
the declared data segment; invalid operation and destination tests require
`-EINVAL` and `-EFAULT` without terminating the task. A bounded
`uname(0x404000)` subsequently
copies a complete zero-initialized `utsname` into
the declared data segment, where the probe validates the system name. Both
copy directions are checked against the segment's 4608-byte `p_memsz`, rather
than the rest of its final mapped page. Copy helpers also accept heap bytes
strictly below the current break, not the entire mapped heap page, ranges
wholly inside the user stack page, and ranges inside the active anonymous
mapping. Copy-in additionally permits the declared
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
sixteen root/table/reserved-segment/heap/stack/mapping frames.

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
equal to the ELF entry, `AT_CLKTCK = 100`, `AT_PLATFORM` pointing to `x86_64`,
conservative `AT_HWCAP = 0` and `AT_HWCAP2 = 0` declarations, and `AT_NULL`.
The zero hardware masks intentionally make no optional instruction-set promise
until the kernel has a maintained Linux feature-bit translation. These
credentials are container-local and do not grant native kernel capabilities.
The probe dereferences and validates `argv[0]`, `envp[0]`, `AT_BASE`,
`AT_FLAGS`, `AT_SECURE`, `AT_EXECFN`, `AT_ENTRY`, `AT_PLATFORM`, `AT_HWCAP`,
and `AT_HWCAP2` from CPL3 before its
syscall sequence. Credential queries include
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
