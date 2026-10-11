# Isolated Service Supervision v0

**Status:** Request/reply completion followed by contained service failure.

The hardware proof assigns distinct roles to the two scheduled ring-3
contexts and native domains. Context 1 is the control client. It sends request
value 1 through the bounded IPC channel and yields. Context 2 is a minimal
isolated service in a different address space and a domain whose sole delegated
capability is IPC read/write. It receives the request, sends reply value 2, and
then executes the measured expected invalid opcode.

The CPL3 exception path removes only the service context, switches back to the
client's CR3, and restores the client's saved register frame. The client must
receive reply 2 and exit normally. Completion additionally requires an empty
IPC queue, one contained service fault, and no `GINUX_KERNEL_PANIC` marker.

This proves failure isolation after a completed operation, not transparent
service restart. There is no supervisor policy, service identity separate from
thread identity, request ID, replay protection, cancellation, outstanding-call
cleanup, restart budget, health protocol, or capability revocation on service
death. Those semantics must be defined before more than one request can be in
flight.
