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
