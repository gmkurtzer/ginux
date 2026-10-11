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

The CPL3 exception path removes only the service context, switches back to the
client's CR3, releases the service's domain thread reservation, retires the
faulted service domain and capability table, and
restores the client's saved register frame. The client must receive reply 2 and
exit normally. Completion additionally requires an empty IPC queue, rejection
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
belonged to the original faulted generation. Event kind occupies the low eight
bits and the generation-tagged domain ID occupies the next 48 bits.

After the fault, the kernel creates one replacement domain generation with the
same IPC-only delegation, explicitly rebinds the service side of the IPC
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
v1 and blocks on an empty request direction. The client wakes it with a direct
message, exhausts its call budget, and exits. The replacement then observes the
message, exits cleanly, and has its capability table retired. The single
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
production policy. There is no backoff, state recovery, service identity separate from thread identity,
request ID, replay protection, cancellation, outstanding-call cleanup, or
health protocol. The four-record FIFO never overwrites queued events. Additional
records set a sticky overflow notification containing the first dropped domain
and a saturating count, allowing teardown to continue. The control plane sees
that notification after the concrete FIFO records. There is no blocking wait,
subscription, recovery of the dropped record contents, or persistence.
Those semantics must be defined before more than one request can be in flight.
