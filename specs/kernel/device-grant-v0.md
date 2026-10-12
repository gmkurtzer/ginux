# Read-only Block-device Grant v0

**Status:** Hardware-observation and revocation boundary verified in QEMU.

## Purpose

This contract is the first step from a kernel-owned virtio bootstrap path to an
isolated storage-driver domain. It delegates bounded authority to a ring-3
observer and exposes one real MMIO page without allowing the observer to change
device state. It deliberately does not delegate queues or DMA yet.

## Authority and mapping

The signed development manifest grants the control plane object 7 with `READ`
authority. The kernel attenuates that root into a fresh stop-only auxiliary
domain. Only an exact generation-tagged handle for object 7 authorizes the
mapping decision; process ancestry and possession of another device or volume
handle do not.

The service receives one fixed leaf at `USER_TEST_ADDRESS + 5 * 4096`. It maps
the physical page containing the modern virtio common-configuration status
byte. The leaf is user-accessible, read-only, non-executable, write-through,
and cache-disabled. The page must be above the 4 GiB allocator ceiling, so it
cannot alias any allocator-owned page-table, code, or stack frame. No other
unmodeled user leaf is admitted.

## Hardware proof and revocation

The kernel first completes all block I/O, clears PCI bus mastering, and resets
the device. The observer then reads status zero directly from the mapped MMIO
byte. Its next instruction attempts to write one to that byte and must take an
x86 page fault with the present, write, and user bits set in the error code.
The denied instruction therefore cannot reach the device.

The native fault path retires the exact auxiliary-domain generation, publishes
its fault record, revokes the object-7 handle, clears and releases all seven
owned address-space frames, and restores the kernel root. Destroying the page
table also removes the external MMIO mapping; the MMIO page itself is never
treated as allocator-owned memory. After retirement the kernel rechecks that
the virtio device is reset and PCI bus mastering is disabled.

## Deliberate limits

- This is a read-only observer, not a block driver or storage service.
- Common configuration outside the mapped page, notification registers, ISR,
  device configuration, PCI configuration, and port I/O are unavailable.
- No writable MMIO, DMA frame, interrupt, queue, or IOMMU domain is delegated.
- The kernel still negotiates virtio and performs all persistent-volume I/O.
- The mapping is a fixed bootstrap layout for one QEMU device, not a stable
  general device-grant ABI.
