# Boot Handoff Contract v5

**Status:** Experimental; internal to the unified EFI image.

Version 5 retains the complete v4 handoff and appends a pointer and exact byte
size for the resident controller ELF embedded in `GINUX.EFI`.

Manifest version 4 binds the controller's nonzero size and SHA-256 digest.
Before ELF inspection or execution, the kernel bounds the complete range in
the transitional identity map and requires its digest to match. The range must
remain present, read-only, and non-executable in the kernel identity map.

After the bootstrap proof suite, the kernel materializes the static PIE into a
fresh W^X user address space, delegates only a read/write console capability,
and transfers execution at ring 3. Clean controller exit requests the existing
firmware shutdown path. Controller faults or invalid exit status fail the boot
gate rather than being mistaken for an administrative shutdown.

All v4 PE inspection, memory-map, ACPI, manifest, bootstrap bundle, root image,
and firmware-exit requirements remain unchanged.
