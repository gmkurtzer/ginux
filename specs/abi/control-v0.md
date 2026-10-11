# Control-Plane Call Gate v0

**Status:** Bring-up ABI; intentionally unstable.

The measured ring-3 control-plane bundle invokes interrupt vector `0x80` with:

- `RAX`: operation number;
- `RDI`: domain-local boot-grant index; and
- `RAX` on return: result value.

Operation 1 is a ping requiring read authority on host-control object 1. It
returns `0x4758` when authorized and `UINT64_MAX - 1` when denied. Operation 2
exits the bring-up thread and does not return. Unknown operations also terminate
the proof program without granting an operation.

The QEMU payload first presents grant index 1, which names the console write-only
capability, and requires denial. It then presents grant index 0, which names the
manifest-derived host-control capability, and requires success. The kernel
accepts the boot only after observing denial, authorization, and explicit exit.

Grant indices are not the stable capability ABI. A later version must use opaque
generation-safe handles, define argument/result structures, cancellation and
restart semantics, copy/validation rules, and version negotiation.
