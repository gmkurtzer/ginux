# Linux Bounded Tmpfs v0

**Status:** Implemented for `minimal-system-v0`.

The measured read-only root supplies the `/tmp` mount-point anchor. The kernel
installs a separate volatile filesystem object before entering Linux PID 1;
no mutable bytes are part of the authenticated root image.

This version admits exactly one regular file named `ginux-ready`. The directory
starts empty. Absolute `openat(AT_FDCWD, "/tmp/ginux-ready",
O_WRONLY | O_CREAT | O_EXCL)` creates a zero-length, mode `0600` file with a
64-byte capacity. A second exclusive creation returns `-EEXIST`. Opening an
absent file without `O_CREAT` returns `-ENOENT`.

Each creation assigns a monotonically increasing, nonzero generation to the
fixed inode. An open descriptor records that generation. Read and write paths
must resolve the descriptor's exact live generation; a retired or substituted
generation is rejected. Descriptor offsets are independent. Writes stage the
complete userspace input before mutation and return `-ENOSPC` if the resulting
extent exceeds 64 bytes.

`getdents64` always reports `.` and `..`, and reports `ginux-ready` only while
the live generation is linked. `unlinkat` supports the exact absolute pathname
with zero flags. This bounded version rejects unlink while a matching
descriptor is open with `-EBUSY`. After the final close, unlink atomically
removes the file and explicitly zeroes all 64 bytes and the recorded length
using volatile stores. Final Linux process teardown performs the same scrub if
the file remains linked.

The in-kernel verifier creates, writes, reads, unlinks, rejects the stale
handle, reuses the inode with a later generation, verifies the replacement is
empty, and scrubs it. PID 1 independently proves the external syscall
lifecycle: empty enumeration, exclusive creation, write, populated
enumeration, close/reopen/read, unlink, failed lookup, and empty enumeration.

The implementation is serialized by the current single-CPU Linux execution
profile. General locking, multiple files, subdirectories, relative creation or
unlink, rename, truncation, vector I/O, mapping, and Linux unlink-while-open
semantics are not provided by v0.
