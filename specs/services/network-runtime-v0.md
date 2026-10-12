# Native Network Service Runtime v0

**Status:** A freestanding x86-64 service artifact executes and exits cleanly
in a capability-empty native domain; device delegation is pending.

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

## Deliberate limits

This milestone does not enable PCI decoding, bus mastering, MSI-X, or packet
queues, and it does not move packet processing out of the kernel. Each boundary
is introduced and tested separately in subsequent milestones.
