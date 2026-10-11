# Linux ELF64 Profile v0

**Status:** Strict static x86-64 parser, mapping, and one-shot ring-3 entry
profile verified in QEMU.

The first Linux-compatibility boundary accepts a deliberately bounded ELF64
profile. Files must be little-endian ELF class 64, current ELF version,
System-V or Linux ABI, `ET_EXEC`, and machine `EM_X86_64`. The ELF header and
program-header sizes must match the published 64-bit layout. Section headers,
processor flags, and extended counts are rejected rather than ignored.

Exactly two program headers are accepted and both must be `PT_LOAD`. Each
segment must be readable, nonempty in the file, contained within the supplied
image, aligned to a nonzero power of two with congruent file and virtual
addresses, and mapped wholly inside the lower canonical user range beginning at
`0x10000`. File size may be smaller than memory size to represent zero-filled
data. Physical addresses must be zero. The segments must not overlap.

The profile requires exactly one executable segment and one writable segment.
Writable-executable mappings are rejected. The entry point must lie inside the
executable segment. The boot verifier accepts a synthetic image with a one-byte
RX segment and an eight-byte file/sixteen-byte memory RW segment, then rejects
bad magic, wrong class, non-executable file type, wrong machine, an out-of-range
entry point, W+X flags, overflowing file size, overlapping virtual ranges, a
non-load program header, and truncation.

This milestone is parser evidence only. It does not map ELF segments, zero BSS,
construct a Linux initial stack or auxiliary vector, execute Linux machine
code, implement Linux syscalls, accept position-independent `ET_DYN`, load
`PT_INTERP`, perform relocations, or support glibc. Those are separate M4
milestones. The strict profile prevents unsupported features from being
silently interpreted with weaker semantics while those pieces are absent.

## Page-image plan

The next bounded loader step accepts only segments whose virtual address is
page-aligned and whose complete memory image fits in one 4096-byte page. It
preserves the ELF virtual addresses and derives separate executable/read-only
and writable/non-executable page metadata. Materialization first zeroes the
whole destination page and then copies the file bytes, so the writable
segment's file tail through `p_memsz` and all remaining page bytes are zero.
The QEMU verifier checks the exact RX instruction byte, RW data bytes, W^X
metadata, and every zero-filled byte in both pages.

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
entry. The synthetic instruction stream issues Linux `getpid`, verifies PID 1
after the kernel resumes it through `SYSRETQ`, and then issues Linux `exit(42)`.
Those calls are accepted only while an atomic probe flag is armed; exit returns
directly to the saved kernel stack. The proof restores and reads back the kernel
root, then clears and releases all seven root/table/payload/stack frames.

This proves instruction fetch and controlled return, not a Linux process. The
root retains supervisor-only low identity leaves around the ELF pages so the
current low-linked kernel can survive the proof transition. The initial stack
pointer is 16-byte aligned and names `argc = 0`, null `argv` and `envp`, an
`AT_PAGESZ = 4096` pair, and `AT_NULL`; the probe checks `argc` from CPL3 before
exiting. Argument strings, a complete auxiliary vector, scheduler ownership,
the supported syscall surface, and general collision policy beyond the loaded
kernel image are not defined yet. The loader does not relocate `ET_EXEC` images
to the existing high test address, because doing so would violate ELF semantics
and hide these remaining prerequisites.

The syscall transition is specified separately in
[Linux Syscall Entry v0](linux-syscall-v0.md).
