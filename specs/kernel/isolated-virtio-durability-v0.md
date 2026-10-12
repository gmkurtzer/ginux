# Isolated Virtio Durability v0

**Status:** Interrupt-driven write, flush, and read-back verified in QEMU.

## Contract

After two generation-bound read completions, the isolated object-7 service
reuses its one owned DMA page and queue head for three more serialized requests:

1. generation three writes 512 bytes of `0x5a` to reserved sector three;
2. generation four submits negotiated virtio request type 4 and requires its
   status and used-ring completion; and
3. generation five fills the DMA buffer with `0xa5`, reads sector three, and
   requires all 512 bytes to be `0x5a`.

Every operation uses the same capability-checked blocking wait, kernel-owned
MSI-X vector, and 100 ms APIC timeout. MSI-X remains masked between requests.
The kernel advances only the exact consumed request generation and rejects
early, stale, duplicate, or concurrent waits.

## Stable teardown proof

The final fault hook revokes bus mastering and resets the device before reading
the DMA page. It requires available and used indices of five, used ID zero for
all entries, lengths 513/513/1/1/513, a successful final status, read descriptor
flags, sector three in the request header, and the complete `0x5a` payload.
Only then may the page be scrubbed and released.

## Deliberate limits

- This proves that the isolated service can issue the ordering primitive needed
  by the persistent-volume protocol. It does not yet move the two-slot record,
  checksum, prepare/commit markers, or recovery scan out of the kernel.
- The same-boot read-back and QEMU flush completion do not independently prove
  physical-media power-loss behavior. The existing two-boot volume gate proves
  persistence for the kernel backend; an isolated two-boot gate remains next.
- Requests are serialized through one descriptor head and one DMA page. There
  is no cancellation, batching, concurrent queue use, or IOMMU domain yet.
