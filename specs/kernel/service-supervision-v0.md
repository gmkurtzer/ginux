# Isolated Service Supervision v0

**Status:** Contained service failure followed by one bounded restart.

The hardware proof assigns distinct roles to the two scheduled ring-3
contexts and native domains. Context 1 is the control client. It sends request
value 1 through the bounded IPC channel and waits until a timer interrupt
preempts it. Context 2 is a minimal
isolated service in a different address space and a domain whose sole delegated
capability is IPC read/write. It receives the request, sends reply value 2, and
then executes the measured expected invalid opcode.

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
remaining permanently blocked when its service disappears.

Before retiring the faulted service, the kernel records one pending lifecycle
event naming that service generation. The control plane can consume event kind
1 only with manage authority on host-control object 1; presenting its console
capability is rejected without consuming the event. The QEMU payload requires
delivery before it exits, and the kernel verifies that the delivered event
belonged to the original faulted generation. Event kind occupies the low eight
bits and the generation-tagged domain ID occupies the next 48 bits.

After the fault, the kernel creates one replacement domain generation with the
same IPC-only delegation and queues a fresh context on the retained service
address space. It rearms the APIC one-shot, returns to the client, and then
preempts that client to run the replacement. The replacement negotiates ABI
v1 and blocks on an empty request direction. The client wakes it with a direct
message, exhausts its call budget, and exits. The replacement then observes the
message, exits cleanly, and has its capability table retired. The single
restart is a fixed
proof action, not a production policy. There is no backoff, configurable
restart budget, state recovery, service identity separate from thread identity,
request ID, replay protection, cancellation, outstanding-call cleanup, or
health protocol. The four-record FIFO has no blocking wait, event classes beyond
the contained-fault record, subscription, overflow recovery, or persistence.
Those semantics must be defined before more than one request can be in flight.
