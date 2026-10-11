# Control-Plane Call Gate v0

**Status:** Bring-up ABI; intentionally unstable.

The measured ring-3 control-plane bundle invokes interrupt vector `0x80` with:

- `RAX`: operation number;
- `RDI`: opaque domain-local capability handle; and
- `RSI`: first immediate argument; and
- `RAX` on return: result value.

Operation 1 is a ping requiring read authority on host-control object 1. It
returns `0x4758` when authorized and `UINT64_MAX - 1` when denied. Operation 2
exits the calling thread and does not return to that context. Operation 3
cooperatively yields and returns zero when the context is scheduled again.
Operation 4 sends the `RSI` word to IPC object 3 and requires write authority;
operation 5 receives one word and requires read authority. IPC failure returns
`UINT64_MAX - 2`.
Unknown operations terminate the proof program without granting an operation.

At initial entry, `RBX` contains the host-control handle, `RCX` contains the
console handle, `R8` contains the IPC handle, and `R9` identifies the proof
context. The QEMU payload first presents the console write-only handle
and requires denial. It then presents the manifest-derived host-control handle
and requires success. The kernel accepts the boot only after observing denial,
authorization, rejection of an IPC call using the wrong object, a FIFO message,
resumption after its peer's contained fault, and explicit survivor exit.

Handles encode a private table slot and generation and are meaningful only in
the calling domain. Stale generations, out-of-range slots, and nonzero reserved
high bits are rejected. A later version must define argument/result structures,
cancellation and restart semantics, copy/validation rules, transfer semantics,
and version negotiation.
