# Isolated Virtio Interrupt Wait v0

**Status:** Five sequential generation-bound MSI-X waits verified in QEMU.

## Boundary

The stop-only block service retains write authority on manifest object 7. It
maps the virtio common configuration, one queue-notification page, and one
generation-owned DMA page. It never maps PCI configuration, the MSI-X table or
pending-bit array, the IDT, or the local APIC.

Control ABI v4 operation 19, command 1 accepts a nonzero request generation in
`RDX`. The call requires `WRITE` authority on object 7 and an exact match with
the kernel's registered device teardown record. Before blocking, the kernel
revalidates the caller domain generation, request generation, queue-zero
MSI-X binding, negotiated features, active queue, bus mastering, and all three
queue addresses against the caller's live DMA lease.

## Block and wake

The service first publishes and notifies its request while MSI-X table entry
zero and the function are masked. It then enters operation 19. The scheduler
saves the complete ring-3 context, marks that exact generation `Blocked`, and
leaves no current userspace thread. Only after the state transition does the
kernel unmask the owned vector and enter an interrupt-enabled `HLT` wait.

Vector 33 records the interrupt, matches the saved object, domain generation,
and request generation, writes success into saved `RAX`, makes the same thread
running, acknowledges the local APIC, and returns to the sleeping kernel call.
The call remasks both the function and table entry before returning to ring 3.
The service validates used ID zero, length 513, status zero, guard bytes, and
the blank sector-two payload. The kernel atomically advances the expected
request from generation one to two and clears the consumed interrupt count and
completion bit while delivery remains masked. The service reuses descriptor
head zero, publishes available index two, notifies again, and completes a
second block/wake cycle. It then submits write, flush, and read-back requests
through generations three, four, and five. The proof rejects request generation
two before the first request and rejects a duplicate wait for consumed
generation one.

Every accepted wait also arms the previously calibrated local-APIC timer for a
100 ms one-shot. The device interrupt cancels and masks that timer before the
call returns. If the timer wins, its handler wakes the same saved wait with an
error; the call masks MSI-X before returning the failure. The boot gate
independently delivers the timeout interrupt and proves its state is consumed,
then proves the real MSI-X path cancels the pending timeout.

## Teardown

The service deliberately faults after validation. Its registered teardown hook
revalidates the active queue and DMA binding, function-masks and entry-masks
MSI-X, disables MSI-X, revokes bus mastering, resets the device, and requires
vector 33 to be idle before removing the temporary IDT gate. Only afterward may
the kernel validate stable DMA contents, restore the kernel root, scrub and
release the DMA page, destroy the service mappings, and retire its authority.

## Deliberate limits

- Five sequential request generations and one outstanding queue-zero request
  are modeled, including read, write, and flush descriptor shapes.
- There is no vector allocator, shared-vector demultiplexing, SMP affinity,
  IOMMU domain, concurrent request ring, or stable general block-service ABI yet.
- The gate proves timeout delivery independently and cancellation on the live
  device-success path; injected live-device timeout and recovery remains a
  future fault-injection test.
- Page-granular notification MMIO still exposes unrelated bytes in its page.
