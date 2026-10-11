# Control-Plane Call Gate v0

**Status:** Version-discoverable bring-up ABI; operations remain experimental.

The measured ring-3 control-plane bundle invokes interrupt vector `0x80` with:

- `RAX`: operation number;
- `RDI`: opaque domain-local capability handle;
- `RSI`: first immediate argument; and
- `RAX` on return: result value.

Operation 0 requires no capability and returns a discovery word. Bits 16-31
contain ABI version 1 and bits 0-15 contain its feature bitmap:

| Bit | Feature |
|---:|---|
| 0 | opaque capability handles |
| 1 | cooperative context switching |
| 2 | isolated address-space switching |
| 3 | bounded IPC |
| 4 | user-fault containment |
| 5 | bounded console writes |
| 6 | service-fault events |
| 7 | blocking IPC receive |
| 8 | sticky generation-tagged IPC peer closure |
| 9 | bounded userspace reads |
| 10 | structured ABI descriptor and bounded userspace writes |
| 11 | structured service-fault details |
| 12 | structured lifecycle-event details |
| 13 | structured IPC delivery details |
| 14 | structured lifecycle service identity |
| 15 | bounded auxiliary lifecycle management |

Unknown feature bits must be ignored. A caller must reject an unsupported ABI
version before invoking other operations. Existing operation semantics cannot
be silently changed within version 1; incompatible changes require a new
version and an overlap period when practical. The scalar discovery word and
structured descriptor are encoded from the same kernel constants so their
version, feature bitmap, and bounded-console limit cannot drift independently.
Those constants also drive symbolic expansion in the signed ring-3 proof
workload, preventing its discovery and descriptor expectations from drifting
from the kernel.

Operation 1 is a ping requiring read authority on host-control object 1. It
returns `0x4758` when authorized and `UINT64_MAX - 1` when denied. Operation 2
exits the calling thread and does not return to that context. A stop-only
auxiliary supplies an exit status from 0 through 255 in `RSI`; larger values
are rejected without removing the thread. Other domain roles ignore `RSI`.
The hardware proof first attempts status 256, requires `UINT64_MAX - 2`, then
exits the same still-running context with status 42.
Operation 3
cooperatively yields and returns zero when the context is scheduled again.
Operation 4 sends the `RSI` word to IPC object 3 and requires write authority;
operation 5 receives one word and requires read authority. The kernel selects
the request or reply direction from the caller's domain role, so an endpoint
cannot receive its own outbound words. Queued words retain an internal exact
sender-generation tag and a per-direction monotonic sequence number that are
validated at dequeue, but operation 5 returns only the scalar value in ABI v1.
Failed enqueues do not consume sequence numbers, while intentional retirement
cleanup can create valid gaps. IPC failure returns `UINT64_MAX - 2`.
After queued replies are drained, operation 5 returns `UINT64_MAX - 4` once
for each generation-tagged service closure retained by the endpoint. Closure
records have a separate internal monotonic sequence that operation 5 does not
expose in ABI v1.
Operation 6 writes one through eight packed bytes from `RSI`, with the byte
count in `RDX`, and requires write authority on console object 2. It returns
the byte count or `UINT64_MAX - 2` on denial or invalid input.
Operation 7 consumes the pending service-fault event and requires manage
authority on host-control object 1. On a contained service fault it returns a
packed word with event kind 1 in bits 0-7 and the 48-bit generation-tagged
faulted domain ID in bits 8-55; bits 56-63 are zero. It returns
`UINT64_MAX - 2` on denial or when no event is pending. This bootstrap revision
holds four events in FIFO order and refuses to overwrite an undelivered event
when full. If additional records arrive while full, teardown continues and the
queue retains one sticky overflow notification. After the four concrete events
are consumed, operation 7 returns event kind 2, the first dropped domain in
bits 8-55, and a saturating dropped-record count in bits 56-63. Consuming that
notification clears the overflow state. Concrete and summarized events carry
an internal monotonic sequence for replay detection, but the packed ABI v1
result does not expose it.
For a concrete service fault, the kernel retains vector, error code, fault
address, and instruction address with the consumed record for supervision
auditing.
The shared ABI constants define lifecycle kind 1 as a supervised-service fault,
kind 2 as an overflow summary, and kind 3 as a clean auxiliary-service
retirement. Kind 3 carries the exact retired generation, zero detail, and no
fault descriptor. It is currently exercised by the bounded registry proof;
the ring-3 proof also creates and cleanly retires an auxiliary generation.
Kind 4 is a stop-only auxiliary-service fault. It carries the exact domain and
stable service identities plus architectural fault metadata, and never
consumes the primary service restart budget.
Kind 5 is a normal stop-only auxiliary exit. It carries the exact retired
domain in bits 8-55 and the workload's eight-bit exit status in bits 56-63 of
operation 7's packed result; the structured lifecycle descriptor exposes the
same value in its detail byte. It carries no fault metadata and is distinct
from a management-requested kind 3 stop.
Operation 8 receives from IPC object 3 and requires read authority. It returns
an already queued word immediately. When the inbound direction is empty and a
peer is runnable, it blocks the caller; a peer send wakes the caller and makes
the sent word the operation's eventual return value only when the object and
inbound endpoint direction match the saved wait key. If blocking would leave
no runnable thread, it returns `UINT64_MAX - 2` without changing scheduler state.
This direct-delivery path is bound to the exact peer generation and consumes
the next sequence number from the same direction as queued delivery. The
number is committed only after the matching waiter is woken; otherwise queued
fallback uses that number. A nonempty direction always queues to preserve FIFO
order.
If the opposite endpoint's domain exits or faults while the caller is blocked,
the kernel makes the caller ready and operation 8 returns
`UINT64_MAX - 4`. Cleanup matches both object and endpoint direction; unrelated
waiters are not released. A caller arriving after teardown receives the same
result from the sticky closure queue instead of blocking on a dead peer.
Operation 9 writes one through 64 bytes from the user virtual address in `RSI`,
with the byte count in `RDX`, and requires write authority on console object 2.
Before copying, the kernel checks overflow, the lower-canonical bound, and the
present/user bits at every page-table level for every covered byte. Large-page
aliases are rejected. With SMAP temporarily opened around only the copy, the
kernel stages the complete input in a fixed kernel buffer, validates printable
UTF-8, and emits it only after every check succeeds. It returns the byte count
or `UINT64_MAX - 2`; invalid ranges produce no partial console output. The
ring-3 proof rejects both an unmapped null range and a range beginning in the
code page but crossing into its absent guard page.
Operation 10 writes a 16-byte little-endian ABI descriptor to the user virtual
address in `RSI`; `RDX` must equal 16. The descriptor contains a `u32` byte
size, `u16` ABI version, `u16` feature bitmap, `u32` maximum buffered-console
length, and a reserved zero `u32`. Copy-out requires present, user, and writable
permissions at every page-table level, rejects large pages and range overflow,
and opens SMAP only for the bounded write. It returns 16 or
`UINT64_MAX - 2`. The ring-3 proof stores it on a writable stack page, checks
every field, and requires the same operation to reject its read-only code page
before continuing.
Operation 11 requires manage authority on host-control object 1 and writes the
most recently consumed concrete kind 1 or kind 4 fault record to the user address in
`RSI`; `RDX` must equal 48. The little-endian record contains `u32` size,
`u16` ABI version, `u16` vector, and `u64` values for domain generation, error
code, fault address, and instruction address, followed by a reserved zero
`u64`. It uses the bounded writable-range copy rules and returns 48 or
`UINT64_MAX - 2`. Overflow notifications have no synthetic fault record. The
ring-3 proof first requires denial with its console-only capability, then uses
host-management authority and validates the descriptor header before
exhausting its call budget. A denied request does not alter the retained record.
The retained fault record is independent of the general lifecycle delivery
cursor: consuming a later clean stop or exit event does not replace it.
Operation 12 requires the same host-management authority and writes a 24-byte
little-endian descriptor for the most recently consumed lifecycle event. It
contains `u32` size, `u16` ABI version, `u8` event kind, `u8` detail, `u64`
generation-tagged domain ID, and `u64` monotonic lifecycle sequence. `RDX` must
equal 24, and the same bounded writable-range validation applies. It returns 24
or `UINT64_MAX - 2`. The ring-3 proof rejects the console capability, then
validates the descriptor header, original service generation, and sequence one
using host-management authority.
Operation 13 requires read authority on IPC object 3 and writes a 32-byte
little-endian descriptor for the calling endpoint's most recent accepted
delivery. It contains `u32` size, `u16` ABI version, `u8` kind, one reserved
zero byte, and `u64` values for source domain generation, monotonic sequence,
and delivered value. Kind 1 identifies a message; kind 2 identifies a sticky
peer closure, whose source and value both name the retired generation. `RDX`
must equal 32, and bounded writable-range validation applies. It returns 32 or
`UINT64_MAX - 2`. The descriptor is observational and does not dequeue data.
The ring-3 proof rejects console authority, then validates the original
service generation, reply sequence one, and value 2 using IPC read authority.
Service-side descriptor state is cleared at retirement and must be empty when
a replacement generation binds; channel-wide sequence counters continue.
Operation 14 requires host-management authority and writes a 32-byte
little-endian descriptor for the most recently consumed lifecycle event. Its
first 24 bytes preserve operation 12's size/version/kind/detail, exact domain
generation, and lifecycle sequence layout, except that the size field is 32.
The final `u64` is the stable logical service ID. The primary service retains
that ID across replacement domain generations; an unrelated auxiliary
registration receives a new service generation. `RDX` must equal 32, bounded
writable-range validation applies, and denial returns `UINT64_MAX - 2` without
altering the retained event. The proof rejects console authority and validates
the original faulted domain, sequence one, and logical service ID.
Operation 15 submits a versioned management request and requires host-control
`MANAGE` authority in `RDI`. `RSI` points to a 32-byte read-only request and
`RDX` must equal 32. The request contains `u32` size, `u16` ABI version, `u16`
action, then `u64` source capability, object, and rights fields. Action 1 creates
an unstarted stop-only auxiliary domain. The kernel copies through the bounded
owned-mapping path, requires the exact size/version/action tuple, revalidates
the delegated source against the requested nonzero object and nonempty rights
subset, seals the child capability table, and returns its generation-tagged
domain ID. Invalid memory, malformed fields, denial, or registry exhaustion
returns `UINT64_MAX - 2` without creating a partial record. The ring-3 proof
requires denial through a non-managing authority, rejects an unknown action
through valid authority, creates one attenuated host-read auxiliary through host-management authority, and the hardware
scheduler proof then executes and retires that exact generation. New lifecycle
actions must receive new action values and explicitly versioned layouts rather
than repurposing existing fields.
Action 2 requests stop of the exact generation in the source field and requires
the object and rights fields to be zero. An unstarted, resource-free target is
retired directly. A queued target is accepted only while its context is staged
or ready:
the scheduler removes it before execution, drains its thread accounting,
releases its complete W^X hierarchy, revokes every delegated root, and emits
kind 3. A running target is still rejected. The ring-3 proof stops one
unstarted generation and one staged generation. It also dispatches an
initialized auxiliary through action 5, allows it to execute one cooperative
yield, observes the exact generation as ready again, and then stops it before
it can resume. After validating all three sequenced retirements, the proof
creates a distinct generation for normal live execution.
Action 3 loads and queues an unstarted auxiliary. The source field is its exact
domain generation, the object field is a readable caller address, and the
rights field is a byte length from 1 through 4096. The kernel allocates a fresh
seven-frame W^X hierarchy, copies only that bounded range through the caller's
validated mapping into the target's initially zeroed RX code frame, binds the
resource record, and accounts one staged thread. Scheduler handoff and
involuntary preemption both leave it ineligible until an explicit action 5.
Any failed stage releases what it acquired and returns
`UINT64_MAX - 2`. Action 3 does not jump directly between domains. The hardware
proof first supplies an unmapped source after
target allocation, requires failure, then successfully loads the same exact
generation. The second allocation can succeed only if the failed transaction
returned every target frame and left no resource binding behind.
Action 4 initializes a staged auxiliary's private payload. The source field is
the exact staged domain generation, the object field is a readable caller
address, and the rights field is a byte length from 1 through 4096. The kernel
copies the complete bounded range into the high end of the target's private NX
stack page, then records its target virtual address in `RSI` and its length in
`RDX`. It rejects absent, running, blocked, ambiguous, or wrong-generation
contexts. The hardware proof initializes both a subsequently cancelled workload
and the executed workload; the latter validates the pointer, length, and bytes
before exiting.
Action 5 starts a staged auxiliary. The source field is its exact domain
generation and the object and rights fields must be zero. The scheduler changes
that one unambiguous, initialized context from `Staged` to `Ready`; normal
scheduling may dispatch it immediately after the management call returns. A
loaded context has an internal initialization latch that only action 4 can set.
Premature starts, repeated initialization or starts, and absent, ready,
running, blocked, or wrong-generation targets fail closed.
Action 6 queries the exact live auxiliary named by source; object and rights
must be zero. It returns 1 for created with no runtime context, 2 for staged, 3
for ready, 4 for running, or 5 for blocked. Non-auxiliary, stopping, retired,
wrong-generation, ambiguous-context, and malformed requests fail closed. The
ring-3 proof observes created, staged, and ready in order and rejects a query
for a retired generation.
Action 7 has the same exact-generation, authority, and zero object/rights
requirements as action 6. It returns a flags word whose bit 0 is set only after
action 4 has installed the private payload; all other bits are zero in version
1. A created or merely loaded auxiliary returns zero. The ring-3 proof observes
the bit clear after loading and set after initialization, before action 5 is
allowed to succeed.
The ring-3 control plane consumes both kind 3 stop records as they are
published. After action 5 it voluntarily yields, resumes only after the
workload has exited, consumes the kind 5 record, and validates the exact
generation and status 42 from the packed result. It then uses operation 12 to
validate the descriptor version, kind, detail, exact generation, and lifecycle
sequence five. Operation 14 additionally proves that the exited auxiliary has
a nonzero stable logical service identity distinct from the primary managed
service. The kernel verifier requires the lifecycle FIFO to be empty afterward;
lifecycle observation is therefore a userspace control-plane responsibility in
this proof rather than deferred kernel-only inspection.
Any charged operation returns `UINT64_MAX - 3` without dispatch when its
domain's call budget is exhausted. Operation 2 remains available so the caller
can exit after exhaustion.
Each charged entry first acquires a bounded in-flight operation reference on
the exact caller generation and releases it before returning through the call
gate, even when dispatch is denied, the budget is exhausted, or the scheduler
selects another context. A service capability table cannot be retired while
such a reference remains. Operation 2 is uncharged and unreferenced so it can
remove the current thread before retiring the service.
Before either bounded copy direction touches user memory, the kernel requires
active CR3 to equal the scheduler's current root, revalidates the complete
fixed page-table hierarchy, and confirms every page-table and payload frame is
still live under the calling domain's exact generation in the allocator
ledger. Per-page permission checks then apply to the requested range. This
recheck narrows the interval in which internal page-table corruption could turn
an otherwise bounded ABI copy into cross-domain access; concurrent mapping
mutation remains unsupported in the single-core bootstrap.
Permission validation walks each covered page exactly once rather than once per
byte. The end address remains exclusive, so an aligned end does not inspect the
following page, while a range containing even one byte of a guard page is
rejected before the copy begins.
Unknown operations terminate the proof program without granting an operation.

At initial entry, `RBX` contains the host-control handle, `RCX` contains the
console handle, `R8` contains the IPC handle, and `R9` identifies the proof
context. For the control context, `R10` contains the initial supervised service
domain ID and `R11` contains its logical service ID. The QEMU payload first writes `BUFFER!\n` from its read-only
executable mapping through operation 9, then requires the same call to reject
an unmapped null range without output. It also fetches and validates the
structured ABI descriptor. It then presents the console write-only
handle to the host-control operation and requires denial before presenting the
host-control handle and requiring success. The kernel accepts the boot only after observing denial,
authorization, rejection of IPC, console, and event calls using the wrong
object, one authorized console record, a FIFO message, receipt of its peer's
contained-fault event with the expected domain generation, resumption, and
explicit survivor exit.
Both contexts also query operation 0 and require the exact version-1 feature
word before continuing.

Handles encode a private table slot and generation and are meaningful only in
the calling domain selected by the scheduler. The call gate never defaults to
the control-plane namespace. Stale generations, out-of-range slots, and nonzero
reserved high bits are rejected. A later version must define general argument/result structures,
cancellation and restart semantics, copy/validation rules, transfer semantics,
and a broader negotiation and deprecation policy.
