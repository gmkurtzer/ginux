# Virtio-backed Persistent Volume v0

**Status:** Bounded M6 QEMU reboot-persistence proof.

## Purpose

This contract binds the two-sector persistent-volume record to a real modern
virtio-blk queue. It proves that the prepare/flush/commit/flush sequence reaches
the QEMU raw disk and that a new VM can recover the exact committed record. It
does not yet make the block device available to a ring-3 service or container.

## Disk and queue profile

The deterministic Q35 machine exposes one 1 MiB modern virtio-blk function with
2,048 512-byte sectors. Sectors zero and one hold the two record slots. Sector
two remains reserved and blank for the non-mutating read probe.

One zeroed, generation-owned 4 KiB DMA page contains an eight-entry split ring,
one 16-byte request header, one 512-byte data buffer, one status byte, and guard
bytes. Queue zero permits one in-flight operation. Available and used indices
advance with wrapping `u16` arithmetic, allowing the same queue to issue all
reads, writes, and flushes needed by a volume transaction.

The backend admits only three request types:

- `VIRTIO_BLK_T_IN` reads one in-range sector and requires 513 device-written
  bytes;
- `VIRTIO_BLK_T_OUT` writes one in-range sector and requires one
  device-written status byte; and
- `VIRTIO_BLK_T_FLUSH` carries no data descriptor and requires one
  device-written status byte.

Every completion must advance the used index exactly once, identify descriptor
zero, report the operation-specific length, preserve data-buffer guard bytes,
and return block status zero. A bounded poll also rejects device failure or a
needs-reset status.

## Reboot acceptance test

The host test creates a fresh zeroed raw image and performs two complete QEMU
boots:

1. The first boot confirms the volume is empty, commits generation one with the
   fixed payload `ginux-reboot-persistent\n`, flushes it, and recovers it before
   shutdown.
2. The second boot uses the same image, recovers generation one and the exact
   payload, and must not report the initialization path.

Both boots must reach `GINUX_KERNEL_OK`. The recovery boot fails if it emits a
panic, rewrites an empty volume, returns another generation, or changes the
payload. The existing in-memory fault model remains responsible for exhaustive
crash-stage, torn-marker, corruption, and generation-ambiguity cases; the raw
disk test proves restart persistence, not host power-cut injection.

## Teardown and limits

The backend clears PCI bus mastering, confirms it is disabled, and resets the
device before the DMA page may be scrubbed and released. A teardown failure is
fatal because recycling a page while DMA may continue would violate ownership.

- The path runs synchronously in kernel bootstrap code with interrupts
  disabled; it is not the intended isolated storage-service architecture.
- QEMU is trusted. There is no IOMMU domain constraining the device to the DMA
  page while bus mastering is enabled.
- The test relies on QEMU's raw block backend and virtio flush completion. It
  does not inject host crashes, torn physical sectors, dishonest caches, or
  media loss.
- There is one queue and one in-flight operation, with no cancellation,
  batching, interrupt completion, throughput measurement, or latency bound.
- Only the two-sector record is exposed; there is no filesystem, partition
  table, container mount, encryption, discard, or general Linux block device.
