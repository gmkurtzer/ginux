# Native Service Lifecycle ABI v0

**Status:** Shared Rust and C constants are exercised by the compiled native
volume service under the QEMU gate.

## Exit contract

An x86-64 native service exits through control ABI operation 2. It places 2 in
`RAX`, a zero reserved capability value in `RDI`, and an unsigned eight-bit
status in `RSI`, then executes `INT 0x80`. A valid exit does not return to the
calling context. Values above 255 are rejected without terminating it.

Status 0 is `COMPLETED`: the service completed its requested work and its
service-specific outputs determine whether it initialized or recovered state.
Status 1 is `REJECTED`: authenticated input or persistent state was invalid and
the service deliberately left it unchanged. Statuses 2 through 255 are
reserved and must not be assigned without a later contract revision.

The kernel emits lifecycle kind 5 (`AUXILIARY_EXITED`) with the exact status,
stable service identity, and retiring domain generation. It revokes the
domain's authority, quiesces any bound device, restores the kernel address
space, scrubs and releases owned frames and DMA, and only then publishes final
retirement. A clean exit carries no architectural fault descriptor.

Panics, privileged instructions, invalid memory access, or any other exception
remain faults. They emit lifecycle kind 4 (`AUXILIARY_FAULT`) with architectural
fault metadata and use the independently tested fault-containment path. The
fault path masks and disables interrupts, revokes bus mastering, resets the
device, and scrubs DMA without trusting queue state or a completed request. A
service must not encode a successful result in a deliberate fault. The volume
proof starts a fresh generation only after fault retirement is complete and
requires stable service identity with freshly delegated authority and DMA.

## Sources and compatibility

`ginux-native-abi` is the canonical `no_std` Rust source. The matching
`native-service-lifecycle-v0.h` file is the C binding. These constants refine
the already frozen control ABI v4 operation-2 contract; they do not change its
calling convention or lifecycle record layout.
