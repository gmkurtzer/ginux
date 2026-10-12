# Linux Bounded Devfs v0

**Status:** Implemented for `minimal-system-v0`.

The authenticated root contains an immutable `/dev` mount-point anchor. The
Linux compatibility layer supplies a fixed synthetic directory with mode
`0555` and exactly four character-device entries: `null`, `zero`, `console`,
and `tty`. Directory enumeration also returns `.` and `..`; the complete six
record result is 152 bytes. The namespace cannot be mutated.

`/dev/null` returns EOF on reads and discards writes. `/dev/zero` returns up to
256 validated zero bytes and discards writes. `/dev/console` is the bounded
serial endpoint with EOF input, a 256-byte write limit, fixed terminal metadata
and geometry, and non-seekable behavior. `/dev/tty` is a strict alias of that
endpoint. Access mode, close-on-exec state, metadata, access checks, scalar and
bounded vector I/O, and descriptor close behavior follow the detailed syscall
contract in [Linux Syscall Surface v0](linux-syscall-v0.md).

PID 1 opens and enumerates `/dev`, validates all four names, exercises null
discard and EOF, verifies zero-fill output, and issues a terminal geometry
query through the tty alias. Its separately opened console descriptor carries
the `DEVFS_READY` and final readiness records. All auxiliary descriptors are
closed before process teardown.

Dynamic device discovery, node creation or removal, ownership changes,
pseudoterminals, controlling-terminal and session management, arbitrary
ioctls, and device classes beyond these four nodes are outside v0.
