# Native Network Service Runtime v0

**Status:** A freestanding x86-64 service artifact is authenticated and
materialized into a bounded W^X address space; execution is pending.

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

## Deliberate limits

This milestone does not yet execute the artifact, delegate device
capabilities, map MMIO or DMA, or move packet processing out of the kernel.
Each boundary is introduced and tested separately in subsequent milestones.
