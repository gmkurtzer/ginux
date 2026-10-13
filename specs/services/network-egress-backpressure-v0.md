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

## Limits

There is one producer and at most one space waiter per packet object. The proof
uses destructive consumers; reliable acknowledgement is covered by the
virtio-net egress proof. Reservation, fairness among multiple producers,
batching, and dynamic queue sizing are not defined.
