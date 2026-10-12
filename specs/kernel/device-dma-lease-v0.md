# Isolated DMA Lease v0

**Status:** Domain ownership, ring-3 initialization, and fault cleanup verified in QEMU.

## Purpose

This contract introduces the memory side of an isolated virtio queue without
yet permitting device DMA. It proves that one candidate DMA page can belong to
the exact driver-domain generation, be writable from ring 3, be bound to inert
queue-address registers, and be reclaimed without retaining service data after
a fault.

## Mapping and ownership

The kernel allocates one zeroed 4 KiB frame to the stop-only block-control
domain in addition to its seven base address-space frames. A fixed leaf at
`USER_TEST_ADDRESS + 6 * 4096` maps it user-readable/writable and
non-executable. It is ordinary allocator-owned RAM, not MMIO.

Root validation requires the optional leaf to have exact RW/NX user flags, a
nonzero physical address below 4 GiB, and no alias with any base frame. Every
CR3 activation rechecks that both the seven base frames and this eighth frame
remain live under the scheduler's exact domain generation. A root naming an
unowned, stale, or cross-domain frame is rejected.

## Ring-3 and teardown proof

After negotiating features and inert queue geometry, the service writes a
fixed 64-bit marker into the candidate DMA page and reads it back. It writes the
lease base to the descriptor address, base plus `0x100` to the driver/available
address, and base plus `0x200` to the device/used address. The kernel reads back
all three values and requires exact equality with the returned lease. Queue
enable and PCI bus mastering remain zero, and `DRIVER_OK` is never set, so the
device cannot access the frame.

The service then deliberately faults. Stop-only teardown revokes object 7 and
invokes the device hook while the service root and lease are still intact. The
hook revalidates exact feature and queue-address state, disables PCI bus
mastering, resets the device, and records quiescence. Only then does teardown
restore the kernel root, zero all eight owned frames, release their exact
generation-tagged leases, and retire the domain. Registry release rejects a
bound device whose quiescence record is absent. The gate reads the former DMA
physical page through the supervisor direct map and requires it to be entirely
zero, while the allocator reports zero frames remaining for the retired owner.

## Deliberate limits

- This is a candidate DMA lease, not an active virtqueue.
- The physical address is embedded in the bounded bootstrap service and written
  only to the three queue-address registers; no general physical-address ABI is
  exposed.
- Bus mastering, queue enable, notifications, interrupts, and `DRIVER_OK`
  remain off.
- There is no IOMMU domain; actual DMA must not be enabled until confinement and
  teardown ordering are separately proved.
- The fixed single-page layout is a bootstrap proof, not a stable allocation
  or driver ABI.
