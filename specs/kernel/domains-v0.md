# Native Domain Metadata v0

**Status:** Sealed control-plane and delegated service domains executing at ring 3.

The kernel constructs one initial control-plane domain and one isolated service
domain in a four-slot bootstrap table retained for the kernel lifetime. A domain
identifier combines a fixed-table slot and generation so stale identifiers
cannot name a later occupant. The record has an explicit role and lifecycle
state and owns a domain-local capability table.

Initial control-plane authority is a finite list of boot grants. Each grant
becomes a root capability, after which the capability table's boot-authority
path is permanently sealed. The domain manager likewise refuses a second
control-plane construction. Later kernel operations query this same published
manager and sealed capability table; they cannot reconstruct authority from the
manifest. Handles are meaningful only in the owning domain's capability
namespace.

The current manifest grants the control plane host control, console, and one
bounded IPC channel. The manager validates that IPC root and delegates only its
read/write authority into a newly sealed service-domain table. The service gets
no host-control or console handle. Every call-gate authorization resolves the
opaque handle in the current scheduler domain, never in a global or
control-plane-default namespace. The boot gate rejects using the service handle
in the control domain and the control IPC handle in the service domain.
The service record also retains a provenance record for each delegated root:
the local root, parent domain generation, and source capability. Every service
authorization resolves local derivation to its root and revalidates that
root's bounded provenance chain, so revoking the control-plane source
invalidates the root and its derived children without relying on cooperative
service cleanup. Per-root records prevent a future multi-capability service
from validating one root through an unrelated source.

The domains own distinct W^X address spaces and scheduler contexts. When the
service's sole thread faults, the exception path first removes the scheduler
slot, then retires the service domain record and its entire capability table
before resuming the client. The boot gate requires the old domain ID and IPC
handle pair to stop resolving. The same slot can then host a newly delegated
service with an incremented domain generation. Even when its local capability
reuses the prior handle encoding, pairing it with the retired domain ID cannot
resolve authority. The boot proof runs the replacement through ABI negotiation
and clean exit, then retires it again. General restart policy and
outstanding-operation cleanup remain undefined. The fault path also publishes a
capability-gated service-fault event to a four-record FIFO before reusing the
service slot. The queue never overwrites an undelivered record and clears each
consumed slot. The event carries the retired generation-tagged domain ID,
not merely the reusable slot. The control proof compares it with the child ID
supplied at initial entry, so the replacement generation cannot be mistaken for
the faulted service.

Each domain also owns a fixed bootstrap call budget. Charging resolves the
current generation-tagged domain before dispatch, so threads cannot gain calls
by yielding or creating another thread. A restarted service receives the new
domain generation's independent budget. Exit is deliberately uncharged and
remains available after exhaustion.

The bootstrap domain metadata also enforces a one-thread limit. Scheduler
creation must first reserve a thread against the exact domain generation, and
normal exit or contained fault releases it. Service retirement is rejected
while any thread remains attached. The QEMU proof rejects a second control
thread and accounts for the control, original service, and replacement service
attachments independently.
