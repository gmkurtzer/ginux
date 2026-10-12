# Virtio Network DMA Confinement v0

**Status:** Inactive confinement construction is verified on QEMU Q35.

## Purpose

This contract establishes the memory and VT-d boundary required before the
virtio-net function may receive queue, notification, interrupt, or bus-master
authority. It deliberately constructs and retires the boundary without
activating translation or touching notification MMIO.

## One-page bootstrap layout

The kernel allocates one zeroed, generation-tagged 4 KiB frame. The first
bootstrap queues will use eight descriptors even though the device advertises
256. The exact non-overlapping layout is:

| Range | Purpose | Required alignment |
|---|---|---:|
| `0x000..0x080` | RX descriptors | 16 bytes |
| `0x080..0x096` | RX available ring | 2 bytes |
| `0x100..0x146` | RX used ring | 4 bytes |
| `0x180..0x200` | TX descriptors | 16 bytes |
| `0x200..0x216` | TX available ring | 2 bytes |
| `0x280..0x2c6` | TX used ring | 4 bytes |
| `0x400..0xa00` | RX packet buffer | 2 bytes |
| `0xa00..0x1000` | TX packet buffer | 2 bytes |

Each 1536-byte packet buffer can contain the modern virtio-net header plus one
1514-byte Ethernet frame. Checked arithmetic rejects zero-sized, overlapping,
misaligned, and out-of-page layouts. Unused gaps remain part of the same owned
page but are not described to the device.

## VT-d hierarchy

Six fresh allocator leases encode one legacy root table, one context table,
and one four-level second-stage page-table path. The root/context entries name
only PCI source `00:03.0`; the final leaf identity-maps only the network DMA
page with read/write permission. Every other entry is zero.

The network DMA lease and all six table leases must be generation-distinct
from the preceding virtio-blk proof. Construction requires the block domain to
be completely retired, VT-d translation to be disabled with a zero root, and
all virtio-net PCI command authority to remain clear.

The construction phase never installs the root, enables VT-d translation,
enables bus mastering, configures a queue, or sets `DRIVER_OK`. The subsequent
[Virtio Network Empty Queue Activation v0](virtio-network-queue-activation-v0.md)
proof activates this exact boundary in a separately ordered phase.

## Deliberate limits

- MSI-X, packet movement, and fault injection are subsequent milestones.
- One page and eight-entry queues are a bootstrap profile, not a throughput
  target.
- The kernel owns this construction proof. The final path must recreate the
  same boundary for a measured, restartable native network service.
