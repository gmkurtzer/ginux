# Native Network Service Runtime v0

**Status:** A freestanding x86-64 service configures and activates confined
empty virtio-net queues in a capability-limited native domain.

## Purpose

The native network service is the future owner of the virtio-net queue data
path. Its first milestone establishes a separately built, memory-safe Rust
artifact without adding network code to the kernel or depending on a hosted
runtime.

## Artifact contract

`ginux-network-service-runtime` is a `no_std`, `no_main`, freestanding x86-64
PIE built for `x86_64-unknown-none`. Its entry point uses only the stable native
control and lifecycle ABI. It requires control ABI v4 and its structured
descriptor before reporting a clean `COMPLETED` lifecycle outcome. An
incompatible scalar or structured ABI fails closed through a driver fault.

The build is part of `make manifest`, `make check`, and the clean `make gate`
dependency graph. Clippy treats every warning as an error. Manifest generation
places the artifact at internal path `native/network` in the deterministic
initramfs, so the signed root-image digest authenticates its exact bytes.

During boot the kernel resolves only that exact path from the published
read-only root, parses it with the bounded native ELF loader, and requires a
nonzero executable entry, exactly one read-only, executable, and writable load
segment, and at least one valid relative relocation. No unmeasured fallback
artifact is accepted.

The kernel independently materializes the accepted plan into freshly owned
pages, copies only bytes covered by load segments, zeros memory tails, applies
only the validated relative relocations, and installs one read-only, one
read-execute, and one read-write region. A non-executable stack is bounded by
unmapped guard pages. The proof checks the resulting mappings and relocation
values, then scrubs and releases the complete temporary address space.

The first live run creates a fresh stop-only auxiliary domain with a sealed,
empty boot capability table. The measured entry executes at ring 3, negotiates
control ABI v4, copies and validates its structured descriptor, and reports
`COMPLETED` through lifecycle ABI v0. The kernel requires an exit rather than a
fault, restores its own page-table root, observes no registered service address
space or owned frame, and retires the domain. No bootstrap pointer, opaque
capability handle, device page, DMA page, or interrupt route is supplied.

The next boundary is specified independently by
[Native Network Service Bootstrap ABI v0](../abi/native-network-bootstrap-v0.md).
Its fixed record separates the network service from the existing block-service
bootstrap ABI and names both queue notification registers, one DMA window, the
opaque generation capability, and the pinned MAC without exposing PCI, MSI-X,
APIC, or IOMMU internals. A second fresh service generation receives and
validates that record, exercises only its read-only logical network-device
probe, and exits cleanly. Its common and notification mappings remain inert
because PCI decoding is disabled; its owned DMA page remains inaccessible to
the device because VT-d and bus mastering are disabled. Teardown revokes the
capability and scrubs the bootstrap, image, stack, page-table, and DMA frames.
A separate measured generation receives an otherwise valid record with a
nonzero reserved field. The service rejects it through its fault path before
accessing MMIO or probing the capability; the kernel contains the fault,
restores its root, revokes the capability, and scrubs all resources while PCI
and VT-d remain inactive.

A third valid generation activates an exact VT-d hierarchy for its sole DMA
page before ring-3 entry. PCI decoding, bus mastering, queues, and MSI-X remain
disabled, so the service still cannot initiate device access. Clean exit first
disables and retires the translation hierarchy and then scrubs the DMA page;
the QEMU gate requires both leases to be gone afterward.
A faulted generation repeats the malformed-reserved-field rejection after its
exact VT-d hierarchy is active. Fault teardown disables translation, scrubs all
six table frames, and releases the DMA page last while PCI authority remains
disabled.

A further generation receives bootstrap feature `EMPTY_QUEUES`, read/write
authority on its generation-bound network capability, and PCI memory decoding
with bus mastering still clear. The service zeros its DMA page, negotiates
exactly `VERSION_1`, `ACCESS_PLATFORM`, `MAC`, and `MRG_RXBUF`, configures
eight-entry receive and transmit rings, leaves both available indices zero,
sets `DRIVER_OK`, and invokes capability command 12. The kernel independently
requires the active VT-d translation, exact queue addresses, disabled MSI-X,
empty DMA page, and the expected device state before enabling bus mastering.
Clean lifecycle exit revokes bus mastering, resets and disables the function,
then retires VT-d before any service-owned frame is scrubbed.

A separate generation arms a kernel-internal activation-return fault injector;
this does not add a test mode to the public bootstrap ABI. The service fully
configures both queues and the kernel genuinely enables bus mastering, then the
capability call returns failure. The compiled driver follows its normal fatal
path and faults at ring 3. Fault handling proves the queues were live, revokes
bus mastering, resets and disables the function, retires VT-d, and scrubs DMA
without trusting a clean service exit.

## Deliberate limits

This milestone publishes no descriptor, writes no notification register,
enables no MSI-X route, and transfers no packet. Interrupt delivery and
service-owned packet processing remain separate milestones.
