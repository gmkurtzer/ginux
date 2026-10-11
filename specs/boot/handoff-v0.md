# Boot Handoff Contract v0

**Status:** Superseded by `handoff-v1.md`.

## Purpose

Define the minimum contract by which the Ginux UEFI component ends firmware boot services and transfers control to the kernel. This contract is intentionally small and will remain experimental until the kernel's physical-memory and virtual-address models are specified.

## Entry

The boot component invokes:

```rust
unsafe extern "sysv64" fn kernel_entry(boot_info: *const BootInfo) -> !
```

The call occurs only after `ExitBootServices()` returns `EFI_SUCCESS`. The kernel does not return to the boot component.

## `BootInfo` v0

`BootInfo` has C layout and contains:

- Base address and byte length of the final UEFI memory map.
- Size and version of each UEFI memory descriptor.
- The UEFI `ResetSystem` runtime-service entry used for controlled shutdown during early development.

The memory-map buffer is aligned, owned by the unified image, and remains allocated after boot services end. The kernel treats the structure and map as immutable.

## Boot-component obligations

1. Perform all firmware-console and allocating boot-service operations before obtaining the final memory map.
2. Obtain the final map and its key into an image-owned buffer.
3. Invoke `ExitBootServices()` without an intervening firmware operation.
4. If the key is stale, reacquire the map and retry once.
5. Transfer only valid pointers and descriptor metadata.
6. Make no boot-service calls after successful exit.

## Kernel obligations

1. Reject a null `BootInfo` or memory-map pointer.
2. Reject a zero-length map or descriptor size.
3. Reject a map length that is not a multiple of the descriptor size.
4. Treat the map as immutable until it has established its own physical-memory inventory.
5. Avoid assuming a fixed UEFI descriptor size; step through the map using the supplied size.

## Current limitations

- The kernel switches to an image-owned 64 KiB stack and then replaces the firmware page tables with the transitional mapping defined in `specs/kernel/paging-v0.md`.
- The map uses a fixed 128 KiB handoff buffer.
- No framebuffer, ACPI root pointer, command line, manifest digest, or native system bundle metadata is passed yet.
- `ResetSystem` is retained only to make early automated runs terminate cleanly.

The next contract revision must address versioned extensibility and the additional platform information required to leave all remaining firmware execution assumptions behind.
