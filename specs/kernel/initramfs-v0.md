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

The generated proof contains `bin`, `dev`, `etc`, `proc`, `sys`, `tmp`, `var`,
`var/lib`, `var/lib/state`, `var/lib/version`, the native `init` marker,
`etc/hostname`, `etc/os-release`, `etc/motd`, the validated
`etc/ginux-system-profile`, and four build-produced Linux ELF artifacts
under internal `linux/*` names: the main, interpreter, primary DSO, and auxiliary
DSO. Two distinct static exec targets are published at their Linux-visible
measured-root paths `bin/signal-probe` and `bin/exec-second`. Both use separately
bounded two-page RX layouts; the first is dedicated to measured signal-runtime
behavior, while the second retains an independent address layout. The hostname is immutable mode `0555` with exact contents
`ginux\n`; `os-release` is immutable mode `0555` with exact contents
`NAME=Ginux\nID=ginux\n`; `motd` contains `Welcome to Ginux\n`; and the nested
state file contains `ready\n` and the version marker contains `0\n`. The archive
also contains a separately linked static candidate at `sbin/init`. Its one-page
RX and one-page RW plan is validated from the measured root. A dedicated
measured exec workload launches it as PID 1, opens the measured console, emits
its exact readiness line, and exits with complete descriptor teardown. The
mode-`0444` profile file binds the
candidate's architecture, canonical init path, console, root format, and
machine-readable ready/partial/missing compatibility rows. The archive layer
requires nonempty executable entries but does not compare them with
kernel-embedded artifact bytes. The signed root-image digest authenticates the
complete archive; after publication, the ELF planners parse and bind the
selected measured files to their canonical paths, interpreter name, dependency
order, SONAMEs, and bounded load layout.

After validation, the current roughly 90 KiB proof image is copied into a
fixed 32-frame, 128 KiB physically contiguous kernel-owned allocation. The
boot proof requires exact bytes, a zero tail across the remaining allocation,
ownership of every frame, sequential physical and direct-map addresses, and no
source alias. It deliberately mutates and restores the private copy to prove
the measured boot source does not change, then scrubs the complete allocation
before releasing every lease. The parser's general 16 MiB bound remains
broader than this v0 publication capacity; images above 128 KiB fail closed
during materialization.

A second lifecycle proof constructs a fixed index of at most 64 nodes. The
22-entry proof crosses the former 16-node limit while remaining representable
by the private one-byte descriptor selector. Each
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
directories, and reports end of the nine-entry proof root without copying. The
published allocation remains allocator-owned after the control-plane domain exits.

The bounded Linux syscall profile resolves the four `/etc` regular files
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
Linux `/` directory enumerates nine direct measured children in archive order.
Its fixed 256-byte staging buffer returns the first eight children through
`bin` on one page and the explicit `sbin` directory on a second cursor page.
Measured directory descriptors use their own bounded node
selectors to recover names from the index. The measured `sys` node is empty.
The measured `proc` node anchors synthetic bounded `1/status`, `1/mountinfo`,
`1/uid_map`, `1/gid_map`, and `1/setgroups` runtime views; none adds mutable
bytes to the authenticated archive. The PID 1 proof enumerates both synthetic
directories, reads the status record, and verifies the immutable root mount
and user-namespace records.
The measured `tmp` node similarly anchors a kernel-owned, 64-byte
`ginux-ready` scratch file whose contents are outside the signed archive and
are scrubbed at process teardown.
The proof also opens and enumerates the
otherwise unlisted `var` node, then opens its indexed `lib` child relative to
the live descriptor. `dev`
retains its synthetic device children while its directory node comes from the
measured root. An open measured-directory descriptor resolves any direct
regular child by constructing a bounded path and consulting the retained
index; the current indexed working directory resolves bounded relative paths the
same way. Multi-component names are accepted only when every component is
nonempty and is neither `.` nor `..`; the proof rejects `../lib`. The proof
obtains `motd` metadata without a kernel filename case. A regular file
descriptor is rejected as a lookup base. The
file or directory selector is bounded by the 64-node publication index and is neither a
persistent inode nor a public ABI.

The Linux-visible `/init` and `/lib64/*` names resolve to the four internal
`linux/*` entries in the same retained index. ELF parsing still uses the
compile-time authenticated plans as an independent shape and identity check,
but every runtime file read and mapping performed by the userspace interpreter
is sourced from the measured root allocation. The QEMU proof executes the
complete main/interpreter/two-DSO graph through those entries.

The native `init` entry remains a format marker for the frozen control ABI
proof; it is not PID 1. The archive-backed Linux graph is a proof workload, not
a runnable system-container image. Its root is the sole immutable mount in the
bounded initial Linux namespace, not a general VFS. Mutable and multiple mount semantics,
ownership beyond normalized root metadata, hard links, special nodes, extended
attributes, compression, loading the image separately from the EFI binary,
mount syscalls, mutable nodes, and VFS or service
publication are outside v0.
