# Native Network Egress Backpressure v0

**Status:** Exact object-9 producer blocking and wakeup are verified in
isolated ring-3 domains under QEMU/TCG.

Operation 29, `EGRESS_SPACE_WAIT`, takes no request buffer or secondary
argument and requires object 9 `WRITE` authority belonging to the currently
bound producer. It returns immediately when the FIFO has space, blocks without
polling when all four entries are occupied, and fails for read-only,
wrong-object, unbound, or exhausted producer state. Wakeup grants no slot: the
producer must retry operation 21 and receive the exact next sequence.

Destructive object-9 consumption wakes the sole object-9 space waiter after
the queue head is removed. Reliable operation 27 does the same only after the
claimed packet's virtio-net transmit has completed and the kernel accepts its
acknowledgement. A claim alone does not free space. Object-8 and object-9 space
wait keys are independent.

The QEMU proof fills the egress FIFO from a write-only producer, rejects a
fifth publication, and blocks that producer. A separate read-only consumer is
denied operation 29, removes the exact head, and wakes only the object-9
producer. The producer retries the fifth publication at the exact next
sequence. The consumer drains all remaining entries in FIFO order, after which
both domains, capabilities, wait state, frames, and producer binding are
retired.

A second proof couples the same boundary to the measured network service and
real virtio-net completion. The producer fills all four slots and blocks on its
fifth publication. The network service claims the head without freeing a slot,
transmits it through confined DMA, and blocks for kernel-owned MSI-X. Only the
kernel-validated operation-27 acknowledgement removes the claimed head and
wakes the producer. The producer then publishes the exact fifth sequence; a
replay claim for the acknowledged sequence is rejected as stale, and the
kernel drains and validates the remaining four replies in FIFO order. Both
domains and all device, interrupt, VT-d, DMA, capability, claim, wait, and
producer state retire cleanly.

## Limits

There is one producer and at most one space waiter per packet object. There is
also one global egress claim. Bootstrap v5 separately proves one network
service can transmit two packets without replacing its device generation.
Reservation, fairness among multiple producers, batching, an unbounded service
loop, and dynamic queue sizing are not defined.
