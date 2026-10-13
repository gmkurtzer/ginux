# Native Network Packet Backpressure v0

**Status:** A full packet FIFO blocks its write-only producer and a ring-3
consumer wakes it by freeing one slot under QEMU/TCG.

Operation 25 is the packet-space wait. It requires packet-object `WRITE`
authority, the currently bound producer generation, and zero in both `RSI` and
`RDX`. If the four-entry FIFO has space, it returns zero immediately. If the
FIFO is full and the packet sequence space is still usable, it registers one
typed packet-space wait and blocks the caller. A read-only caller, unbound or
replacement-mismatched producer, duplicate waiter, nonzero argument, or
exhausted sequence space is rejected without changing the queue.

A successful operation 22 first copies the exact FIFO head to the consumer and
then removes and scrubs that entry. Only after space exists does it wake the
blocked producer with result zero. Waking does not reserve a slot or publish a
packet; the producer must retry operation 21, which performs the normal bounded
copy and allocates the exact next sequence. This keeps backpressure separate
from data ownership and prevents a wake from silently changing packet state.

The executable proof fills the FIFO with four eight-byte packets. A fifth
publication returns busy, after which the write-only producer blocks through
operation 25. The read-only consumer proves it cannot claim that operation,
consumes the exact head, and wakes the producer. The producer retries the fifth
packet into the released slot and exits. The consumer drains the other four
packets in exact sequence order, and both domains retire with an empty queue.

## Limits

There is one bound producer and at most one packet-space waiter. Space waits do
not yet have their own deadline, low-water mark, batch reservation, or
multi-object readiness interface. Control-plane STOP can cancel the blocked
auxiliary domain. The single-CPU check-and-block and consume-and-wake
transitions require locking before SMP.
