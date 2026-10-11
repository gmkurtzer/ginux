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

Unknown feature bits must be ignored. A caller must reject an unsupported ABI
version before invoking other operations. Existing operation semantics cannot
be silently changed within version 1; incompatible changes require a new
version and an overlap period when practical. The scalar discovery word and
structured descriptor are encoded from the same kernel constants so their
version, feature bitmap, and bounded-console limit cannot drift independently.

Operation 1 is a ping requiring read authority on host-control object 1. It
returns `0x4758` when authorized and `UINT64_MAX - 1` when denied. Operation 2
exits the calling thread and does not return to that context. Operation 3
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
for each generation-tagged service closure retained by the endpoint.
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
notification clears the overflow state.
For a concrete service fault, the kernel retains vector, error code, fault
address, and instruction address with the consumed record for supervision
auditing.
Operation 8 receives from IPC object 3 and requires read authority. It returns
an already queued word immediately. When the inbound direction is empty and a
peer is runnable, it blocks the caller; a peer send wakes the caller and makes
the sent word the operation's eventual return value only when the object and
inbound endpoint direction match the saved wait key. If blocking would leave
no runnable thread, it returns `UINT64_MAX - 2` without changing scheduler state.
This direct-delivery path is bound to the exact peer generation but is not yet
assigned a queued-message sequence number.
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
most recently consumed concrete service-fault record to the user address in
`RSI`; `RDX` must equal 48. The little-endian record contains `u32` size,
`u16` ABI version, `u16` vector, and `u64` values for domain generation, error
code, fault address, and instruction address, followed by a reserved zero
`u64`. It uses the bounded writable-range copy rules and returns 48 or
`UINT64_MAX - 2`. Overflow notifications have no synthetic fault record. The
ring-3 proof first requires denial with its console-only capability, then uses
host-management authority and validates the descriptor header before
exhausting its call budget. A denied request does not alter the retained record.
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
domain ID. The QEMU payload first writes `BUFFER!\n` from its read-only
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
