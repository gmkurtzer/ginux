# Control-Plane Call Gate v0

**Status:** Version-discoverable bring-up ABI; operations remain experimental.

The measured ring-3 control-plane bundle invokes interrupt vector `0x80` with:

- `RAX`: operation number;
- `RDI`: opaque domain-local capability handle;
- `RSI`: first immediate argument; and
- `RAX` on return: result value.

Operation 0 requires no capability and returns a discovery word. Bits 16-31
contain ABI version 4 and bits 0-15 contain its feature bitmap:

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
be silently changed within a version; incompatible changes require a new
version and an overlap period when practical. Version 4 preserves operations
0 through 17 and adds the persistent-volume object at operation 18. The
pre-release bootstrap switches atomically to v4 rather than carrying adapters;
`control-v1.rs`, `control-v2.rs`, and `control-v3.rs` remain frozen as drift
evidence. The scalar discovery word and
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
validated at dequeue, but operation 5 returns only the scalar value in ABI v2-v4.
Failed enqueues do not consume sequence numbers, while intentional retirement
cleanup can create valid gaps. IPC failure returns `UINT64_MAX - 2`.
After queued replies are drained, operation 5 returns `UINT64_MAX - 4` once
for each generation-tagged service closure retained by the endpoint. Closure
records have a separate internal monotonic sequence that operation 5 does not
expose in ABI v2-v4.
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
an internal monotonic sequence for replay detection, but the packed ABI v2-v4
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
Kind 6 reports that a supervised service generation has successfully run and
blocked on its request endpoint. It carries the exact service domain and stable
logical service identity with zero detail and no fault metadata. A generation
publishes readiness at most once.
Kind 7 reports a supervised service's clean exit with zero detail. It is
distinct from kind 1 fault termination and retains the exact domain generation
and stable primary-service identity.
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
The ring-3 proof rejects console authority, then validates the replacement's
final reply sequence four and explicit `UINT64_MAX - 2` invalid-command result
using IPC read authority. It also requires the
source generation to differ from the original faulted service; the kernel gate
matches it to the exact replacement generation.
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
before exiting. The executed auxiliary also invokes operation 15 with its
console-only capability and requires denial, proving that reaching the call
gate cannot expand a workload into host-management authority.
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
The ring-3 control plane first consumes the replacement service's kind 7 clean
exit and validates its descriptor at sequence three. It consumes both kind 3 stop records as they are
published. After action 5 it voluntarily yields, resumes only after the
workload has exited, consumes the kind 5 record, and validates the exact
generation and status 42 from the packed result. It then uses operation 12 to
validate the descriptor version, kind, detail, exact generation, and lifecycle
sequence seven. Operation 14 additionally proves that the exited auxiliary has
a nonzero stable logical service identity distinct from the primary managed
service. The kernel verifier requires the lifecycle FIFO to be empty afterward;
lifecycle observation is therefore a userspace control-plane responsibility in
this proof rather than deferred kernel-only inspection.
Any charged operation returns `UINT64_MAX - 3` without dispatch when its
domain's call budget is exhausted. Operation 2 remains available so the caller
can exit after exhaustion.

Operation 16 is the network-device call introduced by v2 and retained by v4. It requires `READ` authority on
network-device object 4. `RSI` is the command and `RDX` its argument. Command 0
requires argument zero and returns the fixed discovery word
`0x0000000105dc0009`: descriptor version 9 in bits 0-15, MTU 1500 in bits
16-31, and one queue in bits 32-63. Read-authorized command 1 requires argument
zero and returns the current link-state bit. Write-authorized command 2 accepts
only zero or one and updates that bit. Write-authorized command 3 stores a
packed IPv4 address in bits 0-31 and prefix in bits 32-39; prefix must be at
most 32, higher bits must be zero, and address zero requires prefix zero.
Read-authorized command 4 requires argument zero and returns the packed state.
Changing or clearing the IPv4 configuration atomically clears a default gateway
that is no longer usable within the new prefix; the whole update advances the
configuration generation once.
Write-authorized command 5 stores zero or one on-subnet, non-self, non-network,
non-broadcast default gateway. Read-authorized command 6 requires argument zero
and returns it. Read-authorized command 7 requires argument zero and returns a
monotonic, saturating configuration generation. Each effective link, IPv4, or
gateway change advances the generation once; accepted no-op writes and rejected
writes do not advance it. Calls are serialized by the current single-CPU call
gate, so equal generations around a series of reads identify one stable state.
Write-authorized command 8 treats the argument as a pointer to a 16-byte static
route record: destination in bytes 0-3, prefix in byte 4, zero reserved bytes
5-7, next-hop in bytes 8-11, and zero reserved bytes 12-15. A nonempty route
requires a canonical destination prefix from 1 through 32 and an on-link usable
next-hop; an all-zero record clears it. Read-authorized command 9 copies the
current record to the writable user address in the argument and returns 16.
Only one such route exists, and an effective route change advances the same
configuration generation. IPv4 reconfiguration also clears the route when its
next-hop is no longer on-link.
Read-authorized command 10 copies a version-1, 56-byte snapshot to the writable
user address in the argument and returns 56. Its seven little-endian 64-bit
words are snapshot version, configuration generation, link state, packed IPv4
configuration, default gateway, packed route destination/prefix, and route
next-hop. The current single-CPU non-nested call gate serializes the copy with
all configuration mutations.
The boot proof also delegates only `READ` authority to a separate observer,
which can use command 10 but receives `UINT64_MAX - 2` from command 2.
A separate `WRITE`-only configurator can use command 2 but is denied commands 1
and 10 before any snapshot bytes are copied to its supplied address.
Write-authorized command 11 copies a version-1, 64-byte apply request from the
user address in its argument. Its eight little-endian 64-bit words are version,
expected generation, link state, packed IPv4 configuration, default gateway,
packed route destination/prefix, route next-hop, and reserved zero. The kernel
rejects a mismatched generation with `UINT64_MAX - 4`, validates the complete
candidate state before mutation, and returns the resulting generation. An
effective apply publishes all fields and advances the generation once; a valid
no-op returns the unchanged generation. Serialization and atomic visibility
currently rely on the single-CPU non-nested call gate and require a locking or
sequence protocol before concurrent device calls are permitted.
Unknown commands, invalid arguments, and
denial return `UINT64_MAX - 2`. This is a
device-authorization skeleton only; it exposes no packets, general route table,
DMA, interrupts, or hardware-backed configuration.

Operation 17 is the v3 read-only root call retained by v4 and requires `READ` authority on
root object 5. `RSI` selects the command and `RDX` supplies its argument.
Command 0 requires argument zero and returns `0x0000004000100002`: profile
version 2 in bits 0-15, the 16-node index limit in bits 16-31, and the 64-byte
read limit in bits 32-47.

Command 1 copies a 40-byte little-endian lookup request from `RDX`. It contains
`u32` size, `u16` ABI version, reserved-zero `u16`, then `u64` values for path
address, path length, output address, and output size. Paths contain 1 through
255 bytes and use the normalized initramfs path rules. Output size must be 24.
An exact match copies a descriptor containing `u32` size, `u16` ABI version,
reserved-zero `u16`, `u32` mode, reserved-zero `u32`, and `u64` data size.

Command 2 copies a 48-byte request with the same header, path address, and path
length, followed by `u64` offset, output address, and requested size. Requested
size must be 1 through 64. A valid request copies at most the requested bytes
and stops at end of file; an offset exactly at end of file returns zero without
touching the output. Kernel staging precedes every copy-out. Unknown commands,
malformed requests, missing paths, invalid offsets or user ranges, and authority
denial return `UINT64_MAX - 2`. No operation grants write, create, remove,
rename, or mount authority.

Command 3 copies a 48-byte directory request. Its header is followed by `u64`
directory-path address, path length, cursor, output address, and output
capacity. Address and length zero name the root; nonempty paths must resolve to
a directory. Capacity must be 32 through 287 bytes. Cursor zero starts at the
first indexed node. A matching direct child produces one fully staged record:
`u32` total size, `u16` ABI version, reserved-zero `u16`, `u64` next cursor,
`u32` mode, `u32` name size, `u64` data size, and the un-terminated basename.
The return value is the record size. A cursor at the end of the matching
children returns zero without touching output; a cursor beyond the index, a
non-directory path, or insufficient capacity fails without partial output.
Profile v2 adds this command; the other operation-17 commands retain their
profile-v1 layouts. Enumeration does not grant mutation or mount authority.

Operation 18 is the v4 persistent-volume call and requires authority on volume
object 6. `RSI` selects the command and `RDX` supplies its argument. Read-only
command 0 requires argument zero and returns descriptor
`0x0000000200400001`: contract version 1 in bits 0-15, maximum transfer 64 in
bits 16-31, and two record slots in bits 32-63. Read-only command 1 requires
argument zero and returns the current generation, or zero for an empty volume.

Read-only command 2 and write-only command 3 copy a 32-byte request from the
address in `RDX`. It contains `u32` size 32, `u16` ABI version 4,
reserved-zero `u16`, then `u64` expected generation, buffer address, and buffer
size. Sizes are 1 through 64. Command 2 stages the exact recovered payload and
copies it to a writable caller range, returning its length without touching
the remaining capacity. Command 3 stages the complete caller payload before
committing it with the two-slot prepare/flush/commit/flush algorithm and
returns the new generation. The expected generation must be zero for an empty
volume or exactly match the current generation; a mismatch returns
`UINT64_MAX - 4` without mutation. Unknown commands, malformed inputs, absent
rights, corrupt media, and device errors return `UINT64_MAX - 2`.

The QEMU proof attenuates signed control-plane object 6 into a write-only
service and a distinct read-only replacement. The writer cannot observe the
volume, commits generation one, faults, and loses its capability. The
replacement recovers the exact payload, is denied mutation, exits, and also
loses its capability. The current backend is volatile kernel memory, so this
proves persistence across isolated service failure rather than VM restart.
Object 7 is reserved by this v4 authority envelope for a block-device grant.
It has no control-call operation yet: the kernel consumes its attenuated
`READ` authority while constructing a read-only ring-3 MMIO mapping and its
separate attenuated `WRITE` authority while constructing a writable control
mapping. The latter proves status mutation followed by fault revocation and
kernel quiescence, but receives no DMA or queue authority. A usable device
driver still requires a later, separately specified contract rather than an
implicit extension of operation 18.
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
domain ID, `R11` contains its logical service ID, and `R14` initially contains
the root handle. The bootstrap stack also retains that same opaque handle at
`0x80002f00`; changing it cannot create authority because every call resolves
the supplied generation-tagged handle in the caller's sealed table. The QEMU payload first writes `BUFFER!\n` from its read-only
executable mapping through operation 9, then requires the same call to reject
an unmapped null range without output. It also fetches and validates the
structured ABI descriptor. It then presents the console write-only
handle to the host-control operation and requires denial before presenting the
host-control handle and requiring success. The kernel accepts the boot only after observing denial,
authorization, rejection of IPC, console, and event calls using the wrong
object, one authorized console record, a FIFO message, receipt of its peer's
contained-fault event with the expected domain generation, resumption, and
explicit survivor exit.
Both contexts also query operation 0 and require the exact version-3 discovery
word before continuing.

Handles encode a private table slot and generation and are meaningful only in
the calling domain selected by the scheduler. The call gate never defaults to
the control-plane namespace. Stale generations, out-of-range slots, and nonzero
reserved high bits are rejected. A later version must define general argument/result structures,
cancellation and restart semantics, copy/validation rules, transfer semantics,
and a broader negotiation and deprecation policy.
