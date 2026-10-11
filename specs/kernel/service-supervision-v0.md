# Isolated Service Supervision v0

**Status:** Contained service failure followed by one bounded restart.

The hardware proof assigns distinct roles to the two scheduled ring-3
contexts and native domains. Context 1 is the control client. It sends request
value 1 through the bounded IPC channel and waits until a timer interrupt
preempts it. Context 2 is a minimal
isolated service in a different address space and a domain whose sole delegated
capability is IPC read/write. It receives the request, sends reply value 2, and
then attempts a measured write through address zero. The absent null page
raises a user-mode write page fault. Containment validates vector 14, error bits
for a user write to a non-present page, the authenticated faulting instruction,
and `CR2 == 0` before restarting the service.
The lifecycle record retains those four values alongside the faulted domain
generation. After the control plane consumes the packed event, a separate
manage-authorized operation copies a versioned 48-byte diagnostic record into
validated writable user memory. The ring-3 workload validates its header, and
the kernel gate checks the retained record against the authenticated
instruction address.
Another manage-authorized operation copies a 24-byte lifecycle descriptor with
the consumed event's kind, detail, exact domain generation, and monotonic
sequence. The ring-3 workload proves that console authority is insufficient and
validates the original service generation and sequence one.
The delivery cursor is lifecycle-generic: consuming a clean auxiliary
retirement replaces the prior event exposed by lifecycle queries. Fault-detail
queries use a distinct retained-fault cursor, so subsequent clean lifecycle
traffic cannot erase the most recently delivered fault diagnostic.

The CPL3 exception path removes only the service context, switches back to the
client's CR3, releases the service's domain thread reservation, retires the
faulted service domain and capability table, and
restores the client's saved register frame. The client must receive reply 2.
After restart it sends value 3 to the replacement, must receive acknowledgement
4, sends GET command 5, and must receive the retained value 3 before exiting
normally. Completion additionally requires an empty IPC queue, rejection
of the stale service domain/handle pair, one contained service fault, and no
`GINUX_KERNEL_PANIC` marker.

Before a normal exit or contained fault removes a domain's running context, it
wakes receivers already blocked on the opposite side of IPC object 3 with the
peer-closed result. Matching includes the endpoint direction, so teardown does
not release unrelated waits. This prevents the last surviving client from
remaining permanently blocked when its service disappears. Each service
retirement also leaves a generation-tagged sticky closure record for receivers
that were not blocked at teardown; replacement binding does not erase it.

Before retiring the faulted service, the kernel records one pending lifecycle
event naming that service generation. The control plane can consume event kind
1 only with manage authority on host-control object 1; presenting its console
capability is rejected without consuming the event. The QEMU payload requires
delivery before it exits, and the kernel verifies that the delivered event
belonged to the original faulted generation and carried lifecycle sequence one.
Every accepted event receives a nonzero monotonic internal sequence; dequeue
rejects zero or non-monotonic records. A full FIFO still consumes sequence
numbers for dropped records, while its later summary identifies the first
dropped sequence and the saturating drop count. Sequence exhaustion rejects a
new event without modifying the FIFO or overflow state. Event kind 3 records a
cleanly retired auxiliary service generation and carries no fault metadata.
Event kind occupies the low eight bits and the generation-tagged domain ID
occupies the next 48 bits.
Event kind 5 records a normal auxiliary exit. Its detail byte is the workload's
bounded exit status, while kind 3 management stops and kind 4 faults retain
zero detail. This lets supervision distinguish requested stop, normal exit,
and contained fault without consulting mutable domain state.
Event kind 7 records a supervised service's normal exit with zero detail. The
normal exit path publishes it before removing the domain record, while the
fault path continues to publish only kind 1. The control plane therefore does
not infer health from an IPC peer closure alone. If lifecycle sequencing is
exhausted, clean retirement is rejected and the domain record remains live;
the local invariant then restores sequence capacity and proves one exact event
is published before removal.

After the fault, the kernel creates one replacement domain generation with the
same logical service identity and IPC-only delegation, explicitly rebinds the service side of the IPC
endpoint to that generation, and queues a fresh context on the retained
service address space. Before creating that generation, the fault path verifies
that the failed service's CR3 is still active, clears every byte of the retained
user stack page with volatile writes, and reads the full page back as zero.
Replacement creation fails closed if this scrub cannot be completed. After the
new domain is created, the allocator atomically changes the owner of all seven
retained address-space frame leases from the failed generation to the
replacement. A wrong-generation release is rejected, and the boot proof must
reclaim the address space under the replacement owner. Requests left for the
failed generation are cleared;
its already-produced reply remains available to the control client. The kernel
rearms the APIC one-shot, returns to the client, and then
preempts that client to run the replacement. The replacement negotiates ABI
v1 and blocks on an empty request direction. The client wakes it directly with
request 3, yields, and requires acknowledgement 4. The replacement retains 3
as volatile state, blocks again, accepts fixed GET command 5, and returns 3
before rejecting unsupported command 6 with `UINT64_MAX - 2`. It then exits
cleanly and retires its capability table. This proves that
restart restores a stateful multi-request service, rather than merely creating
a context that can exit. When the replacement first blocks on its empty request
endpoint, the kernel publishes lifecycle kind 6 exactly once. The control plane
consumes that record, rejects the original generation, and retains the exact
replacement generation for validating reply provenance. Later blocking
receives by that same generation do not republish readiness and are not an
error. After the storage exchange, the control plane consumes kind 7 for the
exact replacement generation and validates lifecycle sequence three. The single
restart consumes the persistent control plane's fixed one-attempt bootstrap
budget. The initial service creation does not consume that budget, and failed
replacement setup does not refund it. A later service generation therefore
cannot create an unbounded restart loop. When no attempt remains, mandatory
fault teardown and lifecycle reporting still complete, and the exception path
resumes the surviving scheduler context without creating or binding another
service domain. If no context survives, it returns normally to the suspended
kernel continuation. Exhaustion is therefore a supervision outcome rather than
a kernel panic. The local manager invariant exercises rejection after the
budget is consumed; the current hardware workload exercises the successful
one-restart path. This is not yet a configurable
production policy. There is no backoff, durable state recovery,
request ID, replay protection, cancellation, outstanding-call cleanup, durable
storage, or health protocol. The four-record FIFO never overwrites queued events. Additional
records set a sticky overflow notification containing the first dropped domain
and sequence plus a saturating count, allowing teardown to continue. The control plane sees
that notification after the concrete FIFO records. There is no blocking wait,
subscription, recovery of the dropped record contents, or persistence.
Those semantics must be defined before more than one request can be in flight.

Replacement setup is a staged transaction across the domain manager, IPC
binding, frame-owner ledger, scheduler, and APIC rearm. Failure before scheduler
insertion quietly removes the new binding and domain without emitting a peer
closure for a service that never ran. Failure after insertion removes the exact
ready context and thread charge, restores all seven frames to the failed
generation, then removes the binding and domain. The consumed restart attempt
is not refunded. A rollback primitive that cannot prove its expected state
fails the exception path rather than masking inconsistent kernel metadata.
The failed generation remains named by the domain manager's retained-frame
quarantine between retirement and successful transfer. Adoption clears that
record only after the allocator proves zero frames remain under the failed
generation and exactly seven belong to the replacement. A normally exiting
replacement enters the same quarantine until final address-space release.
