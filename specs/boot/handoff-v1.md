# Boot Handoff Contract v1

**Status:** Experimental; internal to the unified EFI image.

## Header and compatibility

`BootInfo` begins with a fixed 64-bit magic value, a version, and the byte size
of the structure. The kernel requires the v1 magic and version and rejects a
structure smaller than its known v1 layout. Future compatible extensions may
append fields and increase the size; incompatible semantics require a new
version.

## Payload

The v1 handoff contains:

- the final UEFI memory-map pointer, byte length, descriptor stride, and
  descriptor version;
- the ACPI root-system-description pointer selected from the UEFI
  configuration tables, preferring the ACPI 2.0 GUID over ACPI 1.0; and
- the UEFI `ResetSystem` runtime-service entry retained for automated early
  boot shutdown.

The kernel validates the RSDP signature, ACPI 1.0 checksum, extended length and
checksum when present, arithmetic bounds, and coverage by its transitional
4 GiB identity map before activating its page tables.

## Lifetime and ownership

The boot component calls `ExitBootServices` before entry and makes no later
boot-service calls. The handoff, memory-map storage, ACPI tables, and runtime
service pointer are immutable inputs. The kernel copies or reserves any data it
needs before reclaiming loader, firmware, or ACPI-reclaimable memory.

## Limitations

- ACPI child tables are not parsed yet.
- The structure has no manifest, bundle, command line, framebuffer, random
  seed, or integrity metadata.
- Physical pointers depend on the current transitional identity map.
- `ResetSystem` must be replaced by a kernel-owned shutdown path before the
  runtime-services mapping can be discarded.
