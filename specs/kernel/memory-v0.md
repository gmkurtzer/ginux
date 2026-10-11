# Initial Physical-Memory Classification v0

**Status:** Experimental monotonic allocator enabled below 4 GiB.

After `ExitBootServices()`, the kernel walks the final UEFI memory map using the descriptor stride supplied by firmware. It does not assume that the stride equals the size of the descriptor prefix understood by the kernel.

The initial classification is deliberately conservative:

| UEFI memory type | Initial classification |
|---|---|
| `EfiConventionalMemory` | Available for a future physical-frame allocator |
| `EfiBootServicesCode`, `EfiBootServicesData` | Reclaimable, but not allocated yet |
| `EfiLoaderCode`, `EfiLoaderData` | Reserved because they may contain the unified image, boot information, memory map, or kernel stack |
| All other types | Reserved until explicitly modeled |

The kernel validates that:

- The map pointer and map length are nonzero.
- The descriptor stride is large enough for the required UEFI descriptor prefix.
- The total map length is an exact multiple of the supplied stride.
- Page totals use saturating arithmetic while parsing untrusted firmware data.

The initial allocator sorts and merges conventional ranges, rejects range arithmetic overflow, excludes the first MiB, and limits allocations to the currently mapped physical ceiling. Allocation is monotonic and returns distinct 4 KiB frames; deallocation and boot-service-memory reclamation are not yet supported. The QEMU gate allocates two frames, verifies independent volatile access, and clears both frames.

Loader regions containing the unified image, handoff map, page tables, and kernel stacks are never admitted because only `EfiConventionalMemory` ranges enter the allocator.
