# Native Service Lifecycle ABI v0

**Status:** Shared Rust and C constants are exercised by the compiled native
volume service under the QEMU gate.

## Exit contract

Before touching a delegated device, a native service invokes capability-free
control operation 0. Bits 16 through 31 of its discovery result must equal
control ABI version 4; bits 0, 4, and 10 must advertise opaque capabilities,
fault containment, and the structured descriptor. Unknown feature bits are ignored, while nonzero bits above
31, a different version, or a missing required feature fail closed. The shared
Rust source provides the canonical validator and negative tests.
After scalar discovery, the service invokes capability-free operation 10 to
copy the fixed 16-byte v4 descriptor into writable service memory. It validates
the size, version, required feature subset, 64-byte console limit, and reserved
zero field before reading its bootstrap record or touching MMIO.
The live QEMU proof also returns an older version to one compiled service
generation. Its fatal path is contained before the virtio queue becomes active;
the inactive device, capability, address space, and zero DMA lease are retired
before a compatible replacement generation recovers persistent state.
A second generation receives the valid v4 scalar discovery word but operation
10 returns a descriptor with a nonzero reserved field. It must take the same
pre-device fatal path, retire completely, and permit another compatible
generation to recover.

An x86-64 native service exits through control ABI operation 2. It places 2 in
`RAX`, a zero reserved capability value in `RDI`, and an unsigned eight-bit
status in `RSI`, then executes `INT 0x80`. A valid exit does not return to the
calling context. Values above 255 are rejected without terminating it.

Status 0 is `COMPLETED`: the service completed its requested work and its
service-specific outputs determine whether it initialized or recovered state.
Status 1 is `REJECTED`: authenticated input or persistent state was invalid and
the service deliberately left it unchanged. Status 2 is `RESTART_REQUIRED`:
the kernel safely cancelled an ambiguous device operation, and the current
generation must not reuse its device state. Statuses 3 through 255 are reserved
and must not be assigned without a later contract revision.

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

## Block-wait cancellation

Native block wait operation 19 has two commands. `PREPARED` command 1 waits on
a published request that has not been notified; a timeout returns `FAILED` and
does not consume its generation, so the service may notify and retry it.
`NOTIFIED` command 2 declares that the device may have observed the request. A
successful interrupt returns zero. A timeout masks MSI-X, revokes bus
mastering, resets the device, retires the interrupt route, scrubs the ambiguous
DMA state, and returns `CANCELLED`; that request generation can never be
retried. The service must then exit with `RESTART_REQUIRED`. A fresh domain
generation reinitializes the reset device with a new DMA lease.

## Network device wait

Native network wait operation 20 accepts `TX_NOTIFIED` command 1 and
`RX_POSTED` command 2. A transmit-only generation admits only command 1 with
request generation one. It requires the same domain's write-authorized
network capability, live exact VT-d lease, TX queue 1 bound to MSI-X entry 1,
and an active confined device. The kernel blocks the service, unmasks vector
35, and returns zero only after one matching interrupt. A stale or duplicate
request returns `FAILED` without blocking. A timeout returns only after masking
and disabling MSI-X, revoking bus mastering, resetting the device, and retiring
the route. It returns `CANCELLED`; the service exits with
`RESTART_REQUIRED`, and only a fresh domain generation with a new DMA and VT-d
lease may retry. The QEMU proof forces this path and requires the replacement
generation to complete an interrupt-driven transmit.

An `ARP_PROBE` generation additionally binds RX queue 0 to MSI-X entry 0. Only
after TX command 1/request 1 completes does the kernel advance that exact
device generation and admit RX command 2/request 2. The second wait unmasks
vector 34 and returns zero only after its matching interrupt. A duplicate or
out-of-order command returns `FAILED` without blocking. Both waits must complete
before clean lifecycle exit is accepted. Receive-stage timeout cancellation is
not yet part of this contract.

## Sources and compatibility

`ginux-native-abi` is the canonical `no_std` Rust source. The matching
`native-service-lifecycle-v0.h` file is the C binding. These constants refine
the already frozen control ABI v4 operation-2 contract; they do not change its
calling convention or lifecycle record layout.
The header also provides constant-expression discovery construction and
compatibility predicates. `make abi-conformance` compiles a freestanding C11
consumer that checks accepted and rejected discovery words; `make check` and
the clean gate require it.
