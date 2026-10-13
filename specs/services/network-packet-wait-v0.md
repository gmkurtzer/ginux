# Native Network Packet Wait v0

**Status:** An empty-queue ring-3 consumer blocks and is woken by an isolated
ring-3 producer under QEMU/TCG.

Operation 23 extends packet handoff with an exact-sequence availability wait.
It requires packet-object `READ` authority and the existing 32-byte packet
request v0 with a zero buffer address, zero buffer capacity, and a nonzero
expected sequence. If that sequence is already at the FIFO head, the operation
returns it immediately. If the FIFO is empty and the expected value is exactly
the next unallocated sequence, the scheduler blocks the calling thread. A
future, consumed, or otherwise mismatched sequence returns `STALE` without
blocking. An exhausted sequence space also returns `STALE` rather than creating
a wait that no producer can satisfy.

Only one blocked waiter may claim a sequence. Publication first completes its
bounded kernel copy and sequence allocation, then wakes the exact matching
waiter. The producer has packet `WRITE` without `READ`; the waiter has packet
`READ` without `WRITE`. Waking grants no additional authority and does not
consume the packet. The resumed consumer must issue operation 22 with the same
sequence, and normal copy-then-remove rules still apply.

The executable proof schedules both domains together. The producer yields, the
consumer rejects a future wait and blocks on sequence five, the producer
publishes one 42-byte Ethernet/ARP frame, and the scheduler wakes that consumer.
The consumer validates and removes the exact frame, proves a replayed wait is
stale, and exits. Both address spaces and capabilities are then retired.

## Limits

The wait is FIFO-head specific and has no timeout, cancellation operation,
fanout, readiness set, or multi-object poll interface. It remains valid across
producer exit or replacement because the queue and sequence space are
kernel-owned. A stopped blocked consumer is not yet asynchronously cancelled;
that requires the general blocked-auxiliary cancellation contract. SMP requires
locking around the current single-CPU check-and-block transition.
