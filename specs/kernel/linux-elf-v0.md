# Linux ELF64 Profile v0

**Status:** Strict bounded x86-64 parser, static and interpreted mapping, and
one-shot ring-3 entry profile verified in QEMU.

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
page when userspace seals it with `mprotect`. One `PT_TLS` header is optional.
It must be read-only, contain one through 64 initialized bytes, declare total
memory no smaller than the initialized data, cap total memory and alignment at 64 bytes, and
use a nonzero power-of-two alignment. Its file and virtual offsets must be
congruent, its physical address must be zero or equal its virtual address, and
both its initialized and total extents must preserve the same displacement
inside the writable load. This makes TLS metadata an authenticated overlapping
view rather than an additional mapping. Up to nine recognized headers may
appear in any order. The parser
normalizes load segments by executable, read-only, and writable role before
planning. The parser accepts images without RELRO, while the current executable
mapping proof requires it so the memory transition has an authenticated target.
Unknown and duplicate metadata headers are rejected. A position-independent
dependency may omit a mapped program-header table when it has no `PT_PHDR`;
its headers are not published through `AT_PHDR`. Main executables and
interpreters still require mapped program headers. Each load
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
a nonzero, page-aligned load bias, relocates every segment, entry,
program-header, RELRO, and TLS virtual address with checked arithmetic, and
rejects mappings outside the user
range. The static planner rejects `ET_DYN`, and the position-independent planner
rejects interpreter-bearing images. A separate interpreted-image planner binds an
`ET_EXEC` main image to an exactly matching resolved interpreter path and a
non-interpreted `ET_DYN` image. It preserves the main entry for `AT_ENTRY`, uses
the relocated interpreter entry as the initial instruction pointer, records the
interpreter bias for `AT_BASE`, and rejects page-granular overlap between every
main and interpreter load before mapping. An interpreter authority record binds
the exact `PT_INTERP` path to a SHA-256 digest. The kernel requires both to match
the resolved bytes before it parses, plans, maps, or executes the interpreter;
wrong-path, wrong-digest, mutated, and truncated candidates are rejected. The
authority record is currently a synthetic proof input rather than a
production-signed package manifest. The parser accepts an ordered list of at
most two distinct `DT_NEEDED` names, rejects a third or a duplicate, and
preserves their dynamic-table order. The executable profile currently requires
exactly two names for the LLD runtime graph; the compact synthetic fixture keeps
one. Each dependency must declare one nonempty `DT_SONAME`; an ordered
dependency authority record binds each exact string to a SHA-256 digest and
fixed page-aligned canonical bias.
The kernel authenticates the bytes before parsing or mapping the dependency,
requires a non-interpreted `ET_DYN` image with no nested `DT_NEEDED`, and rejects
page-granular overlap among all main, interpreter, and dependency loads. Wrong
names, wrong internal SONAMEs, wrong digests, truncation, and an overlapping
dependency bias are rejected. An authenticated dependency exporting a
different symbol name is also rejected. The parser also accepts one bounded
`PT_DYNAMIC` table describing one, two, three, five, or six relocation records, the exact
primary `DT_RELA` size and 24-byte entry size, and a terminating `DT_NULL`.
One- and two-record profiles keep every record in the primary table. A
three-record profile may do the same, or may keep its first two records in the
primary table and describe one non-overlapping `R_X86_64_JUMP_SLOT` record with
the all-or-none `DT_JMPREL`, `DT_PLTRELSZ`, and `DT_PLTREL` triplet. The split
table must be exactly one 24-byte `DT_RELA` record. The five-record profile
keeps one relative and two `GLOB_DAT` records in the primary table and exactly
two `JUMP_SLOT` records in the split table. The six-record profile adds a
leading preinit-array relative relocation before that five-record sequence.
Partial triplets, another
relocation encoding, wrong sizes, overlap with the primary table, and split
tables attached to other primary-table sizes are rejected. The first record must
be an `R_X86_64_RELATIVE` relocation with no symbol. A two- or three-record table must
also declare `DT_STRTAB`, `DT_STRSZ`, `DT_SYMTAB`, and `DT_SYMENT`, plus exactly
one of `DT_HASH` or `DT_GNU_HASH`, with a 24-byte symbol-entry size. The
file-backed single-symbol SysV form is bounded to one or two buckets and two
chains. The five- and six-record main profiles use exactly three buckets and
three chains, require each of symbols one and two in its computed bucket
without collision, and require all chain terminators to be zero. The GNU form is
bounded to one bucket, symbol offset one, one 64-bit bloom word, bloom shift
five, and one terminating chain word. Its bloom bits, bucket, and chain must
match the GNU hash of the admitted symbol name. The kernel computes either
symbol-name hash rather than trusting a fixed table offset. In every symbol
profile, the `GLOB_DAT` record immediately following the init-array relative
relocation
must be `R_X86_64_GLOB_DAT`, use
symbol index one, have a zero addend, and reference a global/default-visible
function symbol. A self-contained image must define it with a value and size
inside the executable load. An interpreted main image with `DT_NEEDED` must
instead leave it undefined with zero value and size; the authenticated
dependency must define the same name and may not name another dependency. A
third record in the three-record profile must be `R_X86_64_JUMP_SLOT`, reference the same symbol,
carry a zero addend, and use a third distinct writable target. This profile
performs eager binding; it does not install or invoke a lazy resolver. The
validated symbol-table prefix is exactly two entries for the single-symbol profiles:
an all-zero null entry followed by the referenced symbol. Its nonempty name must start within a
2-to-256-byte file-backed string table and terminate within that table. Every
relocation target must be distinct, eight-byte aligned, and wholly inside the
writable load. A one-record table may not carry partial symbol metadata.
The five- and six-record main profiles instead have an exact three-entry prefix: the null symbol
followed by two distinct undefined global/default-visible functions with zero
value and size. Its two `GLOB_DAT` and two `JUMP_SLOT` records must use symbol
indexes one and two in order and four distinct writable targets.
Duplicate tags, mixed hash styles, missing or mixed symbol metadata, malformed
bucket, bloom, or chain metadata, unsupported relocation types or sizes, invalid symbol indexes/attributes/ranges/names, nonzero
symbol-relocation addends, and executable relocation targets are rejected. The
all-or-none `DT_INIT_ARRAY` and `DT_INIT_ARRAYSZ` pair is admitted only for an
exact one-entry, eight-byte array wholly inside writable memory. Its address
must equal the image's sole relative-relocation target, and the authenticated
addend must point inside that image's executable segment. Duplicate, partial,
empty, multi-entry, unaligned, non-writable, or non-executable-target forms are
rejected. An optional, unique `DT_INIT` value must point directly inside the
same executable segment; zero, duplicate, and out-of-range values are rejected.
Only the six-record executable-main profile admits an all-or-none
`DT_PREINIT_ARRAY` and `DT_PREINIT_ARRAYSZ` pair. It must describe exactly one
eight-byte entry in writable memory, equal the first of exactly two leading
relative-relocation targets, and relocate to executable code. Dependencies may
not declare preinit arrays. Partial, empty, multi-entry, reordered,
unaligned, non-writable, or non-executable-target forms are rejected.
The execution-oriented planning fixture uses a
six-header main image: a dedicated read-only header load at `0x3ff000`, RX text
stored from file offset `0x200` with entry `0x400100`, RW/RELRO data, and
`PT_INTERP`. Its program-header table therefore remains readable at `0x3ff040`
without occupying entry bytes. Static and interpreted plans produce the same
startup-metadata type, so initial RIP and the `AT_PHDR`, `AT_PHNUM`, `AT_BASE`,
and `AT_ENTRY` values cannot silently fall back to static defaults. The existing
static planner rejects images carrying dynamic relocation metadata even when
they have no interpreter declaration, preventing unresolved code from entering
through the static path. The existing
static runtime stack is now populated by a single bounded auxiliary-vector
encoder using that common contract, and the interpreted proof checks the exact
word positions of its dynamic values. A bounded execution proof installs only
the interpreter's two-page RX extent and separate RW/NX page in the initial
user root. It
keeps absent leaves for the main image's read-only/NX header page, four RX
pages, two RW/NX pages, the primary dependency's two RX, one read-only/NX, and
one RW page, and the auxiliary dependency's one RX and one RW page. It
allocates each of those thirteen payload frames only when the matching authenticated mmap request
arrives. It also owns bounded heap, stack, and two `mmap` payload frames. The
proof verifies that the main and both dependency leaves begin absent, validates
their userspace-installed contents and permissions after execution, and
constructs the dynamic-process stack with the common auxiliary-vector encoder.
The compact planning fixture records an entry
at `0x600100`; mapped execution enters the linker-produced runtime interpreter
at `0x600200`.
The interpreter is assembled from `linux-interpreter.S`; the runtime path links
it as a five-header PIE with LLD, while the compact hand-built ELF remains a
parser and planning fixture. The linked interpreter retains the linker-emitted
two-page executable load: its entry and relocation core occupy the first page,
while the page-aligned runtime-file discovery code occupies the second. Both
pages are authenticated, materialized into separately owned frames, mapped RX,
validated after activation, scrubbed, and released as one bounded extent. The
linked interpreter also retains null-only dynamic symbol/string/GNU-hash tables; those tables are validated but
never used for resolution because its sole relocation is relative. The compact
fixture bounds raw interpreter text to its non-overlapping `0x100..0x400`
executable window. At runtime
the linked interpreter first validates its mapped ELF magic, requires a
56-byte program-header entry size, and bounds one through nine headers to its
first 4096-byte RX page. It accepts exactly one nonempty `PT_DYNAMIC` whose
equal file/memory size is a multiple of 16 bytes and contains at most 23
entries, then derives that table's runtime address with checked addition. The
compact fixture retains its fixed synthetic table offset to remain within its
768-byte raw-text bound. Both forms derive their page-aligned load bias and scan each bounded dynamic table for
the admitted primary relocation, split PLT relocation, symbol-table, and
hash-table tags rather than depending on tag order or embedding their values in
its instructions. The compact fixture first writes the load-biased value
`0x601234` to its writable relocation target `0x603080`; the linked interpreter
instead applies LLD's authenticated relative record to its linker-owned data
slot. The compact fixture then applies the dependency's relative
relocation at `0x6131c0` and self-resolves its eager `GLOB_DAT` and split-table
`JUMP_SLOT` targets at `0x6131c8` and `0x6131d0`; all three receive the
dependency entry value `0x610100`. It writes the admitted main-image relative
relocation value `0x400100` to the RELRO target `0x405100`, reads the main and
dependency symbol and hash-table addresses, requires symbol index one to be the
selected SysV main-image bucket and GNU dependency bucket, follows both symbol-name offsets
through their validated `DT_STRTAB` ranges, and compares the complete
NUL-terminated names before resolving the undefined main symbol to the
dependency entry. It writes that value through
`R_X86_64_GLOB_DAT` at `0x405108` and eager `R_X86_64_JUMP_SLOT` at `0x405110`
before jumping to the main entry at `0x400100`. The linker-produced graph main
instead uses two leading relative relocations for one eight-byte preinit array
and one eight-byte init array. Their relocated callback addresses are validated independently by
the kernel. In the synthetic fixture, the
main workload calls the dependency through the jump slot and dependency code
checks all three of its own relocation targets before returning the expected value. The main workload also
requires all three main-image relocated values for a dynamic startup and
untouched zero targets for each static startup; after it exits, the kernel also
validates the interpreter's relocated target through the still-active user root.
The runtime execution proof replaces the synthetic interpreter, main, and
dependency containers with LLD outputs. LLD links a five-header interpreter,
a nine-header `ET_DYN` PIE main with SysV hashing and one `PT_TLS`, and
header-unmapped
`libginux.so` and `libginux-aux.so` providers with GNU hashing. The main is
loaded with bias `0x3ff000`; the providers are authenticated against canonical
biases `0x610000` and `0x630000` and loaded at userspace-selected biases
`0x618000` and `0x638000`. Their
linker-owned GOT/PLT slots and split relocation tables drive the same userspace
interpreter and the complete syscall workload. The parser admits the exact
linker metadata used here: an in-range 32-byte `DT_PLTGOT`, zero `DT_DEBUG`,
`DT_RELACOUNT = 1` for the interpreter and dependencies,
`DT_RELACOUNT = 2` for the preinitialized main, and optional
`DT_FLAGS_1 = DF_1_PIE`; duplicates or other values are rejected. The main's linker metadata tail remains mapped for the
interpreter but is intentionally outside the workload's bounded syscall-copy
extent, preserving the existing `0x404000..0x405200` data contract.
The executable LLD-produced main carries ordered `DT_NEEDED` entries for
`libginux.so` and `libginux-aux.so`. The parser validates both names and the
auxiliary DSO's matching `DT_SONAME`, bounded GNU hash, symbol, and relocation
metadata. Two independent authority records bind the ordered names to exact
SHA-256 digests and page-aligned canonical biases. Planning authenticates both
files before parsing, requires both SONAMEs, and rejects digest substitution or
any pairwise overlap among the main, interpreter, and two dependency plans.
The userspace interpreter first applies the main's leading relative relocation
and invokes its one-entry preinit array with the original startup arguments.
Only then does it preserve dependency order, derive both runtime paths,
map each DSO in its independent authenticated slot, and apply each DSO's
`RELATIVE`, `GLOB_DAT`, and eager `JUMP_SLOT` relocations, then calls each
authenticated `DT_INIT` function followed by its relocated one-entry
`DT_INIT_ARRAY`. After resolving every main relocation, it calls the main's
`DT_INIT` function followed by its one-entry `DT_INIT_ARRAY` before transferring
to the main entry. For both dependencies and the main, the userspace
interpreter requires the load-biased `DT_INIT_ARRAY` address to equal the
relative-relocation target before writing or invoking it.
Each provider's direct init function requires the main preinit phase before it
can record its own first phase. Each provider records initialization in private writable data and its exported
function fails until that state is present, making constructor-before-use order
observable in ring 3. Each direct init function records a first phase that its
init array requires before recording completion. The main constructor calls both providers and records its
own private initialization state only if both calls succeed; the main entry
requires that state. This proves dependency constructors run before the main
constructor, which runs before the main entry. The interpreter retains the
original stack pointer and invokes the preinitializer and all three constructors
with `argc`, `argv`, and `envp` in the System V argument registers. Each constructor requires
`argc = 1`, the `/init` argument and null terminator, and the `GINUX=1`
environment entry and null terminator before publishing initialized state. The
main then calls both providers through its
linker-produced PLT, after the interpreter matches both complete names through
the three-bucket main SysV table and each provider's GNU table. The kernel
independently checks the main's two `GLOB_DAT` and two `JUMP_SLOT` values, both DSO mapping
permissions, code and entry bytes, all six relocation targets, lease cleanup,
frame scrubbing, and restoration of the pre-execution live-frame count.
After applying its self-relocation, the linked interpreter enters the syscall
gate only after consuming the initial stack metadata. It walks at most eight
environment pointers and 32 auxiliary-vector pairs, requires nonzero
`AT_PHDR`, `AT_PHNUM`, and `AT_ENTRY`, and requires `AT_BASE` to equal its
observed page-aligned load address. For the main image, the authenticated
file's `e_phoff` is subtracted from `AT_PHDR` with checked arithmetic and its
`e_phnum` must equal `AT_PHNUM`; this produces the load bias used by the
program-header walker. Final control transfer uses `AT_ENTRY`.

The interpreter then enters the syscall gate from its own admitted RX extent
and opens `/init`. It first inspects a
temporary read-only window, then reopens the file and walks its ELF program
headers in userspace. The walker requires a 56-byte `e_phentsize`, one through
nine headers wholly inside the two-page inspection window, readable
`PT_LOAD` entries, congruent file/virtual page offsets, `p_filesz <= p_memsz`,
one through four pages per load, no unknown permission bits, W^X, and no more
than seven installed pages per image. Checked arithmetic rejects wraparound.
It translates ELF `PF_R/PF_W/PF_X` into mmap protections and derives each
page address and file offset from the selected load bias. The resulting main
requests install offset `0` at `0x3ff000` read-only/NX, offsets `0x1000..0x4000` at
`0x400000..0x403000` RX, and offsets `0x5000..0x6000` at
`0x404000..0x405000` RW/NX through exact one-page `MAP_PRIVATE |
MAP_FIXED_NOREPLACE` requests. The last writable page is later sealed
read-only/NX by the admitted GNU RELRO transition. A repeated header request
returns `-EEXIST`, and a W+X text request returns `-EINVAL`.

This userspace walker is defense in depth over files already authenticated and
fully validated by the kernel plan; it is not yet an independent general ELF
validator. In particular, file identity, ELF class/machine, segment overlap,
entry validity, dynamic metadata, and the final admitted page set remain
kernel-enforced contracts.

The initial-thread execution profile allocates a separate owned RW/NX page for
the main image's TLS. It rounds `p_memsz` up to `p_align`, places that static
block below a 64-byte-aligned x86-64 variant-II thread pointer, copies `p_filesz`
bytes from `.tdata`, and retains zeroes through the `.tbss` tail. The following
16 bytes form a minimal TCB whose first word points to itself and whose second
word is zero. The kernel programs FS before entering the interpreter. Both the
main preinitializer and kernel verify the initialized value, zero tail, and
self pointer. The preinitializer names both hidden TLS symbols through
`R_X86_64_TPOFF32`. The build requires exactly two such object relocations, and
LLD resolves them to `-16` and `-8` without adding a dynamic relocation type.
On exit, the prior supervisor FS base is restored and the owned
TLS frame is zeroed and released; the live-frame count must return to its
pre-execution value. The later bounded `arch_prctl(ARCH_SET_FS)` proof may
replace the initial base, but is not the loader's TLS construction mechanism.
TLS relocations, multiple TLS modules, dynamic TLS, and per-thread allocation
remain outside this profile.

For each loaded file, exactly one nonempty `PT_DYNAMIC` must be present, its
file and memory sizes must match, its size must be a multiple of 16 bytes, and
it may describe at most 23 entries. The interpreter derives the runtime
dynamic-table address by adding the selected bias to `p_vaddr`; the linked main
and dependency scans no longer embed their dynamic addresses or entry counts.
The userspace scans require `DT_RELASZ = 96`, `DT_RELAENT = 24`,
`DT_RELACOUNT = 2`, `DT_PLTRELSZ = 48`, and `DT_PLTREL = DT_RELA` for the
main. Each dependency must instead declare `DT_RELASZ = 48`,
`DT_RELAENT = 24`, `DT_RELACOUNT = 1`, `DT_PLTRELSZ = 24`, and
`DT_PLTREL = DT_RELA`. A missing or different value fails before relocation.
The interpreter keeps its bounded startup scratch in the zero-fill tail after
its linker-emitted `.dynamic` and relocation target so that the bootstrap
dynamic-table offset remains invariant.

The main dynamic-table scan captures two ordered `DT_NEEDED` string-table
offsets, requires both bounded names, and also derives `DT_RELASZ`,
`DT_RELAENT`, `DT_RELACOUNT`, `DT_PREINIT_ARRAY`, and
`DT_PREINIT_ARRAYSZ`. It requires the exact 96-byte main primary table,
24-byte entries, two leading relative relocations, and one eight-byte preinit
array. Before writing or calling the first relocation, it requires that target
to equal the load-biased preinit address. It then constructs each
`/lib64/<needed-name>` on the mapped user stack. It inspects each derived file
before reopening it and applies the same program-header walker at selected
biases `0x618000` and `0x638000`. The primary requests map file offset `0x1000` RX at
`0x618000`, offset `0x2000` RX at `0x619000`, offset `0x3000` R/NX at
`0x61a000`, and offset `0x4000` RW/NX at `0x61b000`. The kernel accepts a
page-aligned base only within `0x610000..0x620000`, binds the first admitted
dependency request, and requires the later page to use the same bias while
preserving every relative displacement from a bounded eight-entry page
contract table derived from the authenticated plan. The current fixture
consumes four entries and proves RX, R/NX, and RW/NX mappings. A repeated first-page
request returns `-EEXIST`; a W+X executable request and an otherwise-valid page
using another in-window bias return `-EINVAL`. The auxiliary requests map file
offset `0x1000` RX at `0x638000` and offset `0x2000` RW/NX at `0x63b000`.
The main preinitializer runs before either dependency is discovered. Only after
that does the interpreter apply both dependencies' remaining relocations and
the main's remaining relocations. The
executable namespace admits exactly `libginux.so` and `libginux-aux.so`, so an
altered, reordered, or unterminated name fails closed. Syscall return
validation accepts only the declared main and interpreter RX extents. The
kernel authenticates and plans all images. Main and dependency payload frames
are allocated from the tracked kernel pool only when their admitted mmap
requests arrive; their content and leaf installation are userspace-driven.
The build still extracts raw `.text` for the static and synthetic parser
fixtures and rejects any nonempty `SHT_REL` or `SHT_RELA` section targeting
those bytes. The executable dynamic interpreter, main, and dependencies instead
retain LLD's complete ELF metadata.
The complete ring-3 probe therefore runs with dynamic `AT_BASE`, main-image
`AT_ENTRY`, and main-image `AT_PHDR`/`AT_PHNUM` values. Dependency dynamic
metadata, entry, and relocation targets are translated from the authenticated
canonical plan by the selected bias before validation. The kernel
restores its own root, scrubs every payload, releases all thirteen demand-allocated
main and dependency leases, and requires the kernel-owned live-frame count to
return to its pre-execution value. This profile performs two exact cross-object
name matches and calls both direct dependencies. It does not support transitive dependency graphs, search
paths, alternate SONAME selection, symbol versions, interposition, general multi-bucket lookup,
lazy binding, dependency preinit arrays, multi-entry preinit/init arrays, fini arrays,
a general-purpose dynamic linker, or glibc. The main bias and
entry are startup-metadata-driven. Each dependency selects among 16
page-aligned candidate biases in its own fixed window and receives frames on
demand, but each relative page layout remains a kernel-authenticated contract;
randomization and general placement are not implemented.

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
installed only for the bounded one- or two-page anonymous or
immutable-runtime-file `mmap` lifecycle. File-backed windows are private,
read-only/NX copies of authenticated bytes and are scrubbed during unmap.
Before changing CR3, the proof rejects either
segment, heap, or stack target page if it overlaps the loaded kernel PE image.
It then activates the root, observes the declared ELF mappings through their
user virtual addresses under SMAP, and enters CPL3 at the ELF
entry at `0x400100`, while syscall return validation remains bounded to the
declared main and interpreter executable extents. The proof instructions live
in checked-in x86-64
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
wholly inside the user stack page, and ranges inside the active mapping.
Copy-in additionally permits the declared main and interpreter RX ELF memory
extents; copy-out does not. The interpreter uses its RX-resident path strings
for `openat`; the main proof reads an embedded RX payload through `write` and
requires `-EFAULT` when `uname` targets that read-only page.
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
