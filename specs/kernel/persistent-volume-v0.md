# Persistent Volume Record v0

**Status:** Two-slot format and recovery algorithm verified against both the
in-memory fault model and a flushed virtio-blk backend across two QEMU boots.
The ring-3 service path still uses the in-memory backend.

## Purpose

This contract is the first M6 persistent-storage primitive. It defines how one
bounded value becomes durable without overwriting the last committed value.
It is a block format and ordering contract for a future isolated storage
service, not a filesystem.

## Device contract

The volume occupies exactly two 512-byte sectors. Its backend provides whole
sector reads and writes plus an explicit flush. A successful flush must make
all preceding writes durable before any later write becomes durable. A crash
discards writes that have not crossed a successful flush. Read, write, and
flush failures are reported and a failed or ambiguous scan is never treated as
an empty volume.

The encoder and decoder live in the workspace's `ginux-volume-format` `no_std`
crate. The kernel backend and the future isolated storage service therefore
consume one canonical implementation of the magic, version, field offsets,
payload bounds, commit marker, zero-padding rules, and SHA-256 validation. The
crate rejects generation zero and oversized payloads before encoding, treats a
prepared record as uncommitted, and contains no device or kernel authority.

The prepare/flush/commit/flush state machine and two-slot recovery scan live in
the separate `ginux-volume-service` `no_std` crate. It depends only on the
format crate and a three-method whole-sector `BlockDevice` trait. The current
kernel backend implements that trait, while the isolated virtio service can
implement the same interface without importing kernel internals. Host unit
tests and the QEMU reboot/corruption gate execute this shared engine.

Canonical empty media consists of two all-zero sectors. If neither slot is
valid and either sector contains any nonzero byte, recovery reports corruption
instead of permitting generation-one initialization. This includes an
interrupted first commit whose prepared record reached durable media but whose
commit marker did not. A valid older slot still permits recovery when the other
slot is corrupt, preserving the intended rollback behavior.

Real backends must document their atomic-write and flush guarantees. The
format detects a torn sector through its digest, but cannot repair a backend
that lies about completed flushes or loses both slots.

## Canonical sector format

All integers are little-endian. Every byte not assigned below must be zero.

| Offset | Size | Field |
|---:|---:|---|
| 0 | 8 | ASCII magic `GINUXPV0` |
| 8 | 4 | format version `1` |
| 12 | 2 | header size `72` |
| 14 | 2 | payload length, at most 440 |
| 16 | 8 | nonzero generation |
| 24 | 8 | commit marker `0x3144455454494d43` |
| 32 | 32 | SHA-256 digest |
| 64 | 8 | reserved zero bytes |
| 72 | 440 | payload followed by a zero tail |

The digest is SHA-256 over all 512 bytes with the digest field replaced by
zeros. It detects corruption; it does not authenticate the record. Authority
and confidentiality belong to the storage-service capability and encryption
layers.

A sector is valid only when every canonical field, the complete commit marker,
the payload bound, the zero tail, and the digest validate. Recovery selects the
valid sector with the highest generation. Equal valid generations are
ambiguous and fail closed. Generation zero is invalid and generation
`UINT64_MAX` cannot be advanced.

## Commit ordering

The inactive slot is updated in five observable stages:

1. Preserve the current committed slot.
2. Write the next-generation sector with a zero commit marker but the digest
   of its final committed form.
3. Flush that prepared sector.
4. Write the final sector containing the commit marker.
5. Flush the committed sector.

Before the final flush, recovery returns the previous generation. After the
final flush, recovery returns the new generation. A prepared sector or a
partially persisted commit marker fails validation, leaving the previous slot
available.

## Boot proof

The in-memory reference backend separates pending writes from durable sectors.
Its boot proof creates generation one and crashes independent device copies
before the prepared write, after the prepared write, after the first flush,
after the commit write, and after the final flush. Each recovery returns
exactly the old or new complete payload required above.

The proof also checks every partial commit-marker length, corruption in each
header class, the digest, reserved bytes, and payload, fallback from a corrupt
new slot to the old slot, oversized payload rejection, generation exhaustion,
and equal-generation ambiguity.

A second proof binds the same `BlockDevice` contract to the modern virtio-blk
queue. The automated gate creates a fresh 1 MiB raw disk, boots once to commit
generation one into sectors zero and one, and boots a second VM with the same
disk. The recovery boot must return generation one and the exact payload
without taking the initialization path. Both boots must complete device reset,
bus-master revocation, DMA scrubbing, and the normal kernel success marker. See
[Virtio-backed Persistent Volume v0](virtio-volume-v0.md).

The gate then copies that disk, corrupts the only committed slot, and attempts
a third boot. The kernel must reset the device, revoke bus mastering, scrub the
DMA lease, reject the volume, and emit its fail-closed panic marker without
modifying the corrupted image. This is rejection testing, not repair.

## Deliberate limits

- The capability-separated ring-3 service still uses volatile kernel memory;
  only the bootstrap proof is bound to virtio-blk.
- The format holds one value of at most 440 bytes; it has no allocation,
  filesystem, journal, names, snapshots, or concurrency.
- Control ABI v4 operation 18 exposes a bounded 64-byte capability-separated
  service path; there is no Linux block-device exposure.
- There is no NVMe backend, general interrupt-driven completion, isolated
  storage service, or IOMMU confinement for the ring-3 volume service. The
  kernel-reference virtio backend has a separate bounded VT-d profile.
- SHA-256 is an integrity checksum here, not a signature or MAC.
- Throughput, p99 latency, endurance, discard, barriers beyond flush, and
  multi-sector atomicity remain M6 work.
