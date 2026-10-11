# Linux ELF64 Profile v0

**Status:** Strict static x86-64 parser profile verified in QEMU; loading and
execution are not yet implemented.

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

The pages are not installed into a page table yet. The current kernel relies on
a low identity map in PML4 slot zero, which conflicts with safely mapping a
normal Linux `ET_EXEC` image at `0x400000` while retaining a stable kernel
physical-memory view. Ginux must first establish a higher-half kernel mapping
and/or separate privileged direct map. The loader will not relocate `ET_EXEC`
images to the existing high test address, because doing so would violate ELF
semantics and hide this prerequisite.
