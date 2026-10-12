# Virtio Block Read v0

**Status:** Bounded M6 QEMU read profile.

## Purpose

This contract performs the first real block-device transaction: one read of
reserved sector two from the deterministic 1 MiB QEMU disk. Sectors zero and
one are reserved for the persistent-volume record. The probe proves queue
mechanics and DMA lease lifecycle without modifying its target sector.

## MMIO and device geometry

The sealed supervisor MMIO window covers the smallest contiguous physical span
containing the validated common, notification, ISR, and device configuration
regions. The public translation object admits only that exact requested span,
even though page tables necessarily map enclosing 4 KiB pages.

The device configuration generation is sampled before and after the 64-bit
capacity read. A changing generation is retried at most eight times. The QEMU
profile requires exactly 2,048 512-byte sectors.

## DMA lease and split ring

The physical-frame allocator provides one zeroed, generation-tagged,
kernel-owned 4 KiB lease. The page contains:

- eight 16-byte descriptor slots at offset `0x000`;
- the available ring at `0x100`;
- the used ring at `0x200`;
- a 16-byte read request for sector two at `0x400`;
- a 512-byte device-writable data buffer at `0x600`; and
- a one-byte device-writable status at `0x900`.

Only three descriptors are published: request header, data, and status. Queue
zero must expose at least three entries and is bounded to at most eight. Queue
addresses contain the leased physical page, not its higher-half alias.
Notification offset multiplication and region bounds are checked before the
notification write. Copied contract tests reject undersized queue geometry,
notification overflow, and out-of-range notification addresses.

The sector buffer begins with a nonzero pattern. Distinct guard bytes
immediately before and after it must remain unchanged, while the deterministic
blank sector must become all zero. The used ring must return descriptor ID zero
and exactly 513 device-written bytes, and the block status must report success.

## Teardown and failure behavior

After completion—or after any failure once negotiation begins—the kernel:

1. clears PCI bus mastering and reads the command register back;
2. resets the device and waits for status zero; and
3. only after successful teardown scrubs and releases the exact DMA lease.

If bus-master revocation or reset cannot be proven, the kernel fails closed and
does not recycle the page.

## Security limitations

The Q35 reference profile now binds the exact virtio source ID to a 48-bit VT-d
domain containing only the single live queue/data page. Translation is enabled
before bus mastering and disabled only after reset and bus-master revocation;
fault status must remain clear. Deliberate unauthorized-DMA rejection and real
hardware remain unproven, and the isolated userspace driver path does not yet
receive this domain.

The probe polls synchronously with interrupts disabled and permits one in-flight
request. The reusable queue added on top of this layout supports bounded reads,
writes, and flushes for the persistent-volume proof, but still has no interrupt
completion, cancellation, userspace block-service API, or timeout recovery
beyond device reset. See [Virtio-backed Persistent Volume v0](virtio-volume-v0.md).
