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

Unknown feature bits must be ignored. A caller must reject an unsupported ABI
version before invoking other operations. Existing operation semantics cannot
be silently changed within version 1; incompatible changes require a new
version and an overlap period when practical.

Operation 1 is a ping requiring read authority on host-control object 1. It
returns `0x4758` when authorized and `UINT64_MAX - 1` when denied. Operation 2
exits the calling thread and does not return to that context. Operation 3
cooperatively yields and returns zero when the context is scheduled again.
Operation 4 sends the `RSI` word to IPC object 3 and requires write authority;
operation 5 receives one word and requires read authority. The kernel selects
the request or reply direction from the caller's domain role, so an endpoint
cannot receive its own outbound words. IPC failure returns `UINT64_MAX - 2`.
Operation 6 writes one through eight packed bytes from `RSI`, with the byte
count in `RDX`, and requires write authority on console object 2. It returns
the byte count or `UINT64_MAX - 2` on denial or invalid input.
Operation 7 consumes the pending service-fault event and requires manage
authority on host-control object 1. It returns event kind 1 for a contained
service fault, or `UINT64_MAX - 2` on denial or when no event is pending. This
bootstrap revision holds at most one such event.
Any charged operation returns `UINT64_MAX - 3` without dispatch when its
domain's call budget is exhausted. Operation 2 remains available so the caller
can exit after exhaustion.
Unknown operations terminate the proof program without granting an operation.

At initial entry, `RBX` contains the host-control handle, `RCX` contains the
console handle, `R8` contains the IPC handle, and `R9` identifies the proof
context. The QEMU payload first presents the console write-only handle
and requires denial. It then presents the manifest-derived host-control handle
and requires success. The kernel accepts the boot only after observing denial,
authorization, rejection of IPC, console, and event calls using the wrong
object, one authorized console record, a FIFO message, receipt of its peer's
contained-fault event, resumption, and explicit survivor exit.
Both contexts also query operation 0 and require the exact version-1 feature
word before continuing.

Handles encode a private table slot and generation and are meaningful only in
the calling domain selected by the scheduler. The call gate never defaults to
the control-plane namespace. Stale generations, out-of-range slots, and nonzero
reserved high bits are rejected. A later version must define argument/result structures,
cancellation and restart semantics, copy/validation rules, transfer semantics,
and a broader negotiation and deprecation policy.
