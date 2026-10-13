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

Bootstrap feature `TRANSMIT_PROBE` advances a fresh generation to the first
service-owned packet operation. After activation, the service builds one
bounded queue-1 descriptor, publishes available index one, notifies the exact
transmit register, and blocks through operation 20 on request generation one.
Bootstrap feature `TX_INTERRUPT` binds TX queue 1 to MSI-X entry 1; the kernel
owns entry programming, vector 35, masking, and route retirement. The kernel
independently validates the exact interrupt, rejects replay of generation one,
and checks the complete descriptor, fixed Ethernet payload, completion identity,
and canaries before ordered teardown.
See [Native Network Service Transmit v0](network-transmit-v0.md).

A separate notified-timeout generation keeps MSI-X masked until the calibrated
device timer expires. The kernel cancels that ambiguous generation by disabling
MSI-X, revoking bus mastering, resetting the NIC, retiring vector 35, and then
retiring VT-d and DMA. The service receives `CANCELLED` and exits
`RESTART_REQUIRED`. A fresh generation with newly delegated authority and
leases must then configure the reset device and complete the same transmit.

Bootstrap feature `ARP_PROBE` advances another fresh generation to the first
service-owned receive. The driver binds RX queue 0 to MSI-X entry 0, publishes
one guarded write-only receive descriptor, transmits a padded ARP request, and
waits for TX request one before RX request two. It validates the complete
one-buffer modern virtio header and deterministic QEMU gateway ARP reply. The
kernel independently validates both rings, the request and reply, endpoint
identities, bounds, canaries, and both interrupt completions before retiring the
two routes, device, VT-d hierarchy, DMA page, and domain. See
[Native Network Service Receive v0](network-receive-v0.md).

A separate generation completes TX request one and then has RX request two
held masked until its APIC deadline. Because the peer may already have written
the receive buffer, the kernel treats that state as ambiguous: it cancels and
retires the entire device generation before returning `CANCELLED`. The service
exits `RESTART_REQUIRED`. A newly authorized generation with fresh DMA and VT-d
leases must then complete a full ARP exchange.

Another generation receives a real valid reply whose ARP operation low byte is
changed by a kernel-internal test injector only after RX completion. No public
bootstrap feature exposes this injector. The service's normal packet parser
rejects the unsupported operation and faults. Fault containment quiesces the
active device and both routes before retiring VT-d and DMA; a fresh generation
then validates an unmodified ARP reply.

Bootstrap feature `RX_REUSE` keeps both queues and routes live after the first
reply. Once the kernel independently validates request two, the service poisons
the parsed receive bytes, republishes descriptor zero in available slot one,
advances both queue indices, and completes TX request three plus RX request
four. Kernel teardown requires both used elements, the overwritten poison, the
second valid reply, and intact guards. See
[Native Network Receive Replenishment v0](network-replenishment-v0.md).

The next additive generation uses bootstrap v1 and receives a second,
write-only packet capability. After validating the second reply, it copies the
42 Ethernet bytes into a bounded kernel mailbox. A separate read-only ring-3
consumer receives the packet after complete driver and DMA retirement without
receiving any device authority. See
[Native Network Packet Handoff v0](network-packet-handoff-v0.md).
An explicit test-only v1 feature then faults a new producer immediately after
publishing sequence two. Fault containment tears down the still-active device
generation while preserving the copied packet for another fresh read-only
consumer.
A second test-only v1 feature publishes both validated replies as sequences
three and four before retirement. Two independent read-only ring-3 consumers
drain the bounded four-entry FIFO in order after device teardown.
An independent two-domain proof then blocks a read-only consumer on exact
sequence five, publishes from a write-only producer, wakes only that consumer,
and consumes the exact frame. See
[Native Network Packet Wait v0](network-packet-wait-v0.md).
The control plane can also stop a blocked consumer, releasing its exact wait
key so a fresh consumer can claim the unchanged sequence. See
[Blocked Auxiliary Cancellation v0](../kernel/blocked-auxiliary-cancellation-v0.md).
The additive timed-wait operation expires an exact wait through the local APIC,
preempts a non-cooperative peer, and leaves the sequence available to a fresh
consumer. Publication before a later deadline cancels that timer. See
[Native Network Packet Wait Timeout v0](network-packet-timeout-v0.md).
When the four-entry FIFO is full, additive operation 25 blocks the bound
write-only producer. Removing the head wakes that producer to retry the exact
next sequence without reserving or mutating a slot during the wake. See
[Native Network Packet Backpressure v0](network-packet-backpressure-v0.md).

Bootstrap v2 adds a read-only packet-egress capability to a fresh device
generation. The service rejects publication and future-sequence access,
consumes the exact protocol-generated ICMP reply into its confined DMA page,
rejects replay, and transmits the bytes through queue 1. The kernel validates
the exact descriptor, reply, guards, and MSI-X completion before ordered
teardown. See
[Native Network Protocol Egress Transmit v0](network-egress-transmit-v0.md).

Bootstrap v3 replaces destructive pre-transmit consumption with a
generation-bound claim and post-completion acknowledgement. A deliberately
faulted driver proves that full device teardown releases the claim while
preserving the packet; a fresh generation then transmits and acknowledges the
same sequence. See
[Reliable Protocol Egress v0](network-reliable-egress-v0.md).

## Deliberate limits

The empty-queue generations publish no descriptor. The transmit generation
sends only one fixed probe. The receive generation performs only one fixed ARP
exchange with the deterministic QEMU gateway; the replenishment generation
reuses one descriptor exactly once. Packet handoff is one copied four-entry
FIFO with one bound producer and sequential consumers. Exact-sequence blocking
is available, control-plane management can stop a blocked consumer, and a
bounded relative timeout can release its exact sequence. A full FIFO also
blocks and wakes its bound producer without polling. Self-cancellation,
concurrent producers, dynamic configuration, claim deadlines, egress-producer
backpressure, peer reception of the protocol egress proof, and container networking remain separate
milestones.
