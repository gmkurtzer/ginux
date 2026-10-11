# Native Domain Metadata v0

**Status:** Sealed control-plane and delegated service domains executing at ring 3.

The kernel constructs one initial control-plane domain and one isolated service
domain in a four-slot bootstrap table retained for the kernel lifetime. The
manager now also has two bounded auxiliary-service registry slots, allowing the
remaining domain records to be registered without replacing the primary
restart-managed service. Auxiliary construction validates every delegation
before publishing a domain, and registry saturation fails without allocating a
record. A domain
identifier combines a fixed-table slot and generation so stale identifiers
cannot name a later occupant. The record has an explicit role and lifecycle
state, a generation-tagged lifecycle parent, and a domain-local capability
table. The control plane has no parent; each bootstrap service is its direct
child. Ancestry walks are bounded by the fixed domain table and reject missing,
stale, and self relationships; malformed chains cannot loop indefinitely.
Domain and logical-service generations never wrap. Allocation scans the bounded
tables for another empty slot whose next generation is representable; an
exhausted slot is permanently skipped. If no usable slot remains, construction
fails before publishing a domain, registry entry, or logical identity.

Service identity is separate from domain identity. The primary logical service
keeps one generation-tagged service ID across its initial and replacement
domain generations. Auxiliary registry slots have independent service-ID
generations, so retiring an auxiliary service and registering unrelated work in
the same slot creates a new logical identity. Lifecycle records retain both the
logical service ID and the exact domain instance ID. Additive call-gate
operation 14 exposes both without changing the existing operation 12 layout.

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

The local registry invariant keeps the primary IPC service live while creating
independent console-write and host-read auxiliary services. It rejects a third
registration, prevents retirement while an exact-generation operation is in
flight, retires only the named auxiliary record, preserves the primary and
unrelated auxiliary authority, and reuses the freed domain and registry slots
with a new generation. Primary-supervised and auxiliary services have distinct
roles and stored failure policies. Primary generations are restart-managed;
auxiliary identities are stop-only. The CPU fault and exit path consults the
stored policy before entering restart and frame-quarantine handling rather
than inferring that behavior from a role name. Only the primary role is
admitted to the current scheduler and routed through IPC peer closure, fault
reporting, restart, and frame quarantine;
auxiliary domains receive an explicit zero-thread admission limit until those
lifecycle contracts are defined. The scheduler applies the stored limit rather
than granting admission from a role name. In-flight operation admission is
likewise checked against a per-domain limit. Auxiliary retirement is an
explicit two-phase transition: a stop
request changes a ready service to stopping, prevents admission of new
operations, and retirement remains blocked until existing operations drain.
The stop transition requires the persistent control-plane generation and its
host-control capability with `MANAGE`; an auxiliary service cannot supervise a
peer, and a control-plane capability for another object is insufficient.
An accepted stop atomically revokes every delegated capability root and all of
its derived children before publishing the stopping state. This removes both
single- and multi-root authority immediately even while an already-accounted
operation is still draining.
Repeated or stale stop requests fail closed, and a newly reused slot starts in
the ready state. Successful retirement publishes lifecycle event kind 3 with
the exact retired generation and no synthetic fault metadata. Reusing a domain
slot therefore produces a distinct, monotonically sequenced audit record.
Event publication precedes registry and capability-table removal; sequence
exhaustion rejects retirement without losing the stopped domain record or
creating an unsequenced teardown. A full event FIFO still permits retirement
because its sticky overflow record preserves the first dropped auxiliary
generation, sequence, and count; this matches the existing mandatory-teardown
policy for service faults.
Auxiliary registration and bounded one-page loading/admission are exposed to
userspace, while auxiliaries deliberately have no restart policy. The boot
proof stages and fully rolls back an unstarted auxiliary domain with a real
allocator-owned W^X address space. A validated exact-generation resource bind
temporarily permits one accounted thread reservation, but no scheduler context
is queued during this transaction.
Stop-only execution is admitted only after auxiliary scheduler removal,
address-space release, authority revocation, and lifecycle publication have a
single fail-closed teardown transaction.
The domain manager already models the final policy step: after threads,
operations, and address-space ownership are gone, a faulted auxiliary identity
is retired without consuming the primary restart budget. Retirement removes
all of that generation's capability authority and publishes lifecycle kind 4
with exact domain identity, stable service identity, and architectural fault
metadata. The proof rejects retirement while an operation is active and
rejects a stale repeat. Fault teardown first revokes every delegated root,
records the fault, and changes the domain to stopping. Only then does it try to
publish and remove the record. If lifecycle sequencing is exhausted, the
faulted record remains stopped with no usable authority rather than returning
to service or disappearing without an audit event.
The common ring-3 exit and fault handlers now dispatch on the stored policy.
A stop-only context closes no primary IPC endpoint and receives no restart.
After scheduler removal and thread detachment, the handler revokes authority,
switches from the auxiliary page-table root back to the retained kernel root,
releases the exact registered address space, verifies that no owned frames
remain, and only then publishes retirement. The switch requires the active
root to match the exact-generation registry entry; this prevents clearing an
active or unrelated hierarchy. The QEMU hardware gate now queues one minimal
exit-only auxiliary context after the primary workload. It requires ring-3
execution, clean stop-only retirement, exact lifecycle domain/service identity,
stale capability rejection, an empty address-space registry, and zero remaining
owned frames. Multi-page executable images and general admission policy are not
yet exposed. A second hardware-gated auxiliary context deliberately raises
`#UD`; the proof requires lifecycle kind 4, exact domain and stable service
identity, vector/error/instruction metadata, zero remaining authority and
frames, and no restart. Only page faults report `CR2`; other exception kinds
record a zero fault-address field instead of leaking stale page-fault state.
Control ABI operation 15 exposes bounded auxiliary registration. Creation
requires the persistent control-plane generation, host-management authority,
and a valid delegated source capability for the exact requested object and
rights. Its fixed-size, versioned management request is copied only from the
caller's validated mapping and leaves action values available for compatible
lifecycle growth. It creates only an unstarted domain: address-space construction
and scheduler admission remain a separate action 3 transaction. The ring-3 proof
denies creation through a non-managing capability, then delegates attenuated
host-read authority into the clean-exit workload through the authorized path.
Management action 2 accepts only an unstarted, resource-free exact generation with zero unused fields,
reuses the existing two-phase stop transition, and publishes normal auxiliary
retirement. The proof creates and stops one unstarted generation before creating
the separate generation admitted to hardware execution.
Management action 3 performs bounded loading and scheduler admission. It
accepts an exact ready generation plus a caller-owned source range of at most
one page, constructs and retains the target's W^X hierarchy, then queues one
accounted context. The control-plane proof writes a seven-byte exit program,
loads it through this action, and exits; ordinary scheduler handoff dispatches
the auxiliary before returning to the kernel verifier.

Lifecycle ancestry and authority provenance are separate fields with separate
checks. Being a child of the control plane permits accounting and cleanup, but
does not grant any host-control capability. The invariant test requires this
least-authority property, retires a child, rejects its stale ancestry, and then
requires a replacement generation to be attached to the persistent control
parent.

The domains own distinct W^X address spaces and scheduler contexts. When the
service's sole thread faults, the exception path first removes the scheduler
slot, then retires the service domain record and its entire capability table
before resuming the client. The boot gate requires the old domain ID and IPC
handle pair to stop resolving. The same slot can then host a newly delegated
service with an incremented domain generation. Even when its local capability
reuses the prior handle encoding, pairing it with the retired domain ID cannot
resolve authority. The boot proof runs the replacement through ABI negotiation
and clean exit, then retires it again. Replacement creation consumes a fixed
one-attempt budget stored in the persistent manager rather than in the reusable
service record. Failed setup is fail-closed and does not restore the attempt.
An exhausted budget rejects replacement construction without changing the
manager, while fault teardown remains valid and the scheduler may resume a
surviving peer.
An unstarted replacement can also be discarded transactionally while its
thread count and allocator-owned frame count are both zero. Unlike retirement,
discard does not overwrite the last genuinely retired service identity; the
consumed restart attempt remains consumed. Restart admission likewise
revalidates that the quarantined generation still owns exactly seven frames
before creating a replacement.
General restart policy and
outstanding-operation cleanup remain undefined. The fault path also publishes a
capability-gated service-fault event to a four-record FIFO before reusing the
service slot. The queue never overwrites an undelivered record and clears each
consumed slot. The event carries the retired generation-tagged domain ID,
not merely the reusable slot. The control proof compares it with the child ID
supplied at initial entry, so the replacement generation cannot be mistaken for
the faulted service. Concrete fault events also retain the architectural vector,
error code, fault address, and instruction address. Delivery preserves that
record after the packed event word is consumed, and the boot gate requires it
to match the measured page-fault site exactly. Overflow notifications contain
only the dropped count and first domain generation; they do not invent fault
metadata for records the bounded queue could not retain.

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

Every charged call-gate dispatch also holds one bounded in-flight operation
reference against the exact caller generation. A second simultaneous operation
is rejected in this bootstrap policy, and every return path—including denial,
budget exhaustion, blocking, and context switching—must release the reference.
Service retirement and unstarted-domain discard both require the count to be
zero before removing the capability table. Thread exit is deliberately outside
this accounting so it remains available after call-budget exhaustion; it
performs retirement only after the scheduler has removed the thread. The QEMU
gate requires all surviving domain records to have zero in-flight operations.

Domain retirement is also reconciled with the physical-frame ledger at the end
of the bounded boot workload. The failed service must own zero frames after its
address-space leases transfer to the replacement, and the control, failed, and
replacement generations must all own zero frames after final address-space
release. A service retirement is now accepted only when the allocator reports
exactly the seven frames in its fixed address-space layout. The manager records
that retired generation in the logical service's bounded quarantine slot until
an explicitly named replacement of that same service owns all seven leases or
final release reduces the retired owner's live count to zero. Independent
service identities cannot overwrite or clear each other's records. Replacement
setup rollback restores the prior per-service record after returning ownership.
The boot gate requires every slot to be clear at completion. This prevents a
retired service's known frame inventory from becoming untracked, but it is a
fixed-layout precursor to a general object graph destructor rather than one.
