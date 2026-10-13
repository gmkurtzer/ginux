# Native Network Service Bootstrap ABI v4

**Status:** Additive blocking-egress extension verified through isolated
publication, scheduler wakeup, virtio-net transmit, and acknowledgement under
QEMU/TCG.

Version 4 preserves the complete 96-byte v3 layout. Its exact feature set is
the reliable-egress base plus `EGRESS_WAIT`; the v3 test-only `EGRESS_FAULT`
bit is not valid in v4. All address, capability, sequence, MAC, and
reserved-zero constraints remain unchanged.

Operation 28, `EGRESS_WAIT`, uses the existing 32-byte packet request with a
zero address and capacity and an exact nonzero sequence. It requires object 9
`READ` authority:

- the current head sequence returns immediately;
- the exact next sequence on an empty FIFO blocks without polling;
- any future, consumed, or otherwise mismatched sequence returns `STALE`;
- publication wakes only a waiter keyed by object 9 and that exact sequence.

Object 8 waiters and their APIC deadlines are independent. A write-only egress
producer and an unrelated device handle cannot wait. After wakeup the consumer
must separately claim, transmit, and acknowledge the packet through operations
26 and 27; wakeup grants no claim or reservation.

The QEMU proof starts the measured network service with an empty egress FIFO.
An isolated write-only producer yields so the service reaches operation 28,
then publishes the exact 42-byte ICMPv4 reply. The exact waiter wakes, claims
the sequence, completes an MSI-X-driven virtio-net transmit, and acknowledges
only after the kernel validates the descriptor, reply, used entry, and guards.
Both domains and all device, interrupt, VT-d, DMA, capability, and wait state
retire cleanly.

## Limits

The wait is untimed, covers one consumer and one fixed reply, and has no claim
deadline. Operation 29 defines producer backpressure separately. Multiple
egress waiters, batching, peer receipt, and continuously resident
protocol/network services remain future milestones.
