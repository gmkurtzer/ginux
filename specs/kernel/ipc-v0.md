# Bounded IPC Channel v0

**Status:** Single capability-gated duplex channel verified from ring 3.

The bootstrap kernel owns one nonblocking duplex channel for control-plane
bring-up. Its request and reply directions each hold at most four `u64`
messages without allocation. Send fails when its outbound direction is full,
receive fails when its inbound direction is empty, and successful receive
clears the consumed slot. The local invariant test covers empty receive,
per-direction capacity enforcement, FIFO order, wraparound, and rejection of
receiving an endpoint's own outbound messages.

The duplex endpoint stores the exact generation-tagged control and service
domain IDs. Queue operations resolve the caller against those bindings rather
than trusting a caller-selected direction. When a service generation retires,
the kernel clears requests that were destined for it, rejects the stale domain,
and leaves already-produced replies available to the control plane. A
replacement generation receives nothing until the kernel explicitly binds its
new ID. The local invariant covers stale access and empty replacement input;
the ring-3 path preserves the original service's reply across retirement while
requiring the replacement to block until a new control request arrives.
Direct delivery to a blocked receiver and creation of a blocking wait use the
same endpoint lookup before touching scheduler state; neither path can bypass
the generation binding merely because no queue operation is needed.

Every queued word also carries the exact sender domain generation internally.
Service-side dequeue requires the recorded source to equal the control-plane
generation. Control-side dequeue rejects a zero or control-plane source and
retains the service generation for supervision auditing. The hardware proof
requires the reply preserved across restart to remain attributed to the
original failed service, not the replacement, and the consumed request to
remain attributed to the control plane. Closure records use the same envelope
and require their source and retired-generation value to agree.

Queued request and reply envelopes also carry independent, monotonically
increasing sequence numbers. Sequence zero is invalid, a failed enqueue does
not consume a number, and service retirement or replacement rebinding does not
reset either direction. Dequeue rejects a number that is not greater than the
last accepted number for that direction. Retirement may deliberately discard
requests for the old service, so gaps are valid and contiguity is not required.
Exhausting either message sequence rejects both direct and queued sends without
waking a waiter, changing audit state, or inserting a record.
The local invariant injects zero and out-of-order envelopes, verifies that a
full-queue send does not advance the counter, and proves that sequencing
continues across service replacement. The QEMU proof audits sequence one in
both directions for the first exchange and request sequence two for the later
direct delivery to the replacement.

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
register, and makes it ready only if object 3 and the receiver-side direction
match the typed scheduler wait key. The wait key also names the exact expected
peer generation obtained from the bound endpoint; object and direction matches
from a stale or unrelated generation cannot wake it. The QEMU proof preempts into the replacement
service, blocks it, resumes control, wakes the service with value 3, and later
observes 3 as the receive result. The scheduler retains the exact waking domain
generation and message sequence in its audit state. The channel commits that
sequence only after a matching waiter is woken; when no waiter matches, the
same number is used by the queued fallback. Direct delivery is attempted only
when that direction's queue is empty, preserving FIFO order even if scheduler
state is inconsistent. Completion requires the source to
match the control plane and the sequence to follow the earlier queued request.
There is at most one bootstrap waiter, and a
failed block rolls back atomically.

Domain exit and contained-fault paths cancel matching blocked receives before
removing the dying context. The saved receive returns the distinct
`UINT64_MAX - 4` peer-closed result and becomes ready, allowing it to be
selected immediately even when it was the last surviving context. The
scheduler invariant proves that object/direction mismatches stay blocked and
that an unrelated ready context remains unchanged.

Service retirement also appends the exact retired domain generation and a
nonzero, monotonically increasing closure sequence to a bounded four-record
closure FIFO. Control-side receive rejects zero or non-monotonic closure
sequences and drains already-produced
replies first, then returns the peer-closed outcome once per queued generation.
Binding a replacement does not erase an older closure record, so a late
receiver cannot confuse restart with an empty live channel. The QEMU gate
observes the original and replacement generations in order after both have
retired, audits closure sequence two for the replacement, then requires the
closure FIFO to be empty. Retirement fails closed if the FIFO is full or the
sequence space is exhausted rather than discarding an unobserved closure or
reusing a number. Closure-sequence exhaustion leaves the service bound and its
pending requests intact.
The restart transaction has a separate exact-generation rollback operation for
a replacement that was bound but never scheduled. It requires an empty request
direction, removes only that binding, and deliberately emits no peer-closure
record for a service that never became observable.

The scalar v1 receive result exposes only the immediate word. A separate
IPC-read-authorized operation copies a versioned descriptor for the calling
endpoint's most recent accepted delivery without consuming another record. It
distinguishes messages from closures and exposes the source generation,
sequence, and value. The bootstrap proof validates the original service's
queued reply; descriptor state is endpoint-local but remains a single latest
record rather than a history. This revision has no capability transfer,
cancellation request, timeout, or cleanup of queued replies after domain
failure. It has one fixed control/service endpoint pair rather
than allocatable endpoint objects. The global queue is safe only
for the current single-CPU model. The timer handler does not access IPC, and
call-gate entries execute with interrupts disabled; locking is still required
before SMP or kernel preemption.
