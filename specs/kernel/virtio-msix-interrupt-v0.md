# Virtio MSI-X Interrupt v0

**Status:** Kernel-owned M6 delivery profile.

## Purpose

This contract proves one real virtio-blk queue completion through MSI-X without
delegating interrupt-controller or MSI-X-table authority to userspace.

## Routing and delivery

The kernel reserves IDT vector 33 for the bounded proof. With interrupts
disabled, it:

1. installs a temporary ring-0 interrupt gate;
2. programs MSI-X table entry zero for fixed delivery to the discovered
   bootstrap local APIC while the entry and function remain masked;
3. enables MSI-X under the function mask, assigns virtio queue zero to table
   entry zero, and constructs the bounded sector-two read queue;
4. enables bus mastering, sets `DRIVER_OK`, unmasks the entry, clears the
   function mask, and notifies queue zero; and
5. enables CPU interrupts for a bounded wait on a kernel atomic set only by the
   device handler.

The handler preserves every general register, records one delivery, writes the
local-APIC EOI register, and returns through `iretq`. Completion is accepted
only when exactly one interrupt occurred and the used-ring ID, used length,
status byte, guard bytes, and blank sector payload all match the request.

## Teardown

With CPU interrupts disabled, teardown function-masks MSI-X, masks table entry
zero, disables MSI-X, revokes PCI bus mastering, and resets the virtio device.
The kernel then requires vector 33 to be absent from the local-APIC in-service,
trigger-mode, and interrupt-request banks before removing the temporary IDT
gate. DMA remains live until this sequence succeeds.

## Limitations

This is a kernel-owned bootstrap proof for one request. The persistent backend
and isolated ring-3 driver still poll their used rings. There is no persistent
vector allocator, blocked service wait, interrupt capability, shared-vector
demultiplexing, affinity policy, SMP routing, IOMMU confinement, or recovery
from a lost or malicious interrupt. MSI-X table pages remain kernel-only.
