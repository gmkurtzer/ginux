# Initramfs Archive Profile v0

**Status:** Independently measured deterministic proof image published read-only during QEMU boot.

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

The generated proof contains `dev`, `etc`, `proc`, `sys`, `tmp`, `var`,
`var/lib`, `var/lib/state`, the native `init` marker, `etc/hostname`,
`etc/os-release`, `etc/motd`, and four build-produced Linux ELF artifacts
under internal `linux/*` names: the main, interpreter, primary DSO, and
auxiliary DSO. The hostname is immutable mode `0555` with exact contents
`ginux\n`; `os-release` is immutable mode `0555` with exact contents
`NAME=Ginux\nID=ginux\n`; `motd` contains `Welcome to Ginux\n`; and the nested
state file contains `ready\n`. The ELF entries must byte-match the authenticated artifacts compiled
into the kernel's bounded execution plan.

After validation, the current roughly 73 KiB proof image is copied into a
fixed 32-frame, 128 KiB physically contiguous kernel-owned allocation. The
boot proof requires exact bytes, a zero tail across the remaining allocation,
ownership of every frame, sequential physical and direct-map addresses, and no
source alias. It deliberately mutates and restores the private copy to prove
the measured boot source does not change, then scrubs the complete allocation
before releasing every lease. The parser's general 16 MiB bound remains
broader than this v0 publication capacity; images above 128 KiB fail closed
during materialization.

A second lifecycle proof constructs a fixed index of at most 16 nodes; the
current proof deliberately occupies all 16 slots. Each
node stores only checked 32-bit name and data offsets inside the owned image; no node
retains a pointer into the handed boot source. Exact `init` and directory
lookups resolve into the owned allocation, while missing, absolute, and
traversal paths fail. Destroying the index scrubs and releases all image frames.

The runtime publication then materializes a third indexed copy and retains its
32 frames under kernel ownership through shutdown. Signed manifest object 5 grants
the control plane `READ` authority only. Control ABI v3 operation 17 exposes a
versioned profile probe, exact-path metadata lookup, reads of at most 64 bytes,
and cursor-based direct-child enumeration.
Path inputs are copied into a 255-byte kernel staging buffer; metadata and file
data are fully staged before bounded copy-out. The ring-3 proof resolves `init`,
validates mode `0100555` and size 24, and reads its complete contents. It also
requires denial through the console capability, rejects `../init`, and rejects
a null output range without mutating the root. Root enumeration returns one
fully staged metadata-and-name record per call, rejects regular files as
directories, and reports end of the seven-entry proof root without copying. The
published allocation remains allocator-owned after the control-plane domain exits.

The bounded Linux syscall profile resolves the three `/etc` regular files
directly from that same retained index. Auxiliary Linux
descriptor state stores a one-based measured-root node selector independently
of the bounded file role. Runtime reads therefore recover bytes by index rather
than selecting hard-coded kernel slices. Its ring-3 interpreter opens both
files read-only, validates their exact contents and metadata, and requires
writable access to fail. The
interpreter also opens `/etc` as an immutable directory and obtains its
`hostname`, `os-release`, and `motd` children from the retained index through
`getdents64`. It verifies the `.` and `..` records, file types, exact child names, directory metadata, EOF,
cursor rewind, and atomic rejection of an undersized output buffer. The
Linux `/` directory likewise enumerates the seven direct measured children in
archive order. Measured directory descriptors use their own bounded node
selectors to recover names from the index. The measured `proc`, `sys`, `tmp`,
`proc`, `sys`, and `tmp` nodes are empty. The proof opens and enumerates the
otherwise unlisted `var` node, then opens its indexed `lib` child relative to
the live descriptor. `dev`
retains its synthetic device children while its directory node comes from the
measured root. An open measured-directory descriptor resolves any direct
regular child by constructing a bounded path and consulting the retained
index; the fixed root working directory resolves bounded relative paths the
same way. Multi-component names are accepted only when every component is
nonempty and is neither `.` nor `..`; the proof rejects `../lib`. The proof
obtains `motd` metadata without a kernel filename case. A regular file
descriptor is rejected as a lookup base. The
file or directory selector is bounded by the 16-node publication index and is neither a
persistent inode nor a public ABI.

The Linux-visible `/init` and `/lib64/*` names resolve to the four internal
`linux/*` entries in the same retained index. ELF parsing still uses the
compile-time authenticated plans as an independent shape and identity check,
but every runtime file read and mapping performed by the userspace interpreter
is sourced from the measured root allocation. The QEMU proof executes the
complete main/interpreter/two-DSO graph through those entries.

The native `init` entry remains a format marker for the frozen control ABI
proof; it is not PID 1. The archive-backed Linux graph is a proof workload, not
a runnable system-container image, and the bounded root view is not a general
mount. Mount semantics,
ownership beyond normalized root metadata, hard links, special nodes, extended
attributes, compression, loading the image separately from the EFI binary,
mounts, mutable nodes, and VFS or service
publication are outside v0.
