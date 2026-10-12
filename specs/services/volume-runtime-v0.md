# Isolated Volume Runtime v0

**Status:** Freestanding x86-64 service artifact builds, is authenticated and
validated as a bounded native PIE, materialized under its final W^X permissions,
and executed on the interrupt-driven virtio path.

## Artifact

`make volume-service` builds `ginux-volume-service-runtime` for Rust's
`x86_64-unknown-none` target. The result is a stripped static PIE ELF with
separate read-only, RX, and RW load segments and a non-executable stack. It has
no host libc, allocator, filesystem, firmware, or kernel-library dependency.
The runtime links the same `ginux-volume-format` and `ginux-volume-service`
`no_std` crates used by the kernel reference backend.

Manifest generation places the artifact at the internal `native/volume` path in the
deterministic initramfs. The existing root-image digest therefore authenticates
the exact service bytes before the kernel copies and publishes them as its
immutable measured root. No corresponding top-level directory is published, so
the native artifact does not become part of the Linux container's root view.

The kernel accepts only a bounded 64 KiB x86-64 static PIE with one read-only,
one RX, and one RW load segment, non-overlapping pages, a non-executable stack,
full RW `GNU_RELRO`, immediate binding, and at most eight relative-only
relocations. Unsupported headers, dynamic tags, writable executable mappings,
symbol relocations, and out-of-range relocation targets or addends fail closed.

The loader copies the six-page image into exclusively owned zeroed frames,
applies the validated relative relocations through the supervisor direct map,
and then exposes only read-only/NX, RX, and post-relocation read-only/NX pages
to ring 3. A separate read-only/NX bootstrap page and bounded two-page
writable/NX stack are separated from the image and adjacent mappings by absent
guard pages. The automated boot proof activates this root,
reads the relocated entrypoint through its userspace address, restores the
kernel root, and scrubs and releases every temporary frame.

## Bootstrap contract

The entry shim accepts a pointer in `RBX` to the read-only, fixed 64-byte
`NativeServiceBootV0` defined by the shared `ginux-native-abi` crate and its C
binding. Kernel and service both validate its magic, version, size, exact
`VIRTIO_BLOCK` feature bitmap, nonzero resource fields, and reserved-zero rule.
It contains the service-visible virtio common-configuration and notification
addresses, owned DMA virtual and device-visible addresses, and opaque object-7
capability handle. The loader relocates the static PIE, maps load segments W^X,
maps the record read-only/NX, and supplies only those bounded resources. See
[Native Service Bootstrap ABI v0](../abi/native-service-bootstrap-v0.md).
The terminal path uses the separately frozen
[Native Service Lifecycle ABI v0](../abi/native-service-lifecycle-v0.md).

The runtime itself negotiates only `FLUSH` and `VERSION_1`, configures queue
zero and MSI-X table entry zero, and implements the shared whole-sector trait
with one descriptor head. The first submission rejects a stale future request,
publishes its available entry without notifying the device, and proves a real
100 ms APIC timeout does not consume the request generation. It then notifies
queue zero, retries that same generation successfully, and rejects a duplicate
wait after completion. Every subsequent submission publishes a monotonically
advancing available index, notifies queue zero, and blocks through control ABI
v4 operation 19 command 2 using the exact next request generation. Used ID, used length,
status, and device status are validated before returning to the shared volume
engine.

Volume slots map to reserved device sectors four and five. On empty media the
shared engine commits generation one containing `ginux-isolated-volume\n` via
prepare/write/flush/commit/write/flush and recovers it. On existing media it
accepts only that exact generation and payload. Success exits through control
ABI operation 2 with `COMPLETED`; the supervisor cross-checks that status
against the final DMA state before accepting initialization or recovery.

Corrupt, ambiguous, exhausted, or valid-but-policy-mismatched media exits with
`REJECTED` after the two-slot scan. The kernel accepts that result only when the
DMA page contains a complete two-read scan. Device errors and runtime failures
still produce a real fault and remain fatal to this boot proof. A separate QEMU
boot corrupts sector four, requires rejection without a write or kernel panic,
verifies the disk checksum is unchanged, and still requires complete device
and DMA teardown.

After the first domain is fully retired, the supervisor starts the same
measured artifact under a fresh domain generation while preserving its logical
service identity. The restart constructor refuses a live registry slot or a
remaining frame quarantine, creates a fresh capability table from the control
plane's original authority, and allocates a new address space and DMA lease.
The replacement must recover the exact record after initialization or repeat
the same fail-closed rejection on corrupt media. Its second clean exit must
again leave the device reset, authority revoked, and every owned frame scrubbed
and released. An independent stop-only auxiliary still executes a genuine
invalid-opcode fault on every ordinary QEMU boot, proving that fault lifecycle
metadata and containment remain distinct from clean completion. A third run of
the compiled service receives a structurally valid but intentionally invalid
opaque device handle. It configures and notifies its queue, cannot authorize
the wait, and executes its real fatal `HLT` path. The kernel accepts only the
measured `HLT` fault, quiesces the device without assuming a completed request,
scrubs the incomplete DMA page, and revokes authority. Only after complete
retirement, it starts a fourth generation with the same logical identity but a
fresh capability table, address space, and DMA lease. That post-fault
generation must recover the exact record or preserve fail-closed rejection,
then cleanly retire while the kernel continues booting.

A separate cancellation generation repeats the initial prepared-request
timeout, notifies that request, and has its MSI-X delivery deliberately withheld
until the APIC deadline. Because the request was notified, the kernel consumes
the attempt, resets the device, returns `CANCELLED`, and accepts only the
service's `RESTART_REQUIRED` exit. A fresh post-cancellation generation must
recover the exact record or preserve corruption rejection before clean exit.

## Deliberate limits

- The compiled service is the sole live write-capable ring-3 virtio runtime;
  the earlier embedded machine-code durability program has been removed.
- ABI v0 is stable but exact-version only; feature negotiation beyond the
  single virtio-block bit requires a later specified ABI version.
- One queue, one DMA page, one outstanding request, and one object-7 device are
  supported. There is no IOMMU domain or concurrent request path.
- Notified timeout recovery is reset-and-restart only. In-place queue recovery,
  selective descriptor cancellation, and concurrent late completions are not
  supported.
