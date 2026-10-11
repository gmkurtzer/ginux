# Bounded IPC Channel v0

**Status:** Single capability-gated duplex channel verified from ring 3.

The bootstrap kernel owns one nonblocking duplex channel for control-plane
bring-up. Its request and reply directions each hold at most four `u64`
messages without allocation. Send fails when its outbound direction is full,
receive fails when its inbound direction is empty, and successful receive
clears the consumed slot. The local invariant test covers empty receive,
per-direction capacity enforcement, FIFO order, wraparound, and rejection of
receiving an endpoint's own outbound messages.

The channel is object 3 in the signed boot authority manifest. Sending requires
`WRITE`; receiving requires `READ`. The call gate resolves the supplied opaque
handle through the sealed control-plane capability table before touching the
queue. The ring-3 proof requires a send with the console capability to fail,
then has the control context enqueue request value 1 in the service-inbound
direction and wait for timer preemption. The service receives 1, sends
transformed reply 2 in the control-inbound direction, and deliberately faults.
The resumed client requires reply 2. Direction is selected from the current
generation-checked domain role after capability validation; callers cannot
choose it as an argument. The boot gate also requires both queues to be empty
at completion.

Operation 8 adds one blocking receive path. If its inbound queue is empty, the
current context becomes blocked only when another context can run. A peer send
then bypasses the queue, writes the word into the blocked context's result
register, and makes it ready. The QEMU proof preempts into the replacement
service, blocks it, resumes control, wakes the service with value 3, and later
observes 3 as the receive result. There is at most one bootstrap waiter, and a
failed block rolls back atomically.

This revision transfers only an immediate word and has no capability transfer,
cancellation, timeout, peer-close notification, or cleanup of queued words
after domain failure. It has one fixed control/service endpoint pair rather
than allocatable endpoint objects. The global queue is safe only
for the current single-CPU model. The timer handler does not access IPC, and
call-gate entries execute with interrupts disabled; locking is still required
before SMP or kernel preemption.
