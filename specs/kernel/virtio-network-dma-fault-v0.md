# Virtio Network DMA Fault v0

**Status:** One unauthorized receive write is blocked and recovered on QEMU Q35.

## Purpose

This contract proves that the virtio-net function cannot write outside its
single authorized DMA page and that one observed VT-d fault can be retired
without panicking the kernel or preventing a later authorized transaction.

## Fault construction

The kernel exclusively owns two fresh 4096-byte frames. The active legacy
VT-d hierarchy maps only the network queue frame for PCI source `00:03.0`.
Every byte of the second frame is filled with `0xa5`; this guard frame remains
owned and directly mapped by the kernel but is deliberately absent from the
device hierarchy.

Queue 0 publishes one 1535-byte device-writable descriptor whose address is
the aligned base of the guard frame. Queue 1 publishes the same bounded ARP
request used by the valid receive proof. Both queues, MSI-X routes, and bus
mastering follow the normal activation order, so the QEMU user-network backend
attempts to place its ARP reply in the unauthorized frame.

## Bounded transport outcome

VT-d may prevent the device from publishing the receive used entry as well as
the payload. The kernel therefore accepts exactly one of two outcomes:

- no RX interrupt and a zero receive used index, ID, and length; or
- exactly one RX interrupt and one receive used entry for descriptor zero with
  a length no larger than the published descriptor.

In either case the device must report neither failure nor reset-needed status,
the descriptor target must remain unchanged, the authorized RX buffer must
remain zero, and every byte of the guard frame must remain `0xa5`.

## Fault validation and recovery

Before inspecting or clearing VT-d fault state, the kernel function-masks and
disables MSI-X, revokes bus mastering, resets the device, disables PCI
decoding, and retires both temporary IDT routes. It then requires one primary
pending fault whose record contains:

- source ID `00:03.0`;
- the guard frame's aligned physical address;
- a write transaction; and
- the QEMU VT-d write-permission fault reason.

The kernel clears the valid record, requires fault status to become clear,
invalidates the translation caches, and verifies that the original root and
translation remain active. A fresh device activation must then complete the
normal bounded ARP request/reply exchange through the authorized DMA page.

Final teardown again quiesces the device and interrupt routes before disabling
translation and clearing its root. The kernel scrubs and releases the six
translation-table frames first, the unmapped guard frame second, and the mapped
DMA frame last. Retirement evidence must show that no table or DMA lease
remains live.

## Deliberate limits

- This is a deterministic QEMU Q35 fault proof, not physical-IOMMU validation.
- It exercises one page-aligned unauthorized write and one device source.
- It does not inject malformed completions, interrupt storms, or timeouts.
- The queue and recovery path remain kernel-owned; native-service fault and
  restart behavior is a later milestone.
