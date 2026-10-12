# PCI Virtio Block Discovery v0

**Status:** Bounded M6 discovery profile.

## Purpose

This contract introduces the first physical-device boundary beneath the
persistent-volume service. It discovers and validates one modern virtio block
function without activating the device or granting it DMA authority.

## Machine profile

- QEMU `q35`, x86-64, PCI configuration mechanism 1.
- Bus 0, function 0 of each of the 32 possible device positions.
- Exactly one PCI vendor `0x1af4`, device `0x1042` function.
- A deterministic 1 MiB raw image attached with legacy virtio disabled.
- Configuration access is owned by the bootstrap processor while interrupts
  remain disabled and before service domains begin executing.

This is intentionally not general PCI enumeration. Bridges, secondary buses,
multifunction devices, PCI Express ECAM, hotplug, and multiple matching block
devices remain outside this profile.

## Validation

The kernel copies the function's 256-byte configuration space using aligned
32-bit reads and validates it without mutating device state. The parser:

- verifies the exact vendor/device identity and type-zero header;
- requires the PCI capability-list status bit;
- accepts only aligned capability offsets from `0x40` through `0xfc`;
- visits at most 48 entries and rejects cycles;
- bounds every vendor capability inside the snapshot;
- requires exactly one common, notification, ISR, and device configuration
  region;
- rejects I/O, reserved, zero, or invalid BAR addresses;
- rejects zero-length or overflowing regions; and
- requires a nonzero notification multiplier.

Every QEMU boot also runs copied-snapshot negative cases for the wrong device
identity, absent capability list, unaligned head, cyclic list, invalid
capability length, and invalid BAR selector. These tests perform no PCI writes.

## Authority and limitations

Discovery creates no userspace capability and performs no MMIO access. The
kernel does not enable memory space, bus mastering, interrupts, queues, or
device features. It does not read or write a disk sector and makes no reboot
persistence claim.

The separate
[Virtio Block Initialization v0](virtio-block-init-v0.md) contract maps the
validated common region, resets the device, and negotiates a minimal feature
set while bus mastering remains disabled. DMA authority and virtqueue
activation still require a separate milestone and failure-containment contract.
