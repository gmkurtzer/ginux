# Isolated Virtio Durability v0

**Status:** The compiled native volume service completes and recovers the
two-slot persistent-volume protocol through interrupt-driven virtio in QEMU.

## Contract

The measured `native/volume` static PIE links the shared `no_std` volume-format
and transaction crates. Its block adapter maps logical slots zero and one to
reserved device sectors four and five. Each operation reuses descriptor head
zero in one owned DMA page and submits exactly one serialized read, write, or
flush request through its generation-bound object-7 wait capability.

On empty media the service reads both slots, prepares generation one in the
inactive slot, flushes it, writes its commit marker last, flushes again, and
recovers the exact `ginux-isolated-volume\n` payload. This is ten completed
requests: two initial reads and two complete four-operation recovery scans
around the prepare and commit writes. On the second VM boot, two reads recover
the committed record without rewriting either sector. The automated gate
requires the fresh and recovered results on their respective boots.

Before notifying its first request, the runtime rejects future request
generation two, enters generation one, and requires the live 100 ms APIC
timeout. The timeout wakes the saved context with failure without consuming
the generation. The service then notifies queue zero, retries generation one
successfully, and rejects a duplicate wait for the consumed generation before
continuing the transaction.

## Stable teardown proof

The service deliberately executes an invalid opcode only after the shared
transaction engine reports success. Its registered fault hook first masks and
disables MSI-X, revokes bus mastering, resets the device, and retires vector
33. It then requires equal available and used indices of ten on a fresh boot or
two on recovery, a successful final sector-five read completion, and an exact
queue-to-lease binding. Only afterward may the kernel restore its root, scrub
and release the DMA and address-space frames, revoke the capability, and retire
the domain.

## Deliberate limits

- QEMU flush completion and two-boot recovery do not independently prove
  physical-media power-loss behavior on real hardware.
- Requests are serialized through one descriptor head and one DMA page. There
  is no batching, concurrent queue use, cancellation after notification, or
  recovery from an ambiguous late device completion.
- The development profile has no IOMMU domain; safe release therefore depends
  on synchronous device reset and bus-master revocation before frame reuse.
