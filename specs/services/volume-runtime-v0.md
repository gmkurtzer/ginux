# Isolated Volume Runtime v0

**Status:** Freestanding x86-64 service artifact builds, is authenticated and
validated as a bounded native PIE, and is materialized under its final W^X
permissions; live service execution remains the next milestone.

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
to ring 3. A separate read-only/NX bootstrap page and writable/NX stack sit
between absent guard pages. The automated boot proof activates this root,
reads the relocated entrypoint through its userspace address, restores the
kernel root, and scrubs and releases every temporary frame.

## Bootstrap contract

The entry shim accepts a pointer in `RBX` to a read-only `BootInfo` containing
the service-visible virtio common-configuration address, queue-notification
address, owned DMA virtual and physical addresses, and opaque object-7
capability handle. The eventual native loader must relocate the static PIE,
map load segments W^X, map that record read-only, and supply only those bounded
resources.

The runtime itself negotiates only `FLUSH` and `VERSION_1`, configures queue
zero and MSI-X table entry zero, and implements the shared whole-sector trait
with one descriptor head. Each submission publishes a monotonically advancing
available index, notifies queue zero, and blocks through control ABI v4
operation 19 using the exact next request generation. Used ID, used length,
status, and device status are validated before returning to the shared volume
engine.

Volume slots map to reserved device sectors three and four. On empty media the
shared engine commits generation one containing `ginux-isolated-volume\n` via
prepare/write/flush/commit/write/flush and recovers it. On existing media it
accepts only that exact generation and payload. Success deliberately faults so
the supervisor can exercise the same mandatory device-quiescence path.

## Deliberate limits

- The artifact is authenticated, parsed, and materialized, but the resulting
  address space is not yet registered or executed. The existing raw ring-3
  proof still owns the live virtio path.
- `BootInfo` is an internal bootstrap structure, not the stable driver ABI.
- One queue, one DMA page, one outstanding request, and one object-7 device are
  supported. There is no IOMMU domain or concurrent request path.
- Cancellation after notification and late completion after timeout are not
  handled yet.
