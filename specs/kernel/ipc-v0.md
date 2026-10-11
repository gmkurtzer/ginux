# Bounded IPC Channel v0

**Status:** Single capability-gated channel verified from ring 3.

The bootstrap kernel owns one nonblocking FIFO channel for control-plane
bring-up. It holds at most four `u64` messages without allocation. Send fails
when full, receive fails when empty, and successful receive clears the consumed
slot. The local invariant test covers empty receive, capacity enforcement,
FIFO order, and wraparound.

The channel is object 3 in the signed boot authority manifest. Sending requires
`WRITE`; receiving requires `READ`. The call gate resolves the supplied opaque
handle through the sealed control-plane capability table before touching the
queue. The ring-3 proof requires a send with the console capability to fail,
then has context 1 enqueue request value 1 and yield. Context 2 acts as an
isolated service: it receives 1, sends transformed reply 2, and deliberately
faults. The resumed client requires reply 2. The boot gate also requires the
channel to be empty at completion.

This revision transfers only an immediate word and has no blocking wait,
endpoint ownership, capability transfer, cancellation, timeout, peer-close
notification, or cleanup after domain failure. The global queue is safe only
for the current single-CPU, interrupts-disabled execution model.
