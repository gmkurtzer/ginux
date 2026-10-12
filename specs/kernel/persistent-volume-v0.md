# Persistent Volume Record v0

**Status:** Two-slot format and recovery algorithm verified against an in-memory
block-device model during every QEMU boot. No physical or virtual block-device
backend is connected yet.

## Purpose

This contract is the first M6 persistent-storage primitive. It defines how one
bounded value becomes durable without overwriting the last committed value.
It is a block format and ordering contract for a future isolated storage
service, not a filesystem or a claim of machine-reboot persistence.

## Device contract

The volume occupies exactly two 512-byte sectors. Its backend provides whole
sector reads and writes plus an explicit flush. A successful flush must make
all preceding writes durable before any later write becomes durable. A crash
discards writes that have not crossed a successful flush. Read, write, and
flush failures are reported and a failed or ambiguous scan is never treated as
an empty volume.

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

The kernel reference backend separates pending writes from durable sectors.
The boot proof creates generation one and crashes independent device copies
before the prepared write, after the prepared write, after the first flush,
after the commit write, and after the final flush. Each recovery returns
exactly the old or new complete payload required above.

The proof also checks every partial commit-marker length, corruption in each
header class, the digest, reserved bytes, and payload, fallback from a corrupt
new slot to the old slot, oversized payload rejection, generation exhaustion,
and equal-generation ambiguity. The automated QEMU gate requires all three
volume validation markers before accepting the boot.

## Deliberate limits

- The reference backend is volatile memory and does not survive QEMU restart.
- The format holds one value of at most 440 bytes; it has no allocation,
  filesystem, journal, names, snapshots, or concurrency.
- There is no storage-service capability ABI or Linux block-device exposure.
- There is no virtio-blk, NVMe, DMA, interrupt, or hardware queue backend.
- SHA-256 is an integrity checksum here, not a signature or MAC.
- Throughput, p99 latency, endurance, discard, barriers beyond flush, and
  multi-sector atomicity remain M6 work.
