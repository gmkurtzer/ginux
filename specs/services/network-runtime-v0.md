# Native Network Service Runtime v0

**Status:** A freestanding x86-64 service artifact is authenticated and its
bounded ELF load plan is accepted; materialization and execution are pending.

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

## Deliberate limits

This milestone does not yet materialize the artifact into a W^X domain,
execute it, delegate device capabilities, map MMIO or DMA, or move packet
processing out of the kernel. Each boundary is introduced and tested
separately in subsequent milestones.
