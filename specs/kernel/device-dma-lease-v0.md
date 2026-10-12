# Isolated DMA Lease v0

**Status:** Isolated interrupt-driven read completion and fault cleanup verified in QEMU.

## Purpose

This contract introduces the memory side of an isolated virtio queue without
yet submitting device I/O. It proves that one DMA page can belong to
the exact driver-domain generation, be writable from ring 3, be bound to an
active queue, and be reclaimed without retaining service data after a fault.

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

After negotiating features and queue geometry, the service writes a fixed
64-bit marker into the DMA page and reads it back. It writes the
lease base to the descriptor address, base plus `0x100` to the driver/available
address, and base plus `0x200` to the device/used address. The kernel reads back
all three values and requires exact equality with the returned lease. The
service replaces its initialization marker with an exact sector-two read
layout: three chained descriptors reference a 16-byte request header, a
512-byte device-write buffer, and a one-byte pending status field. The kernel
arms PCI bus mastering, then the service enables queue zero and sets
`DRIVER_OK`. It publishes descriptor head zero by advancing the available index
to one and writes queue index zero to the separately mapped notification
register. It invokes the capability-checked block-device wait, becomes
scheduler-blocked, and resumes only after the kernel-owned MSI-X vector matches
its domain and request generation. It requires used index one, used ID zero,
length 513, successful status, and 512 zero bytes from reserved sector two.
The kernel advances the expected request generation, and the service reuses
the chain for a second notification and blocked completion, leaving used index
two and two exact length-513 records. The fault hook first masks and disables MSI-X, validates the active
binding, disables PCI bus mastering, and resets the device. It then checks every byte in the now-stable page against
that exact layout. Any changed address, length, flag, chain index, request
field, ring index, buffer byte, or status byte prevents teardown from accepting
the completion.

The service then deliberately faults. Stop-only teardown revokes object 7 and
invokes the device hook while the service root and lease are still intact. The
hook revalidates exact feature and queue-address state, disables PCI bus
mastering, resets the device, validates the stable DMA contents, and records
quiescence. Only then does teardown
restore the kernel root, zero all eight owned frames, release their exact
generation-tagged leases, and retire the domain. Registry release rejects a
bound device whose quiescence record is absent. The gate reads the former DMA
physical page through the supervisor direct map and requires it to be entirely
zero, while the allocator reports zero frames remaining for the retired owner.

## Deliberate limits

- This is a bounded two-generation, interrupt-driven, read-only reuse proof,
  not a general block-I/O service.
- The physical address is embedded in the bounded bootstrap service and written
  only to the three queue-address registers; no general physical-address ABI is
  exposed.
- The notification register's containing page is mapped and written once.
  Page-granular authority currently exposes unrelated bytes in that page.
- Interrupt routing remains kernel-owned; the service receives only a
  generation-bound wait operation, not interrupt-controller authority.
- A calibrated 100 ms kernel timer bounds a missing interrupt and wakes the
  saved request with failure before teardown.
- There is no IOMMU domain; the trusted-QEMU bootstrap permits bus mastering
  only because the sole queue points at one bounded owned page and synchronous
  fault teardown revokes hardware access before releasing it.
- The fixed single-page layout is a bootstrap proof, not a stable allocation
  or driver ABI.
