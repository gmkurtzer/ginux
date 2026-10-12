# Persistent Volume Service v0

**Status:** Capability-separated ring-3 writer and replacement reader verified
under control ABI v4. The backing media remains an in-kernel memory model.

## Authority and lifecycle

Boot object 6 names the bounded persistent-volume service boundary. The signed
development manifest grants its initial control plane `READ|WRITE`; the boot
proof attenuates that authority into two isolated stop-only services:

- a write-only service may commit but cannot probe, query, or read; and
- a read-only replacement may probe, query, and read but cannot commit.

Both domains receive opaque generation-tagged handles. Retirement revokes the
handle and reclaims the complete W^X address space. Process ancestry and entry
into the call gate grant no additional authority.

The writer commits `volume-service\n` as generation one, then executes an
invalid opcode. The kernel retires that exact service generation and publishes
its fault lifecycle event without discarding the mediated volume. A fresh
service identity receives a new read-only capability and recovers the exact
generation and payload. Its denied commit leaves the generation unchanged.

## Control ABI v4 operation 18

`RDI` is a capability handle for object 6, `RSI` selects the command, and `RDX`
supplies its argument. Denial or malformed input returns `UINT64_MAX - 2`.
A stale expected generation returns `UINT64_MAX - 4`.

| Command | Required right | Argument | Result |
|---:|---|---|---|
| 0 | `READ` | zero | descriptor `0x0000000200400001` |
| 1 | `READ` | zero | current generation, or zero when empty |
| 2 | `READ` | pointer to a 32-byte request | copied payload length |
| 3 | `WRITE` | pointer to a 32-byte request | committed generation |

The descriptor encodes service-contract version 1 in bits 0-15, maximum
transfer 64 in bits 16-31, and the two underlying record slots in bits 32-63.

Requests contain a little-endian `u32` size of 32, control ABI `u16` version
4, reserved-zero `u16`, and `u64` expected generation, buffer address, and
buffer size. Read and commit sizes must be 1 through 64. The kernel stages the
complete request and commit payload through the existing owned-page user-copy
checks before mutation. Read stages the complete result before copy-out and
does not touch bytes beyond the returned length.

Commit uses optimistic concurrency: zero is the only accepted expected
generation for an empty volume, and every later commit must name the exact
current generation. The two-slot persistent-volume algorithm performs its
prepare/flush/commit/final-flush sequence before returning the new generation.

## Deliberate limits

- The live backend is volatile kernel memory, so state survives service failure
  but not a QEMU or machine restart.
- The control operation currently hosts the volume engine in the kernel as a
  bounded reference path. M6 must move the storage data path behind an isolated
  block service while retaining the authority and recovery semantics.
- The transfer limit is 64 bytes although the record format can hold 440.
- There is one volume, one serialized caller at a time, and no cancellation,
  queueing, sharing, encryption, filesystem, or Linux block-device exposure.
- The kernel boot proof delegates object 6 from the signed control-plane root;
  the general userspace orchestration path does not yet create that service.
- The separate kernel-bootstrap proof provides virtio-blk DMA, real flushes,
  and reboot recovery, but this ring-3 service is not yet connected to it.
  NVMe, interrupt completion, and performance measurements remain
  unimplemented.
