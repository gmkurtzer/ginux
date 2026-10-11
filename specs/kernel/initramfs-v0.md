# Initramfs Archive Profile v0

**Status:** Independently measured deterministic proof image parsed during QEMU boot.

Ginux uses a bounded ASCII `newc` CPIO profile as the first step toward a
read-only system-container root. The manifest tool generates the proof archive
deterministically, records its exact size and SHA-256 digest in the signed boot
manifest, and embeds the bytes in the EFI image. Handoff v4 carries the range;
the kernel verifies the measurement before parsing any archive metadata.

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
After validation, the current one-page proof image is copied into a newly
allocated, zero-initialized kernel-owned frame. The boot proof requires exact
bytes, a zero tail, allocator ownership, and no source alias. It deliberately
mutates and restores the private copy to prove the measured boot source does
not change, then scrubs the complete frame before releasing its lease.

A second lifecycle proof constructs a fixed index of at most 16 nodes. Each
node stores only checked name and data offsets inside the owned image; no node
retains a pointer into the handed boot source. Exact `init` and directory
lookups resolve into the owned frame, while missing, absolute, and traversal
paths fail. Destroying the index scrubs and releases its image frame.

The `init` data is only a format marker; it is not yet a Linux ELF executable,
PID 1, a mounted VFS root, or a runnable container image. Mount semantics,
ownership beyond normalized root metadata, hard links, special nodes, extended
attributes, compression, loading the image separately from the EFI binary,
multi-page materialization, persistent ownership, directory traversal APIs,
mounts, mutable nodes, and VFS or service publication are outside v0.
