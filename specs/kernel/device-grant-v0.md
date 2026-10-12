# Block-device Control Grant v0

**Status:** Read-only observation and write-only control boundaries verified in QEMU.

## Purpose

This contract is the first step from a kernel-owned virtio bootstrap path to an
isolated storage-driver domain. It separately delegates bounded observation and
control authority to ring-3 services and exposes one real MMIO page. It
deliberately does not enable a queue or device DMA yet.

## Authority and mapping

The signed development manifest grants the control plane object 7 with
`READ|WRITE` authority. The kernel attenuates that root into distinct fresh
stop-only auxiliary domains: the observer receives only `READ`, while the
control service receives only `WRITE`. Only an exact generation-tagged handle
for object 7 authorizes either mapping decision; process ancestry and
possession of another device or volume handle do not.

The service receives one fixed leaf at `USER_TEST_ADDRESS + 5 * 4096`. It maps
the physical page containing the modern virtio common-configuration status
byte. The observer's leaf is read-only; the control service's leaf is writable.
Both are user-accessible, non-executable, write-through, and cache-disabled.
The page must be above the 4 GiB allocator ceiling, so it cannot alias any
allocator-owned page-table, code, or stack frame. No other unmodeled user leaf
is admitted. Root validation accepts the hardware Accessed bit and accepts the
Dirty bit only when the device leaf was explicitly writable.

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

The write-only capability holder starts from that reset state. Its x86 mapping
is necessarily read/write because the architecture has no write-only PTE. The
service sets `ACKNOWLEDGE|DRIVER`, reads both device-feature words, and requires
`VIRTIO_BLK_F_FLUSH` plus `VIRTIO_F_VERSION_1`. It publishes exactly those two
driver features, sets `FEATURES_OK`, and requires the device to retain that
bit. The kernel independently reads back the exact selected features and status
after the service selects queue zero, requires capacity for at least eight
descriptors, and writes the bounded size eight. The service initializes its
owned DMA page and writes the exact descriptor, available, and used
offsets into the three queue-address fields. After the kernel has armed PCI bus
mastering, the service enables queue zero, sets `DRIVER_OK`, confirms the bit,
and takes a deliberate invalid-opcode fault without publishing an available
entry or sending a notification. The fault hook independently reads back the
exact active queue and address-to-lease binding.

The stop-only path first revokes the exact object-7 handle. Its registered
device hook validates the expected features and queue-to-lease binding, disables
PCI bus mastering, resets device status to zero, and confirms both conditions.
Only then may the fault path restore the kernel root, destroy the service
mapping, clear and release all owned frames, retire the domain, and publish the
fault. Address-space release fails closed if a bound device has not completed
quiescence. The service owns one DMA frame and writes its exact ring offsets to
the device. The queue becomes DMA-capable, but the service submits no request
and performs no notification. The lease lifecycle is specified in
[Isolated DMA Lease v0](device-dma-lease-v0.md).

## Deliberate limits

- These are bounded observation/control proofs, not a block driver or storage
  service.
- Page-granular mapping exposes the complete 4 KiB common-configuration page;
  capability attenuation is write-only, but the writable x86 leaf is also
  readable. The proof touches only feature-selection, feature-value, and device
  status fields. Notification registers, ISR, device configuration, PCI
  configuration, and port I/O are unavailable.
- No request submission, completion, interrupt, or IOMMU domain is delegated.
- The kernel still negotiates virtio and performs all persistent-volume I/O.
- The mapping is a fixed bootstrap layout for one QEMU device, not a stable
  general device-grant ABI.
