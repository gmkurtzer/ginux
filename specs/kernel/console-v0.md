# Bootstrap Console Service v0

**Status:** Capability-gated output and bounded interactive input verified from ring 3.

Call-gate operation 6 writes one through eight bytes supplied inline in a
register. The bytes must be printable ASCII, carriage return, or newline. The
caller must present `WRITE` authority for console object 2; a handle carrying
rights over another object is rejected before output. Success returns the
accepted byte count and failure returns `UINT64_MAX - 2`.

The control-plane hardware proof first attempts the operation with its
host-control handle and requires denial. It then emits `USER_OK\n` with the
manifest-derived console handle and requires a result of eight. The isolated
service receives no console handle and starts after this code path. The QEMU
gate requires exactly one complete record and the kernel's post-run marker.

Call-gate operation 9 copies one through 64 printable bytes from a mapped
userspace range and writes them atomically. Call-gate operation 31 blocks while
reading one line from the pinned COM1 console, accepts printable ASCII and
backspace, and copies at most 32 bytes into a writable userspace range. Input
requires `READ` authority for console object 2; output requires `WRITE`.
Successful input renews the bounded call epoch so the resident controller can
remain interactive without acquiring new authority.

The C1 resident controller is a measured static PIE embedded in `GINUX.EFI`.
It owns a read/write console capability, parses `help`, `status`, `components`,
and `shutdown` in ring 3, and exits cleanly to request firmware shutdown. The
kernel never interprets controller commands.

This is a synchronous bootstrap primitive, not a production console service.
It spins on QEMU COM1 in kernel context and has no multiplexing, Unicode policy,
input cancellation, or device-failure recovery. A later console implementation
must preserve the capability and bounded-copy checks at its boundary.
