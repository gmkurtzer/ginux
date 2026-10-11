# Boot Handoff Contract v3

**Status:** Experimental; internal to the unified EFI image.

Version 3 retains the complete v2 prefix and appends the physical base and
mapped byte size of the unified EFI image. Before acquiring the final memory
map, the boot component resolves its own UEFI Loaded Image Protocol and rejects
a missing protocol, null base, zero size, conversion overflow, or end-address
overflow. It performs no protocol calls between the successful final
`GetMemoryMap` and `ExitBootServices` operations.

The kernel requires the complete v3 structure, bounds the image inside the
transitional 4 GiB identity map, and parses it as PE32+ for AMD64. Parsing is
bounded by the handed image size and rejects invalid DOS/PE signatures,
truncated headers, zero or excessive section counts, an undersized optional
header, invalid section alignment, out-of-range virtual sections, and any
section marked both writable and executable. At least one executable section
must exist.

This establishes authoritative image-section boundaries for kernel W^X
enforcement. The paging builder makes only validated executable PE sections and
UEFI runtime-service code executable; executable PE pages are read-only. The
complete in-memory PE image is split into 4 KiB leaves: headers and sections
without the PE write characteristic are read-only, while only validated
writable-section pages retain write permission. Static page-table pages apply
an even stricter read-only override after construction. The image bounds are
firmware-provided metadata and are not a cryptographic
measurement. The manifest and control-plane bundle retain the v2 signature and
measurement rules.
