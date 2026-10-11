# Initramfs Archive Profile v0

**Status:** Deterministic proof image parsed during QEMU boot.

Ginux uses a bounded ASCII `newc` CPIO profile as the first step toward a
read-only system-container root. The kernel build generates the proof archive
deterministically and embeds it in the EFI/kernel image, so it cannot be
replaced independently after build. Its deployment authentication inherits the
platform image trust chain; a separately measured root image remains required
before the first real system container.

The v0 parser accepts only `070701` entries with lowercase eight-digit
hexadecimal fields. Inodes must be nonzero. UID, GID, modification time, device
numbers, and checksum must be zero; link count must be one. Names are NUL terminated, at most 255
bytes excluding the terminator, relative, and composed of ASCII letters,
digits, `_`, `-`, `.`, and `/`. Empty, `.`, and `..` path components, duplicate
paths, nonzero alignment padding, trailing bytes, and missing or malformed
trailers are rejected.

The archive is limited to 16 MiB, 256 non-trailer entries, and 8 MiB per file.
Only regular files, directories, and nonempty symbolic links are admitted.
Directories carry no data. An archive must contain exactly one nonempty,
executable regular file named `init`.

The generated proof contains `dev`, `etc`, `proc`, `sys`, `tmp`, and `init`.
The `init` data is only a format marker; it is not yet a Linux ELF executable,
PID 1, a mounted VFS root, or a runnable container image. Mount semantics,
ownership beyond normalized root metadata, hard links, special nodes, extended
attributes, compression, independent image measurement, and extraction into
owned pages are outside v0.
