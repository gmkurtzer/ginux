# Initial Physical-Memory Classification v0

**Status:** Experimental; inventory only, with no allocator enabled.

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

Before enabling allocation, the kernel must normalize overlapping or adjacent ranges, reject arithmetic overflow, reserve its image and handoff objects explicitly, and define when boot-service ranges become eligible for reuse.
