# Isolated Virtio Interrupt Wait v0

**Status:** Generation-bound MSI-X waits, prepared timeout retry, notified
timeout cancellation, restart recovery, and replay rejection are verified
through the compiled native volume service in QEMU.

## Boundary

The measured native volume service has write authority on manifest object 7.
It maps the virtio common configuration, the page containing queue zero's
notification register, and one generation-owned DMA page. It never maps PCI
configuration, the MSI-X table or pending-bit array, the IDT, or the local
APIC.

Control ABI v4 operation 19 accepts a nonzero request generation in `RDX`.
Command 1 declares a prepared request and command 2 a notified request. Each
call requires `WRITE` authority on object 7 and an exact match with
the registered device teardown record. Before blocking, the kernel revalidates
the caller domain generation, request generation, queue-zero MSI-X binding,
negotiated features, active queue, bus mastering, and all three queue addresses
against the caller's live DMA lease.

## Block and wake

The service publishes a request while delivery is masked, notifies queue zero,
and invokes command 2. The scheduler saves the complete ring-3 context and
marks that exact generation blocked before the kernel unmasks vector 33. The
interrupt handler matches the object, domain, and request generations, wakes
only that context, and acknowledges the local APIC. The call remasks the MSI-X
function and entry before returning to ring 3. Only a fully validated used
entry advances the expected request and clears the consumed interrupt state.

The first request additionally proves three failure cases. A future generation
is rejected without blocking. A command-1 wait issued before notification is
admitted, times out after the calibrated 100 ms APIC deadline, and returns
failure without consuming generation one. After notification, command 2
succeeds; an immediate replay of that consumed generation is rejected.

A separate generation has command-2 interrupt delivery withheld. Its APIC
deadline makes the already-notified request ambiguous, so the kernel masks and
disables MSI-X, revokes bus mastering, resets the device, retires the route,
scrubs DMA, and returns cancellation. The service exits `RESTART_REQUIRED` and
a fresh generation recovers persistent state. The kernel requires exact timeout
counter advances for both paths.

Fresh-volume initialization completes ten generations and recovery completes
two. Every successful device interrupt cancels its associated timeout.

## Teardown

On clean service exit, the registered teardown hook revalidates the active queue
and DMA binding, masks and disables MSI-X, revokes bus mastering, resets the
device, and requires vector 33 to be idle before removing the temporary IDT
gate. Only afterward may the kernel validate stable DMA state, restore its
root, scrub and release service memory, and retire the service authority.
Cancellation performs hardware teardown before returning to the service, so
the later `RESTART_REQUIRED` exit observes an already-quiesced lease.

## Deliberate limits

- Only one outstanding queue-zero request is modeled.
- There is no vector allocator, shared-vector demultiplexing, SMP affinity,
  IOMMU domain, concurrent request ring, or stable general block-service ABI.
- Notified cancellation resets the complete device; selective or in-place
  descriptor cancellation remains unsupported.
- Page-granular notification MMIO exposes unrelated bytes in its page.
