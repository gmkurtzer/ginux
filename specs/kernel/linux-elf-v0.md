# Linux ELF64 Profile v0

**Status:** Strict static x86-64 parser, mapping, and one-shot ring-3 entry
profile verified in QEMU.

The first Linux-compatibility boundary accepts a deliberately bounded ELF64
profile. Files must be little-endian ELF class 64, current ELF version,
System-V or Linux ABI, `ET_EXEC`, and machine `EM_X86_64`. The ELF header and
program-header sizes must match the published 64-bit layout. Section headers,
processor flags, and extended counts are rejected rather than ignored.

Exactly two `PT_LOAD` program headers are required. A third `PT_GNU_STACK`
header is optional and must request read/write, non-executable memory with no
file or virtual range. Each load segment must be readable, nonempty in the
file, contained within the supplied image, aligned to a nonzero power of two
with congruent file and virtual addresses, and mapped wholly inside the lower
canonical user range beginning at `0x10000`. File size may be smaller than
memory size to represent zero-filled data. Physical addresses must be zero.
The segments must not overlap.

The profile requires exactly one executable segment and one writable segment.
Writable-executable mappings are rejected. The entry point must lie inside the
executable segment. The boot verifier accepts a synthetic image whose bounded
RX segment starts at file offset zero, includes its ELF/program headers and
code, and uses a non-page-base entry. Its RW segment has eight file bytes and a
512-byte memory extent. The verifier then rejects
bad magic, wrong class, non-executable file type, wrong machine, an out-of-range
entry point, W+X flags, overflowing file size, overlapping virtual ranges, an
executable GNU stack, a non-load segment header, and truncation. The same image
without the optional GNU stack header is also accepted.

The strict profile prevents unsupported features from being silently
interpreted with weaker semantics. It does not accept position-independent
`ET_DYN`, load `PT_INTERP`, allow general program-header ordering, perform
relocations, or support glibc.

## Page-image plan

The next bounded loader step accepts only segments whose virtual address is
page-aligned and whose complete memory image fits in one 4096-byte page. It
preserves the ELF virtual addresses and derives separate executable/read-only
and writable/non-executable page metadata. Materialization first zeroes the
whole destination page and then copies the file bytes. A load segment may begin
at file offset zero, as real ELF images commonly do, so the first mapped page
preserves the ELF and program-header tables and places instructions at their
file-relative offset. The writable segment's file tail through `p_memsz` and
all remaining page bytes are zero. The QEMU verifier checks the ELF magic,
exact RX instructions, RW data bytes, W^X metadata, and every zero-filled tail
byte.

The QEMU proof installs the two pages into a newly allocated x86-64
root at their declared `0x400000` and `0x401000` addresses. It privately clones
the affected PML4, PDPT, directory, and 2 MiB leaf region; replaces only the two
target leaves; retains the supervisor-only higher-half direct map; and verifies
RX versus RW/NX permissions plus exact materialized contents through the direct
map. A seventh owned page at `0x404000` supplies a RW/NX stack; its adjacent
pages remain supervisor-only. Before changing CR3, the proof rejects either
segment target page if it overlaps the
loaded kernel PE image. It then activates the root, observes both pages through
their declared user virtual addresses under SMAP, and enters CPL3 at the ELF
entry at `0x400100`, while syscall return validation remains bounded to its
`0x400000` executable page. The synthetic instruction stream first verifies
that unsupported call
999 returns Linux `-ENOSYS` without terminating it. It then issues a bounded
Linux `write(1, 0x401000, 8)` whose kernel copy-in verifies the RW payload. A
bounded `uname(0x401000)` then copies a complete zero-initialized `utsname` into
the declared data segment, where the probe validates the system name. It next
issues `getpid`, verifies PID 1 after the kernel resumes it through `SYSRETQ`,
verifies preserved first and fourth argument registers, and issues Linux
`exit(42)`. Those calls are
accepted only while an atomic probe flag is armed; exit returns directly to the
saved kernel stack. The proof restores and reads back the kernel root, then
clears and releases all seven root/table/payload/stack frames.

This proves instruction fetch and controlled return, not a Linux process. The
root retains supervisor-only low identity leaves around the ELF pages so the
current low-linked kernel can survive the proof transition. The initial stack
pointer is 16-byte aligned and names `argc = 1`, an `argv[0]` pointer to the
NUL-terminated `/init` string, a null `argv` terminator, null `envp`,
`AT_PHDR` pointing at the mapped program-header table, `AT_PHENT = 56`,
`AT_PHNUM` matching the parsed header count, `AT_PAGESZ = 4096`, `AT_ENTRY`
equal to the ELF entry, and `AT_NULL`. The probe dereferences and validates
`argv[0]` and `AT_ENTRY` from CPL3 before its syscall sequence. An environment,
the complete Linux auxiliary vector, scheduler ownership, the broader syscall
surface, and general collision policy beyond the loaded kernel image are not
defined yet. The loader does not relocate `ET_EXEC` images to the existing high
test address, because doing so would violate ELF semantics and hide these
remaining prerequisites.

The syscall transition is specified separately in
[Linux Syscall Entry v0](linux-syscall-v0.md).
