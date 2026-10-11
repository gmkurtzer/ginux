# Bootstrap Console Service v0

**Status:** Capability-gated immediate writes verified from ring 3.

Call-gate operation 6 writes one through eight bytes supplied inline in a
register. The bytes must be printable ASCII, carriage return, or newline. The
caller must present `WRITE` authority for console object 2; a handle carrying
rights over another object is rejected before output. Success returns the
accepted byte count and failure returns `UINT64_MAX - 2`.

Both hardware proof contexts first attempt the operation with their
host-control handle and require denial. They then emit `USER_OK\n` with the
manifest-derived console handle and require a result of eight. The QEMU gate
requires exactly two complete records and the kernel's post-run marker.

This is a synchronous bootstrap primitive, not a production console service.
It spins on QEMU COM1 in kernel context and has no buffering, backpressure,
endpoint lifecycle, multiplexing, Unicode policy, input path, cancellation, or
failure recovery. A later isolated console service must replace direct device
access while preserving the capability check at its boundary.
