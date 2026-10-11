# Boot Handoff Contract v4

**Status:** Experimental; internal to the unified EFI image.

Version 4 retains the complete v3 prefix and appends a pointer and exact byte
size for the independently measured root image. The UEFI component passes the
generated read-only `newc` archive without copying or interpreting it.

The signed manifest binds the root image's nonzero size and SHA-256 digest.
Before using any archive metadata, the kernel bounds the complete range inside
the transitional identity map, hashes the exact handed bytes, and requires
both fields to match the manifest. The range is included in initial page-table
construction and must resolve only through present, read-only, non-executable
identity leaves before and after the ring-3 verification workload.

This separates root-image authority from the kernel parser: changing archive
bytes requires regenerating and signing the manifest. The proof image remains
compiled into the unified EFI binary for development convenience. Loading a
separate file from an EFI system partition, production signing, rollback
policy, and measured extraction into owned root pages remain future contracts.

All v3 PE-image inspection, UEFI memory-map, ACPI, manifest, control-plane
bundle, and firmware-exit requirements remain unchanged.
