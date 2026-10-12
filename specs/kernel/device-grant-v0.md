# Block-device Control Grant v0

**Status:** Read-only observation and compiled write-only native volume control
are verified in QEMU.

## Authority

The signed development manifest grants the control plane object 7 with
`READ|WRITE` authority. The kernel attenuates that root into distinct fresh
stop-only domains. A bounded observer receives only `READ`; the measured native
volume service receives only `WRITE`. Exact generation-tagged handles
authorize mapping and control decisions. Process ancestry and possession of
another device or volume handle confer no block-device authority.

## Read-only observer

The observer receives one user-accessible, non-executable, uncached leaf for the
physical page containing the virtio status byte. It reads reset status zero and
then attempts a write, which must produce an x86 user write-protection fault.
Fault handling revokes its handle, removes the mapping, scrubs and releases its
owned frames, restores the kernel root, and retires the exact domain
generation. The kernel then requires the device to remain reset with bus
mastering disabled.

## Native write-only service

The native service's x86 MMIO leaves are necessarily readable as well as
writable because x86 has no write-only PTE. It receives only the common
configuration page, the page containing queue zero's validated notification
register, one owned DMA lease, and a read-only bootstrap record. PCI
configuration, MSI-X tables and pending bits, interrupt-controller registers,
the IDT, other devices, and port I/O remain unavailable.

The service negotiates exactly `VIRTIO_BLK_F_FLUSH` and
`VIRTIO_F_VERSION_1`, selects queue zero with size eight, installs its one-page
split-ring addresses, binds MSI-X entry zero, enables the queue, and sets
`DRIVER_OK`. It implements the shared two-slot persistent-volume engine over
serialized read, write, and flush requests to reserved sectors four and five.

Control ABI v4 operation 19 accepts only the service's object-7 handle and exact
next request generation. Command 1 is a retryable pre-notification wait;
command 2 declares possible device observation and therefore requires reset on
timeout. The kernel independently checks the active queue,
feature state, bus mastering, queue-to-lease binding, domain generation, and
request generation before blocking. Its private MSI-X vector wakes only that
saved service context. A calibrated APIC one-shot bounds a missing interrupt.

## Revocation and failure

The normal runtime exits through lifecycle ABI v0 after transaction success.
The stop-only path revokes object 7 and runs the registered device hook while
the service root and DMA lease still exist. That hook masks and disables MSI-X,
validates the binding, revokes bus mastering, resets the device, retires the
interrupt route, and validates stable completion state. A genuine fault or
notified timeout performs the same hardware isolation without trusting DMA
completion. Only then may the kernel restore its root, scrub and release all
owned frames, retire the domain, and publish lifecycle state. Address-space
release fails closed if device quiescence is absent.

## Deliberate limits

- Capability attenuation is write-only, but writable x86 MMIO mappings are also
  readable.
- Common and notification authority is page-granular; unrelated bytes in those
  pages are not yet sub-page protected.
- MSI-X routing remains entirely kernel-owned.
- One QEMU device, queue zero, one DMA page, and one outstanding request are
  supported. There is no IOMMU domain or stable general device-grant ABI.
